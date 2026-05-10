use crate::core::device::LocalCommDevice;
use crate::service::LocalCommService;
use crate::websocket::payload::EventPayload;
use crate::websocket::{Server, ServerMessage};
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::sync::watch;

mod core;
mod service;

mod websocket;

#[tokio::main]
async fn main() {
    let user_dirs = directories::UserDirs::new().expect("cannot access user directories");
    let download_path = user_dirs
        .download_dir()
        .expect("cannot find download directory")
        .to_path_buf();
    let mut app_data_path = directories::BaseDirs::new()
        .expect("cannot find app data directory")
        .data_dir()
        .to_path_buf();
    app_data_path.push("localcomm-server");

    if !app_data_path.exists() {
        std::fs::create_dir(&app_data_path).expect("cannot create app data directory");
    }

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
        _ = ctrl_c() => {
            println!("Server terminated");
        },
    };

    service.stop();
}
