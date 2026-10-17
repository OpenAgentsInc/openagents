//! Mock screens and revision notes: the `mock.v1` document.
//!
//! A mock is a blank phone- or desktop-sized frame that holds pieces cut from captured
//! screens (each keeping the element it came from), plain rectangles and text
//! boxes. A note is pinned to a point or a box on a screen version or a mock,
//! with its text, its author and whether it is resolved. The document lives
//! in the captures folder as its newest [`Version`], and every edit is one of
//! the intents in [`apply`], checked by [`check`] before it is saved, so a
//! new capture run or a reload never loses it.
//!
//! [`spec`] turns a mock, or a screen with its notes, into the Markdown a
//! building agent reads, and [`render`] draws a mock as a PNG.

use lab_kit::{Refusal, Report, Version};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{Board, Element};

pub const SCHEMA: &str = "mock.v1";

/// The file in the captures folder that holds the newest version.
pub const FILE: &str = "design-lab-mocks.json";

/// A phone frame in points when nothing says otherwise (iPhone 17 Pro Max).
pub const PHONE: (f64, f64) = (440.0, 956.0);
/// A desktop window in points, the size the desktop tour draws at (16:10).
/// The board asks for it when it starts a mock on the Desktop board.
pub const DESKTOP: (f64, f64) = (1280.0, 800.0);

/// Points to picture pixels for captures without an element tree.
pub const DEFAULT_SCALE: f64 = 3.0;

const MAX_MOCKS: usize = 200;
const MAX_ITEMS: usize = 500;
const MAX_NOTES: usize = 2000;
const MAX_TEXT: usize = 4000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mocks {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub mocks: Vec<Mock>,
    #[serde(default)]
    pub notes: Vec<Note>,
    /// The next number an id gets (`m4`, `i4`, `n4`).
    #[serde(default = "one")]
    pub next: u64,
}

fn one() -> u64 {
    1
}

impl Default for Mocks {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            name: "Mocks".into(),
            mocks: Vec::new(),
            notes: Vec::new(),
            next: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mock {
    pub id: String,
    pub title: String,
    /// Points.
    pub width: f64,
    pub height: f64,
    /// A CSS hex color.
    #[serde(default = "white")]
    pub background: String,
    /// Back to front.
    #[serde(default)]
    pub items: Vec<Item>,
}

fn white() -> String {
    "#ffffff".into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A piece of a captured screen.
    Crop,
    Rect,
    Text,
}

/// One thing on a mock, placed in the mock's points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    #[serde(default)]
    pub id: String,
    pub kind: ItemKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Fill for a rectangle, or behind a text box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Text color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Font size in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Corner radius in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

/// Where a crop came from: a picture of a screen version and the box cut
/// from it, in that picture's pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub screen: String,
    /// `<run>/<file>.png`, light or dark.
    pub image: String,
    pub crop: Rect,
    /// The picture's size in pixels.
    pub image_width: u32,
    pub image_height: u32,
    /// The element it was, when one was selected rather than a drawn box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<Element>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
}

/// A revision note pinned to a point (`w` and `h` zero) or a box.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    #[serde(default)]
    pub id: String,
    /// Shown on the pin; counts up and is never reused.
    #[serde(default)]
    pub number: u64,
    pub on: Target,
    pub text: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub unix: u64,
}

/// What a note points at. On a mock the box is in the mock's points; on a
/// screen it is in the picture's pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mock: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
    /// The screen version's picture, `<run>/<file>.png`, light or dark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub at: Rect,
}

fn refuse(message: impl Into<String>) -> Refusal {
    Refusal::new("mock_check", message)
}

/// `<run>/<file>.png` inside the captures folder, or
/// `desktop/<run>/<file>.png` for the desktop app; nothing else.
pub fn safe_picture(p: &str) -> bool {
    crate::capture_path_ok(p, ".png")
}

