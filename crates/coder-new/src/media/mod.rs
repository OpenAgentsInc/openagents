//! The `media` tool (#11172): look into video and audio files on the
//! computer Coder runs on (the user's own computer or a Cloud
//! environment), with ffmpeg and whisper installed there.
//!
//! - `probe`: streams, codecs, durations, rates, and start times.
//! - `frames`: a contact sheet of frames at chosen or evenly spaced times,
//!   shown to the model as an image.
//! - `transcribe`: speech to text with whisper on this computer, with
//!   segment times, and a word error rate when a reference text is given.
//! - `av_offset`: how far the audio is from the picture, signed, and how
//!   fast that changes, measured against an independent reference: the
//!   clip's own flashes and beeps (a clap or sync test), or a separate
//!   reference recording. Never the recorder's own sync figure.
//! - `retime`: write a corrected copy that shifts and stretches the audio
//!   by a measured offset and drift, then measures the copy again.
//!
//! Sign convention: a positive offset means the audio comes later than the
//! picture, so `retime` with that offset moves the audio earlier.

pub mod ffmpeg;
pub mod sync;
pub mod transcript;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};

use ffmpeg::Tools;
use sync::{AvOffset, Fit, NoFit};

/// The largest contact sheet shown to the model, in bytes.
pub const LOOK_MAX_BYTES: u64 = 4 * 1024 * 1024;
/// The most frames on one contact sheet.
const MAX_FRAMES: usize = 16;
/// How much of a clip `av_offset` reads by default, in seconds.
const DEFAULT_MAX_SECONDS: f64 = 600.0;
/// The longest stretch `av_offset` reads, in seconds.
const LIMIT_MAX_SECONDS: f64 = 1800.0;
/// Two events closer than this are one event, in seconds.
const REFRACTORY: f64 = 0.25;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    action: Action,
    path: String,
    times: Option<Vec<f64>>,
    count: Option<usize>,
    width: Option<u32>,
    #[serde(default = "yes")]
    look: bool,
    language: Option<String>,
    model: Option<String>,
    reference_text: Option<String>,
    reference: Option<String>,
    max_offset_seconds: Option<f64>,
    tolerance_seconds: Option<f64>,
    max_seconds: Option<f64>,
    output: Option<String>,
    offset_seconds: Option<f64>,
    drift: Option<f64>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Action {
    Probe,
    Frames,
    Transcribe,
    AvOffset,
    Retime,
}

/// The tool's declaration.
#[must_use]
pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"media",
        "description":"Inspect video and audio files on this computer with ffmpeg and whisper. Actions: probe (streams, codecs, durations, frame and sample rates, start times), frames (a contact sheet of frames at the given times, or count evenly spaced ones, shown to you as an image), transcribe (speech to text with whisper here, with segment times; pass reference_text to get a word error rate), av_offset (how far the audio is from the picture and how fast that drifts, signed: positive means the audio is late; measured from the clip's own flashes and beeps such as a clap or sync test, or against a separate reference recording given as reference), retime (write a corrected copy to output that moves the audio by offset_seconds and drift as av_offset reported them, then measure the copy again). Paths are relative to the working directory.",
        "parameters":{"type":"object","properties":{
            "action":{"type":"string","enum":["probe","frames","transcribe","av_offset","retime"]},
            "path":{"type":"string","minLength":1,"maxLength":4096,"description":"The video or audio file."},
            "times":{"type":"array","items":{"type":"number","minimum":0},"maxItems":16,"description":"frames: the times in seconds."},
            "count":{"type":"integer","minimum":1,"maximum":16,"description":"frames: how many evenly spaced frames when times is not given (default 6)."},
            "width":{"type":"integer","minimum":64,"maximum":960,"description":"frames: each frame's width in pixels (default 320)."},
            "look":{"type":"boolean","description":"frames: show you the contact sheet (default true)."},
            "language":{"type":"string","maxLength":16,"description":"transcribe: the spoken language code, or auto (default en)."},
            "model":{"type":"string","maxLength":4096,"description":"transcribe: a whisper.cpp ggml model path, or an OpenAI whisper model name."},
            "reference_text":{"type":"string","maxLength":65536,"description":"transcribe: the text the speech should be, to compare against."},
            "reference":{"type":"string","maxLength":4096,"description":"av_offset and retime: an independent reference recording of the same events. Without it the clip's own flashes are the picture's reference for its beeps."},
            "max_offset_seconds":{"type":"number","exclusiveMinimum":0,"maximum":30,"description":"av_offset: the largest offset to look for (default 1). Keep it under half the gap between sync events."},
            "tolerance_seconds":{"type":"number","exclusiveMinimum":0,"maximum":1,"description":"av_offset: how far an event may sit from the fitted line (default about one frame)."},
            "max_seconds":{"type":"number","exclusiveMinimum":0,"maximum":1800,"description":"av_offset: how much of the clip to read (default 600)."},
            "output":{"type":"string","maxLength":4096,"description":"retime: the new file; it must not exist yet."},
            "offset_seconds":{"type":"number","minimum":-60,"maximum":60,"description":"retime: av_offset's offset_seconds (positive: audio late)."},
            "drift":{"type":"number","minimum":-0.05,"maximum":0.05,"description":"retime: av_offset's drift as a fraction (0.0014 for 0.14%)."}
        },"required":["action","path"],"additionalProperties":false}
    }})
}

