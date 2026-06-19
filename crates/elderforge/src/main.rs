//! Elderforge editor binary: wires platform, renderer, physics, ECS, scene,
//! and editor together. Owns the event loop.

mod app;
mod systems;

use elderforge::demos::Demo;
use elderforge_platform::{EngineEvent, FrameControl, WindowConfig};

fn main() -> anyhow::Result<()> {
    elderforge_core::init_logging();
    log::info!("Elderforge starting");

    // `--smoke-test` runs a fixed number of frames then exits; used by the
    // windowing integration test to verify the loop opens and shuts down clean.
    let smoke_test = std::env::args().any(|arg| arg == "--smoke-test");
    let max_frames = smoke_test.then_some(30u64);

    // `--demo <name>` selects which scene to run; defaults to the stacking demo.
    let demo = parse_demo()?;
    log::info!("running demo '{}'", demo.name());

    let mut app = app::App::new(demo);
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

/// Read the demo named by `--demo <name>` from the command line, defaulting to
/// the stacking demo when the flag is absent. Errors on an unknown name with
/// the list of valid demos.
fn parse_demo() -> anyhow::Result<Demo> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--demo" {
            let name = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--demo requires a name (one of {})", demo_list()))?;
            return Demo::from_name(&name)
                .ok_or_else(|| anyhow::anyhow!("unknown demo '{name}' (expected one of {})", demo_list()));
        }
    }
    Ok(Demo::Stacking)
}

/// Comma-separated list of valid demo names, for error messages.
fn demo_list() -> String {
    Demo::all().iter().map(|d| d.name()).collect::<Vec<_>>().join(", ")
}
