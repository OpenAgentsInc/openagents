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
    "usage: openagents plugin run NAME_OR_DIR [--in WORKSPACE] [--request TEXT | --request-file FILE]
  Run an installed plugin by name (or KEY:SLUG), or a plugin directory,
  once on WORKSPACE (default: the current directory) through Coder's
  program runtime, the way a Coder turn runs it once selected, and print its reply. The request is what the
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
        return output.usage(NAME, "the plugin name or directory is required", USAGE);
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
    let dir = match resolve_plugin(&dir) {
        Ok(dir) => dir,
        Err(message) => return output.fail(NAME, &message),
    };
    match execute(&dir, &workspace, &request) {
        Ok(ran) => {
            let finished = ran["finished"].as_bool() == Some(true);
            output.emit(&ran, human);
            if finished { 0 } else { crate::EXIT_FAILURE }
        }
        Err(message) => output.fail(NAME, &message),
    }
}

fn resolve_plugin(target: &Path) -> Result<PathBuf, String> {
    if target.join("package.json").is_file() {
        return Ok(target.to_path_buf());
    }
    let layout = background::Layout::from_env().map_err(|error| error.to_string())?;
    resolve_installed(&layout, target)
}

fn resolve_installed(layout: &background::Layout, target: &Path) -> Result<PathBuf, String> {
    if target.join("package.json").is_file() {
        return Ok(target.to_path_buf());
    }
    if target.exists() {
        let canonical = target.canonicalize().map_err(|error| error.to_string())?;
        if let Some(plugin) = background::plugins::installed(layout)
            .into_iter()
            .find(|plugin| {
                plugin.dir.parent() == Some(canonical.as_path()) || plugin.dir == canonical
            })
        {
            return Ok(plugin.dir);
        }
        return Err(format!(
            "{} is not a plugin directory (no package.json)",
            target.display()
        ));
    }
    background::plugins::find(layout, &target.to_string_lossy()).map(|plugin| plugin.dir)
}

/// The run as a person reads it: the reply, and when the workflow's guest
/// rendered nothing, what its last step found, laid out (#10323).
fn human(ran: &Value) -> String {
    let reply = ran["reply"].as_str().unwrap_or_default();
    if ran["rendered"].as_bool() == Some(true) {
        return reply.to_string();
    }
    let found = ran["steps"]
        .as_array()
        .and_then(|steps| {
            steps
                .iter()
                .rev()
                .find(|step| step["output"]["status"].as_str() == Some("ok"))
        })
        .map(|step| &step["output"]["value"]);
    match found {
        Some(value) if !value.is_null() => {
            let mut lines = Vec::new();
            readable(value, 0, &mut lines);
            if lines.len() > READABLE_LINES {
                let more = lines.len() - READABLE_LINES;
                lines.truncate(READABLE_LINES);
                lines.push(format!("… {more} more lines; --json prints all of it"));
            }
            format!("{}\n\n{reply}", lines.join("\n"))
        }
        _ => reply.to_string(),
    }
}

/// At most this many lines of a step's value are shown.
const READABLE_LINES: usize = 80;

/// `value` as indented `key: value` lines, a list of records one per line.
fn readable(value: &Value, depth: usize, lines: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                let key = key.replace('_', " ");
                match item {
                    Value::Object(_) => {
                        lines.push(format!("{pad}{key}:"));
                        readable(item, depth + 1, lines);
                    }
                    Value::Array(list) if list.is_empty() => {
                        lines.push(format!("{pad}{key}: none"))
                    }
                    Value::Array(list) if list.iter().all(|entry| !entry.is_object()) => {
                        lines.push(format!(
                            "{pad}{key}: {}",
                            list.iter().map(scalar).collect::<Vec<_>>().join(", ")
                        ));
                    }
                    Value::Array(list) => {
                        lines.push(format!("{pad}{key}:"));
                        for entry in list {
                            lines.push(format!("{pad}  - {}", record(entry)));
                        }
                    }
                    other => lines.push(format!("{pad}{key}: {}", scalar(other))),
                }
            }
        }
        Value::Array(list) => {
            for entry in list {
                lines.push(format!("{pad}- {}", record(entry)));
            }
        }
        other => lines.push(format!("{pad}{}", scalar(other))),
    }
}

