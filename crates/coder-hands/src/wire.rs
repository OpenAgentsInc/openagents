//! The line the camera daemon publishes for every frame it tracks.
//!
//! `crates/coderos-camera` writes one JSON object a line on
//! `$XDG_RUNTIME_DIR/coderos-camera/hands.sock` while hand tracking is on:
//! the frame's timestamp, every hand as its 21 landmarks and its pose
//! label, and the tracker's status. A reader parses each line with
//! [`Line::parse`], and a writer renders one with [`Line::render`]. The
//! shape is the one thing a reader and the daemon agree on, so it is here
//! rather than in either binary.

use crate::{Frame, Hand, HandPose, Landmark};
use serde::{Deserialize, Serialize};

/// One landmark on the wire, normalized to the frame with `y` growing
/// down, as [`Landmark`] holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Landmark> for Point {
    fn from(l: Landmark) -> Point {
        Point {
            x: l.x,
            y: l.y,
            z: l.z,
        }
    }
}

impl From<Point> for Landmark {
    fn from(p: Point) -> Landmark {
        Landmark {
            x: p.x,
            y: p.y,
            z: p.z,
        }
    }
}

/// One hand on the wire: its 21 landmarks in MediaPipe order and the
/// pose label [`HandPose::label`] gives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireHand {
    pub landmarks: Vec<Point>,
    pub pose: String,
}

/// One line on the hands socket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// Seconds since the Unix epoch when the frame was captured.
    pub timestamp: f64,
    /// Every hand in the frame, none when the tracker saw no hand.
    pub hands: Vec<WireHand>,
    /// The tracker's status, such as `1 hand(s) detected`.
    pub status: String,
    /// Frames the daemon's hands lane dropped between the last line and
    /// this one, so a reader knows when it fell behind. A line a
    /// daemon wrote before the field reads as none dropped.
    #[serde(default)]
    pub dropped: u64,
}

impl Line {
    /// The line for one tracker reading at one moment.
    pub fn from_frame(frame: &Frame, timestamp: f64) -> Line {
        Line {
            timestamp,
            hands: frame
                .hands
                .iter()
                .map(|hand| WireHand {
                    landmarks: hand.landmarks.iter().copied().map(Point::from).collect(),
                    pose: hand.pose.label().to_string(),
                })
                .collect(),
            status: frame.status.clone(),
            dropped: 0,
        }
    }

