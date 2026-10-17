//! Design Lab: every screen of the current phone app on one board.
//!
//! The screens are captured from the app in `apps/openagents/` by
//! `apps/openagents/scripts/design-lab-capture.sh`, which drives the offline
//! demo through every screen. Each capture run is a folder named
//! `<YYYYmmdd-HHMMSS>-<commit>[-dirty]` of `<area>--<screen>.png` files, plus
//! `<area>--<screen>@dark.png` for the same screen in dark mode; a run is one
//! version of every screen it holds, and a screen that looks the same as in
//! the run before (in both modes) keeps its earlier version. Next to each
//! picture the tour saves its element tree, `<area>--<screen>[@dark].json`
//! ([`ElementTree`]), so the board can select a button or a card.
//!
//! Mocks and revision notes are a `mock.v1` document ([`mock`]) kept in the
//! captures folder and changed only through the same kind's checked changes.
//!
//! The board is a `board.v1` document, so it is also a Lab kind
//! ([`kind::Design`]): renames and areas are checked changes, and the card
//! shows the newest version of each screen.

pub mod kind;
pub mod mock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "board.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub schema: String,
    pub name: String,
    pub screens: Vec<Screen>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub area: String,
    /// Oldest first.
    pub versions: Vec<ScreenVersion>,
    /// Whether the newest capture run still has this screen. A screen the
    /// app no longer shows (the tour stopped capturing it) is kept for its
    /// history but hidden by default.
    #[serde(default = "yes")]
    pub current: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenVersion {
    /// `<run>/<file>` inside the captures folder.
    pub image: String,
    /// SHA-256 of the picture.
    pub sha256: String,
    /// The capture run's folder name.
    pub run: String,
    /// The app commit it was captured from.
    pub commit: String,
    pub unix: u64,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// The same screen in dark mode, `<run>/<area>--<screen>@dark.png`, when
    /// the run captured it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_sha256: Option<String>,
    /// The light picture's element tree, `<run>/<area>--<screen>.json`, when
    /// the run saved one. Screens captured before element trees have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elements: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_elements: Option<String>,
}

/// Every visible element of a captured screen, as the tour's
/// `XCUIApplication().snapshot()` saw it. Frames are in points; a picture
/// pixel is `scale` points' worth smaller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementTree {
    #[serde(default)]
    pub scale: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub elements: Vec<Element>,
}

/// One element of a screen: a button, a text field, a cell, ...
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// `button`, `text-field`, `text`, `image`, `cell`, ...
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Element {
    /// `button "Start" (start-button)`, for a spec line.
    pub fn describe(&self) -> String {
        let mut out = if self.kind.is_empty() {
            "element".to_string()
        } else {
            self.kind.clone()
        };
        if let Some(label) = self.label.as_deref().filter(|l| !l.is_empty()) {
            out.push_str(&format!(" \"{label}\""));
        }
        if let Some(value) = self.value.as_deref().filter(|v| !v.is_empty()) {
            out.push_str(&format!(" value \"{value}\""));
        }
        if let Some(id) = self.identifier.as_deref().filter(|i| !i.is_empty()) {
            out.push_str(&format!(" (`{id}`)"));
        }
        out
    }
}

/// A run's element tree, if the board names it.
pub fn elements(captures: &Path, board: &Board, path: &str) -> Option<ElementTree> {
    let bytes = image(captures, board, path)?;
    serde_json::from_slice(&bytes).ok()
}

impl Board {
    pub fn versions(&self) -> usize {
        self.screens.iter().map(|s| s.versions.len()).sum()
    }
}

/// Where capture runs go unless `DESIGN_LAB_CAPTURES` says otherwise.
pub fn default_captures() -> PathBuf {
    if let Ok(dir) = std::env::var("DESIGN_LAB_CAPTURES") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("work/design-lab/captures")
}

