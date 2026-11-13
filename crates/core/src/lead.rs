//! Where to point on screen. The recognizer is always late: by the time it
//! reports a word, the reader is a word or two further on. `Lead` runs the
//! on-screen cursor ahead of the heard words at the reader's own pace, so the
//! highlighted word is the one being said, not one already said.
use crate::Tracker;

/// Typical time from the end of a spoken word to its arrival from the
/// recognizer.
pub const LATENCY_MS: u64 = 600;
/// The cursor never runs more than this many words past the heard words.
pub const MAX_LEAD: f64 = 3.0;
/// New speech this recent means the reader is still talking.
pub const TALKING_MS: u64 = 600;
const SLOWEST: f64 = 90.0 / 60_000.0;
const FASTEST: f64 = 240.0 / 60_000.0;

#[derive(Clone, Debug)]
pub struct Lead {
    /// The word shown as "you are here".
    shown: usize,
    /// Next word to say going by the heard words alone, when last looked.
    head: usize,
    /// Talking time since the heard words last moved.
    since_ms: u64,
    /// Words heard and the talking time they took (recent, decaying).
    words: f64,
    ms: f64,
    /// Words per ms before enough has been heard to measure.
    start_rate: f64,
    /// Time since the heard words last moved, talking or not.
    still_ms: u64,
    /// How long this reader usually pauses between sentences (learned).
    pause_ms: f64,
    /// Extra time to run ahead by, from the reader's settings.
    latency_ms: u64,
    /// Script words were heard in this stretch of talking. Speech that
    /// matched nothing yet (a cough, a comment) must not move the cursor.
    following: bool,
}
impl Lead {
    pub fn new(wpm: f64) -> Self {
        Self {
            shown: 0,
            head: 0,
            since_ms: 0,
            words: 0.0,
            ms: 0.0,
            start_rate: wpm / 60_000.0,
            still_ms: 0,
            pause_ms: 450.0,
            latency_ms: LATENCY_MS,
            following: false,
        }
    }
    pub fn set_wpm(&mut self, wpm: f64) {
        self.start_rate = wpm / 60_000.0;
    }
    /// How late the recognizer is assumed to be.
    pub fn set_latency_ms(&mut self, ms: u64) {
        self.latency_ms = ms;
    }
    /// The reader's pace in words per ms.
    pub fn rate(&self) -> f64 {
        let measured = if self.ms >= 2500.0 { self.words / self.ms } else { self.start_rate };
        measured.clamp(SLOWEST, FASTEST)
    }
    fn head_of(tracker: &Tracker) -> usize {
        tracker.position.map_or(0, |p| p + 1)
    }
    /// Where the cursor would be `ahead_ms` from now. Pure.
    pub fn cursor_after(&self, tracker: &Tracker, ahead_ms: u64, talking: bool) -> Option<usize> {
        let last = tracker.words.len().checked_sub(1)?;
        let head = Self::head_of(tracker);
        if head < self.head {
            // Moved back (re-read, retake, tap): start again from there.
            return Some(head.min(last));
        }
        let mut to = head;
        if talking && self.following {
            // Time the reader has had since the last heard word was said.
            let time = (self.latency_ms + self.since_ms + ahead_ms) as f64;
            let rate = self.rate();
            let words = (time * rate).min(MAX_LEAD) as usize;
            // Readers pause between sentences: on reaching a sentence's first
            // word, allow for this reader's usual pause before moving on.
            let start = tracker.sentence_starts.iter().find(|&&s| s >= head);
            to += match start {
                Some(&start) if head + words >= start => {
                    let to_start = start - head;
                    let left = time - to_start as f64 / rate - self.pause_ms;
                    to_start + ((left.max(0.0) * rate) as usize).min(words - to_start)
                }
                _ => words,
            };
        }
        // Never slide back while reading on, and never trail the heard words.
        Some(to.max(self.shown.min(head + MAX_LEAD as usize)).max(head).min(last))
    }
    pub fn cursor(&self, tracker: &Tracker, talking: bool) -> Option<usize> {
        self.cursor_after(tracker, 0, talking)
    }
    /// Time passes while reading.
    pub fn tick(&mut self, tracker: &Tracker, elapsed_ms: u64, talking: bool) {
        if Self::head_of(tracker) != self.head {
            // Moved by hand (tap, retake, next slide), not by the voice.
            self.following = false;
            self.since_ms = 0;
        }
        self.still_ms = self.still_ms.saturating_add(elapsed_ms);
        if talking {
            self.since_ms = self.since_ms.saturating_add(elapsed_ms);
        } else {
            self.following = false;
        }
        self.settle(tracker, talking);
    }
    /// The tracker took in new speech.
    pub fn heard(&mut self, tracker: &Tracker, talking: bool) {
        let head = Self::head_of(tracker);
        if head > self.head {
            let moved = head - self.head;
            // Steady reading only: a jump or a long silence says nothing
            // about pace.
            if moved <= 6 && self.since_ms > 0 && self.since_ms < 2500 {
                self.words += moved as f64;
                self.ms += self.since_ms as f64;
                if self.ms > 8000.0 {
                    self.words /= 2.0;
                    self.ms /= 2.0;
                }
            }
            // Crossing into a new sentence shows how long this reader
            // pauses there: the time the heard words rested on its first
            // word, less the time it takes to say that word.
            if let Some(&start) = tracker.sentence_starts.iter().find(|&&s| s >= self.head) {
                if start > 0 && start < head {
                    let rested = if start == self.head { self.still_ms as f64 } else { 0.0 };
                    let pause = (rested - 1.0 / self.rate()).clamp(0.0, 1500.0);
                    self.pause_ms += (pause - self.pause_ms) * 0.3;
                }
            }
            self.since_ms = 0;
            self.still_ms = 0;
            self.following = true;
        } else if head < self.head {
            self.following = false;
        }
        self.settle(tracker, talking);
    }
    fn settle(&mut self, tracker: &Tracker, talking: bool) {
        self.shown = self.cursor(tracker, talking).unwrap_or(0);
        self.head = Self::head_of(tracker);
    }
}
