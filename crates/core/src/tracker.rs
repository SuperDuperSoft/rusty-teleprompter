//! Where the reader is, from what the speech recognizer hears.
//!
//! The recognizer sends its growing guess for the current utterance several
//! times a second ("partials"), then a "final" after a pause. Each time, the
//! whole utterance is lined up against the script, starting from where the
//! reader was when the utterance began (the anchor). Three rules follow from
//! that and keep the tracker honest:
//!
//! 1. Every heard word is used once. Words that already moved the position
//!    cannot also "confirm" a copy of the same sentence somewhere else.
//! 2. A repeated, unchanged partial is not new evidence.
//! 3. The next word or two commit at once; a real skip ahead has to be
//!    earned by several distinctive words in a row at the new spot.
use crate::{cue_re, is_number_word, sentence_starts, tokens, unk_re, words, Word, UNK};
use regex::Regex;
use std::sync::OnceLock;

/// What the last `Tracker::accept` call did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    #[default]
    Idle,
    Advanced,
    Back,
    Pending,
    NoMatch,
}

// Alignment scores. A heard word either matches the next script word, was
// misheard, or is the reader's own; a script word may also be skipped.
const CONTENT: f64 = 3.0;
const COMMON: f64 = 1.5;
const WRONG: f64 = -1.3;
const JUNK: f64 = -1.0;
const SKIP: f64 = -0.65;
/// Skipping a long stretch costs this once, plus a little per word, so a
/// reader who really jumps ahead can be found without making far copies cheap.
const JUMP: f64 = -4.0;
const JUMP_STEP: f64 = -0.02;
/// Script words that may lie unheard between two matches of one run.
const NEAR: usize = 3;
/// At most this many heard words are lined up at once; older ones are
/// settled and set aside.
const ROWS: usize = 40;
const SETTLE: usize = 20;
/// How far ahead of the heard words the script is searched.
const AHEAD: usize = 600;

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Start,
    Match,
    Wrong,
    Junk,
    Skip,
    Jump,
}
/// A heard word (1-based row among the words being lined up) matched to a
/// script word.
#[derive(Clone, Copy, Debug)]
struct Pair {
    row: usize,
    index: usize,
    common: bool,
}

