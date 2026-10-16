//! Facts about files that `check` needs and can't compute without them.
//! Whoever has the files makes these (today `scripts/film/cut.py measure`;
//! later the job runners), and `check` stays pure.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Measured {
    /// The frame size the checks are for (the cut's own size).
    pub size: Option<[u32; 2]>,
    /// A missing clip is a note, not a problem (a preview with placeholders).
    pub allow_missing: bool,
    /// By `src` exactly as the document writes it.
    pub sources: BTreeMap<String, Source>,
    /// Title layouts, by shot name.
    pub titles: BTreeMap<String, Title>,
    /// The caption height in pixels at this size.
    pub caption_px: Option<u32>,
    /// Each caption's layout, by its text (layout depends only on the text,
    /// the cut's caption style and the size).
    pub captions: BTreeMap<String, CaptionLayout>,
    /// Voiceover lengths in seconds, by `src`; `None` when the file is missing.
    pub audio: BTreeMap<String, Option<f64>>,
    /// The music's length; `None` when the file is missing.
    pub music: Option<Option<f64>>,
    /// Whether paths exist (overlays, shot inputs, scripts), by path as written.
    pub files: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Source {
    pub exists: bool,
    /// The path with `~` expanded, as messages name a missing file.
    pub path: String,
    /// The file name, for messages.
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub fps: f64,
    pub dur: f64,
    pub audio: bool,
    pub beats: Vec<Beat>,
    pub stills: Vec<Span>,
    pub speech: Option<Speech>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Beat {
    pub from: f64,
    pub to: f64,
    pub what: String,
    pub enter_ok: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speech {
    pub words: Vec<Word>,
    pub quiet: Vec<Span>,
    pub duration: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub w: String,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Title {
    /// Set when the title's `under` clip is missing (its path).
    pub under_missing: Option<String>,
    pub lines: Vec<TitleLine>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TitleLine {
    pub text: String,
    pub px: u32,
    /// x0, y0, x1, y1
    #[serde(rename = "box")]
    pub bounds: [f64; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptionLayout {
    pub lines: u32,
    /// x, y, width, height
    #[serde(rename = "box")]
    pub bounds: [f64; 4],
}
