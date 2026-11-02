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
            Slide {
                id: "c".into(),
                title: "Third".into(),
                text: "And here is the last one".into(),
            },
        ],
    }
}

fn session(pre_roll_secs: f64, hold_at_end: bool) -> Session {
    let mut s = Session::new(script());
    s.configure(&Settings {
        pre_roll_secs,
        hold_at_end,
        ..Settings::default()
    });
    s
}

fn rolled() -> Session {
    let mut s = session(3.0, false);
    s.start();
    s.tick(3000);
    assert_eq!(s.phase, Phase::Reading);
    s
}

#[test]
fn preroll_off_starts_reading() {
    let mut s = Session::new(script());
    s.configure(&Settings::default());
    s.start();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!((s.roll_ms, s.sync_mark), (0, 0));
}
#[test]
fn start_from_ready_rolls_then_reads() {
    let mut s = session(3.0, false);
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
    s.tick(2999);
    assert_eq!((s.phase, s.roll_ms, s.sync_mark), (Phase::Rolling, 1, 0));
    s.tick(1);
    assert_eq!((s.phase, s.roll_ms, s.sync_mark), (Phase::Reading, 0, 1));
}
#[test]
fn speech_ignored_while_rolling() {
    let mut s = session(3.0, false);
    s.start();
    let take = s.take;
    s.speech_for_take("welcome to our bright", false, Some(take));
    assert_eq!(s.tracker.position, None);
    assert_eq!(s.tracker.last_outcome, Outcome::Idle);
    s.tick(3000);
    assert_eq!(s.take, take);
    s.speech_for_take("welcome to our bright", false, Some(take));
    assert_eq!(s.tracker.position, None);
    s.speech_for_take("welcome to our bright", false, Some(take));
    assert_eq!(s.tracker.position, Some(3));
}
#[test]
fn rolling_is_running_and_mic_open_not_listening() {
    assert!(Phase::Rolling.running() && Phase::Rolling.mic_open());
    assert!(!Phase::Rolling.listening());
    assert!(Phase::Reading.mic_open() && Phase::Countdown.mic_open() && Phase::Holding.mic_open());
    assert!(!Phase::Ready.mic_open() && !Phase::Paused.mic_open() && !Phase::Finished.mic_open());
    assert_eq!(
        serde_json::to_string(&Phase::Rolling).unwrap(),
        "\"rolling\""
    );
}
#[test]
fn pause_in_roll_restarts_full_roll() {
    let mut s = session(3.0, false);
    s.start();
    s.tick(2000);
    s.pause();
    assert_eq!(
        (s.phase, s.resume_phase(), s.roll_ms),
        (Phase::Paused, Phase::Rolling, 0)
    );
    s.tick(5000);
    assert_eq!(s.phase, Phase::Paused);
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
    assert_eq!(s.sync_mark, 0);
}
#[test]
fn resume_reading_rolls_again() {
    let mut s = rolled();
    s.speech("welcome to our", true);
    s.pause();
    assert_eq!(s.resume_phase(), Phase::Reading);
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
    assert_eq!(s.tracker.position, Some(2));
    s.tick(3000);
    assert_eq!((s.phase, s.sync_mark), (Phase::Reading, 2));
}
#[test]
fn resume_countdown_does_not_roll() {
    let mut s = rolled();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.pause();
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Countdown, 0));
    let mut h = session(3.0, true);
    h.start();
    h.tick(3000);
    h.speech("welcome to our bright green studio", true);
    h.pause();
    h.start();
    assert_eq!((h.phase, h.roll_ms), (Phase::Holding, 0));
}
#[test]
fn retake_and_back_sentence_roll() {
    let mut s = rolled();
    s.speech("welcome to our", true);
    s.retake();
    assert_eq!(
        (s.phase, s.roll_ms, s.tracker.position),
        (Phase::Rolling, 3000, None)
    );
    s.tick(3000);
    s.speech("welcome to our bright", true);
    s.back_sentence();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
    for from in [Phase::Countdown, Phase::Holding, Phase::Finished] {
        let mut s = session(3.0, from == Phase::Holding);
        s.start();
        s.tick(3000);
        if from == Phase::Finished {
            s.jump(2);
            s.speech("and here is the last one", true);
        } else {
            s.speech("welcome to our bright green studio", true);
        }
        assert_eq!(s.phase, from);
        s.retake();
        assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
    }
    let mut ready = session(3.0, false);
    ready.retake();
    assert_eq!(ready.phase, Phase::Ready);
    ready.start();
    ready.pause();
    ready.back_sentence();
    assert_eq!(ready.phase, Phase::Paused);
}
#[test]
fn take_bumped_once_by_retake_from_rolling() {
    let mut s = session(3.0, false);
    s.start();
    let take = s.take;
    s.tick(1000);
    s.retake();
    assert_eq!(
        (s.take, s.phase, s.roll_ms),
        (take + 1, Phase::Rolling, 3000)
    );
    s.tick(3000);
    assert_eq!((s.take, s.phase), (take + 1, Phase::Reading));
}
#[test]
fn stale_take_speech_after_roll_is_dropped() {
    let mut s = rolled();
    let old = s.take;
    s.retake();
    s.tick(3000);
    s.speech_for_take("welcome to our bright", true, Some(old));
    assert_eq!(s.tracker.position, None);
    s.speech_for_take("welcome to our bright", true, Some(s.take));
    assert_eq!(s.tracker.position, Some(3));
}
#[test]
fn seek_does_not_roll() {
    let mut s = rolled();
    s.seek(3);
    assert_eq!((s.phase, s.roll_ms), (Phase::Reading, 0));
    let mut r = session(3.0, false);
    r.start();
    r.tick(1000);
    r.seek(3);
    assert_eq!(
        (r.phase, r.roll_ms, r.tracker.position),
        (Phase::Rolling, 2000, Some(2))
    );
    let mut c = rolled();
    c.speech("welcome to our bright green studio", true);
    c.seek(2);
    assert_eq!((c.phase, c.roll_ms), (Phase::Reading, 0));
}
#[test]
fn slide_transitions_never_roll() {
    let mut s = rolled();
    s.speech("welcome to our bright green studio", true);
    s.tick(5000);
    assert_eq!((s.slide_index, s.phase, s.roll_ms), (1, Phase::Reading, 0));
    let mut h = session(3.0, true);
    h.start();
    h.tick(3000);
    h.speech("welcome to our bright green studio", true);
    h.start();
    assert_eq!((h.slide_index, h.phase, h.roll_ms), (1, Phase::Reading, 0));
    h.next();
    assert_eq!((h.slide_index, h.phase), (2, Phase::Reading));
    assert_eq!(h.sync_mark, 1);
}
#[test]
fn retake_on_wordless_slide_with_preroll_stays_put() {
    let mut s = Session::new(Script {
        id: "s".into(),
        title: "Empty".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: "[Only a direction]".into(),
        }],
    });
    s.configure(&Settings {
        pre_roll_secs: 3.0,
        ..Settings::default()
    });
    s.retake();
    s.back_sentence();
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Finished, 0));
}
#[test]
fn jump_during_roll_restarts_roll() {
    let mut s = session(3.0, false);
    s.start();
    s.tick(2000);
    let take = s.take;
    s.jump(2);
    assert_eq!(
        (s.slide_index, s.phase, s.roll_ms),
        (2, Phase::Rolling, 3000)
    );
    assert_eq!(s.take, take + 1);
    s.tick(3000);
    assert_eq!((s.phase, s.sync_mark), (Phase::Reading, 1));
    s.jump(0);
    assert_eq!((s.slide_index, s.phase, s.roll_ms), (0, Phase::Reading, 0));
}
#[test]
fn pause_revision_then_start_rolls() {
    let mut s = session(3.0, false);
    s.start();
    s.tick(500);
    s.pause();
    let mut u = script();
    u.revision = 2;
    u.slides[0].text = "Welcome to our bright new studio".into();
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused);
    s.start();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 3000));
}
#[test]
fn configure_during_roll_keeps_roll_ms() {
    let mut s = session(3.0, false);
    s.start();
    s.tick(1000);
    s.configure(&Settings {
        pre_roll_secs: 8.0,
        ..Settings::default()
    });
    assert_eq!(
        (s.phase, s.roll_ms, s.pre_roll_ms),
        (Phase::Rolling, 2000, 8000)
    );
    s.tick(2000);
    assert_eq!(s.phase, Phase::Reading);
}
#[test]
fn revision_refused_while_rolling() {
    let mut s = session(3.0, false);
    s.start();
    let mut u = script();
    u.revision = 2;
    assert!(!s.apply_revision(u));
    assert_eq!((s.phase, s.script.revision), (Phase::Rolling, 1));
}
#[test]
fn skip_roll_marks_sync() {
    let mut s = session(3.0, false);
    s.skip_roll();
    assert_eq!((s.phase, s.sync_mark), (Phase::Ready, 0));
    s.start();
    let take = s.take;
    s.tick(1000);
    s.skip_roll();
    assert_eq!(
        (s.phase, s.roll_ms, s.sync_mark, s.take),
        (Phase::Reading, 0, 1, take)
    );
    s.skip_roll();
    assert_eq!((s.phase, s.sync_mark), (Phase::Reading, 1));
}
#[test]
fn read_again_rolls_from_first_slide() {
    let mut s = rolled();
    s.tick(500);
    s.jump(2);
    s.speech("and here is the last one", true);
    assert_eq!((s.phase, s.elapsed_ms), (Phase::Finished, 500));
    let take = s.take;
    s.read_again();
    assert_eq!(
        (
            s.slide_index,
            s.phase,
            s.roll_ms,
            s.elapsed_ms,
            s.tracker.position
        ),
        (0, Phase::Rolling, 3000, 0, None)
    );
    assert!(s.take > take);
    let mut off = session(0.0, false);
    off.start();
    off.jump(2);
    off.speech("and here is the last one", true);
    off.read_again();
    assert_eq!((off.slide_index, off.phase), (0, Phase::Reading));
    let mut empty = Session::new(Script {
        id: "s".into(),
        title: "Empty".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: "[Only a direction]".into(),
        }],
    });
    empty.read_again();
    assert_eq!((empty.phase, empty.roll_ms), (Phase::Finished, 0));
}
#[test]
fn forward_by_phase() {
    let mut s = session(3.0, false);
    s.forward();
    assert_eq!(s.phase, Phase::Rolling);
    s.tick(600);
    s.forward();
    assert_eq!((s.phase, s.sync_mark), (Phase::Reading, 1));
    s.forward();
    assert_eq!((s.slide_index, s.phase), (1, Phase::Reading));
    s.pause();
    s.forward();
    assert_eq!((s.slide_index, s.phase), (1, Phase::Rolling));
    s.skip_roll();
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.forward();
    assert_eq!((s.slide_index, s.phase), (2, Phase::Reading));
    let mut h = session(3.0, true);
    h.start();
    h.tick(3000);
    h.speech("welcome to our bright green studio", true);
    assert_eq!(h.phase, Phase::Holding);
    h.forward();
    assert_eq!((h.slide_index, h.phase), (1, Phase::Reading));
    h.jump(2);
    h.speech("and here is the last one", true);
    assert_eq!(h.phase, Phase::Finished);
    h.forward();
    assert_eq!(
        (h.slide_index, h.phase, h.roll_ms),
        (0, Phase::Rolling, 3000)
    );
}
#[test]
fn forward_right_after_start_keeps_roll() {
    let mut s = session(3.0, false);
    s.forward();
    s.tick(100);
    s.forward();
    assert_eq!((s.phase, s.roll_ms, s.sync_mark), (Phase::Rolling, 2900, 0));
    s.tick(399);
    s.forward();
    assert_eq!((s.phase, s.roll_ms), (Phase::Rolling, 2501));
    s.tick(1);
    s.forward();
    assert_eq!((s.phase, s.sync_mark), (Phase::Reading, 1));
}
#[test]
fn forward_from_paused_holding_goes_next() {
    let mut s = session(3.0, true);
    s.start();
    s.tick(3000);
    s.speech("welcome to our bright green studio", true);
    s.pause();
    assert_eq!(s.resume_phase(), Phase::Holding);
    s.forward();
    assert_eq!(
        (s.slide_index, s.phase, s.roll_ms),
        (1, Phase::Rolling, 3000)
    );
    let mut off = session(0.0, true);
    off.start();
    off.speech("welcome to our bright green studio", true);
    off.pause();
    off.forward();
    assert_eq!((off.slide_index, off.phase), (1, Phase::Reading));
}
#[test]
fn elapsed_counts_reading_not_rolling() {
    let mut s = session(3.0, true);
    s.tick(1000);
    assert_eq!(s.elapsed_ms, 0);
    s.start();
    s.tick(3000);
    assert_eq!(s.elapsed_ms, 0);
    s.tick(1500);
    assert_eq!(s.elapsed_ms, 1500);
    s.pause();
    s.tick(1000);
    assert_eq!(s.elapsed_ms, 1500);
    s.start();
    s.tick(3000);
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Holding);
    s.tick(700);
    assert_eq!(s.elapsed_ms, 2200);
    let mut c = rolled();
    c.speech("welcome to our bright green studio", true);
    assert_eq!(c.phase, Phase::Countdown);
    c.tick(5000);
    assert_eq!((c.slide_index, c.elapsed_ms), (1, 5000));
    c.jump(2);
    c.speech("and here is the last one", true);
    c.tick(1000);
    assert_eq!((c.phase, c.elapsed_ms), (Phase::Finished, 5000));
}
#[test]
fn restart_resets_elapsed() {
    let mut s = rolled();
    s.tick(4000);
    s.next();
    s.tick(1000);
    assert_eq!(s.elapsed_ms, 5000);
    s.restart();
    assert_eq!((s.slide_index, s.elapsed_ms), (0, 0));
    s.tick(250);
    s.previous();
    s.jump(1);
    s.retake();
    assert_eq!(s.elapsed_ms, 250);
}
