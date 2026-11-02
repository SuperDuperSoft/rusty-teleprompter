use rusty_core::*;

const NOISE: [&str; 4] = [
    "purple elephants dance slowly",
    "quantum lobsters argue loudly",
    "seven yellow kites drift",
    "cheerful robots juggle pancakes",
];

fn script() -> Script {
    Script {
        id: "s".into(),
        title: "Feedback".into(),
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

fn session(settings: &Settings) -> Session {
    let mut s = Session::new(script());
    s.configure(settings);
    s
}

fn reading() -> Session {
    let mut s = session(&Settings::default());
    s.start();
    assert_eq!(s.phase, Phase::Reading);
    s
}

/// Speaks varying unrelated speech (finals and partials) every 250 ms.
fn ramble(s: &mut Session, ms: u64) {
    for i in 0..(ms / 250) as usize {
        s.speech(NOISE[i % 4], i % 2 == 1);
        s.tick(250);
    }
}

fn lost_session() -> Session {
    let mut s = reading();
    ramble(&mut s, 5000);
    assert!(s.lost());
    s
}

#[test]
fn not_hearing_before_any_speech() {
    let mut s = session(&Settings::default());
    assert!(!s.hearing());
    s.start();
    assert!(!s.hearing());
    assert_eq!(s.heard_ago_ms, None);
    s.tick(100);
    assert!(!s.hearing());
    assert_eq!(s.heard_ago_ms, None);
}

#[test]
fn hearing_after_new_speech_then_decays() {
    let mut s = reading();
    s.speech("purple elephants", false);
    assert_eq!(s.heard_ago_ms, Some(0));
    assert!(s.hearing());
    s.tick(699);
    assert!(s.hearing());
    s.tick(1);
    assert!(!s.hearing());
    assert_eq!(s.heard_ago_ms, Some(700));
}

#[test]
fn repeated_identical_partial_is_not_new_speech() {
    let mut s = reading();
    s.speech("purple elephants", false);
    s.tick(800);
    s.speech("purple elephants", false);
    assert_eq!(s.heard_ago_ms, Some(800));
    assert!(!s.hearing());
    s.speech("purple elephants", true);
    assert_eq!(s.heard_ago_ms, Some(0));
    assert!(s.hearing());
    s.speech("   ", true);
    s.tick(10);
    assert_eq!(s.heard_ago_ms, Some(10));
}

#[test]
fn hearing_in_countdown_but_not_paused_or_rolling() {
    let mut s = reading();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.tick(800);
    assert!(!s.hearing());
    s.speech("thank you", false);
    assert!(s.hearing());

    let mut s = reading();
    s.pause();
    s.speech("purple elephants", true);
    assert_eq!(s.heard_ago_ms, None);
    assert!(!s.hearing());

    let mut s = session(&Settings {
        pre_roll_secs: 3.0,
        ..Settings::default()
    });
    s.start();
    assert_eq!(s.phase, Phase::Rolling);
    s.speech("purple elephants", true);
    assert_eq!(s.heard_ago_ms, None);
    assert!(!s.hearing());
}

#[test]
fn stale_take_speech_does_not_light_dot() {
    let mut s = reading();
    s.speech_for_take("purple elephants", true, Some(s.take + 1));
    assert_eq!(s.heard_ago_ms, None);
    assert!(!s.hearing());
    s.speech_for_take("purple elephants", true, Some(s.take));
    assert!(s.hearing());
}

#[test]
fn lost_after_speaking_without_progress() {
    let mut s = reading();
    ramble(&mut s, 4750);
    assert_eq!(s.stuck_ms, 4750);
    assert!(!s.lost());
    s.speech("cheerful robots juggle", false);
    s.tick(250);
    assert_eq!(s.stuck_ms, 5000);
    assert!(s.lost());
    assert_eq!(s.tracker.position, None);
}

#[test]
fn silence_does_not_count_toward_lost() {
    let mut s = reading();
    for _ in 0..200 {
        s.tick(50);
    }
    assert_eq!((s.stuck_ms, s.heard_ago_ms), (0, None));
    assert!(!s.lost());
    // One utterance then silence: only the still-speaking part counts.
    s.speech("purple elephants", true);
    s.tick(10_000);
    assert_eq!(s.stuck_ms, SPEAKING_MS);
    assert!(!s.lost());
}

#[test]
fn batched_ticks_equal_single_ticks() {
    let mut a = reading();
    let mut b = reading();
    for s in [&mut a, &mut b] {
        s.speech("purple elephants", true);
        s.tick(400);
    }
    a.tick(3000);
    for _ in 0..60 {
        b.tick(50);
    }
    assert_eq!(
        (a.stuck_ms, a.heard_ago_ms, a.elapsed_ms),
        (b.stuck_ms, b.heard_ago_ms, b.elapsed_ms)
    );
    assert_eq!(
        (a.stuck_ms, a.heard_ago_ms, a.elapsed_ms),
        (SPEAKING_MS, Some(3400), 3400)
    );
}

#[test]
fn progress_clears_lost() {
    let mut s = lost_session();
    s.speech("welcome to our bright", true);
    assert_eq!(s.tracker.last_outcome, Outcome::Advanced);
    assert_eq!(s.stuck_ms, 0);
    assert!(!s.lost());
}

#[test]
fn back_resync_clears_lost() {
    let text = "The river flows quietly past the old stone mill. Farmers bring golden \
wheat from distant hills every autumn. Children gather apples beneath tall green orchard \
trees. Evening lanterns glow softly along the village road.";
    let mut s = Session::new(Script {
        id: "r".into(),
        title: "Resync".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "First".into(),
            text: text.into(),
        }],
    });
    s.configure(&Settings::default());
    s.start();
    for t in [
        "the river flows quietly past the old stone mill",
        "farmers bring golden wheat from distant hills every autumn",
        "children gather apples beneath tall green orchard trees",
    ] {
        s.speech(t, true);
    }
    assert_eq!(s.tracker.position, Some(25));
    ramble(&mut s, 5000);
    assert!(s.lost());
    s.speech("the river flows quietly past the old stone mill", true);
    assert_eq!(s.tracker.last_outcome, Outcome::Back);
    assert_eq!(s.stuck_ms, 0);
    assert!(!s.lost());
}

