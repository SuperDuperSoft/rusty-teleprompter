use rusty_core::*;

fn script(texts: &[&str]) -> Script {
    Script {
        id: "g".into(),
        title: "Grammar".into(),
        revision: 1,
        slides: texts
            .iter()
            .enumerate()
            .map(|(i, t)| Slide {
                id: format!("s{i}"),
                title: format!("Slide {i}"),
                text: (*t).into(),
            })
            .collect(),
    }
}

#[test]
fn vocabulary_is_sorted_unique_with_unk_last() {
    let v = vocabulary(&script(&["Zebra apple. Apple zebra!", "mango apple"]));
    assert_eq!(v.last().map(String::as_str), Some("[unk]"));
    let body = &v[..v.len() - 1];
    assert!(body.windows(2).all(|p| p[0] < p[1]), "{body:?}");
    for w in ["apple", "mango", "zebra"] {
        assert_eq!(body.iter().filter(|x| *x == w).count(), 1, "{w}");
    }
    assert_eq!(v.iter().filter(|x| *x == "[unk]").count(), 1);
}

#[test]
fn cues_excluded_numbers_expanded() {
    let v = vocabulary(&script(&["[Smile warmly] Read John 3:16 for $25"]));
    assert!(!v.iter().any(|w| w == "smile" || w == "warmly"));
    assert!(!v
        .iter()
        .any(|w| w.chars().any(|c| c.is_ascii_digit() || c == '$')));
    for w in [
        "read", "john", "for", "sixteen", "twenty", "five", "dollars",
    ] {
        assert!(v.iter().any(|x| x == w), "{w}");
    }
}

#[test]
fn number_and_filler_words_included() {
    let v = vocabulary(&script(&["hello"]));
    for w in [
        "zero",
        "nineteen",
        "ninety",
        "hundred",
        "billion",
        "first",
        "twentieth",
        "billionth",
        "chapter",
        "verse",
        "verses",
        "to",
        "through",
        "and",
        "a",
        "oh",
        "point",
        "dollar",
        "dollars",
        "cent",
        "cents",
        "percent",
        "hello",
    ] {
        assert!(v.iter().any(|x| x == w), "{w}");
    }
    assert!(!v.iter().any(String::is_empty));
}

#[test]
fn grammar_json_is_a_json_array() {
    let s = script(&["Hello there", "General Kenobi"]);
    let json = grammar_json(&s).unwrap();
    let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, vocabulary(&s));
    assert_eq!(grammar_skip_reason(&s), None);
}

#[test]
fn oversized_vocabulary_returns_none() {
    let text: Vec<String> = (0..GRAMMAR_MAX).map(|i| format!("word{i}")).collect();
    let s = script(&[&text.join(" ")]);
    assert!(vocabulary(&s).len() > GRAMMAR_MAX);
    assert_eq!(grammar_json(&s), None);
    assert_eq!(grammar_skip_reason(&s), Some("too_large"));
}

#[test]
fn vocabulary_of_wordless_script() {
    for s in [script(&[]), script(&["", "[Only a cue]", "  \n "])] {
        assert!(vocabulary(&s).is_empty());
        assert_eq!(grammar_json(&s), None);
        assert_eq!(grammar_skip_reason(&s), Some("empty"));
    }
}

#[test]
fn unk_tokens_keep_adlib_penalty() {
    let text = "Praise the lord with gladness and sing a joyful song today";
    let mut plain = Tracker::new(text);
    plain.accept("the lord", true);
    assert_eq!(plain.position, Some(2));
    for hyp in ["the [unk] [unk] lord", "the [UNK] [Unk] lord"] {
        let mut t = Tracker::new(text);
        assert_eq!(t.accept(hyp, true), None, "{hyp}");
        assert_eq!(t.last_outcome, Outcome::NoMatch);
    }
    // A script that literally says the reserved token never matches it.
    let mut t = Tracker::new("zzunk zzunk zzunk zzunk");
    assert_eq!(t.accept("[unk] [unk] [unk] [unk]", true), None);
    let mut t = Tracker::new(text);
    assert_eq!(t.accept("[unk]", true), None);
    assert_eq!(t.last_outcome, Outcome::NoMatch);
}

#[test]
fn unk_only_partial_is_idle() {
    let mut s = Session::new(script(&[
        "Welcome to our bright green studio",
        "Next slide",
    ]));
    s.configure(&Settings {
        lost_hint_popup: true,
        ..Settings::default()
    });
    s.start();
    assert_eq!(s.phase, Phase::Reading);
    s.speech("welcome to our", true);
    assert_eq!(s.tracker.last_outcome, Outcome::Advanced);
    let at = s.tracker.position;
    s.tick(5000);
    let stuck = s.stuck_ms;
    for (text, is_final) in [("[unk]", false), (" [unk] [UNK] ", false), ("[unk]", true)] {
        let ago = s.heard_ago_ms;
        s.speech(text, is_final);
        assert_eq!(s.tracker.position, at);
        assert_eq!(s.tracker.last_outcome, Outcome::Idle);
        assert!(!s.hearing());
        assert_eq!(s.heard_ago_ms, ago);
        s.tick(1000);
        assert_eq!(s.stuck_ms, stuck);
    }
    // Real words around [unk] are still heard.
    s.speech("[unk] bright green", false);
    assert!(s.hearing());
}
