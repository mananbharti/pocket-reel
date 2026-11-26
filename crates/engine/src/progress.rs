//! Progress, speed and ETA calculation.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Shared flag used to cancel a running job.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_canceled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// 0.0 ..= 1.0
    pub fraction: f64,
    /// Seconds of output media produced so far.
    pub media_seconds: f64,
    /// Processing speed relative to real time (2.0 = twice as fast as playback).
    pub speed: f64,
    /// Estimated seconds remaining, once enough data is available.
    pub eta_seconds: Option<f64>,
    pub elapsed_seconds: f64,
    pub bytes: u64,
}

/// Turns raw "media seconds done" samples into smoothed progress updates.
pub struct Tracker {
    total: f64,
    started: Instant,
    window: VecDeque<(Instant, f64)>,
    last_emit: Option<Instant>,
    smoothed_rate: Option<f64>,
    interval: Duration,
}

impl Tracker {
    pub fn new(total_media_seconds: f64) -> Self {
        Tracker {
            total: total_media_seconds.max(0.0),
            started: Instant::now(),
            window: VecDeque::new(),
            last_emit: None,
            smoothed_rate: None,
            interval: Duration::from_millis(120),
        }
    }

    /// Feed a sample; returns an update at most every ~120 ms.
    pub fn update(&mut self, media_done: f64, bytes: u64) -> Option<Progress> {
        let now = Instant::now();
        self.window.push_back((now, media_done));
        while let Some(&(t, _)) = self.window.front() {
            if now.duration_since(t) > Duration::from_secs(6) && self.window.len() > 2 {
                self.window.pop_front();
            } else {
                break;
            }
        }
        if let Some(last) = self.last_emit {
            if now.duration_since(last) < self.interval {
                return None;
            }
        }
        self.last_emit = Some(now);
        Some(self.snapshot(now, media_done, bytes))
    }

    fn snapshot(&mut self, now: Instant, media_done: f64, bytes: u64) -> Progress {
        let elapsed = now.duration_since(self.started).as_secs_f64();
        let (t0, m0) = *self.window.front().unwrap_or(&(self.started, 0.0));
        let span = now.duration_since(t0).as_secs_f64();
        let instant_rate = if span > 0.25 { (media_done - m0) / span } else if elapsed > 0.0 { media_done / elapsed } else { 0.0 };
        let rate = match self.smoothed_rate {
            Some(r) => r * 0.7 + instant_rate * 0.3,
            None => instant_rate,
        };
        self.smoothed_rate = Some(rate);
        let fraction = if self.total > 0.0 { (media_done / self.total).clamp(0.0, 1.0) } else { 0.0 };
        let eta = if rate > 1e-6 && elapsed > 1.0 && self.total > 0.0 {
            Some(((self.total - media_done).max(0.0)) / rate)
        } else {
            None
        };
        Progress { fraction, media_seconds: media_done, speed: rate.max(0.0), eta_seconds: eta, elapsed_seconds: elapsed, bytes }
    }

    /// Final 100% update.
    pub fn finish(&mut self, bytes: u64) -> Progress {
        let now = Instant::now();
        let mut p = self.snapshot(now, self.total, bytes);
        p.fraction = 1.0;
        p.eta_seconds = Some(0.0);
        p
    }
}

/// Parse one `-progress` line pair: returns media seconds for `out_time_us`/`out_time_ms`.
pub fn parse_progress_line(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.trim().split_once('=')?;
    Some((k.trim(), v.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_and_eta() {
        let mut t = Tracker::new(100.0);
        t.interval = Duration::from_millis(0);
        std::thread::sleep(Duration::from_millis(5));
        let p = t.update(50.0, 10).unwrap();
        assert!((p.fraction - 0.5).abs() < 1e-9);
        let done = t.finish(20);
        assert_eq!(done.fraction, 1.0);
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress_line("out_time_us=1500000"), Some(("out_time_us", "1500000")));
        assert_eq!(parse_progress_line("nonsense"), None);
    }
}
