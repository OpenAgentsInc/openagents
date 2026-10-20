//! The shared computer in front of the pylons, with a short approach range.

pub const ENTER_RANGE: f64 = 3.0;
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
    use super::*;

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
