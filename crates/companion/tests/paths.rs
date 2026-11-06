use rusty_companion::paths::data_folder_for;
use std::{ffi::OsString, path::PathBuf};

fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let pairs: Vec<(String, String)> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    move |name| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.into())
}

#[test]
fn windows_keeps_local_app_data() {
    let folder = data_folder_for("windows", vars(&[("LOCALAPPDATA", r"C:\Users\a\AppData\Local")])).unwrap();
    assert_eq!(folder, PathBuf::from(r"C:\Users\a\AppData\Local").join("RustyPrompter"));
}

#[test]
fn macos_uses_application_support() {
    let folder = data_folder_for("macos", vars(&[("HOME", "/Users/a")])).unwrap();
    assert_eq!(folder, PathBuf::from("/Users/a/Library/Application Support/RustyPrompter"));
}

#[test]
fn linux_prefers_xdg_data_home() {
    let folder = data_folder_for("linux", vars(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/a")])).unwrap();
    assert_eq!(folder, PathBuf::from("/data/rustyprompter"));
}

#[test]
fn linux_falls_back_to_local_share() {
    let folder = data_folder_for("linux", vars(&[("HOME", "/home/a")])).unwrap();
    assert_eq!(folder, PathBuf::from("/home/a/.local/share/rustyprompter"));
}

#[test]
fn empty_or_relative_xdg_is_ignored() {
    for xdg in ["", "relative/data"] {
        let folder = data_folder_for("linux", vars(&[("XDG_DATA_HOME", xdg), ("HOME", "/home/a")])).unwrap();
        assert_eq!(folder, PathBuf::from("/home/a/.local/share/rustyprompter"), "XDG_DATA_HOME={xdg:?}");
    }
}

#[test]
fn other_unix_follows_linux() {
    let folder = data_folder_for("freebsd", vars(&[("HOME", "/home/a")])).unwrap();
    assert_eq!(folder, PathBuf::from("/home/a/.local/share/rustyprompter"));
}

#[test]
fn missing_home_is_an_error_not_the_current_folder() {
    assert!(data_folder_for("windows", vars(&[])).is_err());
    assert!(data_folder_for("macos", vars(&[("HOME", "")])).is_err());
    assert!(data_folder_for("linux", vars(&[])).is_err());
}

#[test]
fn videos_go_in_the_users_videos_or_movies_folder() {
    use rusty_companion::paths::videos_folder_for;
    let windows = videos_folder_for("windows", vars(&[("USERPROFILE", r"C:\Users\a")])).unwrap();
    assert_eq!(windows, PathBuf::from(r"C:\Users\a").join("Videos").join("RustyPrompter"));
    let mac = videos_folder_for("macos", vars(&[("HOME", "/Users/a")])).unwrap();
    assert_eq!(mac, PathBuf::from("/Users/a/Movies/RustyPrompter"));
    let linux = videos_folder_for("linux", vars(&[("HOME", "/home/a")])).unwrap();
    assert_eq!(linux, PathBuf::from("/home/a/Videos/RustyPrompter"));
    assert!(videos_folder_for("windows", vars(&[])).is_err());
}