#[test]
fn dismiss_and_rewind_clear_lost() {
    let mut s = lost_session();
    s.dismiss_lost();
    assert_eq!(s.stuck_ms, 0);
    assert!(!s.lost());
    ramble(&mut s, 5000);
    assert!(s.lost());
    s.retake();
    assert_eq!((s.stuck_ms, s.heard_ago_ms), (0, None));
    assert!(!s.lost());
    // last_heard is cleared too: the same partial is new speech again.
    s.speech(NOISE[3], false);
    s.tick(50);
    s.seek(1);
    assert_eq!(s.heard_ago_ms, None);
    s.speech(NOISE[3], false);
    assert_eq!(s.heard_ago_ms, Some(0));

    for f in [Session::pause as fn(&mut Session), Session::next] {
        let mut s = lost_session();
        f(&mut s);
        assert_eq!((s.stuck_ms, s.heard_ago_ms), (0, None));
        assert!(!s.lost());
    }
}

#[test]
fn lost_disabled_when_hint_off() {
    let mut s = session(&Settings {
        show_lost_hint: false,
        ..Settings::default()
    });
    s.start();
    ramble(&mut s, 10_000);
    assert_eq!(s.lost_after_ms, 0);
    assert_eq!(s.stuck_ms, 0);
    assert!(!s.lost());
    assert!(s.hearing());
}

#[test]
fn upcoming_covers_next_words() {
    let mut s = reading();
    assert_eq!(s.upcoming(3), Some(0..3));
    s.seek(2);
    assert_eq!(s.upcoming(3), Some(2..5));
    assert_eq!(s.upcoming(10), Some(2..6));
    s.seek(5);
    assert_eq!(s.upcoming(3), Some(5..6));
    s.seek(6);
    assert_eq!(s.upcoming(3), None);
}

#[test]
fn upcoming_extends_to_unit_end() {
    // read0 john1 three2 twenty3 one4 and5 pay6 two7 dollars8 fifty9 cents10 today11
    let mut s = Session::new(Script {
        id: "u".into(),
        title: "Units".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "First".into(),
            text: "Read John 3:21 and pay $2.50 today".into(),
        }],
    });
    assert_eq!(s.tracker.words.len(), 12);
    assert_eq!(s.upcoming(4), Some(0..5));
    assert_eq!(s.upcoming(5), Some(0..5));
    s.seek(7);
    assert_eq!(s.upcoming(1), Some(7..11));
    assert_eq!(s.upcoming(5), Some(7..12));
}

#[test]
fn tick_changes_view_cases() {
    let mut s = session(&Settings::default());
    assert!(!s.tick_changes_view(50));
    s.pause();
    assert_eq!(s.phase, Phase::Paused);
    assert!(!s.tick_changes_view(50));

    let mut s = reading();
    assert!(!s.tick_changes_view(50));
    assert!(!s.tick_changes_view(0));
    // The dot decays past 700 ms.
    s.speech("purple elephants", true);
    assert!(!s.tick_changes_view(699));
    assert!(s.tick_changes_view(700));
    // lost() flips.
    s.stuck_ms = 4900;
    s.elapsed_ms = 100;
    assert!(!s.tick_changes_view(50));
    assert!(s.tick_changes_view(100));
    // elapsed_ms crosses a whole second.
    s.heard_ago_ms = None;
    s.stuck_ms = 0;
    s.elapsed_ms = 950;
    assert!(!s.tick_changes_view(49));
    assert!(s.tick_changes_view(50));
    // Holding behaves like Reading.
    let mut s = session(&Settings {
        hold_at_end: true,
        ..Settings::default()
    });
    s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Holding);
    s.elapsed_ms = 0;
    assert!(!s.tick_changes_view(50));
    assert!(s.tick_changes_view(1000));

    let mut s = reading();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    assert!(s.tick_changes_view(50));
    s.next();
    assert!(!s.tick_changes_view(0));
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
    assert!(!s.tick_changes_view(50));

    let mut s = session(&Settings {
        pre_roll_secs: 3.0,
        ..Settings::default()
    });
    s.start();
    assert_eq!(s.phase, Phase::Rolling);
    assert!(s.tick_changes_view(50));
}
