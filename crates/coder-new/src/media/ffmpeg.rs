//! ffmpeg and ffprobe on this computer: finding them, running them with
//! cancel and a time limit, and reading streams as numbers.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::sync::Series;

/// The longest one ffmpeg run may take.
const RUN_LIMIT: Duration = Duration::from_secs(30 * 60);
/// The most decoded bytes one run may hand back (a 30 minute clip's
/// audio at the analysis rate is about 29 MB).
const OUTPUT_LIMIT: usize = 64 * 1024 * 1024;
/// The sample rate audio is decoded at for analysis.
pub const ANALYSIS_RATE: u32 = 8000;

/// Where a program is: on `PATH`, or in the usual install folders that a
/// non-login shell can miss.
pub fn find(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(
            ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
                .into_iter()
                .map(PathBuf::from),
        )
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The ffmpeg pair, or how to install it.
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Tools {
    /// # Errors
    /// ffmpeg or ffprobe is not installed; the message has the command.
    pub fn locate() -> Result<Self, String> {
        match (find("ffmpeg"), find("ffprobe")) {
            (Some(ffmpeg), Some(ffprobe)) => Ok(Self { ffmpeg, ffprobe }),
            _ => Err(format!(
                "ffmpeg is not installed on this computer. Install it with `{}`, then try again.",
                install_hint()
            )),
        }
    }
}

fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install ffmpeg"
    } else {
        "sudo apt-get install -y ffmpeg"
    }
}

/// Run `program` with `arguments` in `cwd` and return its standard output.
///
/// # Errors
/// It could not start, was canceled, ran too long, printed too much, or
/// exited with a failure (the message ends with its last error lines).
pub async fn run(
    program: &Path,
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Canceled before it started.".into());
    }
    let name = program.file_name().map_or_else(
        || program.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let mut command = tokio::process::Command::new(program);
    command
        .args(arguments)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = command
        .spawn()
        .map_err(|error| format!("{name} could not start: {error}"))?;
    let waiting = child.wait_with_output();
    tokio::pin!(waiting);
    let started = Instant::now();
    let output = loop {
        tokio::select! {
            finished = &mut waiting => break finished,
            () = tokio::time::sleep(Duration::from_millis(100)) => {
                // Dropping the future kills the child.
                if cancel.load(Ordering::Relaxed) {
                    return Err(format!("{name} was canceled."));
                }
                if started.elapsed() > RUN_LIMIT {
                    return Err(format!("{name} ran longer than {} minutes and was stopped.", RUN_LIMIT.as_secs() / 60));
                }
            }
        }
    }
    .map_err(|error| format!("{name} failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(6).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(format!(
            "{name} exited with {}: {}",
            output.status,
            tail.join(" / ")
        ));
    }
    if output.stdout.len() > OUTPUT_LIMIT {
        return Err(format!(
            "{name} produced more than {} MB; measure a shorter stretch with max_seconds.",
            OUTPUT_LIMIT / (1024 * 1024)
        ));
    }
    Ok(output.stdout)
}

/// ffprobe's full answer for `path`.
///
/// # Errors
/// ffprobe failed or printed something that is not JSON.
pub async fn probe_raw(
    tools: &Tools,
    path: &Path,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let words = strings(&[
        "-v",
        "error",
        "-show_format",
        "-show_streams",
        "-of",
        "json",
    ])
    .into_iter()
    .chain([path.display().to_string()])
    .collect::<Vec<_>>();
    let stdout = run(&tools.ffprobe, &words, cwd, cancel).await?;
    serde_json::from_slice(&stdout).map_err(|error| format!("ffprobe printed no JSON: {error}"))
}

/// The parts of ffprobe's answer a person needs: container, duration,
/// and per stream its kind, codec, size or rate, and timing.
#[must_use]
pub fn summarize(raw: &Value) -> Value {
    let format = &raw["format"];
    let streams: Vec<Value> = raw["streams"]
        .as_array()
        .map(|streams| {
            streams
                .iter()
                .map(|stream| {
                    let mut summary = json!({
                        "index": stream["index"],
                        "kind": stream["codec_type"],
                        "codec": stream["codec_name"],
                        "start_seconds": number(&stream["start_time"]),
                        "duration_seconds": stream_duration(stream),
                    });
                    match stream["codec_type"].as_str() {
                        Some("video") => {
                            summary["width"] = stream["width"].clone();
                            summary["height"] = stream["height"].clone();
                            summary["frame_rate"] = json!(frame_rate(stream));
                            summary["frames"] = json!(number(&stream["nb_frames"]));
                        }
                        Some("audio") => {
                            summary["sample_rate"] = json!(number(&stream["sample_rate"]));
                            summary["channels"] = stream["channels"].clone();
                        }
                        _ => {}
                    }
                    if let Some(language) = stream["tags"]["language"].as_str() {
                        summary["language"] = json!(language);
                    }
                    summary
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "container": format["format_name"],
        "duration_seconds": number(&format["duration"]),
        "start_seconds": number(&format["start_time"]),
        "size_bytes": number(&format["size"]),
        "bit_rate": number(&format["bit_rate"]),
        "streams": streams,
    })
}

/// A number that ffprobe printed as a string or a number.
#[must_use]
pub fn number(value: &Value) -> Option<f64> {
    let parsed: Option<f64> = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    };
    parsed.filter(|value| value.is_finite())
}

/// A stream's duration: its own field, or a Matroska `DURATION` tag.
fn stream_duration(stream: &Value) -> Option<f64> {
    number(&stream["duration"]).or_else(|| {
        let tag = stream["tags"]["DURATION"].as_str()?;
        let mut parts = tag.split(':');
        let hours: f64 = parts.next()?.parse().ok()?;
        let minutes: f64 = parts.next()?.parse().ok()?;
        let seconds: f64 = parts.next()?.parse().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    })
}

/// "30000/1001" as 29.97; average rate first, then the base rate.
#[must_use]
pub fn frame_rate(stream: &Value) -> Option<f64> {
    let ratio = |value: &Value| {
        let (top, bottom) = value.as_str()?.split_once('/')?;
        let (top, bottom): (f64, f64) = (top.parse().ok()?, bottom.parse().ok()?);
        (bottom > 0.0 && top > 0.0).then(|| top / bottom)
    };
    ratio(&stream["avg_frame_rate"]).or_else(|| ratio(&stream["r_frame_rate"]))
}

/// The first stream of `kind` ("video" or "audio") in a probe answer.
#[must_use]
pub fn first_stream<'a>(raw: &'a Value, kind: &str) -> Option<&'a Value> {
    raw["streams"]
        .as_array()?
        .iter()
        .find(|stream| stream["codec_type"] == kind && stream["disposition"]["attached_pic"] != 1)
}