/// What the model reads about the tool each turn.
#[must_use]
pub fn instructions() -> &'static str {
    "The media tool inspects video and audio here with ffmpeg and whisper: probe, frames (a contact sheet you see), transcribe, av_offset, and retime. For sync, measure with av_offset rather than reasoning about which way to move the audio: a positive offset means the audio is late, and retime takes av_offset's numbers unchanged. Report a sync fix as done only after av_offset measures the corrected file, which retime does itself.\n"
}

/// Run one `media` call in `cwd`.
///
/// # Errors
/// Invalid arguments, missing ffmpeg or whisper (with the install
/// command), an unreadable file, or a measurement that found nothing.
pub async fn execute(
    arguments: Value,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let arguments: Arguments = serde_json::from_value(arguments)
        .map_err(|error| format!("media arguments do not match the declared schema: {error}"))?;
    let tools = Tools::locate()?;
    let path = existing(cwd, &arguments.path)?;
    match arguments.action {
        Action::Probe => {
            let raw = ffmpeg::probe_raw(&tools, &path, cwd, cancel).await?;
            Ok(ffmpeg::summarize(&raw))
        }
        Action::Frames => frames(&tools, &path, &arguments, cwd, cancel).await,
        Action::Transcribe => {
            let scratch = scratch_dir("transcribe")?;
            let model = arguments.model.as_deref().map(|model| {
                let candidate = cwd.join(model);
                if candidate.is_file() {
                    candidate
                } else {
                    PathBuf::from(model)
                }
            });
            let result = transcript::transcribe(
                &tools,
                &path,
                model.as_deref(),
                arguments.language.as_deref().unwrap_or("en"),
                cwd,
                &scratch,
                cancel,
            )
            .await;
            let _ = std::fs::remove_dir_all(&scratch);
            let (segments, engine) = result?;
            let text = transcript::joined(&segments);
            let mut answer = json!({
                "engine": engine,
                "text": text,
                "segments": segments.iter().map(|segment| json!({
                    "start": round3(segment.start),
                    "end": round3(segment.end),
                    "text": segment.text,
                })).collect::<Vec<_>>(),
            });
            if let Some(reference) = &arguments.reference_text {
                answer["comparison"] = transcript::comparison(reference, &text);
            }
            Ok(answer)
        }
        Action::AvOffset => {
            let reference = match &arguments.reference {
                Some(reference) => Some(existing(cwd, reference)?),
                None => None,
            };
            av_offset(&tools, &path, reference.as_deref(), &arguments, cwd, cancel).await
        }
        Action::Retime => retime(&tools, &path, &arguments, cwd, cancel).await,
    }
}

/// `relative` under `cwd`, which must be a file.
fn existing(cwd: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = cwd.join(relative);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("{} is not a file.", path.display()))
    }
}

