//! Toast text for a remote key action (§12), judged by what the action did.
use rusty_core::{Phase, RemoteAction};

/// `before`/`after` are (phase, slide index) around the dispatch; `take` is
/// the take number after it. `None` when the action changed nothing visible.
pub fn remote_toast_text(
    action: RemoteAction,
    before: (Phase, usize),
    after: (Phase, usize),
    take: u64,
) -> Option<String> {
    let was_idle = matches!(before.0, Phase::Ready | Phase::Paused | Phase::Finished);
    let text = match action {
        RemoteAction::Retake => format!("Retake · take {take}"),
        RemoteAction::BackSentence => "Sentence back".into(),
        _ if after.0 == Phase::Finished && before.0 != Phase::Finished => "Finished".into(),
        _ if after.0 == Phase::Paused && before.0 != Phase::Paused => "Paused".into(),
        // A slide change from a live (or paused-holding) slide is "Next";
        // Read again from Finished also moves the slide but is a start.
        _ if after.1 != before.1 && before.0 != Phase::Finished => "Next ›".into(),
        _ if was_idle && after.0.running() => "Start".into(),
        _ if before.0 == Phase::Rolling && after.0 == Phase::Reading => "Start now".into(),
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Phase::*;
    use RemoteAction::*;

    fn t(action: RemoteAction, before: (Phase, usize), after: (Phase, usize)) -> Option<String> {
        remote_toast_text(action, before, after, 3)
    }

    #[test]
    fn start_pause_and_finish() {
        assert_eq!(t(Forward, (Ready, 0), (Rolling, 0)).as_deref(), Some("Start"));
        assert_eq!(t(Toggle, (Paused, 1), (Reading, 1)).as_deref(), Some("Start"));
        assert_eq!(t(Toggle, (Finished, 3), (Rolling, 0)).as_deref(), Some("Start"));
        assert_eq!(t(Toggle, (Reading, 1), (Paused, 1)).as_deref(), Some("Paused"));
        assert_eq!(t(Toggle, (Rolling, 1), (Paused, 1)).as_deref(), Some("Paused"));
        assert_eq!(t(Forward, (Reading, 3), (Finished, 3)).as_deref(), Some("Finished"));
        assert_eq!(t(Forward, (Rolling, 0), (Reading, 0)).as_deref(), Some("Start now"));
        assert_eq!(t(Retake, (Reading, 2), (Rolling, 2)).as_deref(), Some("Retake · take 3"));
        assert_eq!(t(BackSentence, (Reading, 2), (Reading, 2)).as_deref(), Some("Sentence back"));
    }

    #[test]
    fn pause_from_holding_says_paused() {
        // Holding is listening but not running; pausing it must still show.
        assert_eq!(t(Pause, (Holding, 1), (Paused, 1)).as_deref(), Some("Paused"));
    }

    #[test]
    fn forward_from_holding_says_next_not_start() {
        assert_eq!(t(Forward, (Holding, 1), (Reading, 2)).as_deref(), Some("Next ›"));
        assert_eq!(t(Forward, (Paused, 1), (Reading, 2)).as_deref(), Some("Next ›"));
        assert_eq!(t(Forward, (Reading, 1), (Reading, 2)).as_deref(), Some("Next ›"));
    }

    #[test]
    fn no_op_has_no_toast() {
        assert_eq!(t(Play, (Reading, 1), (Reading, 1)), None);
        assert_eq!(t(Forward, (Rolling, 0), (Rolling, 0)), None);
        assert_eq!(t(Pause, (Paused, 1), (Paused, 1)), None);
    }
}
