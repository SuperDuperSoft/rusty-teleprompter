//! Shared deterministic script matching and playback. No clocks, audio, UI or IO.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

mod numbers;
pub use numbers::is_number_word;
mod remote;
pub mod recording;
pub use remote::*;
mod tracker;
pub use tracker::{Outcome, Tracker};
mod lead;
pub use lead::{Lead, TALKING_MS};

/// Version of the `words()` indexing scheme stored in saved positions.
pub const WORDS_SCHEME: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    pub title: String,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    pub id: String,
    pub title: String,
    pub revision: u64,
    pub slides: Vec<Slide>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub generation: u64,
    pub scripts: Vec<Script>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Indices of words that begin a sentence: the first word, words after a blank
/// line, and capitalised/numeric/money words after `.`, `?` or `!` (not abbreviations).
pub fn sentence_starts(text: &str, words: &[Word]) -> Vec<usize> {
    const ABBREVIATIONS: [&str; 11] = [
        "dr", "mr", "mrs", "ms", "st", "rev", "vs", "etc", "jr", "sr", "no",
    ];
    if words.is_empty() {
        return Vec::new();
    }
    let mut starts = vec![0];
    for i in 1..words.len() {
        let gap: String = text[words[i - 1].end..words[i].start]
            .chars()
            .filter(|c| !matches!(c, ' ' | '\t' | '\r'))
            .collect();
        let blank = gap.contains("\n\n");
        let capital = text[words[i].start..]
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '$');
        let ends = gap.contains(['.', '?', '!'])
            && capital
            && !ABBREVIATIONS.contains(&words[i - 1].text.as_str());
        if blank || ends {
            starts.push(i);
        }
    }
    starts
}

/// First word index of the unit containing word `i` (walks back over
/// zero-width parts that share the unit's start).
pub fn unit_start(words: &[Word], i: usize) -> usize {
    let mut j = i;
    while j > 0 && words[j - 1].start == words[j - 1].end && words[j - 1].start == words[i].start {
        j -= 1;
    }
    j
}
/// Last word index of the unit containing word `i` (walks forward while
/// the current word is zero-width and the next one starts at the same byte).
pub fn unit_end(words: &[Word], i: usize) -> usize {
    let mut j = i;
    while j + 1 < words.len()
        && words[j].start == words[j].end
        && words[j + 1].start == words[j].start
    {
        j += 1;
    }
    j
}

