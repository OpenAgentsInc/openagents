//! Player character controller input resolution helpers.
//!
//! Pure, testable mapping from raw key/button states to controller intents and camera sway.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawButtons {
    pub w: bool,
    pub s: bool,
    pub a: bool,
    pub d: bool,
    pub q: bool,
    pub e: bool,
    pub lmb: bool,
    pub rmb: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolveParams {
    pub dt: f32,
    /// Camera swing turn speed (rad/s) when A or D is held exclusively.
    pub turn_speed_rad_per_s: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResolvedIntents {
    pub forward: bool,
    pub backward: bool,
    pub strafe_left: bool,
    pub strafe_right: bool,
    pub click_move_forward: bool,
    pub run: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolveOutput {
    pub intents: ResolvedIntents,
    /// Signed yaw delta (radians) to apply to the camera for this frame.
    pub cam_yaw_delta: f32,
}

/// Resolve raw button state into controller intents and a camera yaw delta.
///
/// Rules:
/// - Q/E are strafes: Q → right, E → left.
/// - A/D swing the camera when RMB is not held.
/// - When RMB is held, A/D become strafes (A → right, D → left).
/// - Shift (sprint) applies only while holding W without S/Q/E/A/D strafing flags.
/// - Click-move-forward requires LMB+RMB.
#[must_use]
pub fn resolve(raw: RawButtons, p: ResolveParams) -> ResolveOutput {
    let mut intents = ResolvedIntents {
        forward: raw.w,
        backward: raw.s,
        // Flipped strafes to match current renderer mapping (Q→right, E→left)
        strafe_right: raw.q,
        strafe_left: raw.e,
        click_move_forward: raw.lmb && raw.rmb,
        run: false,
    };

    // Under RMB, A/D become strafes (A→right, D→left) to match in-game expectation
    if raw.rmb {
        if raw.a {
            intents.strafe_right = true;
        }
        if raw.d {
            intents.strafe_left = true;
        }
    }

    // Sprint gating: only forward, without back or strafes; Shift held
    let strafing = intents.strafe_left || intents.strafe_right;
    intents.run = raw.shift && intents.forward && !intents.backward && !strafing;

    // Camera swing from A/D (exclusive) only when RMB is NOT held
    let mut cam_yaw_delta = 0.0f32;
    if !raw.rmb && (raw.a ^ raw.d) {
        let dir = if raw.a { 1.0 } else { -1.0 };
        cam_yaw_delta = dir * p.turn_speed_rad_per_s * p.dt;
    }

    ResolveOutput {
        intents,
        cam_yaw_delta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> ResolveParams {
        ResolveParams {
            dt: 0.1,
            turn_speed_rad_per_s: 180f32.to_radians(),
        }
    }

    #[test]
    fn sprint_requires_forward_only() {
        let mut raw = RawButtons {
            w: true,
            shift: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.intents.run);
        raw.q = true; // strafing cancels sprint
        let out = resolve(raw, params());
        assert!(!out.intents.run);
        raw.q = false;
        raw.s = true; // backpedal cancels sprint
        let out = resolve(raw, params());
        assert!(!out.intents.run);
    }

    #[test]
    fn qe_are_strafes() {
        let raw = RawButtons {
            q: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.intents.strafe_right && !out.intents.strafe_left);
        let raw = RawButtons {
            e: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.intents.strafe_left && !out.intents.strafe_right);
    }

    #[test]
    fn ad_swing_camera_only() {
        let mut raw = RawButtons {
            a: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.cam_yaw_delta > 0.0);
        assert_eq!(out.intents.strafe_left, false);
        assert_eq!(out.intents.strafe_right, false);

        raw = RawButtons {
            d: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.cam_yaw_delta < 0.0);
    }

    #[test]
    fn click_move_requires_both_buttons() {
        let raw = RawButtons {
            lmb: true,
            rmb: true,
            ..Default::default()
        };
        let out = resolve(raw, params());
        assert!(out.intents.click_move_forward);
        let raw = RawButtons {
            lmb: true,
            ..Default::default()
        };
        assert!(!resolve(raw, params()).intents.click_move_forward);
        let raw = RawButtons {
            rmb: true,
            ..Default::default()
        };
        assert!(!resolve(raw, params()).intents.click_move_forward);
    }
}
