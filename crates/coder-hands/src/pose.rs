//! Pose labels from 21 MediaPipe-style landmarks. Ported from
//! `OpenAgentsInc/commander` `src/components/hands/handPoseRecognition.ts`.

/// One 3D landmark in normalized image space (y grows down).
#[derive(Clone, Copy, Debug, Default)]
pub struct Landmark {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// MediaPipe Hands joint order.
pub const WRIST: usize = 0;
pub const THUMB_TIP: usize = 4;
pub const INDEX_TIP: usize = 8;
pub const INDEX_MCP: usize = 5;
pub const INDEX_PIP: usize = 6;
pub const MIDDLE_TIP: usize = 12;
pub const MIDDLE_MCP: usize = 9;
pub const MIDDLE_PIP: usize = 10;
pub const RING_TIP: usize = 16;
pub const RING_MCP: usize = 13;
pub const RING_PIP: usize = 14;
pub const PINKY_TIP: usize = 20;
pub const PINKY_MCP: usize = 17;
pub const PINKY_PIP: usize = 18;
pub const THUMB_MCP: usize = 2;
pub const THUMB_IP: usize = 3;

/// Bones drawn on the overlay, MediaPipe `HAND_CONNECTIONS`.
pub const CONNECTIONS: &[[usize; 2]] = &[
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 4],
    [0, 5],
    [5, 6],
    [6, 7],
    [7, 8],
    [0, 9],
    [9, 10],
    [10, 11],
    [11, 12],
    [0, 13],
    [13, 14],
    [14, 15],
    [15, 16],
    [0, 17],
    [17, 18],
    [18, 19],
    [19, 20],
    [5, 9],
    [9, 13],
    [13, 17],
];

/// Thumb-tip to index-tip distance under this fraction of the palm is a
/// pinch. The palm is the wrist to the index knuckle, so the rule reads
/// the same whether the hand is close to the camera or across the room.
///
/// Measured on 2,356 tracked readings from three recorded sessions on
/// 2026-09-18: the readings the earlier image-space rule called a pinch
/// sit under 0.35 of a palm 93% of the time and every other pose sits
/// under it 2% of the time, and no reading of a deliberately flat or open
/// hand falls under 0.38. Over the same recordings the palm ran from 0.22
/// of the frame at the fifth percentile to 0.35 at the ninety-fifth, so
/// the image-space bound of 0.1 it replaces asked for anything from 0.29
/// to 0.45 of a palm depending on how far away the hand was.
pub const PINCH_PALM: f32 = 0.35;
/// A curled finger's tip sits within this fraction of the wrist-to-knuckle
/// length of its knuckle.
const CURL_CLOSE: f32 = 0.7;
/// Or its tip sits this fraction of that length below its PIP joint.
const CURL_DROP: f32 = 0.1;
/// An extended finger's tip is this much farther from its knuckle than its
/// PIP joint is.
const STRAIGHT: f32 = 0.9;
/// A two-finger V spreads its tips past this fraction of the
/// wrist-to-index-knuckle length.
const V_SPREAD: f32 = 0.3;
/// An open hand's tip spread is past this multiple of its knuckle spread.
const OPEN_SPREAD: f32 = 1.6;
/// A flat hand's tip spread stays under this multiple of its knuckle
/// spread.
const FLAT_SPREAD: f32 = 1.7;

/// Commander pose labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HandPose {
    #[default]
    None,
    Fist,
    TwoFingerV,
    FlatHand,
    OpenHand,
    PinchClosed,
}

impl HandPose {
    /// Short status text for the HUD.
    pub fn label(self) -> &'static str {
        match self {
            HandPose::None => "none",
            HandPose::Fist => "fist",
            HandPose::TwoFingerV => "two-finger v",
            HandPose::FlatHand => "flat hand",
            HandPose::OpenHand => "open hand",
            HandPose::PinchClosed => "pinch",
        }
    }
}

/// The palm the pinch rule measures against: the wrist to the index
/// knuckle, in the image plane. `None` when the two land on each other,
/// which is a hand the tracker did not resolve.
///
/// Every palm-relative number the seam sends divides by this one, so a
/// ratio in the state and a constant in a rule mean the same thing
/// (`crate::judge::features`).
pub fn palm_span(landmarks: &[Landmark]) -> Option<f32> {
    if landmarks.len() < 21 {
        return None;
    }
    let wrist = landmarks[WRIST];
    let knuckle = landmarks[INDEX_MCP];
    let dx = wrist.x - knuckle.x;
    let dy = wrist.y - knuckle.y;
    let span = (dx * dx + dy * dy).sqrt();
    (span > f32::EPSILON).then_some(span)
}

