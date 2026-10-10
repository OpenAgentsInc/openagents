//! `ui-format-bench`: the interactive-answer format benchmark (#11113,
//! `docs/research/2026-10-09-ui-format-benchmark.md`).
//!
//! Every prompt in `bench/ui-format/prompts-v1.json` goes to every model,
//! `--runs` times, once in each format: the OpenUI Lang subset and
//! minified nested JSON, each described from the same catalog
//! ([`coder::ui_format`]). Each reply is scored by the same validator, and
//! timed as it streams: first words, the first moment it could draw
//! something, and the whole reply.
//!
//! ```sh
//! scripts/ui-format-bench.sh --model google/gemini-3.8-flash --model zai/glm-5.3-flash
//! ```
//!
//! | Flag | Effect |
//! | --- | --- |
//! | `--model ID` | A model to ask; repeat for more. Required. |
//! | `--runs N` | Runs per prompt, model, and format (default 3). |
//! | `--format lang\|json` | Only one format. |
//! | `--prompt ID` | Only this prompt; repeat for more. |
//! | `--prompts PATH` | Another prompts file. |
//! | `--out PATH` | Where the JSON report goes (default `bench/ui-format/results/<unix>.json`). |
//!
//! The door is `CODER_DOOR_URL` (default the public gateway) with the key in
//! `CODER_DOOR_KEY`, `CODER_AI_GATEWAY_KEY`, or `AI_GATEWAY_API_KEY`.
//! Nothing a model writes is printed; the report holds scores, lengths,
//! and times, and each reply only with `--keep-replies`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::generate::{DEFAULT_DOOR_URL, Generate, Message, Meta, ResponsesDoor, Role};
use coder::ui_format::{self, Format, Prompts, Score};
use serde::Serialize;

/// The longest one reply may take.
const CALL_WAIT: Duration = Duration::from_secs(120);