fn color_ok(c: &str) -> bool {
    parse_color(c).is_some()
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(c: &str) -> Option<[u8; 4]> {
    let hex = c.strip_prefix('#')?;
    let digit = |i: usize| u8::from_str_radix(hex.get(i..i + 1)?, 16).ok();
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        3 => Some([digit(0)? * 17, digit(1)? * 17, digit(2)? * 17, 255]),
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

fn number_ok(n: f64, lo: f64, hi: f64) -> bool {
    n.is_finite() && n >= lo && n <= hi
}

pub fn of(content: &Value) -> Result<Mocks, Refusal> {
    serde_json::from_value(content.clone()).map_err(|e| {
        Refusal::new(
            "document_format",
            format!("Choose a mock.v1 document ({e})."),
        )
    })
}

fn content_of(doc: &Mocks) -> Result<Value, Refusal> {
    serde_json::to_value(doc)
        .map_err(|_| Refusal::new("document_format", "Choose a mock.v1 document."))
}

/// Blocking problems refuse the document; what is fine is counted.
pub fn check(content: &Value) -> Result<Report, Refusal> {
    let doc = of(content)?;
    if doc.schema != SCHEMA {
        return Err(refuse("Choose a mock.v1 document."));
    }
    if doc.mocks.len() > MAX_MOCKS || doc.notes.len() > MAX_NOTES {
        return Err(refuse("This document has too many mocks or notes."));
    }
    let mut ids = BTreeSet::new();
    let mut fresh = |id: &str, what: &str| -> Result<(), Refusal> {
        if id.is_empty() {
            return Err(refuse(format!("A {what} has no id.")));
        }
        if !ids.insert(id.to_string()) {
            return Err(refuse(format!("Two things are both called {id}.")));
        }
        Ok(())
    };
    let mut items = 0;
    for mock in &doc.mocks {
        fresh(&mock.id, "mock")?;
        if mock.title.trim().is_empty() {
            return Err(refuse(format!("The mock {} needs a title.", mock.id)));
        }
        if !number_ok(mock.width, 40.0, 4000.0) || !number_ok(mock.height, 40.0, 8000.0) {
            return Err(refuse(format!("The mock {} has an odd size.", mock.id)));
        }
        if !color_ok(&mock.background) {
            return Err(refuse(format!(
                "The mock {} has an odd background color.",
                mock.id
            )));
        }
        if mock.items.len() > MAX_ITEMS {
            return Err(refuse(format!(
                "The mock {} holds too many pieces.",
                mock.id
            )));
        }
        for item in &mock.items {
            fresh(&item.id, "piece")?;
            check_item(item)?;
            items += 1;
        }
    }
    let mut numbers = BTreeSet::new();
    let mut open = 0;
    for note in &doc.notes {
        fresh(&note.id, "note")?;
        if !numbers.insert(note.number) || note.number == 0 {
            return Err(refuse(format!(
                "The note {} needs a number of its own.",
                note.id
            )));
        }
        if note.text.trim().is_empty() || note.text.len() > MAX_TEXT {
            return Err(refuse(format!("The note {} needs some words.", note.id)));
        }
        let at = &note.on.at;
        if ![at.x, at.y]
            .iter()
            .all(|n| number_ok(*n, -10_000.0, 20_000.0))
            || ![at.w, at.h].iter().all(|n| number_ok(*n, 0.0, 20_000.0))
        {
            return Err(refuse(format!(
                "The note {} points at an odd place.",
                note.id
            )));
        }
        match (&note.on.mock, &note.on.screen, &note.on.image) {
            (Some(mock), None, None) => {
                if !doc.mocks.iter().any(|m| &m.id == mock) {
                    return Err(refuse(format!(
                        "The note {} is on a mock that isn't here.",
                        note.id
                    )));
                }
            }
            (None, Some(screen), Some(image)) if !screen.is_empty() && safe_picture(image) => {}
            _ => {
                return Err(refuse(format!(
                    "The note {} needs a mock, or a screen and its picture.",
                    note.id
                )));
            }
        }
        if !note.resolved {
            open += 1;
        }
    }
    let past = |id: &str| {
        id.get(1..)
            .and_then(|n| n.parse::<u64>().ok())
            .is_some_and(|n| n >= doc.next)
    };
    if ids.iter().any(|id| past(id)) {
        return Err(refuse("An id is ahead of the document's counter."));
    }
    Ok(Report {
        warnings: Vec::new(),
        facts: json!({
            "mocks": doc.mocks.len(),
            "items": items,
            "notes": doc.notes.len(),
            "open_notes": open,
        }),
    })
}

fn check_item(item: &Item) -> Result<(), Refusal> {
    let id = &item.id;
    if ![item.x, item.y]
        .iter()
        .all(|n| number_ok(*n, -4000.0, 8000.0))
        || ![item.w, item.h].iter().all(|n| number_ok(*n, 1.0, 8000.0))
    {
        return Err(refuse(format!("The piece {id} has an odd place or size.")));
    }
    for c in [&item.fill, &item.color].into_iter().flatten() {
        if !color_ok(c) {
            return Err(refuse(format!("The piece {id} has an odd color.")));
        }
    }
    if item.size.is_some_and(|s| !number_ok(s, 4.0, 400.0))
        || item.radius.is_some_and(|r| !number_ok(r, 0.0, 4000.0))
    {
        return Err(refuse(format!(
            "The piece {id} has an odd font size or corner."
        )));
    }
    if item.text.as_ref().is_some_and(|t| t.len() > MAX_TEXT) {
        return Err(refuse(format!("The piece {id} has too much text.")));
    }
    match item.kind {
        ItemKind::Crop => {
            let Some(source) = &item.source else {
                return Err(refuse(format!(
                    "The piece {id} needs the screen it came from."
                )));
            };
            let c = source.crop;
            let (iw, ih) = (
                f64::from(source.image_width),
                f64::from(source.image_height),
            );
            if source.screen.is_empty()
                || !safe_picture(&source.image)
                || iw < 1.0
                || ih < 1.0
                || !number_ok(c.x, 0.0, iw - 1.0)
                || !number_ok(c.y, 0.0, ih - 1.0)
                || !number_ok(c.w, 1.0, iw - c.x + 0.5)
                || !number_ok(c.h, 1.0, ih - c.y + 0.5)
            {
                return Err(refuse(format!(
                    "The piece {id} doesn't name a part of a captured screen."
                )));
            }
        }
        ItemKind::Text => {
            if item.text.as_deref().is_none_or(|t| t.trim().is_empty()) {
                return Err(refuse(format!("The text box {id} needs some text.")));
            }
        }
        ItemKind::Rect => {}
    }
    if item.kind != ItemKind::Crop && item.source.is_some() {
        return Err(refuse(format!(
            "Only a piece of a screen has a source ({id})."
        )));
    }
    Ok(())
}

fn take_id(doc: &mut Mocks, prefix: char) -> String {
    let id = format!("{prefix}{}", doc.next);
    doc.next += 1;
    id
}

fn text(intent: &Value, key: &str) -> Result<String, Refusal> {
    intent[key]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| Refusal::new("invalid_edit", format!("Say the {key}.")))
}

fn mock_mut<'a>(doc: &'a mut Mocks, intent: &Value) -> Result<&'a mut Mock, Refusal> {
    let id = intent["mock"].as_str().unwrap_or_default();
    doc.mocks
        .iter_mut()
        .find(|m| m.id == id)
        .ok_or_else(|| Refusal::new("unknown_mock", format!("There is no mock {id}.")))
}

fn set_num(slot: &mut f64, intent: &Value, key: &str) {
    if let Some(n) = intent[key].as_f64() {
        *slot = n;
    }
}

fn set_opt<T: serde::de::DeserializeOwned>(slot: &mut Option<T>, intent: &Value, key: &str) {
    match intent.get(key) {
        Some(Value::Null) => *slot = None,
        Some(v) => {
            if let Ok(v) = serde_json::from_value(v.clone()) {
                *slot = Some(v);
            }
        }
        None => {}
    }
}

