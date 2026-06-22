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

    // `--resolution <W>x<H>` forces the launch window size; defaults to 1080p.
    let (width, height) = parse_resolution()?;
    log::info!("launch resolution {width}x{height}");

    // `--borderless` hides the editor and clears to black; `--msaa <N>` enables
    // N× multisampling. Both default off / 1×.
    let borderless = std::env::args().any(|arg| arg == "--borderless");
    let msaa = parse_msaa()?;
    log::info!("borderless={borderless}, msaa={msaa}x");
    let options = app::LaunchOptions { borderless, msaa };

    // A borderless capture window also drops the OS title bar and frame.
    let config = WindowConfig {
        width,
        height,
        decorations: !borderless,
        ..WindowConfig::default()
    };

    let mut app = app::App::new(demo, options);
    let mut frame_count = 0u64;
    let mut fatal: Option<anyhow::Error> = None;
    elderforge_platform::run_event_loop(config, |_input, events, raw_events, window| {
        // Feed raw window events to the editor's egui input first, so the UI
        // sees clicks, scrolls, and keystrokes.
        for event in raw_events {
            app.integrate_event(window, event);
        }
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

/// Read the window size from `--resolution <W>x<H>` (e.g. `1920x1080`),
/// defaulting to 1920×1080 when the flag is absent. Errors on a malformed value.
fn parse_resolution() -> anyhow::Result<(u32, u32)> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--resolution" {
            let value = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--resolution requires a value like 1920x1080"))?;
            return parse_dimensions(&value);
        }
    }
    Ok((1920, 1080))
}

/// Read the MSAA sample count from `--msaa <N>`, defaulting to `1` (disabled)
/// when the flag is absent. Only the GPU-portable power-of-two counts 1/2/4/8
/// are accepted; anything else errors.
fn parse_msaa() -> anyhow::Result<u32> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--msaa" {
            let value = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--msaa requires a sample count (1, 2, 4, or 8)"))?;
            let count: u32 = value
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid msaa sample count '{value}'"))?;
            if !matches!(count, 1 | 2 | 4 | 8) {
                anyhow::bail!("unsupported msaa sample count {count} (expected 1, 2, 4, or 8)");
            }
            return Ok(count);
        }
    }
    Ok(1)
}

/// Parse a `<W>x<H>` string (case-insensitive on the `x`) into a non-zero size.
fn parse_dimensions(value: &str) -> anyhow::Result<(u32, u32)> {
    let lower = value.trim().to_ascii_lowercase();
    let (w, h) = lower
        .split_once('x')
        .ok_or_else(|| anyhow::anyhow!("resolution '{value}' must look like 1920x1080"))?;
    let width: u32 = w
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid width in resolution '{value}'"))?;
    let height: u32 = h
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid height in resolution '{value}'"))?;
    if width == 0 || height == 0 {
        anyhow::bail!("resolution '{value}' must have non-zero width and height");
    }
    Ok((width, height))
}
