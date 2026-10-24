//! Progress as shown to the user: weighted by bytes, with a throughput and
//! a remaining time measured on what was actually read, never guessed from
//! file counts.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

/// Throughput is averaged over this much recent time: long enough to smooth
/// disk hiccups, short enough to follow a switch from SSD to network.
const WINDOW: Duration = Duration::from_secs(5);
/// No remaining time before this much elapsed time…
const MIN_ELAPSED: Duration = Duration::from_secs(5);
/// …and this much data, unless reading is very slow.
const MIN_BYTES: u64 = 64 << 20;
const SLOW_ELAPSED: Duration = Duration::from_secs(15);
/// Events sent to the UI at most this often.
const THROTTLE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProgressSnapshot {
    pub current_path: Option<String>,
    #[ts(type = "number")]
    pub file_done: u64,
    #[ts(type = "number")]
    pub file_total: u64,
    pub files_done: u32,
    pub files_total: u32,
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    /// `None` until something was measured.
    pub bytes_per_second: Option<f64>,
    /// `None` means "estimation en cours": not enough measured yet.
    #[ts(type = "number | null")]
    pub remaining_ms: Option<u64>,
}

/// Throughput over a sliding window of recent samples.
#[derive(Debug)]
pub struct ThroughputMeter {
    start: Instant,
    samples: VecDeque<(Instant, u64)>,
}

impl ThroughputMeter {
    pub fn new(start: Instant) -> ThroughputMeter {
        ThroughputMeter {
            start,
            samples: VecDeque::from([(start, 0)]),
        }
    }

    /// Starts measuring afresh from `bytes` already read, after a pause:
    /// the time spent paused must not count as a slow read.
    pub fn restart(&mut self, now: Instant, bytes: u64) {
        self.start = now;
        self.samples.clear();
        self.samples.push_back((now, bytes));
    }

    /// `bytes` is the running total read so far.
    pub fn record(&mut self, now: Instant, bytes: u64) {
        self.samples.push_back((now, bytes));
        // Keep one sample older than the window, so the window stays full.
        while self.samples.len() > 2 && now.duration_since(self.samples[1].0) >= WINDOW {
            self.samples.pop_front();
        }
    }

    pub fn bytes_per_second(&self) -> Option<f64> {
        let (first_time, first_bytes) = *self.samples.front()?;
        let (last_time, last_bytes) = *self.samples.back()?;
        let seconds = last_time.duration_since(first_time).as_secs_f64();
        (seconds >= 0.5).then(|| (last_bytes - first_bytes) as f64 / seconds)
    }

    /// Remaining time for `remaining` bytes, or `None` while the measure is
    /// too short to mean anything.
    pub fn remaining(&self, now: Instant, measured: u64, remaining: u64) -> Option<Duration> {
        let elapsed = now.duration_since(self.start);
        let enough = elapsed >= MIN_ELAPSED && (measured >= MIN_BYTES || elapsed >= SLOW_ELAPSED);
        if !enough {
            return None;
        }
        let rate = self.bytes_per_second()?;
        (rate > 0.0).then(|| Duration::from_secs_f64(remaining as f64 / rate))
    }
}

/// Turns raw "bytes read" notifications into throttled snapshots.
pub struct ProgressReporter<'a> {
    start: Instant,
    last_sent: Option<Instant>,
    meter: ThroughputMeter,
    files_total: u32,
    files_done: u32,
    bytes_total: u64,
    /// Bytes of the files already finished.
    bytes_before: u64,
    sink: Box<dyn FnMut(&ProgressSnapshot) + Send + 'a>,
}

