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

fn held() -> Session {
    let mut s = Session::new(script());
    s.configure(&Settings {
        hold_at_end: true,
        ..Settings::default()
    });
    s.start();
    s
}
#[test]
fn end_of_slide_holds() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Holding);
    s.tick(60_000);
    assert_eq!(s.phase, Phase::Holding);
    assert_eq!(s.slide_index, 0);
}
#[test]
fn final_slide_still_finishes_when_holding() {
    let mut s = held();
    s.next();
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
}
#[test]
fn start_from_holding_goes_next() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.start();
    assert_eq!((s.slide_index, s.phase), (1, Phase::Reading));
}
#[test]
fn pause_and_resume_return_to_holding() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.pause();
    assert_eq!(s.phase, Phase::Paused);
    s.start();
    assert_eq!(s.phase, Phase::Holding);
}
#[test]
fn retake_from_holding_reads_slide_start() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.retake();
    assert_eq!((s.slide_index, s.phase), (0, Phase::Reading));
}
#[test]
fn revision_applies_while_holding() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    let mut u = script();
    u.revision = 2;
    u.slides[1].text = "Changed next slide".into();
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Holding);
}
#[test]
fn revision_removing_held_slide_pauses() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    let mut u = script();
    u.revision = 2;
    u.slides.remove(0);
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused);
    assert_eq!(s.tracker.position, None);
}
#[test]
fn countdown_length_is_configurable() {
    let mut s = Session::new(script());
    s.configure(&Settings {
        countdown_secs: 1.0,
        ..Settings::default()
    });
    s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.remaining_ms, 1000);
    s.tick(1000);
    assert_eq!(s.slide_index, 1);
}
#[test]
fn configure_mid_countdown_keeps_remaining() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.tick(1000);
    s.configure(&Settings {
        countdown_secs: 15.0,
        ..Settings::default()
    });
    assert_eq!(s.remaining_ms, 4000);
    assert_eq!(s.countdown_ms, 15_000);
}
#[test]
fn settings_clamp_and_old_json() {
    let mut st: Settings = serde_json::from_str(r#"{"font_size":40}"#).unwrap();
    assert!(!st.hold_at_end);
    assert_eq!(st.countdown_secs, 5.0);
    st.countdown_secs = 99.0;
    st.clamp();
    assert_eq!(st.countdown_secs, 15.0);
    st.countdown_secs = 0.0;
    st.clamp();
    assert_eq!(st.countdown_secs, 1.0);
}
#[test]
fn new_settings_defaults_and_old_json() {
    let st: Settings =
        serde_json::from_str(r#"{"font_size":40,"hold_at_end":true,"countdown_secs":3.0}"#)
            .unwrap();
    assert_eq!(
        (st.pre_roll_secs, st.speaking_wpm, st.lost_place_secs),
        (0.0, 150.0, 5.0)
    );
    assert!(st.sync_flash && st.voice_resync);
    assert!(!st.lost_hint_popup, "lost-place popup is off by default");
    assert!(!st.volume_keys && !st.script_grammar);
    assert_eq!(
        st,
        Settings {
            font_size: 40.0,
            hold_at_end: true,
            countdown_secs: 3.0,
            ..Settings::default()
        }
    );
}
#[test]
fn new_settings_clamp() {
    let mut st = Settings {
        pre_roll_secs: -1.0,
        speaking_wpm: 10.0,
        lost_place_secs: 0.5,
        ..Settings::default()
    };
    st.clamp();
    assert_eq!(
        (st.pre_roll_secs, st.speaking_wpm, st.lost_place_secs),
        (0.0, 80.0, 2.0)
    );
    st = Settings {
        pre_roll_secs: 99.0,
        speaking_wpm: 999.0,
        lost_place_secs: 99.0,
        ..Settings::default()
    };
    st.clamp();
    assert_eq!(
        (st.pre_roll_secs, st.speaking_wpm, st.lost_place_secs),
        (10.0, 240.0, 15.0)
    );
}
#[test]
fn configure_copies_new_settings() {
    let mut s = Session::new(script());
    assert_eq!(
        (s.pre_roll_ms, s.roll_ms, s.sync_mark, s.wpm),
        (0, 0, 0, 150.0)
    );
    assert_eq!(
        (s.elapsed_ms, s.heard_ago_ms, s.stuck_ms, s.lost_after_ms),
        (0, None, 0, 0)
    );
    assert!(s.voice_resync);
    s.configure(&Settings {
        pre_roll_secs: 3.4567,
        speaking_wpm: 170.0,
        lost_place_secs: 7.25,
        lost_hint_popup: true,
        voice_resync: false,
        ..Settings::default()
    });
    assert_eq!((s.pre_roll_ms, s.wpm, s.lost_after_ms), (3457, 170.0, 7250));
    assert!(!s.voice_resync);
    s.configure(&Settings {
        lost_hint_popup: false,
        ..Settings::default()
    });
    assert_eq!((s.pre_roll_ms, s.wpm, s.lost_after_ms), (0, 150.0, 0));
    assert!(s.voice_resync);
}
#[test]
fn listening_phases() {
    assert!(
        Phase::Reading.listening() && Phase::Countdown.listening() && Phase::Holding.listening()
    );
    assert!(
        !Phase::Paused.listening() && !Phase::Ready.listening() && !Phase::Finished.listening()
    );
    assert!(!Phase::Rolling.listening());
    assert!(Phase::Rolling.running());
    assert!(Phase::Rolling.mic_open());
}
#[test]
fn revision_changing_held_slide_ending_pauses() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    let mut u = script();
    u.revision = 2;
    u.slides[0].text = "Welcome to our bright green studio and more".into();
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused);
    s.start();
    assert_eq!((s.slide_index, s.phase), (0, Phase::Reading));
}
#[test]
fn revision_removing_later_slides_while_holding_pauses() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    let mut u = script();
    u.revision = 2;
    u.slides.remove(1);
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused);
}
#[test]
fn paused_countdown_reports_resume_phase() {
    let mut s = Session::new(script());
    s.start();
    s.speech("welcome to our bright green studio", true);
    s.pause();
    assert_eq!(s.resume_phase(), Phase::Countdown);
    s.start();
    s.retake();
    s.pause();
    assert_eq!(s.resume_phase(), Phase::Reading);
}
#[test]
fn revision_while_paused_holding_keeps_hold() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.pause();
    let mut u = script();
    u.revision = 2;
    u.slides[1].text = "Changed next slide".into();
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused);
    s.start();
    assert_eq!(s.phase, Phase::Holding);
}
#[test]
fn revision_while_paused_holding_changing_ending_reads() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.pause();
    let mut u = script();
    u.revision = 2;
    u.slides[0].text = "Welcome to our bright green studio and more".into();
    assert!(s.apply_revision(u));
    s.start();
    assert_eq!((s.slide_index, s.phase), (0, Phase::Reading));
}
#[test]
fn revision_while_paused_holding_removing_later_slides_reads() {
    let mut s = held();
    s.speech("welcome to our bright green studio", true);
    s.pause();
    let mut u = script();
    u.revision = 2;
    u.slides.remove(1);
    assert!(s.apply_revision(u));
    s.start();
    assert_eq!(s.phase, Phase::Reading);
}

#[test]
fn old_saved_lost_hint_setting_resets_to_off() {
    let st: Settings = serde_json::from_str(r#"{"show_lost_hint":true}"#).unwrap();
    assert!(!st.lost_hint_popup);
}
