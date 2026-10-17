//! The Video jobs over a version's clips rather than its cut: `transcribe`
//! (whisper.cpp, then `lab_words.json`'s fixes), `import` (a clip's facts,
//! poster, frame strip and speech, as `lab.py`'s `import_source` makes them)
//! and `score` (the cut's music bed, by `scripts/film/music.py`).

use super::{ArtifactRole, Job, Path, PathBuf, Value, file_name, json, num, output, read_bounded};
use crate::text;

/// The glossary of names a transcript mishears (`scripts/film/lab_words.json`).
const WORDS: &str = include_str!("../../../../scripts/film/lab_words.json");
const VIDEO: [&str; 5] = ["mov", "mp4", "m4v", "mkv", "webm"];
const AUDIO: [&str; 7] = ["wav", "m4a", "mp3", "aac", "flac", "aif", "aiff"];
/// A strip tile's size, and how many tiles a strip has.
const TILE: (u32, u32) = (384, 216);
const TILES: usize = 6;

/// whisper.cpp's command line program and the model it runs.
#[derive(Clone, Debug)]
pub struct Whisper {
    pub cli: PathBuf,
    pub model: PathBuf,
    pub words: Words,
}

/// The transcription prompt and the fixes for known mishearings.
#[derive(Clone, Debug)]
pub struct Words {
    pub prompt: String,
    /// Whole words or phrases, in the glossary's order, and their spelling.
    pub fixes: Vec<(String, String)>,
}

impl Default for Words {
    fn default() -> Self {
        Self::from_json(WORDS).unwrap_or(Self {
            prompt: String::new(),
            fixes: Vec::new(),
        })
    }
}

impl Words {
    /// A glossary in `lab_words.json`'s shape.
    ///
    /// # Errors
    /// The text isn't that JSON.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(text).map_err(|_| "Read the word list.".to_owned())?;
        Ok(Self {
            prompt: v["prompt"].as_str().unwrap_or_default().to_owned(),
            fixes: v["fixes"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect(),
        })
    }
}

/// Python and `scripts/film/music.py`, which synthesizes the score.
#[derive(Clone, Debug)]
pub struct Score {
    pub python: PathBuf,
    pub script: PathBuf,
}

