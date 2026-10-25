//! Spoken or typed Haunted House requests turned into bounded scene edits.
//!
//! The common phrases resolve here with no network: "add three pumpkins to
//! my left", "put a huge ghost in front of me", "remove it", "undo". The
//! rest goes to one decision request on the website
//! (`POST /v1/verse/house/request`), whose typed answer comes back through
//! [`Intent`] and [`edits`] as well, so positions are always computed in
//! code from the player's pose and only listed haunted-house assets appear.

use serde::{Deserialize, Serialize};

use crate::{Checkpoint, CreatorAsset, Edit};

/// The most props one request adds; the engine accepts eight changes per group.
pub const MAX_COUNT: usize = 8;
/// Where requests that need a decision go.
pub const ENDPOINT: &str = "/v1/verse/house/request";
/// The longest request text.
pub const MAX_TEXT: usize = 1200;
/// Only the house's own props. Shared fantasy and town props stay out of the
/// Haunted House request path.
pub const HOUSE_ASSET_PREFIX: &str = "custom:haunt:";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Ahead,
    Left,
    Right,
    Behind,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    Small,
    Normal,
    Large,
    Huge,
}
impl Size {
    /// The original Haunted House editor's scales.
    pub fn scale(self) -> f64 {
        match self {
            Self::Small => 0.6,
            Self::Normal => 1.0,
            Self::Large => 1.8,
            Self::Huge => 2.5,
        }
    }
}

/// One understood request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Intent {
    Add {
        asset: String,
        count: usize,
        size: Size,
        placement: Placement,
    },
    RemoveSelected,
    Undo,
    Redo,
}

/// The player's standing point on the ground and facing angle (radians;
/// forward is `(sin yaw, 0, -cos yaw)`, right is `(cos yaw, 0, sin yaw)`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pose {
    pub position: [f64; 3],
    pub yaw: f64,
}
impl Pose {
    pub fn from_checkpoint(c: &Checkpoint) -> Self {
        let (dx, dz) = (c.target[0] - c.eye[0], c.target[2] - c.eye[2]);
        let yaw = if dx.abs() + dz.abs() < 1e-9 {
            0.0
        } else {
            dx.atan2(-dz)
        };
        Self {
            position: [c.eye[0], 0.0, c.eye[2]],
            yaw,
        }
    }
    pub fn valid(&self) -> bool {
        self.yaw.is_finite()
            && self
                .position
                .iter()
                .all(|n| n.is_finite() && n.abs() <= 100_000.0)
    }
}

/// The house's own props from the page's creator asset list.
pub fn house_assets(assets: &[CreatorAsset]) -> Vec<CreatorAsset> {
    assets
        .iter()
        .filter(|a| a.valid() && a.id.starts_with(HOUSE_ASSET_PREFIX))
        .cloned()
        .collect()
}

const COUNTS: [&str; 9] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight",
];

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// "pumpkins" to "pumpkin", "werewolves" to "werewolf".
fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ves") {
        return format!("{stem}f");
    }
    if word.ends_with("ss") {
        return word.into();
    }
    word.strip_suffix('s').unwrap_or(word).into()
}

fn asset_named(noun: &str, assets: &[CreatorAsset]) -> Option<String> {
    let noun = singular(noun);
    let alias = match noun.as_str() {
        "jack-o-lantern" | "jack" => "pumpkin",
        "tombstone" | "headstone" | "grave" => "gravestone",
        "casket" => "coffin",
        "wolf" => "werewolf",
        "spirit" | "phantom" | "spook" => "ghost",
        other => other,
    };
    assets
        .iter()
        .find(|a| a.name.to_lowercase() == alias || a.id.rsplit(':').next() == Some(alias))
        .map(|a| a.id.clone())
}

fn size_word(word: &str) -> Option<Size> {
    Some(match word {
        "small" | "little" | "tiny" | "mini" => Size::Small,
        "normal" | "medium" | "regular" => Size::Normal,
        "large" | "big" | "bigger" => Size::Large,
        "huge" | "giant" | "enormous" | "gigantic" | "massive" => Size::Huge,
        _ => return None,
    })
}

