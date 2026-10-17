//! JSON bridge for `scripts/film/lab.py` and `cut.py`: one request on stdin,
//! one answer on stdout.
//!
//! - `{"action": "check", "kind", "document", "measured"}` gives `{"problems", "notes"}`
//! - `{"action": "validate", "kind", "document"}` gives `{"problems"}` (the schema alone)
//! - `{"action": "check_cut", "document", "measured"}` gives `cut.py check`'s `{"problems", "notes"}`
//! - `{"action": "patch", "document", "patch"}` gives `{"document"}`
//! - `{"action": "prepare", "kind", "document", "patch", "summary", "ripple_captions"?}`
//!   gives `{"document", "said"}` (the edited document before it's checked)
//! - `{"action": "commit", "kind", "document", "new", "said", "measured_old", "measured_new"}`
//!   gives `{"said", "notes"}`, or the refusal ("Not saved. ...")
//! - `{"action": "ripple", "document", "new"}` gives `{"document", "moved"}`
//! - `{"action": "plain", "problems"}`, `{"action": "seconds", "at", "document"?}`,
//!   `{"action": "vo_captions", "text", "start", "wpm"}`, `{"action": "tools"}`
//!
//! A refusal is `{"refused": message, "code"}`; a malformed request is `{"error"}`.
//!
//! `video-lab NAME-job --ffmpeg PATH --ffprobe PATH [--quality preview|review]
//! [--fonts DIR] [--whisper PATH --model PATH [--words PATH]] [--python PATH
//! --music PATH]` is a Labs worker's handler for job NAME (`render`, `frames`,
//! `measure`, `transcribe`, `import` or `score`; `video_lab::runner`): the job
//! on stdin, the handler output on stdout, and exit status 1 with the reason
//! on stderr when it can't run. `--fonts` is the film fonts folder with
//! `Geist-Regular.ttf` for captions; `--words` replaces `lab_words.json`.

use std::io::Read as _;

use serde_json::{Value, json};
use video_lab::{check, edit, measure::Measured, patch};

fn measured_from(input: &Value, key: &str) -> Result<Measured, String> {
    serde_json::from_value(input.get(key).cloned().unwrap_or(json!({})))
        .map_err(|e| format!("the {key} measurements don't read ({e})"))
}

fn run(input: &Value) -> Result<Value, lab_kit::Refusal> {
    let bad = |e: String| lab_kit::Refusal::new("request", e);
    let measured = measured_from(input, "measured").map_err(bad)?;
    let doc = input.get("document").cloned().unwrap_or(Value::Null);
    let kind = input
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("cut.v1");
    Ok(
        match input
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "check" => {
                let c = check::check(kind, &doc, &measured);
                json!({"problems": c.problems, "notes": c.notes})
            }
            "validate" => {
                let problems = video_lab::schema::schema(kind)
                    .map(|sch| video_lab::schema::validate(&doc, &sch, &sch, ""))
                    .unwrap_or_else(|| vec![format!("{kind} isn't a Video Lab document kind")]);
                json!({"problems": problems})
            }
            "check_cut" => {
                let c = check::check_cut(&doc, &measured);
                json!({"problems": c.problems, "notes": c.notes})
            }
            "patch" => {
                json!({"document": patch::apply_patch(&doc, input.get("patch").unwrap_or(&Value::Null))?})
            }
            "prepare" => {
                let (new, said) = edit::prepare(
                    kind,
                    &doc,
                    input.get("patch").unwrap_or(&Value::Null),
                    input
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    input
                        .get("ripple_captions")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                )?;
                json!({"document": new, "said": said})
            }
            "commit" => {
                let new = input.get("new").cloned().unwrap_or(Value::Null);
                let said: Vec<String> =
                    serde_json::from_value(input.get("said").cloned().unwrap_or(json!([])))
                        .unwrap_or_default();
                let e = edit::commit(
                    kind,
                    &doc,
                    new,
                    said,
                    &measured_from(input, "measured_old").map_err(bad)?,
                    &measured_from(input, "measured_new").map_err(bad)?,
                )?;
                json!({"said": e.said, "notes": e.notes})
            }
            "ripple" => {
                let mut new = input.get("new").cloned().unwrap_or(Value::Null);
                let moved = edit::ripple(&doc, &mut new);
                json!({"document": new, "moved": moved})
            }
            "plain" => {
                let problems: Vec<String> =
                    serde_json::from_value(input.get("problems").cloned().unwrap_or(json!([])))
                        .unwrap_or_default();
                json!({"problems": check::plain(&problems)})
            }
            "seconds" => {
                json!({"seconds": edit::seconds(input.get("at").unwrap_or(&Value::Null), input.get("document"))?})
            }
            "vo_captions" => json!({"captions": edit::vo_captions(
                input.get("text").and_then(Value::as_str).unwrap_or_default(),
                input.get("start").and_then(Value::as_f64).unwrap_or(0.0),
                input.get("wpm").and_then(Value::as_f64).unwrap_or(150.0),
            )}),
            "tools" => serde_json::from_str(video_lab::TOOLS).unwrap_or(json!([])),
            other => json!({"error": format!("unknown action {other:?}")}),
        },
    )
}

