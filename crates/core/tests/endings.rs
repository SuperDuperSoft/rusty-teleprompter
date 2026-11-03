use rusty_core::*;

fn last(t: &Tracker) -> Option<usize> {
    Some(t.words.len() - 1)
}
const MID: &str = "Good morning everyone and welcome to the annual meeting of the garden \
club where members celebrate flowers vegetables and friendship together every spring \
season with great joy and happy hearts";
const FAR: &str = "Good morning everyone and welcome to the annual meeting of the garden \
club. This year members planted tomatoes peppers beans and squash. \
The children painted wooden signs for every bed and the retired teachers organised a \
watering rota that kept everything alive through the long dry summer. Our treasurer reports \
that seed costs fell while donations from local bakeries doubled. Next month volunteers \
will repair the greenhouse roof before winter storms arrive. Please collect your harvest \
baskets beside the purple shed tonight.";

#[test]
fn outcome_advanced_pending_nomatch() {
    let mut t = Tracker::new("welcome to our bright green studio");
    t.accept("welcome to our", true);
    assert_eq!(t.last_outcome, Outcome::Advanced);
    t.accept("bright green", false);
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept("purple elephants dancing", true);
    assert_eq!(t.last_outcome, Outcome::NoMatch);
}
#[test]
fn outcome_idle_on_empty() {
    let mut t = Tracker::new("");
    t.accept("hello there", true);
    assert_eq!(t.last_outcome, Outcome::Idle);
    let mut t = Tracker::new("welcome to our bright green studio");
    t.accept("purple elephants dancing", true);
    assert_eq!(t.last_outcome, Outcome::NoMatch);
    t.accept("", false);
    assert_eq!(t.last_outcome, Outcome::Idle);
    t.accept("chapter", true);
    assert_eq!(t.last_outcome, Outcome::Idle);
}
#[test]
fn mid_jump_onto_last_word_needs_final() {
    let mut t = Tracker::new(MID);
    assert!((28..=32).contains(&t.words.len()));
    t.seek(2);
    t.accept("with great joy and happy hearts", false);
    t.accept("with great joy and happy hearts", false);
    assert_eq!(t.position, Some(1));
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept("with great joy and happy hearts", true);
    assert_eq!(t.position, last(&t));
    assert_eq!(t.last_outcome, Outcome::Advanced);
}
#[test]
fn far_jump_onto_last_word_needs_final() {
    let mut t = Tracker::new(FAR);
    assert!((75..=85).contains(&t.words.len()));
    t.seek(2);
    let said = "please collect your harvest baskets beside the purple shed tonight";
    t.accept(said, false);
    t.accept(said, false);
    assert_eq!(t.position, Some(1));
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept(said, true);
    assert_eq!(t.position, last(&t));
}
#[test]
fn far_partial_then_final_finishes() {
    let mut t = Tracker::new(FAR);
    t.seek(2);
    let said = "please collect your harvest baskets beside the purple shed tonight";
    t.accept(said, false);
    assert_eq!(t.position, Some(1));
    t.accept(said, true);
    assert_eq!(t.position, last(&t));
}
#[test]
fn ending_needs_content_word() {
    let mut t = Tracker::new("welcome to our bright green studio and you");
    t.accept("welcome to our bright green studio", true);
    assert_eq!(t.position, Some(5));
    t.accept("you", true);
    assert_eq!(t.position, Some(5));
    assert_eq!(t.last_outcome, Outcome::Pending);
}
#[test]
fn all_common_ending_can_finish() {
    let mut t = Tracker::new("Thank you for listening and that is it for us");
    for s in [
        "thank you for listening",
        "thank you for listening",
        "thank you for listening and that is",
        "thank you for listening and that is it for us",
        "thank you for listening and that is it for us",
    ] {
        t.accept(s, false);
    }
    assert_eq!(t.position, last(&t));
    let mut t = Tracker::new("Thank you for listening and that is it for us");
    t.accept("thank you for listening and that", true);
    assert_eq!(t.position, Some(5));
    t.accept("is it for us", true);
    assert_eq!(t.position, last(&t));
}
#[test]
fn separate_final_common_tail_finishes() {
    let mut t = Tracker::new("We are so glad. Thank you all for coming to this");
    t.accept("we are so glad thank you all for coming", true);
    assert_eq!(t.position, Some(8));
    t.accept("to this", true);
    assert_eq!(t.position, last(&t));
}
#[test]
fn word_by_word_finals_finish_common_ending() {
    let mut t = Tracker::new("We could not have done it without you so thank all of you");
    t.accept("we could not have done it without you so thank", true);
    assert_eq!(t.position, Some(9));
    for (word, at) in [("all", 10), ("of", 11), ("you", 12)] {
        t.accept(word, true);
        assert_eq!(t.position, Some(at));
    }
}
#[test]
fn normal_read_still_finishes_with_partials() {
    let mut t = Tracker::new("Today we are making a very useful local teleprompter for the studio");
    t.accept("today we are making a very useful local", true);
    assert_eq!(t.position, Some(7));
    let said = ["teleprompter", "teleprompter for", "teleprompter for the"];
    for s in said {
        t.accept(s, false);
    }
    t.accept("teleprompter for the studio", false);
    assert_ne!(t.position, last(&t));
    t.accept("teleprompter for the studio", false);
    assert_eq!(t.position, last(&t));
}
#[test]
fn pause_before_final_common_words_finishes() {
    // we0 know1 the2 road3 is4 long5 but6 god7 is8 in9 it10 with11 us12
    let mut t = Tracker::new("We know the road is long but God is in it with us");
    t.accept("we know the road is long but god is in it with", true);
    assert_eq!(t.position, Some(11));
    t.accept("us", true);
    assert_eq!(t.position, last(&t));
    assert_eq!(t.last_outcome, Outcome::Advanced);
}
#[test]
fn common_tail_after_skipped_content_word_still_waits() {
    // The content word "studio" sits between the last heard content word and
    // the end, so a lone "you" cannot finish.
    let mut t = Tracker::new("welcome to our bright green and big studio and you");
    t.accept("welcome to our bright green and big", true);
    assert_eq!(t.position, Some(6));
    t.accept("and you", true);
    assert_ne!(t.position, last(&t));
}
