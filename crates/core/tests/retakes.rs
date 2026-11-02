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