struct Options {
    models: Vec<String>,
    runs: usize,
    formats: Vec<Format>,
    only: Vec<String>,
    prompts: Option<PathBuf>,
    out: Option<PathBuf>,
    keep: bool,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        models: Vec::new(),
        runs: 3,
        formats: Format::ALL.to_vec(),
        only: Vec::new(),
        prompts: None,
        out: None,
        keep: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--model" => options.models.push(value()?),
            "--runs" => {
                options.runs = value()?
                    .parse()
                    .map_err(|_| "--runs takes a number".to_string())?;
            }
            "--format" => {
                options.formats = vec![match value()?.as_str() {
                    "lang" => Format::Lang,
                    "json" => Format::Json,
                    other => return Err(format!("--format is lang or json, not {other}")),
                }];
            }
            "--prompt" => options.only.push(value()?),
            "--prompts" => options.prompts = Some(PathBuf::from(value()?)),
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--keep-replies" => options.keep = true,
            "-h" | "--help" => {
                return Err("usage: ui-format-bench --model ID [--model ID] [--runs N] \
                            [--format lang|json] [--prompt ID] [--prompts PATH] [--out PATH] \
                            [--keep-replies]"
                    .into());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if options.models.is_empty() {
        return Err("name at least one --model".into());
    }
    if options.runs == 0 {
        return Err("--runs must be at least 1".into());
    }
    Ok(options)
}

/// One reply.
#[derive(Serialize)]
struct Run {
    format: &'static str,
    model: String,
    prompt: String,
    run: usize,
    score: Score,
    output_tokens: Option<u64>,
    reply_chars: usize,
    first_word_ms: Option<u64>,
    first_render_ms: Option<u64>,
    total_ms: Option<u64>,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply: Option<String>,
}

/// One format on one model.
#[derive(Serialize)]
struct Summary {
    format: &'static str,
    model: String,
    runs: usize,
    valid: usize,
    blank: usize,
    errors: usize,
    output_tokens_p50: Option<u64>,
    block_chars_p50: Option<u64>,
    first_render_ms_p50: Option<u64>,
    total_ms_p50: Option<u64>,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    started_unix: u64,
    door: String,
    runs_per_case: usize,
    summary: Vec<Summary>,
    runs: Vec<Run>,
}

fn p50(mut values: Vec<u64>) -> Option<u64> {
    values.sort_unstable();
    (!values.is_empty()).then(|| values[(values.len() - 1) / 2])
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn ask(door: &ResponsesDoor, format: Format, prompt: &str) -> (Run, String) {
    let instructions = format.instructions();
    let input = [Message {
        role: Role::User,
        text: prompt.to_owned(),
    }];
    let started = Instant::now();
    let mut streamed = String::new();
    let mut first_word: Option<u64> = None;
    let mut first_render: Option<u64> = None;
    let mut sink = |delta: &str| {
        if first_word.is_none() && !delta.trim().is_empty() {
            first_word = Some(ms(started));
        }
        streamed.push_str(delta);
        if first_render.is_none() && ui_format::renders(format, &streamed) {
            first_render = Some(ms(started));
        }
    };
    let mut meta = |_: Meta| {};
    let call = door.generate(&instructions, &input, &mut sink, &mut meta);
    let outcome = tokio::time::timeout(CALL_WAIT, call).await;
    let total = ms(started);
    let mut run = Run {
        format: format.word(),
        model: door.model.clone(),
        prompt: String::new(),
        run: 0,
        score: Score::default(),
        output_tokens: None,
        reply_chars: 0,
        first_word_ms: None,
        first_render_ms: None,
        total_ms: None,
        error: None,
        reply: None,
    };
    let reply = match outcome {
        Ok(Ok((reply, usage))) => {
            run.output_tokens = usage.map(|u| u.output_tokens);
            run.total_ms = Some(total);
            reply
        }
        Ok(Err(error)) => {
            run.error = Some(error.to_string());
            String::new()
        }
        Err(_) => {
            run.error = Some(format!("no whole reply in {} s", CALL_WAIT.as_secs()));
            String::new()
        }
    };
    run.first_word_ms = first_word;
    run.score = ui_format::score(format, &reply);
    // A reply that never drew while streaming may still draw whole.
    run.first_render_ms = first_render.or_else(|| (!run.score.blank).then_some(total));
    run.reply_chars = reply.chars().count();
    (run, reply)
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let text = match &options.prompts {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return ExitCode::from(2);
            }
        },
        None => ui_format::PROMPTS.to_owned(),
    };
    let prompts = match Prompts::parse(&text) {
        Ok(prompts) => prompts,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let key = [
        "CODER_DOOR_KEY",
        "CODER_AI_GATEWAY_KEY",
        "AI_GATEWAY_API_KEY",
    ]
    .iter()
    .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()));
    let Some(key) = key else {
        eprintln!("set CODER_DOOR_KEY, CODER_AI_GATEWAY_KEY, or AI_GATEWAY_API_KEY");
        return ExitCode::from(2);
    };
    let url = std::env::var("CODER_DOOR_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_DOOR_URL.to_owned());
    let started_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let chosen: Vec<_> = prompts
        .prompts
        .iter()
        .filter(|p| options.only.is_empty() || options.only.contains(&p.id))
        .collect();
    if chosen.is_empty() {
        eprintln!("no prompt matches --prompt");
        return ExitCode::from(2);
    }
    let mut runs = Vec::new();
    for model in &options.models {
        let door = ResponsesDoor::new(&url, model, &key);
        for prompt in &chosen {
            for run in 1..=options.runs {
                // Formats alternate within each run, so drift in the
                // provider's speed falls on both alike.
                for format in &options.formats {
                    let (mut result, reply) = ask(&door, *format, &prompt.prompt).await;
                    result.prompt = prompt.id.clone();
                    result.run = run;
                    if options.keep {
                        result.reply = Some(reply);
                    }
                    eprintln!(
                        "{model} {} {} #{run}: valid {}, blank {}, {} fixes{}",
                        format.word(),
                        prompt.id,
                        result.score.valid,
                        result.score.blank,
                        result.score.diagnostics,
                        result
                            .error
                            .as_deref()
                            .map(|e| format!(", error {e}"))
                            .unwrap_or_default()
                    );
                    runs.push(result);
                }
            }
        }
    }
    let mut groups: BTreeMap<(String, &'static str), Vec<&Run>> = BTreeMap::new();
    for run in &runs {
        groups
            .entry((run.model.clone(), run.format))
            .or_default()
            .push(run);
    }
    let summary: Vec<Summary> = groups
        .into_iter()
        .map(|((model, format), group)| Summary {
            format,
            model,
            runs: group.len(),
            valid: group.iter().filter(|r| r.score.valid).count(),
            blank: group.iter().filter(|r| r.score.blank).count(),
            errors: group.iter().filter(|r| r.error.is_some()).count(),
            output_tokens_p50: p50(group.iter().filter_map(|r| r.output_tokens).collect()),
            block_chars_p50: p50(group
                .iter()
                .filter(|r| !r.score.blank)
                .map(|r| r.score.block_chars as u64)
                .collect()),
            first_render_ms_p50: p50(group.iter().filter_map(|r| r.first_render_ms).collect()),
            total_ms_p50: p50(group.iter().filter_map(|r| r.total_ms).collect()),
        })
        .collect();
    let show = |v: Option<u64>| v.map_or("-".to_owned(), |v| v.to_string());
    println!(
        "| Model | Format | Runs | Valid | Blank | Errors | Output tokens p50 | Block chars p50 | First render ms p50 | Total ms p50 |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for s in &summary {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.model,
            s.format,
            s.runs,
            s.valid,
            s.blank,
            s.errors,
            show(s.output_tokens_p50),
            show(s.block_chars_p50),
            show(s.first_render_ms_p50),
            show(s.total_ms_p50)
        );
    }
    let report = Report {
        schema: "openagents.ui-format-bench.report.v1",
        started_unix,
        door: url,
        runs_per_case: options.runs,
        summary,
        runs,
    };
    let out = options
        .out
        .unwrap_or_else(|| PathBuf::from(format!("bench/ui-format/results/{started_unix}.json")));
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&out, json) {
                eprintln!("{}: {e}", out.display());
                return ExitCode::FAILURE;
            }
            eprintln!("report: {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("report: {e}");
            ExitCode::FAILURE
        }
    }
}
