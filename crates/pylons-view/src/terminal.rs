//! The shared computer in front of the pylons, with a short approach range.

pub const ENTER_RANGE: f64 = 3.0;

/// The model a terminal turn may ask for by name: DeepSeek V4.1 Flash,
/// served by a model group of pylons (#12403). The site sends it as
/// `X-OpenAgents-Pylon-Model` (#12416).
pub const GROUP_MODEL: &str = "deepseek-v4.1-flash";
pub const FLINT_MODEL: &str = "flint";

/// Whether the snapshot contains a live complete Flint model.
#[must_use]
pub fn flint_ready(snapshot: &crate::Snapshot) -> bool {
    snapshot.pylons.iter().any(|p| {
        matches!(p.status.as_str(), "online" | "draining")
            && p.group.is_none()
            && p.services
                .iter()
                .any(|s| s.model.split('@').next() == Some(FLINT_MODEL))
    })
}

/// Whether `snapshot` holds an online, ready group head serving
/// [`GROUP_MODEL`]: only then does the terminal's chat offer it.
#[must_use]
pub fn group_ready(snapshot: &crate::Snapshot) -> bool {
    snapshot.pylons.iter().any(|pylon| {
        pylon.status == "online"
            && pylon
                .group
                .as_ref()
                .is_some_and(|g| g.role == "head" && g.state.as_deref() == Some("ready"))
            && pylon
                .services
                .iter()
                .any(|s| s.model.split('@').next() == Some(GROUP_MODEL))
    })
}
pub const LEAVE_RANGE: f64 = 3.8;

/// Centered on the approach from spawn toward the pylons.
pub const POSITION: [f64; 3] = [0.0, 0.0, 4.0];

/// A little hysteresis keeps the composer steady at the edge of reach.
#[must_use]
pub fn in_reach(player: [f64; 3], already_near: bool) -> bool {
    let at = POSITION;
    let range = if already_near {
        LEAVE_RANGE
    } else {
        ENTER_RANGE
    };
    player.iter().all(|v| v.is_finite())
        && (player[1] - at[1]).abs() <= 2.0
        && (player[0] - at[0]).hypot(player[2] - at[2]) <= range
}

/// Solid boxes for the pedestal, monitor, display, and keyboard. Every
/// corner is in world space: no face turns to follow the camera.
#[must_use]
pub fn faces() -> Vec<[[f64; 3]; 4]> {
    let at = POSITION;
    let mut out = Vec::with_capacity(30);
    for (low, high) in [
        ([-0.55, 0.0, -0.35], [0.55, 0.15, 0.35]),
        ([-0.14, 0.15, -0.10], [0.14, 1.25, 0.10]),
        ([-0.88, 1.25, -0.10], [0.88, 2.3, 0.25]),
        ([-0.76, 1.37, -0.12], [0.76, 2.18, -0.10]),
        ([-0.72, 1.05, -0.72], [0.72, 1.15, -0.16]),
    ] {
        let corners: [[f64; 3]; 8] = std::array::from_fn(|i| {
            std::array::from_fn(|axis| {
                at[axis]
                    + if i & (1 << axis) == 0 {
                        low[axis]
                    } else {
                        high[axis]
                    }
            })
        });
        for indices in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            out.push(indices.map(|i| corners[i]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn the_terminal_offers_v41_only_while_a_ready_group_serves_it() {
        let snapshot: crate::Snapshot = serde_json::from_value(serde_json::json!({
            "now": 10, "synced": true, "pylons": [{
                "id": "30200:ab:v41-head", "name": "V4.1", "slug": "v41-head", "npub": "npub1x",
                "status": "online", "family": "gpu", "memory_gb": 96, "busy": 0, "total": 1,
                "since": 1, "observed_at": 9, "expires_at": 200,
                "services": [{"model": "deepseek-v4.1-flash@sha256:ab", "kinds": ["Chat"]}],
                "free": true, "pools": [],
                "group": {"id": "ab:v41", "role": "head", "parts": 4, "state": "ready"}
            }]
        }))
        .unwrap();
        assert!(group_ready(&snapshot));
        let mut flint = snapshot.clone();
        flint.pylons[0].group = None;
        flint.pylons[0].services[0].model = "flint@sha256:ab".into();
        assert!(flint_ready(&flint));
        flint.pylons[0].status = "draining".into();
        assert!(flint_ready(&flint));
        flint.pylons[0].status = "offline".into();
        assert!(!flint_ready(&flint));
        let mut starting = snapshot.clone();
        starting.pylons[0].group.as_mut().unwrap().state = Some("starting".into());
        assert!(!group_ready(&starting));
        let mut other = snapshot.clone();
        other.pylons[0].services[0].model = "clef-flash".into();
        assert!(!group_ready(&other));
    }

    #[test]
    fn approach_has_hysteresis_and_requires_the_same_floor() {
        assert!(!in_reach([0.0, 0.0, -10.0], false));
        assert!(in_reach([0.0, 0.0, 1.0], false));
        assert!(!in_reach([0.0, 0.0, 0.5], false));
        assert!(in_reach([0.0, 0.0, 0.5], true));
        assert!(!in_reach([0.0, 0.0, 0.0], true));
        assert!(!in_reach([0.0, 5.0, 4.0], true));
        assert!(!in_reach([f64::NAN, 0.0, 4.0], true));
        assert!(!in_reach([9.0, 0.0, 4.0], true));
    }
}
