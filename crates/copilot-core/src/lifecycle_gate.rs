//! Sidecar lifecycle gate (Chunk 3, pure part).
//!
//! Invariant 1: the analysis engine (`llama-server` or the llama-helper sidecar)
//! must never run while audio is being captured. This is the single rule the
//! supervisor consults before starting or keeping the process alive.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarAction {
    /// It is safe to start the sidecar.
    Start,
    /// Analysis finished or was cancelled: stop the sidecar.
    Stop,
    /// Capture is active: the sidecar must not be running.
    RefuseWhileCapturing,
}

/// Decide what the supervisor should do given the current capture state and
/// whether the sidecar is currently running.
pub fn supervise(capturing: bool, sidecar_running: bool) -> SidecarAction {
    if capturing {
        SidecarAction::RefuseWhileCapturing
    } else if sidecar_running {
        SidecarAction::Stop
    } else {
        SidecarAction::Start
    }
}

/// Whether the sidecar is permitted to be alive right now.
pub fn sidecar_allowed(capturing: bool) -> bool {
    !capturing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_is_never_allowed_during_capture() {
        assert!(!sidecar_allowed(true));
        assert!(sidecar_allowed(false));
    }

    #[test]
    fn capture_forces_refusal_even_if_running() {
        assert_eq!(supervise(true, true), SidecarAction::RefuseWhileCapturing);
        assert_eq!(supervise(true, false), SidecarAction::RefuseWhileCapturing);
    }

    #[test]
    fn idle_start_and_stop_transitions() {
        assert_eq!(supervise(false, false), SidecarAction::Start);
        assert_eq!(supervise(false, true), SidecarAction::Stop);
    }
}
