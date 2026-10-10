//! Speech to text with whisper on this computer: whisper.cpp
//! (`whisper-cli`) with a ggml model, or OpenAI's `whisper` command.
//! Also a word error rate against a reference text.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use super::ffmpeg::{self, Tools};

/// Models tried in order, best first, when none is named.
const MODEL_PREFERENCE: [&str; 7] = [
    "ggml-large-v3-turbo.bin",
    "ggml-large-v3.bin",
    "ggml-medium.en.bin",
    "ggml-medium.bin",
    "ggml-small.en.bin",
    "ggml-base.en.bin",
    "ggml-base.bin",
];

const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

/// One stretch of speech.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Folders a ggml model is usually kept in.
pub(crate) fn model_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        folders.extend([
            home.join(".cache/whisper.cpp"),
            home.join(".cache/whisper"),
            home.join(".openagents/models/whisper"),
            home.join("models"),
        ]);
    }
    folders.extend(
        [
            "/opt/homebrew/share/whisper-cpp/models",
            "/opt/homebrew/share/whisper-cpp",
            "/usr/local/share/whisper-cpp/models",
            "/usr/local/share/whisper-cpp",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    folders
}

/// The ggml model to use: the one named, `WHISPER_MODEL`, or the best
/// one found in the usual folders. Test-only models are skipped.
#[must_use]
pub fn find_model(named: Option<&Path>, folders: &[PathBuf]) -> Option<PathBuf> {
    if let Some(named) = named {
        return named.is_file().then(|| named.to_path_buf());
    }
    if let Some(path) = std::env::var_os("WHISPER_MODEL").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    for name in MODEL_PREFERENCE {
        if let Some(path) = folders
            .iter()
            .map(|dir| dir.join(name))
            .find(|p| p.is_file())
        {
            return Some(path);
        }
    }
    folders
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("ggml-") && name.ends_with(".bin") && !name.contains("test")
                })
        })
}

