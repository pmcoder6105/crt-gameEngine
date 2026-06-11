//! Minimal scope-based profiling.
//! // TODO: aggregate timings into per-frame stats for the editor profiler panel.

use std::time::Instant;

/// Times a scope and logs the result on drop.
pub struct ScopeTimer {
    label: &'static str,
    start: Instant,
}

impl ScopeTimer {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            start: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> f32 {
        self.start.elapsed().as_secs_f32() * 1000.0
    }
}

impl Drop for ScopeTimer {
    fn drop(&mut self) {
        log::trace!("{}: {:.3} ms", self.label, self.elapsed_ms());
    }
}
