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
//! A job finds a file in the worker's `local_assets` (private files on an
//! enrolled host) or its `files` (the space's stored files it downloaded for
//! this run), both by checksum. Titles and captions are drawn here
//! ([`crate::text`]), a held frame pushes in, and speech and voiceovers are
//! levelled and the music ducked under them, as `cut.py` does. `measure`
//! reads a clip's `<clip>.beats.json` and `<clip>.speech.json` when the
//! version lists them. Shots and scenes render in Blender and Verse, which
//! these runners don't run.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine as _;
use lab_kit::{ArtifactRole, Version};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::measure::{Beat, CaptionLayout, Measured, Source, Span, Speech, Title, TitleLine};
use crate::text::{self, Fonts, Grain};

mod sources;
pub use sources::{Score, Whisper, Words, fix_words, pauses, score_args};

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

/// The programs a job runs, named by absolute path, and the fonts it draws
/// titles and captions with.
#[derive(Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub quality: Quality,
    pub fonts: Fonts,
    /// For `transcribe`, and for `import`'s words.
    pub whisper: Option<Whisper>,
    /// For `score`.
    pub score: Option<Score>,
}

impl Tools {
    /// ffmpeg and ffprobe at preview quality, with Paper Mono for every font.
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self {
            ffmpeg,
            ffprobe,
            quality: Quality::Preview,
            fonts: Fonts::default(),
            whisper: None,
            score: None,
        }
    }
}

#[derive(Deserialize)]
struct Input {
    work: Work,
    #[serde(default)]
    local_assets: BTreeMap<String, PathBuf>,
    /// The space's stored files the worker downloaded for this run.
    #[serde(default)]
    files: BTreeMap<String, PathBuf>,
}

#[derive(Deserialize)]
struct Work {
    version: Version,
}

/// Runs job `name` (`render`, `frames`, `measure`, `transcribe`, `import` or
/// `score`) on the worker's input,
/// writing its intermediate files under `dir`.
///
/// # Errors
/// The input isn't a job, a file is missing or doesn't match the version,
/// ffmpeg fails, or the result is too large to return.
pub fn handle(name: &str, input: &[u8], tools: &Tools, dir: &Path) -> Result<Value, String> {
    let input: Input =
        serde_json::from_slice(input).map_err(|_| "Read the Video job.".to_owned())?;
    let version = &input.work.version;
    let reads_clips = matches!(name, "transcribe" | "import");
    if version.schema != "cut.v1" && !reads_clips {
        return Err("This computer renders cuts. Shots and scenes render in the studio.".into());
    }
    let job = Job {
        tools,
        dir,
        cut: &version.content,
        files: files(version, &input.local_assets, &input.files)?,
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
        "transcribe" => {
            let mut said = job.transcribe()?;
            said["schema"] = json!(version.schema);
            said["content_sha256"] = json!(lab_kit::digest(&version.content));
            vec![output(
                ArtifactRole::Export,
                "application/json",
                &serde_json::to_vec(&said).map_err(|e| e.to_string())?,
            )]
        }
        "import" => job.import()?,
        "score" => vec![job.score()?],
        _ => return Err("This computer runs Video Lab's jobs: render, frames, measure, transcribe, import and score.".into()),
    };
    Ok(json!({ "outputs": outputs }))
}

