//! Names for camera recordings. Shared by the UI (which names a take) and
//! the PC companion (which re-checks the name before touching the disk).

/// Longest name `safe_stem` returns, in bytes (all ASCII).
pub const MAX_STEM: usize = 100;

/// A file name stem with only `A-Z a-z 0-9 - _`. Whitespace becomes one `_`,
/// anything else is dropped, so it can never name a folder or a drive.
/// `None` when nothing usable is left.
pub fn safe_stem(raw: &str) -> Option<String> {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c);
        } else if (c == '_' || c.is_whitespace()) && !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
        if out.len() >= MAX_STEM {
            break;
        }
    }
    let out = out.trim_end_matches('_');
    (!out.is_empty()).then(|| out.to_string())
}

/// `RustyPrompter_<title>_<stamp>`, e.g. `RustyPrompter_Easter_2026-11-05_19-58-03`.
pub fn file_stem(title: &str, stamp: &str) -> String {
    let title = safe_stem(title)
        .map(|t| t.chars().take(60).collect::<String>())
        .map(|t| t.trim_end_matches('_').to_string());
    let stamp = safe_stem(stamp).unwrap_or_default();
    let joined = match title {
        Some(t) if !t.is_empty() => format!("RustyPrompter_{t}_{stamp}"),
        _ => format!("RustyPrompter_{stamp}"),
    };
    safe_stem(&joined).unwrap_or_else(|| "RustyPrompter".into())
}