    /// The line as the socket carries it, with its newline.
    pub fn render(&self) -> String {
        let mut text = serde_json::to_string(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Reads one line off the socket.
    pub fn parse(text: &str) -> Result<Line, String> {
        serde_json::from_str(text.trim()).map_err(|err| format!("hands line: {err}"))
    }

    /// The line back as a tracker frame: each hand's landmarks in the
    /// array the tracker reports and its pose label back as the enum. A
    /// hand whose landmarks are not the 21 the tracker reports is left
    /// out, because a partial hand would pose wrong and point wrong.
    pub fn frame(&self) -> Frame {
        let hands = self
            .hands
            .iter()
            .filter_map(|hand| {
                let landmarks: [Landmark; 21] = hand
                    .landmarks
                    .iter()
                    .copied()
                    .map(Landmark::from)
                    .collect::<Vec<_>>()
                    .try_into()
                    .ok()?;
                Some(Hand {
                    landmarks,
                    pose: Line::pose_of(hand),
                })
            })
            .collect();
        Frame {
            hands,
            status: self.status.clone(),
        }
    }

    /// The pose label of a hand, back as the enum.
    pub fn pose_of(hand: &WireHand) -> HandPose {
        [
            HandPose::Fist,
            HandPose::TwoFingerV,
            HandPose::FlatHand,
            HandPose::OpenHand,
            HandPose::PinchClosed,
        ]
        .into_iter()
        .find(|pose| pose.label() == hand.pose)
        .unwrap_or(HandPose::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hand, INDEX_TIP, THUMB_TIP};

    fn one_hand() -> Frame {
        let mut landmarks = [Landmark::default(); 21];
        landmarks[THUMB_TIP] = Landmark {
            x: 0.25,
            y: 0.5,
            z: 1.0,
        };
        landmarks[INDEX_TIP] = Landmark {
            x: 0.26,
            y: 0.5,
            z: 1.0,
        };
        Frame {
            hands: vec![Hand {
                landmarks,
                pose: HandPose::PinchClosed,
            }],
            status: "1 hand(s) detected".into(),
        }
    }

    #[test]
    fn a_line_carries_the_stamp_every_landmark_and_the_pose_label() {
        let line = Line::from_frame(&one_hand(), 1_789_000_000.25);
        let text = line.render();
        assert!(text.ends_with('\n'), "{text}");
        assert_eq!(text.matches('\n').count(), 1, "one line: {text}");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["timestamp"], 1_789_000_000.25);
        assert_eq!(value["status"], "1 hand(s) detected");
        assert_eq!(value["hands"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            value["hands"][0]["landmarks"].as_array().map(Vec::len),
            Some(21)
        );
        assert_eq!(value["hands"][0]["pose"], "pinch");
        assert_eq!(value["hands"][0]["landmarks"][4]["x"], 0.25);
    }

    #[test]
    fn a_line_parses_back_to_what_was_rendered() {
        let line = Line::from_frame(&one_hand(), 12.5);
        let back = Line::parse(&line.render()).expect("parses");
        assert_eq!(back, line);
        assert_eq!(Line::pose_of(&back.hands[0]), HandPose::PinchClosed);
    }

    #[test]
    fn no_hands_is_an_empty_array_with_the_status() {
        let frame = Frame {
            hands: Vec::new(),
            status: "No hands detected".into(),
        };
        let text = Line::from_frame(&frame, 0.0).render();
        assert!(text.contains("\"hands\":[]"), "{text}");
        assert!(text.contains("No hands detected"), "{text}");
    }

    #[test]
    fn an_unknown_pose_label_reads_as_none() {
        let hand = WireHand {
            landmarks: Vec::new(),
            pose: "salute".into(),
        };
        assert_eq!(Line::pose_of(&hand), HandPose::None);
    }

    #[test]
    fn a_broken_line_names_itself() {
        let err = Line::parse("{").unwrap_err();
        assert!(err.starts_with("hands line: "), "{err}");
    }

    #[test]
    fn a_line_reads_back_as_the_frame_it_carried() {
        let line = Line::parse(&Line::from_frame(&one_hand(), 12.5).render()).expect("parses");
        let frame = line.frame();
        assert_eq!(frame.status, "1 hand(s) detected");
        assert_eq!(frame.hands.len(), 1);
        assert_eq!(frame.hands[0].pose, HandPose::PinchClosed);
        let tip = frame.hands[0].landmarks[INDEX_TIP];
        assert_eq!((tip.x, tip.y, tip.z), (0.26, 0.5, 1.0));
    }

    #[test]
    fn a_hand_with_the_wrong_landmark_count_is_left_out() {
        let line = Line {
            timestamp: 0.0,
            hands: vec![WireHand {
                landmarks: vec![Point::default(); 5],
                pose: "pinch".into(),
            }],
            status: "1 hand(s) detected".into(),
            dropped: 0,
        };
        let frame = line.frame();
        assert!(frame.hands.is_empty());
    }

    #[test]
    fn the_drop_count_renders_and_a_line_without_it_reads_as_none() {
        let mut line = Line::from_frame(&one_hand(), 12.5);
        line.dropped = 7;
        let back = Line::parse(&line.render()).expect("parses");
        assert_eq!(back.dropped, 7);
        let old = Line::parse("{\"timestamp\":1.0,\"hands\":[],\"status\":\"No hands detected\"}")
            .expect("an older line still parses");
        assert_eq!(old.dropped, 0);
    }
}
