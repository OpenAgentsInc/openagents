//! `microcoder <terminal-bench-task>`: run the loop on a Terminal-Bench 4
//! task and stream every step in the terminal.

use std::process::ExitCode;

use microcoder::models::Basis;
use microcoder::models::{
    AnyGenerator, CodexGenerator, JevJudge, OpenRouterGenerator, question_set, route_set,
};
use microcoder::run::{Ending, Limits, Models, Route, USER_PROMPT, run};
use microcoder::show::{Both, Record, Terminal, clock, dollars};
use microcoder::state::State;
use microcoder::{MODEL, STRONG_MODEL, tbench};

const USAGE: &str = "usage: microcoder <terminal-bench-task> [options]
       microcoder kb <command> ... (microcoder kb --help lists the commands)
       microcoder xp <command> ... (microcoder xp --help lists the commands)
       microcoder repository --grant FILE [--store DIRECTORY] [--detach]

Runs Microcoder on a Terminal-Bench 4 task and streams every step: Jev's
judgments, the model's rationale and commands, each command's output, and
the cost. Then it runs the task's own tests and prints the reward beside
Fable 5.1 low's time and cost on the same task.

Options:
  --model SLUG       the model (default gpt-6-luna)
  --provider NAME    codex (the operator's Codex login, the default),
                     openrouter (OPENROUTER_API_KEY), or door (the OpenAgents
                     door at openagents.com/v1/responses on OPENAGENTS_API_KEY
                     or ~/.openagents/bearer; name a gateway model such as
                     --model google/gemini-3.8-flash), or vertex (Vertex AI's
                     OpenAI-compatible endpoint on the token in
                     ~/.openagents/vertex-token; name a model such as
                     --model qwen/qwen3-coder-480b-a35b-instruct-maas)
  --effort LEVEL     low, medium, or high (default medium)
  --strong-model SLUG  the model that writes the acceptance tests on
                     a task Jev judges hard (default gpt-6-sol)
  --route WHEN       when the stronger model writes the tests: auto (when Jev
                     judges the task hard), always, or never (default never)
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
  --kb-trust MODE    which synced entries from other authors the base includes:
                     own (your key's only), listed (also the authors in
                     ~/.openagents/knowledge/trust.json), or all (everyone else's
                     as candidates) (default the trust file's mode, else own)
  --kb-lexical       rank knowledge entries by words alone, without embeddings;
                     summary.json records the retrieval mode either way
  --gate-requirements  before a run with every frozen test passing ends, Jev
                     checks each statement of the task for a test that
                     checks it, and sends the run back to test the ones
                     without one (at most twice)
  --gate-target      the same, once, for a numeric target no test measures
  --gate-credible    ask the model to say whether its solution is credible when
                     it finishes, and send the run back (at most twice) when
                     Jev judges its recent reasoning doubts the solution
  --doubt-threshold P  Jev's probability of doubt that sends the run back
                     (default 0.9)
  --adversarial N    before a green run ends, send it back up to N times to
                     write tests that try to break the solution, while under
                     --budget-fraction of the time and spend limits (default 0)
  --budget-fraction F  the share of the limits adversarial rounds may use
                     (default 0.5)
  --oracle           before the loop, a separate session writes checks from the
                     task's statement and files alone; those that fail on the
                     untouched workspace are frozen with the model's tests
  --oracle-steps N   steps the oracle session may take (default 8)
  --kb-snapshot FILE  use only a verified immutable EXT bundle (--kb candidates)
  --kb-private FILE   use only a private 3188 bundle (--kb candidates)
  --kb-private-grant FILE  exact model-disclosure permission; embeddings disabled
  --kb-key-file FILE  existing private recipient key (default knowledge-key)
  --kb-cache FILE     separate embedding cache for this run
  --run-dir PATH     create a new exact output directory; existing paths refuse
  --container-name NAME  assign an explicit container identity for retained studies
  --keep             leave the container running afterward
  --check-grading    run the task's reference solution instead of the loop,
                     then grade it: a check that grading works, at no model cost

The model is reached through the Codex login in ~/.codex/auth.json (run
`codex login`), or with --provider openrouter through OPENROUTER_API_KEY or
~/.openagents/openrouter.json. Jev needs TYPESAFE_API_KEY or
~/.openagents/jev.json. Tasks come from
MICROCODER_TASKS or ~/.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks.

Exit codes: 0 the tests passed, 1 they didn't, 2 the run couldn't start.";

struct Options {
    task: String,
    run_dir: Option<std::path::PathBuf>,
    container_name: Option<String>,
    model: String,
    /// `codex`, `openrouter`, `door`, or `vertex`.
    provider: String,
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
    /// Which synced entries the base includes.
    kb_trust: knowledge::remote::TrustConfig,
    kb_snapshot: Option<std::path::PathBuf>,
    kb_private: Option<std::path::PathBuf>,
    kb_private_grant: Option<std::path::PathBuf>,
    kb_key_file: Option<std::path::PathBuf>,
    kb_cache: Option<std::path::PathBuf>,
    kb_lexical: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        task: String::new(),
        run_dir: None,
        container_name: None,
        model: MODEL.to_string(),
        provider: "codex".to_string(),
        strong_model: STRONG_MODEL.to_string(),
        effort: Some("medium".to_string()),
        limits: Limits::default(),
        network: None,
        prompt: USER_PROMPT.to_string(),
        keep: false,
        check_grading: false,
        kb: "on".to_string(),
        kb_snapshot: None,
        kb_private: None,
        kb_private_grant: None,
        kb_key_file: None,
        kb_cache: None,
        kb_lexical: false,
        kb_trust: match knowledge::remote::trust_file() {
            Some(path) => knowledge::remote::TrustConfig::read(&path)?,
            None => knowledge::remote::TrustConfig::default(),
        },
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or(format!("{arg} needs a value"));
        let number = |text: String| {
            text.parse::<f64>()
                .map_err(|_| format!("{arg} wants a number, not {text}"))
        };
        match arg.as_str() {
            "--run-dir" => options.run_dir = Some(value()?.into()),
            "--container-name" => {
                let name = value()?;
                if name.is_empty()
                    || name.len() > 200
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err("--container-name requires a bounded alphanumeric name".into());
                }
                options.container_name = Some(name);
            }
            "--model" => options.model = value()?,
            "--provider" => {
                options.provider = value()?;
                if !["codex", "openrouter", "door", "vertex"].contains(&options.provider.as_str()) {
                    return Err(format!(
                        "--provider wants codex, openrouter, door, or vertex, not {}",
                        options.provider
                    ));
                }
            }
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
            "--kb-snapshot" => options.kb_snapshot = Some(value()?.into()),
            "--kb-private" => options.kb_private = Some(value()?.into()),
            "--kb-private-grant" => options.kb_private_grant = Some(value()?.into()),
            "--kb-key-file" => options.kb_key_file = Some(value()?.into()),
            "--kb-cache" => options.kb_cache = Some(value()?.into()),
            "--kb-lexical" => options.kb_lexical = true,
            "--kb-trust" => {
                let mode = value()?;
                options.kb_trust.mode = knowledge::remote::Trust::parse(&mode)
                    .ok_or(format!("--kb-trust wants own, listed, or all, not {mode}"))?;
            }
            "--keep" => options.keep = true,
            "--no-acceptance" => options.limits.acceptance = false,
            "--gate-requirements" => options.limits.gates.requirements = true,
            "--gate-target" => options.limits.gates.target = true,
            "--gate-credible" => options.limits.gates.credible = true,
            "--doubt-threshold" => options.limits.gates.doubt = number(value()?)?,
            "--adversarial" => options.limits.gates.adversarial = number(value()?)? as usize,
            "--budget-fraction" => options.limits.gates.budget_fraction = number(value()?)?,
            "--oracle" => options.limits.gates.oracle = true,
            "--oracle-steps" => options.limits.gates.oracle_steps = number(value()?)? as usize,
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
    if args
        .first()
        .is_some_and(|argument| argument == "repository")
    {
        return ExitCode::from(repository_cli(&args[1..]).await);
    }
    if args.first().is_some_and(|a| a == "kb") {
        if args.get(1).is_some_and(|a| a == "study") {
            return ExitCode::from(microcoder::kbstudy::main(&args[2..]).await);
        }
        let network = [
            "publish",
            "sync",
            "publish-evidence",
            "snapshot-create",
            "snapshot-check",
            "private-seal",
            "private-show",
            "private-grant",
        ];
        if args.get(1).is_some_and(|c| network.contains(&c.as_str())) {
            return ExitCode::from(microcoder::kbnet::main(&args[1..]).await);
        }
        return ExitCode::from(knowledge::cli::main(&args[1..]).await);
    }
    if args.first().is_some_and(|a| a == "xp") {
        return ExitCode::from(microcoder::xpnet::main(&args[1..]).await);
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
        .unwrap_or_else(|| task.agent_network.docker().to_string());
    let make = |model: &str| -> Result<(AnyGenerator, String), String> {
        if options.provider == "openrouter" {
            let slug = if model.contains('/') {
                model.to_string()
            } else {
                format!("openai/{model}")
            };
            let config = openrouter::Config::from_env().map_err(|e| e.to_string())?;
            let recipient = format!("openrouter:{}#{slug}", config.base_url);
            return Ok((
                AnyGenerator::OpenRouter(OpenRouterGenerator {
                    client: openrouter::Client::new(config).map_err(|e| e.to_string())?,
                    model: slug,
                    effort: options.effort.clone(),
                }),
                recipient,
            ));
        }
        if options.provider == "vertex" {
            let generator =
                microcoder::vertex::VertexGenerator::from_env(model, options.effort.clone())?;
            let recipient = format!("vertex:{}#{}", generator.base_url, generator.model);
            return Ok((AnyGenerator::Vertex(generator), recipient));
        }
        if options.provider == "door" {
            let session = format!("microcoder-{}-{}", options.task, std::process::id());
            if options.kb_private.is_some() {
                return Err("private knowledge delivery through the door provider requires an exact endpoint binding and is not supported by this profile".into());
            }
            return Ok((
                AnyGenerator::Door(microcoder::door::DoorGenerator::new(
                    microcoder::door::DoorTransport::from_env()?,
                    model,
                    options.effort.clone(),
                    &session,
                )),
                format!("door:unused-for-public-knowledge#{model}"),
            ));
        }
        let login = microluna::codex::Login::default_path()
            .ok_or("no Codex login: can't find ~/.codex/auth.json; run `codex login`")?;
        let session = format!("microcoder-{}-{}", options.task, std::process::id());
        let transport = microluna::codex::CodexTransport::new(login, &session)
            .map_err(|e| format!("the Codex login can't be used: {e}; run `codex login`"))?;
        let actual_model = model.rsplit('/').next().unwrap_or(model).to_string();
        let recipient = format!("codex:{}#{actual_model}", microluna::codex::BASE_URL);
        Ok((
            AnyGenerator::Codex(CodexGenerator {
                transport,
                model: actual_model,
                effort: options.effort.clone(),
                cache_key: session,
            }),
            recipient,
        ))
    };
    let (generator, generation_recipient) = make(&options.model)?;
    let (strong, strong_recipient) = make(&options.strong_model)?;
    let set = question_set();
    let route = route_set();
    let judge = JevJudge {
        client: jev_client()?,
    };
    let mut recipients = std::collections::BTreeSet::from([
        generation_recipient,
        format!(
            "typesafe:{}#{}",
            judge.client.base_url(),
            judge.client.default_model()
        ),
    ]);
    if !matches!(options.limits.route, Route::Never) {
        recipients.insert(strong_recipient);
    }
    let mut pinned = microcoder::kbinput::load(
        options.kb_snapshot.as_deref(),
        options.kb_private.as_deref(),
        options.kb_private_grant.as_deref(),
        options.kb_key_file.as_deref(),
        &recipients,
        &options.kb,
    )?;
    let private_input = pinned.as_ref().is_some_and(|input| input.private);
    let knowledge_source = pinned.as_ref().map(|input| input.provenance.clone());
    let retained_knowledge = pinned.as_ref().map(|input| input.retained.clone());
    let mut loaded = knowledge::remote::Loaded::default();
    let retriever = if options.kb == "off" {
        None
    } else {
        let base = if let Some(input) = pinned.take() {
            input.base
        } else {
            let dir = knowledge::default_dir();
            let own = knowledge::remote::key_file().and_then(|p| knowledge::remote::own_pubkey(&p));
            let (base, found) = knowledge::remote::load(
                &dir,
                knowledge::remote::default_dir().as_deref(),
                &options.kb_trust,
                own.as_deref(),
                options.kb == "candidates",
            )?;
            loaded = found;
            base
        };
        Some(if private_input || options.kb_lexical {
            knowledge::search::Retriever::lexical(
                base,
                if private_input {
                    "private input: embedding disclosure is disabled"
                } else {
                    "lexical retrieval was explicitly requested"
                },
            )
        } else {
            match knowledge::search::Embedder::from_env() {
                Ok(embedder) => knowledge::search::Retriever::new(
                    base,
                    embedder,
                    options.kb_cache.clone().or_else(knowledge::default_cache),
                ),
                Err(error) => knowledge::search::Retriever::lexical(base, &error),
            }
        })
    };
    let retrieval_mode = match &retriever {
        None => "off",
        Some(retriever) if retriever.embedder().is_none() => "lexical",
        Some(_) => "hybrid-with-lexical-fallback",
    };
    let decision = serde_json::json!({
        "base_url": judge.client.base_url(),
        "model": judge.client.default_model(),
    });
    let mut terminal = Terminal::new();
    let say = |text: &str| terminal_line(text);
    println!(
        "microcoder · {} · {} (effort {}){} · {}, {} min, ${:.2} · network {network}",
        task.name,
        options.model,
        options.effort.as_deref().unwrap_or("default"),
        match options.limits.route {
            Route::Never => String::new(),
            Route::Auto => format!(", tests by {} on a hard task", options.strong_model),
            Route::Always => format!(", tests by {}", options.strong_model),
        },
        options
            .limits
            .max_steps
            .map_or("no step limit".to_string(), |n| format!("up to {n} steps")),
        options.limits.max_seconds / 60,
        options.limits.max_usd
    );
    if let Some(retriever) = &retriever {
        println!(
            "knowledge base: {} entries ({}) from {}{}",
            retriever.base.entries.len(),
            if options.kb == "candidates" {
                "admitted and candidate"
            } else {
                "admitted"
            },
            options
                .kb_snapshot
                .as_ref()
                .or(options.kb_private.as_ref())
                .cloned()
                .unwrap_or_else(knowledge::default_dir)
                .display(),
            if loaded.remote.is_empty() {
                String::new()
            } else {
                format!(
                    ", {} of them synced from {} other authors (trust {})",
                    loaded.remote.values().sum::<usize>(),
                    loaded.remote.len(),
                    options.kb_trust.mode
                )
            }
        );
        for problem in &loaded.problems {
            println!("knowledge base: skipped a synced entry that didn't verify: {problem}");
        }
        match (retriever.embedder(), retriever.lexical_reason()) {
            (Some(embedder), _) => println!(
                "knowledge base: ranked by words and embeddings from {} ({})",
                embedder.provider, embedder.model
            ),
            (None, why) => println!(
                "knowledge base: ranked by words alone: {}",
                why.unwrap_or("no embedder")
            ),
        }
    } else {
        println!("knowledge base: off");
    }
    if let Some((passed, runs, seconds, cost)) = tbench::fable(&task.name) {
        println!(
            "Fable 5.1 low on this task: {passed} of {runs} passed, median {} and ${cost:.2} a pass",
            clock(seconds)
        );
    }
    // Milliseconds, so runs started in the same second get their own
    // directories. Two runs can still start in the same millisecond (a
    // queue starting two at once did), so the directory is claimed with
    // `create_dir`, which fails when it exists, and a taken millisecond
    // moves to the next one.
    let mut stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let run_dir = if let Some(dir) = &options.run_dir {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("can't make run parent: {e}"))?;
        }
        std::fs::create_dir(dir)
            .map_err(|e| format!("can't create new run {}: {e}", dir.display()))?;
        dir.clone()
    } else {
        let runs = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".openagents/microcoder/runs");
        std::fs::create_dir_all(&runs)
            .map_err(|e| format!("can't make {}: {e}", runs.display()))?;
        loop {
            let dir = runs.join(format!("{}-{stamp}", task.name));
            match std::fs::create_dir(&dir) {
                Ok(()) => break dir,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => stamp += 1,
                Err(e) => return Err(format!("can't make {}: {e}", dir.display())),
            }
        }
    };
    if private_input {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        {
            return Err("private run retention requires a supported private filesystem".into());
        }
    }
    if let Some(retained) = &retained_knowledge {
        knowledge::snapshot::write_new(&run_dir.join("knowledge-input.json"), retained)?;
    }
    let mut record = Record::create(&run_dir.join("events.jsonl"))
        .map_err(|e| format!("can't write the record: {e}"))?;
    record.write(&serde_json::json!({
        "event": "started", "task": task.name, "model": options.model, "provider": options.provider, "effort": options.effort,
        "limits": options.limits, "network": network, "prompt": options.prompt,
        "questions": set.id, "questions_file": microcoder::models::QUESTIONS,
        "route": route.id, "route_file": microcoder::models::ROUTE,
        "dispute_file": microcoder::models::DISPUTE,
        "conform_file": microcoder::models::CONFORM,
        "coverage_file": microcoder::models::COVERAGE,
        "requirements_file": microcoder::models::REQUIREMENTS,
        "credible_file": microcoder::models::CREDIBLE,
        "target_file": microcoder::models::TARGET,
        "strong_model": options.strong_model, "route_when": options.limits.route,
        "kb": options.kb, "kb_trust": options.kb_trust.mode.to_string(),
        "knowledge_source": knowledge_source,
        "retrieval_mode": retrieval_mode, "decision": decision,
        "knowledge_file": microcoder::models::KNOWLEDGE,
        "knowledge_entries": retriever.as_ref().map(|r| r.base.entries.iter()
            .map(|e| serde_json::json!({"id": e.id, "version": e.version, "digest": e.digest, "author": e.author}))
            .collect::<Vec<_>>()),
    }));

    let image = tbench::image(&task, &say).await;
    let name = options
        .container_name
        .clone()
        .unwrap_or_else(|| format!("microcoder-{}-{}", task.name, std::process::id()));
    let env = tbench::start(&task, &image, &name, &network, &say).await?;
    say(&format!(
        "container {} is up; commands run in {}",
        env.container, env.workdir
    ));

    if options.check_grading {
        say("running the task's reference solution");
        let solved = tbench::solve(&task, &env, &name).await;
        if let Err(error) = &solved {
            say(error);
        }
        if task.separate {
            tbench::collect(&task, &name, &say).await;
        }
        let verdict = tbench::verify(&task, &env, &name, &say).await;
        println!(
            "grading check: reference solution {} · reward {}",
            if solved.is_ok() { "ran" } else { "failed" },
            reward_text(&verdict)
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

    if task.separate {
        tbench::collect(&task, &name, &say).await;
    }
    let saved = tbench::save_artifacts(&task, &name, &run_dir.join("artifacts")).await;
    say(&format!(
        "saved {} of {} output paths to {}",
        saved.len(),
        task.artifacts.len(),
        run_dir.join("artifacts").display()
    ));
    let verdict = tbench::verify(&task, &env, &name, &say).await;
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
    let total = match outcome.usd {
        Some(usd) => format!("${usd:.4}"),
        None => format!(
            "cost unknown, at least ${:.4} ({} calls unpriced)",
            outcome.known_usd,
            outcome.cost_unknown.len()
        ),
    };
    let basis = if options.provider == "codex" || options.provider == "vertex" {
        Basis::ListPrice
    } else {
        Basis::Billed
    };
    println!(
        "\n{} · reward {} · {} steps · {} · {total} (model {} {basis}, Jev {}, embeddings {}) · ended by {}{}",
        task.name,
        reward_text(&verdict),
        outcome.steps,
        clock(outcome.seconds),
        dollars(outcome.model_usd, 4),
        dollars(outcome.jev_usd, 5),
        dollars(outcome.embedding_usd, 6),
        match &outcome.ending {
            Ending::Finished => "the model finishing".to_string(),
            other => format!("{other:?}"),
        },
        if outcome.knowledge_assisted {
            format!(
                " · knowledge-assisted ({} entries)",
                outcome.knowledge.len()
            )
        } else {
            String::new()
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
        "rewards": verdict.rewards, "reward_unknown_because": verdict.reason,
        "container": name, "image": image, "compose": task.compose,
        "verifier": if task.separate { "separate" } else { "shared" },
        "acceptance_tests": end_state.tests, "frozen_at": end_state.frozen_at,
        "gates": options.limits.gates,
        "oracle_checks": end_state.oracle.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        "test_results": end_state.test_results, "dropped_tests": end_state.dropped,
        "kb": options.kb, "kb_trust": options.kb_trust.mode.to_string(),
        "knowledge_source": knowledge_source,
        "retrieval_mode": retrieval_mode, "decision": decision,
        "knowledge_assisted": outcome.knowledge_assisted,
        // How the model was reached and how its cost was reached: the Codex
        // login reports tokens, priced at list price; OpenRouter bills.
        // Jev is always its published rate times reported tokens.
        "provider": options.provider,
        "cost_basis": basis,
        "cost_bases": {
            "model": basis,
            "jev": Basis::ListPrice,
            "embeddings": retriever.as_ref().and_then(|r| r.embedder()).map(|e| e.basis()),
        },
        "retrieval": microcoder::run::retrieval_summary(
            retriever.is_some(),
            retriever.as_ref().and_then(|r| r.embedder()).map(|e| {
                (
                    if e.provider == knowledge::search::EmbeddingProvider::Openai { "openai" } else { "openrouter" },
                    e.model.as_str(),
                    e.basis(),
                )
            }),
            retriever.as_ref().and_then(|r| r.lexical_reason()),
            &outcome,
        ),
    });
    let _ = std::fs::write(
        run_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
    );
    record.write(&serde_json::json!({"event": "verified", "reward": verdict.reward, "reward_unknown_because": verdict.reason}));
    println!("Record: {}", run_dir.display());
    if options.keep {
        println!("The environment {name} is still running (--keep).");
    } else {
        tbench::remove(&name).await;
    }
    Ok(if passed { 0 } else { 1 })
}

/// The reward, or "unknown" with the reason.
fn reward_text(verdict: &tbench::Verdict) -> String {
    match (verdict.reward, &verdict.reason) {
        (Some(reward), _) => format!("{reward}"),
        (None, Some(reason)) => format!("unknown ({reason})"),
        (None, None) => "unknown".to_string(),
    }
}

async fn repository_cli(arguments: &[String]) -> u8 {
    use std::io::Read;
    if arguments == ["--help"] {
        println!(
            "microcoder repository --grant FILE [--store DIRECTORY] [--detach]\n\nRuns an explicitly admitted repository task through the common owner.\nThe profile requires acceptance=false, route=never, and off or explicitly frozen knowledge context, with no hard dollar limit.\n--detach starts a separate model host and returns a pending launch receipt.\nUse coder task view, cancel, recover, and check for the retained task."
        );
        return 0;
    }
    let result = async {
        let mut grant = None;
        let mut store = None;
        let mut detach = false;
        let mut arguments = arguments.iter();
        while let Some(flag) = arguments.next() {
            if flag == "--detach" {
                if detach {
                    return Err("give --detach only once".into());
                }
                detach = true;
                continue;
            }
            let value = arguments.next().ok_or("an option needs a value")?;
            match flag.as_str() {
                "--grant" if grant.is_none() => grant = Some(std::path::PathBuf::from(value)),
                "--store" if store.is_none() => store = Some(std::path::PathBuf::from(value)),
                _ => return Err("unknown or repeated repository option".into()),
            }
        }
        let path = grant.ok_or("repository requires --grant")?;
        let store = store
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| std::path::PathBuf::from(home).join(".openagents/tasks"))
            })
            .ok_or("no task store path")?;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|error| error.to_string())?
            .take(coder::task::MAX_COMMAND_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if detach {
            return microcoder::repository::launch::start(&store, &bytes)
                .map(|launched| serde_json::json!(launched));
        }
        let judge = JevJudge {
            client: jev_client()?,
        };
        microcoder::repository::execute(&store, &bytes, judge)
            .await
            .map(|task| serde_json::json!(task))
    }
    .await;
    match result {
        Ok(task) => {
            println!("{}", serde_json::json!(task));
            0
        }
        Err(error) => {
            eprintln!("{}", serde_json::json!({"error":error}));
            2
        }
    }
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
