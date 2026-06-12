//! Fixed-timestep accumulation for deterministic simulation stepping.

/// Converts variable frame times into a whole number of fixed-size
/// simulation steps.
///
/// Each frame, feed the elapsed wall-clock time to
/// [`integrate`](Self::integrate) and run the physics step the returned
/// number of times. Leftover time stays in the accumulator and carries
/// into the next frame, so simulation speed matches wall-clock speed on
/// average regardless of frame rate.
///
/// When a frame is so long that it would demand more than
/// `max_steps_per_frame` steps, the step count is clamped and the excess
/// time is discarded. The simulation runs slower than real time for that
/// frame, but each step's cost can never snowball into ever-longer frames
/// (the "death spiral").
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    target_dt: f32,
    max_steps_per_frame: u32,
    accumulator: f32,
}

impl FixedTimestep {
    /// Creates an accumulator stepping at `target_dt` seconds per step
    /// (e.g. `1.0 / 120.0`), running at most `max_steps_per_frame` steps
    /// in a single frame.
    ///
    /// `target_dt` must be positive and `max_steps_per_frame` non-zero.
    pub fn new(target_dt: f32, max_steps_per_frame: u32) -> Self {
        debug_assert!(target_dt > 0.0, "target_dt must be positive");
        debug_assert!(max_steps_per_frame > 0, "max_steps_per_frame must be non-zero");
        Self {
            target_dt,
            max_steps_per_frame,
            accumulator: 0.0,
        }
    }

    /// Adds this frame's elapsed time and returns how many fixed steps to
    /// run, clamped to `max_steps_per_frame`. When clamped, the time that
    /// could not be simulated is dropped to prevent a death spiral.
    pub fn integrate(&mut self, frame_dt: f32) -> u32 {
        // Negative or NaN frame times (clock glitches) contribute nothing.
        if frame_dt > 0.0 {
            self.accumulator += frame_dt;
        }

        let mut steps = (self.accumulator / self.target_dt) as u32;
        if steps > self.max_steps_per_frame {
            steps = self.max_steps_per_frame;
            // Drop the backlog we refuse to simulate.
            self.accumulator = self.target_dt * steps as f32;
        }
        self.accumulator -= self.target_dt * steps as f32;
        steps
    }

    /// Fraction of a step accumulated but not yet simulated, in `[0, 1)`.
    /// Use to interpolate render state between the last two physics states.
    pub fn alpha(&self) -> f32 {
        self.accumulator / self.target_dt
    }

    /// The fixed step size in seconds.
    pub fn target_dt(&self) -> f32 {
        self.target_dt
    }

    /// Discards any accumulated time, e.g. after a pause or scene load.
    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    #[test]
    fn exact_frame_yields_exact_steps() {
        let mut ts = FixedTimestep::new(DT, 8);
        assert_eq!(ts.integrate(DT), 1);
        assert_eq!(ts.integrate(DT * 3.0), 3);
        assert!(ts.alpha() < 1e-3);
    }

    #[test]
    fn short_frames_accumulate_into_steps() {
        let mut ts = FixedTimestep::new(DT, 8);
        // Half a step per frame: steps fire every other frame.
        assert_eq!(ts.integrate(DT * 0.5), 0);
        assert_eq!(ts.integrate(DT * 0.5), 1);
        assert_eq!(ts.integrate(DT * 0.5), 0);
        assert_eq!(ts.integrate(DT * 0.5), 1);
    }

    #[test]
    fn varying_frame_times_preserve_total_steps() {
        let mut ts = FixedTimestep::new(DT, 8);
        let frames = [0.003, 0.011, 0.007, 0.021, 0.001, 0.013];
        let total_time: f32 = frames.iter().sum();
        let total_steps: u32 = frames.iter().map(|&dt| ts.integrate(dt)).sum();
        // All simulated time plus the remaining fraction accounts for
        // every second fed in — nothing lost, nothing duplicated.
        let expected = (total_time / DT) as u32;
        assert_eq!(total_steps, expected);
        assert!(ts.alpha() >= 0.0 && ts.alpha() < 1.0);
    }

    #[test]
    fn huge_frame_clamps_to_max_steps() {
        let mut ts = FixedTimestep::new(DT, 8);
        // A full second at 120Hz would demand 120 steps.
        assert_eq!(ts.integrate(1.0), 8);
        // The backlog was dropped: a normal frame after the spike runs a
        // normal number of steps instead of replaying the missed time.
        assert_eq!(ts.integrate(DT), 1);
    }

    #[test]
    fn sustained_low_fps_never_death_spirals() {
        let mut ts = FixedTimestep::new(DT, 8);
        // 10 fps for 100 frames: every frame wants 12 steps, gets 8,
        // and the accumulator must not grow without bound.
        for _ in 0..100 {
            assert_eq!(ts.integrate(0.1), 8);
            assert!(ts.alpha() < 1.0, "accumulator must stay below one step");
        }
    }

    #[test]
    fn zero_and_negative_dt_yield_no_steps() {
        let mut ts = FixedTimestep::new(DT, 8);
        assert_eq!(ts.integrate(0.0), 0);
        assert_eq!(ts.integrate(-1.0), 0);
        assert_eq!(ts.alpha(), 0.0);
    }

    #[test]
    fn reset_discards_accumulated_time() {
        let mut ts = FixedTimestep::new(DT, 8);
        ts.integrate(DT * 0.9);
        ts.reset();
        assert_eq!(ts.integrate(DT * 0.5), 0);
    }
}
