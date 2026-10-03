use briefing_lab::{Components, ExecutionBrief, Index, Issue, Options, Result};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    time::Instant,
};

const HELP: &str = "briefing-lab index --repo PATH --rev COMMIT_OR_REF --output FILE [--syntax]
briefing-lab preview --repo PATH --rev COMMIT_OR_REF --index FILE --issue-file FILE --output-dir DIR [--no-lexical] [--no-symbols] [--no-history] [--syntax]
  [--execution --manifest PACKAGE/Cargo.toml --environment-id LABEL]
  [--require-tool NAME] [--require-file PATH] [--attempt-dir RUN_DIR]
briefing-lab prepare-run --repo PATH --rev COMMIT_OR_REF --issue-file FILE --manifest PACKAGE/Cargo.toml --environment-id LABEL --artifact-root DIR
  [--require-tool NAME] [--require-file PATH]
briefing-lab record-result --run-dir DIR --request-sha256 DIGEST --exit-code CODE --summary TEXT --next-action TEXT

--manifest, --require-tool, --require-file, and --attempt-dir can repeat where applicable.
All artifacts must be outside the inspected repository. Source is read from Git.
No command is executed. prepare-run allocates artifacts; record-result stores a caller-reported result.
Prerequisite checks always include cargo and git, plus explicitly requested tools/files.";

#[derive(Default)]
struct Args {
    command: String,
    values: BTreeMap<String, String>,
    manifests: Vec<String>,
    tools: Vec<String>,
    files: Vec<PathBuf>,
    attempts: Vec<PathBuf>,
    execution: bool,
    components: Components,
    options: Options,
}
impl Args {
    fn required(&self, key: &str) -> Result<String> {
        self.values
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Missing {key}.").into())
    }
    fn parse(command: String, args: impl Iterator<Item = String>) -> Result<Self> {
        let mut result = Self {
            command,
            ..Self::default()
        };
        let mut args = args.peekable();
        while let Some(key) = args.next() {
            match (result.command.as_str(), key.as_str()) {
                ("index" | "preview", "--syntax") => result.options.syntax = true,
                ("preview", "--no-lexical") => result.components.lexical = false,
                ("preview", "--no-symbols") => result.components.symbols = false,
                ("preview", "--no-history") => result.components.history = false,
                ("preview", "--execution") => result.execution = true,
                (command, key) => {
                    let common = command != "record-result" && matches!(key, "--repo" | "--rev");
                    let allowed = common
                        || match command {
                            "index" => key == "--output",
                            "preview" => matches!(
                                key,
                                "--index"
                                    | "--issue-file"
                                    | "--output-dir"
                                    | "--manifest"
                                    | "--environment-id"
                                    | "--require-tool"
                                    | "--require-file"
                                    | "--attempt-dir"
                            ),
                            "prepare-run" => matches!(
                                key,
                                "--issue-file"
                                    | "--manifest"
                                    | "--environment-id"
                                    | "--artifact-root"
                                    | "--require-tool"
                                    | "--require-file"
                            ),
                            "record-result" => matches!(
                                key,
                                "--run-dir"
                                    | "--request-sha256"
                                    | "--exit-code"
                                    | "--summary"
                                    | "--next-action"
                            ),
                            _ => false,
                        };
                    if !allowed {
                        return Err(format!("Unknown option for {command}: {key}").into());
                    }
                    let value = args
                        .next()
                        .filter(|v| !v.starts_with("--"))
                        .ok_or_else(|| format!("Missing value for {key}."))?;
                    match key {
                        "--manifest" => result.manifests.push(value),
                        "--require-tool" => result.tools.push(value),
                        "--require-file" => result.files.push(value.into()),
                        "--attempt-dir" => result.attempts.push(value.into()),
                        _ => {
                            if result.values.insert(key.into(), value).is_some() {
                                return Err(format!("Duplicate option: {key}").into());
                            }
                        }
                    }
                }
            }
        }
        if result.command == "preview"
            && !result.execution
            && (!result.manifests.is_empty()
                || !result.tools.is_empty()
                || !result.files.is_empty()
                || !result.attempts.is_empty()
                || result.values.contains_key("--environment-id"))
        {
            return Err("Execution options require --execution.".into());
        }
        if result.attempts.len() > 8 {
            return Err("Supply at most eight prior attempt directories.".into());
        }
        Ok(result)
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("briefing-lab: {error}");
        process::exit(1);
    }
}

fn environment(args: &Args) -> Result<briefing_lab::environment::EnvironmentSnapshot> {
    let mut tools = vec!["cargo".to_owned(), "git".to_owned()];
    tools.extend(args.tools.iter().cloned());
    tools.sort();
    tools.dedup();
    briefing_lab::environment::inspect(&args.required("--environment-id")?, &tools, &args.files)
}

