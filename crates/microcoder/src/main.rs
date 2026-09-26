//! `microcoder <terminal-bench-task>`: run the loop on a Terminal-Bench 4
//! task and stream every step in the terminal.

use std::process::ExitCode;

use microcoder::models::{JevJudge, OpenRouterGenerator, question_set, route_set};
use microcoder::run::{Ending, Limits, Models, Route, USER_PROMPT, run};
use microcoder::show::{Both, Record, Terminal, clock};
use microcoder::state::State;
use microcoder::{MODEL, STRONG_MODEL, tbench};

const USAGE: &str = "usage: microcoder <terminal-bench-task> [options]
       microcoder kb search|show|lint ... (microcoder kb --help for more)

Runs Microcoder on a Terminal-Bench 4 task and streams every step: Jev's
judgments, the model's rationale and commands, each command's output, and
the cost. Then it runs the task's own tests and prints the reward beside
Fable 5.1 low's time and cost on the same task.

Options:
  --model SLUG       the OpenRouter model (default openai/gpt-6-luna)
  --effort LEVEL     low, medium, or high (default medium)
  --strong-model SLUG  the OpenRouter model that writes the acceptance tests on
                     a task Jev judges hard (default openai/gpt-6-sol)
  --route WHEN       when the stronger model writes the tests: auto (when Jev
                     judges the task hard), always, or never (default auto)
  --max-steps N      default no limit
  --max-minutes N    default 60
  --max-usd N        model and Jev spend, default 1.00
  --command-seconds N  default 300
  --test-seconds N   how long one acceptance test may run, default 60
  --network NAME     the container's Docker network: default bridge (network on),
                     or none for a task whose task.toml sets allow_internet = false
  --prompt TEXT      the instruction to the model (default \"Solve this task.\")
  --no-acceptance    skip the acceptance tests the model otherwise writes and
                     freezes first, and that must pass before it can finish
  --kb MODE          the shared knowledge base: on (admitted entries), off, or
                     candidates (unreviewed entries too) (default on)
  --keep             leave the container running afterward
  --check-grading    run the task's reference solution instead of the loop,
                     then grade it: a check that grading works, at no model cost

Keys: OPENROUTER_API_KEY or ~/.openagents/openrouter.json, and
TYPESAFE_API_KEY or ~/.openagents/jev.json. Tasks come from
MICROCODER_TASKS or ~/.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks.

Exit codes: 0 the tests passed, 1 they didn't, 2 the run couldn't start.";

struct Options {
    task: String,
    model: String,
    strong_model: String,
    effort: Option<String>,
    limits: Limits,
    /// `None` until the task's own setting decides it.
    network: Option<String>,
    prompt: String,
    keep: bool,
    check_grading: bool,
    /// `on`, `off`, or `candidates`.
    kb: String,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        task: String::new(),
        model: MODEL.to_string(),
        strong_model: STRONG_MODEL.to_string(),
        effort: Some("medium".to_string()),
        limits: Limits::default(),
        network: None,
        prompt: USER_PROMPT.to_string(),
        keep: false,
        check_grading: false,
        kb: "on".to_string(),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or(format!("{arg} needs a value"));
        let number = |text: String| {
            text.parse::<f64>()
                .map_err(|_| format!("{arg} wants a number, not {text}"))
        };
        match arg.as_str() {
            "--model" => options.model = value()?,
            "--strong-model" => options.strong_model = value()?,
            "--route" => {
                options.limits.route = match value()?.as_str() {
                    "auto" => Route::Auto,
                    "always" => Route::Always,
                    "never" => Route::Never,
                    other => {
                        return Err(format!("--route wants auto, always, or never, not {other}"));
                    }
                }
            }
            "--effort" => {
                let effort = value()?;
                options.effort = (effort != "default").then_some(effort);
            }
            "--max-steps" => options.limits.max_steps = Some(number(value()?)? as usize),
            "--max-minutes" => options.limits.max_seconds = (number(value()?)? * 60.0) as u64,
            "--max-usd" => options.limits.max_usd = number(value()?)?,
            "--command-seconds" => options.limits.command_seconds = number(value()?)? as u64,
            "--test-seconds" => options.limits.test_seconds = number(value()?)? as u64,
            "--network" => options.network = Some(value()?),
            "--prompt" => options.prompt = value()?,
            "--kb" => {
                options.kb = value()?;
                if !["on", "off", "candidates"].contains(&options.kb.as_str()) {
                    return Err(format!(
                        "--kb wants on, off, or candidates, not {}",
                        options.kb
                    ));
                }
            }
            "--keep" => options.keep = true,
            "--no-acceptance" => options.limits.acceptance = false,
            "--check-grading" => options.check_grading = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag}\n\n{USAGE}"));
            }
            task if options.task.is_empty() => options.task = task.to_string(),
            extra => return Err(format!("one task at a time; {extra} is extra")),
        }
    }
    if options.task.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(options)
}