fn extension(location: &str) -> String {
    file_name(location)
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn round(x: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (x * k).round() / k
}

/// `lab.py`'s `clock`: `1:05` on the second, `1:05.5` between.
fn clock(t: f64) -> String {
    let t = t.max(0.0);
    let (m, s) = ((t / 60.0).floor(), t % 60.0);
    if s % 1.0 != 0.0 {
        format!("{m}:{s:04.1}")
    } else {
        format!("{m}:{:02}", s as u32)
    }
}

/// Stretches with no voice, from the sound's loudness every 10 ms: under a
/// threshold a third of the way from the room's quiet (its 10th percentile)
/// to speech (its 70th). `lab.py`'s `pauses`, for 16-bit mono samples.
pub fn pauses(samples: &[i16], rate: u32, min_len: f64) -> Vec<Value> {
    let hop = ((f64::from(rate) * 0.01) as usize).max(1);
    let n = samples.len() / hop;
    if n < 10 {
        return Vec::new();
    }
    let db: Vec<f64> = samples[..n * hop]
        .chunks(hop)
        .map(|c| {
            let power = c
                .iter()
                .map(|&s| (f64::from(s) / 32768.0).powi(2))
                .sum::<f64>()
                / hop as f64;
            20.0 * (power + 1e-12).sqrt().log10()
        })
        .collect();
    // A five-step moving average, zero past the ends (numpy's `same`).
    let smooth: Vec<f64> = (0..n)
        .map(|i| {
            (i.saturating_sub(2)..(i + 3).min(n))
                .map(|k| db[k])
                .sum::<f64>()
                / 5.0
        })
        .collect();
    let mut sorted = smooth.clone();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        let at = p / 100.0 * (n - 1) as f64;
        let (lo, hi) = (at.floor() as usize, at.ceil() as usize);
        sorted[lo] + (sorted[hi] - sorted[lo]) * (at - at.floor())
    };
    let (floor, speech) = (percentile(10.0), percentile(70.0));
    let quiet: Vec<bool> = smooth
        .iter()
        .map(|d| *d < floor + 0.35 * (speech - floor))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if quiet[i] {
            let mut j = i;
            while j < n && quiet[j] {
                j += 1;
            }
            if (j - i) as f64 * 0.01 >= min_len {
                out.push(
                    json!({"from": round(i as f64 * 0.01, 2), "to": round(j as f64 * 0.01, 2)}),
                );
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn norm(t: &str) -> String {
    t.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '\'' | ' ' | '-'))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Known mishearings fixed in place, a phrase of k words becoming one word
/// spanning them, so captions and checks read the right names (`lab.py`'s
/// `fix_words`).
pub fn fix_words(words: &[Value], fixes: &[(String, String)]) -> Vec<Value> {
    let mut rules: Vec<(Vec<String>, &str)> = fixes
        .iter()
        .map(|(k, v)| {
            (
                norm(k).split_whitespace().map(String::from).collect(),
                v.as_str(),
            )
        })
        .collect();
    rules.sort_by_key(|(parts, _)| std::cmp::Reverse(parts.len()));
    let text = |w: &Value| w["w"].as_str().unwrap_or_default().to_owned();
    let mut out = Vec::new();
    let mut i = 0;
    'words: while i < words.len() {
        for (parts, right) in &rules {
            let k = parts.len();
            if k == 0 || i + k > words.len() {
                continue;
            }
            if words[i..i + k]
                .iter()
                .map(|w| norm(&text(w)))
                .eq(parts.iter().cloned())
            {
                let last = text(&words[i + k - 1]);
                let tail: String = last
                    .chars()
                    .rev()
                    .take_while(|c| ".,?!;:".contains(*c))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let heard: Vec<String> = words[i..i + k].iter().map(text).collect();
                out.push(json!({
                    "w": format!("{right}{tail}"),
                    "from": words[i]["from"],
                    "to": words[i + k - 1]["to"],
                    "heard": heard.join(" "),
                }));
                i += k;
                continue 'words;
            }
        }
        out.push(words[i].clone());
        i += 1;
    }
    out
}

/// The samples of a 16-bit PCM WAV file's data chunk.
fn wav_samples(bytes: &[u8]) -> Vec<i16> {
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        let body = at + 8;
        if id == b"data" {
            let end = body.saturating_add(len).min(bytes.len());
            return bytes[body..end]
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
        }
        at = body + len + (len & 1);
    }
    Vec::new()
}

/// `(start, end)` of each shot by name, and the cut's length.
fn timeline(cut: &Value) -> (Vec<(String, f64, f64)>, f64) {
    let mut t = 0.0;
    let mut out = Vec::new();
    for s in cut["shots"].as_array().into_iter().flatten() {
        let dur = num(s, "dur", 0.0);
        out.push((
            s["name"].as_str().unwrap_or_default().to_owned(),
            t,
            t + dur,
        ));
        t += dur;
    }
    (out, t)
}

/// The score's length, its whooshes and where its end card starts, from the
/// cut's `score` (`lab.py`'s `score_args`).
pub fn score_args(cut: &Value) -> Option<(f64, Vec<f64>, f64)> {
    let score = cut.get("score").filter(|s| !s.is_null())?;
    let (shots, total) = timeline(cut);
    let start = |name: &str| shots.iter().find(|(n, ..)| n == name).map(|(_, s, _)| *s);
    let cuts = score["cuts_at"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| start(n.as_str()?))
        .map(|c| round(c, 3))
        .collect();
    let end = score["end_at"]
        .as_str()
        .and_then(start)
        .unwrap_or(total - 8.0);
    Some((round(total, 3), cuts, round(end, 3)))
}

