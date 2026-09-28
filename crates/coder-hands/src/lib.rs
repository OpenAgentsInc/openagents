//! Hand landmarks for CoderOS.
//!
//! The crate holds what every reader and writer of hand landmarks shares:
//! the 21 MediaPipe-style joints and the pose labels in [`pose`], the
//! pinned landmark model and its digest check in [`model`], the ONNX
//! landmarker in [`landmarker`] on Linux under the `landmarker` feature,
//! and the JSON line the CoderOS
//! camera daemon publishes on its hands socket in [`wire`]. The camera
//! itself is not here: `crates/coderos-camera` reads the V4L2 node on
//! CoderOS.
//!
//! What a sequence of landmarks means is here too: [`gestures`] turns
//! frames into desk input, [`watch`] asks the seam about a window the
//! rules cannot settle, and [`socket`] says where the camera daemon
//! keeps its sockets. The compositor reads those three modules, and so
//! does `crates/coder-hands-measure`, which builds on macOS as well. The
//! crate depends on nothing that runs on one platform alone, so a
//! measurement of the rules runs where the person is.

pub mod model;
/// The pose rules and the constants they turn on.
pub mod pose;
pub mod wire;

/// The rules a tracked hand becomes desk input by, and the constants
/// each one turns on.
pub mod gestures;

/// Where the CoderOS camera daemon keeps its sockets, and the one verb a
/// reader asks it. Unix alone, because the sockets are Unix sockets.
#[cfg(unix)]
pub mod socket;

/// The Jev seam beside the pose rules, behind the `judge` feature so the
/// camera daemon does not build the client stack.
#[cfg(feature = "judge")]
pub mod judge;

/// The thread the seam's requests run on, so an ask never waits on a
/// frame. Behind the same feature, for the same reason.
#[cfg(feature = "judge")]
pub mod seam;

/// The seam beside the gesture rules: what a caller asks about a window
/// the rules cannot settle, and what it does with the answer. Behind the
/// same feature, for the same reason.
#[cfg(feature = "judge")]
pub mod watch;

#[cfg(all(target_os = "linux", feature = "landmarker"))]
pub mod landmarker;

/// The landmarker on a platform this crate does not run ONNX on, or in a
/// build without the `landmarker` feature, so a caller compiles the same
/// code everywhere and learns at load time.
#[cfg(not(all(target_os = "linux", feature = "landmarker")))]
pub mod landmarker {
    use crate::Frame;
    use std::path::Path;

    /// The side of the square the model reads.
    pub const NET: u32 = 224;

    pub struct Landmarker;

    impl Landmarker {
        pub fn load(path: &Path) -> Result<Landmarker, String> {
            Err(format!(
                "the ONNX landmarker at {} runs on Linux; this platform tracks hands another way",
                path.display()
            ))
        }

        pub fn infer(&mut self, _rgb: &[u8], _width: u32, _height: u32) -> Result<Frame, String> {
            Err("no landmarker on this platform".into())
        }
    }
}

pub use pose::{
    CONNECTIONS, HandPose, INDEX_MCP, INDEX_PIP, INDEX_TIP, Landmark, MIDDLE_MCP, MIDDLE_PIP,
    MIDDLE_TIP, Margins, PINKY_MCP, PINKY_PIP, PINKY_TIP, RING_MCP, RING_PIP, RING_TIP, THUMB_IP,
    THUMB_MCP, THUMB_TIP, WRIST, margins, recognize,
};

/// The words the tracker reports when another process holds the camera
/// node, which on CoderOS is the camera circle's player. The window shows
/// them in its title, so they name the key that releases the node.
pub fn camera_held(device: &str) -> String {
    format!("the camera circle holds {device}; Super+C releases it")
}

/// Whether a tracker status is the camera handoff, so the window can log
/// it as a warning rather than as one more reading.
pub fn is_camera_held(status: &str) -> bool {
    status.ends_with("; Super+C releases it")
}

/// Where a hand's pinch is, how far open it is as a fraction of the palm,
/// and the lower of the thumb and index tips' confidence.
pub type Grip = ((f32, f32), f32, f32);

/// One tracked hand.
#[derive(Clone, Debug)]
pub struct Hand {
    pub landmarks: [Landmark; 21],
    pub pose: HandPose,
}

/// Latest tracker reading.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub hands: Vec<Hand>,
    pub status: String,
}

impl Frame {
    /// Index fingertips of every live hand, normalized 0..1.
    pub fn index_tips(&self) -> Vec<(f32, f32)> {
        self.hands
            .iter()
            .filter_map(|hand| {
                let p = hand.landmarks[INDEX_TIP];
                if p.z < 0.1 { None } else { Some((p.x, p.y)) }
            })
            .collect()
    }

