// SPDX-License-Identifier: MIT OR Apache-2.0
//! Live-mode WebSocket client — the UI-side half of protocol-spec.md §2.
//! `foldback_core::live::LiveServer` (the game-embeddable half) shipped
//! earlier in Phase 2; this connects to it.
//!
//! No explicit "disconnect" command yet — per protocol spec §2.2, either
//! side can disconnect at any time with no protocol-level cleanup
//! required, and the game side closing (or the app quitting) is the only
//! way a live session ends today. A UI-initiated disconnect button is a
//! reasonable follow-up, not built here to keep this slice to the
//! connect-and-stream path actually needed to prove live mode works.

use std::io::Cursor;
use std::net::TcpStream;

use foldback_core::format::FrameReader;
use tauri::{AppHandle, Emitter};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

use crate::session::{frame_to_live_event, LiveEvent};

const LIVE_EVENT: &str = "live-event";

#[derive(serde::Deserialize)]
struct Hello {
    tick_rate_hz: u32,
    peer_count: u32,
    build_id: String,
}

#[tauri::command]
pub fn connect_live(app: AppHandle, url: String) -> Result<(), String> {
    let (mut socket, _response) =
        tungstenite::connect(&url).map_err(|e| format!("could not connect to '{url}': {e}"))?;

    // The first message must be `hello` (protocol spec §2.2 step 2) —
    // read it synchronously so a malformed handshake surfaces as this
    // command's own error, not a silent background-thread failure.
    let hello_msg = socket
        .read()
        .map_err(|e| format!("connected to '{url}' but failed reading hello: {e}"))?;
    let hello_text = hello_msg
        .into_text()
        .map_err(|_| "hello message was not text".to_string())?;
    let hello: Hello = serde_json::from_str(&hello_text)
        .map_err(|e| format!("hello message was not valid JSON: {e}"))?;

    app.emit(
        LIVE_EVENT,
        LiveEvent::Hello {
            tick_rate_hz: hello.tick_rate_hz,
            peer_count: hello.peer_count,
            build_id: hello.build_id,
        },
    )
    .ok();

    std::thread::spawn(move || stream_frames(app, socket));
    Ok(())
}

fn stream_frames(app: AppHandle, mut socket: WebSocket<MaybeTlsStream<TcpStream>>) {
    loop {
        let msg = match socket.read() {
            Ok(m) => m,
            Err(e) => {
                app.emit(
                    LIVE_EVENT,
                    LiveEvent::Disconnected {
                        reason: e.to_string(),
                    },
                )
                .ok();
                return;
            }
        };
        if !msg.is_binary() {
            continue; // control/ping/pong frames — nothing to render
        }
        // Each binary WS message is exactly one `.foldback`-layout frame
        // (protocol spec §2.1) — the same `FrameReader` a file uses,
        // just handed one already-complete message instead of a stream.
        let bytes = msg.into_data();
        let mut reader = FrameReader::new(Cursor::new(bytes.to_vec()));
        match reader.next_frame() {
            Ok(Some(frame)) => {
                app.emit(LIVE_EVENT, frame_to_live_event(frame)).ok();
            }
            Ok(None) => {}
            Err(e) => {
                app.emit(
                    LIVE_EVENT,
                    LiveEvent::Disconnected {
                        reason: format!("malformed frame: {e}"),
                    },
                )
                .ok();
                return;
            }
        }
    }
}
