use rusty_core::*;

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

#[test]
fn take_increments_on_moves_and_rewinds() {
    let mut s = Session::new(script());
    let t0 = s.take;
    s.start();
    s.next();
    assert_eq!(s.take, t0 + 1);
    s.seek(2);
    assert_eq!(s.take, t0 + 2);
    s.retake();
    assert_eq!(s.take, t0 + 3);
}
#[test]
fn retake_mid_read_restarts_slide_reading() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright", true);
    s.retake();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.tracker.position, None);
    assert_eq!(s.slide_index, 0);
}
#[test]
fn retake_while_paused_stays_paused_at_start() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright", true);
    s.pause();
    s.retake();
    assert_eq!(s.phase, Phase::Paused);
    assert_eq!(s.tracker.position, None);
}
#[test]
fn retake_in_countdown_cancels_it() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.retake();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.slide_index, 0);
}
#[test]
fn retake_after_finished_reads_last_slide() {
    let mut s = Session::new(script());
    s.start();
    s.next();
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
    s.retake();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.slide_index, 1);
    assert_eq!(s.tracker.position, None);
}
#[test]
fn stale_take_speech_is_ignored() {
    let mut s = Session::new(script());
    s.start();
    let old = s.take;
    s.retake();
    s.speech_for_take("welcome to our bright green", true, Some(old));
    assert_eq!(s.tracker.position, None);
    s.speech_for_take("welcome to our bright green", true, Some(s.take));
    assert!(s.tracker.position.is_some());
}
#[test]
fn untagged_speech_is_accepted() {
    let mut s = Session::new(script());
    s.start();
    s.speech_for_take("welcome to our bright green", true, None);
    assert!(s.tracker.position.is_some());
}

fn starts(text: &str) -> Vec<usize> {
    sentence_starts(text, &words(text))
}
#[test]
fn abbreviations_verses_and_blank_lines() {
    // dr0 smith1 spoke2 read3 john4 three5 sixteen6 today7 new8 part9 here10
    assert_eq!(
        starts("Dr. Smith spoke. Read John 3:16 today!\n\nnew part here"),
        vec![0, 3, 8]
    );
}
#[test]
fn cue_inside_gap_still_breaks() {
    assert_eq!(starts("Hello there. [pause] Next one"), vec![0, 2]);
}
#[test]
fn lowercase_after_period_is_not_a_start() {
    assert_eq!(starts("We use e.g. this one"), vec![0]);
}
#[test]
fn no_punctuation_is_one_sentence() {
    assert_eq!(starts("just words with no punctuation at all"), vec![0]);
}
fn sentences() -> Session {
    let mut s = Session::new(Script {
        id: "s".into(),
        title: "Sentences".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: "One two three. Four five six. Seven eight nine.".into(),
        }],
    });
    s.start();
    s
}
#[test]
fn back_goes_to_current_sentence_start() {
    let mut s = sentences();
    s.seek(8);
    s.back_sentence();
    assert_eq!(s.tracker.position, Some(5));
}
#[test]
fn back_within_first_two_words_goes_further() {
    let mut s = sentences();
    s.seek(7);
    s.back_sentence();
    assert_eq!(s.tracker.position, Some(2));
}
#[test]
fn back_at_first_sentence_stays_at_zero() {
    let mut s = sentences();
    s.seek(1);
    s.back_sentence();
    assert_eq!(s.tracker.position, None);
}
#[test]
fn back_sentence_bumps_take_and_reads() {
    let mut s = sentences();
    s.seek(8);
    let t = s.take;
    s.back_sentence();
    assert_eq!(s.take, t + 1);
    assert_eq!(s.phase, Phase::Reading);
}
fn unplayable() -> Session {
    Session::new(Script {
        id: "s".into(),
        title: "Empty".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: "[Only a direction]".into(),
        }],
    })
}
#[test]
fn retake_on_wordless_script_stays_finished() {
    let mut s = unplayable();
    assert_eq!(s.phase, Phase::Finished);
    s.retake();
    assert_eq!(s.phase, Phase::Finished);
    s.back_sentence();
    assert_eq!(s.phase, Phase::Finished);
}
#[test]
fn back_sentence_without_punctuation_goes_to_slide_start() {
    let mut s = Session::new(Script {
        id: "s".into(),
        title: "Plain".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: "just words with no punctuation at all".into(),
        }],
    });
    s.start();
    s.seek(4);
    s.back_sentence();
    assert_eq!(s.tracker.position, None);
    assert_eq!(s.phase, Phase::Reading);
}
