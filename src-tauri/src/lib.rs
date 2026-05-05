use crate::server::LocalCommServerApp;
use crate::service::LocalCommService;
use jni::objects::{JClass, JString};
use jni::EnvUnowned;
use serde::Serialize;
use sqlx::{Connection, Executor};
use std::path::PathBuf;
use std::string::String;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{Migration, MigrationKind};
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

mod core;
mod server;
mod service;

#[tauri::command]
fn discover(app: &AppHandle, name: &str) {
    let app_data = app.state::<AppData>();
    app_data.welcome_message;
}

struct AppData {
    welcome_message: &'static str,
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

async fn listen_signal() {
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sighup = signal(SignalKind::hangup()).unwrap();
    let mut sigusr1 = signal(SignalKind::user_defined1()).unwrap();
    let mut sigusr2 = signal(SignalKind::user_defined2()).unwrap();

    println!("PID: {}", std::process::id());

    loop {
        tokio::select! {
            _ = sigint.recv()  => println!("SIGINT"),
            _ = sigterm.recv() => println!("SIGTERM"),
            _ = sighup.recv()  => println!("SIGHUP"),
            _ = sigusr1.recv() => println!("SIGUSR1"),
            _ = sigusr2.recv() => println!("SIGUSR2"),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let migrations = vec![Migration {
        version: 1,
        description: "create_initial_tables",
        sql: "\
        CREATE TABLE devices (\
            name TEXT NOT NULL PRIMARY KEY, \
            ip TEXT NOT NULL, \
            paired BOOLEAN NOT NULL DEFAULT FALSE \
        );"
        .trim(),
        kind: MigrationKind::Up,
    }];

    // conn.execute("DELETE FROM devices WHERE paired = FALSE")
    //     .await?;

    // Spawn mDNS Service
    // tokio::spawn(async {
    //     let mut service = LocalCommService::new("_localcomm._tcp.local.");
    //     service.start();
    // });

    tauri::Builder::default()
        // .manage(AppData {
        //     welcome_message: "Welcome to Tauri!",
        // })
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:mydatabase.db", migrations)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        // .invoke_handler(tauri::generate_handler![discover])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    println!("Done!");

    Ok(())
}
