pub mod payload;

use crate::core::device::LocalCommDevice;
use crate::websocket::payload::{EventPayload, ResponsePayload};
use anyhow::Context;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use futures_util::sink::SinkExt;
use futures_util::stream::{SplitSink, StreamExt};
use rusqlite::fallible_iterator::FallibleIterator;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Serialize, Deserialize)]
pub struct Envelope<T> {
    pub id: String,
    pub payload: T,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum ClientRequest {
    GetConnectedDevices,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Response {
        uuid: String,
        result: ResponsePayload,
    },
    Event {
        payload: EventPayload,
    },
}

#[derive(Clone)]
struct ServerState {
    sender: Arc<Mutex<Option<SplitSink<WebSocket, Message>>>>,
    device_list_receiver: watch::Receiver<Vec<LocalCommDevice>>,
}

pub struct Server {
    app: Router,
    state: ServerState,
}

impl Server {
    pub fn new(device_list_rx: watch::Receiver<Vec<LocalCommDevice>>) -> Self {
        let sender = Arc::default();
        let state = ServerState { sender, device_list_receiver: device_list_rx, };
        let state_clone = state.clone();

        let router = Router::<()>::new().route(
            "/ws",
            any(|ws: WebSocketUpgrade| async move { Self::handler(state_clone, ws).await }),
        );

        Self { app: router, state }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:50051")
            .await
            .context("Error binding 0.0.0.0:50051")?;
        axum::serve(listener, self.app.clone())
            .await
            .context("error serving websocket")?;

        Ok(())
    }

    async fn handler(state: ServerState, ws: WebSocketUpgrade) -> Response {
        ws.on_upgrade(|socket| async move {
            Self::handle_socket(state, socket).await;
        })
    }

    async fn handle_socket(state: ServerState, socket: WebSocket) {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let (mut sender, mut receiver) = WebSocket::split(socket);

        {
            let mut lock = state.sender.lock().await;
            *lock = Some(sender);
        }

        let initiator_handle = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                let _ = tx.send(msg);
            }
        });

        let processor_handle = tokio::spawn(async move {
            while let Some((Message::Text(msg))) = rx.recv().await {
                let req: Envelope<ClientRequest> = match serde_json::from_str(&msg) {
                    Ok(msg) => msg,
                    Err(e) => continue,
                };

                Self::handle_request(state.clone(), req).await;
            }
        });

        tokio::select! {
            _ = initiator_handle => (),
            _ = processor_handle => (),
        }
    }

    async fn handle_request(state: ServerState, request: Envelope<ClientRequest>) {
        match request.payload {
            ClientRequest::GetConnectedDevices => {
                let devices = state.device_list_receiver.borrow().clone();
                let msg = ServerMessage::Response {
                    uuid: request.id.clone(),
                    result: ResponsePayload::GetDeviceList { devices },
                };
                let response = serde_json::to_string(&msg).unwrap();

                let mut lock = state.sender.lock().await;

                if let Some(sender) = lock.as_mut() {
                    sender.send(Message::text(response)).await.unwrap();
                }
            }
        }
    }

    pub async fn send_message(&self, message: ServerMessage) -> anyhow::Result<()> {
        let mut lock = self.state.sender.lock().await;
        let sender = lock.as_mut().context("no sender set!")?;
        let message =
            serde_json::to_string(&message).context("failed to serialize event payload")?;
        sender
            .send(Message::text(message))
            .await
            .context("failed to send websocket message")?;

        Ok(())
    }
}