/// The thumb-to-index gap as a fraction of the palm. `None` for a hand
/// with no palm to measure against.
pub fn pinch_ratio(landmarks: &[Landmark]) -> Option<f32> {
    if landmarks.len() < 21 {
        return None;
    }
    let palm = palm_span(landmarks)?;
    Some(dist(landmarks[THUMB_TIP], landmarks[INDEX_TIP]) / palm)
}

pub(crate) fn dist(a: Landmark, b: Landmark) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn finger_extended(tip: Landmark, pip: Landmark, mcp: Landmark) -> bool {
    let vertical = tip.y < pip.y && pip.y < mcp.y;
    let straight = dist(mcp, tip) > dist(mcp, pip) * STRAIGHT;
    vertical && straight
}

fn finger_curled(tip: Landmark, pip: Landmark, mcp: Landmark, wrist: Landmark) -> bool {
    let tip_lower = tip.y > pip.y;
    let reference = dist(wrist, mcp);
    tip_lower && (dist(tip, mcp) < reference * CURL_CLOSE || tip.y - pip.y > reference * CURL_DROP)
}

/// Classifies one hand. `landmarks` must hold 21 joints.
pub fn recognize(landmarks: &[Landmark]) -> HandPose {
    if landmarks.len() < 21 {
        return HandPose::None;
    }
    if pinch_ratio(landmarks).is_some_and(|ratio| ratio < PINCH_PALM) {
        return HandPose::PinchClosed;
    }
    let wrist = landmarks[WRIST];
    let fist = [INDEX_TIP, MIDDLE_TIP, RING_TIP, PINKY_TIP]
        .iter()
        .zip([INDEX_PIP, MIDDLE_PIP, RING_PIP, PINKY_PIP])
        .zip([INDEX_MCP, MIDDLE_MCP, RING_MCP, PINKY_MCP])
        .all(|((tip, pip), mcp)| {
            finger_curled(landmarks[*tip], landmarks[pip], landmarks[mcp], wrist)
        });
    if fist {
        return HandPose::Fist;
    }
    let index_ext = finger_extended(
        landmarks[INDEX_TIP],
        landmarks[INDEX_PIP],
        landmarks[INDEX_MCP],
    );
    let middle_ext = finger_extended(
        landmarks[MIDDLE_TIP],
        landmarks[MIDDLE_PIP],
        landmarks[MIDDLE_MCP],
    );
    let ring_curl = finger_curled(
        landmarks[RING_TIP],
        landmarks[RING_PIP],
        landmarks[RING_MCP],
        wrist,
    );
    let pinky_curl = finger_curled(
        landmarks[PINKY_TIP],
        landmarks[PINKY_PIP],
        landmarks[PINKY_MCP],
        wrist,
    );
    if index_ext && middle_ext && ring_curl && pinky_curl {
        let spread = dist(landmarks[INDEX_TIP], landmarks[MIDDLE_TIP]);
        if spread > dist(wrist, landmarks[INDEX_MCP]) * V_SPREAD {
            return HandPose::TwoFingerV;
        }
    }
    let all_ext = index_ext
        && middle_ext
        && finger_extended(
            landmarks[RING_TIP],
            landmarks[RING_PIP],
            landmarks[RING_MCP],
        )
        && finger_extended(
            landmarks[PINKY_TIP],
            landmarks[PINKY_PIP],
            landmarks[PINKY_MCP],
        )
        && finger_extended(
            landmarks[THUMB_TIP],
            landmarks[THUMB_IP],
            landmarks[THUMB_MCP],
        );
    if all_ext {
        let tip_spread = dist(landmarks[INDEX_TIP], landmarks[PINKY_TIP]);
        let mcp_spread = dist(landmarks[INDEX_MCP], landmarks[PINKY_MCP]);
        if tip_spread > mcp_spread * OPEN_SPREAD {
            return HandPose::OpenHand;
        }
        if tip_spread < mcp_spread * FLAT_SPREAD {
            return HandPose::FlatHand;
        }
    }
    HandPose::None
}

/// Every rule's distance from its own bound, in units of the bound's
/// constant. A margin is positive when the rule fires and negative when it
/// does not, and in either case the decisive clause is the one nearest its
/// bound, so a margin near zero is a decision the next frame could flip.
/// The gesture module reads these to tell a clear transition from an
/// ambiguous one (`docs/os/hands-judge.md`, "The trigger").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margins {
    /// The pinch rule's margin, in units of [`PINCH_PALM`]:
    /// `(PINCH_PALM - the gap in palms) / PINCH_PALM`.
    pub pinch: f32,
    /// The fist rule's margin.
    pub fist: f32,
    /// The two-finger V rule's margin.
    pub two_finger_v: f32,
    /// The open hand rule's margin.
    pub open_hand: f32,
    /// The flat hand rule's margin.
    pub flat_hand: f32,
}