/// A run folder's time and commit: `20261017-141500-cb54dccbc2[-dirty]`.
pub fn run_info(name: &str) -> Option<(u64, String)> {
    let mut parts = name.splitn(3, '-');
    let (day, time, commit) = (parts.next()?, parts.next()?, parts.next()?);
    if day.len() != 8 || time.len() != 6 || commit.is_empty() {
        return None;
    }
    let num = |s: &str| s.parse::<u64>().ok();
    let (y, m, d) = (num(&day[..4])?, num(&day[4..6])?, num(&day[6..])?);
    let (hh, mm, ss) = (num(&time[..2])?, num(&time[2..4])?, num(&time[4..])?);
    Some((
        days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss,
        commit.to_string(),
    ))
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm), for local run times.
fn days_from_civil(y: u64, m: u64, d: u64) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A capture's area and screen: `drawer--sessions.png`. A dark-mode capture,
/// `drawer--sessions@dark.png`, names the same screen.
pub fn screen_of(file: &str) -> Option<(String, String)> {
    shot_of(file.strip_suffix(".png")?)
}

/// The screen an element tree belongs to: `drawer--sessions.json` or
/// `drawer--sessions@dark.json`.
pub fn elements_of(file: &str) -> Option<(String, String)> {
    shot_of(file.strip_suffix(".json")?)
}

fn shot_of(stem: &str) -> Option<(String, String)> {
    let stem = stem.strip_suffix("@dark").unwrap_or(stem);
    let (area, screen) = stem.split_once("--")?;
    (!area.is_empty() && !screen.is_empty()).then(|| (area.to_string(), screen.to_string()))
}

/// Whether a capture is the dark-mode picture of its screen.
pub fn is_dark(file: &str) -> bool {
    file.ends_with("@dark.png") || file.ends_with("@dark.json")
}

fn title(id: &str) -> String {
    let words = id.replace('-', " ");
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Width and height from a PNG's header.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return (0, 0);
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}

/// One screen's files in one run, each `<run>/<file>`.
#[derive(Default)]
struct Shot {
    area: String,
    name: String,
    light: Option<String>,
    dark: Option<String>,
    elements: Option<String>,
    dark_elements: Option<String>,
}

/// Builds the board from every capture run, oldest run first.
pub fn scan(captures: &Path) -> Result<Board, String> {
    let entries = std::fs::read_dir(captures).map_err(|e| {
        format!(
            "can't read {} ({e}); run design-lab-capture.sh first",
            captures.display()
        )
    })?;
    let mut runs: Vec<(String, u64, String)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            run_info(&name).map(|(unix, commit)| (name, unix, commit))
        })
        .collect();
    runs.sort();
    let newest = runs.last().map(|(run, _, _)| run.clone());
    let mut screens: BTreeMap<String, Screen> = BTreeMap::new();
    let mut in_newest: std::collections::BTreeSet<String> = Default::default();
    for (run, unix, commit) in runs {
        let mut files: Vec<String> = std::fs::read_dir(captures.join(&run))
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files.sort();
        // Each screen's light and dark pictures and element trees in this run.
        let mut shots: BTreeMap<String, Shot> = BTreeMap::new();
        for file in files {
            let (picture, (area, name)) = match (screen_of(&file), elements_of(&file)) {
                (Some(found), _) => (true, found),
                (None, Some(found)) => (false, found),
                (None, None) => continue,
            };
            let shot = shots
                .entry(format!("{area}--{name}"))
                .or_insert_with(|| Shot {
                    area,
                    name,
                    ..Shot::default()
                });
            let slot = match (picture, is_dark(&file)) {
                (true, false) => &mut shot.light,
                (true, true) => &mut shot.dark,
                (false, false) => &mut shot.elements,
                (false, true) => &mut shot.dark_elements,
            };
            *slot = Some(format!("{run}/{file}"));
        }
        for (
            id,
            Shot {
                area,
                name,
                light,
                dark,
                elements,
                dark_elements,
            },
        ) in shots
        {
            // A screen needs its light picture; dark is extra.
            let Some(file) = light else {
                continue;
            };
            if newest.as_deref() == Some(run.as_str()) {
                in_newest.insert(id.clone());
            }
            let bytes = std::fs::read(captures.join(&file)).map_err(|e| e.to_string())?;
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            let dark_sha256 = match &dark {
                Some(dark) => {
                    let bytes = std::fs::read(captures.join(dark)).map_err(|e| e.to_string())?;
                    Some(format!("{:x}", Sha256::digest(&bytes)))
                }
                None => None,
            };
            let screen = screens.entry(id.clone()).or_insert_with(|| Screen {
                title: title(&name),
                area,
                id,
                versions: Vec::new(),
                current: true,
            });
            // Unchanged since the last run: still the same version. A run
            // without a dark picture doesn't count as a change by itself.
            if let Some(last) = screen.versions.last_mut() {
                if last.sha256 == sha256 {
                    // The first element trees of an unchanged screen.
                    if last.elements.is_none() {
                        last.elements = elements.clone();
                    }
                    if last.dark_elements.is_none() && last.dark_sha256 == dark_sha256 {
                        last.dark_elements = dark_elements.clone();
                    }
                }
                if last.sha256 == sha256
                    && (dark_sha256.is_none() || last.dark_sha256 == dark_sha256)
                {
                    continue;
                }
                if last.sha256 == sha256 && last.dark.is_none() {
                    // The first dark picture of an unchanged screen.
                    last.dark = dark;
                    last.dark_sha256 = dark_sha256;
                    last.dark_elements = dark_elements;
                    continue;
                }
            }
            let (width, height) = png_size(&bytes);
            screen.versions.push(ScreenVersion {
                image: file,
                sha256,
                run: run.clone(),
                commit: commit.clone(),
                unix,
                width,
                height,
                dark,
                dark_sha256,
                elements,
                dark_elements,
            });
        }
    }
    let mut screens: Vec<Screen> = screens.into_values().collect();
    for screen in &mut screens {
        screen.current = in_newest.contains(&screen.id);
    }
    screens.sort_by(|a, b| (&a.area, &a.id).cmp(&(&b.area, &b.id)));
    Ok(Board {
        schema: SCHEMA.into(),
        name: "OpenAgents mobile app".into(),
        screens,
    })
}

