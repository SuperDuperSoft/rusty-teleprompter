//! A pretend reader and a pretend microphone, for measuring voice following.
//!
//! The reader says a slide word by word at a human pace, pausing at
//! punctuation. The recognizer behaves like Vosk does on the tablet (see
//! `fixtures/offline-voice.tsv`): it hears each word late, repeats its
//! growing partial several times a second, gets some words wrong, changes
//! its mind about the newest word, and sends a final after a pause.
//!
//! Everything is seeded, so a run always gives the same numbers.
#![allow(dead_code)]
use rusty_core::*;

/// xorshift64*: small, seeded, good enough for a simulation.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// 0.0..1.0
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        low + (self.unit() * (high - low) as f64) as u64
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub wpm: f64,
    /// A word is heard as a different word.
    pub substitute: f64,
    /// A word is not heard at all.
    pub drop: f64,
    /// An extra word is heard that nobody said.
    pub insert: f64,
    /// The newest word of a partial is wrong at first, then corrected.
    pub unstable: f64,
    /// How long after a word ends the recognizer shows it (ms, low..high).
    pub latency: (u64, u64),
    /// How often the recognizer repeats its partial (ms).
    pub partial_every: u64,
    /// The reader skips this many words once, at this word index.
    pub skip: Option<(usize, usize)>,
    /// The reader goes back and re-reads from `to`, once, after saying `at`.
    pub reread: Option<(usize, usize)>,
    /// The reader says this many words of their own, once, before this word.
    pub adlib: Option<(usize, usize)>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            wpm: 150.0,
            substitute: 0.10,
            drop: 0.03,
            insert: 0.02,
            unstable: 0.20,
            latency: (300, 600),
            partial_every: 200,
            skip: None,
            reread: None,
            adlib: None,
        }
    }
}

/// One thing the reader says: a script word, or `None` for a word of their own.
#[derive(Clone, Copy, Debug)]
pub struct Spoken {
    pub word: Option<usize>,
    pub start: u64,
    pub end: u64,
}
/// One message from the recognizer.
#[derive(Clone, Debug)]
pub struct Event {
    pub at: u64,
    pub text: String,
    pub is_final: bool,
}
pub struct Take {
    pub spoken: Vec<Spoken>,
    pub events: Vec<Event>,
    pub length: u64,
}

const JUNK: &[&str] = &[
    "um", "like", "okay", "well", "really", "thing", "going", "people", "right", "just", "know",
    "little", "about", "would", "there", "because", "those", "other", "into", "some",
];

/// Reads `text` once and returns what was said when, and what the recognizer
/// reported when.
pub fn take(text: &str, seed: u64, o: Options) -> Take {
    let mut rng = Rng::new(seed);
    let parts = words(text);
    let vocabulary: Vec<&str> = parts.iter().map(|w| w.text.as_str()).collect();
    // The order the reader says the script words in.
    let mut order: Vec<Option<usize>> = vec![];
    let mut i = 0;
    let (mut skipped, mut reread, mut adlibbed) = (false, false, false);
    while i < parts.len() {
        if let Some((at, n)) = o.adlib.filter(|a| !adlibbed && a.0 == i) {
            let _ = at;
            adlibbed = true;
            order.extend(std::iter::repeat_n(None, n));
        }
        if let Some((_, n)) = o.skip.filter(|s| !skipped && s.0 == i) {
            skipped = true;
            i += n;
            continue;
        }
        order.push(Some(i));
        if let Some((_, to)) = o.reread.filter(|r| !reread && r.0 == i) {
            reread = true;
            i = to;
            continue;
        }
        i += 1;
    }
    // When each is said.
    let beat = 60_000.0 / o.wpm;
    let mut spoken = vec![];
    let mut now = 400_u64;
    for (n, entry) in order.iter().enumerate() {
        let letters = entry.map_or(4, |w| parts[w].text.chars().count());
        let shape = 0.55 + letters as f64 * 0.11;
        let length = (beat * 0.78 * shape * (0.8 + rng.unit() * 0.4)) as u64;
        let start = now;
        let end = start + length.max(120);
        spoken.push(Spoken { word: *entry, start, end });
        // The pause after it depends on the punctuation that follows.
        let gap = match (entry, order.get(n + 1)) {
            (Some(w), Some(Some(next))) if *next == w + 1 => &text[parts[*w].end..parts[*next].start],
            (Some(_), Some(_)) => ". ",
            _ => " ",
        };
        let pause = if gap.contains('\n') {
            rng.range(900, 1400)
        } else if gap.contains(['.', '?', '!']) {
            rng.range(500, 900)
        } else if gap.contains([',', ';', ':']) {
            rng.range(150, 320)
        } else {
            (beat * 0.22 * (0.5 + rng.unit())) as u64
        };
        now = end + pause;
    }
    let length = now + 1500;
    // What the recognizer makes of each spoken word.
    struct Heard {
        text: String,
        shown_at: u64,
        /// Until this time the word shows as `wrong` instead.
        settles_at: u64,
        wrong: String,
        /// Index into `spoken`, to find utterance breaks.
        source: usize,
    }
    let mut heard: Vec<Heard> = vec![];
    for (n, s) in spoken.iter().enumerate() {
        let said = s.word.map_or_else(|| (*rng.pick(JUNK)).to_string(), |w| parts[w].text.clone());
        let mut push = |text: String, rng: &mut Rng| {
            let shown_at = s.end + rng.range(o.latency.0, o.latency.1);
            let unstable = rng.chance(o.unstable);
            heard.push(Heard {
                text,
                shown_at,
                settles_at: if unstable { shown_at + rng.range(150, 400) } else { shown_at },
                wrong: (*rng.pick(JUNK)).to_string(),
                source: n,
            });
        };
        if rng.chance(o.insert) {
            push((*rng.pick(JUNK)).to_string(), &mut rng);
        }
        if rng.chance(o.drop) {
            continue;
        }
        let text = if rng.chance(o.substitute) {
            // Half the mistakes are other words from this very slide: the
            // hardest case for a tracker.
            if rng.chance(0.5) {
                (*rng.pick(&vocabulary)).to_string()
            } else if rng.chance(0.3) {
                "[unk]".to_string()
            } else {
                (*rng.pick(JUNK)).to_string()
            }
        } else {
            said
        };
        push(text, &mut rng);
    }
    // Utterances end at a pause of 600 ms or more; the final arrives 700 ms
    // into the pause.
    let mut events = vec![];
    let mut first = 0;
    while first < heard.len() {
        let mut last = first;
        while last + 1 < heard.len() {
            let a = heard[last].source;
            let b = heard[last + 1].source;
            if b > a && spoken[b].start - spoken[b - 1].end >= 600 {
                break;
            }
            last += 1;
        }
        let final_at = spoken[heard[last].source].end + 700;
        let opens = spoken[heard[first].source].end;
        let mut t = opens - opens % o.partial_every + o.partial_every;
        while t < final_at {
            let shown: Vec<&str> = heard[first..=last]
                .iter()
                .filter(|h| h.shown_at <= t)
                .map(|h| if t < h.settles_at { h.wrong.as_str() } else { h.text.as_str() })
                .collect();
            if !shown.is_empty() {
                events.push(Event { at: t, text: shown.join(" "), is_final: false });
            }
            t += o.partial_every;
        }
        let all: Vec<&str> = heard[first..=last].iter().map(|h| h.text.as_str()).collect();
        events.push(Event { at: final_at, text: all.join(" "), is_final: true });
        first = last + 1;
    }
    events.sort_by_key(|e| e.at);
    Take { spoken, events, length }
}