fn jev_client() -> Result<jev::Client, String> {
    let from_env = std::env::var("TYPESAFE_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty());
    let key = from_env.or_else(|| {
        let path = std::path::PathBuf::from(std::env::var_os("HOME")?).join(".openagents/jev.json");
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        value["api_key"].as_str().map(str::to_string)
    });
    let key =
        key.ok_or("no Jev key: set TYPESAFE_API_KEY or put api_key in ~/.openagents/jev.json")?;
    jev::Client::new(jev::Config::new().api_key(key.trim())).map_err(|e| format!("Jev: {e}"))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "kb") {
        return ExitCode::from(knowledge::cli::main(&args[1..]).await);
    }
    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match go(options).await {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("microcoder: {message}");
            ExitCode::from(2)
        }
    }
}

async fn go(options: Options) -> Result<u8, String> {
    let task = tbench::find(&tbench::tasks_dir(), &options.task)?;
    let network = options
        .network
        .clone()
        .unwrap_or_else(|| if task.internet { "bridge" } else { "none" }.to_string());
    let openrouter = openrouter::Config::from_env().map_err(|e| e.to_string())?;
    let generator = OpenRouterGenerator {
        client: openrouter::Client::new(openrouter).map_err(|e| e.to_string())?,
        model: options.model.clone(),
        effort: options.effort.clone(),
    };
    let strong = OpenRouterGenerator {
        client: openrouter::Client::new(openrouter::Config::from_env().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
        model: options.strong_model.clone(),
        effort: options.effort.clone(),
    };
    let set = question_set();
    let route = route_set();
    let judge = JevJudge {
        client: jev_client()?,
    };
    let retriever = if options.kb == "off" {
        None
    } else {
        let dir = knowledge::default_dir();
        let base = knowledge::Base::load(&dir, options.kb == "candidates")?;
        Some(match knowledge::search::OpenRouterEmbedder::from_env() {
            Ok(embedder) => {
                knowledge::search::Retriever::new(base, embedder, knowledge::default_cache())
            }
            Err(error) => knowledge::search::Retriever::lexical(base, &error),
        })
    };
    let mut terminal = Terminal::new();
    let say = |text: &str| terminal_line(text);
    println!(
        "microcoder · {} · {} (effort {}), tests by {} when needed · {}, {} min, ${:.2} · network {network}",
        task.name,
        options.model,
        options.effort.as_deref().unwrap_or("default"),
        options.strong_model,
        options
            .limits
            .max_steps
            .map_or("no step limit".to_string(), |n| format!("up to {n} steps")),
        options.limits.max_seconds / 60,
        options.limits.max_usd
    );
    if let Some(retriever) = &retriever {
        println!(
            "knowledge base: {} entries ({}) from {}",
            retriever.base.entries.len(),
            if options.kb == "candidates" {
                "admitted and candidate"
            } else {
                "admitted"
            },
            knowledge::default_dir().display()
        );
    } else {
        println!("knowledge base: off");
    }
    if let Some((passed, runs, seconds, cost)) = tbench::fable(&task.name) {
        println!(
            "Fable 5.1 low on this task: {passed} of {runs} passed, median {} and ${cost:.2} a pass",
            clock(seconds)
        );
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let run_dir = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".openagents/microcoder/runs")
        .join(format!("{}-{stamp}", task.name));
    std::fs::create_dir_all(&run_dir)
        .map_err(|e| format!("can't make {}: {e}", run_dir.display()))?;
    let mut record = Record::create(&run_dir.join("events.jsonl"))
        .map_err(|e| format!("can't write the record: {e}"))?;
    record.write(&serde_json::json!({
        "event": "started", "task": task.name, "model": options.model, "effort": options.effort,
        "limits": options.limits, "network": network, "prompt": options.prompt,
        "questions": set.id, "questions_file": microcoder::models::QUESTIONS,
        "route": route.id, "route_file": microcoder::models::ROUTE,
        "dispute_file": microcoder::models::DISPUTE,
        "strong_model": options.strong_model, "route_when": options.limits.route,
        "kb": options.kb, "knowledge_file": microcoder::models::KNOWLEDGE,
        "knowledge_entries": retriever.as_ref().map(|r| r.base.entries.iter()
            .map(|e| serde_json::json!({"id": e.id, "version": e.version, "digest": e.digest}))
            .collect::<Vec<_>>()),
    }));

    let image = tbench::image(&task, &say).await?;
    let name = format!("microcoder-{}-{}", task.name, std::process::id());
    let env = tbench::start(&image, &name, &network, &task.workdir).await?;
    say(&format!(
        "container {name} is up; commands run in {}",
        task.workdir
    ));

    if options.check_grading {
        say("running the task's reference solution");
        let solved = tbench::solve(&task, &env).await;
        if let Err(error) = &solved {
            say(error);
        }
        let verdict = tbench::verify(&task, &env, &network, &say).await;
        println!(
            "grading check: reference solution {} · reward {}",
            if solved.is_ok() { "ran" } else { "failed" },
            verdict
                .reward
                .map_or("unknown".to_string(), |r| format!("{r}"))
        );
        if verdict.reward.is_none_or(|r| r < 1.0) {
            println!("{}", indent_block(&verdict.output));
        }
        if !options.keep {
            tbench::remove(&name).await;
        }
        return Ok(if verdict.reward.is_some_and(|r| r >= 1.0) {
            0
        } else {
            1
        });
    }

    let outcome = tokio::select! {
        result = async {
            let environment = tbench::describe(&env, &network).await;
            println!("{}", indent_block(&environment));
            let state = State {
                environment,
                task: task.instruction.clone(),
                ..State::default()
            };
            let mut both = Both(&mut terminal, &mut record);
            run(state, &options.prompt, &env, &Models { generator: &generator, judge: &judge, set: &set, route: &route, strong: Some(&strong), knowledge: retriever.as_ref() }, &options.limits, &mut both).await
        } => result,
        _ = tokio::signal::ctrl_c() => {
            println!("\nInterrupted; removing the container.");
            tbench::remove(&name).await;
            return Ok(1);
        }
    };
    let (end_state, outcome) = outcome;

    let saved = tbench::save_artifacts(&task, &env, &run_dir.join("artifacts")).await;
    say(&format!(
        "saved {} of {} output paths to {}",
        saved.len(),
        task.artifacts.len(),
        run_dir.join("artifacts").display()
    ));
    let verdict = tbench::verify(&task, &env, &network, &say).await;
    let tail: String = verdict
        .output
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    println!("{}", indent_block(&tail));
    let passed = verdict.reward.is_some_and(|r| r >= 1.0);
    let total = outcome.model_usd + outcome.jev_usd + outcome.embedding_usd;
    println!(
        "\n{} · reward {} · {} steps · {} · ${total:.4} (model ${:.4}, Jev ${:.5}, embeddings ${:.6}) · ended by {}",
        task.name,
        verdict
            .reward
            .map_or("unknown".to_string(), |r| format!("{r}")),
        outcome.steps,
        clock(outcome.seconds),
        outcome.model_usd,
        outcome.jev_usd,
        outcome.embedding_usd,
        match &outcome.ending {
            Ending::Finished => "the model finishing".to_string(),
            other => format!("{other:?}"),
        }
    );
    if let Some((fable_passed, runs, seconds, cost)) = tbench::fable(&task.name) {
        println!(
            "Fable 5.1 low: {fable_passed} of {runs} passed, median {} and ${cost:.2} a pass",
            clock(seconds)
        );
    }
    let summary = serde_json::json!({
        "task": task.name, "model": options.model, "effort": options.effort,
        "outcome": outcome, "reward": verdict.reward, "verifier_output": verdict.output,
        "container": name, "image": image,
        "acceptance_tests": end_state.tests, "frozen_at": end_state.frozen_at,
        "test_results": end_state.test_results, "dropped_tests": end_state.dropped,
        "kb": options.kb,
    });
    let _ = std::fs::write(
        run_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
    );
    record.write(&serde_json::json!({"event": "verified", "reward": verdict.reward}));
    println!("Record: {}", run_dir.display());
    if options.keep {
        println!("The container {name} is still running (--keep).");
    } else {
        tbench::remove(&name).await;
    }
    Ok(if passed { 0 } else { 1 })
}

fn terminal_line(text: &str) {
    println!("        {text}");
}

fn indent_block(text: &str) -> String {
    text.lines()
        .map(|l| format!("        {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