/// One intent on the document. `now` stamps new notes.
pub fn apply_intent(doc: &mut Mocks, intent: &Value, now: u64) -> Result<(), Refusal> {
    let unknown = || {
        Refusal::new(
            "unknown_intent",
            "Change mocks with mock_new, mock_set, mock_delete, item_add, item_set, item_delete, \
             item_order, note_add, note_set, note_delete or batch.",
        )
    };
    match intent["intent"].as_str().ok_or_else(unknown)? {
        "batch" => {
            let changes = intent["changes"]
                .as_array()
                .filter(|c| !c.is_empty() && c.len() <= 200)
                .ok_or_else(|| Refusal::new("invalid_edit", "Give a batch 1 to 200 changes."))?;
            for change in changes {
                if change["intent"] == "batch" {
                    return Err(Refusal::new("invalid_edit", "A batch can't hold a batch."));
                }
                apply_intent(doc, change, now)?;
            }
        }
        "mock_new" => {
            let id = take_id(doc, 'm');
            let title = intent["title"]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Mock {}", doc.mocks.len() + 1));
            doc.mocks.push(Mock {
                id,
                title,
                width: intent["width"].as_f64().unwrap_or(PHONE.0),
                height: intent["height"].as_f64().unwrap_or(PHONE.1),
                background: intent["background"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(white),
                items: Vec::new(),
            });
        }
        "mock_set" => {
            let mock = mock_mut(doc, intent)?;
            if intent.get("title").is_some() {
                mock.title = text(intent, "title")?;
            }
            if let Some(bg) = intent["background"].as_str() {
                mock.background = bg.into();
            }
            set_num(&mut mock.width, intent, "width");
            set_num(&mut mock.height, intent, "height");
        }
        "mock_delete" => {
            let id = mock_mut(doc, intent)?.id.clone();
            doc.mocks.retain(|m| m.id != id);
            doc.notes
                .retain(|n| n.on.mock.as_deref() != Some(id.as_str()));
        }
        "item_add" => {
            let mut item: Item = serde_json::from_value(intent["item"].clone()).map_err(|e| {
                Refusal::new("invalid_edit", format!("Describe the piece to add ({e})."))
            })?;
            item.id = take_id(doc, 'i');
            mock_mut(doc, intent)?.items.push(item);
        }
        "item_set" | "item_delete" | "item_order" => {
            let what = intent["intent"].as_str().unwrap_or_default().to_string();
            let mock = mock_mut(doc, intent)?;
            let id = intent["item"].as_str().unwrap_or_default();
            let at = mock.items.iter().position(|i| i.id == id).ok_or_else(|| {
                Refusal::new(
                    "unknown_item",
                    format!("The mock {} has no piece {id}.", mock.id),
                )
            })?;
            match what.as_str() {
                "item_delete" => {
                    mock.items.remove(at);
                }
                "item_order" => {
                    let item = mock.items.remove(at);
                    if intent["to"] == "back" {
                        mock.items.insert(0, item);
                    } else {
                        mock.items.push(item);
                    }
                }
                _ => {
                    let item = &mut mock.items[at];
                    for (slot, key) in [
                        (&mut item.x, "x"),
                        (&mut item.y, "y"),
                        (&mut item.w, "w"),
                        (&mut item.h, "h"),
                    ] {
                        set_num(slot, intent, key);
                    }
                    set_opt(&mut item.text, intent, "text");
                    set_opt(&mut item.fill, intent, "fill");
                    set_opt(&mut item.color, intent, "color");
                    set_opt(&mut item.size, intent, "size");
                    set_opt(&mut item.radius, intent, "radius");
                }
            }
        }
        "note_add" => {
            let mut note: Note = serde_json::from_value(intent["note"].clone()).map_err(|e| {
                Refusal::new("invalid_edit", format!("Describe the note to add ({e})."))
            })?;
            note.id = take_id(doc, 'n');
            note.number = doc.notes.iter().map(|n| n.number).max().unwrap_or(0) + 1;
            note.text = note.text.trim().to_string();
            note.author = note.author.trim().to_string();
            if note.unix == 0 {
                note.unix = now;
            }
            doc.notes.push(note);
        }
        "note_set" | "note_delete" => {
            let id = intent["note"].as_str().unwrap_or_default();
            let at =
                doc.notes.iter().position(|n| n.id == id).ok_or_else(|| {
                    Refusal::new("unknown_note", format!("There is no note {id}."))
                })?;
            if intent["intent"] == "note_delete" {
                doc.notes.remove(at);
            } else {
                let note = &mut doc.notes[at];
                if intent.get("text").is_some() {
                    note.text = text(intent, "text")?;
                }
                if let Some(resolved) = intent["resolved"].as_bool() {
                    note.resolved = resolved;
                }
            }
        }
        _ => return Err(unknown()),
    }
    Ok(())
}

/// An intent on a `mock.v1` content, checked.
pub fn apply(content: &Value, intent: &Value, now: u64) -> Result<(Value, Report), Refusal> {
    let mut doc = of(content)?;
    apply_intent(&mut doc, intent, now)?;
    let content = content_of(&doc)?;
    let report = check(&content)?;
    Ok((content, report))
}

/// The first version, an empty document.
pub fn first_version() -> Version {
    let content = serde_json::to_value(Mocks::default()).expect("mocks serialize");
    let report = check(&content).expect("an empty document is fine");
    Version {
        document: "mocks".into(),
        number: 0,
        parent: None,
        author: "design-lab".into(),
        summary: "Started".into(),
        created_unix_ms: 0,
        schema: SCHEMA.into(),
        content_sha256: lab_kit::digest(&content),
        report,
        content,
        assets: Vec::new(),
    }
}

fn file(captures: &Path) -> PathBuf {
    captures.join(FILE)
}

/// The newest saved version, or an empty first one.
pub fn load(captures: &Path) -> Result<Version, String> {
    match std::fs::read(file(captures)) {
        Ok(bytes) => {
            let version: Version = serde_json::from_slice(&bytes)
                .map_err(|e| format!("can't read {} ({e})", file(captures).display()))?;
            check(&version.content).map_err(|r| r.message)?;
            Ok(version)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(first_version()),
        Err(e) => Err(format!("can't read {} ({e})", file(captures).display())),
    }
}

/// Applies a checked change on top of `base` and saves the new version.
/// `expect` is the version the editor last saw; another tab's change in
/// between is refused rather than lost.
pub fn change(
    captures: &Path,
    base: &Version,
    expect: Option<u64>,
    intent: &Value,
    author: &str,
    summary: &str,
    now_ms: u64,
) -> Result<Version, Refusal> {
    if expect.is_some_and(|n| n != base.number) {
        return Err(Refusal::new(
            "stale_base",
            "Someone changed the mocks in another window. Reload to see their change.",
        ));
    }
    let (content, report) = apply(&base.content, intent, now_ms / 1000)?;
    let version = Version {
        document: base.document.clone(),
        number: base.number + 1,
        parent: Some(base.number),
        author: author.into(),
        summary: summary.into(),
        created_unix_ms: now_ms,
        schema: SCHEMA.into(),
        content_sha256: lab_kit::digest(&content),
        report,
        content,
        assets: Vec::new(),
    };
    let bytes = serde_json::to_vec_pretty(&version)
        .map_err(|_| Refusal::new("save_failed", "We couldn't save the mocks."))?;
    let tmp = captures.join(format!(".{FILE}.tmp"));
    std::fs::create_dir_all(captures)
        .and_then(|()| std::fs::write(&tmp, bytes))
        .and_then(|()| std::fs::rename(&tmp, file(captures)))
        .map_err(|e| Refusal::new("save_failed", format!("We couldn't save the mocks ({e}).")))?;
    Ok(version)
}

fn round(n: f64) -> String {
    let r = (n * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

fn place(r: &Rect, unit: &str) -> String {
    if r.w == 0.0 && r.h == 0.0 {
        format!("the point ({}, {}) {unit}", round(r.x), round(r.y))
    } else {
        format!(
            "the box at ({}, {}), {} × {} {unit}",
            round(r.x),
            round(r.y),
            round(r.w),
            round(r.h)
        )
    }
}

/// `Settings v2 (dark), commit abc`, for the picture a crop or note names.
fn version_name(board: &Board, screen: &str, image: &str) -> String {
    let Some(s) = board.screens.iter().find(|s| s.id == screen) else {
        return format!("{screen} (a screen no longer on the board), `{image}`");
    };
    for (i, v) in s.versions.iter().enumerate() {
        let dark = v.dark.as_deref() == Some(image);
        if v.image == image || dark {
            return format!(
                "{} (`{}`) v{}{}, commit {}",
                s.title,
                s.id,
                i + 1,
                if dark { " dark" } else { "" },
                v.commit
            );
        }
    }
    format!("{} (`{}`), `{image}`", s.title, s.id)
}

fn overlaps(r: &Rect, x: f64, y: f64, w: f64, h: f64) -> bool {
    if r.w == 0.0 && r.h == 0.0 {
        return r.x >= x && r.x <= x + w && r.y >= y && r.y <= y + h;
    }
    r.x < x + w && r.x + r.w > x && r.y < y + h && r.y + r.h > y
}

fn item_line(board: &Board, n: usize, item: &Item) -> String {
    let at = format!(
        "at ({}, {}), {} × {} pt",
        round(item.x),
        round(item.y),
        round(item.w),
        round(item.h)
    );
    match item.kind {
        ItemKind::Crop => {
            let source = item.source.as_ref();
            let what = source
                .and_then(|s| s.element.as_ref())
                .map(Element::describe)
                .unwrap_or_else(|| "a drawn box".into());
            let mut line = format!("{n}. **{what}** (`{}`) {at}, cut from ", item.id);
            if let Some(s) = source {
                line.push_str(&version_name(board, &s.screen, &s.image));
                line.push_str(&format!(
                    ", pixels ({}, {}) {} × {}",
                    round(s.crop.x),
                    round(s.crop.y),
                    round(s.crop.w),
                    round(s.crop.h)
                ));
                if let Some(e) = &s.element {
                    line.push_str(&format!(
                        "; on that screen it is at ({}, {}), {} × {} pt",
                        round(e.x),
                        round(e.y),
                        round(e.width),
                        round(e.height)
                    ));
                }
            }
            line.push('.');
            line
        }
        ItemKind::Rect => {
            let mut line = format!("{n}. **Rectangle** (`{}`) {at}", item.id);
            if let Some(fill) = &item.fill {
                line.push_str(&format!(", fill {fill}"));
            }
            if let Some(r) = item.radius {
                line.push_str(&format!(", corners {} pt", round(r)));
            }
            line.push('.');
            line
        }
        ItemKind::Text => {
            let mut line = format!(
                "{n}. **Text** (`{}`) {at}: \"{}\"",
                item.id,
                item.text
                    .as_deref()
                    .unwrap_or_default()
                    .replace('\n', " / ")
            );
            line.push_str(&format!(", {} pt", round(item.size.unwrap_or(17.0))));
            if let Some(c) = &item.color {
                line.push_str(&format!(", color {c}"));
            }
            line.push('.');
            line
        }
    }
}

fn note_line(n: &Note, pointing: &str) -> String {
    let by = if n.author.is_empty() {
        String::new()
    } else {
        format!(" ({})", n.author)
    };
    let state = if n.resolved { "resolved" } else { "open" };
    let mut line = format!(
        "- **{}.** [{state}] \"{}\"{by}, at {}",
        n.number,
        n.text.replace('\n', " / "),
        pointing
    );
    line.push('.');
    line
}

/// What [`spec`] makes.
pub struct Spec {
    pub markdown: String,
    /// The mock's picture, for a mock.
    pub mock: Option<String>,
}

/// The Markdown a building agent reads for a mock (by id or title) or a
/// screen (by id). `picture` is where the mock's PNG is, for the link.
pub fn spec(
    board: &Board,
    doc: &Mocks,
    name: &str,
    elements: &dyn Fn(&str) -> Option<crate::ElementTree>,
    picture: &dyn Fn(&str) -> String,
) -> Result<Spec, String> {
    let lower = name.to_lowercase();
    if let Some(mock) = doc
        .mocks
        .iter()
        .find(|m| m.id == name || m.title.to_lowercase() == lower)
    {
        let mut md = format!(
            "# Mock: {}\n\nA {} × {} pt screen (`{}`), background {}. Build this screen; \
             every piece below says where it goes and what it was cut from.\n\n![{}]({})\n\n",
            mock.title,
            round(mock.width),
            round(mock.height),
            mock.id,
            mock.background,
            mock.title,
            picture(&mock.id)
        );
        md.push_str("## Pieces, back to front\n\n");
        if mock.items.is_empty() {
            md.push_str("Nothing is on this mock yet.\n");
        }
        for (i, item) in mock.items.iter().enumerate() {
            md.push_str(&item_line(board, i + 1, item));
            md.push('\n');
        }
        let notes: Vec<&Note> = doc
            .notes
            .iter()
            .filter(|n| n.on.mock.as_deref() == Some(mock.id.as_str()))
            .collect();
        if !notes.is_empty() {
            md.push_str("\n## Notes\n\n");
        }
        for note in notes {
            let under: Vec<String> = mock
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| overlaps(&note.on.at, i.x, i.y, i.w, i.h))
                .map(|(n, i)| format!("piece {} (`{}`)", n + 1, i.id))
                .collect();
            let mut pointing = place(&note.on.at, "pt");
            if !under.is_empty() {
                pointing.push_str(&format!(", over {}", under.join(", ")));
            }
            md.push_str(&note_line(note, &pointing));
            md.push('\n');
        }
        return Ok(Spec {
            markdown: md,
            mock: Some(mock.id.clone()),
        });
    }
    let screen = board
        .screens
        .iter()
        .find(|s| s.id == name || s.title.to_lowercase() == lower)
        .ok_or_else(|| format!("no mock or screen is called {name}"))?;
    let newest = screen.versions.last().ok_or("the screen has no versions")?;
    let mut md = format!(
        "# Screen: {}\n\n`{}` in the {} area of the {} app, {} version{}; the newest is from commit {}.\n\n![{}](img/{})\n",
        screen.title,
        screen.id,
        screen.area,
        crate::APPS
            .iter()
            .find(|(id, _)| *id == screen.app)
            .map_or(screen.app.as_str(), |(_, name)| name),
        screen.versions.len(),
        if screen.versions.len() == 1 { "" } else { "s" },
        newest.commit,
        screen.title,
        newest.image
    );
    let notes: Vec<&Note> = doc
        .notes
        .iter()
        .filter(|n| n.on.screen.as_deref() == Some(screen.id.as_str()))
        .collect();
    md.push_str("\n## Notes\n\n");
    if notes.is_empty() {
        md.push_str("No notes are pinned to this screen.\n");
    }
    for note in notes {
        let image = note.on.image.as_deref().unwrap_or_default();
        let mut pointing = format!(
            "{} on {}",
            place(&note.on.at, "px"),
            version_name(board, &screen.id, image)
        );
        // The elements under the note, from that version's element tree.
        let tree = screen
            .versions
            .iter()
            .find(|v| v.image == image || v.dark.as_deref() == Some(image))
            .and_then(|v| {
                if v.dark.as_deref() == Some(image) {
                    v.dark_elements.as_ref().or(v.elements.as_ref())
                } else {
                    v.elements.as_ref()
                }
            })
            .and_then(|p| elements(p));
        if let Some(tree) = tree {
            let s = if tree.scale > 0.0 {
                tree.scale
            } else {
                DEFAULT_SCALE
            };
            let mut under: Vec<&Element> = tree
                .elements
                .iter()
                .filter(|e| e.kind != "other" || e.identifier.is_some() || e.label.is_some())
                .filter(|e| overlaps(&note.on.at, e.x * s, e.y * s, e.width * s, e.height * s))
                .collect();
            // The smallest elements say the most.
            under.sort_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)));
            let names: Vec<String> = under.iter().take(3).map(|e| e.describe()).collect();
            if !names.is_empty() {
                pointing.push_str(&format!(", over {}", names.join("; ")));
            }
        }
        md.push_str(&note_line(note, &pointing));
        md.push('\n');
    }
    let used: Vec<&Mock> = doc
        .mocks
        .iter()
        .filter(|m| {
            m.items
                .iter()
                .any(|i| i.source.as_ref().is_some_and(|s| s.screen == screen.id))
        })
        .collect();
    if !used.is_empty() {
        md.push_str("\n## Mocks that use it\n\n");
        for m in used {
            md.push_str(&format!("- {} (`{}`)\n", m.title, m.id));
        }
    }
    Ok(Spec {
        markdown: md,
        mock: None,
    })
}

