// SPDX-License-Identifier: MIT OR Apache-2.0
//! Live-mode WebSocket client — the UI-side half of protocol-spec.md §2.
//! `foldback_core::live::LiveServer` (the game-embeddable half) shipped
//! earlier in Phase 2; this connects to it.
//!
//! Per protocol spec §2.2, either side can disconnect at any time with no
//! protocol-level cleanup required. `disconnect_live` below is the
//! UI-initiated half of that — it shuts down the underlying TCP socket,
//! which unblocks the background thread's `socket.read()` with an error
//! and drives it through the exact same `Disconnected` path a game-side
//! close already takes, rather than inventing a second shutdown path.

use std::io::Cursor;
use std::net::TcpStream;
use std::sync::Mutex;

use foldback_core::format::FrameReader;
use tauri::{AppHandle, Emitter, State};
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

/// Holds a clone of the current live connection's underlying TCP stream,
/// so `disconnect_live` can shut it down from the main thread while the
/// background thread owns the `WebSocket` itself. Shutting down a cloned
/// `TcpStream` closes the shared OS socket, so this needs no channel or
/// cooperative check in the read loop.
#[derive(Default)]
pub struct LiveConnection(Mutex<Option<TcpStream>>);

#[tauri::command]
pub fn connect_live(
    app: AppHandle,
    conn: State<LiveConnection>,
    url: String,
) -> Result<(), String> {
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

    // Always ws:// (protocol spec §2.2 — loopback only, no TLS), so this
    // is always the `Plain` variant.
    let MaybeTlsStream::Plain(tcp) = socket.get_ref() else {
        return Err("live connection was not a plain TCP stream".to_string());
    };
    let tcp_clone = tcp
        .try_clone()
        .map_err(|e| format!("could not clone live connection handle: {e}"))?;
    *conn.0.lock().unwrap() = Some(tcp_clone);

    std::thread::spawn(move || stream_frames(app, socket));
    Ok(())
}

/// Shuts down the current live connection, if any. A no-op (not an
/// error) if nothing is connected — matches the button being available
/// whenever the UI thinks it might be live, without the frontend having
/// to track connection state precisely enough to avoid a double call.
#[tauri::command]
pub fn disconnect_live(conn: State<LiveConnection>) -> Result<(), String> {
    if let Some(tcp) = conn.0.lock().unwrap().take() {
        tcp.shutdown(std::net::Shutdown::Both)
            .map_err(|e| format!("could not disconnect: {e}"))?;
    }
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

#[cfg(test)]
mod tests {
    use foldback_core::live::LiveServer;

    use super::*;

    /// Proves the part of `disconnect_live` that isn't exercised by the
    /// UI-level flow: that shutting down a *cloned* `TcpStream` — the
    /// only handle `disconnect_live` has, since `stream_frames` owns the
    /// actual `WebSocket` — unblocks the other clone's blocking
    /// `socket.read()` with an error, the same way a real disconnect
    /// does. Doesn't need a `Tauri` app at all, so it isolates the one
    /// genuinely risky assumption instead of requiring a full UI run.
    #[test]
    fn shutting_down_cloned_stream_unblocks_blocking_read() {
        let server = LiveServer::bind("127.0.0.1:0", 20, 1, *b"disconnect-test\0").unwrap();
        let port = server.local_addr().port();

        let (mut socket, _response) =
            tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
        socket.read().unwrap(); // hello

        let MaybeTlsStream::Plain(tcp) = socket.get_ref() else {
            panic!("expected a plain (non-TLS) stream for a ws:// loopback connection");
        };
        let tcp_clone = tcp.try_clone().unwrap();

        // Mirrors `stream_frames` owning `socket` on a background thread
        // while `disconnect_live` only holds `tcp_clone`.
        let reader = std::thread::spawn(move || socket.read());

        tcp_clone.shutdown(std::net::Shutdown::Both).unwrap();

        let result = reader
            .join()
            .expect("background read thread should not panic");
        assert!(
            result.is_err(),
            "shutting down the cloned stream should have unblocked the read with an error"
        );
    }
}
