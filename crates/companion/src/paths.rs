//! Where the companion keeps its database when `--data` isn't given.
use std::{ffi::OsString, path::PathBuf};

/// The data folder for this computer.
pub fn data_folder() -> anyhow::Result<PathBuf> {
    data_folder_for(std::env::consts::OS, |name: &str| std::env::var_os(name))
}

/// The data folder for `os` (a `std::env::consts::OS` value), reading
/// environment variables through `var` so every platform can be tested anywhere.
/// Empty values count as unset. Windows keeps its original location.
pub fn data_folder_for(
    os: &str,
    var: impl Fn(&str) -> Option<OsString>,
) -> anyhow::Result<PathBuf> {
    let get = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let missing = |name: &str| anyhow::anyhow!("{name} is not set, so RustyPrompter can't find a folder for your scripts. Start it with --data <folder>.");
    match os {
        "windows" => Ok(get("LOCALAPPDATA").ok_or_else(|| missing("LOCALAPPDATA"))?.join("RustyPrompter")),
        "macos" => Ok(get("HOME")
            .ok_or_else(|| missing("HOME"))?
            .join("Library/Application Support/RustyPrompter")),
        // XDG Base Directory spec: relative XDG_DATA_HOME values are invalid and ignored.
        _ => match get("XDG_DATA_HOME").filter(|p| p.has_root()) {
            Some(data) => Ok(data.join("rustyprompter")),
            None => Ok(get("HOME").ok_or_else(|| missing("HOME"))?.join(".local/share/rustyprompter")),
        },
    }
}

/// Where camera recordings made while reading on this PC are saved.
pub fn videos_folder() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    if let Some(known) = known_videos() {
        // Follows the Videos library even when OneDrive has moved it.
        return Ok(known.join("RustyPrompter"));
    }
    videos_folder_for(std::env::consts::OS, |name: &str| std::env::var_os(name))
}

/// The videos folder for `os`: Windows `Videos`, macOS `Movies`, others `Videos`.
pub fn videos_folder_for(
    os: &str,
    var: impl Fn(&str) -> Option<OsString>,
) -> anyhow::Result<PathBuf> {
    let home_var = if os == "windows" { "USERPROFILE" } else { "HOME" };
    let home = var(home_var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("{home_var} is not set, so RustyPrompter can't find your Videos folder."))?;
    let videos = if os == "macos" { "Movies" } else { "Videos" };
    Ok(home.join(videos).join("RustyPrompter"))
}

#[cfg(windows)]
fn known_videos() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Videos, SHGetKnownFolderPath},
    };
    // SAFETY: SHGetKnownFolderPath writes a NUL-terminated wide string that
    // must be freed with CoTaskMemFree, even when the call fails.
    unsafe {
        let mut raw: windows_sys::core::PWSTR = std::ptr::null_mut();
        let result = SHGetKnownFolderPath(&FOLDERID_Videos, 0, std::ptr::null_mut(), &mut raw);
        let path = if result >= 0 && !raw.is_null() {
            let len = (0usize..).take_while(|&i| *raw.add(i) != 0).count();
            Some(PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(raw, len))))
        } else {
            None
        };
        if !raw.is_null() {
            CoTaskMemFree(raw as *const _);
        }
        path.filter(|p| p.is_absolute())
    }
}
