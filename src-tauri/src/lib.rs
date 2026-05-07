use crate::core::device::LocalCommDevice;
use crate::localcomm::local_comm_client::LocalCommClient;
use crate::localcomm::{Empty, GetDeviceListRequest};
use crate::server::LocalCommServerApp;
use crate::service::LocalCommService;
use jni::objects::{JClass, JString};
use jni::EnvUnowned;
use serde::Serialize;
use std::error::Error;
use std::path::PathBuf;
use std::string::String;
use std::sync::OnceLock;
use tauri::{Emitter, Manager};
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tonic::Request;

mod core;
mod server;
mod service;
pub mod localcomm {
    tonic::include_proto!("localcomm");
}

#[tauri::command]
async fn test_discovery(text: &str) -> Result<(), ()> {
    let ip = "0.0.0.0";
    println!("Sending text \"{}\" to {}:50051", text, ip.to_string());
    let mut client = LocalCommClient::connect(format!("http://{}:50051", ip.to_string()))
        .await
        .unwrap();
    let response = client
        .get_device_list(GetDeviceListRequest {})
        .await
        .expect("Failed to send request");

    response.into_inner().list.iter().for_each(|device| {
        println!("Received device: {}", device.name);
    });

    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceListChangedEventPayload {
    list: Vec<LocalCommDevice>,
}

static CANCEL_DISCOVERY_TOKEN: OnceLock<CancellationToken> = OnceLock::new();
#[tauri::command]
async fn discover(app_handle: tauri::AppHandle) -> Result<(), ()> {
    let mut client = LocalCommClient::connect("http://0.0.0.0:50051")
        .await
        .unwrap();

    let mut stream = client
        .listen_for_devices(Empty {})
        .await
        .unwrap()
        .into_inner();

    let token = CancellationToken::new();
    CANCEL_DISCOVERY_TOKEN.set(token.clone()).ok();

    println!("[TAURI_COMMAND_DISCOVERY] Discovering...");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                println!("Discovery stopped");
                break;
            }

            item = stream.next() => {
                match item {
                    Some(Ok(item)) => {
                        let list = item.list;
                        let list: Vec<LocalCommDevice> =
                            list.into_iter().map(LocalCommDevice::from).collect();
                        println!("[TAURI_COMMAND_DISCOVERY] Device list changed {:?}", list);

                        let _ = app_handle.emit("device-list-changed", list);
                    }
                    Some(Err(e)) => {
                        println!("Stream error: {e}");
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn cancel_discovery() {
    if let Some(token) = CANCEL_DISCOVERY_TOKEN.get() {
        token.cancel();
    }
}

struct AppData {
    client: LocalCommClient<tonic::transport::Channel>,
}

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
            let (tx, rx) = watch::channel(initial_devices);

            let mut service = LocalCommService::new(tx, "_localcomm._tcp.local.");
            service.start();

            let server = LocalCommServerApp::serve(
                rx,
                service.devices.clone(),
                download_path,
                app_data_path,
            );

            tokio::select! {
                _ = server => {},
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
    cancel_discovery();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_safe_area_insets_css::init())
        .invoke_handler(tauri::generate_handler![test_discovery, discover, cancel_discovery])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("Done!");
}
