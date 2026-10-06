//! `openagents plugin inspect [NAME]` and `openagents plugin use NAME`:
//! reusing an installed component by its exact release (#10664).
//!
//! `inspect` reads what this computer holds: each installed plugin's exact
//! pin (its `KEY:SLUG` ID, version, and the digest of its package record),
//! whether it is on, and the retained test results whose subject is that
//! exact release, apart from results for other releases of it. It runs
//! nothing, probes nothing, and asks no relay; revocation is checked by
//! `install` from the registry and is reported here as not checked.
//!
//! `use` runs one installed plugin's workflow once through the shared
//! route: the person names the exact pin they inspected, the route is
//! admitted under `openagents_chat::route::admit` and journaled, and
//! `openagents_chat::capability::dispatch` runs it only while
//! `capability::reuse` admits exactly that release. A newer version, other
//! bytes, a disabled or removed plugin is refused and nothing runs. The
//! run is the same read-only workflow run as `openagents plugin run`, and
//! its output is kept by digest. The same request again follows the
//! journal's record instead of running twice.

use std::path::{Path, PathBuf};

use background::Layout;
use background::plugins::{self, Installed};
use openagents_chat::capability::{
    self, Dispatched, Held, Output as RunOutput, Release, Reuse, Runner,
};
use openagents_chat::route::{Journal, Situation, THIS_COMPUTER, admit};
use route_contract::lifecycle::{CheckLabel, Lifecycle};
use route_contract::route::PluginRoute;
use route_contract::snapshot::{CapabilityPin, CheckScope, Surface};
use route_contract::{Digest, RouteRecord, RouteResult};
use serde_json::{Value, json};

use crate::{Args, EXIT_FAILURE, Output};

pub(crate) const INSPECT_USAGE: &str = "usage: openagents plugin inspect [NAME] [--results DIR]...
  Show each installed plugin (or NAME) by its exact release: KEY:SLUG,
  version, and the digest of its package record; whether it is on; and
  the test results under each DIR (default .) whose subject is exactly
  that release, apart from results for other releases. Reads only.";

pub(crate) const USE_USAGE: &str =
    "usage: openagents plugin use NAME --version V --digest D [--request TEXT]
      [--in WORKSPACE] [--thread ID]
  Run installed plugin NAME's workflow once through the shared route,
  only while exactly version V with package digest D is installed and on.
  Another version, other bytes, or a plugin that is off or gone is refused
  and nothing runs. The run is granted reads only on WORKSPACE (default
  .). The same request again shows the first run's result.";

/// `plugin inspect` and `plugin use`; `None` for any other command.
pub fn run(output: &Output, words: &[String]) -> Option<u8> {
    let (command, rest) = words.split_first()?;
    let (name, usage) = match command.as_str() {
        "inspect" => ("plugin inspect", INSPECT_USAGE),
        "use" => ("plugin use", USE_USAGE),
        _ => return None,
    };
    if rest
        .first()
        .is_some_and(|word| matches!(word.as_str(), "--help" | "-h"))
    {
        println!("{usage}");
        return Some(0);
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return Some(output.usage(name, &message, usage)),
    };
    let layout = match Layout::from_env() {
        Ok(layout) => layout,
        Err(error) => return Some(output.fail(name, &error.to_string())),
    };
    Some(if command == "inspect" {
        let roots: Vec<PathBuf> = match args.options("results") {
            dirs if dirs.is_empty() => vec![PathBuf::from(".")],
            dirs => dirs.into_iter().map(PathBuf::from).collect(),
        };
        match inspect(
            &layout,
            args.positional().first().map(String::as_str),
            &roots,
        ) {
            Ok(value) => {
                output.emit(&value, |value| {
                    value["text"].as_str().unwrap_or_default().to_owned()
                });
                0
            }
            Err(message) => output.fail(name, &message),
        }
    } else {
        let Some(plugin) = args.positional().first() else {
            return Some(output.usage(name, "use needs the plugin's name", usage));
        };
        let (Some(version), Some(digest)) = (args.option("version"), args.option("digest")) else {
            return Some(output.usage(
                name,
                "use needs the exact --version and --digest that inspect shows",
                usage,
            ));
        };
        let Ok(digest) = Digest::try_from(digest.to_owned()) else {
            return Some(output.usage(name, "--digest is sha256: and 64 hex digits", usage));
        };
        let home = crate::ext_eval::openagents_home();
        let request = Use {
            plugin,
            pin: CapabilityPin {
                id: String::new(),
                version: version.to_owned(),
                digest,
            },
            request: args.option("request").unwrap_or_default(),
            workspace: Path::new(args.option("in").unwrap_or(".")),
            thread: args.option("thread").unwrap_or("plugin-use"),
        };
        match use_plugin(
            &layout,
            &request,
            &Journal::at(home.join("routes")),
            &home.join("route-artifacts"),
            &mut Workflow::default(),
        ) {
            Ok(value) => {
                let ran = value["dispatched"] == "ran" || value["dispatched"] == "followed";
                output.emit(&value, |value| {
                    value["text"].as_str().unwrap_or_default().to_owned()
                });
                if ran { 0 } else { EXIT_FAILURE }
            }
            Err(message) => output.fail(name, &message),
        }
    })
}

