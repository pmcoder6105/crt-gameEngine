//! Elderforge editor binary: wires platform, renderer, physics, ECS, scene,
//! and editor together. Owns the event loop.

mod app;
mod systems;

use elderforge_platform::{EngineEvent, FrameControl, WindowConfig};

fn main() -> anyhow::Result<()> {
    elderforge_core::init_logging();
    log::info!("Elderforge starting");

    // `--smoke-test` runs a fixed number of frames then exits; used by the
    // windowing integration test to verify the loop opens and shuts down clean.
    let smoke_test = std::env::args().any(|arg| arg == "--smoke-test");
    let max_frames = smoke_test.then_some(30u64);

    let mut app = app::App::new();
    let mut frame_count = 0u64;
    let mut fatal: Option<anyhow::Error> = None;
    elderforge_platform::run_event_loop(WindowConfig::default(), |_input, events, window| {
        for event in events {
            if let EngineEvent::Resized { width, height } = event {
                app.resize(*width, *height);
            }
        }
        if let Err(err) = app.update(window) {
            fatal = Some(err);
            return FrameControl::Exit;
        }
        frame_count += 1;
        match max_frames {
            Some(max) if frame_count >= max => FrameControl::Exit,
            _ => FrameControl::Continue,
        }
    })?;
    if let Some(err) = fatal {
        return Err(err);
    }

    log::info!("Elderforge exiting");
    Ok(())
}
