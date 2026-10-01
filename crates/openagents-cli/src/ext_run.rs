//! `openagents plugin run` (also `ext run`): runs a plugin's workflow once on a workspace,
//! through Coder's program runtime, and prints its reply.
//!
//! This is the step a Coder turn takes after it selects a program, without
//! the selection: the person names the plugin and the request, so the run
//! needs no decision door and no model. The grant is fixed and narrow: the
//! plugin's own workflow, with the `reads` effect only, so a `module`
//! step reads the workspace snapshot its binding grants and nothing
//! delegates, writes, spawns, or reaches the network. A program that
//! needs more is refused before its first step. Nothing is published and
//! nothing in the workspace changes; the read-only smokes and the release
//! gate's Explain this error scenario use it (`docs/plugins/examples/`).

use std::path::{Path, PathBuf};

use coder::package::Package;
use coder::program::{Program, Registry};
use coder::program_authority::Grant;
use coder::runtime::{Inputs, Runtime};
use coder::survey::Survey;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str =
    "usage: openagents plugin run DIR [--in WORKSPACE] [--request TEXT | --request-file FILE]
  Run the workflow of the plugin in DIR once on WORKSPACE (default: the
  current directory) through Coder's program runtime, the way a Coder turn
  runs it once selected, and print its reply. The request is what the
  person would say to Coder, such as a failing command's output.
  The workflow is granted reads only: its Wasm reads the files its
  binding grants, and nothing writes, delegates, spawns, or uses the
  network. Nothing is published. --json prints the run as JSON.";

const NAME: &str = "plugin run";

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|word| matches!(word.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage(NAME, &message, USAGE),
    };
    let Some(dir) = args.positional().first().map(PathBuf::from) else {
        return output.usage(NAME, "the plugin directory is required", USAGE);
    };
    let workspace = args
        .option("in")
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let request = match (args.option("request"), args.option("request-file")) {
        (Some(_), Some(_)) => {
            return output.usage(NAME, "pass --request or --request-file, not both", USAGE);
        }
        (Some(text), None) => text.to_string(),
        (None, Some(path)) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => return output.fail(NAME, &format!("{path}: {error}")),
        },
        (None, None) => String::new(),
    };
    match execute(&dir, &workspace, &request) {
        Ok(ran) => {
            let finished = ran["finished"].as_bool() == Some(true);
            output.emit(&ran, |value| {
                value["reply"].as_str().unwrap_or_default().to_string()
            });
            if finished { 0 } else { crate::EXIT_FAILURE }
        }
        Err(message) => output.fail(NAME, &message),
    }
}

/// Runs the workflow of the plugin at `dir` on `workspace` with
/// `request`, and returns what the run did.
///
/// # Errors
///
/// Names why the plugin doesn't resolve, the workspace can't be read,
/// or the program isn't admitted under the reads-only grant.
pub fn execute(dir: &Path, workspace: &Path, request: &str) -> Result<Value, String> {
    let package = Package::load(&dir.join("package.json"))?;
    let lock = Package::resolve(dir, &package)
        .map_err(|refusal| format!("{}: the package doesn't resolve: {refusal}", dir.display()))?;
    let program_path = dir.join(&lock.program.found);
    let program = Program::load(&program_path)?;
    let programs = Registry::read(program_path.parent().unwrap_or(dir))?;
    let workspace = workspace
        .canonicalize()
        .map_err(|error| format!("{}: {error}", workspace.display()))?;
    let survey = Survey {
        capabilities: Vec::new(),
        programs,
        sources: coder::source::Registry::default(),
        workspace: workspace.clone(),
    };
    let runtime = Runtime::using(survey, None).asking(None);
    runtime
        .admit(&program)
        .map_err(|refused| format!("{} isn't admitted: {refused}", program.slug))?;
    let grant = Grant::selected(Some(&program.slug), Some("reads"));
    let inputs = Inputs::read(request, "");
    runtime
        .authorize(&program, &inputs, &grant)
        .map_err(|refused| {
            format!(
                "{} needs more than reads, and `plugin run` grants reads only: {refused}",
                program.slug
            )
        })?;
    let run = crate::runtime().block_on(runtime.run(&program, &inputs, &grant, None));
    let steps: Vec<Value> = run
        .steps
        .iter()
        .map(|step| {
            let output = serde_json::from_str::<Value>(&step.output)
                .unwrap_or_else(|_| Value::String(step.output.clone()));
            json!({"name": step.name, "kind": step.kind.word(), "output": output})
        })
        .collect();
    Ok(json!({
        "plugin": package.slug,
        "program": program.slug,
        "workspace": workspace.display().to_string(),
        "finished": run.finished(),
        "stopped": run.stopped.as_ref().map(ToString::to_string),
        "steps": steps,
        "reply": run.reply(),
    }))
}
