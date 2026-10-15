use code_index::episodes::{self, Limits};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| -> Option<String> {
        args.iter()
            .position(|a| a == key)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    if args.first().map(String::as_str) == Some("prepare-evaluation") {
        let out =
            PathBuf::from(value("--out").ok_or("Choose a scratch directory with --out DIR.")?);
        let plan = code_index::evaluation_pilot::prepare_pilot(
            &out,
            &value("--strong-model").ok_or("Choose the teacher model with --strong-model NAME.")?,
            &value("--cheap-model").ok_or("Choose the cheaper model with --cheap-model NAME.")?,
        )?;
        println!(
            "{}",
            serde_json::json!({"cases":plan.cases.len(),"arms":plan.arms.len(),"planned_trials":plan.cases.len()*plan.arms.len()*plan.repetitions,"build_cost_usd":plan.build_cost_usd})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("replay-evaluation") {
        let plan: code_index::evaluation::Plan = code_index::read_json(std::path::Path::new(
            &value("--plan").ok_or("Choose a frozen plan with --plan FILE.")?,
        ))?;
        let report: code_index::evaluation::Report = code_index::read_json(std::path::Path::new(
            &value("--report").ok_or("Choose a report with --report FILE.")?,
        ))?;
        let run = PathBuf::from(
            value("--run-dir").ok_or("Choose the original run directory with --run-dir DIR.")?,
        );
        let out = PathBuf::from(
            value("--out").ok_or("Choose a replay scratch directory with --out DIR.")?,
        );
        let target = PathBuf::from(
            value("--target").ok_or("Choose a Cargo target directory with --target DIR.")?,
        );
        let mut receipts = Vec::new();
        for trial in &report.trials {
            let name = code_index::digest(
                format!(
                    "{}:{}:{}:{}",
                    trial.plan_digest, trial.case, trial.arm, trial.repetition
                )
                .as_bytes(),
            );
            let patch =
                std::fs::read(run.join(&name).join("change.patch")).map_err(|e| e.to_string())?;
            receipts.push(code_index::evaluation::replay_trial(
                &plan,
                trial,
                &patch,
                &out.join(name),
                &target,
            )?);
        }
        code_index::private::write(
            &out,
            "receipts.json",
            &serde_json::to_vec(&receipts).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"replayed":receipts.len(),"verified":receipts.iter().filter(|r|r.verified).count()})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("evaluate") {
        let plan: code_index::evaluation::Plan = code_index::read_json(std::path::Path::new(
            &value("--plan").ok_or("Choose a frozen plan with --plan FILE.")?,
        ))?;
        let out =
            PathBuf::from(value("--out").ok_or("Choose a scratch directory with --out DIR.")?);
        let target = PathBuf::from(
            value("--target")
                .or_else(|| std::env::var("CARGO_TARGET_DIR").ok())
                .ok_or("Choose the existing Cargo target directory with --target DIR.")?,
        );
        let report =
            code_index::evaluation::run(&plan, &out, &target, &mut code_index::evaluation::Claude)?;
        println!(
            "{}",
            serde_json::json!({"promotion_passed":report.promotion_passed,"reasons":report.reasons,"totals":report.totals})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("fetch") {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let base = PathBuf::from(value("--store").ok_or("Choose a store with --store DIR.")?);
        let id = code_index::store::fetch(
            &repo,
            &value("--remote").unwrap_or_else(|| "origin".into()),
            &value("--workspace").unwrap_or_else(|| "public".into()),
            &base,
        )?;
        println!("{}", serde_json::json!({"active":id}));
        return Ok(());
    }
    if matches!(
        args.first().map(String::as_str),
        Some("bind" | "refresh" | "publish" | "install" | "rollback" | "withdraw")
    ) {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let context = code_index::recipes::Context::open(
            &repo,
            &value("--rev").unwrap_or_else(|| "HEAD".into()),
            &value("--workspace").unwrap_or_else(|| "local".into()),
        )?;
        let store = value("--store")
            .map(|p| code_index::store::Store::new(std::path::Path::new(&p), &context.snapshot));
        if args[0] == "withdraw" {
            if let Some(store) = &store {
                let removed = store.withdraw_source(
                    &value("--source").ok_or("Choose a source with --source ID.")?,
                    value("--expected").as_deref(),
                )?;
                println!("{}", serde_json::json!({"removed_objects":removed}));
                return Ok(());
            }
        }
        if args[0] == "rollback" {
            let store = store.ok_or("Choose a store with --store DIR.")?;
            store.rollback(
                &value("--digest").ok_or("Choose an artifact with --digest HASH.")?,
                value("--expected").as_deref(),
            )?;
            println!("Rolled back the task index.");
            return Ok(());
        }
        let bundle = if args[0] == "publish" {
            code_index::store::bind(
                &context,
                &code_index::pilot::compile(&context)?,
                Default::default(),
            )?
        } else {
            let input =
                PathBuf::from(value("--input").ok_or("Choose an input with --input FILE.")?);
            if args[0] == "bind" {
                let input: serde_json::Value = code_index::read_json(&input)?;
                if input["schema"] == "openagents.recipe-build.v1" {
                    let build: code_index::builder::Build = serde_json::from_value(input)
                        .map_err(|_| "The build record could not be read.")?;
                    let build = code_index::builder::replay(&context, &build)?;
                    let sources = build
                        .requests
                        .iter()
                        .filter_map(|r| r.private_context.as_ref())
                        .map(|v| atif::digest(v))
                        .collect();
                    code_index::store::bind(&context, &build.artifact, sources)?
                } else {
                    let artifact = serde_json::from_value(input)
                        .map_err(|_| "The task index could not be read.")?;
                    code_index::store::bind(&context, &artifact, Default::default())?
                }
            } else {
                let bundle: code_index::store::Bundle = code_index::read_json(&input)?;
                if args[0] == "refresh" {
                    code_index::store::refresh(&bundle, &context)?
                } else if args[0] == "withdraw" {
                    code_index::store::withdraw(
                        &bundle,
                        &value("--source").ok_or("Choose a source with --source ID.")?,
                    )
                } else {
                    bundle
                }
            }
        };
        if args[0] == "publish" {
            let commit = code_index::store::publish(&repo, &bundle, value("--remote").as_deref())?;
            println!(
                "{}",
                serde_json::json!({"published":commit,"recipes":bundle.artifact.recipes.len()})
            );
        } else if args[0] == "install" {
            let store = store.ok_or("Choose a store with --store DIR.")?;
            let mut valid = bundle.artifact.clone();
            valid.recipes.retain(|r| !bundle.stale.contains_key(&r.id));
            context.verify(&valid)?;
            let id = store.install(&bundle, value("--expected").as_deref())?;
            println!("{}", serde_json::json!({"active":id}));
        } else {
            let out =
                PathBuf::from(value("--out").ok_or("Choose an output directory with --out DIR.")?);
            code_index::private::write(
                &out,
                "bundle.json",
                &serde_json::to_vec(&bundle).map_err(|e| e.to_string())?,
            )?;
            println!(
                "{}",
                serde_json::json!({"recipes":bundle.artifact.recipes.len(),"stale":bundle.stale.len()})
            );
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("query") {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let input: serde_json::Value = code_index::read_json(std::path::Path::new(
            &value("--index").ok_or("Choose an index with --index FILE.")?,
        ))?;
        let bundle = if input["schema"] == "openagents.task-index-bundle.v1" {
            Some(
                serde_json::from_value::<code_index::store::Bundle>(input.clone())
                    .map_err(|_| "The index bundle could not be read.")?,
            )
        } else {
            None
        };
        let artifact = if let Some(bundle) = &bundle {
            bundle.artifact.clone()
        } else {
            serde_json::from_value::<code_index::recipes::Artifact>(input)
                .map_err(|_| "The task index could not be read.")?
        };
        let request = code_index::lookup::Request {
            question: value("--question").ok_or("Choose a question with --question TEXT.")?,
            operation: value("--operation"),
            snapshot: code_index::recipes::snapshot(
                &repo,
                &value("--rev").unwrap_or_else(|| "HEAD".into()),
                &value("--workspace").unwrap_or_else(|| "local".into()),
            )?,
            max_bytes: value("--max-bytes")
                .map(|s| {
                    s.parse::<usize>()
                        .map_err(|_| "Choose an integer context budget.")
                })
                .transpose()?
                .unwrap_or(16000),
            conditions: Default::default(),
            file_candidates: vec![],
            query_vector: None,
            recipe_vectors: Default::default(),
            decision: None,
        };
        let answer = if let Some(bundle) = bundle {
            let graph = code_graph::store::open(&repo)?;
            let dirty = code_index::store::dirty(&bundle, &repo, &graph.index)?;
            code_index::store::query(&bundle, &request, &dirty)?
        } else {
            let mut answer = code_index::lookup::query(&artifact, &request)?;
            if !code_graph::build::code_changes(&repo)?.is_empty() {
                answer.status = code_index::lookup::Status::Stale;
                answer.required_roles_complete = false;
                answer.candidates.clear();
                answer.next_step =
                    "Bind this artifact and check the changed source before using its recipes."
                        .into();
            }
            answer
        };
        println!(
            "{}",
            serde_json::to_string(&answer).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("build")
        || args.first().map(String::as_str) == Some("replay-build")
    {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let out =
            PathBuf::from(value("--out").ok_or("Choose an output directory with --out DIR.")?);
        if args[0] == "replay-build" {
            let record: code_index::builder::Build = code_index::read_json(std::path::Path::new(
                &value("--input").ok_or("Choose a build record with --input FILE.")?,
            ))?;
            let context = code_index::recipes::Context::open(
                &repo,
                &record.artifact.snapshot.commit,
                &record.artifact.snapshot.workspace,
            )?;
            let replayed = code_index::builder::replay(&context, &record)?;
            code_index::private::write(
                &out,
                "task-index.json",
                &serde_json::to_vec(&replayed.artifact).map_err(|e| e.to_string())?,
            )?;
            println!(
                "{}",
                serde_json::json!({"artifact_digest":replayed.artifact_digest,"replayed":true})
            );
            return Ok(());
        }
        let request: code_index::builder::Request = code_index::read_json(std::path::Path::new(
            &value("--input").ok_or("Choose a prepared request with --input FILE.")?,
        ))?;
        let context = code_index::recipes::Context::open(
            &repo,
            &request.snapshot.commit,
            &request.snapshot.workspace,
        )?;
        code_index::private::write(
            &out,
            "build-request.json",
            &serde_json::to_vec(&request).map_err(|e| e.to_string())?,
        )?;
        let result = if let Some(path) = value("--responses") {
            let responses: Vec<code_index::builder::Response> =
                code_index::read_json(std::path::Path::new(&path))?;
            code_index::builder::build(
                &context,
                request,
                &mut code_index::builder::Frozen {
                    responses: responses.into(),
                },
            )?
        } else if args.iter().any(|a| a == "--claude") {
            let mut teacher = code_index::builder::ClaudeTeacher {
                root: repo.clone(),
                output: out.clone(),
                sequence: 0,
            };
            code_index::builder::build(&context, request, &mut teacher)?
        } else {
            let endpoint =
                value("--endpoint").ok_or("Choose a model endpoint or frozen responses.")?;
            let mut teacher = code_index::builder::HttpTeacher::new(
                &endpoint,
                std::env::var("CODE_INDEX_MODEL_TOKEN").ok(),
                request.budget.timeout_secs,
            )?;
            code_index::builder::build(&context, request, &mut teacher)?
        };
        code_index::private::write(
            &out,
            "build.json",
            &serde_json::to_vec(&result).map_err(|e| e.to_string())?,
        )?;
        code_index::private::write(
            &out,
            "task-index.json",
            &serde_json::to_vec(&result.artifact).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"artifact_digest":result.artifact_digest,"model_calls":result.responses.len(),"cost_usd":result.cost_usd})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("prepare-build") {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let out =
            PathBuf::from(value("--out").ok_or("Choose an output directory with --out DIR.")?);
        let context = code_index::recipes::Context::open(
            &repo,
            &value("--rev").unwrap_or_else(|| "HEAD".into()),
            &value("--workspace").unwrap_or_else(|| "local".into()),
        )?;
        let anchors: Vec<code_index::builder::SourceRequest> =
            code_index::read_json(std::path::Path::new(
                &value("--anchors").ok_or("Choose source anchors with --anchors FILE.")?,
            ))?;
        let request = code_index::builder::prepare(
            &context,
            &value("--task").ok_or("Choose a task with --task TEXT.")?,
            &value("--model").ok_or("Choose a model with --model NAME.")?,
            &anchors,
            Default::default(),
        )?;
        code_index::private::write(
            &out,
            "build-request.json",
            &serde_json::to_vec(&request).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"source_references":request.evidence.len(),"model":request.model})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("pilot") {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let out =
            PathBuf::from(value("--out").ok_or("Choose an output directory with --out DIR.")?);
        let context = code_index::recipes::Context::open(
            &repo,
            &value("--rev").unwrap_or_else(|| "HEAD".into()),
            &value("--workspace").unwrap_or_else(|| "local".into()),
        )?;
        let artifact = code_index::pilot::compile(&context)?;
        code_index::private::write(
            &out,
            "task-index.json",
            &serde_json::to_vec(&artifact).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"recipes":artifact.recipes.len(),"commit":artifact.snapshot.commit})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("demand") {
        let input =
            PathBuf::from(value("--input").ok_or("Choose investigations with --input FILE.")?);
        let out = PathBuf::from(
            value("--out").ok_or("Choose a private output directory with --out DIR.")?,
        );
        let corpus: episodes::Corpus = code_index::read_json(&input)?;
        let grouping = value("--grouping")
            .map(|p| {
                code_index::read_json::<code_index::demand::Grouping>(std::path::Path::new(&p))
            })
            .transpose()?;
        let assumptions = value("--assumptions")
            .map(|p| code_index::read_json(std::path::Path::new(&p)))
            .transpose()?
            .unwrap_or_default();
        let demand = code_index::demand::rank(&corpus, grouping.as_ref(), assumptions)?;
        code_index::private::write(
            &out,
            "demand.json",
            &serde_json::to_vec(&demand).map_err(|e| e.to_string())?,
        )?;
        code_index::private::write(
            &out,
            "grouping-request.json",
            &serde_json::to_vec(&code_index::demand::grouping_request(&corpus)?)
                .map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"candidates":demand.candidates.len(),"episodes":corpus.episodes.len(),"cost_assumptions_complete":demand.candidates.iter().all(|c|c.expected_net_saving.is_some())})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("import") {
        return Err("Use code-index import --repo DIR --out DIR with --claude ROOT, --codex ROOT, or --coder ROOT.".into());
    }
    let repo = PathBuf::from(value("--repo").ok_or("Choose the repository with --repo DIR.")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let out =
        PathBuf::from(value("--out").ok_or("Choose a private output directory with --out DIR.")?);
    let history = coder_history::History::open(coder_history::Config {
        claude: value("--claude").map(PathBuf::from),
        codex: value("--codex").map(PathBuf::from),
        coder: value("--coder").map(PathBuf::from),
        ..Default::default()
    })
    .map_err(|e| e.to_string())?;
    let before_ms = value("--before")
        .map(|v| atif::parse_iso(&v).ok_or("Use an ISO 8601 date for --before."))
        .transpose()?;
    let corpus = episodes::import(
        &history,
        &repo,
        Limits {
            before_ms,
            ..Limits::default()
        },
    )?;
    episodes::save(&out, &corpus)?;
    println!("{}", episodes::summary(&corpus));
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
