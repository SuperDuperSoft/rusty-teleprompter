//! Shared deterministic script matching and playback. No clocks, audio, UI or IO.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

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
/// line, and capitalised/numeric words after `.`, `?` or `!` (not abbreviations).
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
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());
        let ends = gap.contains(['.', '?', '!'])
            && capital
            && !ABBREVIATIONS.contains(&words[i - 1].text.as_str());
        if blank || ends {
            starts.push(i);
        }
    }
    starts
}

pub fn words(text: &str) -> Vec<Word> {
    static WORD: OnceLock<Regex> = OnceLock::new();
    static CUE: OnceLock<Regex> = OnceLock::new();
    let re = WORD.get_or_init(|| Regex::new(r"[\p{L}\p{N}]+(?:['’][\p{L}]+)?").unwrap());
    let cues: Vec<_> = CUE
        .get_or_init(|| Regex::new(r"\[[\s\S]*?\]").unwrap())
        .find_iter(text)
        .map(|m| m.range())
        .collect();
    re.find_iter(text)
        .filter(|m| !cues.iter().any(|c| c.contains(&m.start())))
        .map(|m| {
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
            Word {
                text: normalized.to_owned(),
                start: m.start(),
                end: m.end(),
            }
        })
        .collect()
}
fn tokens(s: &str) -> Vec<String> {
    words(s).into_iter().map(|w| w.text).collect()
}

