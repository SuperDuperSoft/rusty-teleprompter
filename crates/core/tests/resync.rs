use rusty_core::*;

// the0 river1 flows2 quietly3 past4 the5 old6 stone7 mill8 farmers9 bring10
// golden11 wheat12 from13 distant14 hills15 every16 autumn17 children18
// gather19 apples20 beneath21 tall22 green23 orchard24 trees25 evening26 ...
const TEXT: &str = "The river flows quietly past the old stone mill. Farmers bring golden \
wheat from distant hills every autumn. Children gather apples beneath tall green orchard \
trees. Evening lanterns glow softly along the village road.";
const S1: &str = "the river flows quietly past the old stone mill";
const S2: &str = "farmers bring golden wheat from distant hills every autumn";
const S3: &str = "children gather apples beneath tall green orchard trees";
// love0 is1 patient2 and3 love4 is5 kind6 always7 we8 sing9 together10 now11
// love12 ... always19 then20 we21 pray22 quietly23 for24 our25 neighbours26 tonight27
const TWICE: &str = "Love is patient and love is kind always. We sing together now. Love is \
patient and love is kind always. Then we pray quietly for our neighbours tonight.";
const LOVE: &str = "love is patient and love is kind always";

fn read_three() -> Tracker {
    let mut t = Tracker::new(TEXT);
    for (s, at) in [(S1, 8), (S2, 17), (S3, 25)] {
        t.accept(s, true);
        assert_eq!(t.position, Some(at));
    }
    t
}
fn script() -> Script {
    Script {
        id: "s".into(),
        title: "Resync".into(),
        revision: 1,
        slides: vec![
            Slide {
                id: "a".into(),
                title: "First".into(),
                text: TEXT.into(),
            },
            Slide {
                id: "b".into(),
                title: "Second".into(),
                text: "This is the next slide".into(),
            },
        ],
    }
}
fn reading_session(settings: &Settings) -> Session {
    let mut s = Session::new(script());
    s.configure(settings);
    s.start();
    for t in [S1, S2, S3] {
        s.speech(t, true);
    }
    assert_eq!(s.tracker.position, Some(25));
    s
}

