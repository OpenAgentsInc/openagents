//! Video Lab's checks: the structure rules of `lab.py check` and every rule
//! of `cut.py check`, over a document and its [`Measured`] facts. Problems
//! refuse an edit; notes don't. Messages are word for word the Lab's.

use serde_json::Value;

use crate::fmt::{f2, g, py};
use crate::measure::{Measured, Source, Speech};
use crate::schema::{schema, validate};

/// A shot can show a still picture this long unless it sets `still_ok`.
pub const STILL_OK: f64 = 0.4;
const FITS: [&str; 3] = ["pad", "blur", "crop"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    pub problems: Vec<String>,
    pub notes: Vec<String>,
}

fn num(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn head(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `kind` is `cut.v1`, `shot.v1` or `scene.v1`.
pub fn check(kind: &str, data: &Value, m: &Measured) -> Checked {
    let Some(sch) = schema(kind) else {
        return Checked {
            problems: vec![format!("{kind} isn't a Video Lab document kind")],
            notes: vec![],
        };
    };
    let problems = validate(data, &sch, &sch, "");
    if !problems.is_empty() {
        return Checked {
            problems,
            notes: vec![],
        };
    }
    match kind {
        "cut.v1" => check_cut_document(data, m),
        "shot.v1" => check_shot(data, m),
        _ => check_scene(data),
    }
}

fn check_cut_document(data: &Value, m: &Measured) -> Checked {
    let mut problems = Vec::new();
    let shots = data["shots"].as_array().cloned().unwrap_or_default();
    let names: Vec<&str> = shots
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default())
        .collect();
    let mut dup: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| names.iter().filter(|x| x == &n).count() > 1)
        .collect();
    dup.sort();
    dup.dedup();
    for n in dup {
        problems.push(format!(
            "two shots are called {n}; shot names must be different"
        ));
    }
    let score = data
        .get("score")
        .filter(|s| !s.is_null() && s.as_object().is_some_and(|o| !o.is_empty()));
    if let Some(score) = score {
        let mut marks: Vec<String> = score["cuts_at"]
            .as_array()
            .into_iter()
            .flatten()
            .map(py)
            .collect();
        if let Some(end) = score
            .get("end_at")
            .filter(|e| !e.is_null() && e.as_str() != Some(""))
        {
            marks.push(py(end));
        }
        for n in marks {
            if !names.contains(&n.as_str()) {
                problems.push(format!(
                    "the score marks a shot called {n}, which the cut doesn't have"
                ));
            }
        }
    }
    for shot in &shots {
        let x = num(shot, "xfade", 0.0);
        if x > num(shot, "dur", 0.0) {
            problems.push(format!(
                "{}: its crossfade ({}s) is longer than the shot ({}s)",
                s(shot, "name"),
                py(&shot["xfade"]),
                py(&shot["dur"])
            ));
        }
    }
    let mut test = data.clone();
    if score.is_some()
        && let Some(o) = test.as_object_mut()
    {
        o.shift_remove("music");
    }
    let cut = check_cut(&test, m);
    problems.extend(cut.problems);
    Checked {
        problems,
        notes: cut
            .notes
            .into_iter()
            .filter(|n| !n.contains("upscaled"))
            .collect(),
    }
}

/// Shot starts, tails (the next shot's crossfade) and the total, as `cut.py load` computes them.
pub fn layout(cut: &Value) -> (Vec<(f64, f64)>, f64) {
    let shots = cut["shots"].as_array().cloned().unwrap_or_default();
    let mut t = 0.0;
    let mut out = Vec::new();
    for (i, shot) in shots.iter().enumerate() {
        let tail = shots
            .get(i + 1)
            .map(|n| num(n, "xfade", 0.0))
            .unwrap_or(0.0);
        out.push((t, tail));
        t += num(shot, "dur", 0.0);
    }
    (out, t)
}

