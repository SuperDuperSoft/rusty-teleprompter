//! Voice following, measured. A pretend reader reads a long slide through a
//! pretend microphone (see `sim`), and these tests hold the result to limits:
//! the on-screen cursor stays on the word being said, and never leaps away.
mod sim;
use rusty_core::Session;
use sim::*;

const LONG: &str = include_str!("fixtures/long-slide.txt");
const SEEDS: std::ops::RangeInclusive<u64> = 1..=4;

fn cursor(s: &Session) -> Option<usize> {
    s.cursor()
}
fn heard(s: &Session) -> Option<usize> {
    s.tracker.position
}
fn runs(o: Options, shown: fn(&Session) -> Option<usize>) -> Vec<Report> {
    SEEDS.map(|seed| measure(LONG, &take(LONG, seed, o), shown)).collect()
}
fn mean(all: &[Report], f: fn(&Report) -> f64) -> f64 {
    all.iter().map(f).sum::<f64>() / all.len() as f64
}
/// Prints a row with `cargo test -- --nocapture`, for tuning.
fn show(name: &str, all: &[Report]) {
    println!(
        "{name:<26} mean {:+.2}  on word..2 ahead {:>3.0}%  2+ behind {:>3.0}%  4+ ahead {:>4.1}%  worst ahead {:>3} behind {:>4}  biggest step {:>3}",
        mean(all, |r| r.mean),
        mean(all, |r| r.good) * 100.0,
        mean(all, |r| r.behind) * 100.0,
        mean(all, |r| r.ahead) * 100.0,
        all.iter().map(|r| r.worst_ahead).max().unwrap(),
        all.iter().map(|r| r.worst_behind).min().unwrap(),
        all.iter().map(|r| r.biggest_step).max().unwrap(),
    );
}

#[test]
fn typical_microphone_keeps_the_cursor_on_the_word_being_said() {
    let all = runs(Options::default(), cursor);
    show("typical mic", &all);
    assert!(mean(&all, |r| r.mean).abs() < 0.5, "cursor drifts from the voice");
    assert!(mean(&all, |r| r.behind) < 0.08, "cursor trails the voice");
    assert!(mean(&all, |r| r.ahead) < 0.01, "cursor runs away from the voice");
    assert!(mean(&all, |r| r.good) > 0.70);
    assert!(all.iter().all(|r| r.backward_moves == 0));
}
#[test]
fn the_cursor_beats_the_raw_microphone_position() {
    // What the microphone has confirmed is always a few words behind; that
    // is what the screen used to show.
    let raw = runs(Options::default(), heard);
    show("typical mic, heard words", &raw);
    assert!(mean(&raw, |r| r.mean) < -1.5);
    assert!(mean(&raw, |r| r.behind) > 0.7);
}
#[test]
fn no_leap_on_a_long_slide_with_repeated_sentences() {
    for (name, o) in [
        ("clean mic", Options { substitute: 0.02, drop: 0.0, insert: 0.0, unstable: 0.05, ..Options::default() }),
        ("typical mic", Options::default()),
        ("bad mic", Options { substitute: 0.22, drop: 0.08, insert: 0.05, unstable: 0.3, ..Options::default() }),
    ] {
        let all = runs(o, heard);
        show(name, &all);
        for r in &all {
            assert!(r.biggest_step <= 20, "{name}: leapt {} words at once", r.biggest_step);
            assert!(r.worst_ahead <= 16, "{name}: got {} words ahead of the voice", r.worst_ahead);
        }
    }
}
#[test]
fn fast_and_slow_readers_are_followed_at_their_own_pace() {
    for (name, wpm) in [("fast reader", 190.0), ("slow reader", 110.0)] {
        let all = runs(Options { wpm, ..Options::default() }, cursor);
        show(name, &all);
        assert!(mean(&all, |r| r.mean).abs() < 0.6, "{name}");
        assert!(mean(&all, |r| r.behind) < 0.10, "{name}");
        assert!(mean(&all, |r| r.ahead) < 0.01, "{name}");
    }
}
#[test]
fn a_real_skip_ahead_is_followed() {
    let all = runs(Options { skip: Some((200, 40)), ..Options::default() }, cursor);
    show("skips 40 words", &all);
    // Behind only for the few words it takes to prove the skip.
    assert!(mean(&all, |r| r.behind) < 0.12);
    assert!(all.iter().all(|r| r.biggest_step >= 30), "the skip was never followed");
}
#[test]
fn going_back_to_re_read_is_followed() {
    let all = runs(Options { reread: Some((230, 212)), ..Options::default() }, cursor);
    show("re-reads a sentence", &all);
    assert!(all.iter().filter(|r| r.backward_moves > 0).count() >= 3);
    assert!(mean(&all, |r| r.ahead) < 0.05);
}
#[test]
fn the_readers_own_words_do_not_move_the_cursor_far() {
    let all = runs(Options { adlib: Some((300, 12)), ..Options::default() }, cursor);
    show("ad-libs 12 words", &all);
    assert!(mean(&all, |r| r.ahead) < 0.01);
    assert!(all.iter().all(|r| r.worst_ahead <= 8));
}
