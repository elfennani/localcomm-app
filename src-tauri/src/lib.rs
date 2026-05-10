use crate::core::device::LocalCommDevice;
use crate::service::LocalCommService;
use crate::websocket::payload::{EventPayload, ResponsePayload};
use crate::websocket::{ClientRequest, Envelope, Server, ServerMessage};
use anyhow::{anyhow, Context};
use jni::objects::{JClass, JString};
use jni::EnvUnowned;
use serde::Serialize;
use std::error::Error;
use std::path::PathBuf;
use std::string::String;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::async_runtime::handle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tonic::{async_trait, Request};
use uuid::Uuid;

mod core;
mod service;
mod websocket;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceListChangedEventPayload {
    list: Vec<LocalCommDevice>,
}

static CANCEL_DISCOVERY_TOKEN: OnceLock<CancellationToken> = OnceLock::new();

struct Client {
    app_handle: Option<AppHandle>,
    tx: std::sync::mpsc::Sender<ServerMessage>,
}

impl Client {
    fn new(tx: std::sync::mpsc::Sender<ServerMessage>, app_handle: Option<AppHandle>) -> Self {
        Client { app_handle, tx }
    }
}

#[async_trait]
impl ezsockets::ClientExt for Client {
    type Call = ();

    async fn on_text(&mut self, text: ezsockets::Utf8Bytes) -> Result<(), ezsockets::Error> {
        let response: ServerMessage =
            serde_json::from_str(text.as_str()).context("Failed to deserialize server message")?;

        match response {
            ServerMessage::Event { payload } => {
                if let Some(app_handle) = self.app_handle.as_ref() {
                    app_handle
                        .emit("state-event", payload)
                        .context("Failed to emit event")?;
                }
            }
            ServerMessage::Response { .. } => {
                self.tx.send(response).context("Failed to send response")?;
            }
        }

        Ok(())
    }

    async fn on_binary(&mut self, bytes: ezsockets::Bytes) -> Result<(), ezsockets::Error> {
        Ok(())
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), ezsockets::Error> {
        let () = call;
        Ok(())
    }
}

#[tauri::command]
async fn get_nearby_devices(app_handle: AppHandle) -> Result<Vec<LocalCommDevice>, tauri::Error> {
    let (tx, rx) = std::sync::mpsc::channel::<ServerMessage>();
    let config = ezsockets::ClientConfig::new("ws://localhost:50051/ws");
    let (handle, future) = ezsockets::connect(|_client| Client::new(tx, None), config).await;

    let token = CancellationToken::new();
    let token_child = token.clone();

    // Cancel the request the moment it times out after 30 seconds
    // let timeout = std::time::Duration::from_secs(30);

    tokio::spawn(async move {
        tokio::select! {
            _ = token_child.cancelled() => (),
            _ = future => (),
        }
    });

    let uuid = Uuid::new_v4().to_string();
    let request = Envelope {
        id: uuid.clone(),
        payload: ClientRequest::GetConnectedDevices,
    };

    handle.text(serde_json::to_string(&request).unwrap()).ok();

    let result = timeout(Duration::from_secs(5), async {
        for msg in rx {
            if let ServerMessage::Response {
                uuid: response_uuid,
                result,
            } = msg
            {
                if uuid == response_uuid {
                    return Some(result);
                }
            }
        }

        None::<ResponsePayload>
    })
    .await;

    let result = match result {
        Ok(Some(result)) => result,
        Ok(None) => return Err(tauri::Error::from(anyhow!("Channel closed before response"))),
        Err(_) => return Err(tauri::Error::from(anyhow!("Timed out waiting for device list"))),
    };

    if let ResponsePayload::GetDeviceList { devices } = result {
        Ok(devices)
    } else {
        Err(tauri::Error::from(anyhow!("Wrong response type")))
    }
}

struct AppData {}

static TOKEN: OnceLock<CancellationToken> = OnceLock::new();

#[no_mangle]
pub extern "system" fn Java_com_elfen_localcomm_app_MainActivity_startService<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass,
    download_path: JString<'caller>,
    app_data_path: JString<'caller>,
) {
    let outcome = unowned_env.with_env(|env| -> Result<_, jni::errors::Error> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let token = CancellationToken::new();
            TOKEN.set(token.clone()).ok();

            let download_path = PathBuf::from(download_path.to_string());
            let app_data_path = PathBuf::from(app_data_path.to_string());

            let initial_devices: Vec<LocalCommDevice> = Vec::new();
            let (tx, mut rx) = watch::channel(initial_devices);

            let mut service = LocalCommService::new(tx, "_localcomm._tcp.local.");
            service.start();

            let server = Arc::new(Server::new(rx.clone()));
            let server_clone = server.clone();

            tokio::spawn(async move {
                loop {
                    let device_list = rx.borrow().clone();

                    if let Err(err) = server_clone
                        .send_message(ServerMessage::Event {
                            payload: EventPayload::DeviceListChanged {
                                items: device_list.clone(),
                            },
                        })
                        .await
                    {
                        eprintln!("error sending device list: {}", err);
                    }

                    if rx.changed().await.is_err() {
                        break;
                    }
                }

                Ok::<(), anyhow::Error>(())
            });

            tokio::select! {
                _ = server.serve() => {},
                _ = token.cancelled() => {
                    println!("Server terminated");
                },
            };

            service.stop();
        });

        Ok(())
    });

    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_com_elfen_localcomm_app_MainActivity_stopService(
    mut _unowned_env: EnvUnowned,
    _class: JClass,
) {
    if let Some(token) = TOKEN.get() {
        token.cancel();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_safe_area_insets_css::init())
        .invoke_handler(tauri::generate_handler![get_nearby_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("Done!");
}