/// `cut.py check`: the checks that need no rendering.
pub fn check_cut(cut: &Value, m: &Measured) -> Checked {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let [w, h] = m.size.unwrap_or_else(|| {
        let size = cut["size"].as_array();
        let at = |i: usize| {
            size.and_then(|a| a.get(i))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32
        };
        [at(0), at(1)]
    });
    let (wf, hf) = (f64::from(w), f64::from(h));
    let bpm = cut.get("bpm").cloned().unwrap_or(Value::from(120));
    let beat = 60.0 / bpm.as_f64().unwrap_or(120.0);
    let (layout, total) = layout(cut);
    let shots = cut["shots"].as_array().cloned().unwrap_or_default();
    let no_source = Source::default();

    for (shot, (start, tail)) in shots.iter().zip(&layout) {
        let (start, tail) = (*start, *tail);
        let name = s(shot, "name");
        if cut.get("timeline").is_none() && (start / beat - (start / beat).round()).abs() > 1e-6 {
            problems.push(format!(
                "{name}: starts at {}s, off the {} BPM beat grid",
                f2(start),
                py(&bpm)
            ));
        }
        if let Some(title) = shot.get("title") {
            let measured = m.titles.get(name);
            if title.get("under").is_some_and(|u| !u.is_null())
                && let Some(missing) = measured.and_then(|t| t.under_missing.as_ref())
            {
                let line = format!("{name}: missing {missing}");
                if m.allow_missing {
                    notes.push(line)
                } else {
                    problems.push(line)
                }
            }
            let lines = measured.map(|t| t.lines.clone()).unwrap_or_default();
            let safe = (wf * 0.1, hf * 0.1, wf * 0.9, hf * 0.9);
            for l in &lines {
                let [x0, y0, x1, y1] = l.bounds;
                if x0 < safe.0 || x1 > safe.2 || y0 < safe.1 || y1 > safe.3 {
                    problems.push(format!("{name}: '{}' leaves the title-safe area", l.text));
                }
                if f64::from(l.px) < hf * 0.025 {
                    problems.push(format!(
                        "{name}: '{}' is {}px, too small to read at {h}p",
                        l.text, l.px
                    ));
                }
            }
            for pair in lines.windows(2) {
                if pair[0].bounds[3] > pair[1].bounds[1] {
                    problems.push(format!(
                        "{name}: '{}' overlaps '{}'",
                        pair[0].text, pair[1].text
                    ));
                }
            }
            continue;
        }
        let src_text = s(shot, "src");
        let p = m.sources.get(src_text).unwrap_or(&no_source);
        if !p.exists {
            let line = format!("{name}: missing {}", expanded(src_text, p));
            if m.allow_missing {
                notes.push(line)
            } else {
                problems.push(line)
            }
            continue;
        }
        if let Some(over) = shot.get("over").filter(|o| !o.is_null()) {
            let path = s(over, "src");
            if !m.files.get(path).copied().unwrap_or(false) {
                problems.push(format!("{name}: missing overlay {path}"));
            }
        }
        let dur = num(shot, "dur", 0.0);
        let speed = num(shot, "speed", 1.0);
        let play = shot.get("play").and_then(Value::as_f64);
        let played = play.unwrap_or(dur + tail);
        let start_in = num(shot, "in", 0.0);
        let need = start_in + played * speed;
        if p.dur + 1e-3 < need {
            problems.push(format!(
                "{name}: {} is {}s, the cut needs {}s",
                p.name,
                f2(p.dur),
                f2(need)
            ));
        }
        let fit = shot.get("fit").and_then(Value::as_str).unwrap_or("pad");
        if !FITS.contains(&fit) {
            problems.push(format!("{name}: fit is pad, blur or crop, not {fit}"));
        }
        if p.h > 0 && (f64::from(p.w) / f64::from(p.h) - wf / hf).abs() > 0.01 && fit == "pad" {
            problems.push(format!(
                "{name}: {} is {}x{}, not {w}:{h}; it would be letterboxed (set its fit to blur or crop)",
                p.name, p.w, p.h
            ));
        }
        let sound = shot.get("sound").is_some_and(truthy);
        if sound {
            speech_problems(shot, name, p, &mut problems, &mut notes);
        }
        if p.h < h {
            notes.push(format!("{name}: {} is {}p, upscaled to {h}p", p.name, p.h));
        }
        let end_in = start_in + played * speed;
        if p.dur + 1e-3 >= need && !sound {
            let ok = num(shot, "still_ok", STILL_OK);
            let held = dur - played;
            if play.is_some() && held > ok + 1e-6 {
                problems.push(format!(
                    "{name}: holds a still frame for {}s (only {}s of {} plays in a {}s slot), so the animation pauses; end the shot when the clip's move does, on the beat grid, or play more of the clip",
                    g(held), g(played), p.name, g(dur)
                ));
            }
            for still in &p.stills {
                let (a, b) = (still.from.max(start_in), still.to.min(end_in));
                if b - a > ok + 1e-6 {
                    problems.push(format!(
                        "{name}: the picture is still from {}s to {}s of {} ({:.1}s on screen); end the shot before {}s or start it after {}s",
                        f2(a), f2(b), p.name, b - a, f2(a), f2(b)
                    ));
                }
            }
        }
        for b in &p.beats {
            if b.from + 1e-3 < end_in && end_in < b.to - 1e-3 {
                let need = (b.to - start_in) / speed;
                let on_grid = (need / beat - 1e-6).ceil() * beat - tail;
                problems.push(format!(
                    "{name}: cuts {} off at {}s of {}; it runs {}s to {}s, so the shot needs at least {}s of the clip (now {}s; {}s on the beat grid), or should end before {}s",
                    b.what, f2(end_in), p.name, f2(b.from), f2(b.to), f2(need), f2(played), g(on_grid.max(need)), f2(b.from)
                ));
            }
            if start_in - 1e-3 > b.from && start_in + 1e-3 < b.to && !b.enter_ok {
                notes.push(format!(
                    "{name}: starts partway into {} ({}s to {}s)",
                    b.what,
                    f2(b.from),
                    f2(b.to)
                ));
            }
        }
    }

    let captions = cut["captions"].as_array().cloned().unwrap_or_default();
    let mut order: Vec<usize> = (0..captions.len()).collect();
    order.sort_by(|a, b| {
        num(&captions[*a], "at", 0.0)
            .partial_cmp(&num(&captions[*b], "at", 0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if !order.is_empty() {
        if let Some(px) = m.caption_px
            && f64::from(px) < hf * 0.022
        {
            problems.push(format!("captions are {px}px, too small to read at {h}p"));
        }
        for pair in order.windows(2) {
            let (a, b) = (&captions[pair[0]], &captions[pair[1]]);
            if num(a, "at", 0.0) + num(a, "dur", 0.0) > num(b, "at", 0.0) + 1e-3 {
                problems.push(format!(
                    "caption '{}' overlaps '{}'",
                    head(s(a, "text"), 30),
                    head(s(b, "text"), 30)
                ));
            }
        }
        for &i in &order {
            let c = &captions[i];
            let text = s(c, "text");
            let (at, cdur) = (num(c, "at", 0.0), num(c, "dur", 0.0));
            if let Some(l) = m.captions.get(text) {
                if l.lines > 2 {
                    problems.push(format!(
                        "caption '{}' wraps to {} lines",
                        head(text, 40),
                        l.lines
                    ));
                }
                let [x, y, _, bh] = l.bounds;
                if x < wf * 0.05 || y + bh > hf * 0.95 {
                    problems.push(format!(
                        "caption '{}' leaves the action-safe area",
                        head(text, 40)
                    ));
                }
            }
            if at < 0.0 || at + cdur > total + 1e-3 {
                problems.push(format!(
                    "caption '{}' runs off the timeline",
                    head(text, 40)
                ));
            }
            for (shot, (start, _)) in shots.iter().zip(&layout) {
                if !shot.get("sound").is_some_and(truthy) || shot.get("src").is_none() {
                    continue;
                }
                let sdur = num(shot, "dur", 0.0);
                if !(*start - 1e-3 <= at && at < start + sdur - 1e-3) {
                    continue;
                }
                let Some(sp) = m
                    .sources
                    .get(s(shot, "src"))
                    .filter(|p| p.exists)
                    .and_then(|p| p.speech.as_ref())
                else {
                    continue;
                };
                let a = num(shot, "in", 0.0) + at - start;
                let (ok, spoken) = caption_matches(text, sp, a, a + cdur);
                if !ok {
                    problems.push(format!(
                        "caption '{}' over {} doesn't match what's said there ('{}')",
                        head(text, 40),
                        s(shot, "name"),
                        head(&spoken, 60)
                    ));
                }
            }
        }
    }

    let mut voices: Vec<Value> = cut
        .get("voice")
        .filter(|v| truthy(v))
        .cloned()
        .into_iter()
        .collect();
    voices.extend(cut["voices"].as_array().cloned().unwrap_or_default());
    let mut spans: Vec<(f64, f64, String)> = Vec::new();
    for v in &voices {
        let src = s(v, "src");
        let Some(Some(len)) = m.audio.get(src) else {
            problems.push(format!("voiceover missing: {src}"));
            continue;
        };
        let name = file_name(src);
        let a = num(v, "at", 0.0);
        let b = a + len;
        spans.push((a, b, name.clone()));
        if b > total + 1e-3 {
            problems.push(format!(
                "voiceover {name} runs to {}s, past the end at {}s",
                f2(b),
                f2(total)
            ));
        }
        for (shot, (start, _)) in shots.iter().zip(&layout) {
            if shot.get("sound").is_some_and(truthy)
                && *start < b - 1e-3
                && a < start + num(shot, "dur", 0.0) - 1e-3
            {
                problems.push(format!(
                    "voiceover {name} ({}s to {}s) plays over {}, where someone is speaking",
                    f2(a),
                    f2(b),
                    s(shot, "name")
                ));
            }
        }
    }
    spans.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    for pair in spans.windows(2) {
        let (x, y) = (&pair[0], &pair[1]);
        if y.0 < x.1 - 1e-3 {
            problems.push(format!(
                "voiceover {} starts at {}s, before {} ends at {}s",
                y.2,
                f2(y.0),
                x.2,
                f2(x.1)
            ));
        }
    }

    if let Some(music) = cut
        .get("music")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
    {
        match m.music {
            Some(Some(mdur)) => {
                if mdur + 1e-3 < total {
                    problems.push(format!("music is {}s, the cut is {}s", f2(mdur), f2(total)));
                }
            }
            _ => problems.push(format!("music missing: {music}")),
        }
    }
    Checked { problems, notes }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// The path as `expand` writes it in a message.
fn expanded(src: &str, p: &Source) -> String {
    if !p.path.is_empty() {
        return p.path.clone();
    }
    match src.strip_prefix("~/") {
        Some(rest) => std::env::var("HOME")
            .map(|h| format!("{h}/{rest}"))
            .unwrap_or_else(|_| src.to_owned()),
        None => src.to_owned(),
    }
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

fn speech_problems(
    shot: &Value,
    name: &str,
    p: &Source,
    problems: &mut Vec<String>,
    notes: &mut Vec<String>,
) {
    if !p.audio {
        problems.push(format!(
            "{name}: keeps its sound, but {} has no sound",
            p.name
        ));
        return;
    }
    if num(shot, "speed", 1.0) != 1.0 {
        problems.push(format!(
            "{name}: keeps its sound, so it plays at its own speed (remove speed)"
        ));
    }
    if let Some(play) = shot.get("play").and_then(Value::as_f64)
        && play < num(shot, "dur", 0.0) - 1e-6
    {
        problems.push(format!(
            "{name}: keeps its sound, so it can't hold a frame (remove play)"
        ));
    }
    let Some(sp) = &p.speech else {
        notes.push(format!(
            "{name}: keeps its sound, but {} has no transcript yet, so its cuts can't be checked against the speech (lab_import makes one)",
            p.name
        ));
        return;
    };
    let a = num(shot, "in", 0.0);
    for (label, t) in [("starts", a), ("ends", a + num(shot, "dur", 0.0))] {
        if !in_pause(t, sp, 0.02) {
            let word = word_at(t, sp)
                .map(|w| format!(", in '{w}'"))
                .unwrap_or_default();
            let pauses: Vec<String> = near_pauses(t, sp, 2)
                .iter()
                .map(|x| format!("{}s", f2(*x)))
                .collect();
            problems.push(format!(
                "{name}: {label} mid-speech at {}s of {}{word}; cut in a pause, e.g. at {}",
                f2(t),
                p.name,
                pauses.join(" or ")
            ));
        }
    }
}

pub fn in_pause(t: f64, sp: &Speech, slack: f64) -> bool {
    if t >= sp.duration.unwrap_or(1e9) - slack {
        return true;
    }
    sp.quiet
        .iter()
        .any(|q| q.from - slack <= t && t <= q.to + slack)
}

pub fn word_at(t: f64, sp: &Speech) -> Option<String> {
    if let Some(w) = sp.words.iter().find(|w| w.from < t && t < w.to) {
        return Some(w.w.clone());
    }
    let mut best: Option<(&crate::measure::Word, f64)> = None;
    for w in &sp.words {
        let d = (w.from - t).abs().min((w.to - t).abs());
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((w, d));
        }
    }
    best.map(|(w, _)| w.w.clone())
}

pub fn near_pauses(t: f64, sp: &Speech, n: usize) -> Vec<f64> {
    let mut qs = sp.quiet.clone();
    qs.sort_by(|a, b| {
        let da = ((a.from + a.to) / 2.0 - t).abs();
        let db = ((b.from + b.to) / 2.0 - t).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<f64> = qs
        .iter()
        .take(n)
        .map(|q| ((q.from + q.to) / 2.0 * 100.0).round() / 100.0)
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// The words in a text, as the Lab compares them (`cut.py said`).
pub fn said(text: &str) -> Vec<String> {
    let lower = text.to_lowercase().replace('-', " ");
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '\'' {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|w| w.trim_matches('\'').to_owned())
        .collect()
}

pub fn heard(sp: &Speech, a: f64, b: f64) -> Vec<String> {
    sp.words
        .iter()
        .filter(|w| w.to > a && w.from < b)
        .map(|w| w.w.clone())
        .collect()
}

pub fn caption_matches(text: &str, sp: &Speech, a: f64, b: f64) -> (bool, String) {
    let words = said(text);
    if words.is_empty() {
        return (true, String::new());
    }
    let spoken = heard(sp, a - 1.0, b + 1.0).join(" ");
    let pool: std::collections::HashSet<String> = said(&spoken).into_iter().collect();
    let found = words.iter().filter(|w| pool.contains(*w)).count();
    (found as f64 >= 0.6 * words.len() as f64, spoken)
}

fn check_shot(data: &Value, m: &Measured) -> Checked {
    let mut problems = Vec::new();
    let renderer = s(data, "renderer");
    if renderer == "studio" && data.get("spec").is_none() {
        problems.push("a studio shot needs its spec".into());
    }
    if renderer == "film-script" {
        if data.get("script").is_none() || data.get("args").is_none() {
            problems.push("a film-script shot needs 'script' and 'args'".into());
        } else if !m.files.get(s(data, "script")).copied().unwrap_or(false) {
            problems.push(format!("{} isn't in the repository", s(data, "script")));
        } else {
            let has_out = match &data["args"] {
                Value::String(a) => a.contains("{out}"),
                Value::Array(items) => items.iter().any(|i| i.as_str() == Some("{out}")),
                _ => false,
            };
            if !has_out {
                problems.push("the script's args need {out}, where the render goes".into());
            }
        }
    }
    if let Some(inputs) = data.get("inputs").and_then(Value::as_object) {
        for (name, p) in inputs {
            let path = p.as_str().unwrap_or_default();
            if !m.files.get(path).copied().unwrap_or(false) {
                problems.push(format!("input {name} is missing: {path}"));
            }
        }
    }
    Checked {
        problems,
        notes: vec![],
    }
}

fn check_scene(data: &Value) -> Checked {
    let mut problems = Vec::new();
    let keys = data["shot"]["rig"]["keys"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let ts: Vec<f64> = keys.iter().map(|k| num(k, "t", 0.0)).collect();
    let mut sorted = ts.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if ts != sorted {
        problems.push("the camera keys must be in time order".into());
    }
    let duration = &data["shot"]["duration"];
    if let Some(last) = keys.last()
        && num(last, "t", 0.0) > duration.as_f64().unwrap_or(0.0) + 1e-6
    {
        problems.push(format!(
            "a camera key at {}s is past the scene's end ({}s)",
            py(&last["t"]),
            py(duration)
        ));
    }
    Checked {
        problems,
        notes: vec![],
    }
}

/// Problems as a person reads them: a run of shots knocked off the beat grid
/// by one change is one sentence (`lab.py plain`).
pub fn plain(problems: &[String]) -> Vec<String> {
    let parse = |p: &str| -> Option<(String, String, String)> {
        let (name, rest) = p.split_once(": starts at ")?;
        if name.contains(':') {
            return None;
        }
        let (at, rest) = rest.split_once("s, off the ")?;
        let bpm = rest.strip_suffix(" BPM beat grid")?;
        let numeric = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit() || c == '.');
        (numeric(at) && numeric(bpm)).then(|| (name.to_owned(), at.to_owned(), bpm.to_owned()))
    };
    let grid: Vec<(String, String, String)> = problems.iter().filter_map(|p| parse(p)).collect();
    let mut rest: Vec<String> = problems
        .iter()
        .filter(|p| parse(p).is_none())
        .cloned()
        .collect();
    if grid.len() > 1 {
        let (name, at, bpm) = &grid[0];
        let n = grid.len() - 1;
        let after = if n == 1 {
            "the shot after it".to_owned()
        } else {
            format!("the {n} shots after it")
        };
        let beat = 60.0 / bpm.parse::<f64>().unwrap_or(120.0);
        rest.insert(0, format!(
            "{name} and {after} would start off the {bpm} BPM beat grid ({name} at {at}s); keep shot lengths to whole beats (multiples of {} s)",
            g(beat)
        ));
        rest
    } else {
        let mut out: Vec<String> = problems
            .iter()
            .filter(|p| parse(p).is_some())
            .cloned()
            .collect();
        out.extend(rest);
        out
    }
}
