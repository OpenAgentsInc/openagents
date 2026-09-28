//! What the seam answered about a recorded run, kept beside it.
//!
//! The seam is asked once, over the windows a replay of the run finds
//! ambiguous, and every answer is written here with the row it was asked
//! about. The scorer then reads the file instead of the network, so a
//! floor that moves is scored against the same answers as often as you
//! like, and the owner waves once.
//!
//! The file is JSON lines, one [`Answer`] a line, and it sits beside the
//! run as `<run>.answers.jsonl`.

use std::path::{Path, PathBuf};

use coder_hands::judge::ReportView;
use serde::{Deserialize, Serialize};

use crate::run::Phase;

/// One window the seam was asked about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// The run row the window ended on, which ties the answer to a cue.
    pub row: usize,
    /// The cue that row belongs to.
    pub cue: usize,
    /// That cue's label.
    pub label: String,
    /// Which part of the cue the row arrived in.
    pub phase: Phase,
    /// Frames the window carried.
    pub window: usize,
    /// The rules' label on that row.
    pub pose: String,
    /// The margin that label was decided on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f32>,
    /// What the rules did on that row, by act word.
    pub rules: Vec<String>,
    /// How long the round trip took.
    pub elapsed_ms: u64,
    /// Whether it finished inside the seam's deadline.
    pub met_deadline: bool,
    /// The answer, when one arrived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportView>,
    /// Why no answer arrived, when the request failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

impl Answer {
    /// The answer as the file carries it, with its newline.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = serde_json::to_string(self)
            .unwrap_or_else(|error| format!("{{\"broken\":\"{error}\"}}"));
        text.push('\n');
        text
    }
}

/// The file that sits beside a run.
#[must_use]
pub fn beside(run: &Path) -> PathBuf {
    let mut name = run.file_name().unwrap_or_default().to_os_string();
    name.push(".answers.jsonl");
    run.with_file_name(name)
}

/// Every answer in the text of a file.
///
/// # Errors
///
/// Returns the sentence that names the line that would not read.
pub fn parse(text: &str) -> Result<Vec<Answer>, String> {
    let mut answers = Vec::new();
    for (number, line) in text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
    {
        let answer: Answer =
            serde_json::from_str(line).map_err(|error| format!("line {}: {error}", number + 1))?;
        answers.push(answer);
    }
    Ok(answers)
}

/// Every answer in a file, and none when the file is not there, because
/// a run that was never asked about scores on the rules alone.
///
/// # Errors
///
/// Returns the sentence that names the file or the line that would not
/// read.
pub fn read(path: &Path) -> Result<Vec<Answer>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    parse(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Writes every answer to `path`.
///
/// # Errors
///
/// Returns the sentence that names the file that would not open.
pub fn write(path: &Path, answers: &[Answer]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let text: String = answers.iter().map(Answer::render).collect();
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}
