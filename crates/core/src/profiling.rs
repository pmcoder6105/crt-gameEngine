//! Scope-based profiling.
//!
//! Create a [`TimingScope`] at the top of any block you want timed; when it
//! drops, the span duration is recorded into a thread-local collector.
//! Once per frame, drain the collector with [`take_spans`] or render an
//! aggregated table with [`report`].
//!
//! ```
//! use elderforge_core::profiling;
//!
//! {
//!     let _scope = profiling::TimingScope::new("physics_step");
//!     // ... work ...
//! }
//! println!("{}", profiling::report());
//! ```

use std::cell::RefCell;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

thread_local! {
    static COLLECTOR: RefCell<Vec<SpanRecord>> = const { RefCell::new(Vec::new()) };
}

/// One completed timing span: a label and how long the scope lived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRecord {
    /// Name given to the [`TimingScope`] that produced this record.
    pub label: &'static str,
    /// Wall-clock duration of the scope.
    pub duration: Duration,
}

/// Times the enclosing scope and records the duration on drop.
///
/// Records go to a per-thread collector, so scopes on different threads
/// never contend; each thread drains its own records with [`take_spans`].
pub struct TimingScope {
    label: &'static str,
    start: Instant,
}

impl TimingScope {
    /// Starts timing a span with the given label.
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            start: Instant::now(),
        }
    }

    /// Time elapsed since this scope was created.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        let record = SpanRecord {
            label: self.label,
            duration: self.start.elapsed(),
        };
        COLLECTOR.with(|c| c.borrow_mut().push(record));
    }
}

/// Drains and returns all spans recorded on the current thread, in
/// completion order. Call once per frame to keep the collector bounded.
pub fn take_spans() -> Vec<SpanRecord> {
    COLLECTOR.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Drains the current thread's spans and renders them as a table,
/// aggregated by label and sorted by total time descending.
pub fn report() -> String {
    format_spans(&take_spans())
}

/// Renders spans as an aggregated table without touching the collector.
/// Columns: label, call count, total ms, average ms.
pub fn format_spans(spans: &[SpanRecord]) -> String {
    // Aggregate by label, preserving first-seen order before sorting.
    let mut rows: Vec<(&'static str, u32, Duration)> = Vec::new();
    for span in spans {
        match rows.iter_mut().find(|(label, ..)| *label == span.label) {
            Some((_, count, total)) => {
                *count += 1;
                *total += span.duration;
            }
            None => rows.push((span.label, 1, span.duration)),
        }
    }
    rows.sort_by(|a, b| b.2.cmp(&a.2));

    let label_width = rows
        .iter()
        .map(|(label, ..)| label.len())
        .max()
        .unwrap_or(0)
        .max("scope".len());

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:label_width$}  {:>6}  {:>10}  {:>10}",
        "scope", "calls", "total ms", "avg ms"
    );
    for (label, count, total) in rows {
        let total_ms = total.as_secs_f64() * 1000.0;
        let _ = writeln!(
            out,
            "{label:label_width$}  {count:>6}  {total_ms:>10.3}  {:>10.3}",
            total_ms / count as f64
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_record_into_thread_local_collector() {
        take_spans(); // isolate from other tests on this thread
        {
            let _a = TimingScope::new("outer");
            let _b = TimingScope::new("inner");
        }
        let spans = take_spans();
        // Inner drops first.
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].label, "inner");
        assert_eq!(spans[1].label, "outer");
        assert!(take_spans().is_empty(), "take_spans must drain");
    }

    #[test]
    fn format_aggregates_repeated_labels() {
        let spans = [
            SpanRecord {
                label: "step",
                duration: Duration::from_millis(2),
            },
            SpanRecord {
                label: "step",
                duration: Duration::from_millis(4),
            },
            SpanRecord {
                label: "render",
                duration: Duration::from_millis(1),
            },
        ];
        let table = format_spans(&spans);
        let step_line = table
            .lines()
            .find(|l| l.starts_with("step"))
            .expect("step row present");
        assert!(step_line.contains('2'), "call count");
        assert!(step_line.contains("6.000"), "total ms");
        assert!(step_line.contains("3.000"), "avg ms");
        // Sorted by total descending: step (6ms) before render (1ms).
        let step_pos = table.find("step").expect("step in table");
        let render_pos = table.find("render").expect("render in table");
        assert!(step_pos < render_pos);
    }
}