fn environment_key(snapshot: &briefing_lab::environment::EnvironmentSnapshot) -> String {
    format!("{}:{}", snapshot.label, snapshot.fingerprint)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

fn run() -> Result<()> {
    let mut cli = env::args().skip(1);
    let command = cli.next().unwrap_or_default();
    if command == "--help" || command.is_empty() {
        println!("{HELP}");
        return Ok(());
    }
    if !matches!(
        command.as_str(),
        "index" | "preview" | "prepare-run" | "record-result"
    ) {
        return Err("Expected index, preview, prepare-run, or record-result.".into());
    }
    let args = Args::parse(command, cli)?;
    if args.command == "record-result" {
        let result = briefing_lab::attempt::record_result(
            &PathBuf::from(args.required("--run-dir")?),
            &args.required("--request-sha256")?,
            args.required("--exit-code")?.parse()?,
            &args.required("--summary")?,
            &args.required("--next-action")?,
        )?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    let repo = PathBuf::from(args.required("--repo")?);
    let revision = args.required("--rev")?;
    if args.command == "index" {
        let output = PathBuf::from(args.required("--output")?);
        briefing_lab::check_output(&repo, &output)?;
        let index = briefing_lab::build_index_with_options(&repo, &revision, args.options)?;
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, serde_json::to_vec(&index)?)?;
        println!(
            "Indexed {} files at {} in {:.3} ms: {}",
            index.files.len(),
            index.commit,
            index.index_ms,
            output.display()
        );
        return Ok(());
    }
    let total = Instant::now();
    let verify_start = Instant::now();
    let commit = briefing_lab::resolve(&repo, &revision)?;
    let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
    let load_start = Instant::now();
    let issue_bytes = fs::read(args.required("--issue-file")?)?;
    let issue: Issue = serde_json::from_slice(&issue_bytes)?;
    let issue_digest = briefing_lab::issue_digest(&issue)?;
    if args.command == "prepare-run" {
        if args.manifests.len() != 1 {
            return Err("prepare-run requires exactly one --manifest.".into());
        }
        let manifest = briefing_lab::execution::prepare(&repo, &commit, &args.manifests)?;
        let environment = environment(&args)?;
        if !environment.ready {
            return Err(format!(
                "Requested prerequisites are missing: {}",
                serde_json::to_string(&environment)?
            )
            .into());
        }
        let request = briefing_lab::attempt::prepare_run(
            &repo,
            &PathBuf::from(args.required("--artifact-root")?),
            &commit,
            &issue_digest,
            &environment_key(&environment),
            manifest.packages[0].test_argv.clone(),
        )?;
        write_new(
            &PathBuf::from(&request.run_dir).join("preparation.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "openagents.briefing-lab.preparation.v1",
                "manifest": manifest,
                "environment": environment,
                "issue_sha256": issue_digest,
                "notes": ["No command has run. The request names committed source, not a verified clean execution checkout. Claim admission and runtime deployment are outside this experiment."],
                "preparation_ms_before_output": total.elapsed().as_secs_f64() * 1000.0,
            }))?,
        )?;
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }
    let index: Index = serde_json::from_slice(&fs::read(args.required("--index")?)?)?;
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let mut brief = briefing_lab::assemble_with_options(
        &repo,
        &index,
        &commit,
        issue,
        args.components,
        args.options,
    )?;
    if args.execution {
        let preparation_start = Instant::now();
        if args.manifests.is_empty() {
            return Err("--execution requires at least one explicit --manifest.".into());
        }
        let manifest = briefing_lab::execution::prepare(&repo, &commit, &args.manifests)?;
        let environment = environment(&args)?;
        let key = environment_key(&environment);
        let commands: Vec<_> = manifest
            .packages
            .iter()
            .map(|package| package.test_argv.clone())
            .collect();
        let attempts = args
            .attempts
            .iter()
            .map(|path| {
                briefing_lab::attempt::load_attempt(path, &commit, &issue_digest, &key, &commands)
            })
            .collect::<Result<Vec<_>>>()?;
        brief.execution = Some(ExecutionBrief {
            schema: "openagents.briefing-lab.execution-preview.v1".into(),
            manifest,
            environment,
            attempts,
            notes: vec![
                "Package scope and prerequisite lists are explicit inputs. This preview does not infer complete build requirements or select a sufficient acceptance suite.".into(),
                "Attempt applicability compares the declared commit, complete issue digest, observed prerequisite fingerprint, and proposed command. It does not verify the actual checkout, runner, runtime version, claims, or every environment input.".into(),
            ],
        });
        brief.timings_ms.insert(
            "execution_preparation".into(),
            preparation_start.elapsed().as_secs_f64() * 1000.0,
        );
    }
    let directory = PathBuf::from(args.required("--output-dir")?);
    briefing_lab::check_output(&repo, &directory)?;
    briefing_lab::check_output(&repo, &directory.join("briefing.json"))?;
    briefing_lab::check_output(&repo, &directory.join("briefing.md"))?;
    brief
        .timings_ms
        .insert("revision_validation".into(), verify_ms);
    brief
        .timings_ms
        .insert("index_and_issue_load".into(), load_ms);
    brief
        .timings_ms
        .insert("original_index_build_separate".into(), index.index_ms);
    brief.timings_ms.insert(
        "warm_preview_before_output".into(),
        total.elapsed().as_secs_f64() * 1000.0,
    );
    brief.notes.push(format!(
        "Input issue JSON SHA-256: {}",
        briefing_lab::sha256(&issue_bytes)
    ));
    let serialize_start = Instant::now();
    let _ = serde_json::to_vec(&brief)?;
    let _ = briefing_lab::markdown(&brief);
    brief.timings_ms.insert(
        "output_serialization_sample".into(),
        serialize_start.elapsed().as_secs_f64() * 1000.0,
    );
    fs::create_dir_all(&directory)?;
    fs::write(
        directory.join("briefing.json"),
        serde_json::to_vec_pretty(&brief)?,
    )?;
    fs::write(
        directory.join("briefing.md"),
        briefing_lab::markdown(&brief),
    )?;
    println!(
        "Preview: {}\nStructured evidence: {}\nComplete local preview including output: {:.3} ms",
        directory.join("briefing.md").display(),
        directory.join("briefing.json").display(),
        total.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
