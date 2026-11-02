use rusty_core::*;
const TEXT: &str = "Welcome to our bright green studio and the camera is ready";
// words: welcome0 to1 our2 bright3 green4 studio5 and6 the7 camera8 is9 ready10

#[test]
fn growing_partial_commits_previous_end() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright", false);
    assert_eq!(t.position, None);
    t.accept("welcome to our bright green studio", false);
    assert_eq!(t.position, Some(3));
}
#[test]
fn rewritten_newest_word_does_not_overshoot() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright", false);
    t.accept("welcome to our bright green studio", false);
    t.accept("welcome to our bright green stadium", false);
    assert!(t.position <= Some(4));
}
#[test]
fn common_only_extension_does_not_commit() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright green studio", false);
    t.accept("welcome to our bright green studio and the", false);
    assert_eq!(t.position, None);
}
#[test]
fn extension_beyond_four_words_does_not_commit() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our", false);
    t.accept("welcome to our bright green studio and the camera", false);
    assert_eq!(t.position, None);
}
