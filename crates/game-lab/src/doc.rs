//! The `game.v1` document: a game's editable surface as typed JSON
//! (docs/game-lab/README.md). For The Ice Cream Man it holds the house
//! (seed, extra hiding spots), the player, the Ice Cream Man's behaviour,
//! items and ammo, how far noises carry, the escape rules, and the words
//! the game shows. Every number maps onto [`icecream_rules::Tuning`], so a
//! version plays as data: the same build reads it from the page address.

use icecream_rules::Tuning;
use icecream_rules::layout::KEYS;
use icecream_rules::tuning::{FIELDS, Field};
use icecream_web::copy;
use serde_json::{Map, Value, json};

/// The document kind.
pub const KIND: &str = "game.v1";
/// The one game so far.
pub const ICE_CREAM_MAN: &str = "the-ice-cream-man";
/// The sections that hold numbers, in order.
pub const SECTIONS: [&str; 9] = [
    "layout", "player", "man", "items", "weapons", "minions", "noise", "rules", "look",
];
/// The front door's two rules.
pub const DOOR_OPENS: &str = "opens_with_all_keys";
pub const DOOR_LOCKED: &str = "locked";
/// The longest a line of the game's text may be.
pub const MAX_WORDS: usize = 160;

/// The words a document may change, and the game's own.
#[must_use]
pub fn words() -> [(&'static str, &'static str, &'static str); 4] {
    [
        ("title", copy::TITLE, "the game's name on its title card"),
        ("tagline", copy::TAGLINE, "the line under the title"),
        (
            "escaped_line",
            copy::ESCAPED_LINE,
            "the line when you escape",
        ),
        (
            "scooped_line",
            copy::SCOOPED_LINE,
            "the line when he scoops you",
        ),
    ]
}

/// A number as a document keeps it: six decimals at most, whole numbers
/// as integers.
#[must_use]
pub fn number(value: f64, whole: bool) -> Value {
    if whole {
        json!(value.round() as i64)
    } else {
        let rounded = (value * 1e6).round() / 1e6;
        json!(rounded)
    }
}

/// Version 1: The Ice Cream Man as the game plays today.
#[must_use]
pub fn original() -> Value {
    let mut doc = json!({
        "schema": KIND,
        "game": ICE_CREAM_MAN,
        "layout": {"seed": icecream_rules::bot::ESCAPE_SEED},
        "player": {},
        "man": {},
        "items": {},
        "weapons": {},
        "minions": {},
        "noise": {},
        "rules": {"front_door": DOOR_OPENS, "keys": KEYS},
        "look": {},
        "copy": {},
    });
    for f in FIELDS {
        doc[f.section][f.name] = number(f.default, f.whole);
    }
    for (name, text, _) in words() {
        doc["copy"][name] = json!(text);
    }
    doc
}

/// Fills in what a document saved before the game's code gained a setting
/// doesn't have: each new section and value at its default, which plays
/// the game as it was (the new things are off by default). Returns
/// whether anything was added.
pub fn upgrade(doc: &mut Value) -> bool {
    let Some(top) = doc.as_object_mut() else {
        return false;
    };
    let mut added = false;
    for f in FIELDS {
        let section = top
            .entry(f.section)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(values) = section.as_object_mut()
            && !values.contains_key(f.name)
        {
            values.insert(f.name.into(), number(f.default, f.whole));
            added = true;
        }
    }
    added
}

/// A document read and checked: what a run plays by.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub tuning: Tuning,
    /// The house the card plays.
    pub seed: u64,
    pub door_locked: bool,
    /// The words that differ from the game's own.
    pub words: Vec<(String, String)>,
}

/// What a refusal says when a change needs the game's code changed rather
/// than its settings.
pub const NEEDS_CODE: &str = "That needs the game's code changed, not just its settings, so I can't do it as an edit here; Coder can make that change as a separate step (once the person agrees, game_propose_code records it for Coder).";

fn about(name: &str) -> Option<&'static Field> {
    icecream_rules::tuning::field(name)
}

