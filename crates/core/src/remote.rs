//! Bluetooth clicker / pedal key mapping: hold detection and debounce. Times are passed in.
use serde::{Deserialize, Serialize};

pub const HOLD_MS: f64 = 900.0;
pub const STUCK_KEY_MS: f64 = 10_000.0;
pub const REPEAT_GUARD_MS: f64 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAction {
    Forward,
    Toggle,
    Play,
    Pause,
    BackSentence,
    Retake,
}

use RemoteAction::*;

/// (canonical key, short action, hold action, volume key).
type Binding = (&'static str, RemoteAction, Option<RemoteAction>, bool);
const BINDINGS: [Binding; 17] = [
    ("PageDown", Forward, Some(Toggle), false),
    ("ArrowRight", Forward, Some(Toggle), false),
    ("ArrowDown", Forward, Some(Toggle), false),
    ("MediaTrackNext", Forward, Some(Toggle), false),
    ("PageUp", BackSentence, Some(Retake), false),
    ("ArrowLeft", BackSentence, Some(Retake), false),
    ("ArrowUp", BackSentence, Some(Retake), false),
    ("MediaTrackPrevious", BackSentence, Some(Retake), false),
    (" ", Toggle, None, false),
    ("Enter", Toggle, None, false),
    ("MediaPlayPause", Toggle, None, false),
    ("MediaPlay", Play, None, false),
    ("MediaPause", Pause, None, false),
    ("b", Pause, None, false),
    (".", Pause, None, false),
    ("AudioVolumeUp", Forward, Some(Toggle), true),
    ("AudioVolumeDown", BackSentence, Some(Retake), true),
];

fn binding(key: &str, volume_keys: bool) -> Option<&'static Binding> {
    let key = match key {
        "Spacebar" | "Space" => " ".to_string(),
        k if k.chars().count() == 1 => k.to_lowercase(),
        k => k.to_string(),
    };
    BINDINGS
        .iter()
        .find(|b| b.0 == key && (volume_keys || !b.3))
}

/// (short-press action, hold action) for a DOM `KeyboardEvent.key` value.
pub fn remote_binding(
    key: &str,
    volume_keys: bool,
) -> Option<(RemoteAction, Option<RemoteAction>)> {
    binding(key, volume_keys).map(|b| (b.1, b.2))
}

#[derive(Clone, Debug)]
struct Held {
    key: &'static str,
    hold: RemoteAction,
    down_at: f64,
    fired: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Remote {
    pub volume_keys: bool,
    held: Option<Held>,
    /// A key discarded as stuck, ignored until its key-up.
    ignored: Option<&'static str>,
    last_fire: Option<(RemoteAction, f64)>,
}

impl Remote {
    pub fn new(volume_keys: bool) -> Self {
        Self {
            volume_keys,
            ..Self::default()
        }
    }
    /// Bound keys, for the JS preventDefault allowlist.
    pub fn keys(&self) -> Vec<&'static str> {
        BINDINGS
            .iter()
            .filter(|b| self.volume_keys || !b.3)
            .map(|b| b.0)
            .collect()
    }
    pub fn handles(&self, key: &str) -> bool {
        binding(key, self.volume_keys).is_some()
    }
    pub fn key(
        &mut self,
        key: &str,
        down: bool,
        repeat: bool,
        now_ms: f64,
    ) -> Option<RemoteAction> {
        let &(key, short, hold, _) = binding(key, self.volume_keys)?;
        if self.ignored == Some(key) {
            if !down {
                self.ignored = None;
            }
            return None;
        }
        let mine = self.held.as_ref().is_some_and(|h| h.key == key);
        if !down {
            let held = if mine { self.held.take() } else { None };
            return match held {
                Some(h) if !h.fired => self.fire(short, now_ms),
                _ => None,
            };
        }
        if repeat {
            let h = self.held.as_mut().filter(|h| h.key == key && !h.fired)?;
            if now_ms - h.down_at < HOLD_MS {
                return None;
            }
            h.fired = true;
            return Some(h.hold);
        }
        self.held = None;
        match hold {
            None => self.fire(short, now_ms),
            Some(hold) => {
                self.held = Some(Held {
                    key,
                    hold,
                    down_at: now_ms,
                    fired: false,
                });
                None
            }
        }
    }
    pub fn tick(&mut self, now_ms: f64) -> Option<RemoteAction> {
        let h = self.held.as_mut()?;
        let age = now_ms - h.down_at;
        if !h.fired && age >= HOLD_MS {
            h.fired = true;
            return Some(h.hold);
        }
        if age >= STUCK_KEY_MS {
            self.ignored = Some(h.key);
            self.held = None;
        }
        None
    }
    pub fn reset(&mut self) {
        self.held = None;
        self.ignored = None;
    }
    /// A short action, dropped (and the guard extended) when it repeats within `REPEAT_GUARD_MS`.
    fn fire(&mut self, action: RemoteAction, now_ms: f64) -> Option<RemoteAction> {
        let last = self.last_fire.replace((action, now_ms));
        match last {
            Some((a, t)) if a == action && now_ms - t < REPEAT_GUARD_MS => None,
            _ => Some(action),
        }
    }
}