impl Margins {
    /// The rule nearest its bound, by name and margin. `|`margin`|`
    /// small is the thin margin an ambiguous transition carries.
    pub fn thinnest(&self) -> (&'static str, f32) {
        [
            ("pinch", self.pinch),
            ("fist", self.fist),
            ("two_finger_v", self.two_finger_v),
            ("open_hand", self.open_hand),
            ("flat_hand", self.flat_hand),
        ]
        .into_iter()
        .min_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .unwrap_or(("none", 0.0))
    }
}

/// `a < bound` as a margin: positive while the bound holds, scaled by the
/// bound.
fn under(a: f32, bound: f32) -> f32 {
    (bound - a) / bound
}

/// `a > bound` as a margin.
fn over(a: f32, bound: f32) -> f32 {
    (a - bound) / bound
}

/// A rule's clauses folded to one margin: the weakest satisfied clause
/// when they all hold, else the failing clause nearest its bound — how
/// close the rule came to firing.
fn rule(clauses: &[f32]) -> f32 {
    if clauses.iter().all(|margin| *margin > 0.0) {
        clauses.iter().copied().fold(f32::INFINITY, f32::min)
    } else {
        clauses
            .iter()
            .copied()
            .filter(|margin| *margin <= 0.0)
            .fold(f32::NEG_INFINITY, f32::max)
    }
}

/// [`finger_extended`] as a margin. Its ordering clauses carry no
/// constant, so they normalize by the palm's wrist-to-knuckle length.
fn extended_margin(tip: Landmark, pip: Landmark, mcp: Landmark, palm: f32) -> f32 {
    let vertical = (pip.y - tip.y).min(mcp.y - pip.y) / palm;
    let straight = over(dist(mcp, tip), dist(mcp, pip) * STRAIGHT);
    vertical.min(straight)
}

/// [`finger_curled`] as a margin: the tip-lower ordering and the closer of
/// the two curl distances, all scaled by the wrist-to-knuckle length.
fn curled_margin(tip: Landmark, pip: Landmark, mcp: Landmark, wrist: Landmark) -> f32 {
    let reference = dist(wrist, mcp);
    let lower = (tip.y - pip.y) / reference;
    let close = under(dist(tip, mcp), reference * CURL_CLOSE);
    let drop = over(tip.y - pip.y, reference * CURL_DROP);
    lower.min(close.max(drop))
}

const FINGERS: [(usize, usize, usize); 4] = [
    (INDEX_TIP, INDEX_PIP, INDEX_MCP),
    (MIDDLE_TIP, MIDDLE_PIP, MIDDLE_MCP),
    (RING_TIP, RING_PIP, RING_MCP),
    (PINKY_TIP, PINKY_PIP, PINKY_MCP),
];

