use crate::server::localcomm::local_comm_client::LocalCommClient;
use crate::server::localcomm::{GetDeviceListRequest, TextTypeRequest};
use crate::server::LocalCommServerApp;
use crate::service::LocalCommService;
use jni::objects::{JClass, JString};
use jni::EnvUnowned;
use local_ip_address::local_ip;
use std::error::Error;
use std::path::PathBuf;
use std::string::String;
use std::sync::{Mutex, OnceLock};
use tauri::{generate_handler, AppHandle, Manager};
use tauri::process::restart;
use tokio_util::sync::CancellationToken;

mod core;
mod server;
mod service;

#[tauri::command]
async fn test_discovery(text: &str) -> Result<(), ()> {
    let ip = local_ip().unwrap();
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
            let _app_data_path = PathBuf::from(app_data_path.to_string());

            let mut service = LocalCommService::new("_localcomm._tcp.local.");
            service.start();

            let server = LocalCommServerApp::serve(service.devices.clone(), download_path);

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
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![test_discovery])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("Done!");
}
