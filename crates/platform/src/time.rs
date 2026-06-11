//! Frame clock with a fixed-timestep accumulator. Physics runs at a fixed
//! 120 Hz regardless of render frame rate.

use std::time::Instant;

pub const FIXED_TIMESTEP_HZ: f32 = 120.0;

pub struct Clock {
    last_tick: Instant,
    accumulator: f32,
    fixed_dt: f32,
    delta: f32,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            accumulator: 0.0,
            fixed_dt: 1.0 / FIXED_TIMESTEP_HZ,
            delta: 0.0,
        }
    }

    /// Advance the clock. Returns the frame delta time in seconds.
    pub fn tick(&mut self) -> f32 {
        let now = Instant::now();
        self.delta = (now - self.last_tick).as_secs_f32();
        self.last_tick = now;
        self.accumulator += self.delta;
        self.delta
    }

    /// True while another fixed step should run this frame; consumes one step.
    pub fn consume_fixed_step(&mut self) -> bool {
        if self.accumulator >= self.fixed_dt {
            self.accumulator -= self.fixed_dt;
            true
        } else {
            false
        }
    }

    pub fn fixed_dt(&self) -> f32 {
        self.fixed_dt
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}
