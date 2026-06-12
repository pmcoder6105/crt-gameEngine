//! Elderforge editor binary: wires platform, renderer, physics, ECS, scene,
//! and editor together. Owns the event loop.

mod app;
mod systems;

fn main() -> anyhow::Result<()> {
    elderforge_core::init_logging();
    log::info!("Elderforge starting");

    let mut app = app::App::new();
    // TODO: create the window through elderforge-platform, set up the
    // renderer surface, and drive `app.update()` from the winit event loop
    // (egui-winit feeds window events into the editor UI).
    app.update();

    log::info!("Elderforge exiting");
    Ok(())
}
