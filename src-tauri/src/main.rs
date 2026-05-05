// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod server;
mod service;

fn main() {
    // let mut service = LocalCommService::new("_localcomm._tcp.local.");
    // service.start();

    let _ = localcomm_app_lib::run();
}
