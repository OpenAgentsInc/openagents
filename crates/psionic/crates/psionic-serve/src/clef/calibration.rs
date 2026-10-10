//! Calibration maps and hidden-row export for the Clef lane.
//!
//! **Calibration** (`--decision-calibration FILE`, roadmap X1, #11216). A
//! map is a versioned JSON file fitted offline on a corpus's calibration
//! partition. It names the one `noul` question it applies to (by its exact
//! instruction text) and the head it was fitted against; a map fitted on
//! another head is refused at load, because its numbers describe a different
//! model. The served answer carries the calibrated probability, the raw one
//! stays in the `psionic.raw` block, and `psionic.calibration_digest` sits
//! beside `head_digest` so a caller can tell which map produced a number.
//!
//! **Row export** (`--decision-export-rows DIR`, roadmap X2a, #11217). For
//! every question of every decision, the head's pooled inputs are appended
//! to `DIR/rows.f16` (little-endian f16) with one JSON line in
//! `DIR/rows.jsonl` saying where they are. A trainer reads the two files; it
//! does not link this crate. Rows per question, each `hidden_size` wide:
//! `last` (the normalized last hidden row, the head's global vector),
//! `question` (the mean normalized hidden row over the question span), then
//! one `option:<id>` mean per option in prompt order.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The schema tag a calibration map carries.
pub const CALIBRATION_SCHEMA: &str = "openagents.clef.calibration.v1";

/// How a raw probability is rescaled.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalibrationMap {
    /// `σ(a · logit(p) + b)`.
    Platt { a: f64, b: f64 },
    /// `σ(logit(p) / t)`.
    Temperature { t: f64 },
}

impl CalibrationMap {
    /// The calibrated probability of a raw `noul` probability.
    #[must_use]
    pub fn apply(&self, p: f64) -> f64 {
        let p = p.clamp(1e-6, 1.0 - 1e-6);
        let logit = (p / (1.0 - p)).ln();
        let z = match *self {
            Self::Platt { a, b } => a * logit + b,
            Self::Temperature { t } => logit / t,
        };
        1.0 / (1.0 + (-z).exp())
    }
}

#[derive(Deserialize)]
struct CalibrationFile {
    schema: String,
    name: String,
    version: u32,
    question: CalibrationQuestion,
    map: CalibrationMap,
    #[serde(default)]
    head_digest: Option<String>,
}

#[derive(Deserialize)]
struct CalibrationQuestion {
    #[serde(rename = "type")]
    kind: String,
    instructions: String,
}

/// A loaded calibration map.
#[derive(Clone, Debug)]
pub struct ClefCalibration {
    /// `name@version`.
    pub id: String,
    /// `sha256:` over the file's bytes.
    pub digest: String,
    /// The exact `noul` instruction text it applies to.
    pub instructions: String,
    /// The head it was fitted against, when the file names one.
    pub head_digest: Option<String>,
    /// The map.
    pub map: CalibrationMap,
}

impl ClefCalibration {
    /// Reads and checks a map file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_bytes(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Parses a map document.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let file: CalibrationFile =
            serde_json::from_slice(bytes).map_err(|error| format!("not a calibration map: {error}"))?;
        if file.schema != CALIBRATION_SCHEMA {
            return Err(format!(
                "schema is `{}`; expected `{CALIBRATION_SCHEMA}`",
                file.schema
            ));
        }
        if file.question.kind != "noul" {
            return Err(format!(
                "only noul questions are calibrated; this map names `{}`",
                file.question.kind
            ));
        }
        match file.map {
            CalibrationMap::Platt { a, b } if !(a.is_finite() && b.is_finite() && a > 0.0) => {
                return Err(String::from("a Platt map needs finite a > 0 and finite b"));
            }
            CalibrationMap::Temperature { t } if !(t.is_finite() && t > 0.0) => {
                return Err(String::from("a temperature must be finite and positive"));
            }
            _ => {}
        }
        Ok(Self {
            id: format!("{}@{}", file.name, file.version),
            digest: format!("sha256:{}", hex::encode(Sha256::digest(bytes))),
            instructions: file.question.instructions,
            head_digest: file.head_digest,
            map: file.map,
        })
    }
}

/// The head's pooled inputs for one decision, captured for export.
#[derive(Clone, Debug, Default)]
pub struct HeadInputs {
    /// Span means: one per question, then one per option in prompt order.
    pub span_means: Vec<Vec<f32>>,
    /// The normalized last hidden row.
    pub last: Vec<f32>,
}

struct ExportFiles {
    index: File,
    rows: File,
    offset: u64,
}

/// Appends head inputs to `DIR/rows.f16` and `DIR/rows.jsonl`.
pub struct RowExport {
    dir: PathBuf,
    files: Mutex<ExportFiles>,
}

impl std::fmt::Debug for RowExport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowExport").field("dir", &self.dir).finish_non_exhaustive()
    }
}