/// The files this computer has for the version, by path as the document
/// writes it, each checked against the version's checksum.
fn files(
    version: &Version,
    local: &BTreeMap<String, PathBuf>,
    stored: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut found = BTreeMap::new();
    for asset in &version.assets {
        let Some(path) = local
            .get(&asset.sha256)
            .or_else(|| stored.get(&asset.sha256))
        else {
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
        let out = self.path(&format!("seg-{i}.mp4"));
        if shot.get("title").is_some() {
            self.title(shot, tail, w, h, &out)?;
            return Ok(out);
        }
        let length = num(shot, "dur", 0.0) + tail;
        match shot["play"].as_f64().filter(|p| *p < length - 1e-6) {
            Some(play) if num(shot, "push", 0.0) > 0.0 => self.held(i, shot, play, length, w, h)?,
            _ => self.clip(shot, tail, w, h, &out)?,
        }
        Ok(out)
    }

    fn encode_args(&self) -> Vec<String> {
        [
            "-an", "-c:v", "libx264", "-crf", "16", "-preset", "veryfast", "-pix_fmt", "yuv420p",
        ]
        .map(String::from)
        .to_vec()
    }

    /// A clip's picture for its slot: `play` seconds then its last frame held.
    fn clip(&self, shot: &Value, tail: f64, w: u32, h: u32, out: &str) -> Result<(), String> {
        let fps = self.fps();
        let name = shot["name"].as_str().unwrap_or_default();
        let length = num(shot, "dur", 0.0) + tail;
        let src = shot["src"].as_str().unwrap_or_default();
        let file = self.file(src)?.to_string_lossy().into_owned();
        let speed = num(shot, "speed", 1.0);
        let mut vf = String::new();
        if (speed - 1.0).abs() > 1e-9 {
            vf += &format!("setpts=PTS/{speed},");
        }
        vf += &format!("{},fps={fps},setsar=1", fit_filter(shot, w, h));
        let mut args: Vec<String> = vec!["-ss".into(), format!("{}", num(shot, "in", 0.0))];
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
        args.extend(self.encode_args());
        args.push(out.to_owned());
        self.ffmpeg(&args, &format!("cut the shot {name}"))?;
        Ok(())
    }

    /// An encoder that takes `w` x `h` RGB frames on stdin and writes `out`.
    fn encoder(&self, w: u32, h: u32, out: &str) -> Result<std::process::Child, String> {
        Command::new(&self.tools.ffmpeg)
            .args(["-nostdin", "-hide_banner", "-v", "error", "-y"])
            .args([
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-s",
                &format!("{w}x{h}"),
            ])
            .args(["-r", &format!("{}", self.fps()), "-i", "-"])
            .args(self.encode_args())
            .arg(out)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Install ffmpeg on this computer.".to_owned())
    }

    fn finish(child: std::process::Child, what: &str) -> Result<(), String> {
        let mut child = child;
        drop(child.stdin.take());
        match child.wait() {
            Ok(s) if s.success() => Ok(()),
            _ => Err(format!("ffmpeg couldn't {what}.")),
        }
    }

    /// A title card (`cut.py`'s `render_title`): the lines over the backdrop
    /// with grain, or with `"under": {"src", "in", "dim"}` over that clip.
    fn title(&self, shot: &Value, tail: f64, w: u32, h: u32, out: &str) -> Result<(), String> {
        let fps = self.fps();
        let name = shot["name"].as_str().unwrap_or_default();
        let dur = num(shot, "dur", 0.0);
        let frames = ((dur + tail) * fps).round() as usize;
        let title = &shot["title"];
        let under = title["under"].is_object().then(|| &title["under"]);
        let mut spec = title.clone();
        if spec.get("grain").is_none() {
            spec["grain"] = json!(under.is_none());
        }
        let fonts = &self.tools.fonts;
        let lines = text::title_layout(&spec, w, h, fonts);
        let mut grain = spec["grain"]
            .as_bool()
            .unwrap_or(true)
            .then(|| Grain::new(name.len() as u64));
        let mut base = text::backdrop(w, h);
        let mut decoder = match under {
            Some(u) => {
                let file = self.file(u["src"].as_str().unwrap_or_default())?;
                Some(
                    Command::new(&self.tools.ffmpeg)
                        .args([
                            "-nostdin",
                            "-v",
                            "error",
                            "-ss",
                            &format!("{}", num(u, "in", 0.0)),
                        ])
                        .arg("-i")
                        .arg(file)
                        .args(["-vf", &format!("{},fps={fps}", fit_filter(shot, w, h))])
                        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|_| "Install ffmpeg on this computer.".to_owned())?,
                )
            }
            None => None,
        };
        let dim = under.map_or(0.0, |u| num(u, "dim", 0.0)) as f32;
        let mut enc = self.encoder(w, h, out)?;
        let mut sink = enc.stdin.take().ok_or("ffmpeg didn't start.")?;
        let mut raw = vec![0u8; (w * h * 3) as usize];
        let mut wrote = Ok(());
        for k in 0..frames {
            if let Some(stdout) = decoder.as_mut().and_then(|d| d.stdout.as_mut())
                && stdout.read_exact(&mut raw).is_ok()
            {
                base = raw.iter().map(|&c| f32::from(c) * (1.0 - dim)).collect();
            }
            let frame = text::draw_title(
                &spec,
                &lines,
                w,
                h,
                k as f64 / fps,
                dur,
                &base,
                grain.as_mut(),
                fonts,
            );
            if let Err(e) = sink.write_all(&frame) {
                wrote = Err(e);
                break;
            }
        }
        drop(sink);
        if let Some(mut d) = decoder {
            let _ = d.kill();
            let _ = d.wait();
        }
        Self::finish(enc, &format!("draw the title {name}"))?;
        wrote.map_err(|_| format!("ffmpeg couldn't draw the title {name}."))
    }

    /// `play` seconds of the clip, then its last frame held with an eased
    /// push-in of `push` (a fraction of the frame), `cut.py`'s `render_held`.
    fn held(
        &self,
        i: usize,
        shot: &Value,
        play: f64,
        length: f64,
        w: u32,
        h: u32,
    ) -> Result<(), String> {
        let fps = self.fps();
        let name = shot["name"].as_str().unwrap_or_default();
        let size = (w * h * 3) as usize;
        let mut parts = Vec::new();
        let still: Vec<u8> = if play > 0.0 {
            let played = self.path(&format!("seg-{i}-play.mp4"));
            let mut part = shot.clone();
            part["play"] = Value::Null;
            part["dur"] = json!(play);
            self.clip(&part, 0.0, w, h, &played)?;
            let raw = self.raw(&["-sseof".into(), "-0.25".into(), "-i".into(), played.clone()])?;
            parts.push(played);
            raw
        } else {
            let file = self.file(shot["src"].as_str().unwrap_or_default())?;
            self.raw(&[
                "-ss".into(),
                format!("{}", num(shot, "in", 0.0)),
                "-i".into(),
                file.to_string_lossy().into_owned(),
                "-frames:v".into(),
                "1".into(),
                "-vf".into(),
                fit_filter(shot, w, h),
            ])?
        };
        if still.len() < size {
            return Err(format!("ffmpeg couldn't take the last frame of {name}."));
        }
        let still = &still[still.len() - size..];
        let n = ((length - play) * fps).round() as usize;
        let push = num(shot, "push", 0.0);
        let hold = self.path(&format!("seg-{i}-hold.mp4"));
        let mut enc = self.encoder(w, h, &hold)?;
        let mut sink = enc.stdin.take().ok_or("ffmpeg didn't start.")?;
        let mut wrote = Ok(());
        for k in 0..n {
            let u = (k + 1) as f64 / n.max(1) as f64;
            let z = 1.0 + push * u * u * (3.0 - 2.0 * u);
            if let Err(e) = sink.write_all(&zoom(still, w, h, z)) {
                wrote = Err(e);
                break;
            }
        }
        drop(sink);
        Self::finish(enc, &format!("hold the last frame of {name}"))?;
        wrote.map_err(|_| format!("ffmpeg couldn't hold the last frame of {name}."))?;
        parts.push(hold);
        let listing = self.dir.join(format!("seg-{i}-parts.txt"));
        let list: String = parts.iter().map(|p| format!("file '{p}'\n")).collect();
        std::fs::write(&listing, list).map_err(|e| e.to_string())?;
        self.ffmpeg(
            &[
                "-f".into(),
                "concat".into(),
                "-safe".into(),
                "0".into(),
                "-i".into(),
                listing.to_string_lossy().into_owned(),
                "-c".into(),
                "copy".into(),
                self.path(&format!("seg-{i}.mp4")),
            ],
            &format!("join the shot {name}"),
        )?;
        Ok(())
    }

    /// The frames ffmpeg decodes with `args`, as RGB bytes one after another.
    fn raw(&self, args: &[String]) -> Result<Vec<u8>, String> {
        let out = Command::new(&self.tools.ffmpeg)
            .args(["-nostdin", "-v", "error"])
            .args(args)
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "Install ffmpeg on this computer.".to_owned())?;
        Ok(out.stdout)
    }

    /// The mean loudness of a file's sound (or a stretch of it), in dB.
    fn mean_db(&self, file: &Path, start: Option<f64>, dur: Option<f64>) -> Option<f64> {
        let mut args: Vec<String> = Vec::new();
        if let Some(s) = start {
            args.extend(["-ss".into(), format!("{s}")]);
        }
        if let Some(d) = dur {
            args.extend(["-t".into(), format!("{d}")]);
        }
        args.extend([
            "-i".into(),
            file.to_string_lossy().into_owned(),
            "-map".into(),
            "0:a:0".into(),
            "-af".into(),
            "volumedetect".into(),
            "-f".into(),
            "null".into(),
            "-".into(),
        ]);
        let log = self.ffmpeg(&args, "measure the sound").ok()?;
        log.split("mean_volume: ")
            .nth(1)?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    }

    /// How much to lift speech so it sits a little above the music around it
    /// (`cut.py`'s `level_db`), applied after a gentle compressor.
    fn level_db(&self, file: &Path, start: f64, dur: f64, music: Option<&Path>) -> f64 {
        let Some(music) = music else { return 0.0 };
        let (Some(speech), Some(bed)) = (
            self.mean_db(file, Some(start), Some(dur)),
            self.mean_db(music, None, None),
        ) else {
            return 0.0;
        };
        let bed = bed + 20.0 * num(self.cut, "music_gain", 1.0).max(1e-6).log10();
        // The compressor takes about 3 dB off speech; aim 3 dB over the music.
        (bed + 3.0 - speech + 3.0).clamp(-6.0, 24.0)
    }

    /// The whole cut: shots cut and crossfaded, captions over them, the music
    /// ducked under the voiceovers and the shots' own sound, as H.264 and AAC.
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
        let mut n = shots.len();
        let mut caps: Vec<Value> = self.cut["captions"].as_array().cloned().unwrap_or_default();
        caps.sort_by(|a, b| num(a, "at", 0.0).total_cmp(&num(b, "at", 0.0)));
        if !caps.is_empty() {
            let st = text::caption_style(self.cut, h);
            for (j, c) in caps.iter().enumerate() {
                let cap = text::caption_layout(
                    c["text"].as_str().unwrap_or_default(),
                    st,
                    w,
                    h,
                    &self.tools.fonts,
                );
                let [x, y, bw, bh] = cap.bounds;
                let png = self.dir.join(format!("caption-{j:03}.png"));
                text::write_png(
                    &png,
                    &text::caption_rgba(&cap, &self.tools.fonts),
                    bw.max(1) as u32,
                    bh.max(1) as u32,
                )?;
                let (at, dur) = (num(c, "at", 0.0), num(c, "dur", 0.0));
                inputs.extend([
                    "-loop".into(),
                    "1".into(),
                    "-framerate".into(),
                    format!("{fps}"),
                    "-t".into(),
                    format!("{dur:.3}"),
                    "-i".into(),
                    png.to_string_lossy().into_owned(),
                ]);
                let f = (dur / 4.0).min(0.15);
                chain.push(format!(
                    "[{n}:v]format=rgba,fade=t=in:st=0:d={f}:alpha=1,fade=t=out:st={:.3}:d={f}:alpha=1,setpts=PTS-STARTPTS+{at}/TB[c{j}]",
                    dur - f
                ));
                chain.push(format!(
                    "{label}[c{j}]overlay=x={x}:y={y}:eof_action=pass[k{j}]"
                ));
                label = format!("[k{j}]");
                n += 1;
            }
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

        let music = match self.cut["music"].as_str() {
            Some(src) => Some(self.file(src)?.to_path_buf()),
            None => None,
        };
        match &music {
            Some(file) => inputs.extend(["-i".into(), file.to_string_lossy().into_owned()]),
            None => {
                inputs.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(String::from))
            }
        }
        let timing = if self.cut.get("timeline").is_some() {
            crate::timeline::music_filter(self.cut)? + ","
        } else {
            String::new()
        };
        let mut voices: Vec<Value> = self.cut["voices"].as_array().cloned().unwrap_or_default();
        if self.cut["voice"].is_object() {
            voices.insert(0, self.cut["voice"].clone());
        }
        // (file, at, length) for each voiceover, and (shot, start, length) for
        // each shot that keeps its own sound: the music ducks under both.
        let mut voiced = Vec::new();
        for voice in &voices {
            let file = self.file(voice["src"].as_str().unwrap_or_default())?;
            voiced.push((voice, file, self.probe(file)?.dur));
        }
        let spoken: Vec<(&Value, f64)> = shots
            .iter()
            .zip(&slots)
            .filter(|(shot, _)| shot["sound"].is_object())
            .map(|(shot, (start, _))| (shot, *start))
            .collect();
        let mut ducks: Vec<(f64, f64)> = spoken
            .iter()
            .map(|(shot, start)| (*start, num(shot, "dur", 0.0)))
            .collect();
        ducks.extend(voiced.iter().map(|(v, _, len)| (num(v, "at", 0.0), *len)));
        let mut m = format!(
            "[{n}:a]{timing}aresample=48000,aformat=channel_layouts=stereo,atrim=end={total},asetpts=PTS-STARTPTS,volume={}",
            num(self.cut, "music_gain", 1.0)
        );
        if !ducks.is_empty() {
            m += &format!(
                ",volume='{}':eval=frame",
                duck_expr(&ducks, num(self.cut, "music_duck", 0.2), 0.35)
            );
        }
        if fade_out > 0.0 {
            m += &format!(",afade=t=out:st={}:d={fade_out}", total - fade_out);
        }
        let mut audio = vec![m + "[m]"];
        n += 1;
        let compress = "acompressor=threshold=-26dB:ratio=3:attack=5:release=150";
        let mut beds = Vec::new();
        for (j, (voice, file, len)) in voiced.iter().enumerate() {
            inputs.extend(["-i".into(), file.to_string_lossy().into_owned()]);
            let ms = (num(voice, "at", 0.0) * 1000.0).round() as u64;
            // Lifted like a person talking on screen, so the two sit at one level.
            let lift = self.level_db(file, 0.0, *len, music.as_deref());
            audio.push(format!(
                "[{n}:a:0]aresample=48000,aformat=channel_layouts=stereo,{compress},volume={lift:.1}dB,volume={},adelay={ms}|{ms}[vo{j}]",
                num(voice, "gain", 1.0)
            ));
            beds.push(format!("[vo{j}]"));
            n += 1;
        }
        for (j, (shot, start)) in spoken.iter().enumerate() {
            let dur = num(shot, "dur", 0.0);
            let at = num(shot, "in", 0.0);
            let file = self.file(shot["src"].as_str().unwrap_or_default())?;
            inputs.extend([
                "-ss".into(),
                format!("{at}"),
                "-t".into(),
                format!("{dur:.3}"),
                "-i".into(),
                file.to_string_lossy().into_owned(),
            ]);
            let ms = (start * 1000.0).round() as u64;
            let f = (dur / 4.0).min(0.05);
            let lift = self.level_db(file, at, dur, music.as_deref());
            audio.push(format!(
                "[{n}:a:0]aresample=48000,aformat=channel_layouts=stereo,atrim=end={dur:.3},asetpts=PTS-STARTPTS,afade=t=in:d={f},afade=t=out:st={:.3}:d={f},{compress},volume={lift:.1}dB,volume={},adelay={ms}|{ms}[sp{j}]",
                dur - f,
                num(&shot["sound"], "gain", 1.0)
            ));
            beds.push(format!("[sp{j}]"));
            n += 1;
        }
        let loudnorm = "loudnorm=I=-14:TP=-1.0:LRA=11";
        audio.push(if !beds.is_empty() {
            format!(
                "[m]{}amix=inputs={}:normalize=0:duration=first,{loudnorm}[a]",
                beds.concat(),
                1 + beds.len()
            )
        } else if music.is_none() {
            "[m]anull[a]".into()
        } else {
            format!("[m]{loudnorm}[a]")
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
    /// length, sound, still stretches, beats and speech; title and caption
    /// layouts in the render's fonts; the voiceovers' and music's lengths;
    /// and which overlays exist (`cut.py`'s `measure`).
    fn measure(&self) -> Result<Measured, String> {
        let (w, h) = self.size();
        let fonts = &self.tools.fonts;
        let mut m = Measured {
            size: Some([w, h]),
            ..Measured::default()
        };
        for shot in self.shots() {
            if shot.get("title").is_some() {
                let mut spec = shot["title"].clone();
                let mut title = Title::default();
                if let Some(under) = spec["under"]["src"].as_str()
                    && !self.files.contains_key(under)
                {
                    title.under_missing = Some(under.to_owned());
                    if let Some(o) = spec.as_object_mut() {
                        o.remove("under");
                    }
                }
                title.lines = text::title_layout(&spec, w, h, fonts)
                    .into_iter()
                    .map(|l| TitleLine {
                        text: l.text,
                        px: l.px,
                        bounds: l.bounds,
                    })
                    .collect();
                m.titles
                    .insert(shot["name"].as_str().unwrap_or_default().to_owned(), title);
                continue;
            }
            if let Some(over) = shot["over"]["src"].as_str() {
                m.files
                    .insert(over.to_owned(), self.files.contains_key(over));
            }
            let Some(src) = shot["src"].as_str() else {
                continue;
            };
            let mut source = m.sources.remove(src).unwrap_or_else(|| Source {
                path: src.to_owned(),
                name: file_name(src).to_owned(),
                ..Source::default()
            });
            if let Some(file) = self.files.get(src) {
                if !source.exists {
                    let probed = self.probe(file)?;
                    source = Source {
                        path: source.path,
                        name: source.name,
                        beats: self.sidecar::<Vec<Beat>>(src, "beats").unwrap_or_default(),
                        ..probed
                    };
                }
                if shot["sound"].is_object() {
                    if source.speech.is_none() {
                        source.speech = self.sidecar::<Speech>(src, "speech");
                    }
                } else if source.stills.is_empty() {
                    source.stills = self.stills(file)?;
                }
            }
            m.sources.insert(src.to_owned(), source);
        }
        let caps = self.cut["captions"].as_array().cloned().unwrap_or_default();
        if !caps.is_empty() {
            let st = text::caption_style(self.cut, h);
            m.caption_px = Some(st.px);
            for c in &caps {
                let t = c["text"].as_str().unwrap_or_default();
                let cap = text::caption_layout(t, st, w, h, fonts);
                m.captions.insert(
                    t.to_owned(),
                    CaptionLayout {
                        lines: cap.lines.len() as u32,
                        bounds: cap.bounds.map(|v| v as f64),
                    },
                );
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

    /// A clip's `<clip>.{what}.json` beside it, when the version lists it.
    fn sidecar<T: serde::de::DeserializeOwned>(&self, src: &str, what: &str) -> Option<T> {
        let stem = match src.rsplit_once('.') {
            Some((stem, ext)) if !ext.contains('/') => stem,
            _ => src,
        };
        let file = self.files.get(&format!("{stem}.{what}.json"))?;
        serde_json::from_slice(&std::fs::read(file).ok()?).ok()
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

/// An ffmpeg volume expression: 1 outside the slots, `level` inside them,
/// easing down over `ramp` seconds before each and back up after it.
fn duck_expr(slots: &[(f64, f64)], level: f64, ramp: f64) -> String {
    let terms: Vec<String> = slots
        .iter()
        .map(|(a, dur)| {
            let b = a + dur;
            format!(
                "clip(min((t-{:.3})/{ramp},({:.3}-t)/{ramp}),0,1)",
                a - ramp,
                b + ramp
            )
        })
        .collect();
    if terms.is_empty() {
        "1".into()
    } else {
        format!("1-{:.3}*min(1,{})", 1.0 - level, terms.join("+"))
    }
}

/// `still` (RGB, `w` x `h`) scaled by `z` about its center, bilinear.
fn zoom(still: &[u8], w: u32, h: u32, z: f64) -> Vec<u8> {
    if (z - 1.0).abs() < 1e-9 {
        return still.to_vec();
    }
    let (wf, hf) = (f64::from(w), f64::from(h));
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    let at = |x: i64, y: i64, k: usize| -> f64 {
        let x = x.clamp(0, i64::from(w) - 1) as usize;
        let y = y.clamp(0, i64::from(h) - 1) as usize;
        f64::from(still[(y * w as usize + x) * 3 + k])
    };
    let mut out = Vec::with_capacity(still.len());
    for y in 0..h {
        let sy = (f64::from(y) + 0.5) / z + cy - cy / z - 0.5;
        let (y0, fy) = (sy.floor() as i64, sy - sy.floor());
        for x in 0..w {
            let sx = (f64::from(x) + 0.5) / z + cx - cx / z - 0.5;
            let (x0, fx) = (sx.floor() as i64, sx - sx.floor());
            for k in 0..3 {
                let top = at(x0, y0, k) * (1.0 - fx) + at(x0 + 1, y0, k) * fx;
                let bottom = at(x0, y0 + 1, k) * (1.0 - fx) + at(x0 + 1, y0 + 1, k) * fx;
                out.push((top * (1.0 - fy) + bottom * fy).round() as u8);
            }
        }
    }
    out
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
        Some(Tools::new(find("ffmpeg")?, find("ffprobe")?))
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
        assert!(handle("lift", &input(&f, cut("x")), &tools, &work).is_err());
        assert!(handle("render", b"{}", &tools, &work).is_err());
    }

    /// The mean of channel 0 over a box of a decoded frame.
    fn mean(raw: &[u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> f64 {
        let mut sum = 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                sum += f64::from(raw[((y * w + x) * 3) as usize]);
            }
        }
        sum / f64::from((x1 - x0) * (y1 - y0))
    }

    #[test]
    fn titles_captions_and_push_ins_are_drawn() {
        let Some(tools) = tools() else { return };
        let f = fixture(&tools, "text");
        let work = f.dir.join("work");
        let film = json!({
            "fps": 30, "size": [320, 180], "out": "~/out.mp4",
            "captions": [{"text": "Every edit is a version", "at": 1.6, "dur": 0.8}],
            "shots": [
                {"name": "card", "title": {"lines": [{"text": "OPENAGENTS", "size": 0.12, "weight": "bold"}]}, "dur": 1.5},
                {"name": "hold", "src": "~/clips/still.mp4", "in": 0, "dur": 1.0, "play": 0.3, "push": 0.2},
                {"name": "end", "src": "~/clips/still.mp4", "in": 0, "dur": 0.5}
            ]
        });
        let job = Job {
            tools: &tools,
            dir: &work,
            cut: &film,
            files: files(
                &serde_json::from_slice::<Input>(&input(&f, film.clone()))
                    .unwrap()
                    .work
                    .version,
                &f.local,
                &BTreeMap::new(),
            )
            .unwrap(),
        };
        // The card: dark at the start, its line in bright letters once it has risen.
        let seg = job.segment(0, &film["shots"][0], 0.0, 320, 180).unwrap();
        let first = job
            .raw(&["-i".into(), seg.clone(), "-frames:v".into(), "1".into()])
            .unwrap();
        let late = job
            .raw(&[
                "-ss".into(),
                "1.4".into(),
                "-i".into(),
                seg,
                "-frames:v".into(),
                "1".into(),
            ])
            .unwrap();
        let bright = |raw: &[u8]| raw.iter().filter(|&&c| c > 150).count();
        assert_eq!(bright(&first[..320 * 180 * 3]), 0);
        assert!(bright(&late[..320 * 180 * 3]) > 300, "{}", bright(&late));
        // The hold plays 0.3 s and fills its slot.
        let seg = job.segment(1, &film["shots"][1], 0.0, 320, 180).unwrap();
        let probed = job.probe(Path::new(&seg)).unwrap();
        assert!((probed.dur - 1.0).abs() < 0.1, "{}", probed.dur);

        // The whole cut, with the caption's dark band over the grey clip.
        let out = handle("render", &input(&f, film.clone()), &tools, &work).unwrap();
        let file = work.join("captioned.mp4");
        std::fs::write(&file, decode(&out["outputs"][0])).unwrap();
        let probed = job.probe(&file).unwrap();
        assert!((probed.dur - 3.0).abs() < 0.15, "{}", probed.dur);
        let frame_at = |t: &str| {
            job.raw(&[
                "-ss".into(),
                t.into(),
                "-i".into(),
                file.to_string_lossy().into_owned(),
                "-frames:v".into(),
                "1".into(),
            ])
            .unwrap()
        };
        let cap = text::caption_layout(
            "Every edit is a version",
            text::caption_style(&film, 180),
            320,
            180,
            &tools.fonts,
        );
        let [x, y, bw, bh] = cap.bounds.map(|v| v as u32);
        // The band's left margin, where no letter is drawn.
        let band = (x + 2, y + 2, x + 6, y + bh - 2);
        let under = mean(&frame_at("2.0"), 320, band.0, band.1, band.2, band.3);
        let clear = mean(&frame_at("2.8"), 320, band.0, band.1, band.2, band.3);
        assert!(under + 40.0 < clear, "{under} {clear} {bw}");

        // measure lays the title and caption out for check.
        let out = handle("measure", &input(&f, film.clone()), &tools, &work).unwrap();
        let measured: Value = serde_json::from_slice(&decode(&out["outputs"][0])).unwrap();
        assert_eq!(measured["titles"]["card"]["lines"][0]["px"], 21);
        assert_eq!(measured["captions"]["Every edit is a version"]["lines"], 1);
        assert_eq!(measured["caption_px"], 8);
    }

    #[test]
    fn import_makes_a_poster_a_strip_and_the_pauses() {
        let Some(tools) = tools() else { return };
        let f = fixture(&tools, "import");
        let work = f.dir.join("work");
        let out = handle(
            "import",
            &input(&f, cut("~/clips/moving.mp4")),
            &tools,
            &work,
        )
        .unwrap();
        let outputs = out["outputs"].as_array().unwrap();
        let listed: Value = serde_json::from_slice(&decode(&outputs[0])).unwrap();
        let sources = listed["sources"].as_array().unwrap();
        // moving.mp4 and still.mp4; music.m4a isn't video.
        assert_eq!(sources.len(), 2);
        let moving = sources.iter().find(|s| s["name"] == "moving.mp4").unwrap();
        assert_eq!(
            (moving["w"].as_u64(), moving["audio"].as_bool()),
            (Some(320), Some(true))
        );
        assert!(moving["speech"]["how"].is_null());
        let strip = &outputs[moving["strip"].as_u64().unwrap() as usize];
        assert_eq!(strip["role"], "strip");
        assert_eq!(
            outputs[moving["poster"].as_u64().unwrap() as usize]["role"],
            "poster"
        );
        let file = work.join("got-strip.jpg");
        std::fs::write(&file, decode(strip)).unwrap();
        let job = Job {
            tools: &tools,
            dir: &work,
            cut: &Value::Null,
            files: BTreeMap::new(),
        };
        assert_eq!(job.probe(&file).unwrap().w, 384 * 6 + 30);
        // transcribe needs whisper.cpp named.
        let err = handle(
            "transcribe",
            &input(&f, cut("~/clips/moving.mp4")),
            &tools,
            &work,
        )
        .unwrap_err();
        assert!(err.contains("--whisper"), "{err}");
    }

    #[test]
    fn transcribe_hears_speech_and_fixes_the_names() {
        let Some(mut tools) = tools() else { return };
        let model = std::env::var_os("WHISPER_MODEL")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join("work/ggml-small.en.bin"))
            })
            .filter(|p| p.is_file());
        let (Some(cli), Some(model), Some(say)) = (find("whisper-cli"), model, find("say")) else {
            return;
        };
        tools.whisper = Some(Whisper {
            cli,
            model,
            words: Words::default(),
        });
        let mut f = fixture(&tools, "transcribe");
        let work = f.dir.join("work");
        let speech = f.dir.join("speech.aiff");
        assert!(
            Command::new(say)
                .arg("-o")
                .arg(&speech)
                .arg("Open agents builds Coder.")
                .status()
                .unwrap()
                .success()
        );
        let sha = sha256_file(&speech).unwrap();
        f.assets = vec![AssetEntry {
            location: "~/clips/speech.aiff".into(),
            sha256: sha.clone(),
            class: AssetClass::Public,
        }];
        f.local = BTreeMap::from([(sha, speech)]);
        let out = handle(
            "transcribe",
            &input(&f, cut("~/clips/moving.mp4")),
            &tools,
            &work,
        )
        .unwrap();
        let said: Value = serde_json::from_slice(&decode(&out["outputs"][0])).unwrap();
        let clip = &said["clips"]["~/clips/speech.aiff"];
        assert_eq!(clip["how"], "whisper.cpp");
        let words: Vec<&str> = clip["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["w"].as_str().unwrap())
            .collect();
        assert!(!words.is_empty());
        assert!(
            !words.iter().any(|w| w.eq_ignore_ascii_case("agents")),
            "{words:?}"
        );
    }

    #[test]
    fn score_makes_the_music_bed_to_the_cuts_length() {
        let Some(mut tools) = tools() else { return };
        let python = PathBuf::from("/usr/bin/python3");
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/film/music.py");
        let has_numpy = Command::new(&python)
            .args(["-c", "import numpy"])
            .status()
            .is_ok_and(|s| s.success());
        if !has_numpy || !script.is_file() {
            return;
        }
        tools.score = Some(Score { python, script });
        let f = fixture(&tools, "score");
        let work = f.dir.join("work");
        // music.py writes a launch film's arrangement, so the cut is film-length.
        let film = json!({
            "fps": 30, "size": [320, 180], "out": "~/out.mp4",
            "shots": [
                {"name": "open", "src": "~/clips/moving.mp4", "dur": 12.0},
                {"name": "middle", "src": "~/clips/moving.mp4", "dur": 8.0},
                {"name": "end", "src": "~/clips/moving.mp4", "dur": 10.0}
            ],
            "score": {"cuts_at": ["middle"], "end_at": "end"}
        });
        let out = handle("score", &input(&f, film), &tools, &work).unwrap();
        let score = &out["outputs"][0];
        assert_eq!(score["media_type"], "audio/flac");
        let file = work.join("got-score.flac");
        std::fs::write(&file, decode(score)).unwrap();
        let job = Job {
            tools: &tools,
            dir: &work,
            cut: &Value::Null,
            files: BTreeMap::new(),
        };
        assert!((job.probe(&file).unwrap().dur - 30.0).abs() < 0.1);
        let err = handle(
            "score",
            &input(&f, cut("~/clips/moving.mp4")),
            &tools,
            &work,
        )
        .unwrap_err();
        assert_eq!(err, "This cut has no score.");
    }
}