fn scratch_dir(kind: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let dir = std::env::temp_dir()
        .join("openagents-media")
        .join(format!("{kind}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    Ok(dir)
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// The times a contact sheet shows: the ones asked for, or `count` evenly
/// spaced ones, each in the middle of its share of the clip.
fn sheet_times(
    times: Option<&[f64]>,
    count: Option<usize>,
    duration: Option<f64>,
) -> Result<Vec<f64>, String> {
    if let Some(times) = times.filter(|times| !times.is_empty()) {
        if times.len() > MAX_FRAMES {
            return Err(format!(
                "A contact sheet holds at most {MAX_FRAMES} frames."
            ));
        }
        if times.iter().any(|time| !time.is_finite() || *time < 0.0) {
            return Err("Frame times are seconds from the start, zero or more.".into());
        }
        return Ok(times.to_vec());
    }
    let count = count.unwrap_or(6).clamp(1, MAX_FRAMES);
    let duration = duration
        .filter(|duration| *duration > 0.0)
        .ok_or("The clip's duration is unknown; pass times.")?;
    Ok((0..count)
        .map(|index| round3((index as f64 + 0.5) / count as f64 * duration))
        .collect())
}

async fn frames(
    tools: &Tools,
    path: &Path,
    arguments: &Arguments,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let raw = ffmpeg::probe_raw(tools, path, cwd, cancel).await?;
    if ffmpeg::first_stream(&raw, "video").is_none() {
        return Err(format!("{} has no picture.", path.display()));
    }
    let times = sheet_times(
        arguments.times.as_deref(),
        arguments.count,
        ffmpeg::duration(&raw),
    )?;
    let width = arguments.width.unwrap_or(320).clamp(64, 960);
    let scratch = scratch_dir("frames")?;
    let result = async {
        for (index, time) in times.iter().enumerate() {
            let mut words = ffmpeg::strings(&[
                "-v",
                "error",
                "-nostdin",
                "-y",
                "-ss",
                &format!("{time:.3}"),
                "-i",
            ]);
            words.push(path.display().to_string());
            words.extend(ffmpeg::strings(&[
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:-2"),
            ]));
            words.push(
                scratch
                    .join(format!("f{index:03}.png"))
                    .display()
                    .to_string(),
            );
            ffmpeg::run(&tools.ffmpeg, &words, cwd, cancel).await?;
            if !scratch.join(format!("f{index:03}.png")).is_file() {
                return Err(format!(
                    "There is no frame at {time} s; the clip is shorter."
                ));
            }
        }
        let columns = times.len().min(4);
        let rows = times.len().div_ceil(columns);
        let sheet = std::env::temp_dir().join("openagents-media").join(format!(
            "sheet-{}.png",
            scratch
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        ));
        let mut words =
            ffmpeg::strings(&["-v", "error", "-nostdin", "-y", "-start_number", "0", "-i"]);
        words.push(scratch.join("f%03d.png").display().to_string());
        words.extend(ffmpeg::strings(&[
            "-vf",
            &format!("tile={columns}x{rows}:padding=4:color=black"),
            "-frames:v",
            "1",
        ]));
        words.push(sheet.display().to_string());
        ffmpeg::run(&tools.ffmpeg, &words, cwd, cancel).await?;
        Ok::<_, String>((sheet, columns, rows))
    }
    .await;
    let _ = std::fs::remove_dir_all(&scratch);
    let (sheet, columns, rows) = result?;
    let size = std::fs::metadata(&sheet).map_or(u64::MAX, |meta| meta.len());
    let mut answer = json!({
        "sheet": sheet.display().to_string(),
        "times": times,
        "columns": columns,
        "rows": rows,
        "order": "left to right, then top to bottom",
        "size": size,
    });
    if arguments.look && size <= LOOK_MAX_BYTES {
        answer["look"] = json!({"path": sheet.display().to_string(), "media_type": "image/png"});
    } else if arguments.look {
        answer["note"] = json!(format!(
            "The sheet is {size} bytes, over the {LOOK_MAX_BYTES} byte limit for viewing; use a smaller width or fewer frames."
        ));
    }
    Ok(answer)
}

/// The user message that shows a contact sheet to the model, when a
/// `media` output asks for one.
#[must_use]
pub fn look_message(output: &Value) -> Option<Value> {
    let path = output.get("look")?.get("path")?.as_str()?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() as u64 > LOOK_MAX_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let times = output["times"]
        .as_array()
        .map(|times| {
            times
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    Some(json!({"role":"user","content":[
        {"type":"text","text":format!("The contact sheet the media tool made, left to right then top to bottom, at {times} seconds.")},
        {"type":"image_url","image_url":{"url":url}}
    ]}))
}

/// One timeline's events: picture brightenings and sound onsets.
struct Events {
    picture: Option<Vec<f64>>,
    audio: Vec<f64>,
    frame_rate: Option<f64>,
    duration: f64,
}

async fn events(
    tools: &Tools,
    path: &Path,
    want_picture: bool,
    max_seconds: f64,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Events, String> {
    let raw = ffmpeg::probe_raw(tools, path, cwd, cancel).await?;
    let video = ffmpeg::first_stream(&raw, "video");
    let frame_rate = video.and_then(ffmpeg::frame_rate);
    let picture = if want_picture && video.is_some() {
        let series = ffmpeg::brightness(tools, path, &raw, max_seconds, cwd, cancel).await?;
        Some(sync::picture_events(&series, REFRACTORY))
    } else {
        None
    };
    let (samples, start) = ffmpeg::samples(tools, path, &raw, max_seconds, cwd, cancel).await?;
    let envelope = sync::envelope(&samples, f64::from(ffmpeg::ANALYSIS_RATE), 8, start);
    let audio = sync::audio_events(&envelope, REFRACTORY);
    let duration = ffmpeg::duration(&raw).unwrap_or(0.0).min(max_seconds);
    Ok(Events {
        picture,
        audio,
        frame_rate,
        duration,
    })
}

fn no_fit(what: &str, error: NoFit, found: (usize, usize)) -> String {
    let NoFit::TooFewPairs(pairs) = error;
    format!(
        "Could not measure {what}: {} and {} events were found, and {pairs} matched within max_offset_seconds. \
         The clip needs sharp sync events in both (a clap, a flash with a beep, or a sync test). \
         If the offset may be larger, raise max_offset_seconds.",
        found.0, found.1
    )
}

fn fit_json(fit: &Fit) -> Value {
    json!({
        "offset_seconds": round_us(fit.offset),
        "drift": round_us(fit.drift * 1000.0) / 1000.0,
        "pairs": fit.pairs,
        "rms_ms": round3(fit.rms * 1000.0),
        "span_seconds": [round3(fit.span.0), round3(fit.span.1)],
    })
}

/// Rounded to the microsecond.
fn round_us(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

async fn av_offset(
    tools: &Tools,
    path: &Path,
    reference: Option<&Path>,
    arguments: &Arguments,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let max_seconds = arguments
        .max_seconds
        .unwrap_or(DEFAULT_MAX_SECONDS)
        .clamp(1.0, LIMIT_MAX_SECONDS);
    let max_offset = arguments
        .max_offset_seconds
        .unwrap_or(1.0)
        .clamp(0.001, 30.0);
    let subject = events(tools, path, true, max_seconds, cwd, cancel).await?;
    let tolerance = arguments.tolerance_seconds.unwrap_or_else(|| {
        subject
            .frame_rate
            .map_or(0.04, |rate| (1.2 / rate).clamp(0.02, 0.1))
    });
    let picture = subject.picture.clone().ok_or_else(|| {
        format!(
            "{} has no picture to measure the audio against.",
            path.display()
        )
    })?;
    let (measured, method, detail) = match reference {
        None => {
            let fit =
                sync::fit(&picture, &subject.audio, max_offset, tolerance).map_err(|error| {
                    no_fit(
                        "audio against picture",
                        error,
                        (picture.len(), subject.audio.len()),
                    )
                })?;
            (
                AvOffset::direct(&fit),
                "the clip's own picture flashes against its sound onsets",
                json!({"audio_against_picture": fit_json(&fit)}),
            )
        }
        Some(reference) => {
            let theirs = events(tools, reference, true, max_seconds, cwd, cancel).await?;
            let audio = sync::fit(&theirs.audio, &subject.audio, max_offset, tolerance).map_err(
                |error| {
                    no_fit(
                        "the audio against the reference",
                        error,
                        (theirs.audio.len(), subject.audio.len()),
                    )
                },
            )?;
            let picture_fit = match &theirs.picture {
                Some(reference_picture) => Some(
                    sync::fit(reference_picture, &picture, max_offset, tolerance).map_err(
                        |error| {
                            no_fit(
                                "the picture against the reference",
                                error,
                                (reference_picture.len(), picture.len()),
                            )
                        },
                    )?,
                ),
                None => None,
            };
            let mut detail = json!({"audio_against_reference": fit_json(&audio)});
            if let Some(fit) = &picture_fit {
                detail["picture_against_reference"] = fit_json(fit);
            }
            (
                AvOffset::via_reference(picture_fit.as_ref(), &audio),
                if picture_fit.is_some() {
                    "the picture and the sound each against the reference recording"
                } else {
                    "the sound against the reference recording's sound; the picture is taken to run on the reference's clock"
                },
                detail,
            )
        }
    };
    let duration = subject.duration;
    Ok(json!({
        "offset_seconds": round_us(measured.offset),
        "offset_at_end_seconds": round_us(measured.at(duration)),
        "drift": round_us(measured.drift * 1000.0) / 1000.0,
        "drift_percent": round_us(measured.drift * 100.0),
        "drift_ms_per_minute": round3(measured.drift * 60_000.0),
        "duration_seconds": round3(duration),
        "sign": "positive: the audio is later than the picture; negative: earlier",
        "summary": measured.summary(duration),
        "method": method,
        "detail": detail,
        "fix": {"action": "retime", "offset_seconds": round_us(measured.offset), "drift": round_us(measured.drift * 1000.0) / 1000.0},
    }))
}

/// The audio filter that undoes `offset` (audio late, seconds) and
/// `drift` (fraction): audio at `offset + (1 + drift) × t` moves to `t`.
/// The stretch relabels the sample rate and resamples back, which keeps
/// every onset exactly where it belongs (atempo smears them by tens of
/// milliseconds); the pitch moves by the drift, 0.14% for 0.14%, which no
/// one hears.
fn retime_filter(offset: f64, drift: f64, sample_rate: u32) -> String {
    let mut steps = Vec::new();
    if offset > 0.0 {
        steps.push(format!("atrim=start={offset:.6}"));
        steps.push("asetpts=PTS-STARTPTS".to_owned());
    } else if offset < 0.0 {
        steps.push(format!("adelay={:.3}:all=1", -offset * 1000.0));
    }
    let stretched = (f64::from(sample_rate) * (1.0 + drift)).round() as u32;
    if drift != 0.0 && stretched != sample_rate {
        steps.push(format!("asetrate={stretched}"));
        steps.push(format!("aresample={sample_rate}"));
    }
    if steps.is_empty() {
        steps.push("anull".to_owned());
    }
    steps.join(",")
}

/// The audio codec for a container, by the output's extension.
fn audio_codec(output: &Path) -> &'static str {
    match output
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "m4v" | "m4a") => "aac",
        Some("webm") => "libopus",
        Some("mp3") => "libmp3lame",
        _ => "pcm_s16le",
    }
}

async fn retime(
    tools: &Tools,
    path: &Path,
    arguments: &Arguments,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let output = cwd.join(arguments.output.as_deref().ok_or("retime needs output.")?);
    if output.exists() {
        return Err(format!(
            "{} already exists; retime never replaces a file.",
            output.display()
        ));
    }
    let offset = arguments
        .offset_seconds
        .ok_or("retime needs offset_seconds from av_offset.")?;
    let drift = arguments.drift.unwrap_or(0.0);
    if !offset.is_finite() || !drift.is_finite() || drift.abs() > 0.05 {
        return Err("offset_seconds and drift must be av_offset's numbers.".into());
    }
    let raw = ffmpeg::probe_raw(tools, path, cwd, cancel).await?;
    let sample_rate = ffmpeg::first_stream(&raw, "audio")
        .and_then(|stream| ffmpeg::number(&stream["sample_rate"]))
        .filter(|rate| *rate >= 1.0)
        .ok_or_else(|| format!("{} has no sound to retime.", path.display()))?
        as u32;
    let filter = retime_filter(offset, drift, sample_rate);
    let mut words = ffmpeg::strings(&["-v", "error", "-nostdin", "-n", "-i"]);
    words.push(path.display().to_string());
    words.extend(ffmpeg::strings(&[
        "-map",
        "0:v?",
        "-map",
        "0:a:0",
        "-c:v",
        "copy",
        "-af",
        &filter,
        "-c:a",
        audio_codec(&output),
    ]));
    words.push(output.display().to_string());
    let shown = std::iter::once("ffmpeg".to_owned())
        .chain(
            words
                .iter()
                .map(|word| crate::bundled_runtime::shell_word(word)),
        )
        .collect::<Vec<_>>()
        .join(" ");
    if let crate::approval::Verdict::Refused(why) = crate::approval::check(&shown) {
        return Err(why);
    }
    ffmpeg::run(&tools.ffmpeg, &words, cwd, cancel).await?;
    let reference = match &arguments.reference {
        Some(reference) => Some(existing(cwd, reference)?),
        None => None,
    };
    let remeasured = av_offset(tools, &output, reference.as_deref(), arguments, cwd, cancel)
        .await
        .unwrap_or_else(|error| json!({"error": error}));
    Ok(json!({
        "output": output.display().to_string(),
        "audio_filter": filter,
        "remeasured": remeasured,
        "note": "The fix is done only if remeasured shows an offset and drift near zero.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_matches_what_execute_reads() {
        let definition = definition();
        let properties = definition["function"]["parameters"]["properties"]
            .as_object()
            .unwrap();
        for field in [
            "action",
            "path",
            "times",
            "count",
            "width",
            "look",
            "language",
            "model",
            "reference_text",
            "reference",
            "max_offset_seconds",
            "tolerance_seconds",
            "max_seconds",
            "output",
            "offset_seconds",
            "drift",
        ] {
            assert!(properties.contains_key(field), "{field}");
        }
        assert!(
            serde_json::from_value::<Arguments>(json!({"action":"av_offset","path":"a.mkv"}))
                .is_ok()
        );
        assert!(
            serde_json::from_value::<Arguments>(json!({"action":"cut","path":"a.mkv"})).is_err()
        );
        assert!(
            serde_json::from_value::<Arguments>(json!({"action":"probe","path":"a","extra":1}))
                .is_err()
        );
    }

    #[test]
    fn contact_sheet_times_are_spread_or_taken_as_given() {
        assert_eq!(
            sheet_times(None, Some(4), Some(20.0)).unwrap(),
            [2.5, 7.5, 12.5, 17.5]
        );
        assert_eq!(
            sheet_times(Some(&[1.0, 3.5]), None, None).unwrap(),
            [1.0, 3.5]
        );
        assert!(sheet_times(None, None, None).is_err());
        assert!(sheet_times(Some(&[-1.0]), None, Some(5.0)).is_err());
        assert!(sheet_times(Some(&[0.0; 17]), None, Some(5.0)).is_err());
    }

    #[test]
    fn retime_moves_late_audio_earlier_and_speeds_up_slow_audio() {
        assert_eq!(
            retime_filter(0.4, 0.0014, 48_000),
            "atrim=start=0.400000,asetpts=PTS-STARTPTS,asetrate=48067,aresample=48000"
        );
        assert_eq!(
            retime_filter(-0.25, -0.001, 44_100),
            "adelay=250.000:all=1,asetrate=44056,aresample=44100"
        );
        assert_eq!(retime_filter(0.0, 0.000_000_1, 8000), "anull");
        assert_eq!(retime_filter(0.0, 0.0, 48_000), "anull");
        assert_eq!(audio_codec(Path::new("out.MP4")), "aac");
        assert_eq!(audio_codec(Path::new("out.mkv")), "pcm_s16le");
    }

    /// Acceptance (#11172) on real files when ffmpeg is installed: a clip
    /// with a flash every 2 s and audio 0.4 s late running 0.14% slow is
    /// measured within 20 ms, and retime's copy measures near zero.
    #[tokio::test]
    async fn a_drifting_fixture_is_measured_and_fixed_with_ffmpeg() {
        let Ok(tools) = Tools::locate() else {
            eprintln!("skipped: ffmpeg is not installed");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let duration = 30.0;
        // A white flash every 2 s; a beep 0.4 s after each, 0.14% slow.
        let picture = format!(
            "color=c=black:s=32x32:r=30:d={duration},drawbox=x=0:y=0:w=32:h=32:color=white:t=fill:enable='lt(mod(t,2),0.05)'"
        );
        let sound = format!(
            "aevalsrc=exprs='if(gte(t,0.4)*lt(mod((t-0.4)/1.0014,2),0.05),0.5*sin(2*PI*1000*t),0)':s=48000:d={duration}"
        );
        let words = ffmpeg::strings(&[
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-f",
            "lavfi",
            "-i",
            picture.as_str(),
            "-f",
            "lavfi",
            "-i",
            sound.as_str(),
            "-c:v",
            "mpeg4",
            "-q:v",
            "2",
            "-c:a",
            "pcm_s16le",
            "-shortest",
            "clip.mkv",
        ]);
        ffmpeg::run(&tools.ffmpeg, &words, dir.path(), &cancel)
            .await
            .unwrap();

        let measured = execute(
            json!({"action":"av_offset","path":"clip.mkv"}),
            dir.path(),
            &cancel,
        )
        .await
        .unwrap();
        let offset = measured["offset_seconds"].as_f64().unwrap();
        let end = measured["offset_at_end_seconds"].as_f64().unwrap();
        let reported = measured["duration_seconds"].as_f64().unwrap();
        assert!((offset - 0.4).abs() < 0.020, "{measured}");
        assert!(
            (end - (0.4 + 0.0014 * reported)).abs() < 0.020,
            "{measured}"
        );
        assert!(
            (measured["drift_percent"].as_f64().unwrap() - 0.14).abs() < 0.02,
            "{measured}"
        );

        let fixed = execute(
            json!({"action":"retime","path":"clip.mkv","output":"fixed.mkv",
                   "offset_seconds": offset, "drift": measured["drift"]}),
            dir.path(),
            &cancel,
        )
        .await
        .unwrap();
        let again = &fixed["remeasured"];
        assert!(
            again["offset_seconds"].as_f64().unwrap().abs() < 0.020,
            "{fixed}"
        );
        assert!(
            again["offset_at_end_seconds"].as_f64().unwrap().abs() < 0.020,
            "{fixed}"
        );

        let probe = execute(
            json!({"action":"probe","path":"clip.mkv"}),
            dir.path(),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(probe["streams"].as_array().unwrap().len(), 2);
        let sheet = execute(
            json!({"action":"frames","path":"clip.mkv","count":3,"width":64}),
            dir.path(),
            &cancel,
        )
        .await
        .unwrap();
        assert!(look_message(&sheet).is_some(), "{sheet}");
        let _ = std::fs::remove_file(sheet["sheet"].as_str().unwrap());
    }

    /// Acceptance (#11172): whisper.cpp's sample clip transcribes to its
    /// known words. Runs where whisper.cpp, its sample, and a real model
    /// are installed.
    #[tokio::test]
    async fn the_whisper_sample_transcribes_to_its_reference_text() {
        let sample = Path::new("/opt/homebrew/share/whisper-cpp/jfk.wav");
        let installed = Tools::locate().is_ok()
            && ffmpeg::find("whisper-cli").is_some()
            && transcript::find_model(None, &transcript::model_folders()).is_some();
        if !sample.is_file() || !installed {
            eprintln!("skipped: ffmpeg, whisper.cpp, its sample, or a model is missing");
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let answer = execute(
            json!({"action":"transcribe","path":sample.display().to_string(),
                   "reference_text":"And so my fellow Americans, ask not what your country can do for you, ask what you can do for your country."}),
            Path::new("/"),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(answer["comparison"]["matches"], true, "{answer}");
    }
}