impl Job<'_> {
    /// The version's media files this computer has, by location, filtered by
    /// extension.
    fn clips(&self, kinds: &[&[&str]]) -> Vec<(&str, &Path)> {
        self.files
            .iter()
            .filter(|(loc, _)| {
                let e = extension(loc);
                kinds.iter().any(|k| k.contains(&e.as_str()))
            })
            .map(|(loc, p)| (loc.as_str(), p.as_path()))
            .collect()
    }

    /// Every clip's words with times and its pauses.
    pub(super) fn transcribe(&self) -> Result<Value, String> {
        let whisper = self.tools.whisper.as_ref().ok_or_else(|| {
            "Name whisper.cpp and its model with --whisper PATH --model PATH.".to_owned()
        })?;
        let mut clips = serde_json::Map::new();
        for (k, (loc, file)) in self.clips(&[&VIDEO, &AUDIO]).into_iter().enumerate() {
            if let Some(speech) = self.speech(k, file, Some(whisper))? {
                clips.insert(loc.to_owned(), speech);
            }
        }
        if clips.is_empty() {
            return Err("This version has no clips with sound on this computer.".into());
        }
        Ok(json!({ "clips": clips }))
    }

    /// A clip's speech (`{"duration", "quiet", "words", "how"}`), or `None`
    /// when it has no sound.
    fn speech(
        &self,
        k: usize,
        file: &Path,
        whisper: Option<&Whisper>,
    ) -> Result<Option<Value>, String> {
        let probed = self.probe(file)?;
        if !probed.audio {
            return Ok(None);
        }
        let wav = self.dir.join(format!("sound16k-{k}.wav"));
        self.ffmpeg(
            &[
                "-i".into(),
                file.to_string_lossy().into_owned(),
                "-map".into(),
                "0:a:0".into(),
                "-ac".into(),
                "1".into(),
                "-ar".into(),
                "16000".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                wav.to_string_lossy().into_owned(),
            ],
            &format!("read the sound of {}", file_name(&file.to_string_lossy())),
        )?;
        let samples = wav_samples(&std::fs::read(&wav).map_err(|e| e.to_string())?);
        let quiet = pauses(&samples, 16_000, 0.12);
        let words = match whisper {
            Some(w) => Some(fix_words(&self.whisper_words(&wav, w)?, &w.words.fixes)),
            None => None,
        };
        let _ = std::fs::remove_file(&wav);
        Ok(Some(json!({
            "duration": round(probed.dur, 3),
            "quiet": quiet,
            "how": words.as_ref().map(|_| "whisper.cpp"),
            "words": words.unwrap_or_default(),
        })))
    }

    /// Words with times from whisper.cpp, one word a segment.
    fn whisper_words(&self, wav: &Path, w: &Whisper) -> Result<Vec<Value>, String> {
        let stem = wav.with_extension("");
        let mut cmd = std::process::Command::new(&w.cli);
        cmd.arg("-m")
            .arg(&w.model)
            .arg("-f")
            .arg(wav)
            .args(["-ojf", "-of"])
            .arg(&stem)
            .args(["-ml", "1", "-sow", "-np"]);
        if !w.words.prompt.is_empty() {
            cmd.args(["--prompt", &w.words.prompt]);
        }
        let out = cmd
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|_| "Install whisper.cpp on this computer.".to_owned())?;
        if !out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&out.stderr));
            return Err("whisper.cpp couldn't transcribe the sound.".into());
        }
        let json_path = PathBuf::from(format!("{}.json", stem.display()));
        let data: Value =
            serde_json::from_slice(&std::fs::read(&json_path).map_err(|e| e.to_string())?)
                .map_err(|_| "Read whisper.cpp's transcript.".to_owned())?;
        let _ = std::fs::remove_file(&json_path);
        Ok(data["transcription"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|seg| {
                let t = seg["text"].as_str()?.trim();
                (!t.is_empty()).then(|| {
                    json!({
                        "w": t,
                        "from": round(num(&seg["offsets"], "from", 0.0) / 1000.0, 2),
                        "to": round(num(&seg["offsets"], "to", 0.0) / 1000.0, 2),
                    })
                })
            })
            .collect())
    }

    /// Each video clip's facts, a poster and a strip of six frames, and its
    /// speech (words when whisper.cpp is named, its pauses either way). The
    /// first output lists the clips; each names its poster's and strip's
    /// place in the outputs.
    pub(super) fn import(&self) -> Result<Vec<Value>, String> {
        let clips = self.clips(&[&VIDEO]);
        if clips.is_empty() {
            return Err("This version has no video clips on this computer.".into());
        }
        let mut outputs = vec![Value::Null];
        let mut sources = Vec::new();
        for (k, (loc, file)) in clips.into_iter().enumerate() {
            let p = self.probe(file)?;
            let poster = self.dir.join(format!("import-{k}-poster.jpg"));
            self.ffmpeg(
                &[
                    "-ss".into(),
                    format!("{:.2}", (p.dur / 2.0).min(5.0)),
                    "-i".into(),
                    file.to_string_lossy().into_owned(),
                    "-frames:v".into(),
                    "1".into(),
                    "-vf".into(),
                    "scale=960:-2".into(),
                    "-q:v".into(),
                    "3".into(),
                    poster.to_string_lossy().into_owned(),
                ],
                &format!("make a poster of {}", file_name(loc)),
            )?;
            let strip = self.strip(k, file, p.dur)?;
            outputs.push(output(
                ArtifactRole::Poster,
                "image/jpeg",
                &read_bounded(&poster)?,
            ));
            outputs.push(output(
                ArtifactRole::Strip,
                "image/jpeg",
                &read_bounded(&strip)?,
            ));
            sources.push(json!({
                "src": loc,
                "name": file_name(loc),
                "w": p.w,
                "h": p.h,
                "fps": round(p.fps, 3),
                "duration": round(p.dur, 3),
                "audio": p.audio,
                "poster": outputs.len() - 2,
                "strip": outputs.len() - 1,
                "speech": self.speech(k, file, self.tools.whisper.as_ref())?,
            }));
        }
        outputs[0] = output(
            ArtifactRole::Export,
            "application/json",
            &serde_json::to_vec(&json!({ "sources": sources })).map_err(|e| e.to_string())?,
        );
        Ok(outputs)
    }

    /// Six frames across the clip with their times above them (`lab.py`'s
    /// `contact`), as JPEG.
    fn strip(&self, k: usize, file: &Path, dur: f64) -> Result<PathBuf, String> {
        let (tw, th) = TILE;
        let n = TILES as u32;
        let (w, h) = (tw * n + 6 * (n - 1), th + 30);
        let mut sheet: Vec<u8> = [14u8, 14, 16].repeat((w * h) as usize);
        for i in 0..TILES {
            let t = dur * (i as f64 + 0.5) / TILES as f64;
            let raw = self.raw(&[
                "-ss".into(),
                format!("{t:.2}"),
                "-i".into(),
                file.to_string_lossy().into_owned(),
                "-frames:v".into(),
                "1".into(),
                "-vf".into(),
                format!("scale={tw}:{th}:force_original_aspect_ratio=decrease,pad={tw}:{th}:-1:-1"),
            ])?;
            if raw.len() >= (tw * th * 3) as usize {
                let x0 = i as u32 * (tw + 6);
                for y in 0..th {
                    let from = (y * tw * 3) as usize;
                    let to = (((y + 30) * w + x0) * 3) as usize;
                    sheet[to..to + (tw * 3) as usize]
                        .copy_from_slice(&raw[from..from + (tw * 3) as usize]);
                }
            }
            text::draw_label(
                &mut sheet,
                w,
                h,
                f64::from(i as u32 * (tw + 6) + 6),
                8.0,
                &clock(t),
                12.0,
                [220, 220, 225],
                &self.tools.fonts,
            );
        }
        let raw = self.dir.join(format!("import-{k}-strip.rgb"));
        std::fs::write(&raw, &sheet).map_err(|e| e.to_string())?;
        let out = self.dir.join(format!("import-{k}-strip.jpg"));
        self.ffmpeg(
            &[
                "-f".into(),
                "rawvideo".into(),
                "-pix_fmt".into(),
                "rgb24".into(),
                "-s".into(),
                format!("{w}x{h}"),
                "-i".into(),
                raw.to_string_lossy().into_owned(),
                "-frames:v".into(),
                "1".into(),
                "-q:v".into(),
                "3".into(),
                out.to_string_lossy().into_owned(),
            ],
            "make the strip",
        )?;
        Ok(out)
    }

    /// The cut's score, made to its length by `music.py`, as FLAC.
    pub(super) fn score(&self) -> Result<Value, String> {
        let tool = self.tools.score.as_ref().ok_or_else(|| {
            "Name Python and music.py with --python PATH --music PATH.".to_owned()
        })?;
        let (seconds, cuts, end) = score_args(self.cut).ok_or("This cut has no score.")?;
        let wav = self.dir.join("score.wav");
        let cuts: Vec<String> = cuts.iter().map(f64::to_string).collect();
        let out = std::process::Command::new(&tool.python)
            .arg(&tool.script)
            .arg(&wav)
            .args(["--seconds", &seconds.to_string()])
            .args(["--cuts", &cuts.join(",")])
            .args(["--end", &end.to_string()])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|_| "Install Python on this computer.".to_owned())?;
        if !out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&out.stderr));
            return Err("music.py couldn't make the score.".into());
        }
        let flac = self.dir.join("score.flac");
        self.ffmpeg(
            &[
                "-i".into(),
                wav.to_string_lossy().into_owned(),
                "-c:a".into(),
                "flac".into(),
                flac.to_string_lossy().into_owned(),
            ],
            "encode the score",
        )?;
        Ok(output(
            ArtifactRole::Export,
            "audio/flac",
            &read_bounded(&flac)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_mishearings_are_fixed_and_keep_their_punctuation() {
        let words: Vec<Value> = [
            ("Open", 0.0, 0.3),
            ("agents,", 0.3, 0.6),
            ("Coda", 0.7, 1.0),
            ("runs.", 1.0, 1.2),
        ]
        .iter()
        .map(|(w, a, b)| json!({"w": w, "from": a, "to": b}))
        .collect();
        let fixed = fix_words(&words, &Words::default().fixes);
        let said: Vec<&str> = fixed.iter().map(|w| w["w"].as_str().unwrap()).collect();
        assert_eq!(said, ["OpenAgents,", "Coder", "runs."]);
        assert_eq!(fixed[0]["heard"], "Open agents,");
        assert_eq!(
            (fixed[0]["from"].as_f64(), fixed[0]["to"].as_f64()),
            (Some(0.0), Some(0.6))
        );
    }

    #[test]
    fn pauses_are_the_quiet_stretches() {
        // 0.5 s of tone, 0.4 s of silence, 0.5 s of tone at 16 kHz.
        let tone = |n: usize| (0..n).map(|i| ((i as f64 * 0.2).sin() * 12000.0) as i16);
        let samples: Vec<i16> = tone(8000)
            .chain(std::iter::repeat_n(0, 6400))
            .chain(tone(8000))
            .collect();
        let quiet = pauses(&samples, 16_000, 0.12);
        assert_eq!(quiet.len(), 1, "{quiet:?}");
        let (from, to) = (
            quiet[0]["from"].as_f64().unwrap(),
            quiet[0]["to"].as_f64().unwrap(),
        );
        assert!(
            (from - 0.5).abs() < 0.05 && (to - 0.9).abs() < 0.05,
            "{from} {to}"
        );
    }

    #[test]
    fn the_score_follows_the_cut() {
        let cut = json!({"shots": [{"name": "a", "dur": 4}, {"name": "b", "dur": 6}, {"name": "end", "dur": 10}],
                         "score": {"cuts_at": ["b", "gone"], "end_at": "end"}});
        assert_eq!(score_args(&cut), Some((20.0, vec![4.0], 10.0)));
        let mut cut = cut;
        cut["score"]["end_at"] = Value::Null;
        assert_eq!(score_args(&cut).unwrap().2, 12.0);
        assert_eq!(score_args(&json!({"shots": []})), None);
        assert_eq!(clock(65.0), "1:05");
        assert_eq!(clock(65.5), "1:05.5");
    }
}
