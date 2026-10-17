//! Which files may leave the computer that has them.
//!
//! The rules are `scripts/film/lab_private.json`, the list `lab.py` guards
//! renders with: a path that contains one of a rule's lowercase substrings
//! is [`AssetClass::PrivateLocal`] (MetaHuman exports, Fab packs, Berkeley
//! Mono, and clips drawn with it). On the platform these classes go on a
//! version's asset list, so only an enrolled host renders such a version
//! and everything made from it is marked not for training.

use std::sync::OnceLock;

use lab_kit::{AssetClass, AssetEntry};
use serde::Deserialize;
use serde_json::Value;

const RULES: &str = include_str!("../../../scripts/film/lab_private.json");

#[derive(Debug, Deserialize)]
pub struct Rule {
    #[serde(rename = "match")]
    pub patterns: Vec<String>,
    pub reason: String,
}

#[derive(Deserialize)]
struct Rules {
    rules: Vec<Rule>,
}

/// The rules, in file order.
pub fn rules() -> &'static [Rule] {
    static PARSED: OnceLock<Vec<Rule>> = OnceLock::new();
    PARSED.get_or_init(|| {
        serde_json::from_str::<Rules>(RULES)
            .map(|r| r.rules)
            .unwrap_or_default()
    })
}

/// The class of a file, by its path as a document writes it, and the
/// plain reason when it's private.
pub fn class_for(location: &str) -> (AssetClass, Option<&'static str>) {
    let path = location.to_lowercase();
    rules()
        .iter()
        .find(|rule| rule.patterns.iter().any(|p| path.contains(p.as_str())))
        .map_or((AssetClass::Public, None), |rule| {
            (AssetClass::PrivateLocal, Some(rule.reason.as_str()))
        })
}

/// An asset entry for a file the document uses, classed by the rules.
pub fn entry(location: &str, sha256: &str) -> AssetEntry {
    AssetEntry {
        location: location.to_owned(),
        sha256: sha256.to_owned(),
        class: class_for(location).0,
    }
}

/// Every file path a document names, in order and without repeats: shot
/// clips and overlays, title backgrounds, music, and voiceovers (`cut.v1`);
/// a shot's script and inputs (`shot.v1`).
pub fn sources(content: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |v: &Value| {
        if let Some(s) = v.as_str().filter(|s| !s.is_empty())
            && !out.iter().any(|o| o == s)
        {
            out.push(s.to_owned());
        }
    };
    for shot in content["shots"].as_array().into_iter().flatten() {
        add(&shot["src"]);
        add(&shot["over"]["src"]);
        add(&shot["title"]["under"]["src"]);
    }
    add(&content["music"]);
    add(&content["voice"]["src"]);
    for voice in content["voices"].as_array().into_iter().flatten() {
        add(&voice["src"]);
    }
    add(&content["script"]);
    for input in content["inputs"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        add(input);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_lab_rules_class_private_files() {
        assert!(!rules().is_empty());
        for (path, private) in [
            ("~/work/film-out/clips/metahuman-take3.mp4", true),
            ("/Users/x/Draconia/face.mov", true),
            ("~/assets/fab-pack/rock.mp4", true),
            ("~/work/film-out/segments/a.mp4", true),
            ("~/work/fonts/BerkeleyMono.ttf", true),
            ("~/work/film-out/clips/street.mp4", false),
        ] {
            let (class, reason) = class_for(path);
            assert_eq!(class == AssetClass::PrivateLocal, private, "{path}");
            assert_eq!(reason.is_some(), private, "{path}");
        }
        assert_eq!(
            entry("/x/metahuman.mp4", "ab").class,
            AssetClass::PrivateLocal
        );
    }

    #[test]
    fn sources_lists_every_file_a_cut_names_once() {
        let cut = json!({
            "music": "~/m.wav",
            "voices": [{"src": "~/v.wav", "at": 0}],
            "shots": [
                {"name": "a", "src": "~/a.mp4", "dur": 1, "over": {"src": "~/o.mov"}},
                {"name": "b", "src": "~/a.mp4", "dur": 1},
                {"name": "t", "title": {"lines": [], "under": {"src": "~/u.mp4"}}, "dur": 1}
            ]
        });
        assert_eq!(
            sources(&cut),
            ["~/a.mp4", "~/o.mov", "~/u.mp4", "~/m.wav", "~/v.wav"]
        );
    }
}
