use rusty_core::RemoteAction::*;
use rusty_core::*;

/// A full short press: down at `t`, up 50 ms later.
fn press(r: &mut Remote, key: &str, t: f64) -> Vec<RemoteAction> {
    [
        r.key(key, true, false, t),
        r.key(key, false, false, t + 50.0),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[test]
fn every_bound_key_short_press() {
    let cases: [(&str, RemoteAction); 15] = [
        ("PageDown", Forward),
        ("ArrowRight", Forward),
        ("ArrowDown", Forward),
        ("MediaTrackNext", Forward),
        ("PageUp", BackSentence),
        ("ArrowLeft", BackSentence),
        ("ArrowUp", BackSentence),
        ("MediaTrackPrevious", BackSentence),
        (" ", Toggle),
        ("Enter", Toggle),
        ("MediaPlayPause", Toggle),
        ("MediaPlay", Play),
        ("MediaPause", Pause),
        ("b", Pause),
        (".", Pause),
    ];
    for (key, action) in cases {
        let mut r = Remote::new(false);
        assert_eq!(press(&mut r, key, 0.0), vec![action], "{key:?}");
    }
    let mut r = Remote::new(true);
    assert_eq!(press(&mut r, "AudioVolumeUp", 0.0), vec![Forward]);
    assert_eq!(press(&mut r, "AudioVolumeDown", 1000.0), vec![BackSentence]);
    assert_eq!(press(&mut Remote::new(false), "x", 0.0), vec![]);
    assert_eq!(
        remote_binding("PageDown", false),
        Some((Forward, Some(Toggle)))
    );
    assert_eq!(
        remote_binding("PageUp", false),
        Some((BackSentence, Some(Retake)))
    );
    assert_eq!(remote_binding("Enter", false), Some((Toggle, None)));
    assert_eq!(remote_binding("Tab", false), None);
}

#[test]
fn keys_without_hold_fire_on_down() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("Enter", true, false, 0.0), Some(Toggle));
    assert_eq!(r.key("Enter", false, false, 50.0), None);
    assert_eq!(r.key("MediaPlay", true, false, 1000.0), Some(Play));
    assert_eq!(r.tick(3000.0), None);
    assert_eq!(r.key("MediaPlay", false, false, 3000.0), None);
}

#[test]
fn hold_fires_once_on_repeat() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageDown", true, false, 0.0), None);
    let mut fired = vec![];
    let mut t = 500.0;
    while t <= 2000.0 {
        fired.extend(r.key("PageDown", true, true, t));
        t += 30.0;
    }
    assert_eq!(fired, vec![Toggle]);
    assert_eq!(r.key("PageDown", false, false, 2050.0), None);
}

#[test]
fn hold_fires_from_tick_without_repeats() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageUp", true, false, 0.0), None);
    assert_eq!(r.tick(899.0), None);
    assert_eq!(r.tick(900.0), Some(Retake));
    assert_eq!(r.tick(1500.0), None);
    assert_eq!(r.key("PageUp", false, false, 1600.0), None);
}

#[test]
fn hold_suppresses_short() {
    let mut r = Remote::new(false);
    r.key("ArrowRight", true, false, 0.0);
    assert_eq!(r.key("ArrowRight", true, true, 950.0), Some(Toggle));
    assert_eq!(r.key("ArrowRight", false, false, 1000.0), None);
    // A short press below the hold time fires the short action on key-up.
    assert_eq!(r.key("ArrowRight", true, false, 2000.0), None);
    assert_eq!(r.key("ArrowRight", false, false, 2899.0), Some(Forward));
}

#[test]
fn auto_repeat_ignored() {
    let mut r = Remote::new(false);
    assert_eq!(r.key(" ", true, false, 0.0), Some(Toggle));
    for i in 1..60 {
        assert_eq!(r.key(" ", true, true, i as f64 * 33.0), None);
        assert_eq!(r.tick(i as f64 * 33.0), None);
    }
    assert_eq!(r.key(" ", false, false, 2000.0), None);
}

#[test]
fn up_without_down_ignored() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageDown", false, false, 0.0), None);
    assert_eq!(r.key("Enter", false, false, 10.0), None);
    assert_eq!(r.tick(5000.0), None);
}