#[derive(Clone, Debug)]
pub struct Tracker {
    pub words: Vec<Word>,
    pub position: Option<usize>,
    partial_end: Option<usize>,
    votes: usize,
    far_end: Option<usize>,
    far_votes: usize,
    context: Vec<String>,
}
impl Tracker {
    pub fn new(text: &str) -> Self {
        Self {
            words: words(text),
            position: None,
            partial_end: None,
            votes: 0,
            far_end: None,
            far_votes: 0,
            context: vec![],
        }
    }
    pub fn seek(&mut self, next_word: usize) {
        self.position = next_word.min(self.words.len()).checked_sub(1);
        self.reset_utterance();
    }
    pub fn reset_utterance(&mut self) {
        self.partial_end = None;
        self.votes = 0;
        self.far_end = None;
        self.far_votes = 0;
        self.context.clear();
    }
    pub fn accept(&mut self, hypothesis: &str, is_final: bool) -> Option<usize> {
        let incoming = tokens(hypothesis);
        if incoming.is_empty() || self.words.is_empty() {
            return self.position;
        }
        let mut heard = self.context.clone();
        let mut fresh_from = heard.len();
        heard.extend(incoming);
        if heard.len() > 24 {
            let drop = heard.len() - 24;
            heard.drain(..drop);
            fresh_from = fresh_from.saturating_sub(drop);
        }
        if is_final {
            self.context = heard
                .iter()
                .skip(heard.len().saturating_sub(12))
                .cloned()
                .collect();
        }
        let pos = self.position.map(|p| p as isize).unwrap_or(-1);
        let left = (pos - 24).max(0) as usize;
        // Search the whole slide; distance rules below keep far moves honest.
        let right = self.words.len();
        let n = right - left;
        let rows = heard.len() + 1;
        let mut scores = vec![vec![0.0_f64; n + 1]; rows];
        let mut cells = vec![vec![Cell::default(); n + 1]; rows];
        let mut best: Option<(f64, usize, usize, bool)> = None;
        for i in 1..=heard.len() {
            let fresh = i > fresh_from;
            for j in 1..=n {
                let script_word = &self.words[left + j - 1].text;
                let equal = similar(&heard[i - 1], script_word);
                let common = is_common(script_word);
                let gain = if !equal {
                    -1.6
                } else if common {
                    1.5
                } else {
                    3.0
                };
                let diagonal = scores[i - 1][j - 1] + gain;
                let skip = scores[i][j - 1] - 0.65;
                let adlib = scores[i - 1][j] - 1.4;
                let value = 0.0_f64.max(diagonal.max(skip.max(adlib)));
                scores[i][j] = value;
                if value == 0.0 {
                    continue;
                }
                let (pi, pj) = if value == diagonal {
                    (i - 1, j - 1)
                } else if value == skip {
                    (i, j - 1)
                } else {
                    (i - 1, j)
                };
                let matched = value == diagonal && equal;
                let here = (left + j - 1) as isize;
                let mut cell = cells[pi][pj];
                if scores[pi][pj] == 0.0 {
                    cell = Cell {
                        start: here,
                        start_row: i,
                        ..Cell::default()
                    };
                }
                if matched {
                    cell.matches += 1;
                    if fresh {
                        // Fresh matches form a run; a gap of more than four
                        // script words starts a new run at the new spot.
                        if cell.fresh == 0 || here - cell.last_fresh > 4 {
                            cell.fresh = 0;
                            cell.fresh_content = 0;
                            cell.fresh_first = here;
                        }
                        cell.fresh += 1;
                        cell.fresh_content += usize::from(!common);
                        cell.last_fresh = here;
                    }
                }
                cells[i][j] = cell;
                let end = here;
                if !matched || !fresh || i < heard.len().saturating_sub(1).max(1) || end <= pos {
                    continue;
                }
                let jump = end - pos;
                let start_jump = cell.start - pos;
                let required = if start_jump > 45 {
                    5
                } else if start_jump > 12 {
                    4
                } else {
                    3.min(self.words.len()).min(heard.len())
                };
                if cell.matches < required || value < required as f64 * 1.8 {
                    continue;
                }
                if start_jump > 12 && cell.matches as f64 / ((i + 1 - cell.start_row) as f64) < 0.6
                {
                    continue;
                }
                // Moving ahead must be earned by what was just said, right at
                // the new spot. Old context and common words cannot carry it.
                if jump > 4 {
                    let span = (end - cell.fresh_first + 1) as usize;
                    if cell.fresh < 3 || cell.fresh_content == 0 || cell.fresh * 2 < span {
                        continue;
                    }
                }
                if jump > 12 {
                    let span = (end - cell.fresh_first + 1) as usize;
                    if cell.fresh < 4 || cell.fresh_content < 2 || cell.fresh * 3 < span * 2 {
                        continue;
                    }
                }
                let far = jump > 45;
                if far && (cell.fresh < 5 || cell.fresh_content < 3) {
                    continue;
                }
                let rank = value - (jump - 4).max(0) as f64 * 0.045;
                if best.is_none_or(|b| rank > b.0) {
                    best = Some((rank, end as usize, cell.matches, far));
                }
            }
        }
        let Some((_, end, matches, far)) = best else {
            return self.position;
        };
        if end == self.words.len() - 1 && matches < 3.min(self.words.len()) {
            return self.position;
        }
        if far {
            // A far jump must be seen twice in a row at about the same spot.
            if self.far_end.is_some_and(|f| end.abs_diff(f) <= 3) {
                self.far_votes += 1;
            } else {
                self.far_votes = 1;
            }
            self.far_end = Some(end);
            if self.far_votes >= 2 {
                self.position = Some(end);
                self.far_end = None;
                self.far_votes = 0;
                self.partial_end = None;
                self.votes = 0;
            }
            return self.position;
        }
        self.far_end = None;
        self.far_votes = 0;
        // A growing partial confirms the end it showed last time, as long as
        // the newer words include something distinctive.
        if let Some(prev) = self.partial_end {
            if prev < end
                && end <= prev + 4
                && self.words[prev + 1..=end].iter().any(|w| !is_common(&w.text))
                && self.position.is_none_or(|p| prev > p)
            {
                self.position = Some(prev);
            }
        }
        if self.partial_end == Some(end) {
            self.votes += 1;
        } else {
            self.partial_end = Some(end);
            self.votes = 1;
        }
        if is_final || self.votes >= 2 {
            self.position = Some(end);
        }
        self.position
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct Cell {
    start: isize,
    start_row: usize,
    matches: usize,
    fresh: usize,
    fresh_content: usize,
    fresh_first: isize,
    last_fresh: isize,
}
/// Short, frequent English words. They still count, but weigh less and can
/// never justify moving far ahead on their own.
fn is_common(word: &str) -> bool {
    const COMMON: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "do", "for", "from", "had", "has",
        "have", "he", "her", "him", "his", "i", "if", "in", "is", "it", "its", "it's", "me", "my",
        "no", "not", "of", "on", "or", "our", "she", "so", "that", "the", "their", "them", "then",
        "there", "they", "this", "to", "up", "us", "was", "we", "were", "what", "when", "which",
        "who", "will", "with", "you", "your",
    ];
    COMMON.contains(&word)
}
fn similar(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().min(b.len()) < 5 || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ac) in a.iter().enumerate() {
        let mut next = vec![i + 1; b.len() + 1];
        for (j, bc) in b.iter().enumerate() {
            next[j + 1] = (next[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + usize::from(ac != bc));
        }
        previous = next;
    }
    previous[b.len()] <= 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Ready,
    Reading,
    Countdown,
    Holding,
    Paused,
    Finished,
}
impl Phase {
    pub fn running(self) -> bool {
        matches!(self, Self::Reading | Self::Countdown)
    }
    /// True while the mic should be live (reading, counting down or holding).
    pub fn listening(self) -> bool {
        matches!(self, Self::Reading | Self::Countdown | Self::Holding)
    }
}

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
    resume: Phase,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub script_id: String,
    pub revision: u64,
    pub slide_id: String,
    pub next_word: usize,
    pub finished: bool,
}
impl Session {
    pub fn new(script: Script) -> Self {
        let index = script
            .slides
            .iter()
            .position(|s| !words(&s.text).is_empty());
        let tracker = Tracker::new(index.map(|i| script.slides[i].text.as_str()).unwrap_or(""));
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
            resume: Phase::Reading,
        }
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
    }
    pub fn start(&mut self) {
        if self.phase == Phase::Finished {
            return;
        }
        if self.phase == Phase::Holding {
            self.next();
            return;
        }
        self.phase = if self.phase == Phase::Paused {
            self.resume
        } else {
            Phase::Reading
        };
        self.tracker.reset_utterance();
    }
    pub fn pause(&mut self) {
        if self.phase.listening() || self.phase == Phase::Ready {
            self.resume = if self.phase == Phase::Ready {
                Phase::Reading
            } else {
                self.phase
            };
            self.phase = Phase::Paused;
        }
    }
    pub fn speech(&mut self, text: &str, is_final: bool) {
        if self.phase != Phase::Reading || text.trim().is_empty() {
            return;
        }
        self.tracker.accept(text, is_final);
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
    /// Applies speech only if it belongs to the current take (or is untagged).
    pub fn speech_for_take(&mut self, text: &str, is_final: bool, take: Option<u64>) {
        if take.is_some_and(|t| t != self.take) {
            return;
        }
        self.speech(text, is_final);
    }
    pub fn tick(&mut self, elapsed: u64) {
        if self.phase == Phase::Countdown {
            self.remaining_ms = self.remaining_ms.saturating_sub(elapsed);
            if self.remaining_ms == 0 {
                self.next();
            }
        }
    }
    pub fn next(&mut self) {
        if let Some(i) = self.playable_from(self.slide_index + 1) {
            self.move_to(i);
        } else {
            self.phase = Phase::Finished;
            self.remaining_ms = self.countdown_ms;
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
        }
    }
    pub fn jump(&mut self, index: usize) {
        if index < self.script.slides.len() && !words(&self.script.slides[index].text).is_empty() {
            self.move_to(index);
        }
    }
    fn move_to(&mut self, i: usize) {
        let paused = matches!(self.phase, Phase::Ready | Phase::Paused);
        self.slide_index = i;
        self.tracker = Tracker::new(&self.script.slides[i].text);
        self.remaining_ms = self.countdown_ms;
        self.take += 1;
        self.resume = Phase::Reading;
        self.phase = if paused {
            Phase::Paused
        } else {
            Phase::Reading
        };
    }
    pub fn seek(&mut self, next_word: usize) {
        self.rewind_to(next_word);
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
        self.rewind_to(target);
    }
    /// Restarts the current slide's reading from the first word.
    pub fn retake(&mut self) {
        self.rewind_to(0);
    }
    /// Shared seek rule: moves the tracker, starts a new take and leaves
    /// Ready/Paused alone while any active phase goes back to Reading. A
    /// slide with no words keeps its phase (nothing to read).
    pub(crate) fn rewind_to(&mut self, next_word: usize) {
        self.tracker.seek(next_word);
        self.remaining_ms = self.countdown_ms;
        self.resume = Phase::Reading;
        self.take += 1;
        if !self.tracker.words.is_empty()
            && matches!(
                self.phase,
                Phase::Countdown | Phase::Holding | Phase::Finished
            )
        {
            self.phase = Phase::Reading;
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
        self.tracker = Tracker::new(self.slide().map(|s| s.text.as_str()).unwrap_or(""));
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
        }
    }
    pub fn restore(&mut self, saved: &Position) {
        if saved.finished
            || saved.script_id != self.script.id
            || saved.revision != self.script.revision
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
            self.tracker = Tracker::new(&self.script.slides[i].text);
            self.tracker.seek(saved.next_word);
            self.phase = Phase::Ready;
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
        if !["auto", "portrait", "landscape"].contains(&self.orientation.as_str()) {
            self.orientation = "auto".into();
        }
    }
}

pub fn demo() -> Script {
    Script { id: "rusty-demo".into(), title: "Meet RustyPrompter".into(), revision: 1, slides: vec![
    Slide { id: "welcome".into(), title: "Your voice. Your pace.".into(), text: "[Look at the lens]\n\nWelcome to RustyPrompter. I can take my time, skip a few words, and keep reading naturally.\n\nWhen I finish this slide, a five second countdown gives me a moment before the next one.".into() },
    Slide { id: "next".into(), title: "Ready for the next take".into(), text: "This is the next slide. My scripts come from the computer, and voice following works right here on my tablet.\n\nNo subscription. Just my words, at my pace. That is the end of this demonstration.".into() },
] }
}
