use rusty_core::*;

#[test]
fn real_offline_recognition_stream() {
    let mut tracker = Tracker::new("Welcome to our studio. Today we are making a very useful local teleprompter. I can pause and take a breath. [Look at camera] Now the bright green camera light means we are recording. That is the end of this slide.");
    let mut previous = None;
    for line in include_str!("fixtures/offline-voice.tsv").lines() {
        let (kind, text) = line.split_once('\t').unwrap();
        tracker.accept(text, kind == "final");
        assert!(tracker.position >= previous);
        previous = tracker.position;
    }
    assert_eq!(tracker.position, Some(tracker.words.len() - 1));
}
#[test]
fn replay_never_resyncs_back() {
    let mut tracker = Tracker::new("Welcome to our studio. Today we are making a very useful local teleprompter. I can pause and take a breath. [Look at camera] Now the bright green camera light means we are recording. That is the end of this slide.");
    for line in include_str!("fixtures/offline-voice.tsv").lines() {
        let (kind, text) = line.split_once('\t').unwrap();
        tracker.accept(text, kind == "final");
        assert_ne!(tracker.last_outcome, Outcome::Back, "{line}");
    }
}
fn script() -> Script {
    Script {
        id: "s".into(),
        title: "Studio".into(),
        revision: 1,
        slides: vec![
            Slide {
                id: "a".into(),
                title: "First".into(),
                text: "Welcome to our bright green studio".into(),
            },
            Slide {
                id: "b".into(),
                title: "Second".into(),
                text: "This is the next slide".into(),
            },
        ],
    }
}
fn track(text: &str, spoken: &str) -> Tracker {
    let mut t = Tracker::new(text);
    t.accept(spoken, true);
    t
}

