//! The file one recorded run writes, and the gestures it asks for.
//!
//! A run is JSON lines. The first line is the [`Header`]: when the run
//! started, the camera it read, that camera's capture size, and the
//! script it followed. Every line after it is a [`Row`]: the cue that was
//! on screen, the phase of that cue, and the landmark line the camera
//! published, exactly as it arrived. The rows are every frame the run
//! saw, in order, so a replay reads the same sequence the desk read.
//!
//! The label on a row is what you were asked to do when the frame
//! arrived, which is what the scorer counts a decision against. A frame
//! recorded while the prompt counted you in carries [`Phase::Ready`]
//! instead, and the scorer reads those as rest.
//!
//! The header also names the [`Source`] the frames came from. A run
//! recorded from the CoderOS camera daemon and one recorded from a Mac's
//! own tracker are two measurements, not one, so the scorer prints the
//! source rather than letting them be read as the same thing. A run
//! written before the field existed carries none.

use coder_hands::wire::Line;
use serde::{Deserialize, Serialize};

/// The word the header carries so a reader knows what it opened.
pub const KIND: &str = "hands-run";

/// The format's version. A reader refuses a version it does not know.
pub const VERSION: u32 = 1;

/// The cue a run carries when nobody was prompted: a file of raw
/// landmark lines, recorded while a person used the machine rather than
/// while a script asked them for one gesture at a time. The scorer
/// groups every frame of such a run under this one label and owes it no
/// act, because nothing in the file says what the hand meant.
pub const UNLABELLED: &str = "unlabelled";

/// The camera a run was recorded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The CoderOS camera daemon, read off its hands socket.
    Daemon,
    /// A Mac's own camera, through AVFoundation and Vision.
    Vision,
}

impl Source {
    /// What the report calls it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Source::Daemon => "the CoderOS camera daemon",
            Source::Vision => "this machine's own camera",
        }
    }
}

/// One gesture the recorder asks for: the label the rows carry, the
/// prompt you read, and the acts the rules owe when you make it.
#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    /// The label a row carries, and the row the report groups by.
    pub label: &'static str,
    /// What the recorder prints when the cue starts.
    pub prompt: &'static str,
    /// The acts the rules owe this gesture, by the word
    /// `coder_hands::gestures::Act::word` gives, in the order they come.
    pub owed: &'static [&'static str],
}

/// The gestures a run asks for, in the order it asks for them. Each one
/// is a gesture the desk's rules act on, and the acts beside it are what the
/// scorer counts.
pub const GESTURES: &[Gesture] = &[
    Gesture {
        label: "point",
        prompt: "Point at the screen and move your finger.",
        owed: &["point"],
    },
    Gesture {
        label: "pinch",
        prompt: "Pinch your thumb and finger together, hold, then open your hand.",
        owed: &["press", "release"],
    },
    Gesture {
        label: "swipe_left",
        prompt: "Hold your hand flat and sweep it to your left.",
        owed: &["swipe left"],
    },
    Gesture {
        label: "fist",
        prompt: "Close your hand into a fist and hold it until the prompt changes.",
        owed: &["escape"],
    },
];

/// The acts that command the desk. An act on this list that the cue does
/// not owe is one the desk took and you did not ask for; a pointer move
/// is not on it, because a hand on the way to a gesture moves the
/// pointer and costs nothing.
pub const COMMANDS: &[&str] = &["press", "release", "swipe left", "swipe right", "escape"];

/// The seconds a cue counts you in for, before the frames it scores.
pub const READY_SECONDS: f64 = 3.0;

/// The seconds a cue records for.
pub const RECORD_SECONDS: f64 = 4.0;

/// How many times a run asks for the whole gesture list by default.
pub const PASSES: usize = 2;

/// The gesture with this label, or `None` when the run carries a label
/// the table does not name.
#[must_use]
pub fn gesture(label: &str) -> Option<&'static Gesture> {
    GESTURES.iter().find(|gesture| gesture.label == label)
}

/// One cue of the script: the gesture, the prompt, and how long each of
/// its two phases runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    /// The gesture's label, which [`gesture`] reads back.
    pub label: String,
    /// The prompt the recorder printed.
    pub prompt: String,
    /// The seconds of count-in before the cue.
    pub ready: f64,
    /// The seconds the cue recorded for.
    pub seconds: f64,
}

/// Which part of a cue a row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// The count-in, which the scorer reads as rest.
    Ready,
    /// The cue itself, which the scorer counts.
    Record,
}

impl Phase {
    /// The word the report prints.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Phase::Ready => "ready",
            Phase::Record => "record",
        }
    }
}

/// The first line of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// [`KIND`], so a reader knows what it opened.
    pub kind: String,
    /// [`VERSION`].
    pub version: u32,
    /// Seconds since the Unix epoch when the run started.
    pub started: f64,
    /// The camera the run was recorded from. A run written before the
    /// field existed carries none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// The camera's capture size, such as `1280x720`, when the session
    /// named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,
    /// The frame's width over its height, which the rules measure a palm
    /// against.
    pub aspect: f32,
    /// The script the run followed, cue by cue.
    pub script: Vec<Cue>,
}

impl Header {
    /// A header for a run of `script` that starts at `started`.
    #[must_use]
    pub fn new(
        started: f64,
        source: Option<Source>,
        capture: Option<String>,
        aspect: f32,
        script: Vec<Cue>,
    ) -> Header {
        Header {
            kind: KIND.to_string(),
            version: VERSION,
            started,
            source,
            capture,
            aspect,
            script,
        }
    }