#[test]
fn first_event_repeat_is_ignored() {
    let mut r = Remote::new(false);
    for i in 0..50 {
        assert_eq!(r.key("PageDown", true, true, i as f64 * 33.0), None);
        assert_eq!(r.tick(i as f64 * 33.0), None);
    }
    assert_eq!(r.key("PageDown", false, false, 1700.0), None);
    assert_eq!(press(&mut r, "PageDown", 2000.0), vec![Forward]);
    assert_eq!(r.key("Enter", true, true, 3000.0), None);
}

#[test]
fn other_key_cancels_hold() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageDown", true, false, 0.0), None);
    assert_eq!(r.key("PageUp", true, false, 100.0), None);
    assert_eq!(r.key("PageDown", false, false, 150.0), None);
    assert_eq!(r.key("PageUp", false, false, 200.0), Some(BackSentence));
    assert_eq!(r.tick(2000.0), None);
}

#[test]
fn no_hold_key_while_other_key_held() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageDown", true, false, 0.0), None);
    assert_eq!(r.key("b", true, false, 100.0), Some(Pause));
    assert_eq!(r.tick(1000.0), None);
    assert_eq!(r.key("PageDown", false, false, 1100.0), None);
    assert_eq!(r.key("b", false, false, 1200.0), None);
}

#[test]
fn stuck_key_cleared() {
    let mut r = Remote::new(false);
    r.key("PageDown", true, false, 0.0);
    assert_eq!(r.tick(900.0), Some(Toggle));
    assert_eq!(r.tick(9_999.0), None);
    assert_eq!(r.tick(10_000.0), None);
    // Discarded: the late key-up fires nothing, and the key works again after it.
    assert_eq!(r.key("PageDown", false, false, 12_000.0), None);
    assert_eq!(press(&mut r, "PageDown", 13_000.0), vec![Forward]);
}

#[test]
fn repeat_after_stuck_discard_does_not_refire() {
    let mut r = Remote::new(false);
    r.key("ArrowLeft", true, false, 0.0);
    assert_eq!(r.tick(1000.0), Some(Retake));
    assert_eq!(r.tick(10_500.0), None);
    let mut fired = vec![];
    let mut t = 10_500.0;
    while t < 13_000.0 {
        fired.extend(r.key("ArrowLeft", true, true, t));
        fired.extend(r.tick(t));
        t += 33.0;
    }
    assert_eq!(fired, vec![]);
    assert_eq!(r.key("ArrowLeft", false, false, 13_000.0), None);
    // Other keys keep working while one is ignored.
    let mut r = Remote::new(false);
    r.key("ArrowLeft", true, false, 0.0);
    r.tick(11_000.0);
    assert_eq!(r.key("Enter", true, false, 11_100.0), Some(Toggle));
}

#[test]
fn fresh_press_after_lost_key_up_is_not_swallowed() {
    // Hold fires, then the key-up is lost (e.g. a system dialog took focus).
    let mut r = Remote::new(false);
    r.key("PageDown", true, false, 0.0);
    assert_eq!(r.tick(900.0), Some(Toggle));
    assert_eq!(r.tick(10_000.0), None);
    // The user's next real press works as a normal short press.
    assert_eq!(press(&mut r, "PageDown", 20_000.0), vec![Forward]);
    // And a repeat down after a stuck discard still cannot re-arm the hold.
    let mut r = Remote::new(false);
    r.key("PageDown", true, false, 0.0);
    r.tick(900.0);
    r.tick(10_000.0);
    assert_eq!(r.key("PageDown", true, true, 10_100.0), None);
    assert_eq!(r.tick(12_000.0), None);
}

#[test]
fn reset_clears_held() {
    let mut r = Remote::new(false);
    r.key("PageDown", true, false, 0.0);
    r.reset();
    assert_eq!(r.tick(1000.0), None);
    assert_eq!(r.key("PageDown", false, false, 1100.0), None);
    // reset also clears a stuck key's `ignored`.
    r.key("PageUp", true, false, 2000.0);
    r.tick(12_500.0);
    r.reset();
    assert_eq!(r.key("PageUp", true, false, 13_000.0), None);
    assert_eq!(r.key("PageUp", false, false, 13_100.0), Some(BackSentence));
}