/// One record on one line: `path Cargo.toml · bytes 35`.
fn record(entry: &Value) -> String {
    match entry {
        Value::Object(map) => map
            .iter()
            .map(|(key, item)| format!("{} {}", key.replace('_', " "), scalar(item)))
            .collect::<Vec<_>>()
            .join(" · "),
        other => scalar(other),
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "none".into(),
        Value::Object(_) | Value::Array(_) => value.to_string(),
        other => other.to_string(),
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
    let Some(pinned) = &lock.program else {
        let name = if package.name.is_empty() {
            &package.slug
        } else {
            &package.name
        };
        return Err(if package.background.is_empty() {
            format!(
                "{name} has no workflow to run: its skills guide Coder's runs while it is on (`openagents plugin install {}` and `openagents plugin enable {}`).",
                dir.display(),
                package.slug
            )
        } else {
            format!(
                "{name} has no workflow to run here; it runs in the background when it is on (`openagents plugin enable {}`).",
                package.slug
            )
        });
    };
    let program_path = dir.join(&pinned.found);
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
        "rendered": run.rendered().is_some(),
        "reply": run.reply(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_names_and_parent_directories_resolve_to_the_version() {
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(source.path().join("programs")).unwrap();
        let program = json!({
            "slug": "project-map",
            "definition": {
                "id": format!("{}:project-map/project-map", background::plugins::LOCAL_KEY),
                "steps": [{"name": "repo_map", "kind": "module"}]
            }
        })
        .to_string();
        std::fs::write(source.path().join("programs/project-map.json"), &program).unwrap();
        std::fs::write(
            source.path().join("package.json"),
            json!({
                "v": 1, "slug": "project-map", "name": "Project map", "version": "0.1.0",
                "program": {"name": "project-map", "digest": coder::package::digest(&program)}
            })
            .to_string(),
        )
        .unwrap();
        crate::plugin_local::install_into(&layout, source.path()).unwrap();
        let installed = background::plugins::find(&layout, "project-map").unwrap();
        assert_eq!(
            resolve_installed(&layout, Path::new("project-map")).unwrap(),
            installed.dir
        );
        assert_eq!(
            resolve_installed(&layout, Path::new(&installed.id)).unwrap(),
            installed.dir
        );
        assert_eq!(
            resolve_installed(&layout, installed.dir.parent().unwrap()).unwrap(),
            installed.dir
        );
        assert_eq!(
            resolve_installed(&layout, source.path()).unwrap(),
            source.path()
        );
        assert!(
            resolve_installed(&layout, Path::new("missing-plugin"))
                .unwrap_err()
                .contains("No plugin")
        );
        let other = layout
            .extensions()
            .join("b".repeat(64))
            .join("project-map/0.1.0");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::copy(
            source.path().join("package.json"),
            other.join("package.json"),
        )
        .unwrap();
        assert!(
            resolve_installed(&layout, Path::new("project-map"))
                .unwrap_err()
                .contains("Several")
        );
        assert_eq!(
            resolve_installed(&layout, Path::new(&installed.id)).unwrap(),
            installed.dir
        );
    }

    #[test]
    fn a_run_whose_guest_renders_nothing_shows_what_it_found() {
        let ran = json!({
            "rendered": false,
            "reply": "project-map ran its 1 step.",
            "steps": [{"output": {"status": "ok", "value": {
                "files": 2,
                "languages": [{"language": "Rust", "files": 1}],
                "manifests": ["Cargo.toml"],
                "tests": {"dirs": [], "files": 0}
            }}}]
        });
        let text = human(&ran);
        assert!(text.contains("files: 2"), "{text}");
        assert!(text.contains("- language Rust · files 1"), "{text}");
        assert!(text.contains("manifests: Cargo.toml"), "{text}");
        assert!(text.contains("  dirs: none"), "{text}");
        assert!(text.ends_with("project-map ran its 1 step."), "{text}");
    }

    #[test]
    fn a_rendered_reply_is_shown_as_it_is() {
        let ran = json!({"rendered": true, "reply": "# Map\n\nran its 1 step.", "steps": []});
        assert_eq!(human(&ran), "# Map\n\nran its 1 step.");
    }
}