    /// One hand's grip: where the pinch is, how far the fingers are open
    /// as a fraction of the palm, and the lower of the two tips'
    /// confidence.
    pub fn pinch_grips(&self) -> Vec<Grip> {
        self.grips()
    }

    /// Every hand as the midpoint between its thumb and index tips, the
    /// distance between them as a fraction of the palm, and the lower of
    /// the two tips' confidence.
    ///
    /// The palm, wrist to index knuckle, is how far away the hand is, so
    /// the fraction reads the same whether a hand is close to the camera or
    /// across the room, where the raw distance does not. Measured on a
    /// recorded session on 2026-09-18: a pinch sits under 0.35 of a palm in
    /// 96% of readings and the rest of the poses sit above 0.42.
    fn grips(&self) -> Vec<Grip> {
        self.hands
            .iter()
            .filter_map(|hand| {
                let a = hand.landmarks[THUMB_TIP];
                let b = hand.landmarks[INDEX_TIP];
                let wrist = hand.landmarks[WRIST];
                let knuckle = hand.landmarks[INDEX_MCP];
                let palm = ((wrist.x - knuckle.x).powi(2) + (wrist.y - knuckle.y).powi(2)).sqrt();
                if palm <= f32::EPSILON {
                    return None;
                }
                let gap = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
                Some((
                    ((a.x + b.x) * 0.5, (a.y + b.y) * 0.5),
                    gap / palm,
                    a.z.min(b.z),
                ))
            })
            .collect()
    }

    /// Midpoints of every hand whose thumb and index sit within `bound` of
    /// each other, camera-normalized 0..1, with that distance beside each
    /// one.
    ///
    /// [`Self::pinch_points`] answers the pose rule, which is one
    /// threshold. A drag needs two: a hand grabs at the rule's distance and
    /// keeps its grip until the fingers open well past it, because fingers
    /// bounce while a hand moves and a single threshold drops the drag
    /// every time they do.
    pub fn pinch_holds(&self, bound: f32) -> Vec<((f32, f32), f32)> {
        self.hands
            .iter()
            .filter_map(|hand| {
                let a = hand.landmarks[THUMB_TIP];
                let b = hand.landmarks[INDEX_TIP];
                if a.z < 0.1 || b.z < 0.1 {
                    return None;
                }
                let dx = a.x - b.x;
                let dy = a.y - b.y;
                let dz = a.z - b.z;
                let gap = (dx * dx + dy * dy + dz * dz).sqrt();
                (gap <= bound).then_some((((a.x + b.x) * 0.5, (a.y + b.y) * 0.5), gap))
            })
            .collect()
    }

    /// Midpoints of closed pinches, camera-normalized 0..1.
    pub fn pinch_points(&self) -> Vec<(f32, f32)> {
        self.hands
            .iter()
            .filter(|hand| hand.pose == HandPose::PinchClosed)
            .filter_map(|hand| {
                let a = hand.landmarks[THUMB_TIP];
                let b = hand.landmarks[INDEX_TIP];
                if a.z < 0.1 || b.z < 0.1 {
                    None
                } else {
                    Some(((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_handoff_names_the_device_and_the_key() {
        let status = camera_held("/dev/video0");
        assert!(status.contains("/dev/video0"), "{status}");
        assert!(status.contains("Super+C"), "{status}");
        assert!(is_camera_held(&status));
        assert!(!is_camera_held("No hands detected"));
    }

    #[test]
    fn pinch_points_are_the_thumb_index_midpoint() {
        let mut landmarks = [Landmark::default(); 21];
        landmarks[THUMB_TIP] = Landmark {
            x: 0.2,
            y: 0.4,
            z: 1.0,
        };
        landmarks[INDEX_TIP] = Landmark {
            x: 0.4,
            y: 0.6,
            z: 1.0,
        };
        let frame = Frame {
            hands: vec![Hand {
                landmarks,
                pose: HandPose::PinchClosed,
            }],
            status: String::new(),
        };
        let pts = frame.pinch_points();
        assert_eq!(pts.len(), 1);
        assert!((pts[0].0 - 0.3).abs() < 1e-4);
        assert!((pts[0].1 - 0.5).abs() < 1e-4);
    }

    #[test]
    fn index_tips_skip_a_hand_with_no_depth() {
        let mut landmarks = [Landmark::default(); 21];
        landmarks[INDEX_TIP] = Landmark {
            x: 0.5,
            y: 0.5,
            z: 0.0,
        };
        let frame = Frame {
            hands: vec![Hand {
                landmarks,
                pose: HandPose::None,
            }],
            status: String::new(),
        };
        assert!(frame.index_tips().is_empty());
    }
}
