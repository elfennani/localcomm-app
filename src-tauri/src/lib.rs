use crate::server::LocalCommServerApp;
use crate::service::LocalCommService;
use jni::objects::{JClass, JString};
use jni::EnvUnowned;
use serde::Serialize;
use sqlx::{Connection, Executor, SqliteConnection};
use std::path::PathBuf;
use std::string::String;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{Migration, MigrationKind};

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

#[no_mangle]
pub extern "system" fn Java_com_elfen_localcomm_app_MainActivity_hello<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass,
    absolutePath: JString<'caller>,
) -> JString<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<_, jni::errors::Error> {
        let absolute_path: String = absolutePath.to_string();
        let absolute_path = PathBuf::from(absolute_path);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = LocalCommServerApp::serve(Arc::default(), absolute_path.clone());
            server.await.unwrap();
        });

        JString::from_str(env, absolute_path.to_str().unwrap())
    });

    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
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
