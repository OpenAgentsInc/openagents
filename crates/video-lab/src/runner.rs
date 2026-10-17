//! The Labs worker's Video jobs: `render`, `frames` and `measure` for a
//! `cut.v1` version, run with ffmpeg (`video-lab render-job` and the others).
//!
//! A job reads the worker's input on stdin (`{"work": {"version": ..},
//! "local_assets": {checksum: path}}`) and answers with the handler output
//! (`{"outputs": [..]}`). The worker runs it with an empty environment, so
//! the operator names ffmpeg and ffprobe by absolute path in the handler's
//! arguments, and a job opens only the files the worker passes it, each
//! checked against the version's checksum. A path a document names is never
//! opened as written.
//!
//! What these runners leave to the Python renderer for now (`cut.py`): title
//! text and captions are drawn by `cut.py` with the film fonts, so here a
//! title is its background (black, or its `under` clip) and captions are not
//! burned in; a held frame doesn't push in; speech isn't levelled against the
//! music; and beats and transcripts aren't measured (`<clip>.beats.json`,
//! whisper). Shots and scenes render in Blender and Verse, which these
//! runners don't run.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use lab_kit::{ArtifactRole, Version};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::measure::{Measured, Source, Span};

/// A job may return at most this many bytes in one file.
pub const MAX_OUTPUT: u64 = 12 * 1024 * 1024;
/// The strip's frame width.
const STRIP_WIDTH: u32 = 320;
/// A still stretch at least this long is measured (`cut.py`'s `STILL_OK`).
const STILL: f64 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// At most 360 lines, quick to encode and small enough to stream.
    Preview,
    /// The cut's own size.
    Review,
}

/// The programs a job runs, named by absolute path.
#[derive(Clone, Debug)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub quality: Quality,
}

#[derive(Deserialize)]
struct Input {
    work: Work,
    #[serde(default)]
    local_assets: BTreeMap<String, PathBuf>,
}

#[derive(Deserialize)]
struct Work {
    version: Version,
}

/// Runs job `name` (`render`, `frames` or `measure`) on the worker's input,
/// writing its intermediate files under `dir`.
///
/// # Errors
/// The input isn't a job, a file is missing or doesn't match the version,
/// ffmpeg fails, or the result is too large to return.
pub fn handle(name: &str, input: &[u8], tools: &Tools, dir: &Path) -> Result<Value, String> {
    let input: Input =
        serde_json::from_slice(input).map_err(|_| "Read the Video job.".to_owned())?;
    let version = &input.work.version;
    if version.schema != "cut.v1" {
        return Err("This computer renders cuts. Shots and scenes render in the studio.".into());
    }
    let job = Job {
        tools,
        dir,
        cut: &version.content,
        files: files(version, &input.local_assets)?,
    };
    let outputs = match name {
        "render" => vec![job.render()?],
        "frames" => job.frames()?,
        "measure" => {
            let mut measured = serde_json::to_value(job.measure()?).map_err(|e| e.to_string())?;
            measured["schema"] = json!(version.schema);
            measured["content_sha256"] = json!(lab_kit::digest(&version.content));
            vec![output(
                ArtifactRole::Export,
                "application/json",
                &serde_json::to_vec(&measured).map_err(|e| e.to_string())?,
            )]
        }
        _ => return Err("This computer runs Video render, frames and measure jobs.".into()),
    };
    Ok(json!({ "outputs": outputs }))
}

/// The files this computer has for the version, by path as the document
/// writes it, each checked against the version's checksum.
fn files(
    version: &Version,
    local: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut found = BTreeMap::new();
    for asset in &version.assets {
        let Some(path) = local.get(&asset.sha256) else {
            continue;
        };
        if sha256_file(path).as_deref() != Some(asset.sha256.as_str()) {
            return Err(format!(
                "{} on this computer isn't the file this version uses.",
                file_name(&asset.location)
            ));
        }
        found.insert(asset.location.clone(), path.clone());
    }
    Ok(found)
}

