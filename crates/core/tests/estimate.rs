use rusty_core::*;

fn slide(id: &str, text: &str) -> Slide {
    Slide {
        id: id.into(),
        title: id.into(),
        text: text.into(),
    }
}
fn script(texts: &[&str]) -> Script {
    Script {
        id: "s".into(),
        title: "Studio".into(),
        revision: 1,
        slides: texts
            .iter()
            .enumerate()
            .map(|(i, t)| slide(&i.to_string(), t))
            .collect(),
    }
}
/// 5 words, an empty slide, then 3 words; 60 wpm makes one word one second.
fn session(pre_roll_secs: f64, hold_at_end: bool) -> Session {
    let mut s = Session::new(script(&[
        "alpha bravo charlie delta echo",
        "[pause]",
        "foxtrot golf hotel",
    ]));
    s.configure(&Settings {
        pre_roll_secs,
        hold_at_end,
        speaking_wpm: 60.0,
        ..Settings::default()
    });
    s
}
fn read_first(s: &mut Session) {
    s.start();
    s.speech("alpha bravo charlie delta echo", true);
}

#[test]
fn estimate_at_150_wpm() {
    assert_eq!(estimate_ms(150, 150.0), 60000);
    assert_eq!(estimate_ms(0, 150.0), 0);
    assert_eq!(estimate_ms(1, 0.0), 60000);
    let text = vec!["word"; 150].join(" ");
    assert_eq!(slide_estimate_ms(&slide("a", &text), 150.0), 60000);
}
#[test]
fn cues_do_not_count() {
    let s = slide("a", "[Look at the lens] hello there [smile]");
    assert_eq!(slide_estimate_ms(&s, 60.0), 2000);
}
#[test]
fn numbers_count_as_spoken() {
    // "read twenty one lines"
    assert_eq!(slide_estimate_ms(&slide("a", "Read 21 lines"), 60.0), 4000);
}
#[test]
fn script_total_adds_gaps() {
    let sc = script(&["one two", "[cue only]", "three four", "five six"]);
    let settings = Settings {
        speaking_wpm: 60.0,
        countdown_secs: 5.0,
        ..Settings::default()
    };
    assert_eq!(script_estimate_ms(&sc, &settings), 6000 + 2 * 5000);
    assert_eq!(script_estimate_ms(&script(&["", "[x]"]), &settings), 0);
}
#[test]
fn hold_mode_has_no_gaps() {
    let sc = script(&["one two", "[cue only]", "three four", "five six"]);
    let settings = Settings {
        speaking_wpm: 60.0,
        hold_at_end: true,
        ..Settings::default()
    };
    assert_eq!(script_estimate_ms(&sc, &settings), 6000);
}
#[test]
fn format_duration_cases() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(999), "0:01");
    assert_eq!(format_duration(1000), "0:01");
    assert_eq!(format_duration(61000), "1:01");
    assert_eq!(format_duration(3723000), "1:02:03");
    assert_eq!(format_duration(725000), "12:05");
}
#[test]
fn time_left_shrinks_as_reading_progresses() {
    let mut s = session(0.0, false);
    assert_eq!(s.time_left_ms(), 8000 + 5000);
    s.start();
    assert_eq!(s.time_left_ms(), 13000);
    s.speech("alpha bravo charlie", true);
    assert_eq!(s.tracker.position, Some(2));
    assert_eq!(s.time_left_ms(), 2000 + 3000 + 5000);
}
#[test]
fn time_left_in_countdown_uses_remaining() {
    let mut s = session(0.0, false);
    read_first(&mut s);
    assert_eq!(s.phase, Phase::Countdown);
    assert_eq!(s.time_left_ms(), 5000 + 3000);
    s.tick(2000);
    assert_eq!(s.time_left_ms(), 3000 + 3000);
}
#[test]
fn time_left_paused_from_countdown_uses_remaining() {
    let mut s = session(0.0, false);
    read_first(&mut s);
    s.tick(2000);
    s.pause();
    assert_eq!(
        (s.phase, s.resume_phase()),
        (Phase::Paused, Phase::Countdown)
    );
    assert_eq!(s.time_left_ms(), 3000 + 3000);
}
#[test]
fn time_left_in_rolling_adds_roll() {
    let mut s = session(3.0, false);
    s.start();
    assert_eq!(s.phase, Phase::Rolling);
    assert_eq!(s.time_left_ms(), 3000 + 13000);
    s.tick(1000);
    assert_eq!(s.time_left_ms(), 2000 + 13000);
}
#[test]
fn time_left_in_holding() {
    let mut s = session(0.0, true);
    read_first(&mut s);
    assert_eq!(s.phase, Phase::Holding);
    assert_eq!(s.time_left_ms(), 3000);
    // The first gap stays 0 even if countdowns come back on while holding.
    s.configure(&Settings {
        speaking_wpm: 60.0,
        ..Settings::default()
    });
    assert_eq!(s.time_left_ms(), 3000);
}
#[test]
fn time_left_finished_is_zero() {
    let mut s = session(0.0, false);
    read_first(&mut s);
    s.next();
    s.speech("foxtrot golf hotel", true);
    assert_eq!(s.phase, Phase::Finished);
    assert_eq!(s.time_left_ms(), 0);
}