fn placement(rest: &[String]) -> Option<Placement> {
    let rest: Vec<&str> = rest
        .iter()
        .map(String::as_str)
        .filter(|w| {
            ![
                "to", "on", "at", "the", "my", "your", "of", "me", "you", "please", "side", "hand",
            ]
            .contains(w)
        })
        .collect();
    match rest.as_slice() {
        [] | ["here"] | ["ahead"] | ["in", "front"] | ["front"] | ["forward"] => {
            Some(Placement::Ahead)
        }
        ["left"] => Some(Placement::Left),
        ["right"] => Some(Placement::Right),
        ["behind"] | ["back"] | ["in", "back"] => Some(Placement::Behind),
        _ => None,
    }
}

/// The number of props `text` asks for: the first count word or number
/// before the noun, one when none is written, `None` when it is out of range.
pub fn count_in(text: &str) -> Option<usize> {
    for word in words(text) {
        if let Some(n) = COUNTS.iter().position(|c| *c == word) {
            return (1..=MAX_COUNT).contains(&n).then_some(n);
        }
        if let Ok(n) = word.parse::<usize>() {
            return (1..=MAX_COUNT).contains(&n).then_some(n);
        }
        if ["couple", "pair"].contains(&word.as_str()) {
            return Some(2);
        }
        if ["several", "some", "few"].contains(&word.as_str()) {
            return Some(3);
        }
        if [
            "nine", "ten", "eleven", "twelve", "twenty", "hundred", "thousand", "dozen",
        ]
        .contains(&word.as_str())
        {
            return None;
        }
    }
    Some(1)
}

/// The closed phrases the house understands without a network call.
pub fn parse(text: &str, assets: &[CreatorAsset]) -> Option<Intent> {
    if text.len() > MAX_TEXT {
        return None;
    }
    let mut w = words(text);
    // Politeness carries no instruction.
    while let Some(first) = w.first().map(String::as_str) {
        if [
            "please", "can", "could", "would", "will", "you", "hey", "ok", "okay", "now", "just",
        ]
        .contains(&first)
        {
            w.remove(0);
        } else {
            break;
        }
    }
    while w.last().is_some_and(|l| l == "please" || l == "now") {
        w.pop();
    }
    let joined = w.join(" ");
    match joined.as_str() {
        "undo"
        | "undo that"
        | "undo it"
        | "undo last change"
        | "undo the last change"
        | "go back" => return Some(Intent::Undo),
        "redo" | "redo that" | "redo it" => return Some(Intent::Redo),
        "remove it"
        | "remove this"
        | "remove that"
        | "delete it"
        | "delete this"
        | "delete that"
        | "remove the selected object"
        | "delete the selected object"
        | "remove selection"
        | "delete selection" => return Some(Intent::RemoveSelected),
        _ => {}
    }
    let mut i = 0;
    let verb_len = match w.as_slice() {
        [a, b, ..] if a == "give" && b == "me" => 2,
        [a, ..] if ["add", "put", "place", "spawn", "create", "make"].contains(&a.as_str()) => 1,
        _ => return None,
    };
    i += verb_len;
    let mut count = 1;
    let mut counted = false;
    if let Some(word) = w.get(i) {
        if let Some(n) = COUNTS.iter().position(|c| c == word) {
            count = n;
            counted = true;
            i += 1;
        } else if let Ok(n) = word.parse::<usize>() {
            count = n;
            counted = true;
            i += 1;
        } else if ["a", "an"].contains(&word.as_str()) {
            i += 1;
        } else if word == "some" || word == "few" || word == "several" {
            count = 3;
            counted = true;
            i += 1;
        }
    }
    if w.get(i).is_some_and(|x| x == "few") {
        count = 3;
        i += 1;
    }
    let mut size = Size::Normal;
    while let Some(word) = w.get(i) {
        if let Some(s) = size_word(word) {
            size = s;
            i += 1;
        } else if [
            "very", "really", "super", "spooky", "scary", "orange", "more",
        ]
        .contains(&word.as_str())
        {
            i += 1;
        } else {
            break;
        }
    }
    let noun = w.get(i)?;
    let asset = asset_named(noun, assets)?;
    // "add pumpkins" names no count; the decision service settles that.
    if !counted && singular(noun) != *noun {
        return None;
    }
    if !(1..=MAX_COUNT).contains(&count) {
        return None;
    }
    let placement = placement(&w[i + 1..])?;
    Some(Intent::Add {
        asset,
        count,
        size,
        placement,
    })
}

