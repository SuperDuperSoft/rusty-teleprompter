use rusty_core::recording::{file_stem, safe_stem};

#[test]
fn keeps_safe_characters_and_turns_spaces_into_underscores() {
    assert_eq!(safe_stem("Sunday Welcome-2_b").as_deref(), Some("Sunday_Welcome-2_b"));
}

#[test]
fn drops_path_and_odd_characters() {
    assert_eq!(safe_stem("../../evil\\name:?*").as_deref(), Some("evilname"));
    assert_eq!(safe_stem("Café “quotes”").as_deref(), Some("Caf_quotes"));
}

#[test]
fn collapses_repeated_underscores_and_trims_them() {
    assert_eq!(safe_stem("  a   b  ").as_deref(), Some("a_b"));
}

#[test]
fn rejects_names_with_nothing_left() {
    assert_eq!(safe_stem(""), None);
    assert_eq!(safe_stem("../:*"), None);
}

#[test]
fn limits_length() {
    let long = "x".repeat(300);
    assert_eq!(safe_stem(&long).unwrap().len(), 100);
}

#[test]
fn file_stem_combines_app_title_and_time() {
    assert_eq!(
        file_stem("Easter Sunday", "2026-11-05_19-58-03"),
        "RustyPrompter_Easter_Sunday_2026-11-05_19-58-03"
    );
}

#[test]
fn file_stem_without_a_usable_title() {
    assert_eq!(file_stem("“”", "2026-11-05_19-58-03"), "RustyPrompter_2026-11-05_19-58-03");
}

#[test]
fn file_stem_output_is_always_safe() {
    let stem = file_stem(&"Long title ".repeat(30), "2026-11-05_19-58-03");
    assert_eq!(safe_stem(&stem).as_deref(), Some(stem.as_str()));
    assert!(stem.ends_with("2026-11-05_19-58-03"));
}
