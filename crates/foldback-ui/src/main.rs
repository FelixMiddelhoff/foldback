// SPDX-License-Identifier: MIT OR Apache-2.0
// Foldback UI v1: Tauri shell over `.foldback` session files, offline or
// live (foldback-protocol-spec.md §2).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod live;
mod session;

use std::path::PathBuf;

use session::SessionData;

#[tauri::command]
fn load_session(path: String) -> Result<SessionData, session::LoadError> {
    session::load(&PathBuf::from(path))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(live::LiveConnection::default())
        .invoke_handler(tauri::generate_handler![
            load_session,
            live::connect_live,
            live::disconnect_live
        ])
        .run(tauri::generate_context!())
        .expect("error while running foldback-ui");
}
