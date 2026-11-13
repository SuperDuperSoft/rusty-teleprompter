//! Random input must never panic or leave the position out of range.
mod sim;
use rusty_core::*;
use sim::Rng;

const TEXTS: &[&str] = &[
    "",
    "Amen",
    "[pause] [look up]",
    "Yes yes yes yes yes yes yes yes yes yes yes yes",
    "John 3:16 says $5 on the 21st. [smile] Then 1,250 people came, and 3 left.",
    "The the the and and and of of of to to to a a a in in in",
    include_str!("fixtures/long-slide.txt"),
];
const NOISE: &[&str] = &["um", "[unk]", "[UNK]", "zzunk", "chapter", "verse", "and", "hundred", "a", "one", "three", "sixteen", "", "  ", "thé", "🙂", "o'clock"];

#[test]
fn random_speech_never_breaks_the_tracker() {
    for (n, text) in TEXTS.iter().enumerate() {
        for seed in 1..=40 {
            let mut rng = Rng::new(seed * 31 + n as u64);
            let mut t = Tracker::new(text);
            t.allow_back = rng.chance(0.8);
            let vocabulary: Vec<String> = t.words.iter().map(|w| w.text.clone()).collect();
            let mut said: Vec<String> = vec![];
            for _ in 0..300 {
                match rng.range(0, 20) {
                    0 => said.clear(),
                    1 => {
                        said.pop();
                    }
                    2 => t.seek(rng.range(0, vocabulary.len() as u64 + 3) as usize),
                    3 => t.reset_utterance(),
                    _ => {
                        for _ in 0..rng.range(1, 4) {
                            let next = t.position.map_or(0, |p| p + 1);
                            let word = if !vocabulary.is_empty() && rng.chance(0.6) {
                                // Mostly the next script word, sometimes any.
                                let i = if rng.chance(0.7) { next.min(vocabulary.len() - 1) } else { rng.range(0, vocabulary.len() as u64) as usize };
                                vocabulary[i].clone()
                            } else {
                                (*rng.pick(NOISE)).to_string()
                            };
                            said.push(word);
                        }
                    }
                }
                let is_final = rng.chance(0.15);
                let before = t.position;
                let after = t.accept(&said.join(" "), is_final);
                assert_eq!(after, t.position);
                assert!(after.is_none_or(|p| p < t.words.len()), "out of range in {text:?}");
                if t.last_outcome != Outcome::Back {
                    assert!(after >= before, "moved back without a Back outcome");
                }
                if is_final {
                    said.clear();
                }
            }
        }
    }
}
#[test]
fn random_speech_never_breaks_a_session() {
    for seed in 1..=30 {
        let mut rng = Rng::new(seed);
        let slides = TEXTS
            .iter()
            .enumerate()
            .map(|(i, t)| Slide { id: format!("s{i}"), title: String::new(), text: (*t).into() })
            .collect();
        let mut s = Session::new(Script { id: "f".into(), title: "F".into(), revision: 1, slides });
        s.configure(&Settings { highlight_lead: rng.unit() * 1.5, pre_roll_secs: 0.0, ..Settings::default() });
        s.start();
        let mut said = String::new();
        for _ in 0..600 {
            match rng.range(0, 30) {
                0 => s.next(),
                1 => s.previous(),
                2 => s.back_sentence(),
                3 => s.retake(),
                4 => s.pause(),
                5 => s.start(),
                6 => s.seek(rng.range(0, 40) as usize),
                _ => {
                    let next = s.tracker.position.map_or(0, |p| p + 1);
                    let word = match s.tracker.words.get(next) {
                        Some(w) if rng.chance(0.75) => w.text.clone(),
                        _ => (*rng.pick(NOISE)).to_string(),
                    };
                    said.push(' ');
                    said.push_str(&word);
                    let is_final = rng.chance(0.2);
                    s.speech(&said, is_final);
                    if is_final {
                        said.clear();
                    }
                }
            }
            let elapsed = rng.range(0, 400);
            let _ = s.tick_changes_view(elapsed);
            s.tick(elapsed);
            let len = s.tracker.words.len();
            match s.cursor() {
                None => assert_eq!(len, 0),
                Some(c) => {
                    assert!(c < len);
                    // Never behind the heard words, never far ahead of them.
                    let head = s.tracker.position.map_or(0, |p| p + 1).min(len - 1);
                    assert!(c >= head && c <= head + 3, "cursor {c}, heard up to {head}");
                }
            }
        }
    }
}