/// The scene edits for `intent` at `pose`. `selected` is the selected object.
pub fn edits(
    intent: &Intent,
    pose: &Pose,
    selected: Option<&str>,
) -> Result<Vec<Edit>, &'static str> {
    match intent {
        Intent::Add {
            asset,
            count,
            size,
            placement,
        } => {
            if !(1..=MAX_COUNT).contains(count) {
                return Err("Add between 1 and 8 props at a time.");
            }
            if !pose.valid() {
                return Err("Walk into the house, then try again.");
            }
            let scale = size.scale();
            let spacing = (scale * 0.8 + 0.4).max(1.0);
            let (s, c) = pose.yaw.sin_cos();
            let forward = [s, -c];
            let right = [c, s];
            // The group's first prop sits beside the player, the rest extend outward.
            let (along, out, start) = match placement {
                Placement::Left => (forward, [-right[0], -right[1]], 1.4 + scale * 0.4),
                Placement::Right => (forward, right, 1.4 + scale * 0.4),
                Placement::Ahead => (right, forward, 2.0 + scale * 0.4),
                Placement::Behind => (right, [-forward[0], -forward[1]], 2.0 + scale * 0.4),
            };
            let columns = (*count).min(4);
            Ok((0..*count)
                .map(|n| {
                    let row = (n / columns) as f64;
                    let column = (n % columns) as f64;
                    let across = (column - (columns as f64 - 1.0) / 2.0) * spacing;
                    let depth = start + row * spacing;
                    let x = pose.position[0] + out[0] * depth + along[0] * across;
                    let z = pose.position[2] + out[1] * depth + along[1] * across;
                    Edit::Place {
                        asset: asset.clone(),
                        position: [round(x), pose.position[1], round(z)],
                        scale: (scale != 1.0).then_some(scale),
                    }
                })
                .collect())
        }
        Intent::RemoveSelected => selected
            .map(|target| {
                vec![Edit::Remove {
                    target: target.into(),
                }]
            })
            .ok_or("Select an object first, then say remove it."),
        Intent::Undo | Intent::Redo => Ok(Vec::new()),
    }
}

fn round(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

/// What `POST /v1/verse/house/request` takes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HouseRequest {
    pub text: String,
    pub assets: Vec<CreatorAsset>,
    pub player: Pose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
}
impl HouseRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.text.trim().is_empty() || self.text.len() > MAX_TEXT {
            return Err("Describe one change in a short request.");
        }
        if self.assets.len() > 256 || self.assets.iter().any(|a| !a.valid()) {
            return Err("Reload the house and try again.");
        }
        if !self.player.valid() {
            return Err("Walk into the house, then try again.");
        }
        if self
            .selected
            .as_deref()
            .is_some_and(|s| !crate::protocol::id(s))
        {
            return Err("Select the object again.");
        }
        Ok(())
    }
}

/// One edit in the endpoint's answer.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum HouseEdit {
    Place {
        asset: String,
        position: [f64; 3],
        yaw: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<f64>,
    },
    Remove {
        target: String,
    },
    Undo,
    Redo,
}

/// What `POST /v1/verse/house/request` answers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HouseAnswer {
    pub edits: Vec<HouseEdit>,
    pub say: String,
}

/// The answer for an understood request.
pub fn answer(
    intent: &Intent,
    pose: &Pose,
    selected: Option<&str>,
) -> Result<HouseAnswer, &'static str> {
    let edits = match intent {
        Intent::Undo => vec![HouseEdit::Undo],
        Intent::Redo => vec![HouseEdit::Redo],
        _ => edits(intent, pose, selected)?
            .into_iter()
            .map(|e| match e {
                Edit::Place {
                    asset,
                    position,
                    scale,
                } => HouseEdit::Place {
                    asset,
                    position,
                    yaw: pose.yaw,
                    scale,
                },
                Edit::Remove { target } => HouseEdit::Remove { target },
                Edit::Move { target, .. } | Edit::Material { target, .. } => {
                    HouseEdit::Remove { target }
                }
            })
            .collect(),
    };
    Ok(HouseAnswer {
        say: describe(intent),
        edits,
    })
}