/// How well the highlight kept up, sampled every `STEP` ms while reading.
#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    pub frames: usize,
    /// Mean of (shown word - word being said). 0 is perfect; negative is behind.
    pub mean: f64,
    /// Share of the time the highlight was 2 or more words behind.
    pub behind: f64,
    /// Share of the time it was more than 3 words ahead.
    pub ahead: f64,
    /// Share of the time it was on the word being said or up to 2 ahead.
    pub good: f64,
    /// Furthest it ever got ahead / behind, in words.
    pub worst_ahead: isize,
    pub worst_behind: isize,
    /// Times it moved backward.
    pub backward_moves: usize,
    /// Biggest single forward move, in words.
    pub biggest_step: usize,
    pub finished: bool,
}
pub const STEP: u64 = 50;

/// Plays a take into a fresh session and measures `shown(session)`, the word
/// index the screen marks as "you are here".
pub fn measure(text: &str, take: &Take, shown: impl Fn(&Session) -> Option<usize>) -> Report {
    let script = Script {
        id: "sim".into(),
        title: "Sim".into(),
        revision: 1,
        slides: vec![Slide { id: "s".into(), title: "S".into(), text: text.into() }],
    };
    let mut session = Session::new(script);
    session.start();
    let mut report = Report::default();
    let mut sum = 0_i64;
    let mut next_event = 0;
    let mut said = 0;
    let mut previous: Option<usize> = None;
    let mut t = 0;
    while t < take.length {
        t += STEP;
        session.tick(STEP);
        while next_event < take.events.len() && take.events[next_event].at <= t {
            let e = &take.events[next_event];
            session.speech(&e.text, e.is_final);
            next_event += 1;
        }
        if session.phase != Phase::Reading {
            report.finished = session.tracker.position == Some(session.tracker.words.len() - 1);
            break;
        }
        // The word being said now, or the next one during a pause.
        while said < take.spoken.len() && take.spoken[said].end <= t {
            said += 1;
        }
        let Some(target) = take.spoken[said.min(take.spoken.len() - 1)..]
            .iter()
            .find_map(|s| s.word)
        else {
            continue;
        };
        if t < take.spoken[0].start {
            continue;
        }
        let now = shown(&session);
        let here = now.map_or(-1, |p| p as isize);
        let error = here - target as isize;
        report.frames += 1;
        sum += error as i64;
        report.behind += f64::from(error <= -2);
        report.ahead += f64::from(error > 3);
        report.good += f64::from((0..=2).contains(&error));
        report.worst_ahead = report.worst_ahead.max(error);
        report.worst_behind = report.worst_behind.min(error);
        if let (Some(a), Some(b)) = (previous, now) {
            if b < a {
                report.backward_moves += 1;
            }
            report.biggest_step = report.biggest_step.max(b.saturating_sub(a));
        }
        previous = now.or(previous);
    }
    let n = report.frames.max(1) as f64;
    report.mean = sum as f64 / n;
    report.behind /= n;
    report.ahead /= n;
    report.good /= n;
    report
}