/// Spoken words with byte spans. Numbers expand to several words: all but the
/// last are zero-width at the unit's start, the last spans the whole unit.
pub fn words(text: &str) -> Vec<Word> {
    static START: OnceLock<Regex> = OnceLock::new();
    static WORD: OnceLock<Regex> = OnceLock::new();
    let start = START.get_or_init(|| Regex::new(r"\$[0-9]|[\p{L}\p{N}]").unwrap());
    let re = WORD.get_or_init(|| Regex::new(r"^[\p{L}\p{N}]+(?:['’][\p{L}]+)?").unwrap());
    let cues: Vec<_> = cue_re().find_iter(text).map(|m| m.range()).collect();
    let (mut out, mut pos, mut cue) = (vec![], 0, 0);
    while let Some(m) = start.find_at(text, pos) {
        let s = m.start();
        while cue < cues.len() && cues[cue].end <= s {
            cue += 1;
        }
        if cues.get(cue).is_some_and(|c| c.contains(&s)) {
            pos = cues[cue].end;
            continue;
        }
        let first = text[s..].chars().next().unwrap_or(' ');
        if (first == '$' || first.is_ascii_digit())
            && !text[..s]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
        {
            if let Some(end) = numbers::numeric(text, s, &mut out) {
                pos = end;
                continue;
            }
        }
        let Some(m) = re.find(&text[s..]) else {
            pos = s + first.len_utf8();
            continue;
        };
        pos = s + m.end();
        let lower = m.as_str().to_lowercase().replace('’', "'");
        let normalized = match lower.as_str() {
            "0" => "zero",
            "1" => "one",
            "2" => "two",
            "3" => "three",
            "4" => "four",
            "5" => "five",
            "6" => "six",
            "7" => "seven",
            "8" => "eight",
            "9" => "nine",
            "10" => "ten",
            "11" => "eleven",
            "12" => "twelve",
            "13" => "thirteen",
            "14" => "fourteen",
            "15" => "fifteen",
            "16" => "sixteen",
            "17" => "seventeen",
            "18" => "eighteen",
            "19" => "nineteen",
            "20" => "twenty",
            _ => &lower,
        };
        out.push(Word {
            text: normalized.to_owned(),
            start: s,
            end: pos,
        });
    }
    out
}
pub(crate) fn cue_re() -> &'static Regex {
    static CUE: OnceLock<Regex> = OnceLock::new();
    CUE.get_or_init(|| Regex::new(r"\[[\s\S]*?\]").unwrap())
}
pub(crate) fn tokens(s: &str) -> Vec<String> {
    words(s).into_iter().map(|w| w.text).collect()
}
/// Vosk's "[unk]" (any ASCII case).
pub(crate) fn unk_re() -> &'static Regex {
    static UNK: OnceLock<Regex> = OnceLock::new();
    UNK.get_or_init(|| Regex::new(r"(?i-u)\[unk\]").unwrap())
}
/// Stands in for "[unk]" in hypotheses, which the cue rule would strip; it
/// never matches a script word.
pub(crate) const UNK: &str = "zzunk";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Ready,
    Reading,
    Countdown,
    Holding,
    Paused,
    Finished,
    Rolling,
}
impl Phase {
    pub fn running(self) -> bool {
        matches!(self, Self::Reading | Self::Countdown | Self::Rolling)
    }
    /// True while speech counts (reading, counting down or holding).
    pub fn listening(self) -> bool {
        matches!(self, Self::Reading | Self::Countdown | Self::Holding)
    }
    /// True while the mic should be live: listening, or warm through a pre-roll.
    pub fn mic_open(self) -> bool {
        self.listening() || self == Self::Rolling
    }
}

/// Forward is ignored this early in a pre-roll (double press, key bounce).
pub const ROLL_GUARD_MS: u64 = 500;
/// The heard dot stays lit this long after new speech.
pub const HEARD_MS: u64 = 700;
/// Speech this recent counts as still speaking (for the lost-place hint).
pub const SPEAKING_MS: u64 = 1500;