/// The SHA-256 of a file, in lowercase hex.
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash).ok()?;
    Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn file_name(location: &str) -> &str {
    location.rsplit('/').next().unwrap_or(location)
}

fn output(role: ArtifactRole, media_type: &str, bytes: &[u8]) -> Value {
    json!({
        "role": role,
        "media_type": media_type,
        "content_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

fn num(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn even(x: f64) -> u32 {
    ((x / 2.0).round() as u32).max(8) * 2
}

/// `(start, tail)` per shot: where its slot starts, and how far it runs past
/// it to cover the next shot's crossfade.
fn slots(cut: &Value) -> (Vec<(f64, f64)>, f64) {
    let shots = cut["shots"].as_array().cloned().unwrap_or_default();
    let mut t = 0.0;
    let mut out = Vec::new();
    for (i, shot) in shots.iter().enumerate() {
        let tail = shots.get(i + 1).map_or(0.0, |n| num(n, "xfade", 0.0));
        out.push((t, tail));
        t += num(shot, "dur", 0.0);
    }
    (out, t)
}

/// The filter that puts a clip into a `w` x `h` frame, by the shot's `fit`.
fn fit_filter(shot: &Value, w: u32, h: u32) -> String {
    match shot["fit"].as_str().unwrap_or("pad") {
        "crop" => {
            let focus = |i: usize| shot["focus"][i].as_f64().unwrap_or(0.5);
            format!(
                "scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h}:(iw-{w})*{}:(ih-{h})*{}",
                focus(0),
                focus(1)
            )
        }
        "blur" => format!(
            "split[fitbg][fitfg];[fitbg]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},gblur=sigma={},eq=brightness=-0.12:saturation=0.75[fitb];[fitfg]scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos[fitf];[fitb][fitf]overlay=(W-w)/2:(H-h)/2",
            (h / 40).max(8)
        ),
        _ => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos,pad={w}:{h}:-1:-1:black"
        ),
    }
}

struct Job<'a> {
    tools: &'a Tools,
    dir: &'a Path,
    cut: &'a Value,
    files: BTreeMap<String, PathBuf>,
}

impl Job<'_> {
    fn shots(&self) -> Vec<Value> {
        self.cut["shots"].as_array().cloned().unwrap_or_default()
    }

    fn fps(&self) -> f64 {
        num(self.cut, "fps", 30.0)
    }

    fn size(&self) -> (u32, u32) {
        let at = |i: usize| self.cut["size"][i].as_u64().unwrap_or(16) as u32;
        (at(0), at(1))
    }

    /// The frame size for this job's quality.
    fn frame(&self) -> (u32, u32) {
        let (w, h) = self.size();
        match self.tools.quality {
            Quality::Preview if h > 360 => (even(f64::from(w) * 360.0 / f64::from(h)), 360),
            _ => (w, h),
        }
    }

    /// The local file for a path the document names.
    fn file(&self, src: &str) -> Result<&Path, String> {
        self.files
            .get(src)
            .map(PathBuf::as_path)
            .ok_or_else(|| format!("{} isn't on this computer.", file_name(src)))
    }

    fn ffmpeg(&self, args: &[String], what: &str) -> Result<String, String> {
        let out = Command::new(&self.tools.ffmpeg)
            .args(["-nostdin", "-hide_banner", "-y"])
            .args(args)
            .output()
            .map_err(|_| "Install ffmpeg on this computer.".to_owned())?;
        let log = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() {
            Ok(log)
        } else {
            let tail: String = log
                .chars()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            eprintln!("{tail}");
            Err(format!("ffmpeg couldn't {what}."))
        }
    }

    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// One shot's picture for its slot and tail, as `seg-{i}.mp4`.
    fn segment(&self, i: usize, shot: &Value, tail: f64, w: u32, h: u32) -> Result<String, String> {
        let fps = self.fps();
        let name = shot["name"].as_str().unwrap_or_default();
        let length = num(shot, "dur", 0.0) + tail;
        let out = self.path(&format!("seg-{i}.mp4"));
        let encode = |crf: &str| -> Vec<String> {
            [
                "-an", "-c:v", "libx264", "-crf", crf, "-preset", "veryfast", "-pix_fmt", "yuv420p",
            ]
            .map(String::from)
            .to_vec()
        };
        let mut args: Vec<String> = Vec::new();
        let filter;
        if shot.get("title").is_some() {
            match shot["title"]["under"]["src"].as_str() {
                Some(src) => {
                    let file = self.file(src)?;
                    args.extend([
                        "-stream_loop".into(),
                        "-1".into(),
                        "-i".into(),
                        file.to_string_lossy().into_owned(),
                    ]);
                    filter = format!("{},fps={fps},setsar=1", fit_filter(shot, w, h));
                }
                None => {
                    args.extend([
                        "-f".into(),
                        "lavfi".into(),
                        "-i".into(),
                        format!("color=c=black:s={w}x{h}:r={fps}"),
                    ]);
                    filter = "setsar=1".into();
                }
            }
            args.extend(["-t".into(), format!("{length:.3}"), "-vf".into(), filter]);
        } else {
            let src = shot["src"].as_str().unwrap_or_default();
            let file = self.file(src)?.to_string_lossy().into_owned();
            let speed = num(shot, "speed", 1.0);
            let mut vf = String::new();
            if (speed - 1.0).abs() > 1e-9 {
                vf += &format!("setpts=PTS/{speed},");
            }
            vf += &format!("{},fps={fps},setsar=1", fit_filter(shot, w, h));
            args.extend(["-ss".into(), format!("{}", num(shot, "in", 0.0))]);
            if let Some(play) = shot["play"].as_f64().filter(|p| *p < length - 1e-6) {
                // Plays `play` seconds, then holds the last frame for the rest of the slot.
                args.extend(["-t".into(), format!("{:.3}", play.max(1.0 / fps))]);
                vf += &format!(",tpad=stop_mode=clone:stop_duration={length:.3}");
            }
            args.extend(["-i".into(), file]);
            if let Some(over) = shot["over"]["src"].as_str() {
                let at = num(&shot["over"], "at", 0.0);
                args.extend(["-i".into(), self.file(over)?.to_string_lossy().into_owned()]);
                args.extend([
                    "-filter_complex".into(),
                    format!(
                        "[0:v]{vf}[base];[1:v]scale={w}:{h}:flags=lanczos,fps={fps},format=yuva444p,setpts=PTS-STARTPTS+{at}/TB[top];[base][top]overlay=eof_action=pass:format=auto,format=yuv420p"
                    ),
                ]);
            } else {
                args.extend(["-vf".into(), vf]);
            }
            args.extend(["-t".into(), format!("{length:.3}")]);
        }
        args.extend(encode("16"));
        args.push(out.clone());
        self.ffmpeg(&args, &format!("cut the shot {name}"))?;
        Ok(out)
    }

    /// The whole cut: shots cut and crossfaded, the music under them, the
    /// voiceovers and the shots' own sound mixed in, as H.264 and AAC.
    fn render(&self) -> Result<Value, String> {
        let shots = self.shots();
        if shots.is_empty() {
            return Err("This cut has no shots.".into());
        }
        let (w, h) = self.frame();
        let fps = self.fps();
        let (slots, total) = slots(self.cut);
        let mut inputs: Vec<String> = Vec::new();
        for (i, (shot, (_, tail))) in shots.iter().zip(&slots).enumerate() {
            let seg = self.segment(i, shot, *tail, w, h)?;
            inputs.extend(["-i".into(), seg]);
        }
        let mut chain: Vec<String> = (0..shots.len())
            .map(|i| format!("[{i}:v]settb=1/{fps},setpts=N/({fps}*TB),format=yuv420p[s{i}]"))
            .collect();
        let mut label = "[s0]".to_owned();
        for (i, shot) in shots.iter().enumerate().skip(1) {
            let x = num(shot, "xfade", 0.0);
            let next = format!("[v{i}]");
            if x > 0.0 {
                chain.push(format!(
                    "{label}[s{i}]xfade=transition={}:duration={x}:offset={}{next}",
                    shot["xfade_kind"].as_str().unwrap_or("fade"),
                    slots[i].0
                ));
            } else {
                chain.push(format!(
                    "{label}[s{i}]concat=n=2:v=1:a=0,settb=1/{fps},setpts=N/({fps}*TB){next}"
                ));
            }
            label = next;
        }
        let mut post = format!("{label}trim=end={total},setpts=PTS-STARTPTS");
        let (fade_in, fade_out) = (
            num(self.cut, "fade_in", 0.0),
            num(self.cut, "fade_out", 0.0),
        );
        if fade_in > 0.0 {
            post += &format!(",fade=t=in:st=0:d={fade_in}");
        }
        if fade_out > 0.0 {
            post += &format!(",fade=t=out:st={}:d={fade_out}", total - fade_out);
        }
        chain.push(post + "[v]");

        let mut n = shots.len();
        let music = self.cut["music"].as_str();
        match music {
            Some(src) => {
                inputs.extend(["-i".into(), self.file(src)?.to_string_lossy().into_owned()])
            }
            None => {
                inputs.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(String::from))
            }
        }
        let timing = if self.cut.get("timeline").is_some() {
            crate::timeline::music_filter(self.cut)? + ","
        } else {
            String::new()
        };
        let mut audio = vec![format!(
            "[{n}:a]{timing}aresample=48000,aformat=channel_layouts=stereo,atrim=end={total},asetpts=PTS-STARTPTS,volume={}{}[m]",
            num(self.cut, "music_gain", 1.0),
            if fade_out > 0.0 {
                format!(",afade=t=out:st={}:d={fade_out}", total - fade_out)
            } else {
                String::new()
            }
        )];
        n += 1;
        let mut beds = Vec::new();
        let mut voices: Vec<Value> = self.cut["voices"].as_array().cloned().unwrap_or_default();
        if self.cut["voice"].is_object() {
            voices.insert(0, self.cut["voice"].clone());
        }
        for (j, voice) in voices.iter().enumerate() {
            let file = self.file(voice["src"].as_str().unwrap_or_default())?;
            inputs.extend(["-i".into(), file.to_string_lossy().into_owned()]);
            let ms = (num(voice, "at", 0.0) * 1000.0).round() as u64;
            audio.push(format!(
                "[{n}:a:0]aresample=48000,aformat=channel_layouts=stereo,volume={},adelay={ms}|{ms}[vo{j}]",
                num(voice, "gain", 1.0)
            ));
            beds.push(format!("[vo{j}]"));
            n += 1;
        }
        for (j, (shot, (start, _))) in shots.iter().zip(&slots).enumerate() {
            if !shot["sound"].is_object() {
                continue;
            }
            let dur = num(shot, "dur", 0.0);
            let file = self.file(shot["src"].as_str().unwrap_or_default())?;
            inputs.extend([
                "-ss".into(),
                format!("{}", num(shot, "in", 0.0)),
                "-t".into(),
                format!("{dur:.3}"),
                "-i".into(),
                file.to_string_lossy().into_owned(),
            ]);
            let ms = (start * 1000.0).round() as u64;
            let f = (dur / 4.0).min(0.05);
            audio.push(format!(
                "[{n}:a:0]aresample=48000,aformat=channel_layouts=stereo,atrim=end={dur:.3},asetpts=PTS-STARTPTS,afade=t=in:d={f},afade=t=out:st={:.3}:d={f},volume={},adelay={ms}|{ms}[sp{j}]",
                dur - f,
                num(&shot["sound"], "gain", 1.0)
            ));
            beds.push(format!("[sp{j}]"));
            n += 1;
        }
        audio.push(if beds.is_empty() {
            "[m]anull[a]".into()
        } else {
            format!(
                "[m]{}amix=inputs={}:normalize=0:duration=first[a]",
                beds.concat(),
                1 + beds.len()
            )
        });
        let out = self.path("render.mp4");
        let crf = if self.tools.quality == Quality::Preview {
            "30"
        } else {
            "23"
        };
        let mut args = inputs;
        args.extend([
            "-filter_complex".into(),
            [chain, audio].concat().join(";"),
            "-map".into(),
            "[v]".into(),
            "-map".into(),
            "[a]".into(),
            "-t".into(),
            format!("{total:.3}"),
            "-r".into(),
            format!("{fps}"),
        ]);
        args.extend(
            [
                "-c:v",
                "libx264",
                "-crf",
                crf,
                "-preset",
                "veryfast",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-ar",
                "48000",
                "-movflags",
                "+faststart",
            ]
            .map(String::from),
        );
        args.push(out.clone());
        self.ffmpeg(&args, "put the cut together")?;
        let bytes = read_bounded(Path::new(&out))?;
        Ok(output(ArtifactRole::Render, "video/mp4", &bytes))
    }

    /// A poster (the first shot's middle frame) and a strip of every shot's
    /// middle frame, side by side, as JPEG.
    fn frames(&self) -> Result<Vec<Value>, String> {
        let shots = self.shots();
        if shots.is_empty() {
            return Err("This cut has no shots.".into());
        }
        let (w, h) = self.size();
        let (tw, th) = (
            STRIP_WIDTH,
            even(f64::from(STRIP_WIDTH) * f64::from(h) / f64::from(w)),
        );
        let mut frames = Vec::new();
        for (i, shot) in shots.iter().take(24).enumerate() {
            let out = self.path(&format!("frame-{i}.png"));
            let dur = num(shot, "dur", 0.0);
            let mut args: Vec<String> = Vec::new();
            match shot["src"]
                .as_str()
                .or(shot["title"]["under"]["src"].as_str())
            {
                Some(src) => {
                    let mid = shot["play"]
                        .as_f64()
                        .map_or(dur / 2.0, |p| p.min(dur / 2.0));
                    let at = num(shot, "in", 0.0) + mid * num(shot, "speed", 1.0);
                    args.extend([
                        "-ss".into(),
                        format!("{at:.3}"),
                        "-i".into(),
                        self.file(src)?.to_string_lossy().into_owned(),
                    ]);
                }
                None => args.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    format!("color=c=black:s={tw}x{th}"),
                ]),
            }
            args.extend([
                "-frames:v".into(),
                "1".into(),
                "-vf".into(),
                format!("{},setsar=1", fit_filter(shot, tw, th)),
                out.clone(),
            ]);
            self.ffmpeg(
                &args,
                &format!(
                    "take a frame of {}",
                    shot["name"].as_str().unwrap_or_default()
                ),
            )?;
            frames.push(out);
        }
        let poster = self.path("poster.jpg");
        self.ffmpeg(
            &[
                "-i".into(),
                frames[0].clone(),
                "-q:v".into(),
                "3".into(),
                poster.clone(),
            ],
            "make the poster",
        )?;
        let strip = self.path("strip.jpg");
        let mut args: Vec<String> = frames
            .iter()
            .flat_map(|f| ["-i".into(), f.clone()])
            .collect();
        if frames.len() > 1 {
            let inputs: String = (0..frames.len()).map(|i| format!("[{i}:v]")).collect();
            args.extend([
                "-filter_complex".into(),
                format!("{inputs}hstack=inputs={}", frames.len()),
            ]);
        }
        args.extend(["-q:v".into(), "4".into(), strip.clone()]);
        self.ffmpeg(&args, "make the strip")?;
        Ok(vec![
            output(
                ArtifactRole::Poster,
                "image/jpeg",
                &read_bounded(Path::new(&poster))?,
            ),
            output(
                ArtifactRole::Strip,
                "image/jpeg",
                &read_bounded(Path::new(&strip))?,
            ),
        ])
    }

    /// The facts `check` needs about the cut's files: each clip's size, rate,
    /// length, sound and still stretches; the voiceovers' and music's lengths;
    /// and which overlays exist.
    fn measure(&self) -> Result<Measured, String> {
        let (w, h) = self.size();
        let mut m = Measured {
            size: Some([w, h]),
            ..Measured::default()
        };
        for shot in self.shots() {
            if let Some(src) = shot["src"].as_str()
                && !m.sources.contains_key(src)
            {
                let source = match self.files.get(src) {
                    Some(file) => {
                        let mut s = self.probe(file)?;
                        s.stills = self.stills(file)?;
                        s
                    }
                    None => Source::default(),
                };
                m.sources.insert(
                    src.to_owned(),
                    Source {
                        path: src.to_owned(),
                        name: file_name(src).to_owned(),
                        ..source
                    },
                );
            }
            if let Some(over) = shot["over"]["src"].as_str() {
                m.files
                    .insert(over.to_owned(), self.files.contains_key(over));
            }
        }
        let mut voices: Vec<Value> = self.cut["voices"].as_array().cloned().unwrap_or_default();
        voices.push(self.cut["voice"].clone());
        for src in voices.iter().filter_map(|v| v["src"].as_str()) {
            let length = match self.files.get(src) {
                Some(file) => Some(self.probe(file)?.dur),
                None => None,
            };
            m.audio.insert(src.to_owned(), length);
        }
        if let Some(src) = self.cut["music"].as_str() {
            m.music = Some(match self.files.get(src) {
                Some(file) => Some(self.probe(file)?.dur),
                None => None,
            });
        }
        Ok(m)
    }

    /// Size as shown (a rotation flag applied), rate, length and sound.
    fn probe(&self, file: &Path) -> Result<Source, String> {
        let out = Command::new(&self.tools.ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_type,width,height,r_frame_rate:stream_side_data=rotation:format=duration",
                "-of",
                "json",
            ])
            .arg(file)
            .output()
            .map_err(|_| "Install ffprobe on this computer.".to_owned())?;
        let name = file_name(&file.to_string_lossy()).to_owned();
        let j: Value = serde_json::from_slice(&out.stdout)
            .ok()
            .filter(|_| out.status.success())
            .ok_or_else(|| format!("ffprobe couldn't read {name}."))?;
        let streams = j["streams"].as_array().cloned().unwrap_or_default();
        let dur = j["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0);
        let audio = streams.iter().any(|s| s["codec_type"] == "audio");
        let mut source = Source {
            exists: true,
            dur,
            audio,
            ..Source::default()
        };
        if let Some(v) = streams.iter().find(|s| s["codec_type"] == "video") {
            let (mut w, mut h) = (
                v["width"].as_u64().unwrap_or(0) as u32,
                v["height"].as_u64().unwrap_or(0) as u32,
            );
            let rotation = v["side_data_list"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|s| s["rotation"].as_i64())
                .unwrap_or(0);
            if rotation.abs() % 180 == 90 {
                std::mem::swap(&mut w, &mut h);
            }
            let fps = v["r_frame_rate"].as_str().and_then(|r| {
                let (a, b) = r.split_once('/')?;
                let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
                (b > 0.0).then_some(a / b)
            });
            (source.w, source.h, source.fps) = (w, h, fps.unwrap_or(0.0));
        }
        Ok(source)
    }

    /// Stretches where the picture doesn't change, at 64x36 so grain doesn't
    /// count as motion (`cut.py`'s `freezes`, by ffmpeg's freezedetect).
    fn stills(&self, file: &Path) -> Result<Vec<Span>, String> {
        let log = self.ffmpeg(
            &[
                "-i".into(),
                file.to_string_lossy().into_owned(),
                "-vf".into(),
                format!("scale=64:36,freezedetect=n=0.008:d={STILL}"),
                "-an".into(),
                "-f".into(),
                "null".into(),
                "-".into(),
            ],
            "look for still stretches",
        )?;
        let at = |line: &str, key: &str| -> Option<f64> {
            line.split(key)
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        };
        let mut spans = Vec::new();
        let mut from = None;
        for line in log.lines() {
            if let Some(t) = at(line, "freeze_start: ") {
                from = Some(t);
            } else if let Some(t) = at(line, "freeze_end: ")
                && let Some(f) = from.take()
            {
                spans.push(Span { from: f, to: t });
            }
        }
        if let Some(f) = from {
            spans.push(Span {
                from: f,
                to: self.probe(file)?.dur,
            });
        }
        Ok(spans)
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_OUTPUT {
        return Err("The render is larger than a job may return. Render a preview.".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VideoKind;
    use lab_kit::{AssetClass, AssetEntry, LabKind, Report};

    fn find(tool: &str) -> Option<PathBuf> {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .chain([
                PathBuf::from("/opt/homebrew/bin"),
                PathBuf::from("/usr/bin"),
            ])
            .map(|d| d.join(tool))
            .find(|p| p.is_file())
    }

    /// The tools, or `None` (the test is skipped) on a computer without ffmpeg.
    fn tools() -> Option<Tools> {
        Some(Tools {
            ffmpeg: find("ffmpeg")?,
            ffprobe: find("ffprobe")?,
            quality: Quality::Preview,
        })
    }

    struct Fixture {
        dir: PathBuf,
        assets: Vec<AssetEntry>,
        local: BTreeMap<String, PathBuf>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Tiny synthetic clips: a moving test pattern with a tone, a still grey
    /// frame, and a tone for the music.
    fn fixture(tools: &Tools, name: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!("video-lab-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("work")).unwrap();
        let mut fixture = Fixture {
            dir: dir.clone(),
            assets: Vec::new(),
            local: BTreeMap::new(),
        };
        for (file, source) in [
            ("moving.mp4", "testsrc=size=320x180:rate=30:duration=3"),
            ("still.mp4", "color=c=gray:size=320x180:rate=30:duration=3"),
            ("music.m4a", ""),
        ] {
            let path = dir.join(file);
            let mut args: Vec<String> = Vec::new();
            if !source.is_empty() {
                args.extend(["-f", "lavfi", "-i", source].map(String::from));
            }
            args.extend(["-f", "lavfi", "-i", "sine=frequency=440:duration=3"].map(String::from));
            if !source.is_empty() {
                args.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p"].map(String::from));
            }
            args.extend(["-c:a", "aac", "-shortest"].map(String::from));
            args.push(path.to_string_lossy().into_owned());
            let ok = Command::new(&tools.ffmpeg)
                .args(["-v", "error", "-y"])
                .args(&args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "make {file}");
            let sha = sha256_file(&path).unwrap();
            let location = format!("~/clips/{file}");
            fixture.assets.push(AssetEntry {
                location,
                sha256: sha.clone(),
                class: AssetClass::PrivateLocal,
            });
            fixture.local.insert(sha, path);
        }
        fixture
    }

    fn cut(second: &str) -> Value {
        json!({
            "fps": 30, "size": [320, 180], "out": "~/out.mp4", "bpm": 120,
            "music": "~/clips/music.m4a",
            "shots": [
                {"name": "open", "src": "~/clips/moving.mp4", "in": 0, "dur": 1.0},
                {"name": "card", "title": {"lines": [{"text": "Hi", "size": 0.05}]}, "dur": 0.5, "xfade": 0.25},
                {"name": "end", "src": second, "in": 0.5, "dur": 1.0}
            ]
        })
    }

    fn input(fixture: &Fixture, content: Value) -> Vec<u8> {
        let version = Version {
            document: "film".into(),
            number: 1,
            parent: None,
            author: "test".into(),
            summary: String::new(),
            created_unix_ms: 0,
            schema: "cut.v1".into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: Report::default(),
            assets: fixture.assets.clone(),
        };
        serde_json::to_vec(&json!({"work": {"version": version}, "local_assets": fixture.local}))
            .unwrap()
    }

    fn decode(output: &Value) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(output["content_base64"].as_str().unwrap())
            .unwrap()
    }

    #[test]
    fn measure_finds_the_clips_facts_and_check_reads_them() {
        let Some(tools) = tools() else { return };
        let f = fixture(&tools, "measure");
        let work = f.dir.join("work");
        let good = cut("~/clips/moving.mp4");
        let out = handle("measure", &input(&f, good.clone()), &tools, &work).unwrap();
        assert_eq!(out["outputs"][0]["role"], "export");
        let measured: Value = serde_json::from_slice(&decode(&out["outputs"][0])).unwrap();
        assert_eq!(measured["content_sha256"], lab_kit::digest(&good));
        let source = &measured["sources"]["~/clips/moving.mp4"];
        assert_eq!(source["w"], 320);
        assert_eq!(source["audio"], true);
        assert!(
            (source["dur"].as_f64().unwrap() - 3.0).abs() < 0.1,
            "{source}"
        );
        assert!(measured["music"].as_f64().is_some());
        VideoKind::check(&good, &measured).unwrap();

        // A still grey clip pauses the film, and check says so.
        let still = cut("~/clips/still.mp4");
        let out = handle("measure", &input(&f, still.clone()), &tools, &work).unwrap();
        let measured: Value = serde_json::from_slice(&decode(&out["outputs"][0])).unwrap();
        assert!(
            !measured["sources"]["~/clips/still.mp4"]["stills"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            VideoKind::check(&still, &measured).unwrap_err().code,
            "check"
        );
    }

    #[test]
    fn render_and_frames_make_the_cut_and_its_pictures() {
        let Some(tools) = tools() else { return };
        let f = fixture(&tools, "render");
        let work = f.dir.join("work");
        let good = cut("~/clips/moving.mp4");
        let out = handle("render", &input(&f, good.clone()), &tools, &work).unwrap();
        let render = &out["outputs"][0];
        assert_eq!(render["role"], "render");
        assert_eq!(render["media_type"], "video/mp4");
        let file = work.join("rendered.mp4");
        std::fs::write(&file, decode(render)).unwrap();
        let job = Job {
            tools: &tools,
            dir: &work,
            cut: &good,
            files: BTreeMap::new(),
        };
        let probed = job.probe(&file).unwrap();
        assert!((probed.dur - 2.5).abs() < 0.15, "{}", probed.dur);
        assert!(probed.audio);
        assert_eq!((probed.w, probed.h), (320, 180));

        let out = handle("frames", &input(&f, good.clone()), &tools, &work).unwrap();
        let roles: Vec<&str> = out["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["poster", "strip"]);
        let strip = work.join("got-strip.jpg");
        std::fs::write(&strip, decode(&out["outputs"][1])).unwrap();
        let probed = job.probe(&strip).unwrap();
        assert_eq!(probed.w, 3 * STRIP_WIDTH);
    }

    #[test]
    fn a_job_opens_only_the_versions_own_files() {
        let Some(tools) = tools() else { return };
        let mut f = fixture(&tools, "files");
        let work = f.dir.join("work");
        // Not passed by the worker: the job won't open the path as written.
        let missing = handle(
            "render",
            &input(&f, cut("~/clips/elsewhere.mp4")),
            &tools,
            &work,
        )
        .unwrap_err();
        assert_eq!(missing, "elsewhere.mp4 isn't on this computer.");
        // A local file that isn't the one the version names is refused.
        let (sha, path) = f.local.pop_first().unwrap();
        let other = f.dir.join("other.bin");
        std::fs::write(&other, b"not the clip").unwrap();
        f.local.insert(sha, other);
        let err = handle(
            "measure",
            &input(&f, cut("~/clips/moving.mp4")),
            &tools,
            &work,
        )
        .unwrap_err();
        assert!(err.ends_with("isn't the file this version uses."), "{err}");
        let _ = path;
        assert!(handle("transcribe", &input(&f, cut("x")), &tools, &work).is_err());
        assert!(handle("render", b"{}", &tools, &work).is_err());
    }
}