/// A picture's or element tree's bytes, only if the board holds it.
pub fn image(captures: &Path, board: &Board, image: &str) -> Option<Vec<u8>> {
    let known = board.screens.iter().flat_map(|s| &s.versions).any(|v| {
        v.image == image
            || [&v.dark, &v.elements, &v.dark_elements]
                .iter()
                .any(|p| p.as_deref() == Some(image))
    });
    if !known {
        return None;
    }
    std::fs::read(captures.join(image)).ok()
}

/// The board page. It fetches `board.json` and pictures from `img/{image}`,
/// so the served and exported boards are the same file.
pub const PAGE: &str = include_str!("board.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_name_their_area_and_screen() {
        assert_eq!(
            screen_of("drawer--sessions.png"),
            Some(("drawer".into(), "sessions".into()))
        );
        assert_eq!(screen_of("drawer.png"), None);
        assert_eq!(screen_of("--x.png"), None);
        assert_eq!(screen_of("a--b.jpg"), None);
        assert_eq!(title("sessions-scrolled"), "Sessions scrolled");
    }

    #[test]
    fn run_folders_give_time_and_commit() {
        let (unix, commit) = run_info("20261017-141500-cb54dccbc2-dirty").unwrap();
        assert_eq!(unix, 1_792_246_500);
        assert_eq!(commit, "cb54dccbc2-dirty");
        assert_eq!(run_info("notes"), None);
    }

    #[test]
    fn dark_captures_pair_with_their_screen() {
        assert_eq!(
            screen_of("drawer--sessions@dark.png"),
            Some(("drawer".into(), "sessions".into()))
        );
        assert!(is_dark("drawer--sessions@dark.png"));
        assert!(!is_dark("drawer--sessions.png"));
        let dir = std::env::temp_dir().join(format!("design-lab-dark-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01".to_vec();
        // Run 1: light only. Run 2: the same light picture, now with dark.
        // Run 3: unchanged in both modes.
        for (run, dark) in [
            ("20261017-100000-aaaa", None),
            ("20261017-110000-bbbb", Some(1u8)),
            ("20261017-120000-cccc", Some(1)),
        ] {
            std::fs::create_dir_all(dir.join(run)).unwrap();
            std::fs::write(dir.join(run).join("home--home.png"), &png).unwrap();
            if let Some(extra) = dark {
                let mut bytes = png.clone();
                bytes.push(extra);
                std::fs::write(dir.join(run).join("home--home@dark.png"), bytes).unwrap();
            }
        }
        let board = scan(&dir).unwrap();
        assert_eq!(board.screens.len(), 1);
        let versions = &board.screens[0].versions;
        assert_eq!(versions.len(), 1);
        let dark = versions[0].dark.as_deref().unwrap();
        assert_eq!(dark, "20261017-110000-bbbb/home--home@dark.png");
        assert!(image(&dir, &board, dark).is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn screens_missing_from_the_newest_run_are_not_current() {
        let dir = std::env::temp_dir().join(format!("design-lab-current-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01".to_vec();
        std::fs::create_dir_all(dir.join("20261017-100000-aaaa")).unwrap();
        std::fs::create_dir_all(dir.join("20261017-110000-bbbb")).unwrap();
        for file in ["welcome--sign-in.png", "home--home.png"] {
            std::fs::write(dir.join("20261017-100000-aaaa").join(file), &png).unwrap();
        }
        std::fs::write(dir.join("20261017-110000-bbbb/home--home.png"), &png).unwrap();
        let board = scan(&dir).unwrap();
        let current = |id: &str| board.screens.iter().find(|s| s.id == id).unwrap().current;
        assert!(current("home--home"));
        assert!(!current("welcome--sign-in"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn element_trees_pair_with_their_pictures() {
        assert_eq!(
            elements_of("drawer--sessions@dark.json"),
            Some(("drawer".into(), "sessions".into()))
        );
        assert!(is_dark("drawer--sessions@dark.json"));
        let dir = std::env::temp_dir().join(format!("design-lab-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01".to_vec();
        let tree = r#"{"scale": 3, "width": 440, "height": 956, "elements": [
            {"type": "button", "identifier": "go", "label": "Go", "x": 1, "y": 2, "width": 3, "height": 4}]}"#;
        // Run 1 has no element tree; run 2 is the same picture with trees in
        // both modes; run 3 changes the picture without one.
        for (run, trees, extra) in [
            ("20261017-100000-aaaa", false, 0u8),
            ("20261017-110000-bbbb", true, 0),
            ("20261017-120000-cccc", false, 1),
        ] {
            std::fs::create_dir_all(dir.join(run)).unwrap();
            let mut bytes = png.clone();
            bytes.push(extra);
            std::fs::write(dir.join(run).join("home--home.png"), &bytes).unwrap();
            std::fs::write(dir.join(run).join("home--home@dark.png"), &bytes).unwrap();
            if trees {
                std::fs::write(dir.join(run).join("home--home.json"), tree).unwrap();
                std::fs::write(dir.join(run).join("home--home@dark.json"), tree).unwrap();
            }
        }
        let board = scan(&dir).unwrap();
        let versions = &board.screens[0].versions;
        assert_eq!(versions.len(), 2);
        assert_eq!(
            versions[0].elements.as_deref(),
            Some("20261017-110000-bbbb/home--home.json")
        );
        assert_eq!(
            versions[0].dark_elements.as_deref(),
            Some("20261017-110000-bbbb/home--home@dark.json")
        );
        // A screen captured without its tree still works; it just has none.
        assert_eq!(versions[1].elements, None);
        let read = elements(&dir, &board, versions[0].elements.as_deref().unwrap()).unwrap();
        assert_eq!(read.scale, 3.0);
        assert_eq!(read.elements[0].describe(), "button \"Go\" (`go`)");
        assert!(elements(&dir, &board, "20261017-120000-cccc/home--home.json").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_screen_gets_a_new_version_only_when_it_changes() {
        let dir = std::env::temp_dir().join(format!("design-lab-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(1206u32.to_be_bytes());
        png.extend(2622u32.to_be_bytes());
        for (run, extra) in [
            ("20261017-100000-aaaa", 0u8),
            ("20261017-110000-bbbb", 0),
            ("20261017-120000-cccc", 1),
        ] {
            std::fs::create_dir_all(dir.join(run)).unwrap();
            let mut bytes = png.clone();
            bytes.push(extra);
            std::fs::write(dir.join(run).join("home--home.png"), bytes).unwrap();
        }
        let board = scan(&dir).unwrap();
        assert_eq!(board.screens.len(), 1);
        let versions = &board.screens[0].versions;
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].commit, "aaaa");
        assert_eq!(versions[1].commit, "cccc");
        assert_eq!((versions[0].width, versions[0].height), (1206, 2622));
        assert!(image(&dir, &board, &versions[1].image).is_some());
        assert!(image(&dir, &board, "../secret").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
