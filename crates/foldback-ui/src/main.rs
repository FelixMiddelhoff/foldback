// SPDX-License-Identifier: MIT OR Apache-2.0
// Foldback UI v1: Tauri shell over `.foldback` session files. Offline
// replay only — live mode is Phase 2 (foldback-protocol-spec.md §2).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
        .invoke_handler(tauri::generate_handler![load_session])
        .run(tauri::generate_context!())
        .expect("error while running foldback-ui");
}
