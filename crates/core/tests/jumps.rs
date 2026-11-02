//! Regression tests: the tracker must not leap ahead on one distinctive word,
//! on common words, or by riding old context, yet must still follow real skips.
use rusty_core::*;

/// A long script with numbered, distinct sentences and "ducks" far ahead.
fn long_script() -> String {
    let mut s = String::from(
        "Good morning everyone and welcome to our service today. \
         We gathered here this morning to talk about patience and kindness. \
         Many people struggle with waiting for good things to arrive. \
         ",
    );
    for i in 0..12 {
        s.push_str(&format!(
            "Paragraph {} explains how small steady habits grow into lasting change over many seasons. ",
            ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet", "kilo", "lima"][i]
        ));
    }
    s.push_str("Down by the pond the ducks were swimming in a quiet line together. ");
    s.push_str("That is where our story ends for this morning.");
    s
}

fn index_of(t: &Tracker, word: &str) -> usize {
    t.words.iter().position(|w| w.text == word).unwrap()
}

/// Feed words the way Vosk does: growing partials, then a final.
fn speak(t: &mut Tracker, sentence: &str) {
    let words: Vec<&str> = sentence.split_whitespace().collect();
    for n in 1..=words.len() {
        t.accept(&words[..n].join(" "), false);
    }
    t.accept(sentence, true);
}

#[test]
fn one_distinct_word_never_jumps_far() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    speak(&mut t, "we gathered here this morning to talk about");
    let before = t.position.unwrap();
    // Ad-lib that mentions ducks (which appear ~150 words later).
    speak(&mut t, "ducks");
    speak(&mut t, "i love the ducks");
    let after = t.position.unwrap();
    assert!(after < before + 6, "jumped from {before} to {after}");
    assert!(after < index_of(&t, "ducks"));
}

#[test]
fn old_context_cannot_carry_a_far_jump() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    speak(
        &mut t,
        "we gathered here this morning to talk about patience and kindness",
    );
    let before = t.position.unwrap();
    // A partial with one far word right after strong context.
    t.accept("the ducks", false);
    t.accept("the ducks were", false);
    let after = t.position.unwrap();
    assert!(after < before + 6, "jumped from {before} to {after}");
}

#[test]
fn common_words_do_not_jump() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    let before = t.position.unwrap();
    speak(&mut t, "and the in a to of");
    let after = t.position.unwrap();
    assert!(after < before + 6, "jumped from {before} to {after}");
}

#[test]
fn repeated_paragraph_stays_on_nearest_copy() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    speak(
        &mut t,
        "we gathered here this morning to talk about patience and kindness",
    );
    speak(
        &mut t,
        "many people struggle with waiting for good things to arrive",
    );
    speak(
        &mut t,
        "paragraph alpha explains how small steady habits grow into lasting change",
    );
    let p = t.position.unwrap();
    let alpha_end = index_of(&t, "alpha") + 10;
    assert!(
        p <= alpha_end + 3 && p + 3 >= alpha_end,
        "at {p}, expected near {alpha_end}"
    );
}

#[test]
fn deliberate_skip_ahead_is_followed() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    // Speaker skips many paragraphs and reads the duck sentence clearly.
    speak(
        &mut t,
        "down by the pond the ducks were swimming in a quiet line",
    );
    let line = index_of(&t, "line");
    let p = t.position.unwrap();
    assert!(p + 1 >= line, "at {p}, expected near {line}");
}

#[test]
fn skipping_one_sentence_still_works_with_partials() {
    let mut t = Tracker::new(&long_script());
    speak(
        &mut t,
        "good morning everyone and welcome to our service today",
    );
    speak(
        &mut t,
        "many people struggle with waiting for good things to arrive",
    );
    let arrive = index_of(&t, "arrive");
    assert_eq!(t.position, Some(arrive));
}

#[test]
fn slow_reader_progresses_word_by_word() {
    let mut t = Tracker::new(&long_script());
    for w in "good morning everyone and welcome to our service today".split(' ') {
        t.accept(w, true);
    }
    assert_eq!(t.position, Some(index_of(&t, "today")));
}

fn thousand_word_script() -> String {
    let mut s = String::from("Thank you all for coming tonight to hear this message. ");
    let fill = [
        "river", "mountain", "garden", "window", "harvest", "lantern", "meadow", "valley",
    ];
    let mut i = 0;
    while s.split_whitespace().count() < 1000 {
        s.push_str(&format!(
            "The {} reminds us that every season brings something new to learn. ",
            fill[i % fill.len()]
        ));
        i += 1;
    }
    s.push_str("Finally the ducks were swimming across the lake at sunset.");
    s
}

#[test]
fn ducks_a_thousand_words_down_does_not_jump() {
    let mut t = Tracker::new(&thousand_word_script());
    speak(
        &mut t,
        "thank you all for coming tonight to hear this message",
    );
    let before = t.position.unwrap();
    for adlib in [
        "ducks",
        "ducks ducks",
        "the ducks",
        "look at the ducks swimming",
        "the ducks across the lake",
    ] {
        speak(&mut t, adlib);
        let after = t.position.unwrap();
        assert!(
            after < before + 6,
            "'{adlib}' jumped from {before} to {after}"
        );
    }
}

#[test]
fn shared_phrase_ad_lib_does_not_jump() {
    let mut t = Tracker::new(&thousand_word_script());
    speak(
        &mut t,
        "thank you all for coming tonight to hear this message",
    );
    let before = t.position.unwrap();
    speak(&mut t, "the lake at sunset");
    speak(&mut t, "we will learn at sunset across the lake");
    let after = t.position.unwrap();
    assert!(after < before + 6, "jumped from {before} to {after}");
}

#[test]
fn long_slide_stays_fast() {
    let mut t = Tracker::new(&thousand_word_script());
    let started = std::time::Instant::now();
    for _ in 0..40 {
        t.accept("the river reminds us that every season brings", false);
    }
    // Generous bound for debug builds on slow CI.
    assert!(
        started.elapsed().as_secs_f64() < 4.0,
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn full_read_of_repetitive_script_tracks_every_sentence() {
    let text = thousand_word_script();
    let mut t = Tracker::new(&text);
    let mut spoken = 0;
    for sentence in text.split_inclusive('.') {
        let clean: String = sentence
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect();
        let n = clean.split_whitespace().count();
        if n == 0 {
            continue;
        }
        speak(&mut t, clean.trim());
        spoken += n;
        let p = t.position.unwrap() + 1;
        assert!(
            p <= spoken && p + 2 >= spoken,
            "after {spoken} words tracker at {p}"
        );
    }
    assert_eq!(t.position, Some(t.words.len() - 1));
}