#[derive(Clone, Debug)]
pub struct Tracker {
    pub words: Vec<Word>,
    /// The last script word known to have been said.
    pub position: Option<usize>,
    pub last_outcome: Outcome,
    /// Lets clearly re-read earlier words move the position back.
    pub allow_back: bool,
    /// Word indices that begin a sentence.
    pub sentence_starts: Vec<usize>,
    /// The script word said just before the words being lined up (-1: none).
    anchor: isize,
    /// Leading words of the current utterance that are already used up.
    settled: Vec<String>,
    /// The last hypothesis, to recognise a repeat.
    last_tokens: Vec<String>,
    /// Row of the heard word that last moved the position (0: none yet).
    used_row: usize,
    /// A far landing seen once: (first script index, utterance length then).
    far_seen: Option<(usize, usize)>,
    /// A partial has reached the slide's last word and only needs to hold.
    end_pending: bool,
    back_end: Option<usize>,
    back_votes: usize,
    context: Vec<String>,
    cleanup: Cleanup,
}
impl Tracker {
    pub fn new(text: &str) -> Self {
        let words = words(text);
        Self {
            cleanup: Cleanup::new(text, &words),
            sentence_starts: sentence_starts(text, &words),
            words,
            position: None,
            last_outcome: Outcome::Idle,
            allow_back: true,
            anchor: -1,
            settled: vec![],
            last_tokens: vec![],
            used_row: 0,
            far_seen: None,
            end_pending: false,
            back_end: None,
            back_votes: 0,
            context: vec![],
        }
    }
    fn at(&self) -> isize {
        self.position.map_or(-1, |p| p as isize)
    }
    pub fn seek(&mut self, next_word: usize) {
        self.position = next_word.min(self.words.len()).checked_sub(1);
        self.reset_utterance();
    }
    /// Forget what was heard so far: whatever the recognizer still holds of
    /// the current utterance no longer counts.
    pub fn reset_utterance(&mut self) {
        self.settled = std::mem::take(&mut self.last_tokens);
        self.last_tokens = self.settled.clone();
        self.anchor = self.at();
        self.used_row = 0;
        self.far_seen = None;
        self.end_pending = false;
        self.back_end = None;
        self.back_votes = 0;
        self.context.clear();
    }
    pub fn accept(&mut self, hypothesis: &str, is_final: bool) -> Option<usize> {
        let before = std::mem::take(&mut self.last_outcome);
        // Back votes only survive into the very next call.
        let back_end = self.back_end.take();
        let back_votes = std::mem::take(&mut self.back_votes);
        let incoming = tokens(&unk_re().replace_all(hypothesis, format!(" {UNK} ")));
        if incoming.is_empty() || self.words.is_empty() {
            return self.position;
        }
        let mut heard = self.context.clone();
        let mut fresh_from = heard.len();
        heard.extend(incoming);
        self.cleanup.apply(&mut heard, &mut fresh_from);
        if fresh_from >= heard.len() {
            return self.position;
        }
        let utterance = heard.split_off(fresh_from);
        let last = self.words.len() - 1;
        // An unchanged partial is not new evidence and moves nothing forward.
        // It does show the hypothesis is holding still, which is what the
        // slide's last word and a move back are waiting for.
        let repeat = !is_final && utterance == self.last_tokens;
        if repeat && self.end_pending {
            self.end_pending = false;
            self.position = Some(last);
            self.last_outcome = Outcome::Advanced;
            return self.position;
        }
        let reached_end = std::mem::take(&mut self.end_pending);
        // The recognizer started a new utterance if the settled words are gone.
        if !utterance.starts_with(&self.settled) {
            self.settled.clear();
            self.anchor = self.at();
            self.used_row = 0;
            self.far_seen = None;
        }
        let pos = self.at();
        let work = &utterance[self.settled.len()..];
        let pairs = if repeat { vec![] } else { self.align(work, self.anchor) };
        // Walk the new matches in order, run by run. A run that starts after
        // a gap must prove itself; within reach, each match must be trusted.
        let news: Vec<Pair> = pairs.iter().copied().filter(|p| p.index as isize > pos).collect();
        let mut last_index = pos;
        let mut last_row = pairs
            .iter()
            .rev()
            .find(|p| p.index as isize <= pos)
            .map_or(self.used_row, |p| p.row);
        let mut good: Vec<Pair> = vec![];
        let mut pending = false;
        let mut far_noted = None;
        let mut k = 0;
        while k < news.len() {
            let gap = (news[k].index as isize - last_index - 1) as usize;
            let mut end = k + 1;
            while end < news.len() && news[end].index - news[end - 1].index - 1 <= NEAR {
                end += 1;
            }
            let run = &news[k..end];
            let count = run.len();
            let content = run.iter().filter(|p| !p.common).count();
            let span = run[count - 1].index - run[0].index + 1;
            let proven = if gap <= NEAR {
                false
            } else if gap <= 12 {
                count >= 3 && content >= 1 && count * 2 >= span
            } else if gap <= 45 {
                count >= 4 && content >= 2 && count * 3 >= span * 2
            } else {
                count >= 5 && content >= 3 && count * 3 >= span * 2
            };
            if gap > NEAR && !proven {
                pending |= count >= 2;
                break;
            }
            // A long jump must also be seen again in a longer hypothesis.
            if gap > 12 && !is_final {
                let seen = self
                    .far_seen
                    .is_some_and(|(at, len)| at.abs_diff(run[0].index) <= 3 && utterance.len() > len);
                if !seen {
                    far_noted = Some(match self.far_seen {
                        Some((at, len)) if at.abs_diff(run[0].index) <= 3 => (at, len),
                        _ => (run[0].index, utterance.len()),
                    });
                    pending = true;
                    break;
                }
            }
            for (n, p) in run.iter().enumerate() {
                let script_gap = p.index as isize - last_index - 1;
                let heard_gap = p.row as isize - last_row as isize - 1;
                let follows = script_gap == 0 && heard_gap == 0;
                let followed = run
                    .get(n + 1)
                    .is_some_and(|q| q.index == p.index + 1 && q.row == p.row + 1);
                let distinct = !p.common && script_gap <= 2 && heard_gap <= 1;
                if follows || followed || proven || distinct {
                    good.push(*p);
                } else {
                    pending = true;
                }
                last_index = p.index as isize;
                last_row = p.row;
            }
            k = end;
        }
        if !repeat {
            self.far_seen = far_noted;
        }
        // The slide's last word ends the slide, so it has to be really heard
        // and has to hold: a final, or a partial that stays put.
        if good.last().is_some_and(|p| p.index == last) {
            let tail_common = self.words[self.words.len().saturating_sub(3)..]
                .iter()
                .all(|w| is_common(&w.text));
            let unbroken = good.len() as isize == last as isize - pos
                && good[0].index as isize == pos + 1;
            let heard_it = good.iter().any(|p| !p.common)
                || (tail_common && good.len() >= 2.min(self.words.len()))
                || (unbroken && self.content_near(pos));
            let near = last as isize - pos <= 12 || is_final;
            if !(heard_it && near && (is_final || reached_end)) {
                self.end_pending = heard_it && near;
                good.pop();
                pending = true;
            }
        }
        let mut advanced = false;
        if let Some(p) = good.last() {
            self.position = Some(p.index);
            self.used_row = p.row;
            self.last_outcome = Outcome::Advanced;
            advanced = true;
        } else if repeat {
            if matches!(before, Outcome::Pending | Outcome::NoMatch) {
                self.last_outcome = before;
            }
        } else {
            self.last_outcome = if pending { Outcome::Pending } else { Outcome::NoMatch };
        }
        // Words the script ahead cannot explain may be the reader going back.
        if !advanced && self.allow_back {
            let unexplained = &work[self.used_row.min(work.len())..];
            let back = self
                .position
                .filter(|_| unexplained.len() >= 5)
                .and_then(|p| self.back_candidate(unexplained, p));
            if let Some(Back { end, .. }) = back {
                let votes = if back_end.is_some_and(|b| b.abs_diff(end) <= 2) {
                    back_votes + 1
                } else {
                    1
                };
                if is_final || votes >= 2 {
                    self.position = Some(end);
                    self.last_tokens = utterance.clone();
                    self.reset_utterance();
                    self.last_outcome = Outcome::Back;
                } else {
                    self.back_end = Some(end);
                    self.back_votes = votes;
                    self.last_outcome = Outcome::Pending;
                }
            }
        }
        if is_final {
            let mut context = std::mem::take(&mut self.context);
            context.extend(utterance);
            let drop = context.len().saturating_sub(12);
            self.context = context.split_off(drop);
            self.settled.clear();
            self.last_tokens.clear();
            self.anchor = self.at();
            self.used_row = 0;
            self.far_seen = None;
            self.end_pending = false;
            return self.position;
        }
        if self.last_outcome != Outcome::Back {
            // Long utterances: settle the oldest words where they landed.
            let work_len = utterance.len() - self.settled.len();
            if work_len > ROWS {
                let cut = work_len - SETTLE;
                if let Some(p) = pairs.iter().rev().find(|p| p.row <= cut) {
                    self.anchor = self.anchor.max((p.index as isize).min(self.at()));
                }
                let from = self.settled.len();
                self.settled.extend_from_slice(&utterance[from..from + cut]);
                self.used_row = self.used_row.saturating_sub(cut);
            }
            self.last_tokens = utterance;
        }
        self.position
    }
    /// True when a content word was said at or just before `pos`, or only
    /// common words lie between the last content word and the end: the
    /// common words that close the slide then count as its ending.
    fn content_near(&self, pos: isize) -> bool {
        if pos < 0 {
            return false;
        }
        let pos = pos as usize;
        let recent = self.words[pos.saturating_sub(3)..=pos]
            .iter()
            .any(|w| !is_common(&w.text));
        recent || self.words[pos + 1..].iter().all(|w| is_common(&w.text))
    }
    /// Lines `work` up against the script after `anchor` and returns the
    /// matched words in order. Every heard word is placed exactly once.
    fn align(&self, work: &[String], anchor: isize) -> Vec<Pair> {
        let first = (anchor + 1) as usize;
        let m = work.len();
        let n = self.words.len().saturating_sub(first).min(m + AHEAD);
        if m == 0 || n == 0 {
            return vec![];
        }
        let width = n + 1;
        let mut score = vec![f64::NEG_INFINITY; (m + 1) * width];
        let mut step = vec![Step::Start; (m + 1) * width];
        let mut origin = vec![0_u32; (m + 1) * width];
        score[0] = 0.0;
        for i in 0..=m {
            let mut jump = f64::NEG_INFINITY;
            let mut jump_from = 0;
            for j in 0..=n {
                if i == 0 && j == 0 {
                    continue;
                }
                let here = i * width + j;
                let mut best = f64::NEG_INFINITY;
                let mut how = Step::Start;
                if i > 0 && j > 0 {
                    let word = &self.words[first + j - 1].text;
                    let (gain, kind) = if !similar(&work[i - 1], word) {
                        (WRONG, Step::Wrong)
                    } else if is_common(word) {
                        (COMMON, Step::Match)
                    } else {
                        (CONTENT, Step::Match)
                    };
                    best = score[here - width - 1] + gain;
                    how = kind;
                }
                if i > 0 && score[here - width] + JUNK > best {
                    best = score[here - width] + JUNK;
                    how = Step::Junk;
                }
                if j > 0 {
                    if score[here - 1] + SKIP > best {
                        best = score[here - 1] + SKIP;
                        how = Step::Skip;
                    }
                    jump += JUMP_STEP;
                    if score[here - 1] + JUMP > jump {
                        jump = score[here - 1] + JUMP;
                        jump_from = j - 1;
                    }
                    if jump > best {
                        best = jump;
                        how = Step::Jump;
                        origin[here] = jump_from as u32;
                    }
                }
                score[here] = best;
                step[here] = how;
            }
        }
        // The best way to place every heard word; ties stay nearest.
        let mut j = 0;
        for c in 1..=n {
            if score[m * width + c] > score[m * width + j] {
                j = c;
            }
        }
        let mut i = m;
        let mut pairs = vec![];
        while i > 0 || j > 0 {
            match step[i * width + j] {
                Step::Match => {
                    let index = first + j - 1;
                    pairs.push(Pair { row: i, index, common: is_common(&self.words[index].text) });
                    i -= 1;
                    j -= 1;
                }
                Step::Wrong => {
                    i -= 1;
                    j -= 1;
                }
                Step::Junk => i -= 1,
                Step::Skip => j -= 1,
                Step::Jump => j = origin[i * width + j] as usize,
                Step::Start => break,
            }
        }
        pairs.reverse();
        pairs
    }
    /// Where the reader went back to: only the new words, aligned against the
    /// script behind `pos`, clearly better than anything near `pos`.
    fn back_candidate(&self, incoming: &[String], pos: usize) -> Option<Back> {
        let left = pos.saturating_sub(BACK_WINDOW);
        let n = pos + 1 - left;
        let rows = incoming.len();
        let mut scores = vec![vec![0.0_f64; n + 1]; rows + 1];
        let mut runs = vec![vec![Run::default(); n + 1]; rows + 1];
        for i in 1..=rows {
            for j in 1..=n {
                let here = left + j - 1;
                let script_word = &self.words[here].text;
                let equal = similar(&incoming[i - 1], script_word);
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
                let mut run = runs[pi][pj];
                if scores[pi][pj] == 0.0 {
                    run = Run {
                        start: here,
                        start_row: i,
                        ..Run::default()
                    };
                }
                if value == diagonal && equal {
                    run.matches += 1;
                    run.content += usize::from(!common);
                    run.last = here;
                    run.last_row = i;
                }
                runs[i][j] = run;
            }
        }
        let last = (1..=n)
            .map(|j| (scores[rows][j], runs[rows][j]))
            .filter(|(v, _)| *v > 0.0);
        // A nearby copy of what was said means the reader is still here.
        let mut here = 0.0_f64;
        for (v, r) in last.clone().filter(|(_, r)| r.last + BACK_MIN > pos) {
            if r.matches >= 5 {
                return None;
            }
            here = here.max(v);
        }
        let rank = |b: &Back| b.value - (pos - b.end) as f64 * 0.045;
        let mut best: Option<Back> = None;
        for (value, r) in last {
            if r.last + BACK_MIN > pos
                || r.matches < 5
                || r.content < 3
                || r.matches * 4 < (r.last - r.start + 1) * 3
                || r.matches * 5 < rows * 3
                || r.start_row > 3
                // The newest words must be the re-read ones: a copy matched
                // earlier in the utterance and then left behind is the
                // reader moving on, not going back.
                || r.last_row + 1 < rows
                || value < here + 3.0
            {
                continue;
            }
            let b = Back {
                end: r.last,
                value,
                matches: r.matches,
            };
            if best.as_ref().is_none_or(|o| {
                rank(&b) > rank(o) || (rank(&b) == rank(o) && b.matches > o.matches)
            }) {
                best = Some(b);
            }
        }
        best
    }
}
const BACK_WINDOW: usize = 80;
const BACK_MIN: usize = 4;
/// The best backward match of one `accept` call.
struct Back {
    end: usize,
    value: f64,
    matches: usize,
}
/// A run of the backward alignment; `last` is the script index of its latest
/// match and `last_row` the heard row of that match.
#[derive(Clone, Copy, Default)]
struct Run {
    start: usize,
    start_row: usize,
    matches: usize,
    content: usize,
    last: usize,
    last_row: usize,
}
const SCALES: [&str; 4] = ["hundred", "thousand", "million", "billion"];
const FILLERS: [&str; 3] = ["chapter", "verse", "verses"];
const RANGES: [&str; 3] = ["to", "through", "thru"];
/// What the recognizer adds around spoken numbers that the slide's own words
/// do not have, unless the slide really says it.
#[derive(Clone, Debug, Default)]
struct Cleanup {
    has_range: bool,
    has_filler: [bool; 3],
    and_shapes: Vec<(String, String)>,
    a_scales: Vec<String>,
    range_shapes: Vec<(String, String, String)>,
}
impl Cleanup {
    fn new(text: &str, words: &[Word]) -> Self {
        static RANGE: OnceLock<Regex> = OnceLock::new();
        let range = RANGE.get_or_init(|| Regex::new(r"[0-9]{1,3}:[0-9]{1,3}[-–][0-9]").unwrap());
        let w: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        let mut c = Self {
            has_range: range.is_match(&cue_re().replace_all(text, " ")),
            has_filler: FILLERS.map(|f| w.contains(&f)),
            ..Self::default()
        };
        for p in w.windows(2) {
            if p[0] == "a" && SCALES.contains(&p[1]) {
                c.a_scales.push(p[1].into());
            }
        }
        for t in w.windows(3) {
            if t[1] == "and" && is_number_word(t[2]) {
                c.and_shapes.push((t[0].into(), t[2].into()));
            }
            if RANGES.contains(&t[1]) && is_number_word(t[0]) && is_number_word(t[2]) {
                c.range_shapes.push((t[0].into(), t[1].into(), t[2].into()));
            }
        }
        c
    }
    /// Rewrites `heard` in place; `fresh_from` drops by the tokens removed
    /// before it.
    fn apply(&self, heard: &mut Vec<String>, fresh_from: &mut usize) {
        let number = |t: Option<&String>| t.is_some_and(|t| is_number_word(t));
        for i in 0..heard.len().saturating_sub(1) {
            if heard[i] == "a"
                && SCALES.contains(&heard[i + 1].as_str())
                && !self.a_scales.contains(&heard[i + 1])
            {
                heard[i] = "one".into();
            }
        }
        let mut pass = |drop: &dyn Fn(&[String], usize) -> bool| {
            let old = std::mem::take(heard);
            for (i, t) in old.iter().enumerate() {
                if drop(&old, i) {
                    if i < *fresh_from {
                        *fresh_from -= 1;
                    }
                } else {
                    heard.push(t.clone());
                }
            }
        };
        pass(&|h, i| {
            h[i] == "and"
                && i > 0
                && [
                    "hundred", "thousand", "million", "billion", "dollar", "dollars",
                ]
                .contains(&h[i - 1].as_str())
                && match h.get(i + 1) {
                    None => !self.and_shapes.iter().any(|s| s.0 == h[i - 1]),
                    Some(n) => {
                        is_number_word(n)
                            && !self.and_shapes.contains(&(h[i - 1].clone(), n.clone()))
                    }
                }
        });
        pass(&|h, i| {
            FILLERS
                .iter()
                .position(|f| *f == h[i])
                .is_some_and(|f| !self.has_filler[f])
                && (i + 1 == h.len() || number(h.get(i + 1)))
        });
        if self.has_range {
            pass(&|h, i| {
                RANGES.contains(&h[i].as_str())
                    && i > 0
                    && number(h.get(i - 1))
                    && number(h.get(i + 1))
                    && !self.range_shapes.contains(&(
                        h[i - 1].clone(),
                        h[i].clone(),
                        h[i + 1].clone(),
                    ))
            });
        }
    }
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
    if a == UNK || b == UNK {
        return false;
    }
    if a == b {
        return true;
    }
    let (x, y) = (a, b);
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().min(b.len()) < 5 || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    if is_number_word(x) || is_number_word(y) {
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