#[test]
fn complete_read() {
    let t = track(
        "Today we are making a local teleprompter",
        "today we are making a local teleprompter",
    );
    assert_eq!(t.position, Some(t.words.len() - 1));
}
#[test]
fn omitted_words_and_fillers() {
    let t = track(
        "Today we are making a very useful local teleprompter for everyone",
        "today um we making useful local teleprompter for everyone",
    );
    assert_eq!(t.position, Some(t.words.len() - 1));
}
#[test]
fn skipped_sentence() {
    let t=track("Welcome to our studio. This sentence contains extra details that we might decide to leave out today. Now the bright green camera light means we are recording.", "welcome to our studio now the bright green camera light means we are recording");
    assert_eq!(t.position, Some(t.words.len() - 1));
}
#[test]
fn silence_waits() {
    let t = track("welcome to the studio", "");
    assert_eq!(t.position, None);
}
#[test]
fn unrelated_speech_waits() {
    let t = track(
        "welcome to our bright green studio",
        "please bring me a cup of coffee",
    );
    assert_eq!(t.position, None);
}
#[test]
fn repeated_phrase_nearest() {
    let t = track(
        "the green light is on and now we pause because the green light is on at the end",
        "the green light is on",
    );
    assert_eq!(t.position, Some(4));
}
#[test]
fn lone_ending_word_cannot_finish() {
    let t = track(
        "welcome to this rather wonderful presentation",
        "presentation",
    );
    assert_ne!(t.position, Some(t.words.len() - 1));
}
#[test]
fn short_slide_one() {
    assert_eq!(track("hello", "hello").position, Some(0));
}
#[test]
fn short_slide_two() {
    assert_eq!(track("thank you", "thank you").position, Some(1));
}
#[test]
fn numbers_are_spoken_words() {
    assert_eq!(
        track("This is slide 1", "this is slide one").position,
        Some(3)
    );
}
#[test]
fn last_word_needs_a_partial_that_holds() {
    // Words move the position at once, but the slide's last word ends the
    // slide, so a partial has to hold still (or a final arrive) first.
    let mut t = Tracker::new("welcome to our studio");
    t.accept("welcome to our studio", false);
    assert_eq!(t.position, Some(2));
    t.accept("welcome to our studio", false);
    assert_eq!(t.position, Some(3));
}
#[test]
fn revised_partial_never_reverses() {
    let mut t = track(
        "welcome to our bright green studio",
        "welcome to our bright",
    );
    let old = t.position;
    t.accept("welcome to", false);
    assert_eq!(old, t.position);
}
#[test]
fn separate_finals_keep_context() {
    let mut t = track(
        "welcome to our bright green studio",
        "welcome to our bright green",
    );
    t.accept("studio", true);
    assert_eq!(t.position, Some(5));
}
#[test]
fn seek_clears_context() {
    let mut t = track(
        "welcome to our bright green studio",
        "welcome to our bright green",
    );
    t.seek(0);
    t.accept("studio", true);
    assert_eq!(t.position, None);
}
#[test]
fn directions_preserve_utf8_offsets() {
    let text = "Hello [look at camera] café’s stage.";
    let w = words(text);
    assert_eq!(
        w.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        vec!["hello", "café's", "stage"]
    );
    assert_eq!(&text[w[1].start..w[1].end], "café’s");
}
#[test]
fn minor_recognition_error() {
    let t = track(
        "welcome to the recording studio",
        "welcome to the recordin studio",
    );
    assert_eq!(t.position, Some(4));
}
#[test]
fn countdown_exactly_five_seconds() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.tick(4999);
    assert_eq!(s.slide_index, 0);
    s.tick(1);
    assert_eq!(s.slide_index, 1);
    assert_eq!(s.phase, Phase::Reading);
}
#[test]
fn countdown_pause_freezes() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.tick(1600);
    s.pause();
    s.tick(99999);
    assert_eq!(s.remaining_ms, 3400);
    s.start();
    assert_eq!(s.phase, Phase::Countdown);
    s.tick(3400);
    assert_eq!(s.slide_index, 1);
}
#[test]
fn repeated_result_does_not_reset_countdown() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.tick(2000);
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.remaining_ms, 3000);
}
#[test]
fn pause_near_end_does_not_advance() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our", true);
    s.tick(99999);
    assert_eq!(s.phase, Phase::Reading);
}
#[test]
fn empty_slides_skipped() {
    let mut sc = script();
    sc.slides.insert(
        1,
        Slide {
            id: "empty".into(),
            title: "".into(),
            text: "[Pause]".into(),
        },
    );
    let mut s = Session::new(sc);
    s.start();
    s.next();
    assert_eq!(s.slide_index, 2);
}
#[test]
fn empty_script_finishes() {
    let mut sc = script();
    sc.slides.clear();
    assert_eq!(Session::new(sc).phase, Phase::Finished);
}
#[test]
fn final_slide_finishes() {
    let mut s = Session::new(script());
    s.start();
    s.next();
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
}
#[test]
fn manual_next_cancels_countdown() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.next();
    s.tick(9000);
    assert_eq!(s.slide_index, 1);
    assert_eq!(s.phase, Phase::Reading);
}
#[test]
fn previous_cancels_countdown() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.previous();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.tracker.position, None);
}
#[test]
fn seek_cancels_countdown() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.seek(1);
    s.tick(99999);
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.tracker.position, Some(0));
}
#[test]
fn manual_next_final_retains_navigation() {
    let mut s = Session::new(script());
    s.start();
    s.next();
    s.next();
    assert_eq!(s.phase, Phase::Finished);
    assert_eq!(s.slide_index, 1);
    s.previous();
    assert_eq!(s.slide_index, 0);
}
#[test]
fn no_edits_while_reading() {
    let mut s = Session::new(script());
    s.start();
    let mut change = script();
    change.revision = 2;
    assert!(!s.apply_revision(change));
    assert_eq!(s.script.revision, 1);
}
#[test]
fn paused_revision_preserves_unique_anchor() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green", true);
    s.pause();
    let mut change = script();
    change.revision = 2;
    change.slides[0].text = "Good morning. Welcome to our bright green studio".into();
    assert!(s.apply_revision(change));
    assert_eq!(s.tracker.position, Some(6));
}
#[test]
fn removed_anchor_resets_position() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our", true);
    s.pause();
    let mut change = script();
    change.slides[0].text = "Entirely different words".into();
    s.apply_revision(change);
    assert_eq!(s.tracker.position, None);
}
#[test]
fn removed_slide_falls_back() {
    let mut s = Session::new(script());
    let mut change = script();
    change.slides.remove(0);
    s.apply_revision(change);
    assert_eq!(s.slide().unwrap().id, "b");
}
#[test]
fn finished_stays_finished_after_edit() {
    let mut s = Session::new(script());
    s.next();
    s.next();
    let mut change = script();
    change.revision = 2;
    s.apply_revision(change);
    assert_eq!(s.phase, Phase::Finished);
}
#[test]
fn restore_starts_paused_at_saved_word() {
    let mut s = Session::new(script());
    s.start();
    s.next();
    s.seek(3);
    let saved = s.position();
    let mut recovered = Session::new(script());
    recovered.restore(&saved);
    assert_eq!(recovered.phase, Phase::Ready);
    assert_eq!(recovered.slide_index, 1);
    assert_eq!(recovered.tracker.position, Some(2));
}
#[test]
fn completed_session_reopens_at_start() {
    let mut s = Session::new(script());
    s.next();
    s.next();
    let mut recovered = Session::new(script());
    recovered.restore(&s.position());
    assert_eq!(recovered.slide_index, 0);
}
#[test]
fn settings_bounds() {
    let mut s = Settings {
        font_size: 500.0,
        margins: 0.0,
        ..Settings::default()
    };
    s.clamp();
    assert_eq!(s.font_size, 90.0);
    assert_eq!(s.margins, 2.0);
}
