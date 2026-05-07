use crate::core::device::LocalCommDevice;
use crate::service::LocalCommService;
use crate::server::LocalCommServerApp;
use std::path::PathBuf;
use tokio::signal::ctrl_c;
use tokio::sync::watch;

mod core;
mod server;
mod service;
mod client;
pub mod localcomm {
    tonic::include_proto!("localcomm");
}

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
    let (tx, rx) = watch::channel(initial_devices);

    let mut service = LocalCommService::new(tx, "_localcomm._tcp.local.");
    service.start();

    let server =
        LocalCommServerApp::serve(rx, service.devices.clone(), download_path, app_data_path);

    tokio::select! {
        _ = server => {},
        _ = ctrl_c() => {
            println!("Server terminated");
        },
    };

    service.stop();
}
