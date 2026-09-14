//! Regenerates the golden `.foldback` fixtures under `tests/fixtures/`.
//! Run with `cargo run -p foldback-cli --example gen_fixtures` whenever the
//! fixture *content* should change — the fixture files themselves are
//! checked into git and diffed by the golden-file tests, not regenerated
//! at test time, so this must be run deliberately and the result reviewed.

use foldback_core::session::Session;

fn main() {
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::create_dir_all(&fixtures_dir).unwrap();

    // Clean: two peers, ten ticks, always agree.
    {
        let path = fixtures_dir.join("clean.foldback");
        let mut s = Session::builder()
            .tick_rate_hz(60)
            .peer_count(2)
            .record_to(&path)
            .build()
            .unwrap();
        for tick in 0..10u64 {
            let hash = 1000 + tick;
            s.record_peer_hash(tick, 0, hash).unwrap();
            s.record_peer_hash(tick, 1, hash).unwrap();
        }
        s.finish_recording().unwrap();
    }

    // Diverging: two peers, ten ticks, disagree starting at tick 6.
    {
        let path = fixtures_dir.join("diverging.foldback");
        let mut s = Session::builder()
            .tick_rate_hz(60)
            .peer_count(2)
            .record_to(&path)
            .build()
            .unwrap();
        for tick in 0..10u64 {
            let hash_a = 2000 + tick;
            let hash_b = if tick >= 6 { hash_a + 9999 } else { hash_a };
            s.record_peer_hash(tick, 0, hash_a).unwrap();
            s.record_peer_hash(tick, 1, hash_b).unwrap();
        }
        s.finish_recording().unwrap();
    }

    // Truncated: same as clean, but the file is cut mid-recording (no
    // EndOfStream) — proves the CLI handles a crashed-mid-recording file
    // without panicking, per the format's truncation-tolerance design.
    {
        let path = fixtures_dir.join("truncated.foldback");
        {
            let mut s = Session::builder()
                .tick_rate_hz(60)
                .peer_count(1)
                .record_to(&path)
                .build()
                .unwrap();
            for tick in 0..5u64 {
                s.hash_tick(tick, format!("state-{tick}").as_bytes())
                    .unwrap();
            }
            // Deliberately no finish_recording() call — simulates a crash.
        }
        let bytes = std::fs::read(&path).unwrap();
        let cut = bytes.len() - 3; // slice into the last frame's payload
        std::fs::write(&path, &bytes[..cut]).unwrap();
    }

    println!("wrote fixtures to {}", fixtures_dir.display());
}
