use rusty_core::*;

fn last(t: &Tracker) -> Option<usize> {
    Some(t.words.len() - 1)
}

#[test]
fn spoken_chapter_and_verse_matches() {
    let mut t = Tracker::new("Read John 3:16 with me today");
    let end = t.accept("read john chapter three verse sixteen with me today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn spoken_psalm_number_matches() {
    let mut t = Tracker::new("Psalm 23 is a song");
    let end = t.accept("psalm twenty three is a song", true);
    assert_eq!(end, last(&t));
}
#[test]
fn hundred_and_matches() {
    let mut t = Tracker::new("We fed 150 people today");
    t.seek(3);
    t.accept("hundred and fifty", false);
    assert_eq!(t.accept("hundred and fifty", false), Some(4));
    t.seek(0);
    let end = t.accept("we fed one hundred and fifty people today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn a_hundred_matches() {
    let mut t = Tracker::new("We fed 100 people today");
    t.accept("a hundred people", false);
    assert_eq!(t.accept("a hundred people", false), Some(4));
    t.seek(0);
    let end = t.accept("we fed a hundred people today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn range_through_matches() {
    let mut t = Tracker::new("Romans 8:28-30 today");
    t.seek(2);
    t.accept("eight through thirty", false);
    assert_eq!(t.accept("eight through thirty", false), Some(4));
    t.seek(0);
    let end = t.accept("romans eight twenty eight through thirty today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn script_word_verse_is_not_dropped() {
    let mut t = Tracker::new("Read verse 16 aloud now");
    t.accept("read verse", false);
    assert_eq!(t.accept("read verse", false), Some(1));
    assert_eq!(t.accept("read verse sixteen aloud now", true), last(&t));
}
#[test]
fn partial_ending_in_verse_still_advances() {
    let mut t = Tracker::new("Read John 3:16 with me today");
    t.accept("john chapter three verse", false);
    assert_eq!(t.accept("john chapter three verse", false), Some(2));
}
#[test]
fn final_split_between_chapter_and_number() {
    let mut t = Tracker::new("Read John 3:16 with me today");
    assert_eq!(t.accept("read john chapter", true), Some(1));
    let end = t.accept("three sixteen with me today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn split_hundred_and() {
    let mut t = Tracker::new("We fed 150 people today");
    t.accept("we fed one hundred and", true);
    let end = t.accept("fifty people today", true);
    assert_eq!(end, last(&t));
}
#[test]
fn trailing_and_without_next_token() {
    // No "hundred and <number>" on the slide: a trailing "and" is dropped.
    let mut t = Tracker::new("We fed 100 and more today");
    t.accept("we fed one hundred and", true);
    assert_eq!(t.position, Some(3));
    // The slide says "hundred and fifty": a trailing "and" is kept.
    let mut t = Tracker::new("We counted one hundred and fifty sheep");
    t.accept("we counted one hundred and", true);
    assert_eq!(t.position, Some(4));
}