#[test]
fn rereading_earlier_sentence_moves_back() {
    let mut t = read_three();
    t.accept(S1, true);
    assert_eq!(t.position, Some(8));
    assert_eq!(t.last_outcome, Outcome::Back);
}
#[test]
fn back_resync_needs_final_or_two_partials() {
    let mut t = read_three();
    t.accept(S1, false);
    assert_eq!(t.position, Some(25));
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept(S1, false);
    assert_eq!(t.position, Some(8));
    assert_eq!(t.last_outcome, Outcome::Back);
}
#[test]
fn back_vote_cleared_by_intervening_forward_advance() {
    let mut t = read_three();
    t.accept(S1, false);
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept("evening lanterns glow", true);
    assert_eq!(t.position, Some(28));
    assert_eq!(t.last_outcome, Outcome::Advanced);
    t.accept(S1, false);
    assert_eq!(t.position, Some(28));
    assert_eq!(t.last_outcome, Outcome::Pending);
}
#[test]
fn back_vote_cleared_by_nomatch() {
    let mut t = read_three();
    t.accept(S1, false);
    t.accept("purple elephants dancing", false);
    assert_eq!(t.last_outcome, Outcome::NoMatch);
    t.accept(S1, false);
    assert_eq!(t.position, Some(25));
    assert_eq!(t.last_outcome, Outcome::Pending);
}
#[test]
fn rereading_last_words_does_not_move_back() {
    let mut t = read_three();
    t.accept("green orchard trees", true);
    assert_eq!(t.position, Some(25));
    assert_ne!(t.last_outcome, Outcome::Back);
    for _ in 0..2 {
        t.accept("beneath tall green orchard trees", false);
        assert_eq!(t.position, Some(25));
        assert_ne!(t.last_outcome, Outcome::Back);
    }
}
#[test]
fn common_words_never_resync_back() {
    let mut t = Tracker::new(
        "It was a cold day and we were at the gate. Then the bright parade marched \
         through the crowded square with drums. Everyone cheered loudly.",
    );
    t.accept("it was a cold day and we were at the gate", true);
    assert_eq!(t.position, Some(10));
    t.accept(
        "then the bright parade marched through the crowded square with drums",
        true,
    );
    assert_eq!(t.position, Some(21));
    for f in [false, false, true] {
        t.accept("and we were at the", f);
        assert_eq!(t.position, Some(21));
        assert_ne!(t.last_outcome, Outcome::Back);
    }
}
#[test]
fn final_ending_at_position_does_not_resync() {
    let mut t = Tracker::new(TEXT);
    t.accept(S1, true);
    t.accept(S2, true);
    assert_eq!(t.position, Some(17));
    t.accept(S2, true);
    assert_eq!(t.position, Some(17));
    assert_ne!(t.last_outcome, Outcome::Back);
}
#[test]
fn repeated_passage_resyncs_to_nearest_copy() {
    let mut t = Tracker::new(TWICE);
    t.accept(
        "love is patient and love is kind always we sing together now",
        true,
    );
    assert_eq!(t.position, Some(11));
    t.accept(
        "love is patient and love is kind always then we pray quietly",
        true,
    );
    assert_eq!(t.position, Some(23));
    t.accept(LOVE, true);
    assert_eq!(t.position, Some(19));
    assert_eq!(t.last_outcome, Outcome::Back);
}
#[test]
fn repeated_passage_three_words_past_near_copy_does_not_jump_to_far_copy() {
    let mut t = Tracker::new(TWICE);
    t.accept(
        "love is patient and love is kind always we sing together now",
        true,
    );
    assert_eq!(t.position, Some(11));
    t.accept("love is patient and love is kind always then we pray", true);
    assert_eq!(t.position, Some(22));
    for f in [false, false, true] {
        t.accept(LOVE, f);
        assert_eq!(t.position, Some(22));
        assert_ne!(t.last_outcome, Outcome::Back);
    }
}
#[test]
fn adlib_quoting_earlier_sentence_does_not_resync() {
    let mut t = Tracker::new(
        "Love is patient and love is kind. We sing together now in the warm hall. \
         Then we pray quietly for our neighbours tonight.",
    );
    t.accept("love is patient and love is kind", true);
    t.accept("we sing together now in the warm hall", true);
    t.accept("then we pray quietly", true);
    assert_eq!(t.position, Some(18));
    for f in [false, false, true] {
        t.accept("as i said earlier love is patient love is kind", f);
        assert_eq!(t.position, Some(18));
        assert_ne!(t.last_outcome, Outcome::Back);
    }
}
#[test]
fn forward_after_resync_ignores_pre_resync_context() {
    let mut t = read_three();
    t.accept(S1, false);
    t.accept(S1, false);
    assert_eq!(t.position, Some(8));
    // With the old location's words still in context, one word could not
    // advance on its own.
    t.accept("farmers", true);
    assert_eq!(t.position, Some(9));
    assert_eq!(t.last_outcome, Outcome::Advanced);
}
#[test]
fn resync_keeps_take() {
    let mut s = reading_session(&Settings::default());
    let take = s.take;
    s.speech(S1, true);
    assert_eq!(s.tracker.position, Some(8));
    assert_eq!(s.tracker.last_outcome, Outcome::Back);
    assert_eq!(s.take, take);
    assert_eq!(s.phase, Phase::Reading);
}
#[test]
fn resync_disabled_by_setting() {
    let settings = Settings {
        voice_resync: false,
        ..Settings::default()
    };
    let mut s = reading_session(&settings);
    assert!(!s.tracker.allow_back);
    s.speech(S1, false);
    s.speech(S1, false);
    s.speech(S1, true);
    assert_eq!(s.tracker.position, Some(25));
    assert_ne!(s.tracker.last_outcome, Outcome::Back);
    s.configure(&Settings::default());
    assert!(s.tracker.allow_back);
}
#[test]
fn allow_back_survives_new_trackers() {
    let mut s = Session::new(script());
    assert!(s.tracker.allow_back);
    s.configure(&Settings {
        voice_resync: false,
        ..Settings::default()
    });
    s.next();
    assert_eq!(s.slide_index, 1);
    assert!(!s.tracker.allow_back);
    s.jump(0);
    assert_eq!(s.slide_index, 0);
    assert!(!s.tracker.allow_back);
    s.pause();
    let mut update = script();
    update.revision = 2;
    assert!(s.apply_revision(update));
    assert!(!s.tracker.allow_back);
    let saved = s.position();
    s.restore(&saved);
    assert!(!s.tracker.allow_back);
}
#[test]
fn small_position_no_underflow() {
    let mut t = Tracker::new(TEXT);
    t.seek(3);
    assert_eq!(t.position, Some(2));
    for f in [false, false, true] {
        t.accept("purple elephants dance on tables", f);
        assert_eq!(t.position, Some(2));
        t.accept("the river flows river flows", f);
        assert_eq!(t.position, Some(2));
    }
    t.seek(1);
    t.accept("purple elephants dance on tables", true);
    assert_eq!(t.position, Some(0));
}
#[test]
fn forward_reading_after_resync_continues() {
    let mut t = read_three();
    t.accept(S1, true);
    assert_eq!(t.position, Some(8));
    t.accept(S2, true);
    assert_eq!(t.position, Some(17));
    assert_eq!(t.last_outcome, Outcome::Advanced);
    t.accept(S3, true);
    assert_eq!(t.position, Some(25));
    t.accept("evening lanterns glow softly", true);
    assert_eq!(t.position, Some(29));
}
#[test]
fn reading_repeat_ahead_then_junk_does_not_resync() {
    let mut t = Tracker::new(TWICE);
    t.accept(
        "love is patient and love is kind always we sing together now",
        true,
    );
    assert_eq!(t.position, Some(11));
    t.accept(&format!("{LOVE} zebra banana"), true);
    assert!(t.position >= Some(11), "{:?}", t.position);
    assert_ne!(t.last_outcome, Outcome::Back);
}
#[test]
fn rereading_then_one_junk_word_still_resyncs() {
    let mut t = read_three();
    t.accept(&format!("{S1} zebra"), true);
    assert_eq!(t.position, Some(8));
    assert_eq!(t.last_outcome, Outcome::Back);
}
#[test]
fn resync_needs_sixty_percent_coverage() {
    // 7 matches in 11 heard words: 35 >= 33.
    let mut t = read_three();
    t.accept(
        "zebra river zebra flows zebra quietly zebra past old stone mill",
        true,
    );
    assert_eq!(t.position, Some(8));
    assert_eq!(t.last_outcome, Outcome::Back);
    // 7 matches in 12 heard words: 35 < 36.
    let mut t = read_three();
    t.accept(
        "zebra river zebra flows zebra quietly zebra past zebra old stone mill",
        true,
    );
    assert_eq!(t.position, Some(25));
    assert_ne!(t.last_outcome, Outcome::Back);
}
#[test]
fn resync_needs_seventy_five_percent_density() {
    // 7 of the 9 script words from "farmers" to "autumn": 28 >= 27.
    let mut t = read_three();
    t.accept("farmers bring golden wheat distant hills autumn", true);
    assert_eq!(t.position, Some(17));
    assert_eq!(t.last_outcome, Outcome::Back);
    // 5 of 9: 20 < 27.
    let mut t = read_three();
    t.accept("farmers golden from hills autumn", true);
    assert_eq!(t.position, Some(25));
    assert_ne!(t.last_outcome, Outcome::Back);
}
#[test]
fn idle_accept_clears_back_votes() {
    let mut t = read_three();
    t.accept(S1, false);
    assert_eq!(t.last_outcome, Outcome::Pending);
    t.accept("", false);
    assert_eq!(t.last_outcome, Outcome::Idle);
    t.accept(S1, false);
    assert_eq!(t.position, Some(25));
    assert_eq!(t.last_outcome, Outcome::Pending);
}