/// A short spoken confirmation.
pub fn describe(intent: &Intent) -> String {
    match intent {
        Intent::Add {
            asset,
            count,
            size,
            placement,
        } => {
            let name = asset
                .rsplit(':')
                .next()
                .unwrap_or("prop")
                .replace(['_', '-'], " ");
            let size = match size {
                Size::Normal => "",
                Size::Small => "small ",
                Size::Large => "large ",
                Size::Huge => "huge ",
            };
            let place = match placement {
                Placement::Ahead => "in front of you",
                Placement::Left => "to your left",
                Placement::Right => "to your right",
                Placement::Behind => "behind you",
            };
            let count_word = COUNTS.get(*count).copied().unwrap_or("some");
            if *count == 1 {
                format!("Added a {size}{name} {place}.")
            } else {
                format!("Added {count_word} {size}{name}s {place}.")
            }
        }
        Intent::RemoveSelected => "Removed it.".into(),
        Intent::Undo => "Undone.".into(),
        Intent::Redo => "Redone.".into(),
    }
}

/// What the house says when a request isn't understood.
pub const NOT_UNDERSTOOD: &str = "Couldn't do that. Try 'add three pumpkins to my left'.";

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Vec<CreatorAsset> {
        [
            ("quaternius:fantasy-standard:barrel", "Barrel"),
            ("quaternius:fantasy-standard:crate_wooden", "Crate Wooden"),
            ("custom:haunt:werewolf", "Werewolf"),
            ("custom:haunt:ghost", "Ghost"),
            ("custom:haunt:pumpkin", "Pumpkin"),
            ("custom:haunt:coffin", "Coffin"),
            ("custom:haunt:gravestone", "Gravestone"),
        ]
        .into_iter()
        .map(|(id, name)| CreatorAsset {
            id: id.into(),
            name: name.into(),
        })
        .collect::<Vec<_>>()
    }
    fn house() -> Vec<CreatorAsset> {
        house_assets(&assets())
    }
    fn add(asset: &str, count: usize, size: Size, placement: Placement) -> Intent {
        Intent::Add {
            asset: format!("custom:haunt:{asset}"),
            count,
            size,
            placement,
        }
    }

    #[test]
    fn common_phrases_resolve_locally() {
        let a = house();
        assert_eq!(
            parse("add three pumpkins to my left", &a),
            Some(add("pumpkin", 3, Size::Normal, Placement::Left))
        );
        assert_eq!(
            parse("Add 5 huge pumpkins to my left.", &a),
            Some(add("pumpkin", 5, Size::Huge, Placement::Left))
        );
        assert_eq!(
            parse("please put a ghost in front of me", &a),
            Some(add("ghost", 1, Size::Normal, Placement::Ahead))
        );
        assert_eq!(
            parse("place two tiny coffins on my right", &a),
            Some(add("coffin", 2, Size::Small, Placement::Right))
        );
        assert_eq!(
            parse("Could you add a gravestone behind me?", &a),
            Some(add("gravestone", 1, Size::Normal, Placement::Behind))
        );
        assert_eq!(
            parse("spawn four werewolves", &a),
            Some(add("werewolf", 4, Size::Normal, Placement::Ahead))
        );
        assert_eq!(
            parse("add a jack-o-lantern to the right", &a),
            Some(add("pumpkin", 1, Size::Normal, Placement::Right))
        );
        assert_eq!(parse("undo", &a), Some(Intent::Undo));
        assert_eq!(parse("Undo that.", &a), Some(Intent::Undo));
        assert_eq!(parse("redo", &a), Some(Intent::Redo));
        assert_eq!(
            count_in("could you maybe put 5 pumpkins over there"),
            Some(5)
        );
        assert_eq!(count_in("pumpkins, a dozen"), None);
        assert_eq!(count_in("a pumpkin"), Some(1));
        assert_eq!(count_in("put a couple of ghosts ahead"), Some(2));
        assert_eq!(parse("remove it", &a), Some(Intent::RemoveSelected));
    }

    #[test]
    fn other_requests_need_a_decision() {
        let a = house();
        // Town and fantasy props are not haunted-house assets.
        assert_eq!(parse("add two barrels to my left", &a), None);
        assert_eq!(parse("add a crate", &a), None);
        assert_eq!(parse("add a dragon to my left", &a), None);
        assert_eq!(parse("add nine pumpkins", &a), None);
        assert_eq!(parse("add 0 pumpkins", &a), None);
        assert_eq!(parse("add pumpkins to my left", &a), None);
        assert_eq!(parse("add pumpkins next to the coffin", &a), None);
        assert_eq!(parse("make the fur rougher", &a), None);
        assert_eq!(parse("what can I add?", &a), None);
        assert_eq!(parse(&"add a pumpkin ".repeat(200), &a), None);
    }

    #[test]
    fn groups_sit_on_the_requested_side_without_stacking() {
        // Facing -Z: left is -X.
        let pose = Pose {
            position: [10.0, 0.0, 20.0],
            yaw: 0.0,
        };
        let e = edits(
            &add("pumpkin", 3, Size::Normal, Placement::Left),
            &pose,
            None,
        )
        .unwrap();
        assert_eq!(e.len(), 3);
        let mut points = Vec::new();
        for edit in &e {
            let Edit::Place {
                asset,
                position,
                scale,
            } = edit
            else {
                panic!("{edit:?}")
            };
            assert_eq!(asset, "custom:haunt:pumpkin");
            assert_eq!(*scale, None);
            assert!(position[0] < 10.0 - 1.0, "{position:?}");
            assert_eq!(position[1], 0.0);
            points.push(*position);
        }
        for (i, a) in points.iter().enumerate() {
            for b in &points[i + 1..] {
                assert!(((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() >= 0.9);
            }
        }
        // Facing +X (yaw 90°): right is +Z, ahead is +X.
        let east = Pose {
            position: [0.0, 0.0, 0.0],
            yaw: std::f64::consts::FRAC_PI_2,
        };
        let e = edits(&add("ghost", 1, Size::Huge, Placement::Right), &east, None).unwrap();
        let Edit::Place {
            position, scale, ..
        } = &e[0]
        else {
            panic!()
        };
        assert!(
            position[2] > 1.0 && position[0].abs() < 0.01,
            "{position:?}"
        );
        assert_eq!(*scale, Some(2.5));
        let e = edits(
            &add("ghost", 1, Size::Normal, Placement::Ahead),
            &east,
            None,
        )
        .unwrap();
        let Edit::Place { position, .. } = &e[0] else {
            panic!()
        };
        assert!(position[0] > 1.0, "{position:?}");
        assert!(e.iter().all(|e| matches!(e, Edit::Place { .. })));
        assert!(edits(&Intent::RemoveSelected, &east, None).is_err());
        assert_eq!(
            edits(&Intent::RemoveSelected, &east, Some("prop-1")).unwrap(),
            vec![Edit::Remove {
                target: "prop-1".into()
            }]
        );
        let edits8 = edits(&add("pumpkin", 8, Size::Huge, Placement::Left), &east, None).unwrap();
        assert_eq!(edits8.len(), 8);
    }

    #[test]
    fn the_pose_comes_from_the_camera_checkpoint() {
        let p = Pose::from_checkpoint(&Checkpoint {
            eye: [1.0, 1.7, 2.0],
            target: [1.0, 1.7, 1.0],
            mode: crate::CameraMode::Walk,
        });
        assert_eq!(p.position, [1.0, 0.0, 2.0]);
        assert!(p.yaw.abs() < 1e-9);
        let p = Pose::from_checkpoint(&Checkpoint {
            eye: [0.0, 1.7, 0.0],
            target: [1.0, 1.0, 0.0],
            mode: crate::CameraMode::Walk,
        });
        assert!((p.yaw - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn answers_name_the_change() {
        let pose = Pose {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
        };
        let a = answer(&add("pumpkin", 3, Size::Huge, Placement::Left), &pose, None).unwrap();
        assert_eq!(a.say, "Added three huge pumpkins to your left.");
        assert_eq!(a.edits.len(), 3);
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["edits"][0]["op"], "place");
        assert_eq!(json["edits"][0]["scale"], 2.5);
        assert_eq!(
            answer(&Intent::Undo, &pose, None).unwrap().edits,
            vec![HouseEdit::Undo]
        );
        let r: HouseRequest = serde_json::from_value(serde_json::json!({"text":"add a pumpkin","assets":[{"id":"custom:haunt:pumpkin","name":"Pumpkin"}],"player":{"position":[0,0,0],"yaw":0}})).unwrap();
        assert!(r.validate().is_ok());
        assert!(serde_json::from_value::<HouseRequest>(serde_json::json!({"text":"x","assets":[],"player":{"position":[0,0,0],"yaw":0},"extra":1})).is_err());
    }
}
