//! Made-up hands: 21 landmarks for a pose at a place on the page, as the
//! landmarker would report them. The tests replay sequences of these, and
//! the page's test hook feeds them in when there is no camera (a headless
//! browser has none), so every interaction can be checked without one.

use coder_hands::Landmark;

use crate::track::{Mapping, anchor};

/// A pose to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    /// The index finger out, the rest curled.
    Point,
    /// The thumb tip on the index tip.
    Pinch,
    /// The thumb and index a little apart: almost a pinch, not yet one.
    Near,
    /// Every finger curled.
    Fist,
    /// Every finger out and spread.
    Open,
}

impl Pose {
    pub fn parse(name: &str) -> Option<Pose> {
        Some(match name {
            "point" => Pose::Point,
            "pinch" => Pose::Pinch,
            "near" => Pose::Near,
            "fist" => Pose::Fist,
            "open" | "palm" => Pose::Open,
            _ => return None,
        })
    }
}

/// The palm's length (the wrist to the index knuckle) as a fraction of the
/// frame's height: a hand about half a metre from a laptop's camera.
pub const PALM: f32 = 0.2;

/// A curled finger from its knuckle: the PIP joint above it, the tip folded
/// back down next to the knuckle.
fn curled(mcp: (f32, f32)) -> [(f32, f32); 4] {
    [
        mcp,
        (mcp.0, mcp.1 - 0.35),
        (mcp.0 + 0.04, mcp.1 - 0.22),
        (mcp.0 + 0.03, mcp.1 - 0.05),
    ]
}

/// The pose in palm units, the wrist at the origin, `y` down, as the
/// unmirrored camera sees a right hand held up with its palm to it.
fn template(pose: Pose) -> [(f32, f32); 21] {
    let wrist = (0.0, 0.0);
    let thumb_out = [(-0.35, -0.15), (-0.6, -0.35), (-0.8, -0.55), (-0.95, -0.75)];
    let thumb_in = [
        (-0.3, -0.15),
        (-0.45, -0.35),
        (-0.35, -0.55),
        (-0.15, -0.65),
    ];
    let index_out = [(-0.3, -0.95), (-0.35, -1.35), (-0.38, -1.6), (-0.4, -1.85)];
    let middle_out = [(0.0, -1.0), (0.0, -1.45), (0.0, -1.72), (0.0, -1.98)];
    let ring_out = [(0.25, -0.92), (0.28, -1.3), (0.3, -1.55), (0.32, -1.78)];
    let pinky_out = [(0.48, -0.8), (0.55, -1.1), (0.6, -1.3), (0.63, -1.48)];
    let (thumb, index, middle, ring, pinky) = match pose {
        Pose::Point => (
            thumb_in,
            index_out,
            curled(middle_out[0]),
            curled(ring_out[0]),
            curled(pinky_out[0]),
        ),
        Pose::Pinch | Pose::Near => {
            let gap = if pose == Pose::Pinch { 0.04 } else { 0.7 };
            let tip = (-0.6, -1.05);
            (
                [
                    (-0.35, -0.15),
                    (-0.55, -0.4),
                    (-0.62, -0.7),
                    (tip.0 - gap * 0.3, tip.1 + gap),
                ],
                [(-0.3, -0.95), (-0.42, -1.3), (-0.55, -1.28), tip],
                middle_out,
                ring_out,
                pinky_out,
            )
        }
        Pose::Fist => (
            [(-0.3, -0.15), (-0.45, -0.35), (-0.35, -0.55), (-0.1, -0.6)],
            curled(index_out[0]),
            curled(middle_out[0]),
            curled(ring_out[0]),
            curled(pinky_out[0]),
        ),
        Pose::Open => (
            thumb_out,
            [(-0.3, -0.95), (-0.45, -1.33), (-0.55, -1.57), (-0.65, -1.8)],
            middle_out,
            [(0.25, -0.92), (0.38, -1.28), (0.47, -1.5), (0.55, -1.72)],
            [(0.48, -0.8), (0.68, -1.06), (0.82, -1.22), (0.95, -1.38)],
        ),
    };
    let mut out = [(0.0, 0.0); 21];
    out[0] = wrist;
    for (finger, joints) in [thumb, index, middle, ring, pinky].iter().enumerate() {
        for (j, p) in joints.iter().enumerate() {
            out[1 + finger * 4 + j] = *p;
        }
    }
    out
}

/// The pose with its cursor at `(px, py)`, fractions of the page, in a
/// frame `aspect` wide for each unit high. Coordinates are the frame's,
/// normalized and unmirrored, as the landmarker reports them.
pub fn hand(pose: Pose, px: f32, py: f32, aspect: f32, map: &Mapping) -> [Landmark; 21] {
    // In "aspect space": x scaled by the frame's aspect so both axes share
    // one unit, which is what the pose rules read.
    let t = template(pose);
    let mut lm = [Landmark::default(); 21];
    for (i, (x, y)) in t.iter().enumerate() {
        lm[i] = Landmark {
            x: x * PALM,
            y: y * PALM,
            z: 0.0,
        };
    }
    let fist = if pose == Pose::Fist { 1.0 } else { 0.0 };
    let (ax, ay) = anchor(&lm, fist);
    let (ix, iy) = map.to_frame(px, py);
    for p in lm.iter_mut() {
        p.x = (p.x - ax) / aspect + ix;
        p.y = p.y - ay + iy;
    }
    lm
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_hands::{HandPose, recognize};

    fn aspect_space(lm: &[Landmark; 21], aspect: f32) -> Vec<Landmark> {
        lm.iter()
            .map(|p| Landmark {
                x: p.x * aspect,
                ..*p
            })
            .collect()
    }

    #[test]
    fn coder_hands_reads_each_pose_as_meant() {
        let map = Mapping::default();
        let aspect = 4.0 / 3.0;
        let read = |pose| recognize(&aspect_space(&hand(pose, 0.5, 0.5, aspect, &map), aspect));
        assert_eq!(read(Pose::Pinch), HandPose::PinchClosed);
        assert_eq!(read(Pose::Fist), HandPose::Fist);
        assert_eq!(read(Pose::Open), HandPose::OpenHand);
        assert_ne!(read(Pose::Near), HandPose::PinchClosed);
        assert!(matches!(read(Pose::Point), HandPose::None));
    }
}
