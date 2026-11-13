//! How quickly and how safely heard words move the position.
use rusty_core::*;
const TEXT: &str = "Welcome to our bright green studio and the camera is ready";
// words: welcome0 to1 our2 bright3 green4 studio5 and6 the7 camera8 is9 ready10
const LONG: &str = include_str!("fixtures/long-slide.txt");

fn index_after(t: &Tracker, from: usize, word: &str) -> usize {
    from + t.words[from..].iter().position(|w| w.text == word).unwrap()
}

#[test]
fn first_partial_moves_at_once() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome", false);
    assert_eq!(t.position, Some(0));
    t.accept("welcome to our bright", false);
    assert_eq!(t.position, Some(3));
}
#[test]
fn rewritten_newest_word_never_reverses_or_overshoots() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright", false);
    t.accept("welcome to our bright green studio", false);
    assert_eq!(t.position, Some(5));
    t.accept("welcome to our bright green stadium", false);
    assert_eq!(t.position, Some(5));
    t.accept("welcome to our bright", false);
    assert_eq!(t.position, Some(5));
}
#[test]
fn common_words_right_after_the_position_follow() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright green studio", false);
    t.accept("welcome to our bright green studio and the", false);
    assert_eq!(t.position, Some(7));
}
#[test]
fn lone_common_word_after_a_gap_waits_for_the_next_word() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright green studio", true);
    // "is" is three words on; alone it proves nothing.
    t.accept("is", false);
    assert_eq!(t.position, Some(5));
    t.accept("is ready", true);
    assert_eq!(t.position, Some(10));
}
#[test]
fn one_misheard_word_does_not_stall() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to hour bright", false);
    assert_eq!(t.position, Some(3));
    t.accept("welcome to hour bright green", false);
    assert_eq!(t.position, Some(4));
}
#[test]
fn a_dropped_word_does_not_stall() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our green", false);
    assert_eq!(t.position, Some(4));
}
#[test]
fn own_words_between_script_words_are_skipped_over() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our um really bright green studio", true);
    assert_eq!(t.position, Some(5));
}
#[test]
fn repeated_partial_never_jumps_to_a_far_copy_of_the_sentence() {
    // "The grace of God is not something we earn" is on the slide twice,
    // about 480 words apart. The recognizer repeats its partial several
    // times a second; the repeats must not send the highlight to the copy.
    let mut t = Tracker::new(LONG);
    let the = index_after(&t, 25, "the");
    let earn = index_after(&t, the, "earn");
    assert_eq!(earn, the + 8);
    let far = index_after(&t, earn + 1, "earn");
    assert!(far > earn + 400);
    t.seek(the);
    let said = "the grace of god is not something we earn";
    for _ in 0..8 {
        t.accept(said, false);
        assert_eq!(t.position, Some(earn));
    }
    t.accept(said, true);
    assert_eq!(t.position, Some(earn));
}
#[test]
fn growing_partial_after_the_sentence_stays_near() {
    let mut t = Tracker::new(LONG);
    let the = index_after(&t, 25, "the");
    let earn = the + 8;
    t.seek(the);
    let mut said = String::new();
    for word in "the grace of god is not something we earn the grace of god is something we receive"
        .split(' ')
    {
        if !said.is_empty() {
            said.push(' ');
        }
        said.push_str(word);
        t.accept(&said, false);
        t.accept(&said, false);
        assert!(t.position.unwrap() <= earn + 8, "jumped to {:?} after {said:?}", t.position);
    }
    assert_eq!(t.position, Some(earn + 8));
}
#[test]
fn every_heard_word_is_used_once_across_a_long_utterance() {
    // Fifty words in one breath: older words are settled, and the position
    // keeps following to the end without ever leaving the neighbourhood.
    let mut t = Tracker::new(LONG);
    let spoken: Vec<String> = t.words[..60].iter().map(|w| w.text.clone()).collect();
    for n in 1..=spoken.len() {
        let said = spoken[..n].join(" ");
        t.accept(&said, false);
        let at = t.position.unwrap();
        assert!(at + 1 >= n.saturating_sub(1) && at < n, "{at} after {n} words");
    }
}
#[test]
fn seek_voids_what_the_recognizer_still_holds() {
    // After a tap or retake the recognizer may still send the old utterance;
    // those words must not move the new position.
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright green studio and the", false);
    t.seek(0);
    t.accept("welcome to our bright green studio and the", false);
    t.accept("welcome to our bright green studio and the camera", false);
    assert_eq!(t.position, None);
    // A new utterance counts again.
    t.accept("welcome to", false);
    assert_eq!(t.position, Some(1));
}