#[test]
fn double_forward_within_debounce_fires_once() {
    let mut r = Remote::new(false);
    assert_eq!(press(&mut r, "PageDown", 0.0), vec![Forward]);
    assert_eq!(press(&mut r, "PageDown", 150.0), vec![]);
    // Different actions are not debounced against each other.
    assert_eq!(press(&mut r, "PageUp", 250.0), vec![BackSentence]);
}

#[test]
fn clicker_down_up_pairs_while_held_do_not_skip_slides() {
    let mut r = Remote::new(false);
    let mut fired = vec![];
    let mut t = 0.0;
    while t < 2000.0 {
        fired.extend(r.key("PageDown", true, false, t));
        fired.extend(r.key("PageDown", false, false, t + 20.0));
        fired.extend(r.tick(t + 20.0));
        t += 40.0;
    }
    assert_eq!(fired, vec![Forward]);
}

#[test]
fn forward_after_guard_fires_again() {
    let mut r = Remote::new(false);
    assert_eq!(press(&mut r, "PageDown", 0.0), vec![Forward]);
    assert_eq!(press(&mut r, "PageDown", 500.0), vec![Forward]);
}

#[test]
fn media_play_and_pause_are_distinct() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("MediaPlay", true, false, 0.0), Some(Play));
    assert_eq!(r.key("MediaPause", true, false, 100.0), Some(Pause));
    assert_eq!(r.key("MediaPlayPause", true, false, 200.0), Some(Toggle));
    assert_eq!(
        serde_json::to_string(&BackSentence).unwrap(),
        "\"back_sentence\""
    );
}

#[test]
fn volume_keys_only_when_enabled() {
    let mut r = Remote::new(false);
    assert!(!r.handles("AudioVolumeUp"));
    assert_eq!(press(&mut r, "AudioVolumeUp", 0.0), vec![]);
    assert_eq!(remote_binding("AudioVolumeDown", false), None);
    let mut r = Remote::new(true);
    assert!(r.handles("AudioVolumeDown"));
    r.key("AudioVolumeUp", true, false, 0.0);
    assert_eq!(r.tick(900.0), Some(Toggle));
    r.key("AudioVolumeUp", false, false, 950.0);
    r.key("AudioVolumeDown", true, false, 2000.0);
    assert_eq!(r.tick(2900.0), Some(Retake));
}

#[test]
fn letters_case_insensitive_and_space_aliases() {
    let mut r = Remote::new(false);
    assert_eq!(r.key("B", true, false, 0.0), Some(Pause));
    for (i, key) in [" ", "Spacebar", "Space"].into_iter().enumerate() {
        assert!(r.handles(key));
        assert_eq!(
            r.key(key, true, false, 1000.0 * (i + 1) as f64),
            Some(Toggle)
        );
    }
    // Aliases resolve to the same binding; other names are case-sensitive.
    assert_eq!(remote_binding("Spacebar", false), Some((Toggle, None)));
    assert!(!r.handles("pagedown"));
}

#[test]
fn keys_list_matches_bindings() {
    for volume in [false, true] {
        let r = Remote::new(volume);
        let keys = r.keys();
        for k in &keys {
            assert!(r.handles(k), "{k:?}");
            assert!(remote_binding(k, volume).is_some());
        }
        let mut all = keys.clone();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), keys.len());
        assert_eq!(keys.len(), if volume { 17 } else { 15 });
        assert!(keys.contains(&" ") && keys.contains(&"b") && keys.contains(&"."));
        assert_eq!(keys.contains(&"AudioVolumeUp"), volume);
    }
}

#[test]
fn queued_key_up_applied_before_late_tick_is_a_short_press() {
    // The pump drains queued key events before ticking the hold timer: a
    // release that sat in the queue past HOLD_MS is still a short press.
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageUp", true, false, 40.0), None);
    assert_eq!(r.key("PageUp", false, false, 940.0), Some(BackSentence));
    assert_eq!(r.tick(940.0), None);
    let mut r = Remote::new(false);
    assert_eq!(r.key("PageDown", true, false, 40.0), None);
    assert_eq!(r.key("PageDown", false, false, 1200.0), Some(Forward));
    assert_eq!(r.tick(1200.0), None);
}