/// What this computer holds of `plugin` now: its exact pin, on or off.
/// Nobody looked for a revocation here.
fn held_of(plugin: &Installed) -> Held {
    let bytes = std::fs::read(plugin.dir.join("package.json")).unwrap_or_default();
    Held {
        pin: CapabilityPin {
            id: plugin.id.clone(),
            version: plugin.version.clone(),
            digest: Digest::of_bytes(&bytes),
        },
        enabled: plugin.enabled,
        revoked: None,
    }
}

/// Whether a result's subject is `plugin`: by the package slug in its
/// DefinitionRef ID (`<publisher>:<slug>/<program>`).
fn names(subject: &str, plugin: &Installed) -> bool {
    subject
        .split_once(':')
        .and_then(|(_, rest)| rest.split('/').next())
        .is_some_and(|slug| slug == plugin.slug)
}

fn inspect(layout: &Layout, name: Option<&str>, roots: &[PathBuf]) -> Result<Value, String> {
    let plugins = match name {
        Some(name) => vec![plugins::find(layout, name)?],
        None => plugins::installed(layout),
    };
    let studies: Vec<ext_eval::study::Listed> = roots
        .iter()
        .flat_map(|root| ext_eval::study::list(root))
        .collect();
    let mut rows = Vec::new();
    let mut lines = Vec::new();
    for plugin in &plugins {
        let held = held_of(plugin);
        let evidence: Vec<Value> = studies
            .iter()
            .filter(|study| study.subject.as_deref().is_some_and(|s| names(s, plugin)))
            .map(|study| {
                json!({
                    "dir": study.dir,
                    "reported": study.reported,
                    "ended_at": study.ended_at,
                    "exact": study.subject_digest.as_deref() == Some(held.pin.digest.as_str()),
                })
            })
            .collect();
        let exact = evidence.iter().filter(|e| e["exact"] == true).count();
        let has_workflow = coder::package::Package::load(&plugin.dir.join("package.json"))
            .is_ok_and(|package| package.program.is_some());
        lines.push(format!(
            "{} {} {}  {}  {} results for this release, {} for others",
            plugin.id,
            plugin.version,
            held.pin.digest,
            if plugin.enabled { "on" } else { "off" },
            exact,
            evidence.len() - exact
        ));
        rows.push(json!({
            "id": plugin.id,
            "slug": plugin.slug,
            "name": plugin.name,
            "version": plugin.version,
            "digest": held.pin.digest.as_str(),
            "enabled": plugin.enabled,
            "dir": plugin.dir.display().to_string(),
            "background": plugin.background,
            "workflow": has_workflow,
            "revocation": "not_checked",
            "evidence": evidence,
            "commands": {
                "test": format!("openagents plugin test run {}", plugin.dir.display()),
                "turn": format!(
                    "openagents plugin {} {}",
                    if plugin.enabled { "disable" } else { "enable" },
                    plugin.id
                ),
                "use": has_workflow.then(|| format!(
                    "openagents plugin use {} --version {} --digest {} --request TEXT",
                    plugin.id, plugin.version, held.pin.digest
                )),
            },
        }));
    }
    if rows.is_empty() {
        lines.push("No plugins are installed on this computer.".into());
    }
    Ok(json!({
        "v": "openagents.plugin-inspect.v1",
        "roots": roots.iter().map(|root| root.display().to_string()).collect::<Vec<_>>(),
        "plugins": rows,
        "text": lines.join("\n"),
    }))
}