/// An RGBA picture.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn blend(&mut self, x: i64, y: i64, c: [u8; 4], cover: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let a = f32::from(c[3]) / 255.0 * cover.clamp(0.0, 1.0);
        let i = (y as usize * self.w + x as usize) * 4;
        for k in 0..3 {
            let d = f32::from(self.px[i + k]);
            self.px[i + k] = (d + (f32::from(c[k]) - d) * a).round() as u8;
        }
        self.px[i + 3] = 255;
    }

    fn fill(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, c: [u8; 4]) {
        let (x0, y0) = (x.floor().max(0.0) as i64, y.floor().max(0.0) as i64);
        let (x1, y1) = ((x + w).ceil() as i64, (y + h).ceil() as i64);
        let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
        for py in y0..y1.min(self.h as i64) {
            for px in x0..x1.min(self.w as i64) {
                let (cx, cy) = (px as f64 + 0.5, py as f64 + 0.5);
                // Distance outside the rounded corner, for a soft edge.
                let qx = (x + r - cx).max(cx - (x + w - r)).max(0.0);
                let qy = (y + r - cy).max(cy - (y + h - r)).max(0.0);
                let cover = if r > 0.0 && qx > 0.0 && qy > 0.0 {
                    (r - (qx * qx + qy * qy).sqrt() + 0.5).clamp(0.0, 1.0)
                } else {
                    let inside_x = (cx - x).min(x + w - cx) + 0.5;
                    let inside_y = (cy - y).min(y + h - cy) + 0.5;
                    inside_x.min(inside_y).clamp(0.0, 1.0)
                };
                self.blend(px, py, c, cover as f32);
            }
        }
    }
}