/// The whole clip's duration from a probe answer.
#[must_use]
pub fn duration(raw: &Value) -> Option<f64> {
    number(&raw["format"]["duration"]).or_else(|| {
        raw["streams"]
            .as_array()?
            .iter()
            .filter_map(stream_duration)
            .reduce(f64::max)
    })
}

/// The brightness of every frame of the first video stream, 0 to 255.
///
/// # Errors
/// The clip has no video, or ffmpeg failed.
pub async fn brightness(
    tools: &Tools,
    path: &Path,
    raw: &Value,
    max_seconds: f64,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Series, String> {
    let stream =
        first_stream(raw, "video").ok_or_else(|| format!("{} has no picture.", path.display()))?;
    let rate = frame_rate(stream).ok_or("The picture's frame rate is unknown.")?;
    let words = decode_words(
        path,
        max_seconds,
        &[
            "-map",
            "0:v:0",
            "-vf",
            "scale=1:1:flags=area,format=gray",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-",
        ],
    );
    let bytes = run(&tools.ffmpeg, &words, cwd, cancel).await?;
    Ok(Series {
        start: number(&stream["start_time"]).unwrap_or(0.0),
        rate,
        values: bytes.into_iter().map(f32::from).collect(),
    })
}

/// The first audio stream as 16-bit mono samples at [`ANALYSIS_RATE`],
/// and the time of its first sample.
///
/// # Errors
/// The clip has no audio, or ffmpeg failed.
pub async fn samples(
    tools: &Tools,
    path: &Path,
    raw: &Value,
    max_seconds: f64,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(Vec<i16>, f64), String> {
    let stream =
        first_stream(raw, "audio").ok_or_else(|| format!("{} has no sound.", path.display()))?;
    let rate = ANALYSIS_RATE.to_string();
    let words = decode_words(
        path,
        max_seconds,
        &[
            "-map", "0:a:0", "-ac", "1", "-ar", &rate, "-f", "s16le", "-",
        ],
    );
    let bytes = run(&tools.ffmpeg, &words, cwd, cancel).await?;
    let samples = bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok((samples, number(&stream["start_time"]).unwrap_or(0.0)))
}

fn decode_words(path: &Path, max_seconds: f64, output: &[&str]) -> Vec<String> {
    let mut words = strings(&["-v", "error", "-nostdin", "-i"]);
    words.push(path.display().to_string());
    words.extend(strings(&["-t", &format!("{max_seconds:.3}")]));
    words.extend(strings(output));
    words
}

#[must_use]
pub fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_answers_are_summarized_with_numbers() {
        let raw = json!({
            "format": {"format_name": "matroska,webm", "duration": "20.000000", "start_time": "0.000000", "size": "1000"},
            "streams": [
                {"index": 0, "codec_type": "video", "codec_name": "mpeg4", "width": 32, "height": 32,
                 "avg_frame_rate": "30000/1001", "r_frame_rate": "30000/1001", "start_time": "0.000000",
                 "tags": {"DURATION": "00:00:20.020000000"}},
                {"index": 1, "codec_type": "audio", "codec_name": "pcm_s16le", "sample_rate": "48000",
                 "channels": 1, "start_time": "0.021000", "tags": {"language": "eng"}}
            ]
        });
        let summary = summarize(&raw);
        assert_eq!(summary["duration_seconds"], json!(20.0));
        let video = &summary["streams"][0];
        assert_eq!(video["kind"], "video");
        assert!((video["frame_rate"].as_f64().unwrap() - 29.97).abs() < 0.01);
        assert!((video["duration_seconds"].as_f64().unwrap() - 20.02).abs() < 1e-9);
        let audio = &summary["streams"][1];
        assert_eq!(audio["sample_rate"], json!(48000.0));
        assert_eq!(audio["start_seconds"], json!(0.021));
        assert_eq!(audio["language"], "eng");
        assert_eq!(duration(&raw), Some(20.0));
        assert!(first_stream(&raw, "audio").is_some());
        assert!(first_stream(&raw, "subtitle").is_none());
    }

    #[test]
    fn decoding_reads_only_the_asked_stretch() {
        let words = decode_words(Path::new("/a b.mkv"), 90.0, &["-f", "s16le", "-"]);
        assert_eq!(
            words,
            [
                "-v", "error", "-nostdin", "-i", "/a b.mkv", "-t", "90.000", "-f", "s16le", "-"
            ]
        );
    }
}
