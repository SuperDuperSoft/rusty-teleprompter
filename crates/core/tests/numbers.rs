use rusty_core::*;

fn said(text: &str) -> String {
    words(text)
        .into_iter()
        .map(|w| w.text)
        .collect::<Vec<_>>()
        .join(" ")
}
fn spans(text: &str) -> Vec<&str> {
    words(text).iter().map(|w| &text[w.start..w.end]).collect()
}
fn session(text: &str) -> Session {
    Session::new(Script {
        id: "s".into(),
        title: "Numbers".into(),
        revision: 1,
        slides: vec![Slide {
            id: "a".into(),
            title: "A".into(),
            text: text.into(),
        }],
    })
}

#[test]
fn cardinals() {
    assert_eq!(said("0"), "zero");
    assert_eq!(said("7"), "seven");
    assert_eq!(said("13"), "thirteen");
    assert_eq!(said("21"), "twenty one");
    assert_eq!(said("99"), "ninety nine");
    assert_eq!(said("100"), "one hundred");
    assert_eq!(said("123"), "one hundred twenty three");
    assert_eq!(said("1000"), "one thousand");
    assert_eq!(said("1,250,000"), "one million two hundred fifty thousand");
}
#[test]
fn years() {
    assert_eq!(said("in 1990"), "in nineteen ninety");
    assert_eq!(said("since 1905"), "since nineteen oh five");
    assert_eq!(said("by 1900"), "by nineteen hundred");
    assert_eq!(said("in 2000"), "in two thousand");
    assert_eq!(said("in 2005"), "in two thousand five");
    assert_eq!(said("in 2024"), "in twenty twenty four");
    assert_eq!(said("in 1066"), "in one thousand sixty six");
}
#[test]
fn year_needs_context() {
    assert_eq!(said("1500 people"), "one thousand five hundred people");
    assert_eq!(said("2024 members"), "two thousand twenty four members");
}
#[test]
fn ordinals() {
    assert_eq!(said("1st"), "first");
    assert_eq!(said("2nd"), "second");
    assert_eq!(said("3rd"), "third");
    assert_eq!(said("21st"), "twenty first");
    assert_eq!(said("100th"), "one hundredth");
    assert_eq!(said("12th"), "twelfth");
}
#[test]
fn money_and_percent() {
    assert_eq!(said("$1"), "one dollar");
    assert_eq!(said("$5"), "five dollars");
    assert_eq!(said("$2.50"), "two dollars fifty cents");
    assert_eq!(said("$0.50"), "fifty cents");
    assert_eq!(said("$1,250.00"), "one thousand two hundred fifty dollars");
    assert_eq!(said("50%"), "fifty percent");
    assert_eq!(said("2.5%"), "two point five percent");
    assert_eq!(spans("pay $5 now"), vec!["pay", "", "$5", "now"]);
}
#[test]
fn money_falls_back_to_plain_form_when_grouped_does_not_apply() {
    // grouped `$1,234` is followed by a digit, so the plain `$1` form applies
    assert_eq!(
        said("$1,2345"),
        "one dollar two thousand three hundred forty five"
    );
}
#[test]
fn decimals_and_leading_zeros() {
    assert_eq!(said("3.14"), "three point one four");
    assert_eq!(said("007"), "zero zero seven");
}
#[test]
fn verse_parts_keep_their_own_spans() {
    assert_eq!(said("John 3:16"), "john three sixteen");
    assert_eq!(spans("John 3:16"), vec!["John", "3", "16"]);
}
#[test]
fn verse_range_and_time() {
    assert_eq!(said("8:28-30"), "eight twenty eight thirty");
    assert_eq!(said("8:28–9:2"), "eight twenty eight nine two");
    assert_eq!(said("3:05"), "three oh five");
    assert_eq!(said("10:00"), "ten");
}
#[test]
fn letters_next_to_digits_unchanged() {
    assert_eq!(said("3D"), "3d");
    assert_eq!(said("mp3"), "mp3");
    assert_eq!(said("covid19"), "covid19");
    assert_eq!(said("1990s"), "1990s");
    assert_eq!(said("80s"), "80s");
    assert_eq!(said("5:30pm"), "five 30pm");
}
#[test]
fn possessive_year() {
    assert_eq!(said("in 1990's"), "in nineteen ninety");
    assert_eq!(spans("in 1990's"), vec!["in", "", "1990's"]);
}
#[test]
fn non_ascii_digits_are_plain_words() {
    assert_eq!(words("٣").len(), 1);
    assert_eq!(said("٣"), "٣");
    assert_eq!(said("３"), "３");
}
#[test]
fn huge_number_digit_by_digit() {
    assert_eq!(
        said("1234567890123"),
        "one two three four five six seven eight nine zero one two three"
    );
}
#[test]
fn spans_never_overlap() {
    let text = "In 1990's we paid $1,250.00 [cue 5:30] for 50% of 2.5% at 8:28–9:2, \
        then 3:05 or 10:00; 1st, 21st, 007, 3.14, $0.50, $ 5, $$7, US$9, 3D mp3 \
        covid19 1990s 80s 5:30pm ٣ ３ 1234567890123 1,000,000,000,000 $1. 1,25 \
        8:28-30 since 1905 Done. $5 is enough.";
    let w = words(text);
    assert!(!w.is_empty());
    for i in 0..w.len() {
        assert!(w[i].start <= w[i].end);
        let _ = &text[w[i].start..w[i].end];
        if i > 0 {
            assert!(w[i - 1].end <= w[i].start);
            let _ = &text[w[i - 1].end..w[i].start];
        }
    }
    assert!(!w.iter().any(|w| w.text == "cue"));
}
#[test]
fn unit_start_and_end_walk() {
    // the0 twenty1 one2 people3 three4 sixteen5
    let w = words("the 21 people 3:16");
    assert_eq!(w.len(), 6);
    assert_eq!((unit_start(&w, 2), unit_end(&w, 1)), (1, 2));
    assert_eq!((unit_start(&w, 1), unit_end(&w, 2)), (1, 2));
    assert_eq!((unit_start(&w, 0), unit_end(&w, 0)), (0, 0));
    assert_eq!((unit_start(&w, 3), unit_end(&w, 3)), (3, 3));
    assert_eq!((unit_start(&w, 5), unit_end(&w, 4)), (5, 4));
}
#[test]
fn sentence_start_on_expanded_number() {
    let text = "Done. 21 people came.";
    assert_eq!(sentence_starts(text, &words(text)), vec![0, 1]);
}
#[test]
fn sentence_start_on_money() {
    let text = "Done. $5 is enough.";
    let w = words(text);
    assert_eq!(w[1].text, "five");
    assert_eq!(sentence_starts(text, &w), vec![0, 1]);
}
#[test]
fn eighty_does_not_match_eighth() {
    let mut t = Tracker::new("the eighth day");
    t.accept("the eighty day", true);
    assert_ne!(t.position, Some(2));
}
#[test]
fn position_saved_with_old_scheme_is_not_restored() {
    let mut s = session("one two three four five");
    s.seek(3);
    let current = s.position();
    assert_eq!(current.words_scheme, WORDS_SCHEME);
    let old: Position = serde_json::from_str(
        r#"{"script_id":"s","revision":1,"slide_id":"a","next_word":2,"finished":false}"#,
    )
    .unwrap();
    assert_eq!(old.words_scheme, 0);
    let mut fresh = session("one two three four five");
    let before = (fresh.tracker.position, fresh.phase);
    fresh.restore(&old);
    assert_eq!((fresh.tracker.position, fresh.phase), before);
    fresh.restore(&current);
    assert_eq!(fresh.tracker.position, s.tracker.position);
    let json = serde_json::to_string(&current).unwrap();
    let back: Position = serde_json::from_str(&json).unwrap();
    assert_eq!(back.words_scheme, WORDS_SCHEME);
}
#[test]
fn ordinal_suffix_is_ascii_only() {
    // U+017F (long s) case-folds to "s"; it must not count as "st".
    assert_eq!(said("1ſt"), "1ſt");
    assert_eq!(said("1ST"), "first");
    assert_eq!(said("2Nd"), "second");
    assert_eq!(said("1K"), "1k");
}
#[test]
fn decimal_dollars_are_not_cents() {
    assert_eq!(said("$2.5 million"), "two point five dollars million");
    assert_eq!(said("$1,000.5"), "one thousand point five dollars");
    assert_eq!(said("$2.505"), "two point five zero five dollars");
    assert_eq!(said("$2.50"), "two dollars fifty cents");
    assert_eq!(said("It cost $2. Then"), "it cost two dollars then");
    assert_eq!(spans("$2.5 million"), ["", "", "", "$2.5", "million"]);
    let text = "We raised $2.5 million this year";
    let w = words(text);
    assert_eq!(sentence_starts(text, &w), [0]);
}