/// One `use`: the plugin named, the exact pin the person admitted (its ID
/// is filled from the plugin found), and the request.
struct Use<'a> {
    plugin: &'a str,
    pin: CapabilityPin,
    request: &'a str,
    workspace: &'a Path,
    thread: &'a str,
}

/// Runs an installed plugin's workflow: the read-only run `plugin run`
/// makes. Its output is the run as JSON; nothing independent checks it.
#[derive(Default)]
struct Workflow {
    dir: PathBuf,
    workspace: PathBuf,
}

impl Runner for Workflow {
    fn run(&mut self, _: &Release, arguments: &Value) -> Result<RunOutput, String> {
        let request = arguments["request"].as_str().unwrap_or_default();
        let ran = crate::ext_run::execute(&self.dir, &self.workspace, request)?;
        if ran["finished"] != true {
            return Err(ran["stopped"]
                .as_str()
                .unwrap_or("the workflow did not finish")
                .to_owned());
        }
        Ok(RunOutput {
            artifact: serde_json::to_vec_pretty(&ran).unwrap_or_default(),
            check: CheckLabel::Unchecked,
            cost_microusd: Some(0),
        })
    }
}

fn use_plugin(
    layout: &Layout,
    request: &Use<'_>,
    journal: &Journal,
    artifacts: &Path,
    runner: &mut dyn RunnerAt,
) -> Result<Value, String> {
    let installed = plugins::installed(layout);
    let found = plugins::find(layout, request.plugin)?;
    let held: Vec<Held> = installed.iter().map(held_of).collect();
    let pin = CapabilityPin {
        id: found.id.clone(),
        ..request.pin.clone()
    };
    let reuse = capability::reuse(&pin, &held);
    let arguments = json!({"request": request.request});
    let workspace = request
        .workspace
        .canonicalize()
        .map_err(|error| format!("{}: {error}", request.workspace.display()))?;
    // The same pin, request, and workspace is the same request: once it
    // ran, it follows the journal rather than running again. A refused one
    // may be asked again.
    let key = Digest::of_bytes(
        json!([pin, request.request, workspace.display().to_string()])
            .to_string()
            .as_bytes(),
    );
    let request_id = format!("use-{}", &key.as_str()[7..23]);
    if let Some(record) = journal
        .latest(request.thread, &request_id)
        .filter(|record| !record.runs.is_empty())
    {
        return Ok(answer(&record, &pin, &reuse, "followed", None, artifacts));
    }
    let situation = Situation {
        surface: Surface::Cli,
        caller: "local:openagents-cli".into(),
        request: request_id.clone(),
        thread: Some(request.thread.to_owned()),
        computer: THIS_COMPUTER.into(),
        project: None,
        ready: true,
        bound: None,
        check: CheckScope::ExecutorExit,
    };
    let result = RouteResult::Plugin {
        plugin: PluginRoute::Run {
            capability: pin.clone(),
            arguments,
        },
    };
    let snapshot = admit(&result, &situation, None, request.request, None);
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            })
    };
    let mut record = RouteRecord::received(
        request_id,
        situation.thread.clone(),
        result,
        snapshot,
        now(),
    )
    .map_err(|error| format!("the route record was refused: {error:?}"))?;
    // Naming the exact pin on the command line is the person's admission.
    record
        .step(Lifecycle::Proposed, "offer", now())
        .map_err(|error| format!("the route could not be offered: {error:?}"))?;
    record
        .step(Lifecycle::Admitted, "exact_pin_named", now())
        .map_err(|error| format!("the route could not be admitted: {error:?}"))?;
    journal.write(&record).map_err(|error| error.to_string())?;
    runner.at(&found.dir, &workspace);
    let catalog = capability::installed_catalog(&held);
    let mut failure = None;
    let dispatched = capability::dispatch(
        &mut record,
        &catalog,
        runner.runner(),
        journal,
        &mut |digest, bytes| {
            let path = artifacts.join(&digest.as_str()[7..]);
            if let Err(error) =
                std::fs::create_dir_all(artifacts).and_then(|()| std::fs::write(&path, bytes))
            {
                failure = Some(format!("{}: {error}", path.display()));
            }
        },
        now(),
    )
    .map_err(|error| error.to_string())?;
    if let Some(failure) = failure {
        return Err(failure);
    }
    let (word, reason) = match &dispatched {
        Dispatched::Ran { .. } => ("ran", None),
        Dispatched::Failed { reason } => ("failed", Some(reason.clone())),
        Dispatched::Refused(_) => ("refused", None),
        Dispatched::NewOffer { reason, .. } => ("new_offer", Some(reason.clone())),
        Dispatched::Followed => ("followed", None),
    };
    Ok(answer(&record, &pin, &reuse, word, reason, artifacts))
}