/// A worker handler: `NAME-job` with its tools named by flag.
fn job(name: &str, args: &[String]) -> std::process::ExitCode {
    let flag = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(std::path::PathBuf::from)
    };
    let (Some(ffmpeg), Some(ffprobe)) = (flag("--ffmpeg"), flag("--ffprobe")) else {
        eprintln!("Name ffmpeg and ffprobe with --ffmpeg PATH --ffprobe PATH.");
        return std::process::ExitCode::from(2);
    };
    let quality = match flag("--quality").as_deref().and_then(|q| q.to_str()) {
        None | Some("preview") => video_lab::runner::Quality::Preview,
        Some("review") => video_lab::runner::Quality::Review,
        Some(_) => {
            eprintln!("Choose --quality preview or review.");
            return std::process::ExitCode::from(2);
        }
    };
    let mut tools = video_lab::runner::Tools::new(ffmpeg, ffprobe);
    tools.quality = quality;
    if let Some(dir) = flag("--fonts") {
        match video_lab::text::Fonts::with_film_fonts(&dir) {
            Ok(fonts) => tools.fonts = fonts,
            Err(why) => {
                eprintln!("{why}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    if let (Some(cli), Some(model)) = (flag("--whisper"), flag("--model")) {
        let words = match flag("--words") {
            Some(path) => match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| video_lab::runner::Words::from_json(&t))
            {
                Ok(words) => words,
                Err(why) => {
                    eprintln!("{why}");
                    return std::process::ExitCode::from(2);
                }
            },
            None => video_lab::runner::Words::default(),
        };
        tools.whisper = Some(video_lab::runner::Whisper { cli, model, words });
    }
    if let (Some(python), Some(script)) = (flag("--python"), flag("--music")) {
        tools.score = Some(video_lab::runner::Score { python, script });
    }
    let mut input = Vec::new();
    let dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    let result = std::io::stdin()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut input)
        .map_err(|e| e.to_string())
        .and_then(|_| video_lab::runner::handle(name, &input, &tools, &dir));
    match result {
        Ok(output) => {
            println!("{output}");
            std::process::ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(name) = args.first().and_then(|a| a.strip_suffix("-job")) {
        return job(name, &args[1..]);
    }
    let mut data = String::new();
    let answer = match std::io::stdin()
        .take(64 * 1024 * 1024)
        .read_to_string(&mut data)
    {
        Err(e) => json!({"error": e.to_string()}),
        Ok(_) => match serde_json::from_str::<Value>(&data) {
            Err(e) => json!({"error": format!("the request isn't JSON ({e})")}),
            Ok(input) => run(&input).unwrap_or_else(|r| {
                if r.code == "request" {
                    json!({"error": r.message})
                } else {
                    json!({"refused": r.message, "code": r.code})
                }
            }),
        },
    };
    println!("{answer}");
    std::process::ExitCode::SUCCESS
}