/// Transcribe the first audio stream of `path`.
///
/// # Errors
/// No whisper or model on this computer (the message says how to get
/// one), or a failed run.
pub async fn transcribe(
    tools: &Tools,
    path: &Path,
    model: Option<&Path>,
    language: &str,
    cwd: &Path,
    scratch: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(Vec<Segment>, String), String> {
    // whisper wants 16 kHz mono 16-bit WAV.
    let wav = scratch.join("speech.wav");
    let mut words = ffmpeg::strings(&["-v", "error", "-nostdin", "-y", "-i"]);
    words.push(path.display().to_string());
    words.extend(ffmpeg::strings(&[
        "-map",
        "0:a:0",
        "-ac",
        "1",
        "-ar",
        "16000",
        "-c:a",
        "pcm_s16le",
    ]));
    words.push(wav.display().to_string());
    ffmpeg::run(&tools.ffmpeg, &words, cwd, cancel).await?;

    let cpp = ffmpeg::find("whisper-cli").or_else(|| ffmpeg::find("whisper-cpp"));
    if let Some(program) = cpp {
        let model = find_model(model, &model_folders()).ok_or_else(|| {
            format!(
                "whisper.cpp is installed but has no model. Download one with \
                 `mkdir -p ~/.cache/whisper.cpp && curl -L -o ~/.cache/whisper.cpp/ggml-base.en.bin {MODEL_URL}`, \
                 or pass model with a ggml model's path."
            )
        })?;
        let base = scratch.join("speech");
        let mut words = ffmpeg::strings(&["-m"]);
        words.push(model.display().to_string());
        words.extend(ffmpeg::strings(&["-f"]));
        words.push(wav.display().to_string());
        words.extend(ffmpeg::strings(&["-l", language, "-oj", "-np", "-of"]));
        words.push(base.display().to_string());
        ffmpeg::run(&program, &words, cwd, cancel).await?;
        let text = std::fs::read_to_string(base.with_extension("json"))
            .map_err(|error| format!("whisper.cpp wrote no transcript: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("whisper.cpp's transcript is not JSON: {error}"))?;
        let model_name = model
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        return Ok((segments(&value), format!("whisper.cpp {model_name}")));
    }
    if let Some(program) = ffmpeg::find("whisper") {
        let model_name = model
            .and_then(|model| model.to_str())
            .unwrap_or("base")
            .to_owned();
        let mut words = vec![wav.display().to_string()];
        words.extend(ffmpeg::strings(&[
            "--model",
            &model_name,
            "--language",
            language,
            "--output_format",
            "json",
            "--output_dir",
        ]));
        words.push(scratch.display().to_string());
        ffmpeg::run(&program, &words, cwd, cancel).await?;
        let text = std::fs::read_to_string(scratch.join("speech.json"))
            .map_err(|error| format!("whisper wrote no transcript: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("whisper's transcript is not JSON: {error}"))?;
        return Ok((segments(&value), format!("whisper {model_name}")));
    }
    Err(format!(
        "whisper is not installed on this computer. Install whisper.cpp with `{}` and a model with \
         `mkdir -p ~/.cache/whisper.cpp && curl -L -o ~/.cache/whisper.cpp/ggml-base.en.bin {MODEL_URL}`.",
        if cfg!(target_os = "macos") {
            "brew install whisper-cpp"
        } else {
            "pipx install openai-whisper"
        }
    ))
}

/// Segments from either whisper's JSON: whisper.cpp's `transcription`
/// (offsets in milliseconds) or OpenAI whisper's `segments` (seconds).
#[must_use]
pub fn segments(value: &Value) -> Vec<Segment> {
    if let Some(items) = value["transcription"].as_array() {
        return items
            .iter()
            .map(|item| Segment {
                start: item["offsets"]["from"].as_f64().unwrap_or(0.0) / 1000.0,
                end: item["offsets"]["to"].as_f64().unwrap_or(0.0) / 1000.0,
                text: item["text"].as_str().unwrap_or_default().trim().to_owned(),
            })
            .filter(|segment| !segment.text.is_empty())
            .collect();
    }
    value["segments"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| Segment {
                    start: item["start"].as_f64().unwrap_or(0.0),
                    end: item["end"].as_f64().unwrap_or(0.0),
                    text: item["text"].as_str().unwrap_or_default().trim().to_owned(),
                })
                .filter(|segment| !segment.text.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The whole text of `segments`.
#[must_use]
pub fn joined(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lower-case words without punctuation; "country." and "Country" match.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

/// Word error rate of `heard` against `reference`: substitutions,
/// insertions, and deletions over the reference's word count.
#[must_use]
pub fn word_error_rate(reference: &str, heard: &str) -> f64 {
    let (reference, heard) = (words(reference), words(heard));
    if reference.is_empty() {
        return if heard.is_empty() { 0.0 } else { 1.0 };
    }
    let mut previous: Vec<usize> = (0..=heard.len()).collect();
    for (row, expected) in reference.iter().enumerate() {
        let mut current = vec![row + 1; heard.len() + 1];
        for (column, word) in heard.iter().enumerate() {
            let substitution = previous[column] + usize::from(expected != word);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        previous = current;
    }
    previous[heard.len()] as f64 / reference.len() as f64
}

/// The comparison the tool reports when a reference text is given.
#[must_use]
pub fn comparison(reference: &str, heard: &str) -> Value {
    let rate = word_error_rate(reference, heard);
    json!({
        "word_error_rate": (rate * 1000.0).round() / 1000.0,
        "matches": rate <= 0.05,
        "reference_words": words(reference).len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_whisper_formats_read_as_segments() {
        let cpp = json!({"transcription": [
            {"offsets": {"from": 0, "to": 3200}, "text": " And so my fellow Americans,"},
            {"offsets": {"from": 3200, "to": 11000}, "text": " ask not what your country can do for you"},
            {"offsets": {"from": 11000, "to": 11000}, "text": " "}
        ]});
        let read = segments(&cpp);
        assert_eq!(read.len(), 2);
        assert_eq!(read[1].start, 3.2);
        assert_eq!(read[0].text, "And so my fellow Americans,");
        let openai = json!({"text": "hi", "segments": [{"start": 0.5, "end": 1.0, "text": " hi"}]});
        assert_eq!(
            segments(&openai),
            [Segment {
                start: 0.5,
                end: 1.0,
                text: "hi".into()
            }]
        );
        assert_eq!(
            joined(&read),
            "And so my fellow Americans, ask not what your country can do for you"
        );
    }

    #[test]
    fn word_error_rate_ignores_case_and_punctuation() {
        let reference = "And so, my fellow Americans: ask not what your country can do for you.";
        assert_eq!(
            word_error_rate(
                reference,
                "and so my fellow americans ask not what your country can do for you"
            ),
            0.0
        );
        // One substitution and one deletion over 14 words.
        let rate = word_error_rate(
            reference,
            "and so my fellow citizens ask not what your country can do for",
        );
        assert!((rate - 2.0 / 14.0).abs() < 1e-9, "{rate}");
        assert_eq!(comparison(reference, reference)["matches"], true);
        assert_eq!(comparison(reference, "something else")["matches"], false);
        assert_eq!(word_error_rate("", ""), 0.0);
    }

    #[test]
    fn a_named_model_must_exist_and_test_models_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_model(Some(&dir.path().join("none.bin")), &[]), None);
        std::fs::write(dir.path().join("for-tests-ggml-tiny.bin"), b"x").unwrap();
        let folders = [dir.path().to_path_buf()];
        // WHISPER_MODEL may be set on a developer's computer; only check
        // folder search when it is not.
        if std::env::var_os("WHISPER_MODEL").is_none() {
            assert_eq!(find_model(None, &folders), None);
            std::fs::write(dir.path().join("ggml-tiny.en.bin"), b"x").unwrap();
            std::fs::write(dir.path().join("ggml-base.en.bin"), b"x").unwrap();
            assert_eq!(
                find_model(None, &folders),
                Some(dir.path().join("ggml-base.en.bin"))
            );
        }
    }
}