/// Every rule's margin over one hand's landmarks. `None` when the hand is
/// not a full 21 joints or collapses to a point, the conditions under
/// which [`recognize`] answers [`HandPose::None`] without reading a rule.
pub fn margins(landmarks: &[Landmark]) -> Option<Margins> {
    if landmarks.len() < 21 {
        return None;
    }
    let wrist = landmarks[WRIST];
    let palm = dist(wrist, landmarks[MIDDLE_MCP]);
    if palm == 0.0 {
        return None;
    }
    let curled: Vec<f32> = FINGERS
        .iter()
        .map(|&(tip, pip, mcp)| {
            curled_margin(landmarks[tip], landmarks[pip], landmarks[mcp], wrist)
        })
        .collect();
    let extended: Vec<f32> = FINGERS
        .iter()
        .map(|&(tip, pip, mcp)| {
            extended_margin(landmarks[tip], landmarks[pip], landmarks[mcp], palm)
        })
        .chain(std::iter::once(extended_margin(
            landmarks[THUMB_TIP],
            landmarks[THUMB_IP],
            landmarks[THUMB_MCP],
            palm,
        )))
        .collect();
    let spread = dist(landmarks[INDEX_TIP], landmarks[MIDDLE_TIP]);
    let tip_spread = dist(landmarks[INDEX_TIP], landmarks[PINKY_TIP]);
    let mcp_spread = dist(landmarks[INDEX_MCP], landmarks[PINKY_MCP]);
    Some(Margins {
        pinch: match pinch_ratio(landmarks) {
            Some(ratio) => under(ratio, PINCH_PALM),
            None => f32::NEG_INFINITY,
        },
        fist: rule(&curled),
        two_finger_v: rule(&[
            extended[0],
            extended[1],
            curled[2],
            curled[3],
            over(spread, dist(wrist, landmarks[INDEX_MCP]) * V_SPREAD),
        ]),
        open_hand: rule(
            &extended
                .iter()
                .copied()
                .chain(std::iter::once(over(tip_spread, mcp_spread * OPEN_SPREAD)))
                .collect::<Vec<_>>(),
        ),
        flat_hand: rule(
            &extended
                .iter()
                .copied()
                .chain(std::iter::once(under(tip_spread, mcp_spread * FLAT_SPREAD)))
                .collect::<Vec<_>>(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(x: f32, y: f32) -> Landmark {
        Landmark { x, y, z: 0.0 }
    }

    #[test]
    fn empty_landmarks_are_none() {
        assert_eq!(recognize(&[]), HandPose::None);
    }

    #[test]
    fn close_thumb_and_index_is_a_pinch() {
        let mut pts = [Landmark::default(); 21];
        pts[WRIST] = tip(0.5, 0.8);
        pts[INDEX_MCP] = tip(0.5, 0.6);
        pts[THUMB_TIP] = tip(0.4, 0.4);
        pts[INDEX_TIP] = tip(0.41, 0.41);
        assert_eq!(recognize(&pts), HandPose::PinchClosed);
    }

    /// The same gap reads the same at two distances, which is what the
    /// palm-relative rule buys over the image-space one it replaces.
    #[test]
    fn a_pinch_reads_the_same_near_and_far() {
        let near = curled_hand(0.03);
        let mut far = [Landmark::default(); 21];
        for (slot, point) in far.iter_mut().zip(near.iter()) {
            slot.x = 0.5 + (point.x - 0.5) * 0.4;
            slot.y = 0.5 + (point.y - 0.5) * 0.4;
        }
        assert_eq!(recognize(&near), HandPose::PinchClosed);
        assert_eq!(recognize(&far), HandPose::PinchClosed);
        let near = margins(&near).expect("21 joints have margins").pinch;
        let far = margins(&far).expect("21 joints have margins").pinch;
        assert!((near - far).abs() < 0.01, "near {near}, far {far}");
    }

    /// A hand the tracker collapsed onto one point has no palm to
    /// measure against, so no rule reads it as a pinch.
    #[test]
    fn a_hand_with_no_palm_is_not_a_pinch() {
        let pts = [Landmark::default(); 21];
        assert_eq!(recognize(&pts), HandPose::None);
    }

    /// A hand with its four fingers curled and the thumb tip `pinch`
    /// image units off the index tip. Its palm, wrist to index knuckle,
    /// is 0.202 image units, so a gap of `pinch` is `pinch / 0.202` of a
    /// palm and the pinch rule's margin is the one under test.
    fn curled_hand(pinch: f32) -> [Landmark; 21] {
        let mut pts = [Landmark::default(); 21];
        let put = |pts: &mut [Landmark; 21], joint: usize, x: f32, y: f32| {
            pts[joint] = Landmark { x, y, z: 0.0 };
        };
        put(&mut pts, WRIST, 0.5, 0.8);
        put(&mut pts, MIDDLE_MCP, 0.5, 0.6);
        for (at, (mcp, pip, tip)) in [
            (0.47, (INDEX_MCP, INDEX_PIP, INDEX_TIP)),
            (0.50, (MIDDLE_MCP, MIDDLE_PIP, MIDDLE_TIP)),
            (0.54, (RING_MCP, RING_PIP, RING_TIP)),
            (0.57, (PINKY_MCP, PINKY_PIP, PINKY_TIP)),
        ] {
            put(&mut pts, mcp, at, 0.60);
            put(&mut pts, pip, at, 0.62);
            put(&mut pts, tip, at, 0.72);
        }
        put(&mut pts, THUMB_MCP, 0.44, 0.68);
        put(&mut pts, THUMB_IP, 0.45, 0.66);
        pts[INDEX_TIP] = tip(0.47, 0.72);
        put(&mut pts, THUMB_TIP, 0.47 + pinch, 0.72);
        pts
    }

    #[test]
    fn a_clear_pinch_carries_a_wide_margin() {
        let margins = margins(&curled_hand(0.03)).expect("21 joints have margins");
        assert!((margins.pinch - 0.576).abs() < 0.01, "{}", margins.pinch);
        let (_, thinnest) = margins.thinnest();
        assert!(thinnest.abs() > 0.15, "thinnest {thinnest}");
    }

    #[test]
    fn a_borderline_pinch_carries_a_thin_margin() {
        let margins = margins(&curled_hand(0.078)).expect("21 joints have margins");
        assert!((margins.pinch + 0.103).abs() < 0.01, "{}", margins.pinch);
        let (rule, thinnest) = margins.thinnest();
        assert_eq!(rule, "pinch");
        assert!(thinnest.abs() < 0.15, "thinnest {thinnest}");
    }

    #[test]
    fn degenerate_landmarks_have_no_margins() {
        assert_eq!(margins(&[]), None);
        assert_eq!(margins(&[Landmark::default(); 21]), None);
    }
}