#[derive(Clone, Debug)]
pub struct Session {
    pub script: Script,
    pub slide_index: usize,
    pub phase: Phase,
    pub tracker: Tracker,
    pub remaining_ms: u64,
    pub take: u64,
    pub countdown_ms: u64,
    pub hold_at_end: bool,
    pub pre_roll_ms: u64,
    pub roll_ms: u64,
    pub sync_mark: u64,
    pub wpm: f64,
    pub elapsed_ms: u64,
    pub heard_ago_ms: Option<u64>,
    pub stuck_ms: u64,
    pub lost_after_ms: u64,
    pub voice_resync: bool,
    resume: Phase,
    last_heard: String,
    /// Length of the current pre-roll, fixed when it started.
    roll_len: u64,
    /// Runs the on-screen cursor ahead of the heard words.
    lead: Lead,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub script_id: String,
    pub revision: u64,
    pub slide_id: String,
    pub next_word: usize,
    pub finished: bool,
    #[serde(default)]
    pub words_scheme: u32,
}
impl Session {
    pub fn new(script: Script) -> Self {
        let index = script
            .slides
            .iter()
            .position(|s| !words(&s.text).is_empty());
        let tracker = Self::new_tracker(
            index.map(|i| script.slides[i].text.as_str()).unwrap_or(""),
            true,
        );
        Self {
            script,
            slide_index: index.unwrap_or(0),
            phase: if index.is_some() {
                Phase::Ready
            } else {
                Phase::Finished
            },
            tracker,
            remaining_ms: 5000,
            take: 0,
            countdown_ms: 5000,
            hold_at_end: false,
            pre_roll_ms: 0,
            roll_ms: 0,
            sync_mark: 0,
            wpm: 150.0,
            elapsed_ms: 0,
            heard_ago_ms: None,
            stuck_ms: 0,
            lost_after_ms: 0,
            voice_resync: true,
            resume: Phase::Reading,
            last_heard: String::new(),
            roll_len: 0,
            lead: Lead::new(150.0),
        }
    }
    fn new_tracker(text: &str, allow_back: bool) -> Tracker {
        let mut tracker = Tracker::new(text);
        tracker.allow_back = allow_back;
        tracker
    }
    pub fn slide(&self) -> Option<&Slide> {
        self.script.slides.get(self.slide_index)
    }
    /// The phase a paused session returns to on resume.
    pub fn resume_phase(&self) -> Phase {
        self.resume
    }
    fn playable_from(&self, start: usize) -> Option<usize> {
        (start..self.script.slides.len()).find(|i| !words(&self.script.slides[*i].text).is_empty())
    }
    pub fn configure(&mut self, settings: &Settings) {
        self.countdown_ms = (settings.countdown_secs * 1000.0).round() as u64;
        self.hold_at_end = settings.hold_at_end;
        self.pre_roll_ms = (settings.pre_roll_secs * 1000.0).round() as u64;
        self.wpm = settings.speaking_wpm;
        self.lead.set_wpm(settings.speaking_wpm);
        self.lead.set_latency_ms((settings.highlight_lead.clamp(0.0, 1.5) * 1000.0).round() as u64);
        self.lost_after_ms = if settings.lost_hint_popup {
            (settings.lost_place_secs * 1000.0).round() as u64
        } else {
            0
        };
        self.voice_resync = settings.voice_resync;
        self.tracker.allow_back = settings.voice_resync;
    }
    pub fn start(&mut self) {
        match self.phase {
            Phase::Finished | Phase::Rolling => return,
            Phase::Holding => return self.next(),
            _ => {}
        }
        self.clear_heard();
        let idle = matches!(self.phase, Phase::Ready | Phase::Paused);
        let resume = if self.phase == Phase::Paused {
            self.resume
        } else {
            Phase::Reading
        };
        self.phase = if resume == Phase::Rolling {
            Phase::Reading
        } else {
            resume
        };
        self.tracker.reset_utterance();
        if idle && self.phase == Phase::Reading {
            self.enter_roll();
        }
    }
    pub fn pause(&mut self) {
        if self.phase.listening() || matches!(self.phase, Phase::Ready | Phase::Rolling) {
            self.resume = if self.phase == Phase::Ready {
                Phase::Reading
            } else {
                self.phase
            };
            self.phase = Phase::Paused;
            self.roll_ms = 0;
            self.clear_heard();
        }
    }
    /// Forgets recent speech: no heard dot, no lost-place time.
    fn clear_heard(&mut self) {
        self.stuck_ms = 0;
        self.heard_ago_ms = None;
        self.last_heard.clear();
    }
    /// Starts a fresh pre-roll; does nothing when pre-roll is off or the
    /// slide has no words.
    fn enter_roll(&mut self) {
        if self.pre_roll_ms == 0 || self.tracker.words.is_empty() {
            return;
        }
        self.phase = Phase::Rolling;
        self.roll_ms = self.pre_roll_ms;
        self.roll_len = self.pre_roll_ms;
        self.resume = Phase::Reading;
        self.tracker.reset_utterance();
    }
    /// Ends the pre-roll: reading starts and the sync mark advances.
    fn finish_roll(&mut self) {
        self.phase = Phase::Reading;
        self.roll_ms = 0;
        self.sync_mark += 1;
        self.tracker.reset_utterance();
        self.clear_heard();
    }
    /// Ends the pre-roll now (sync mark included). No-op outside Rolling.
    pub fn skip_roll(&mut self) {
        if self.phase == Phase::Rolling {
            self.finish_roll();
        }
    }
    /// "Again" after Finished: first playable slide, elapsed_ms = 0, then
    /// roll (or Reading when pre-roll is off). No-op if nothing is playable.
    pub fn read_again(&mut self) {
        if let Some(i) = self.playable_from(0) {
            self.move_to(i);
            self.elapsed_ms = 0;
            self.phase = Phase::Reading;
            self.enter_roll();
        }
    }
    /// The remote's Forward.
    pub fn forward(&mut self) {
        match self.phase {
            Phase::Ready => self.start(),
            Phase::Paused if self.resume == Phase::Holding => {
                self.next();
                self.start();
            }
            Phase::Paused => self.start(),
            Phase::Rolling if self.roll_len.saturating_sub(self.roll_ms) >= ROLL_GUARD_MS => {
                self.skip_roll()
            }
            Phase::Rolling => {}
            Phase::Reading | Phase::Countdown | Phase::Holding => self.next(),
            Phase::Finished => self.read_again(),
        }
    }
    pub fn speech(&mut self, text: &str, is_final: bool) {
        // Only "[unk]" means nothing recognisable was said.
        if unk_re().replace_all(text, "").trim().is_empty() {
            self.tracker.last_outcome = Outcome::Idle;
            return;
        }
        if self.phase.listening() && (is_final || text != self.last_heard) {
            self.heard_ago_ms = Some(0);
            self.last_heard = text.to_owned();
        }
        if self.phase != Phase::Reading {
            return;
        }
        self.tracker.accept(text, is_final);
        self.lead.heard(&self.tracker, self.talking());
        if matches!(self.tracker.last_outcome, Outcome::Advanced | Outcome::Back) {
            self.stuck_ms = 0;
        }
        if !self.tracker.words.is_empty()
            && self.tracker.position == Some(self.tracker.words.len() - 1)
        {
            if self.playable_from(self.slide_index + 1).is_none() {
                self.phase = Phase::Finished;
            } else {
                self.remaining_ms = self.countdown_ms;
                self.phase = if self.hold_at_end {
                    Phase::Holding
                } else {
                    Phase::Countdown
                };
            }
        }
    }
    /// Tag the mic's speech must carry to be applied: changes with every new
    /// take and again when a pre-roll ends (take + sync_mark, both only ever
    /// grow), so speech recognised during "3-2-1" that is still in flight
    /// after the roll is dropped as stale.
    pub fn voice_tag(&self) -> u64 {
        self.take + self.sync_mark
    }
    /// Applies speech only if its tag is the current `voice_tag()` (or it is
    /// untagged).
    pub fn speech_for_take(&mut self, text: &str, is_final: bool, tag: Option<u64>) {
        if tag.is_some_and(|t| t != self.voice_tag()) {
            return;
        }
        self.speech(text, is_final);
    }
    pub fn tick(&mut self, elapsed: u64) {
        match self.phase {
            Phase::Countdown => {
                self.remaining_ms = self.remaining_ms.saturating_sub(elapsed);
                self.elapsed_ms += elapsed;
                if self.remaining_ms == 0 {
                    self.next();
                }
            }
            Phase::Rolling => {
                self.roll_ms = self.roll_ms.saturating_sub(elapsed);
                if self.roll_ms == 0 {
                    self.finish_roll();
                }
            }
            Phase::Reading => {
                self.elapsed_ms += elapsed;
                self.stuck_ms += self.speaking_ms(elapsed);
                self.lead.tick(&self.tracker, elapsed, self.talking());
            }
            Phase::Holding => self.elapsed_ms += elapsed,
            _ => {}
        }
        if self.phase.listening() {
            self.heard_ago_ms = self.heard_ago_ms.map(|m| m.saturating_add(elapsed));
        }
    }
    /// The still-speaking part of the next `elapsed` ms that counts toward
    /// the lost-place hint (0 when the hint is off or the reader is quiet).
    fn speaking_ms(&self, elapsed: u64) -> u64 {
        let heard_ago = self.heard_ago_ms.unwrap_or(SPEAKING_MS);
        if self.lost_after_ms > 0 && self.heard_ago_ms.is_some_and(|m| m < SPEAKING_MS) {
            elapsed.min(SPEAKING_MS - heard_ago)
        } else {
            0
        }
    }
    /// True while the reader seems to be mid-sentence (for the cursor).
    fn talking(&self) -> bool {
        self.phase == Phase::Reading && self.heard_ago_ms.is_some_and(|m| m < TALKING_MS)
    }
    /// The word to show as "you are here": the one being said now, not the
    /// last one the microphone reported. None on an empty slide.
    pub fn cursor(&self) -> Option<usize> {
        self.lead.cursor(&self.tracker, self.talking())
    }
    /// True while new speech arrived within the last HEARD_MS.
    pub fn hearing(&self) -> bool {
        self.phase.listening() && self.heard_ago_ms.is_some_and(|m| m < HEARD_MS)
    }
    /// True when the reader has been speaking for lost_after_ms without the
    /// position moving.
    pub fn lost(&self) -> bool {
        self.phase == Phase::Reading
            && self.lost_after_ms > 0
            && self.stuck_ms >= self.lost_after_ms
    }
    /// Hides the hint until the reader speaks for lost_after_ms again.
    pub fn dismiss_lost(&mut self) {
        self.stuck_ms = 0;
    }
    /// Word-index range of the next `n` expected words, extended to the end of
    /// the unit containing its last word. None at the end of the slide.
    pub fn upcoming(&self, n: usize) -> Option<std::ops::Range<usize>> {
        let words = &self.tracker.words;
        let next = self.tracker.position.map_or(0, |p| p + 1);
        if next >= words.len() {
            return None;
        }
        Some(next..unit_end(words, (next + n.max(1)).min(words.len()) - 1) + 1)
    }
    /// True when tick(elapsed) would change anything the prompter shows.
    /// Pure; does not tick.
    pub fn tick_changes_view(&self, elapsed: u64) -> bool {
        match self.phase {
            _ if elapsed == 0 => false,
            Phase::Countdown | Phase::Rolling => true,
            Phase::Reading | Phase::Holding => {
                let heard = self.heard_ago_ms.map(|m| m.saturating_add(elapsed));
                let hearing = heard.is_some_and(|m| m < HEARD_MS);
                let stuck = if self.phase == Phase::Reading {
                    self.stuck_ms + self.speaking_ms(elapsed)
                } else {
                    self.stuck_ms
                };
                let lost = self.phase == Phase::Reading
                    && self.lost_after_ms > 0
                    && stuck >= self.lost_after_ms;
                let cursor = self.phase == Phase::Reading
                    && self.lead.cursor_after(&self.tracker, elapsed, self.talking()) != self.cursor();
                cursor
                    || hearing != self.hearing()
                    || lost != self.lost()
                    || (self.elapsed_ms + elapsed) / 1000 != self.elapsed_ms / 1000
            }
            _ => false,
        }
    }
    /// Estimated time to finish from here: words after the position on this
    /// slide and on every later playable slide at `wpm`, plus a transition gap
    /// per later playable slide (countdown_ms unless hold_at_end), with the
    /// current remaining_ms replacing the first gap while in Countdown (also
    /// when Paused with resume Countdown), and roll_ms added while Rolling.
    /// In Holding the first gap is 0. 0 when Finished.
    pub fn time_left_ms(&self) -> u64 {
        if self.phase == Phase::Finished {
            return 0;
        }
        let phase = if self.phase == Phase::Paused {
            self.resume
        } else {
            self.phase
        };
        let here = self
            .tracker
            .words
            .len()
            .saturating_sub(self.tracker.position.map_or(0, |p| p + 1));
        let later: Vec<usize> = self.script.slides
            [(self.slide_index + 1).min(self.script.slides.len())..]
            .iter()
            .map(|s| words(&s.text).len())
            .filter(|c| *c > 0)
            .collect();
        let gap = if self.hold_at_end {
            0
        } else {
            self.countdown_ms
        };
        let gaps = match (later.len() as u64, phase) {
            (0, _) => 0,
            (n, Phase::Countdown) => self.remaining_ms + (n - 1) * gap,
            (n, Phase::Holding) => (n - 1) * gap,
            (n, _) => n * gap,
        };
        let roll = if phase == Phase::Rolling {
            self.roll_ms
        } else {
            0
        };
        estimate_ms(here + later.iter().sum::<usize>(), self.wpm) + gaps + roll
    }
    pub fn next(&mut self) {
        if let Some(i) = self.playable_from(self.slide_index + 1) {
            self.move_to(i);
        } else {
            self.phase = Phase::Finished;
            self.remaining_ms = self.countdown_ms;
            self.roll_ms = 0;
        }
    }
    pub fn previous(&mut self) {
        let index = (0..self.slide_index)
            .rev()
            .find(|i| !words(&self.script.slides[*i].text).is_empty())
            .or_else(|| self.playable_from(0));
        if let Some(i) = index {
            self.move_to(i);
        }
    }
    pub fn restart(&mut self) {
        if let Some(i) = self.playable_from(0) {
            self.move_to(i);
            self.elapsed_ms = 0;
        }
    }
    pub fn jump(&mut self, index: usize) {
        if index < self.script.slides.len() && !words(&self.script.slides[index].text).is_empty() {
            self.move_to(index);
        }
    }
    fn move_to(&mut self, i: usize) {
        let paused = matches!(self.phase, Phase::Ready | Phase::Paused);
        let rolling = self.phase == Phase::Rolling;
        self.slide_index = i;
        self.tracker = Self::new_tracker(&self.script.slides[i].text, self.voice_resync);
        self.remaining_ms = self.countdown_ms;
        self.take += 1;
        self.resume = Phase::Reading;
        self.phase = if paused {
            Phase::Paused
        } else {
            Phase::Reading
        };
        self.roll_ms = 0;
        self.clear_heard();
        if rolling {
            self.enter_roll();
        }
    }
    pub fn seek(&mut self, next_word: usize) {
        self.rewind_to(next_word, false);
    }
    /// Jumps to the start of the current sentence, or the previous one when
    /// already within its first two words.
    pub fn back_sentence(&mut self) {
        let next = self.tracker.position.map_or(0, |p| p + 1);
        let starts = self
            .slide()
            .map(|s| sentence_starts(&s.text, &self.tracker.words))
            .unwrap_or_default();
        let c = starts
            .iter()
            .copied()
            .filter(|&s| s <= next)
            .max()
            .unwrap_or(0);
        let target = if next - c >= 2 {
            c
        } else {
            starts.iter().copied().filter(|&s| s < c).max().unwrap_or(0)
        };
        self.rewind_to(target, true);
    }
    /// Restarts the current slide's reading from the first word.
    pub fn retake(&mut self) {
        self.rewind_to(0, true);
    }
    /// Shared seek rule: moves the tracker, starts a new take and leaves
    /// Ready/Paused alone while any active phase goes back to Reading, or to
    /// a fresh pre-roll when `roll` is set. Without `roll` a running pre-roll
    /// carries on. A slide with no words keeps its phase (nothing to read).
    pub(crate) fn rewind_to(&mut self, next_word: usize, roll: bool) {
        self.tracker.seek(next_word);
        self.remaining_ms = self.countdown_ms;
        self.resume = Phase::Reading;
        self.take += 1;
        self.clear_heard();
        if self.tracker.words.is_empty() {
            return;
        }
        match self.phase {
            Phase::Ready | Phase::Paused => {}
            Phase::Rolling if !roll => {}
            _ => {
                self.phase = Phase::Reading;
                self.roll_ms = 0;
                if roll {
                    self.enter_roll();
                }
            }
        }
    }
    pub fn apply_revision(&mut self, update: Script) -> bool {
        if self.phase.running() || update.id != self.script.id {
            return false;
        }
        let was_holding = self.phase == Phase::Holding;
        let paused_holding = self.phase == Phase::Paused && self.resume == Phase::Holding;
        let old_id = self.slide().map(|s| s.id.clone());
        let anchor: Vec<_> = self
            .tracker
            .position
            .map(|p| {
                self.tracker.words[p.saturating_sub(2)..=p]
                    .iter()
                    .map(|w| w.text.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.script = update;
        let found = self
            .script
            .slides
            .iter()
            .position(|s| Some(&s.id) == old_id.as_ref() && !words(&s.text).is_empty());
        self.slide_index = found.or_else(|| self.playable_from(0)).unwrap_or(0);
        self.tracker = Self::new_tracker(
            self.slide().map(|s| s.text.as_str()).unwrap_or(""),
            self.voice_resync,
        );
        if found.is_some() && !anchor.is_empty() {
            let normalized: Vec<_> = self.tracker.words.iter().map(|w| w.text.clone()).collect();
            let matches: Vec<_> = normalized
                .windows(anchor.len())
                .enumerate()
                .filter(|(_, v)| *v == anchor)
                .map(|(i, _)| i)
                .collect();
            if matches.len() == 1 {
                self.tracker.seek(matches[0] + anchor.len());
            }
        }
        self.remaining_ms = self.countdown_ms;
        self.take += 1;
        self.resume = Phase::Reading;
        self.clear_heard();
        if was_holding || paused_holding {
            let at_end = !self.tracker.words.is_empty()
                && self.tracker.position == Some(self.tracker.words.len() - 1);
            let keep =
                found.is_some() && at_end && self.playable_from(self.slide_index + 1).is_some();
            if was_holding && !keep {
                self.phase = Phase::Paused;
            }
            if paused_holding && keep {
                self.resume = Phase::Holding;
            }
        }
        if self.tracker.words.is_empty() {
            self.phase = Phase::Finished;
        }
        true
    }
    pub fn position(&self) -> Position {
        Position {
            script_id: self.script.id.clone(),
            revision: self.script.revision,
            slide_id: self.slide().map(|s| s.id.clone()).unwrap_or_default(),
            next_word: self.tracker.position.map(|p| p + 1).unwrap_or(0),
            finished: self.phase == Phase::Finished,
            words_scheme: WORDS_SCHEME,
        }
    }
    pub fn restore(&mut self, saved: &Position) {
        if saved.finished
            || saved.script_id != self.script.id
            || saved.revision != self.script.revision
            || saved.words_scheme != WORDS_SCHEME
        {
            return;
        }
        if let Some(i) = self
            .script
            .slides
            .iter()
            .position(|s| s.id == saved.slide_id && !words(&s.text).is_empty())
        {
            self.slide_index = i;
            self.tracker = Self::new_tracker(&self.script.slides[i].text, self.voice_resync);
            self.tracker.seek(saved.next_word);
            self.phase = Phase::Ready;
            self.roll_ms = 0;
            self.clear_heard();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub font_size: f64,
    pub margins: f64,
    pub line_spacing: f64,
    pub guide: f64,
    pub show_guide: bool,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub orientation: String,
    pub hold_at_end: bool,
    pub countdown_secs: f64,
    pub pre_roll_secs: f64,
    pub sync_flash: bool,
    pub speaking_wpm: f64,
    /// "Lost your place?" popup while voice tracking. Off by default; renamed
    /// from show_lost_hint so old saved settings (which had it on) reset to off.
    pub lost_hint_popup: bool,
    pub lost_place_secs: f64,
    pub voice_resync: bool,
    pub volume_keys: bool,
    pub script_grammar: bool,
    /// Seconds the highlight runs ahead of the microphone, which always
    /// hears late. More if the highlight trails your voice, less if it leads.
    pub highlight_lead: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 42.0,
            margins: 7.0,
            line_spacing: 1.4,
            guide: 35.0,
            show_guide: true,
            mirror_x: false,
            mirror_y: false,
            orientation: "auto".into(),
            hold_at_end: false,
            countdown_secs: 5.0,
            pre_roll_secs: 0.0,
            sync_flash: true,
            speaking_wpm: 150.0,
            lost_hint_popup: false,
            lost_place_secs: 5.0,
            voice_resync: true,
            volume_keys: false,
            script_grammar: false,
            highlight_lead: 0.6,
        }
    }
}
impl Settings {
    pub fn clamp(&mut self) {
        self.font_size = self.font_size.clamp(22.0, 90.0);
        self.margins = self.margins.clamp(2.0, 20.0);
        self.line_spacing = self.line_spacing.clamp(1.0, 2.4);
        self.guide = self.guide.clamp(15.0, 60.0);
        self.countdown_secs = self.countdown_secs.clamp(1.0, 15.0);
        self.pre_roll_secs = self.pre_roll_secs.clamp(0.0, 10.0);
        self.speaking_wpm = self.speaking_wpm.clamp(80.0, 240.0);
        self.lost_place_secs = self.lost_place_secs.clamp(2.0, 15.0);
        self.highlight_lead = self.highlight_lead.clamp(0.0, 1.5);
        if !["auto", "portrait", "landscape"].contains(&self.orientation.as_str()) {
            self.orientation = "auto".into();
        }
    }
}

pub fn estimate_ms(word_count: usize, wpm: f64) -> u64 {
    (word_count as f64 * 60000.0 / wpm.max(1.0)).round() as u64
}
pub fn slide_estimate_ms(slide: &Slide, wpm: f64) -> u64 {
    estimate_ms(words(&slide.text).len(), wpm)
}
/// All slides plus one transition gap between consecutive playable slides:
/// countdown_secs when !hold_at_end, 0 when holding. Empty slides count 0.
pub fn script_estimate_ms(script: &Script, settings: &Settings) -> u64 {
    let counts: Vec<usize> = script.slides.iter().map(|s| words(&s.text).len()).collect();
    let playable = counts.iter().filter(|c| **c > 0).count() as u64;
    let gap = if settings.hold_at_end {
        0
    } else {
        (settings.countdown_secs * 1000.0).round() as u64
    };
    estimate_ms(counts.iter().sum(), settings.speaking_wpm) + playable.saturating_sub(1) * gap
}
/// Elapsed time as format_duration shows it, but with seconds rounded down,
/// so the shown second changes exactly when `elapsed_ms / 1000` does.
pub fn format_elapsed(ms: u64) -> String {
    format_duration(ms / 1000 * 1000)
}
/// "0:42", "12:05", "1:02:03" (seconds rounded up; 0 → "0:00").
pub fn format_duration(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub const GRAMMAR_MAX: usize = 3000;
/// Sorted, de-duplicated tokens of all slides (normalised by words()), plus
/// all number words, plus "chapter", "verse", "verses", "to", "through",
/// "and", "a", "oh", "point", "dollar", "dollars", "cent", "cents",
/// "percent", with "[unk]" appended last. Empty when no slide has words.
pub fn vocabulary(script: &Script) -> Vec<String> {
    const EXTRA: [&str; 14] = [
        "chapter", "verse", "verses", "to", "through", "and", "a", "oh", "point", "dollar",
        "dollars", "cent", "cents", "percent",
    ];
    let mut v: Vec<String> = script.slides.iter().flat_map(|s| tokens(&s.text)).collect();
    if v.is_empty() {
        return v;
    }
    v.extend(numbers::number_words().chain(EXTRA).map(String::from));
    v.sort_unstable();
    v.dedup();
    v.push("[unk]".into());
    v
}
/// JSON array of vocabulary(script) for Vosk, or None when the script has
/// no playable words or the vocabulary has more than GRAMMAR_MAX entries.
pub fn grammar_json(script: &Script) -> Option<String> {
    let v = vocabulary(script);
    if v.is_empty() || v.len() > GRAMMAR_MAX {
        return None;
    }
    serde_json::to_string(&v).ok()
}
/// Why grammar_json returned None, for the UI message: "empty" | "too_large".
pub fn grammar_skip_reason(script: &Script) -> Option<&'static str> {
    let n = vocabulary(script).len();
    if n == 0 {
        Some("empty")
    } else if n > GRAMMAR_MAX {
        Some("too_large")
    } else {
        None
    }
}

pub fn demo() -> Script {
    Script { id: "rusty-demo".into(), title: "Meet RustyPrompter".into(), revision: 1, slides: vec![
    Slide { id: "welcome".into(), title: "Your voice. Your pace.".into(), text: "[Look at the lens]\n\nWelcome to RustyPrompter. I can take my time, skip a few words, and keep reading naturally.\n\nWhen I finish this slide, a five second countdown gives me a moment before the next one.".into() },
    Slide { id: "next".into(), title: "Ready for the next take".into(), text: "This is the next slide. My scripts come from the computer, and voice following works right here on my tablet.\n\nNo subscription. Just my words, at my pace. That is the end of this demonstration.".into() },
] }
}
