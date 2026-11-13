//! The on-screen cursor: the word being said now, ahead of the microphone.
use rusty_core::*;

// welcome0 to1 our2 bright3 green4 studio5 | the6 camera7 is8 ready9 and10 we11 start12 now13
const TEXT: &str = "Welcome to our bright green studio. The camera is ready and we start now";

fn reading() -> Session {
    let mut s = Session::new(Script {
        id: "s".into(),
        title: "Lead".into(),
        revision: 1,
        slides: vec![Slide { id: "a".into(), title: "A".into(), text: TEXT.into() }],
    });
    s.start();
    assert_eq!(s.phase, Phase::Reading);
    s
}

#[test]
fn starts_on_the_first_word() {
    let s = reading();
    assert_eq!(s.cursor(), Some(0));
}
#[test]
fn speech_that_matches_nothing_does_not_move_it() {
    let mut s = reading();
    s.speech("okay everybody quiet please", false);
    s.tick(400);
    s.speech("okay everybody quiet please thank you", true);
    s.tick(400);
    assert_eq!(s.cursor(), Some(0));
}
#[test]
fn runs_ahead_of_the_heard_words_while_talking() {
    let mut s = reading();
    s.speech("welcome", false);
    // "welcome" was heard, so the reader is past it: at least on "to".
    assert_eq!(s.tracker.position, Some(0));
    assert!(s.cursor() >= Some(1));
    // Time passes with no news from the microphone: the reader reads on.
    s.tick(300);
    assert!(s.cursor() >= Some(2));
    // But never more than three words past what was heard.
    s.tick(5000);
    assert!(s.cursor() <= Some(4));
}
#[test]
fn stops_when_the_reader_goes_quiet() {
    let mut s = reading();
    s.speech("welcome to", false);
    s.tick(100);
    let at = s.cursor();
    // No new speech for a while: hold still, do not creep on.
    s.tick(2000);
    let held = s.cursor();
    s.tick(5000);
    assert_eq!(s.cursor(), held);
    assert!(held >= at && held <= Some(5));
}
#[test]
fn never_slides_back_while_reading_on() {
    let mut s = reading();
    let mut last = s.cursor();
    for said in ["welcome", "welcome to", "welcome to our", "welcome to our bright", "welcome to our bright green"] {
        s.speech(said, false);
        for _ in 0..4 {
            s.tick(50);
            assert!(s.cursor() >= last);
            last = s.cursor();
        }
    }
}
#[test]
fn waits_at_a_new_sentence_for_the_readers_pause() {
    let mut s = reading();
    s.speech("welcome to our bright green", false);
    s.tick(200);
    // The reader is finishing the sentence; the cursor may reach the first
    // word of the next one, but waits there for the pause.
    assert!(s.cursor() <= Some(6));
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.cursor(), Some(6));
    // Mid-sentence it would already be a word or two on; here it allows
    // for the pause first.
    s.tick(100);
    assert_eq!(s.cursor(), Some(6));
    let mut mid = reading();
    mid.speech("welcome to our", false);
    mid.tick(100);
    assert!(mid.cursor() >= Some(4));
}
#[test]
fn follows_a_tap_or_retake_back() {
    let mut s = reading();
    s.speech("welcome to our bright green studio the camera", false);
    assert!(s.cursor() >= Some(8));
    s.seek(2);
    assert_eq!(s.cursor(), Some(2));
    s.tick(50);
    assert_eq!(s.cursor(), Some(2));
}
#[test]
fn highlight_lead_setting_changes_how_far_ahead() {
    let at = |lead: f64| {
        let mut s = reading();
        s.configure(&Settings { highlight_lead: lead, ..Settings::default() });
        s.speech("welcome", false);
        s.cursor().unwrap()
    };
    assert_eq!(at(0.0), 1);
    assert!(at(1.2) > at(0.0));
}
#[test]
fn tick_changes_view_when_the_cursor_would_move() {
    let mut s = reading();
    s.speech("welcome", false);
    let now = s.cursor();
    let mut moved = false;
    for _ in 0..20 {
        let will = s.tick_changes_view(50);
        s.tick(50);
        if s.cursor() != now {
            assert!(will, "the cursor moved but no redraw was asked for");
            moved = true;
            break;
        }
    }
    assert!(moved);
}