impl RowExport {
    /// Opens (or continues) an export directory.
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        let open = |name: &str| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(name))
                .map_err(|error| format!("{}: {error}", dir.join(name).display()))
        };
        let rows = open("rows.f16")?;
        let offset = rows
            .metadata()
            .map_err(|error| format!("{}: {error}", dir.display()))?
            .len();
        Ok(Self {
            dir: dir.to_path_buf(),
            files: Mutex::new(ExportFiles { index: open("rows.jsonl")?, rows, offset }),
        })
    }

    /// Writes one decision's rows. `questions` holds `(id, option ids,
    /// logits)` in prompt order; `provenance` is copied onto every line.
    pub fn write(
        &self,
        request_sha256: &str,
        inputs: &HeadInputs,
        questions: &[(String, Vec<String>, Vec<f32>)],
        provenance: &Value,
    ) -> Result<(), String> {
        let width = inputs.last.len();
        let mut files = self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut option_index = questions.len();
        for (question_index, (id, option_ids, logits)) in questions.iter().enumerate() {
            let mut names = vec![String::from("last"), String::from("question")];
            let mut block: Vec<&[f32]> = vec![&inputs.last, &inputs.span_means[question_index]];
            for option in option_ids {
                names.push(format!("option:{option}"));
                block.push(&inputs.span_means[option_index]);
                option_index += 1;
            }
            let mut bytes = Vec::with_capacity(block.len() * width * 2);
            for row in &block {
                for value in row.iter() {
                    bytes.extend_from_slice(&half::f16::from_f32(*value).to_le_bytes());
                }
            }
            if let Err(error) = files.rows.write_all(&bytes) {
                // a partial write would shift every later offset: cut it back
                let keep = files.offset;
                let _ = files.rows.set_len(keep);
                return Err(format!("row export: {error}"));
            }
            let line = json!({
                "request_sha256": request_sha256,
                "question": id,
                "question_index": question_index,
                "offset": files.offset,
                "width": width,
                "rows": names,
                "dtype": "f16le",
                "logits": logits,
                "psionic": provenance,
            });
            files.offset += bytes.len() as u64;
            writeln!(files.index, "{line}").map_err(|error| format!("row export: {error}"))?;
        }
        files.rows.flush().map_err(|error| format!("row export: {error}"))?;
        files.index.flush().map_err(|error| format!("row export: {error}"))
    }
}

/// `sha256:` hex of a request body, the key a client joins exported rows on.
#[must_use]
pub fn request_digest(body: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(body.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_doc(kind: &str) -> String {
        format!(
            r#"{{"schema":"{CALIBRATION_SCHEMA}","name":"file-relevance","version":1,
               "question":{{"type":"noul","instructions":"Is this file relevant?"}},
               "map":{kind},"head_digest":"sha256:abc"}}"#
        )
    }

    #[test]
    fn platt_and_temperature_maps_apply_and_digest() {
        let platt = ClefCalibration::from_bytes(map_doc(r#"{"kind":"platt","a":1.0,"b":0.0}"#).as_bytes())
            .expect("platt");
        assert!((platt.map.apply(0.3) - 0.3).abs() < 1e-9, "identity Platt keeps p");
        assert_eq!(platt.id, "file-relevance@1");
        assert!(platt.digest.starts_with("sha256:"));
        let shifted = ClefCalibration::from_bytes(map_doc(r#"{"kind":"platt","a":1.0,"b":1.0}"#).as_bytes())
            .expect("shifted");
        assert!(shifted.map.apply(0.3) > 0.3, "a positive bias raises p");
        let temp = ClefCalibration::from_bytes(map_doc(r#"{"kind":"temperature","t":2.0}"#).as_bytes())
            .expect("temperature");
        assert!(temp.map.apply(0.9) < 0.9 && temp.map.apply(0.1) > 0.1, "t > 1 softens");
    }

    #[test]
    fn bad_maps_are_refused() {
        assert!(ClefCalibration::from_bytes(map_doc(r#"{"kind":"platt","a":-1.0,"b":0.0}"#).as_bytes()).is_err());
        assert!(ClefCalibration::from_bytes(map_doc(r#"{"kind":"temperature","t":0.0}"#).as_bytes()).is_err());
        let wrong = map_doc(r#"{"kind":"platt","a":1.0,"b":0.0}"#).replace("\"noul\"", "\"choice\"");
        assert!(ClefCalibration::from_bytes(wrong.as_bytes()).is_err());
    }

    #[test]
    fn export_writes_rows_and_index() {
        let dir = std::env::temp_dir().join(format!("clef-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let export = RowExport::open(&dir).expect("open");
        let inputs = HeadInputs {
            span_means: vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]],
            last: vec![0.5, -0.5],
        };
        export
            .write(
                "sha256:x",
                &inputs,
                &[(String::from("q"), vec![String::from("true"), String::from("false")], vec![0.1, -0.1])],
                &json!({}),
            )
            .expect("write");
        let rows = std::fs::read(dir.join("rows.f16")).expect("rows");
        assert_eq!(rows.len(), 4 * 2 * 2, "last, question, two options, width 2, f16");
        let first = half::f16::from_le_bytes([rows[0], rows[1]]).to_f32();
        assert!((first - 0.5).abs() < 1e-3);
        let index = std::fs::read_to_string(dir.join("rows.jsonl")).expect("index");
        let line: Value = serde_json::from_str(index.trim()).expect("json");
        assert_eq!(line["rows"][2], "option:true");
        assert_eq!(line["offset"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