    /// The header as the file carries it, with its newline.
    #[must_use]
    pub fn render(&self) -> String {
        render(self)
    }
}

/// One frame of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// Which cue of the script was on screen, counting from zero.
    pub cue: usize,
    /// That cue's label.
    pub label: String,
    /// Which part of the cue the frame arrived in.
    pub phase: Phase,
    /// The landmark line the camera daemon published.
    pub line: Line,
}

impl Row {
    /// The row as the file carries it, with its newline.
    #[must_use]
    pub fn render(&self) -> String {
        render(self)
    }
}

/// One value as a JSON line. A value that does not render leaves an
/// object that says so, which a reader then refuses by name rather than
/// reading a truncated file.
fn render<T: Serialize>(value: &T) -> String {
    let mut text =
        serde_json::to_string(value).unwrap_or_else(|error| format!("{{\"broken\":\"{error}\"}}"));
    text.push('\n');
    text
}

/// A whole run, read back.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The first line.
    pub header: Header,
    /// Every frame after it, in order.
    pub rows: Vec<Row>,
}

impl Run {
    /// One run read from the text of a file.
    ///
    /// # Errors
    ///
    /// Returns the sentence that names the line that would not read.
    pub fn parse(text: &str) -> Result<Run, String> {
        let mut lines = text
            .lines()
            .enumerate()
            .filter(|(_, row)| !row.trim().is_empty());
        let (number, first) = lines.next().ok_or_else(|| "the run is empty".to_string())?;
        if serde_json::from_str::<Header>(first).is_err() {
            return Run::parse_lines(text);
        }
        let header: Header =
            serde_json::from_str(first).map_err(|error| format!("line {}: {error}", number + 1))?;
        if header.kind != KIND {
            return Err(format!(
                "line {}: this is a {} file, not a {KIND} file",
                number + 1,
                header.kind
            ));
        }
        if header.version != VERSION {
            return Err(format!(
                "line {}: the run is version {}, and this reader knows version {VERSION}",
                number + 1,
                header.version
            ));
        }
        let mut rows = Vec::new();
        for (number, text) in lines {
            let row: Row = serde_json::from_str(text)
                .map_err(|error| format!("line {}: {error}", number + 1))?;
            rows.push(row);
        }
        Ok(Run { header, rows })
    }

    /// A run built from a file of raw landmark lines, the shape the
    /// camera daemon publishes and a recorder writes when it is watching
    /// rather than prompting. Every frame lands under [`UNLABELLED`] in
    /// [`Phase::Record`], so the scorer reads the whole file as one
    /// stretch and reports what the rules decided without claiming to
    /// know what the hand meant.
    ///
    /// A line carrying the same hands as the line before it is dropped.
    /// A recorder that polls faster than the camera answers writes the
    /// same reading many times over, and the rate read off those
    /// timestamps is the poll loop's rather than the camera's: one such
    /// recording of 2026-09-18 holds 1.7 million lines for 1,036
    /// readings and reads as 148,000 frames a second.
    ///
    /// # Errors
    ///
    /// Returns the sentence that names the line that would not read.
    pub fn parse_lines(text: &str) -> Result<Run, String> {
        let mut rows: Vec<Row> = Vec::new();
        let mut last: Option<String> = None;
        for (number, text) in text
            .lines()
            .enumerate()
            .filter(|(_, row)| !row.trim().is_empty())
        {
            let line: Line = serde_json::from_str(text).map_err(|error| {
                format!(
                    "line {}: this is neither a {KIND} header nor a landmark line: {error}",
                    number + 1
                )
            })?;
            let reading = serde_json::to_string(&line.hands).unwrap_or_default();
            if last.as_ref() == Some(&reading) {
                continue;
            }
            last = Some(reading);
            rows.push(Row {
                cue: 0,
                label: UNLABELLED.to_string(),
                phase: Phase::Record,
                line,
            });
        }
        let started = rows.first().map_or(0.0, |row| row.line.timestamp);
        let seconds = rows
            .last()
            .map_or(0.0, |row| row.line.timestamp - started)
            .max(0.0);
        let script = vec![Cue {
            label: UNLABELLED.to_string(),
            prompt: "Nobody was prompted: these are frames of a hand at work.".to_string(),
            ready: 0.0,
            seconds,
        }];
        Ok(Run {
            header: Header::new(started, None, None, 16.0 / 9.0, script),
            rows,
        })
    }

    /// One run read from a file.
    ///
    /// # Errors
    ///
    /// Returns the sentence that names the file or the line that would
    /// not read.
    pub fn read(path: &std::path::Path) -> Result<Run, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Run::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The whole run as the file carries it.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = self.header.render();
        for row in &self.rows {
            text.push_str(&row.render());
        }
        text
    }
}

/// The script a run follows: `passes` times through [`GESTURES`].
#[must_use]
pub fn script(passes: usize) -> Vec<Cue> {
    let mut cues = Vec::new();
    for _ in 0..passes {
        for gesture in GESTURES {
            cues.push(Cue {
                label: gesture.label.to_string(),
                prompt: gesture.prompt.to_string(),
                ready: READY_SECONDS,
                seconds: RECORD_SECONDS,
            });
        }
    }
    cues
}

/// The seconds a script takes, which is what the recorder tells you
/// before it starts.
#[must_use]
pub fn script_seconds(cues: &[Cue]) -> f64 {
    cues.iter().map(|cue| cue.ready + cue.seconds).sum()
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
