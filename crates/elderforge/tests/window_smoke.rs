//! Windowing smoke test: launches the elderforge binary with `--smoke-test`,
//! which opens the editor window, runs 30 frames, and exits.
//!
//! Runs as a subprocess because winit event loops must live on a process's
//! main thread (a hard requirement on macOS), and `#[test]` functions run
//! on worker threads. Requires a GUI session — this will fail on a headless
//! machine.

use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn window_opens_for_30_frames_and_exits_clean() {
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_elderforge"))
        .arg("--smoke-test")
        .output()
        .expect("failed to launch elderforge binary");

    assert!(
        output.status.success(),
        "smoke test exited with {:?}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    // 30 frames should be over in moments; a long run means the frame cap
    // never fired and something else killed the process.
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "smoke test took too long; frame-cap exit did not trigger"
    );
}