/// Reads a document, checking every value against the schema: the
/// problems in plain words when it doesn't fit.
///
/// # Errors
/// Each problem found, in plain words.
pub fn read(doc: &Value) -> Result<Game, Vec<String>> {
    let mut problems = Vec::new();
    let Some(top) = doc.as_object() else {
        return Err(vec!["The game document must be a JSON object.".into()]);
    };
    if doc["schema"] != KIND {
        problems.push(format!("The document's schema must stay {KIND}."));
    }
    if doc["game"] != ICE_CREAM_MAN {
        problems.push("The document is The Ice Cream Man; it can't become another game.".into());
    }
    for key in top.keys() {
        let known =
            ["schema", "game", "copy"].contains(&key.as_str()) || SECTIONS.contains(&key.as_str());
        if !known {
            problems.push(format!(
                "The game has no part called \"{key}\". {NEEDS_CODE}"
            ));
        }
    }
    let mut tuning = Tuning::default();
    let mut seed = 0;
    let mut door_locked = false;
    for section in SECTIONS {
        let Some(values) = doc[section].as_object() else {
            problems.push(format!("The \"{section}\" section is missing."));
            continue;
        };
        for (name, value) in values {
            match (section, name.as_str()) {
                ("layout", "seed") => match value.as_u64() {
                    Some(s) if s <= u64::from(u32::MAX) => seed = s,
                    _ => problems.push(
                        "layout.seed, which picks the house, must be a whole number from 0 to 4294967295."
                            .into(),
                    ),
                },
                ("rules", "front_door") => match value.as_str() {
                    Some(DOOR_OPENS) => {}
                    Some(DOOR_LOCKED) => door_locked = true,
                    _ => problems.push(format!(
                        "rules.front_door must be \"{DOOR_OPENS}\" or \"{DOOR_LOCKED}\"."
                    )),
                },
                ("rules", "keys") => {
                    if value.as_u64() != Some(KEYS as u64) {
                        problems.push(format!(
                            "The house always hides {KEYS} keys; changing how many the door needs isn't a setting yet. {NEEDS_CODE}"
                        ));
                    }
                }
                (_, name) => match about(name) {
                    Some(field) if field.section == section => match value.as_f64() {
                        Some(n) => {
                            if let Err(why) = tuning.set(name, n) {
                                problems.push(format!("{section}.{why}."));
                            }
                        }
                        None => problems.push(format!("{section}.{name} must be a number.")),
                    },
                    Some(field) => problems.push(format!(
                        "{name} belongs in the \"{}\" section, not \"{section}\".",
                        field.section
                    )),
                    None => problems.push(format!(
                        "The game has no setting called {section}.{name}. {NEEDS_CODE}"
                    )),
                },
            }
        }
        for f in FIELDS.iter().filter(|f| f.section == section) {
            if !values.contains_key(f.name) {
                problems.push(format!("{section}.{} is missing ({}).", f.name, f.about));
            }
        }
    }
    let mut words = Vec::new();
    match doc["copy"].as_object() {
        None => problems.push("The \"copy\" section, the game's words, is missing.".into()),
        Some(copy) => {
            for key in copy.keys() {
                if !self::words().iter().any(|(name, _, _)| name == key) {
                    problems.push(format!(
                        "The game shows no text called copy.{key}; new text needs a new place on screen. {NEEDS_CODE}"
                    ));
                }
            }
            for (name, own, _) in self::words() {
                match copy.get(name).and_then(Value::as_str) {
                    Some(text) if text.trim().is_empty() => {
                        problems.push(format!("copy.{name} can't be empty."));
                    }
                    Some(text) if text.chars().count() > MAX_WORDS => problems.push(format!(
                        "copy.{name} is too long to fit on screen ({MAX_WORDS} characters at most)."
                    )),
                    Some(text) => {
                        if text != own {
                            words.push((name.to_owned(), text.to_owned()));
                        }
                    }
                    None => problems.push(format!("copy.{name} must be text.")),
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(Game {
            tuning,
            seed,
            door_locked,
            words,
        })
    } else {
        Err(problems)
    }
}

/// The game's rules, beyond each value's bounds: what would break a run.
/// Problems in plain words.
#[must_use]
pub fn rule_problems(game: &Game) -> Vec<String> {
    let t = &game.tuning;
    let mut problems = Vec::new();
    if game.door_locked {
        problems.push(
            "With the front door locked for good there's no way out: the front door is the only exit, so nobody could ever escape."
                .into(),
        );
    }
    if t.volley_min >= t.volley_max {
        problems.push(format!(
            "He'd never fire sprinkles: volley_min ({}) must be less than volley_max ({}).",
            t.volley_min, t.volley_max
        ));
    }
    if t.fudge_start > t.fudge_max {
        problems.push(format!(
            "You'd start with more hot fudge ({}) than you can carry ({}).",
            t.fudge_start, t.fudge_max
        ));
    }
    if t.pistol_ammo > t.pistol_max {
        problems.push(format!(
            "The cone pistol would come with more cones ({}) than you can carry ({}).",
            t.pistol_ammo, t.pistol_max
        ));
    }
    if t.sugar_ammo > t.sugar_max {
        problems.push(format!(
            "The sugar launcher would come with more sugar ({}) than you can carry ({}).",
            t.sugar_ammo, t.sugar_max
        ));
    }
    if t.earned_weapons == 1 && t.minion_count == 0 {
        problems.push(
            "The candy cane sword and the sugar launcher are earned by melting minions, so with no minions they could never be earned: give the house minions (minions.minion_count) as well."
                .into(),
        );
    }
    if t.fog_near >= t.fog_far {
        problems.push(format!(
            "The fog must start (fog_near, {}) nearer than where it's thickest (fog_far, {}).",
            t.fog_near, t.fog_far
        ));
    }
    if t.farts_start > t.farts_max {
        problems.push(format!(
            "You'd start with more fart clouds ({}) than you can carry ({}).",
            t.farts_start, t.farts_max
        ));
    }
    problems
}

/// The page-address fragment that plays this version on the game's one
/// build: the seed, then each number and word that differs from the game's
/// own (`icecream-web`'s `Options`).
#[must_use]
pub fn fragment(game: &Game) -> String {
    let defaults = Tuning::default();
    let mut parts = vec![format!("seed={}", game.seed)];
    for f in FIELDS {
        let (Some(value), Some(own)) = (game.tuning.get(f.name), defaults.get(f.name)) else {
            continue;
        };
        if value != own {
            let rounded = (value * 1e6).round() / 1e6;
            parts.push(format!("t.{}={rounded}", f.name));
        }
    }
    for (name, text) in &game.words {
        parts.push(format!("c.{name}={}", encode(text)));
    }
    parts.join("&")
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Every value that differs between two documents: (path, before, after).
#[must_use]
pub fn changes(a: &Value, b: &Value) -> Vec<(String, Value, Value)> {
    let mut out = Vec::new();
    let empty = Map::new();
    let sections: Vec<&str> = SECTIONS.iter().copied().chain(["copy"]).collect();
    for section in sections {
        let before = a[section].as_object().unwrap_or(&empty);
        let after = b[section].as_object().unwrap_or(&empty);
        let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
        names.sort();
        names.dedup();
        for name in names {
            let x = before.get(name).cloned().unwrap_or(Value::Null);
            let y = after.get(name).cloned().unwrap_or(Value::Null);
            if !same(&x, &y) {
                out.push((format!("{section}.{name}"), x, y));
            }
        }
    }
    out
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

/// The `game.v1` JSON Schema (docs/game-lab/schemas/game.v1.json).
#[must_use]
pub fn schema() -> Value {
    let mut properties = Map::new();
    properties.insert("schema".into(), json!({"const": KIND}));
    properties.insert("game".into(), json!({"const": ICE_CREAM_MAN}));
    for section in SECTIONS {
        let mut fields = Map::new();
        let mut required = Vec::new();
        if section == "layout" {
            fields.insert(
                "seed".into(),
                json!({"type": "integer", "minimum": 0, "maximum": u32::MAX, "description": "the house the card plays"}),
            );
            required.push(json!("seed"));
        }
        if section == "rules" {
            fields.insert(
                "front_door".into(),
                json!({"enum": [DOOR_OPENS, DOOR_LOCKED], "description": "how the front door, the only way out, opens; the rules refuse \"locked\", since nobody could escape"}),
            );
            fields.insert(
                "keys".into(),
                json!({"const": KEYS, "description": "keys the house hides and the door needs (fixed in the game's code)"}),
            );
            required.push(json!("front_door"));
            required.push(json!("keys"));
        }
        for f in FIELDS.iter().filter(|f| f.section == section) {
            fields.insert(
                f.name.into(),
                json!({
                    "type": if f.whole { "integer" } else { "number" },
                    "minimum": number(f.min, f.whole),
                    "maximum": number(f.max, f.whole),
                    "default": number(f.default, f.whole),
                    "description": f.about,
                }),
            );
            required.push(json!(f.name));
        }
        properties.insert(
            section.into(),
            json!({"type": "object", "additionalProperties": false, "required": required, "properties": fields}),
        );
    }
    let mut copy = Map::new();
    for (name, own, what) in words() {
        copy.insert(
            name.into(),
            json!({"type": "string", "minLength": 1, "maxLength": MAX_WORDS, "default": own, "description": what}),
        );
    }
    let names: Vec<&str> = words().iter().map(|(n, _, _)| *n).collect();
    properties.insert(
        "copy".into(),
        json!({"type": "object", "additionalProperties": false, "required": names, "properties": copy}),
    );
    let mut required: Vec<&str> = vec!["schema", "game"];
    required.extend(SECTIONS);
    required.push("copy");
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://openagents.com/schemas/game.v1.json",
        "title": "game.v1: a game's editable surface in Game Lab (docs/game-lab/README.md)",
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_reads_back_as_the_game_today() {
        let game = read(&original()).unwrap();
        assert_eq!(game.tuning, Tuning::default());
        assert!(!game.door_locked);
        assert!(game.words.is_empty());
        assert_eq!(fragment(&game), format!("seed={}", game.seed));
    }

    #[test]
    fn values_outside_the_schema_are_refused_in_plain_words() {
        let mut doc = original();
        doc["man"]["man_chase"] = json!(500.0);
        doc["rules"]["door_locked"] = json!(true);
        doc["copy"]["subtitle"] = json!("hi");
        doc["items"]["fudge_start"] = json!(2.5);
        doc["rules"]["keys"] = json!(5);
        let problems = read(&doc).unwrap_err().join("\n");
        assert!(
            problems.contains("man_chase must be between 0.5 and 14"),
            "{problems}"
        );
        assert!(
            problems.contains("no setting called rules.door_locked"),
            "{problems}"
        );
        assert!(problems.contains("copy.subtitle"), "{problems}");
        assert!(
            problems.contains("fudge_start must be a whole number"),
            "{problems}"
        );
        assert!(problems.contains("always hides 3 keys"), "{problems}");
        assert!(problems.contains("Coder"), "{problems}");
    }

    #[test]
    fn a_locked_front_door_breaks_the_rules() {
        let mut doc = original();
        doc["rules"]["front_door"] = json!(DOOR_LOCKED);
        let game = read(&doc).unwrap();
        assert!(rule_problems(&game)[0].contains("no way out"));
    }

    #[test]
    fn a_version_plays_from_its_fragment() {
        let mut doc = original();
        doc["man"]["man_chase"] = json!(4.48);
        doc["layout"]["extra_closets"] = json!(2);
        doc["copy"]["title"] = json!("The Slow Man & Co");
        let game = read(&doc).unwrap();
        let fragment = fragment(&game);
        assert!(fragment.contains("t.man_chase=4.48"), "{fragment}");
        assert!(fragment.contains("t.extra_closets=2"), "{fragment}");
        assert!(
            fragment.contains("c.title=The%20Slow%20Man%20%26%20Co"),
            "{fragment}"
        );
        let changed = changes(&original(), &doc);
        assert_eq!(changed.len(), 3, "{changed:?}");
    }

    #[test]
    fn the_checked_in_schema_is_the_one_the_lab_uses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/game-lab/schemas/game.v1.json"
        );
        let file: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the schema file")).unwrap();
        assert_eq!(
            file,
            schema(),
            "regenerate it: cargo run -p game-lab -- schema > docs/game-lab/schemas/game.v1.json"
        );
    }
}
