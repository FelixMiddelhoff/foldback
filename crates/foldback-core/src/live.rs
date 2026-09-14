// SPDX-License-Identifier: MIT OR Apache-2.0
//! Live-mode transport (protocol spec §2): a loopback WebSocket a game
//! embeds so the UI can watch a session as it happens, not just after
//! recording a `.foldback` file. Blocking I/O throughout — but entirely
//! on a background thread this module owns, so the *game's* thread never
//! blocks (the one hard requirement in the spec: "must never stall the
//! game loop waiting for a UI to connect"). The background thread
//! blocking on `accept()`/handshake/`recv()` is fine; it isn't the game
//! loop.
//!
//! Binary frames use `Frame::write_to`'s exact byte layout — deliberately
//! the same as a `.foldback` file's frames, so a reader has one parser
//! for both live and offline data, not two (protocol spec §2.1).
//!
//! `start_recording` (spec §2.2 step 5, UI asks the game to record to a
//! file while also streaming live) isn't implemented here — this module
//! covers hello + frame streaming + reconnect, the core of live mode; a
//! game that also wants simultaneous file recording can already do that
//! itself with a second `Session::builder().record_to(path)`.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Sender};
use std::thread;

use serde::Serialize;
use tungstenite::{Message, WebSocket};

use crate::format::{Frame, FORMAT_VERSION};

/// A running live-mode server. Dropping this closes the channel to the
/// background thread, which then exits after its current connection (if
/// any) drops.
pub struct LiveServer {
    tx: Sender<Frame>,
    local_addr: SocketAddr,
}

impl LiveServer {
    /// Binds the listen socket immediately, so a port-in-use error
    /// surfaces synchronously to the caller, then spawns the background
    /// thread that owns the rest of the connection lifecycle. Binding to
    /// port 0 (let the OS pick one) works — read it back with
    /// [`LiveServer::local_addr`].
    pub fn bind<A: ToSocketAddrs>(
        addr: A,
        tick_rate_hz: u32,
        peer_count: u32,
        build_id: [u8; 16],
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;
        let (tx, rx) = mpsc::channel::<Frame>();
        thread::spawn(move || run(listener, rx, tick_rate_hz, peer_count, build_id));
        Ok(LiveServer { tx, local_addr })
    }

    /// The address actually bound — the same as what was passed to
    /// `bind`, except with port 0 resolved to whatever the OS assigned.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Queues a frame to stream to the connected UI, if any. Never
    /// blocks the caller — an unbounded channel send — and silently
    /// drops the frame if no UI is connected yet or the background
    /// thread has already exited.
    pub fn send_frame(&self, frame: Frame) {
        let _ = self.tx.send(frame);
    }
}

#[derive(Serialize)]
struct Hello {
    #[serde(rename = "type")]
    kind: &'static str,
    protocol_version: u16,
    tick_rate_hz: u32,
    peer_count: u32,
    build_id: String,
}

fn run(
    listener: TcpListener,
    rx: mpsc::Receiver<Frame>,
    tick_rate_hz: u32,
    peer_count: u32,
    build_id: [u8; 16],
) {
    let build_id_hex: String = build_id.iter().map(|b| format!("{b:02x}")).collect();

    loop {
        let Ok((stream, _addr)) = listener.accept() else {
            continue; // spurious accept error — keep listening
        };
        let Ok(mut socket) = tungstenite::accept(stream) else {
            continue; // failed WS handshake — wait for the next attempt
        };
        if send_hello(&mut socket, tick_rate_hz, peer_count, &build_id_hex).is_err() {
            continue;
        }

        // Stream frames until this connection drops (reconnect by going
        // back to `accept()`) or the game side has shut down (`rx.recv()`
        // returns `Err` once every `Sender` — i.e. the `LiveServer` — is
        // dropped), in which case this thread exits for good.
        loop {
            match rx.recv() {
                Ok(frame) => {
                    if send_frame(&mut socket, &frame).is_err() {
                        break;
                    }
                }
                Err(_) => return,
            }
        }
    }
}

fn send_hello(
    socket: &mut WebSocket<TcpStream>,
    tick_rate_hz: u32,
    peer_count: u32,
    build_id_hex: &str,
) -> tungstenite::Result<()> {
    let hello = Hello {
        kind: "hello",
        protocol_version: FORMAT_VERSION,
        tick_rate_hz,
        peer_count,
        build_id: build_id_hex.to_string(),
    };
    let text = serde_json::to_string(&hello).expect("Hello always serializes");
    socket.send(Message::Text(text.into()))
}

fn send_frame(socket: &mut WebSocket<TcpStream>, frame: &Frame) -> tungstenite::Result<()> {
    let mut buf = Vec::new();
    frame
        .write_to(&mut buf)
        .expect("writing a Frame to an in-memory Vec never fails");
    socket.send(Message::Binary(buf.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::FrameReader;
    use std::io::Cursor;
    use std::time::Duration;
    use tungstenite::connect;

    #[test]
    fn a_connecting_client_receives_hello_then_streamed_frames_in_order() {
        let server = LiveServer::bind("127.0.0.1:0", 60, 2, [7u8; 16]).unwrap();
        let port = server.local_addr().port();

        // Give the background thread a moment to reach `accept()`.
        thread::sleep(Duration::from_millis(50));

        let (mut ws, _resp) = connect(format!("ws://127.0.0.1:{port}")).unwrap();

        let hello_msg = ws.read().unwrap();
        let hello_text = hello_msg.into_text().unwrap();
        let hello: serde_json::Value = serde_json::from_str(&hello_text).unwrap();
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["tick_rate_hz"], 60);
        assert_eq!(hello["peer_count"], 2);
        assert_eq!(hello["build_id"], "07070707070707070707070707070707");

        let sent = vec![
            Frame::TickHash {
                tick: 1,
                peer_id: 0,
                hash: 0xdead_beef,
            },
            Frame::TickHash {
                tick: 2,
                peer_id: 0,
                hash: 0xfeed_face,
            },
        ];
        for f in &sent {
            server.send_frame(f.clone());
        }

        for expected in &sent {
            let msg = ws.read().unwrap();
            let bytes = msg.into_data();
            let mut reader = FrameReader::new(Cursor::new(bytes.to_vec()));
            let got = reader.next_frame().unwrap().unwrap();
            assert_eq!(&got, expected);
        }
    }

    #[test]
    fn dropping_the_server_lets_the_background_thread_exit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel::<Frame>();
        let handle = thread::spawn(move || run(listener, rx, 60, 1, [0u8; 16]));

        thread::sleep(Duration::from_millis(50));
        let (mut ws, _resp) = connect(format!("ws://127.0.0.1:{port}")).unwrap();
        ws.read().unwrap(); // hello

        drop(tx); // simulates dropping LiveServer
        handle.join().unwrap();
    }
}