impl<'a> ProgressReporter<'a> {
    pub fn new(
        files_total: u32,
        bytes_total: u64,
        sink: impl FnMut(&ProgressSnapshot) + Send + 'a,
    ) -> ProgressReporter<'a> {
        let start = Instant::now();
        ProgressReporter {
            start,
            last_sent: None,
            meter: ThroughputMeter::new(start),
            files_total,
            files_done: 0,
            bytes_total,
            bytes_before: 0,
            sink: Box::new(sink),
        }
    }

    /// Progress inside the current file.
    pub fn file_progress(&mut self, path: &str, done: u64, total: u64) {
        let now = Instant::now();
        self.meter.record(now, self.bytes_before + done);
        let finished = done == total;
        if !finished
            && self
                .last_sent
                .is_some_and(|t| now.duration_since(t) < THROTTLE)
        {
            return;
        }
        self.send(now, Some(path), done, total);
    }

    /// The current file is over (hashed, failed or skipped); `size` is what
    /// it counted for in the total.
    pub fn file_finished(&mut self, size: u64) {
        self.files_done += 1;
        self.bytes_before += size;
        self.send(Instant::now(), None, 0, 0);
    }

    fn send(&mut self, now: Instant, path: Option<&str>, done: u64, total: u64) {
        self.last_sent = Some(now);
        let bytes_done = self.bytes_before + done;
        let snapshot = ProgressSnapshot {
            current_path: path.map(str::to_owned),
            file_done: done,
            file_total: total,
            files_done: self.files_done,
            files_total: self.files_total,
            bytes_done,
            bytes_total: self.bytes_total,
            elapsed_ms: now.duration_since(self.start).as_millis() as u64,
            bytes_per_second: self.meter.bytes_per_second(),
            remaining_ms: self
                .meter
                .remaining(now, bytes_done, self.bytes_total.saturating_sub(bytes_done))
                .map(|d| d.as_millis() as u64),
        };
        (self.sink)(&snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1 << 20;

    #[test]
    fn throughput_follows_recent_speed() {
        let start = Instant::now();
        let mut meter = ThroughputMeter::new(start);
        assert_eq!(meter.bytes_per_second(), None, "nothing measured yet");
        // 10 s at 100 MiB/s, then 10 s at 10 MiB/s.
        for s in 1..=10 {
            meter.record(start + Duration::from_secs(s), s * 100 * MIB);
        }
        let fast = meter.bytes_per_second().unwrap();
        assert!((fast - (100 * MIB) as f64).abs() < 1.0, "{fast}");
        for s in 11..=20 {
            meter.record(
                start + Duration::from_secs(s),
                1000 * MIB + (s - 10) * 10 * MIB,
            );
        }
        let slow = meter.bytes_per_second().unwrap();
        assert!(
            (slow - (10 * MIB) as f64).abs() < 1.0,
            "only the last 5 s count: {slow}"
        );
    }

    #[test]
    fn remaining_time_waits_for_enough_measure() {
        let start = Instant::now();
        let mut meter = ThroughputMeter::new(start);
        let at = |s: u64| start + Duration::from_secs(s);
        meter.record(at(2), 200 * MIB);
        assert_eq!(
            meter.remaining(at(2), 200 * MIB, 1000 * MIB),
            None,
            "under 5 s"
        );
        meter.record(at(6), 600 * MIB);
        let eta = meter.remaining(at(6), 600 * MIB, 1000 * MIB).unwrap();
        assert_eq!(eta.as_secs(), 10, "100 MiB/s, 1000 MiB left");

        // Very slow source: 1 MiB/s never reaches 64 MiB quickly.
        let mut slow = ThroughputMeter::new(start);
        slow.record(at(6), 6 * MIB);
        assert_eq!(slow.remaining(at(6), 6 * MIB, 100 * MIB), None);
        slow.record(at(16), 16 * MIB);
        assert!(
            slow.remaining(at(16), 16 * MIB, 100 * MIB).is_some(),
            "after 15 s anyway"
        );
    }

    #[test]
    fn reporter_throttles_but_always_sends_the_end_of_a_file() {
        let mut sent = Vec::new();
        {
            let mut reporter = ProgressReporter::new(1, 10 * MIB, |s| sent.push(s.clone()));
            for i in 0..=10 {
                reporter.file_progress("a.bin", i * MIB, 10 * MIB);
            }
            reporter.file_finished(10 * MIB);
        }
        assert!(sent.len() < 11, "intermediate events are throttled");
        let end = sent.iter().find(|s| s.file_done == 10 * MIB).unwrap();
        assert_eq!(end.bytes_done, 10 * MIB);
        let last = sent.last().unwrap();
        assert_eq!((last.files_done, last.bytes_done), (1, 10 * MIB));
    }
}
