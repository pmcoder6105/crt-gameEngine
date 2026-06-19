//! Editor panels.

mod asset_browser;
mod hierarchy;
mod inspector;
mod overlays;
mod sim_controls;
mod stats;
mod toolbar;

pub use asset_browser::AssetBrowser;
pub use hierarchy::Hierarchy;
pub use inspector::Inspector;
pub use overlays::Overlays;
pub use sim_controls::SimControls;
pub use stats::Stats;
pub use toolbar::Toolbar;