/// A decoded picture: RGBA, 8 bits.
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Decodes a PNG to RGBA.
pub fn decode(bytes: &[u8]) -> Result<Picture, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("the picture is too large")?
    ];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("an indexed picture wasn't expanded".into()),
    };
    Ok(Picture { w, h, rgba })
}

const FONT: &[u8] = include_bytes!("../../openagents-web/static/fonts/geist/Geist-Medium.ttf");

/// Draws a mock as a PNG at `scale` pixels a point. `picture` reads a
/// capture's PNG by its `<run>/<file>` path.
pub fn render(
    mock: &Mock,
    scale: f64,
    picture: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
    let w = (mock.width * scale).round().max(1.0) as usize;
    let h = (mock.height * scale).round().max(1.0) as usize;
    let bg = parse_color(&mock.background).unwrap_or([255, 255, 255, 255]);
    let mut canvas = Canvas {
        w,
        h,
        px: [bg[0], bg[1], bg[2], 255].repeat(w * h),
    };
    let font = FontRef::try_from_slice(FONT).map_err(|e| e.to_string())?;
    let mut pictures: std::collections::BTreeMap<String, Option<Picture>> = Default::default();
    for item in &mock.items {
        let (x, y, iw, ih) = (
            item.x * scale,
            item.y * scale,
            item.w * scale,
            item.h * scale,
        );
        let radius = item.radius.unwrap_or(0.0) * scale;
        if let Some(fill) = item.fill.as_deref().and_then(parse_color) {
            canvas.fill(x, y, iw, ih, radius, fill);
        }
        match item.kind {
            ItemKind::Crop => {
                let Some(source) = &item.source else { continue };
                let pic = pictures
                    .entry(source.image.clone())
                    .or_insert_with(|| picture(&source.image).and_then(|b| decode(&b).ok()));
                let Some(pic) = pic else {
                    // A missing picture shows as a gray box.
                    canvas.fill(x, y, iw, ih, radius, [200, 200, 200, 255]);
                    continue;
                };
                let c = source.crop;
                for py in (y.floor().max(0.0) as i64)..((y + ih).ceil() as i64).min(h as i64) {
                    let sy = c.y + ((py as f64 + 0.5 - y) / ih) * c.h;
                    if sy < 0.0 || sy >= pic.h as f64 {
                        continue;
                    }
                    for px in (x.floor().max(0.0) as i64)..((x + iw).ceil() as i64).min(w as i64) {
                        let sx = c.x + ((px as f64 + 0.5 - x) / iw) * c.w;
                        if sx < 0.0 || sx >= pic.w as f64 {
                            continue;
                        }
                        let i = (sy as usize * pic.w + sx as usize) * 4;
                        let p = &pic.rgba[i..i + 4];
                        canvas.blend(px, py, [p[0], p[1], p[2], p[3]], 1.0);
                    }
                }
            }
            ItemKind::Text => {
                let size = (item.size.unwrap_or(17.0) * scale) as f32;
                let color = item
                    .color
                    .as_deref()
                    .and_then(parse_color)
                    .unwrap_or([17, 17, 17, 255]);
                let scaled = font.as_scaled(PxScale::from(size));
                let line_h = f64::from(size) * 1.25;
                let pad = 4.0 * scale;
                let mut lines: Vec<String> = Vec::new();
                for para in item.text.as_deref().unwrap_or_default().split('\n') {
                    let mut line = String::new();
                    for word in para.split(' ') {
                        let next = if line.is_empty() {
                            word.to_string()
                        } else {
                            format!("{line} {word}")
                        };
                        let width: f32 = next
                            .chars()
                            .map(|ch| scaled.h_advance(scaled.glyph_id(ch)))
                            .sum();
                        if f64::from(width) > iw - pad * 2.0 && !line.is_empty() {
                            lines.push(std::mem::replace(&mut line, word.to_string()));
                        } else {
                            line = next;
                        }
                    }
                    lines.push(line);
                }
                for (row, line) in lines.iter().enumerate() {
                    let base = y + pad + line_h * row as f64 + f64::from(scaled.ascent());
                    if base - f64::from(scaled.ascent()) > y + ih {
                        break;
                    }
                    let mut pen = (x + pad) as f32;
                    for ch in line.chars() {
                        let id = scaled.glyph_id(ch);
                        let glyph = id.with_scale_and_position(size, point(pen, base as f32));
                        pen += scaled.h_advance(id);
                        if let Some(outline) = font.outline_glyph(glyph) {
                            let b = outline.px_bounds();
                            outline.draw(|gx, gy, cover| {
                                canvas.blend(
                                    b.min.x as i64 + i64::from(gx),
                                    b.min.y as i64 + i64::from(gy),
                                    color,
                                    cover,
                                );
                            });
                        }
                    }
                }
            }
            ItemKind::Rect => {}
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&canvas.px)
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementTree, Screen, ScreenVersion};

    fn board() -> Board {
        Board {
            schema: crate::SCHEMA.into(),
            name: "Phone".into(),
            screens: vec![
                Screen {
                    id: "settings--settings".into(),
                    app: crate::PHONE.into(),
                    title: "Settings".into(),
                    area: "settings".into(),
                    versions: vec![ScreenVersion {
                        image: "20261017-100000-aaaa/settings--settings.png".into(),
                        sha256: "a".repeat(64),
                        run: "20261017-100000-aaaa".into(),
                        commit: "aaaa".into(),
                        unix: 1,
                        width: 4,
                        height: 4,
                        dark: Some("20261017-100000-aaaa/settings--settings@dark.png".into()),
                        dark_sha256: None,
                        elements: Some("20261017-100000-aaaa/settings--settings.json".into()),
                        dark_elements: None,
                    }],
                    current: true,
                },
                Screen {
                    id: "home--home".into(),
                    app: crate::PHONE.into(),
                    title: "Home".into(),
                    area: "home".into(),
                    versions: vec![ScreenVersion {
                        image: "20261017-100000-aaaa/home--home.png".into(),
                        sha256: "b".repeat(64),
                        run: "20261017-100000-aaaa".into(),
                        commit: "aaaa".into(),
                        unix: 1,
                        width: 4,
                        height: 4,
                        dark: None,
                        dark_sha256: None,
                        elements: None,
                        dark_elements: None,
                    }],
                    current: true,
                },
            ],
        }
    }

    fn button() -> Element {
        Element {
            kind: "button".into(),
            identifier: Some("account-sign-in".into()),
            label: Some("Sign in".into()),
            value: None,
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
        }
    }

    fn tree() -> ElementTree {
        ElementTree {
            scale: 1.0,
            width: 4.0,
            height: 4.0,
            elements: vec![button()],
        }
    }

    fn crop(screen: &str, image: &str, element: Option<Element>) -> Value {
        json!({
            "kind": "crop", "x": 10, "y": 20, "w": 2, "h": 2,
            "source": {
                "screen": screen, "image": image,
                "crop": {"x": 0, "y": 0, "w": 2, "h": 2},
                "image_width": 4, "image_height": 4,
                "element": element,
            }
        })
    }

    fn step(content: &Value, intent: Value) -> Value {
        apply(content, &intent, 100).unwrap().0
    }

    /// A mock built from two screens' pieces, a rectangle, a text box, and
    /// notes on the mock and on a screen.
    fn built() -> Value {
        let mut c = serde_json::to_value(Mocks::default()).unwrap();
        c = step(
            &c,
            json!({"intent": "mock_new", "title": "Start screen", "width": 40, "height": 80}),
        );
        c = step(
            &c,
            json!({"intent": "batch", "changes": [
                {"intent": "item_add", "mock": "m1",
                 "item": crop("settings--settings", "20261017-100000-aaaa/settings--settings.png", Some(button()))},
                {"intent": "item_add", "mock": "m1",
                 "item": crop("home--home", "20261017-100000-aaaa/home--home.png", None)},
                {"intent": "item_add", "mock": "m1",
                 "item": {"kind": "rect", "x": 0, "y": 60, "w": 40, "h": 20, "fill": "#ff0000"}},
                {"intent": "item_add", "mock": "m1",
                 "item": {"kind": "text", "x": 0, "y": 0, "w": 40, "h": 10, "text": "Start", "size": 8}},
            ]}),
        );
        c = step(
            &c,
            json!({"intent": "note_add", "note": {"on": {"mock": "m1", "at": {"x": 11, "y": 21}},
                "text": "Put the composer here", "author": "chris"}}),
        );
        step(
            &c,
            json!({"intent": "note_add", "note": {"on": {"screen": "settings--settings",
                "image": "20261017-100000-aaaa/settings--settings.png",
                "at": {"x": 0, "y": 0, "w": 1, "h": 1}},
                "text": "This button should say Start", "author": "chris"}}),
        )
    }

    #[test]
    fn mocks_and_notes_change_only_through_checked_intents() {
        let c = built();
        let doc = of(&c).unwrap();
        assert_eq!(doc.mocks[0].items.len(), 4);
        assert_eq!(doc.mocks[0].items[0].id, "i2");
        assert_eq!(
            doc.notes.iter().map(|n| n.number).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(doc.notes[0].unix, 100);
        assert_eq!(check(&c).unwrap().facts["open_notes"], 2);

        // Move, restyle, reorder, duplicate-by-add, resolve.
        let c = step(
            &c,
            json!({"intent": "item_set", "mock": "m1", "item": "i2", "x": 16, "y": 24}),
        );
        let c = step(
            &c,
            json!({"intent": "item_set", "mock": "m1", "item": "i4", "fill": null}),
        );
        let c = step(
            &c,
            json!({"intent": "item_order", "mock": "m1", "item": "i5", "to": "back"}),
        );
        let c = step(
            &c,
            json!({"intent": "note_set", "note": "n6", "resolved": true}),
        );
        let doc = of(&c).unwrap();
        let m = &doc.mocks[0];
        assert_eq!((m.items[1].x, m.items[1].y), (16.0, 24.0));
        assert_eq!(m.items[0].id, "i5");
        assert!(
            m.items
                .iter()
                .find(|i| i.id == "i4")
                .unwrap()
                .fill
                .is_none()
        );
        assert!(doc.notes[0].resolved);
        assert_eq!(check(&c).unwrap().facts["open_notes"], 1);

        let refuse = |intent: Value| apply(&c, &intent, 0).unwrap_err().code;
        assert_eq!(
            refuse(json!({"intent": "item_set", "mock": "m9", "item": "i2"})),
            "unknown_mock"
        );
        assert_eq!(
            refuse(json!({"intent": "item_delete", "mock": "m1", "item": "i99"})),
            "unknown_item"
        );
        assert_eq!(refuse(json!({"intent": "paint"})), "unknown_intent");
        assert_eq!(
            refuse(json!({"intent": "note_set", "note": "n6", "text": " "})),
            "invalid_edit"
        );
        // A crop outside its picture, a path out of the captures folder, a
        // note on a mock that isn't there, and an odd color are refused.
        let mut outside = crop("home--home", "20261017-100000-aaaa/home--home.png", None);
        outside["source"]["crop"]["w"] = json!(9);
        assert_eq!(
            refuse(json!({"intent": "item_add", "mock": "m1", "item": outside})),
            "mock_check"
        );
        let escape = crop("home--home", "../../etc/passwd.png", None);
        assert_eq!(
            refuse(json!({"intent": "item_add", "mock": "m1", "item": escape})),
            "mock_check"
        );
        assert_eq!(
            refuse(
                json!({"intent": "note_add", "note": {"on": {"mock": "m9", "at": {"x": 0, "y": 0}}, "text": "x"}})
            ),
            "mock_check"
        );
        assert_eq!(
            refuse(json!({"intent": "item_set", "mock": "m1", "item": "i4", "fill": "red"})),
            "mock_check"
        );
        // A batch is all or nothing.
        assert_eq!(
            refuse(json!({"intent": "batch", "changes": [
                {"intent": "item_delete", "mock": "m1", "item": "i2"},
                {"intent": "item_delete", "mock": "m1", "item": "i99"}]})),
            "unknown_item"
        );

        // Deleting a mock takes its notes with it.
        let c = step(&c, json!({"intent": "mock_delete", "mock": "m1"}));
        let doc = of(&c).unwrap();
        assert!(doc.mocks.is_empty());
        assert_eq!(doc.notes.len(), 1);
        assert_eq!(
            doc.notes[0].on.screen.as_deref(),
            Some("settings--settings")
        );
    }

    #[test]
    fn changes_save_to_the_captures_folder_and_refuse_a_stale_base() {
        let dir = std::env::temp_dir().join(format!("design-lab-mocks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let v0 = load(&dir).unwrap();
        assert_eq!(v0.number, 0);
        let new = json!({"intent": "mock_new"});
        let v1 = change(&dir, &v0, Some(0), &new, "chris", "New", 5_000).unwrap();
        assert_eq!((v1.number, v1.parent), (1, Some(0)));
        assert_eq!(load(&dir).unwrap(), v1);
        assert_eq!(
            change(&dir, &v1, Some(0), &new, "chris", "New", 6_000)
                .unwrap_err()
                .code,
            "stale_base"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_spec_names_every_piece_its_source_and_every_note() {
        let doc = of(&built()).unwrap();
        let b = board();
        let spec = spec(&b, &doc, "Start screen", &|_| Some(tree()), &|id| {
            format!("specs/{id}.png")
        })
        .unwrap();
        let md = &spec.markdown;
        assert_eq!(spec.mock.as_deref(), Some("m1"));
        assert!(md.starts_with("# Mock: Start screen"), "{md}");
        assert!(md.contains("![Start screen](specs/m1.png)"), "{md}");
        assert!(md.contains(
            "1. **button \"Sign in\" (`account-sign-in`)** (`i2`) at (10, 20), 2 × 2 pt, cut from \
             Settings (`settings--settings`) v1, commit aaaa"
        ), "{md}");
        assert!(md.contains("2. **a drawn box** (`i3`)"), "{md}");
        assert!(md.contains("Home (`home--home`) v1"), "{md}");
        assert!(
            md.contains("**Rectangle** (`i4`) at (0, 60), 40 × 20 pt, fill #ff0000"),
            "{md}"
        );
        assert!(
            md.contains("**Text** (`i5`) at (0, 0), 40 × 10 pt: \"Start\", 8 pt"),
            "{md}"
        );
        assert!(
            md.contains(
                "- **1.** [open] \"Put the composer here\" (chris), at the point (11, 21) pt, over \
             piece 1 (`i2`), piece 2 (`i3`)."
            ),
            "{md}"
        );

        let screen = super::spec(&b, &doc, "settings--settings", &|_| Some(tree()), &|_| {
            String::new()
        })
        .unwrap();
        let md = &screen.markdown;
        assert!(screen.mock.is_none());
        assert!(md.starts_with("# Screen: Settings"), "{md}");
        assert!(
            md.contains(
                "\"This button should say Start\" (chris), at the box at (0, 0), 1 × 1 px on \
             Settings (`settings--settings`) v1, commit aaaa, over button \"Sign in\" \
             (`account-sign-in`)."
            ),
            "{md}"
        );
        assert!(md.contains("- Start screen (`m1`)"), "{md}");
        assert!(super::spec(&b, &doc, "nothing", &|_| None, &|_| String::new()).is_err());
    }

    fn png4() -> Vec<u8> {
        // 4 × 4: the top-left 2 × 2 is blue, the rest green.
        let mut px = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                px.extend(if x < 2 && y < 2 {
                    [0, 0, 255, 255]
                } else {
                    [0, 255, 0, 255]
                });
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 4, 4);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        out
    }

    #[test]
    fn a_mock_renders_its_pieces_to_a_png() {
        let doc = of(&built()).unwrap();
        let png = render(&doc.mocks[0], 2.0, &|p| p.ends_with(".png").then(png4)).unwrap();
        let pic = decode(&png).unwrap();
        assert_eq!((pic.w, pic.h), (80, 160));
        let at =
            |x: usize, y: usize| pic.rgba[(y * pic.w + x) * 4..(y * pic.w + x) * 4 + 3].to_vec();
        // The settings crop (the blue corner) at (10, 20) pt, 2 pixels a point.
        assert_eq!(at(21, 41), [0, 0, 255]);
        // The red rectangle and the white background.
        assert_eq!(at(10, 140), [255, 0, 0]);
        assert_eq!(at(70, 100), [255, 255, 255]);
        assert_eq!(parse_color("#abc"), Some([0xaa, 0xbb, 0xcc, 255]));
        assert_eq!(parse_color("red"), None);
    }
}