/// The runner `use` dispatches through, pointed at the plugin and
/// workspace first; a test supplies its own.
trait RunnerAt {
    fn at(&mut self, dir: &Path, workspace: &Path);
    fn runner(&mut self) -> &mut dyn Runner;
}

impl RunnerAt for Workflow {
    fn at(&mut self, dir: &Path, workspace: &Path) {
        self.dir = dir.to_path_buf();
        self.workspace = workspace.to_path_buf();
    }
    fn runner(&mut self) -> &mut dyn Runner {
        self
    }
}

fn answer(
    record: &RouteRecord,
    pin: &CapabilityPin,
    reuse: &Reuse,
    dispatched: &str,
    reason: Option<String>,
    artifacts: &Path,
) -> Value {
    let outputs: Vec<String> = record
        .runs
        .iter()
        .flat_map(|run| run.artifacts.iter())
        .map(|digest| artifacts.join(&digest.as_str()[7..]).display().to_string())
        .collect();
    let why = match reuse {
        Reuse::Admitted => None,
        Reuse::Changed { held } => Some(format!(
            "this computer holds {} {} ({}), not the release named; inspect it and name it to use it",
            held.id, held.version, held.digest
        )),
        Reuse::Disabled => Some(format!("{} is off; turn it on first", pin.id)),
        Reuse::Revoked => Some(format!("{}'s release was revoked", pin.id)),
        Reuse::Missing => Some(format!("{} is not installed", pin.id)),
    };
    let text = match (dispatched, &why) {
        ("ran" | "followed", _) => {
            let reply = outputs
                .first()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|ran| ran["reply"].as_str().map(str::to_owned))
                .unwrap_or_default();
            format!(
                "{reply}\n\n{} {} ran once under route {} ({}); unchecked.",
                pin.id,
                pin.version,
                record.request,
                if dispatched == "followed" {
                    "shown from the journal, not run again"
                } else {
                    "journaled"
                }
            )
        }
        (_, Some(why)) => format!("Not run: {why}."),
        (_, None) => format!(
            "Not run: {}.",
            reason
                .clone()
                .unwrap_or_else(|| dispatched.replace('_', " "))
        ),
    };
    json!({
        "v": "openagents.plugin-use.v1",
        "dispatched": dispatched,
        "reuse": reuse.word(),
        "reason": reason.or(why),
        "pin": pin,
        "request": record.request,
        "thread": record.thread,
        "state": record.state,
        "outputs": outputs,
        "text": text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Installs a plugin with a workflow under a scratch home.
    fn install(layout: &Layout, version: &str) -> Installed {
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(source.path().join("programs")).unwrap();
        let program = json!({
            "slug": "notes",
            "definition": {
                "id": format!("{}:notes/notes", plugins::LOCAL_KEY),
                "steps": [{"name": "read", "kind": "module"}]
            }
        })
        .to_string();
        std::fs::write(source.path().join("programs/notes.json"), &program).unwrap();
        std::fs::write(
            source.path().join("package.json"),
            json!({
                "v": 1, "slug": "notes", "name": "Meeting notes", "version": version,
                "program": {"name": "notes", "digest": coder::package::digest(&program)}
            })
            .to_string(),
        )
        .unwrap();
        crate::plugin_local::install_into(layout, source.path()).unwrap();
        plugins::find(layout, "notes").unwrap()
    }

    #[derive(Default)]
    struct Fake {
        runs: usize,
        dir: PathBuf,
    }

    impl Runner for Fake {
        fn run(&mut self, _: &Release, arguments: &Value) -> Result<RunOutput, String> {
            self.runs += 1;
            Ok(RunOutput {
                artifact: serde_json::to_vec(&json!({
                    "finished": true,
                    "reply": format!("Action items for: {}", arguments["request"].as_str().unwrap()),
                }))
                .unwrap(),
                check: CheckLabel::Unchecked,
                cost_microusd: Some(0),
            })
        }
    }

    impl RunnerAt for Fake {
        fn at(&mut self, dir: &Path, _: &Path) {
            self.dir = dir.to_path_buf();
        }
        fn runner(&mut self) -> &mut dyn Runner {
            self
        }
    }

    #[test]
    fn an_inspected_release_runs_once_and_a_changed_one_does_not() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let journal = Journal::at(home.path().join("routes"));
        let artifacts = home.path().join("route-artifacts");
        let workspace = tempfile::tempdir().unwrap();
        let plugin = install(&layout, "0.1.0");

        // Inspecting reads the exact pin and runs nothing.
        let inspected = inspect(&layout, Some("notes"), &[home.path().to_path_buf()]).unwrap();
        let row = &inspected["plugins"][0];
        assert_eq!(row["version"], "0.1.0");
        assert_eq!(row["enabled"], false);
        assert_eq!(row["revocation"], "not_checked");
        let digest = Digest::try_from(row["digest"].as_str().unwrap().to_owned()).unwrap();
        assert!(
            row["commands"]["use"]
                .as_str()
                .unwrap()
                .contains(digest.as_str())
        );

        let mut fake = Fake::default();
        let ask = |version: &str| Use {
            plugin: "notes",
            pin: CapabilityPin {
                id: String::new(),
                version: version.into(),
                digest: digest.clone(),
            },
            request: "Ana sends the budget by Friday.",
            workspace: workspace.path(),
            thread: "plugin-use",
        };
        // Off: refused, nothing runs.
        let answer = use_plugin(&layout, &ask("0.1.0"), &journal, &artifacts, &mut fake).unwrap();
        assert_eq!(answer["dispatched"], "refused");
        assert_eq!(answer["reuse"], "disabled");
        assert_eq!(fake.runs, 0);

        // Turned on, the same request runs.
        plugins::set_enabled(&layout, &plugin.id, true).unwrap();
        let fresh = ask("0.1.0");
        let answer = use_plugin(&layout, &fresh, &journal, &artifacts, &mut fake).unwrap();
        assert_eq!(answer["dispatched"], "ran", "{answer}");
        assert_eq!(answer["reuse"], "admitted");
        assert_eq!(fake.runs, 1);
        assert_eq!(fake.dir, plugin.dir);
        assert!(
            answer["text"]
                .as_str()
                .unwrap()
                .starts_with("Action items for: Ana sends the budget by Friday.")
        );
        let output = answer["outputs"][0].as_str().unwrap();
        assert!(Path::new(output).starts_with(&artifacts));
        // The same request again follows the journal.
        let again = use_plugin(&layout, &fresh, &journal, &artifacts, &mut fake).unwrap();
        assert_eq!(again["dispatched"], "followed");
        assert_eq!(fake.runs, 1);

        // A newer version installed: the inspected pin is not resolved to
        // it, and nothing runs.
        install(&layout, "0.2.0");
        let newer = Use {
            request: "Ben books the room.",
            ..ask("0.1.0")
        };
        let answer = use_plugin(&layout, &newer, &journal, &artifacts, &mut fake).unwrap();
        assert_eq!(answer["dispatched"], "refused");
        assert_eq!(answer["reuse"], "changed");
        assert!(answer["text"].as_str().unwrap().contains("0.2.0"));
        assert_eq!(fake.runs, 1);
    }

    /// The catalog's explain-error plugin, installed and turned on under a
    /// scratch home, runs its real read-only workflow once through `use`.
    #[test]
    fn a_catalog_plugin_runs_its_real_workflow_through_the_shared_route() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-explain-error");
        crate::plugin_local::install_into(&layout, &source).unwrap();
        let plugin = plugins::find(&layout, "explain-error").unwrap();
        plugins::set_enabled(&layout, &plugin.id, true).unwrap();
        let pin = held_of(&plugin).pin;
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("src")).unwrap();
        std::fs::write(
            workspace.path().join("src/lib.rs"),
            "pub fn size(secret: &str) -> bool {\n    secret.len() >= \"8\"\n}\n",
        )
        .unwrap();
        let request = Use {
            plugin: "explain-error",
            pin: CapabilityPin {
                id: String::new(),
                ..pin
            },
            request: "error[E0308]: mismatched types\n --> src/lib.rs:2:22\n",
            workspace: workspace.path(),
            thread: "plugin-use",
        };
        let journal = Journal::at(home.path().join("routes"));
        let artifacts = home.path().join("route-artifacts");
        let answer = use_plugin(
            &layout,
            &request,
            &journal,
            &artifacts,
            &mut Workflow::default(),
        )
        .unwrap();
        assert_eq!(answer["dispatched"], "ran", "{answer}");
        assert!(
            answer["text"].as_str().unwrap().contains("E0308"),
            "{answer}"
        );
        let record = journal
            .latest("plugin-use", answer["request"].as_str().unwrap())
            .unwrap();
        assert_eq!(record.runs.len(), 1);
        assert_eq!(record.runs[0].projection.check, CheckLabel::Unchecked);
    }

    #[test]
    fn inspect_separates_results_for_this_release_from_others() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let plugin = install(&layout, "0.1.0");
        let digest = held_of(&plugin).pin.digest;
        let results = home.path().join("work/results");
        for (name, subject_digest) in [
            ("exact", digest.as_str().to_owned()),
            ("older", format!("sha256:{}", "0".repeat(64))),
        ] {
            let dir = results.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("report.json"),
                json!({
                    "v": ext_eval::report::REPORT_SCHEMA,
                    "verdict": "pass",
                    "ended_at": 10,
                    "subject": {"definition": {
                        "id": format!("{}:notes/notes", plugins::LOCAL_KEY),
                        "artifact": {"digest": subject_digest},
                    }},
                })
                .to_string(),
            )
            .unwrap();
        }
        let inspected = inspect(&layout, None, &[home.path().join("work")]).unwrap();
        let evidence = inspected["plugins"][0]["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 2);
        let exact: Vec<&Value> = evidence.iter().filter(|e| e["exact"] == true).collect();
        assert_eq!(exact.len(), 1);
        assert!(exact[0]["dir"].as_str().unwrap().ends_with("exact"));
        assert!(
            inspected["text"]
                .as_str()
                .unwrap()
                .contains("1 results for this release, 1 for others")
        );
    }
}
