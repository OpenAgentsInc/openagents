//! Persistent execution of the shared Kiln agent, without a browser or shell tool.
use crate::{
    Result, load, now,
    objectives::{self, ObjectiveRecord},
    save,
};
use kiln::{
    agent::{Agent, Command},
    patch, remote,
};
use kiln_git::{bridge, keys, relay, review};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub relay: String,
    pub authority: String,
    pub repository: String,
    pub runner: String,
    pub checkout: PathBuf,
    pub token_file: PathBuf,
    pub model: String,
    pub crates: BTreeSet<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub objective: String,
    pub objective_revision: Value,
    pub issue: u64,
    pub base: String,
    pub krate: String,
    pub text: String,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !hex(&self.authority, 64)
            || !hex(&self.runner, 64)
            || !self.relay.starts_with("wss://")
            || !self.repository.starts_with("30617:")
            || self.repository.split(':').count() != 3
            || !self.checkout.is_absolute()
            || !self.token_file.is_absolute()
            || self.model.is_empty()
            || self.crates.is_empty()
            || self.crates.len() > 64
            || self.crates.iter().any(|n| {
                n.is_empty()
                    || n.len() > 64
                    || !n.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                    })
            })
        {
            return Err("invalid native Kiln host configuration".into());
        }
        Ok(())
    }
}
impl Task {
    pub fn validate(&self, c: &Config) -> Result<()> {
        if !hex(&self.id, 64)
            || !hex(&self.base, 40)
            || self.issue == 0
            || self.issue == 12469
            || !c.crates.contains(&self.krate)
            || self.text.trim().is_empty()
            || self.text.len() > 12000
        {
            return Err("task is outside the admitted Kiln profile".into());
        }
        Ok(())
    }
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit())
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub task: Task,
    pub objective: ObjectiveRecord,
    pub agent: Agent,
    pub files: BTreeMap<String, String>,
    pub original: BTreeMap<String, String>,
    pub pending: Vec<Command>,
    pub phase: String,
    pub turns: usize,
    pub calls: usize,
    pub until: u64,
    pub checked: Option<String>,
    #[serde(default)]
    pub check: Option<remote::Outcome>,
    pub patch: Option<Event>,
    pub pull: Option<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub runner: Option<RunnerIntent>,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub results: Vec<(RunnerIntent, Event)>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct RunnerIntent {
    pub event: Event,
    pub command: Option<Command>,
    pub publication: bool,
    pub fingerprint: String,
    pub full_test: bool,
    pub runner: String,
    pub relay: String,
}
fn profile(c: &Config, key: &keys::Signer) -> String {
    kiln::sha256_hex(&serde_json::to_vec(&(crate::BUILD, key.key.pubkey(), c)).unwrap_or_default())
}
fn validate_objective(task: &Task, objective: &ObjectiveRecord) -> Result<()> {
    if task.objective != objective.objective.id || task.objective_revision != objective.revision {
        return Err("objective revision changed since the task was prepared".into());
    }
    if !objective
        .objective
        .source_paths
        .iter()
        .any(|p| format!("{}/", remote::crate_directory(&task.krate)).starts_with(p))
    {
        return Err("crate does not belong to the selected objective".into());
    }
    Ok(())
}
impl Job {
    pub fn summary(&self) -> Value {
        json!({"id":self.task.id,"objective":self.task.objective,"objective_revision":self.objective.revision,"issue":self.task.issue,"base":self.task.base,"crate":self.task.krate,"phase":self.phase,"turns":self.turns,"calls":self.calls,"until":self.until,"checked":self.checked.is_some(),"check":self.check.as_ref().map(|c|json!({"code":c.code,"summary":c.summary,"ms":c.ms,"event":c.check})),"patch":self.patch.as_ref().map(|p|&p.id),"pull":self.pull,"error":self.error,"latest":self.agent.steps.last().map(|s|json!({"thought":kiln::agent::clip(&s.thought,500),"observation":s.observations.last().map(|o|json!({"command":o.command,"code":o.code,"output":kiln::agent::clip(&o.output,2000)})),"problem":s.problem}))})
    }
}
pub fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr)
            .chars()
            .take(1000)
            .collect());
    }
    String::from_utf8(out.stdout).map_err(|_| "source is not UTF-8".into())
}
fn normalized(path: &str) -> Result<String> {
    if path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(|p| p == ".." || p == ".")
        || path.contains('\0')
    {
        return Err("path must be inside the repository".into());
    }
    Ok(path.trim_end_matches('/').to_string())
}
fn writable(task: &Task, path: &str) -> Result<()> {
    let p = normalized(path)?;
    if !p.starts_with(&format!("{}/", remote::crate_directory(&task.krate)))
        || review::path_problem(&p).is_some()
    {
        return Err("file is outside the admitted crate".into());
    }
    Ok(())
}
fn listing(c: &Config, j: &Job, p: &str) -> Result<Vec<String>> {
    let p = normalized(p)?;
    let s = git(
        &c.checkout,
        &["ls-tree", "-r", "--name-only", &j.task.base, "--", &p],
    )?;
    let mut names: BTreeSet<String> = s.lines().map(str::to_owned).collect();
    names.extend(
        j.files
            .keys()
            .filter(|f| p.is_empty() || *f == &p || f.starts_with(&format!("{p}/")))
            .cloned(),
    );
    if names.len() > 4000 {
        return Err("narrow the directory before reading it".into());
    }
    Ok(names.into_iter().collect())
}
fn read(c: &Config, j: &Job, p: &str) -> Result<String> {
    let p = normalized(p)?;
    if let Some(t) = j.files.get(&p) {
        return Ok(t.clone());
    }
    let t = git(&c.checkout, &["show", &format!("{}:{p}", j.task.base)])?;
    if t.len() > 512 * 1024 {
        return Err("file exceeds the read limit".into());
    }
    Ok(t)
}
fn fingerprint(j: &Job) -> String {
    kiln::sha256_hex(&serde_json::to_vec(&j.files).unwrap_or_default())
}
fn check_diff(j: &Job) -> Result<String> {
    let changes = patch::changes(&j.original, &j.files);
    let text = patch::format_patch(
        &patch::Header {
            author: "Kiln",
            email: "kiln@openagents.com",
            subject: "Check proposed change",
            body: "",
            date: 0,
        },
        &changes,
    );
    if changes.is_empty() || text.len() > remote::MAX_FILE_BYTES {
        return Err("there is no bounded code change to check".into());
    }
    Ok(text)
}
fn edit(c: &Config, j: &mut Job, p: &str, text: String) -> Result<String> {
    writable(&j.task, p)?;
    if text.len() > 512 * 1024 {
        return Err("file exceeds runner's byte bound".into());
    }
    if j.files.values().map(String::len).sum::<usize>() - j.files.get(p).map_or(0, String::len)
        + text.len()
        > 2 * 1024 * 1024
        || (!j.files.contains_key(p) && j.files.len() >= 32)
    {
        return Err("changed files exceed runner's byte bound".into());
    }
    if !j.original.contains_key(p) {
        let original = if listing(c, j, p)?.contains(&p.to_string()) {
            read(c, j, p)?
        } else {
            String::new()
        };
        if j.original.values().map(String::len).sum::<usize>() + original.len() > 2 * 1024 * 1024 {
            return Err("original file retention limit exceeded".into());
        }
        j.original.insert(p.into(), original);
    }
    j.files.insert(p.into(), text);
    j.checked = None;
    j.check = None;
    j.patch = None;
    Ok(format!("updated {p}"))
}
fn tool(c: &Config, j: &Job, line: &str) -> Result<String> {
    let a = kiln::agent::split(line)?;
    let arg = |i| a.get(i).map(String::as_str).ok_or("tool argument missing");
    match arg(0)? {
        "ls" => Ok(listing(
            c,
            j,
            a.last()
                .filter(|s| *s != "ls" && *s != "-R")
                .map_or("", String::as_str),
        )?
        .join("\n")),
        "view" => {
            let path = arg(1)?;
            let start: usize = arg(2)?.parse().map_err(|_| "line number")?;
            let end: usize = arg(3)?.parse().map_err(|_| "line number")?;
            if start == 0 || end < start || end - start >= 80 {
                return Err("view accepts at most 80 lines".into());
            }
            Ok(read(c, j, path)?
                .lines()
                .enumerate()
                .skip(start - 1)
                .take(end - start + 1)
                .map(|(n, l)| format!("{}:{l}", n + 1))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "grep" => {
            let insensitive = a.get(1).is_some_and(|s| s == "-i");
            let i = if insensitive { 2 } else { 1 };
            let pattern = arg(i)?;
            let p = a.get(i + 1).map_or("", String::as_str);
            let mut out = String::new();
            for name in listing(c, j, p)? {
                let text = read(c, j, &name)?;
                for (n, line) in text.lines().enumerate() {
                    let found = pattern.split("\\|").any(|term| {
                        if insensitive {
                            line.to_lowercase().contains(&term.to_lowercase())
                        } else {
                            line.contains(term)
                        }
                    });
                    if found {
                        out.push_str(&format!("{name}:{}:{line}\n", n + 1));
                        if out.len() > 8000 {
                            return Ok(kiln::agent::clip(&out, 8000));
                        }
                    }
                }
            }
            Ok(out)
        }
        _ => Err("available tools: ls, view, grep, cargo, write, patch".into()),
    }
}
pub async fn submit(root: &Path, c: &Config, key: &keys::Signer, task: Task) -> Result<Job> {
    task.validate(c)?;
    let lock = std::fs::File::create(root.join("submit.lock")).map_err(|e| e.to_string())?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|e| e.to_string())?;
    let path = root.join("jobs").join(format!("{}.json", task.id));
    if path.exists() {
        let old: Job = load(&path)?;
        if serde_json::to_value(&old.task).ok() != serde_json::to_value(&task).ok() {
            return Err("task ID already has different inputs".into());
        }
        return Ok(old);
    }
    let jobs = root.join("jobs");
    std::fs::create_dir_all(&jobs).map_err(|e| e.to_string())?;
    let mut count = 0;
    let mut today = 0;
    for entry in std::fs::read_dir(&jobs).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let old: Job = load(&p)?;
        count += 1;
        if old.until.saturating_sub(7200) / 86400 == now() / 86400 {
            today += 1;
        }
        if !matches!(old.phase.as_str(), "completed" | "failed" | "stopped") {
            return Err("another native Kiln attempt is active or unresolved".into());
        }
        if old.task.issue == task.issue && (old.phase == "completed" || old.pull.is_some()) {
            return Err(
                "this issue already has a native attempt; inspect its result before rework".into(),
            );
        }
    }
    if count >= 512 || today >= 5 {
        return Err("native Kiln attempt limit reached".into());
    }
    let objective = objectives::consult(key, &c.authority, &c.relay, &task.objective).await?;
    validate_objective(&task, &objective)?;
    git(&c.checkout, &["fetch", "--quiet", "origin", "main"])?;
    git(
        &c.checkout,
        &["merge-base", "--is-ancestor", &task.base, "origin/main"],
    )?;
    git(
        &c.checkout,
        &["cat-file", "-e", &format!("{}^{{commit}}", task.base)],
    )?;
    let mut agent = Agent::new(
        &format!(
            "{}\n\nIssue #{}. Objective: {}\n{}",
            task.text, task.issue, objective.objective.title, objective.objective.intent
        ),
        11000,
    );
    agent.instructions = kiln::agent::RUST_INSTRUCTIONS
        .replace("inside a web browser", "in a persistent native host")
        .replace("{crates}", &task.krate);
    agent.instructions.push_str("\nYou cannot run a shell or access host files. To propose a change use {\"publish\":\"short title\",\"body\":\"why and checks\"}. A complete unfiltered cargo test is required for the exact edited files. Do not stop after merely investigating. Never alter manifests, build scripts, or other crates. Use view in small ranges.\n");
    let job = Job {
        task,
        objective,
        agent,
        files: BTreeMap::new(),
        original: BTreeMap::new(),
        pending: vec![],
        phase: "queued".into(),
        turns: 0,
        calls: 0,
        until: now() + 7200,
        checked: None,
        check: None,
        patch: None,
        pull: None,
        error: None,
        runner: None,
        profile: profile(c, key),
        results: vec![],
    };
    save(&path, &job)?;
    Ok(job)
}

pub async fn advance(root: &Path, c: &Config, key: &keys::Signer, path: &Path) -> Result<Job> {
    let mut j: Job = load(path)?;
    if let Some(intent) = j.runner.clone() {
        match relay::runner_result(&intent.relay, key, &intent.runner, &intent.event).await? {
            Some((outcome, result)) => {
                finish_runner(&mut j, &intent, outcome, result)?;
                save(path, &j)?;
                return Ok(j);
            }
            None => {
                let admitted = now() < j.until
                    && !root
                        .join("jobs")
                        .join(format!("{}.cancel", j.task.id))
                        .exists()
                    && j.profile == profile(c, key);
                if admitted {
                    if let Ok(current) =
                        objectives::consult(key, &c.authority, &c.relay, &j.task.objective).await
                    {
                        if current.revision == j.objective.revision {
                            relay::publish(&intent.relay, &key.secret, &intent.event).await?;
                        }
                    }
                }
                j.error = Some(format!(
                    "Awaiting the original runner result: {}",
                    intent.event.id
                ));
                if now() >= j.until {
                    j.phase = "unknown".into();
                }
                save(path, &j)?;
                tokio::time::sleep(Duration::from_secs(20)).await;
                return Ok(j);
            }
        }
    }
    if j.profile != profile(c, key) {
        return Err("execution profile changed; inspect and readmit the task".into());
    }
    if matches!(j.phase.as_str(), "completed" | "failed" | "stopped") {
        return Ok(j);
    }
    if j.phase == "checking" || j.phase == "unknown" {
        return Err("runner result is unknown after interruption; inspect before retrying".into());
    }
    if j.phase == "bridging" {
        return deliver(root, c, key, path, j).await;
    }
    if now() >= j.until
        || j.turns >= 48
        || j.calls >= 96
        || root
            .join("jobs")
            .join(format!("{}.cancel", j.task.id))
            .exists()
    {
        j.phase = "stopped".into();
        save(path, &j)?;
        return Ok(j);
    }
    let current = objectives::consult(key, &c.authority, &c.relay, &j.task.objective).await?;
    if current.revision != j.objective.revision {
        return Err("objective changed; submit a separately reviewed task".into());
    }
    if j.phase == "publishing" {
        return deliver(root, c, key, path, j).await;
    }
    if j.pending.is_empty() {
        j.turns += 1;
        j.phase = "model".into();
        save(path, &j)?;
        let files = listing(c, &j, &remote::crate_directory(&j.task.krate))?.join("\n");
        let ask = pylon::client::Ask {
            relay: c.relay.clone(),
            pylon: None,
            prompt: j.agent.prompt(&files),
            wait: Duration::from_secs(240),
            publish_receipt: false,
            home: root.join("receipts"),
            checkers: Default::default(),
            pay: None,
        };
        let buyer = pylon::identity::Identity::from_secret(key.secret)?;
        let answered = pylon::client::ask_model(
            &buyer,
            &ask,
            &pylon::group::Want::Model(c.model.clone()),
            &[],
            Some(&j.agent.instructions),
        )
        .await;
        let reply = match answered {
            Ok(answer) if answer.text.is_some() => answer.text.unwrap_or_default(),
            result => {
                let error = match result {
                    Ok(answer) => format!("{:?}", answer.error),
                    Err(e) => e,
                };
                if error.to_lowercase().contains("prompt") || error.to_lowercase().contains("input")
                {
                    j.agent.budget = (j.agent.budget * 3 / 4).max(4000);
                }
                j.agent.steps.push(kiln::agent::Step {
                    problem: Some(kiln::agent::clip(&error, 1000)),
                    ..Default::default()
                });
                j.phase = "tools".into();
                save(path, &j)?;
                tokio::time::sleep(Duration::from_secs(10)).await;
                return Ok(j);
            }
        };
        let (_, reply) = kiln::transcript::split_thinking(&reply);
        j.pending = j.agent.plan(&reply);
        j.phase = "tools".into();
        save(path, &j)?;
        return Ok(j);
    }
    let command = j.pending.remove(0);
    j.calls += 1;
    let result = match &command {
        Command::Write { path: p, content } => edit(c, &mut j, p, content.clone()),
        Command::Patch {
            path: p,
            search,
            replace,
        } => match read(c, &j, p) {
            Err(e) => Err(e),
            Ok(old) => {
                if search.is_empty() || old.matches(search).count() != 1 {
                    Err("search text must match exactly once".into())
                } else {
                    edit(c, &mut j, p, old.replacen(search, replace, 1))
                }
            }
        },
        Command::Run(line) if line.starts_with("cargo ") => {
            let validation = (|| -> Result<()> {
                remote::parse_command(
                    &kiln::agent::split(line)?,
                    std::slice::from_ref(&j.task.krate),
                )?;
                if !j.files.is_empty() {
                    check_diff(&j)?;
                }
                Ok(())
            })();
            if let Err(e) = validation {
                j.agent.observe(&command, 1, e);
                save(path, &j)?;
                return Ok(j);
            }
            let args = remote::parse_command(
                &kiln::agent::split(line)?,
                std::slice::from_ref(&j.task.krate),
            )?;
            let job = remote::Job {
                v: remote::NATIVE_JOB_V.into(),
                base: j.task.base.clone(),
                args: args.clone(),
                files: BTreeMap::new(),
                deleted: vec![],
                patch: None,
                action: None,
                diff: if j.files.is_empty() {
                    None
                } else {
                    Some(check_diff(&j)?)
                },
            };
            job.check(std::slice::from_ref(&j.task.krate))?;
            j.checked = None;
            let event = relay::runner_request(
                key,
                &c.runner,
                &job,
                &[nostr::domain::Tag::new(vec![
                    "expiration".into(),
                    j.until.to_string(),
                ])],
            )?;
            let intent = RunnerIntent {
                event,
                command: Some(command.clone()),
                publication: false,
                fingerprint: fingerprint(&j),
                full_test: args == ["test", "-p", j.task.krate.as_str()],
                runner: c.runner.clone(),
                relay: c.relay.clone(),
            };
            j.runner = Some(intent.clone());
            j.phase = "checking".into();
            save(path, &j)?;
            let (outcome, result) = relay::ask_runner_event(
                &c.relay,
                key,
                &c.runner,
                &intent.event,
                Duration::from_secs(1250),
                &|s| eprintln!("runner: {s}"),
            )
            .await?;
            finish_runner(&mut j, &intent, outcome, result)?;
            save(path, &j)?;
            return Ok(j);
        }
        Command::Run(line) => tool(c, &j, line),
        Command::Publish { subject, body } => {
            if let Err(e) = prepare_patch(c, key, &mut j, subject, body) {
                j.agent.observe(&command, 1, e);
                save(path, &j)?;
                return Ok(j);
            }
            j.phase = "publishing".into();
            save(path, &j)?;
            return Ok(j);
        }
        Command::Done(summary) => {
            // Completion is a proposal only: the host still requires exact tests and a patch.
            j.agent.done = None;
            if j.checked == Some(fingerprint(&j)) && !j.files.is_empty() {
                let subject = format!("fix({}): improve issue #{}", j.task.krate, j.task.issue);
                if let Err(e) = prepare_patch(c, key, &mut j, &subject, summary) {
                    j.agent.steps.push(kiln::agent::Step {
                        problem: Some(e),
                        ..Default::default()
                    });
                    save(path, &j)?;
                    return Ok(j);
                }
                j.phase = "publishing".into();
                save(path, &j)?;
                return Ok(j);
            }
            Err("run the full crate tests on a real change, then publish before saying done".into())
        }
    };
    let (code, text) = match result {
        Ok(t) => (0, t),
        Err(e) => (1, e),
    };
    if matches!(command, Command::Done(_)) {
        j.agent.steps.push(kiln::agent::Step {
            problem: Some(text),
            ..Default::default()
        });
    } else {
        j.agent
            .observe(&command, code, kiln::agent::clip(&text, 8000));
    }
    save(path, &j)?;
    Ok(j)
}
fn finish_runner(
    j: &mut Job,
    intent: &RunnerIntent,
    outcome: remote::Outcome,
    result: Event,
) -> Result<()> {
    if j.runner.as_ref() != Some(intent) {
        return Err("runner intent was already settled or differs".into());
    }
    if !kiln::cj::answers(
        &result,
        &intent.event.id,
        &intent.runner,
        &intent.event.pubkey,
    ) {
        return Err("runner result declaration differs".into());
    }
    if intent.fingerprint != fingerprint(j) || outcome.v != remote::RESULT_V {
        return Err("runner result differs from the retained inputs".into());
    }
    j.results.push((intent.clone(), result));
    if intent.publication {
        if outcome.code != Some(0) || outcome.check.is_none() {
            j.runner = None;
            j.check = Some(outcome);
            j.phase = "failed".into();
            j.error = Some("The published patch did not pass its runner check".into());
            return Ok(());
        }
        j.phase = "bridging".into();
    } else {
        let command = intent.command.as_ref().ok_or("runner command is missing")?;
        if outcome.code == Some(0) && intent.full_test {
            j.checked = Some(intent.fingerprint.clone());
        }
        j.agent.observe(
            command,
            if outcome.code == Some(0) { 0 } else { 1 },
            kiln::agent::clip(&format!("{}\n{}", outcome.summary, outcome.log), 8000),
        );
        j.phase = "tools".into();
    }
    j.error = None;
    j.check = Some(outcome);
    j.runner = None;
    Ok(())
}
fn prepare_patch(
    c: &Config,
    key: &keys::Signer,
    j: &mut Job,
    subject: &str,
    body: &str,
) -> Result<()> {
    if j.files.is_empty() || j.checked != Some(fingerprint(j)) {
        return Err("the exact change needs a passing full crate test".into());
    }
    let changes = patch::changes(&j.original, &j.files);
    if changes.is_empty() {
        return Err("there is no change to propose".into());
    }
    let body = format!(
        "{body}\n\nRefs #{}.\nForeman objective: {}",
        j.task.issue, j.task.objective
    );
    let text = patch::format_patch(
        &patch::Header {
            author: "Kiln",
            email: "kiln@openagents.com",
            subject,
            body: &body,
            date: now(),
        },
        &changes,
    );
    let private = [
        &j.objective.revision["work"]["scope"],
        &j.objective.revision["work"]["item"],
        &j.objective.revision["artifact"]["digest"],
    ]
    .into_iter()
    .filter_map(Value::as_str)
    .any(|id| text.contains(id));
    if private || text.len() > patch::MAX_PATCH_BYTES || !patch::secret_findings(&text).is_empty() {
        return Err("patch exceeds the publication policy".into());
    }
    let owner = c.repository.split(':').nth(1).ok_or("repository address")?;
    j.patch = Some(key.key.sign(
        patch::PATCH_KIND,
        patch::patch_tags(
            &c.repository,
            owner,
            &j.task.base,
            std::slice::from_ref(&j.task.krate),
        ),
        text,
        now(),
    ));
    Ok(())
}
async fn deliver(
    root: &Path,
    c: &Config,
    key: &keys::Signer,
    path: &Path,
    mut j: Job,
) -> Result<Job> {
    let event = j.patch.clone().ok_or("saved patch missing")?;
    if j.phase == "publishing" {
        relay::publish(&c.relay, &key.secret, &event).await?;
        let job = remote::Job {
            v: remote::NATIVE_JOB_V.into(),
            base: j.task.base.clone(),
            args: vec!["test".into(), "-p".into(), j.task.krate.clone()],
            files: BTreeMap::new(),
            deleted: vec![],
            patch: Some(event.clone()),
            action: None,
            diff: None,
        };
        let request = relay::runner_request(
            key,
            &c.runner,
            &job,
            &[nostr::domain::Tag::new(vec![
                "expiration".into(),
                j.until.to_string(),
            ])],
        )?;
        let intent = RunnerIntent {
            event: request,
            command: None,
            publication: true,
            fingerprint: fingerprint(&j),
            full_test: true,
            runner: c.runner.clone(),
            relay: c.relay.clone(),
        };
        j.runner = Some(intent.clone());
        j.phase = "checking".into();
        save(path, &j)?;
        let (outcome, result) = relay::ask_runner_event(
            &c.relay,
            key,
            &c.runner,
            &intent.event,
            Duration::from_secs(1250),
            &|s| eprintln!("patch check: {s}"),
        )
        .await?;
        finish_runner(&mut j, &intent, outcome, result)?;
        save(path, &j)?;
        if j.phase == "failed" {
            return Ok(j);
        }
    }
    let read = review::fetch(&c.relay, &event.id, Some(&c.repository)).await?;
    read.ready(false, true)?;
    let hub = bridge::GitHub::new(
        "https://api.github.com",
        "OpenAgentsInc/openagents",
        &c.token_file,
    );
    if now() >= j.until
        || root
            .join("jobs")
            .join(format!("{}.cancel", j.task.id))
            .exists()
    {
        if let Some(found) = hub.for_patch(&event).await? {
            j.pull = Some(
                json!({"url":found.url,"number":found.number,"observed_head":found.head,"verification":"not_checked"}),
            );
            j.checked = None;
            j.phase = "stopped".into();
            j.error = Some("Stopped; an existing draft PR needs review".into());
            save(path, &j)?;
            return Ok(j);
        }
        return Err("PR delivery remains unresolved after its deadline or cancellation".into());
    }
    let config = bridge::Config {
        relay: c.relay.clone(),
        address: c.repository.clone(),
        github: "OpenAgentsInc/openagents".into(),
        api: "https://api.github.com".into(),
        token_file: c.token_file.clone(),
        checkout: c.checkout.clone(),
        state: root.join("bridge"),
        openers: None,
        key: keys::Signer {
            key: key.key.clone(),
            secret: key.secret,
        },
    };
    let tracked = bridge::open(&config, &hub, &read, key.key.pubkey()).await?;
    j.pull = Some(serde_json::to_value(tracked).map_err(|e| e.to_string())?);
    j.phase = "completed".into();
    save(path, &j)?;
    Ok(j)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_cannot_escape_or_change_build_inputs() {
        let t = Task {
            id: "11".repeat(32),
            objective: "improve-kiln".into(),
            objective_revision: Value::Null,
            issue: 12497,
            base: "22".repeat(20),
            krate: "kiln".into(),
            text: "fix".into(),
        };
        assert!(writable(&t, "crates/kiln/src/agent.rs").is_ok());
        for p in [
            "crates/kiln/Cargo.toml",
            "crates/kiln/build.rs",
            "crates/kiln/../nostr/src/lib.rs",
            "/etc/passwd",
            "crates/nostr/src/lib.rs",
        ] {
            assert!(writable(&t, p).is_err(), "{p}");
        }
    }
    #[test]
    fn a_restart_retains_the_loop_and_a_changed_file_invalidates_its_check() {
        let temp = tempfile::tempdir().unwrap();
        let key = keys::Signer {
            secret: secp256k1::SecretKey::from_byte_array([1; 32]).unwrap(),
            key: kiln::cj::Key::from_bytes([1; 32]).unwrap(),
        };
        let catalog =
            grid_foreman::objectives::Catalog::parse(grid_foreman::objectives::CATALOG.as_bytes())
                .unwrap();
        let objective = objectives::import(&temp.path().join("objectives"), &key, &catalog, 100)
            .unwrap()
            .remove(1);
        let config = Config {
            relay: "wss://relay.example".into(),
            authority: key.key.pubkey().into(),
            repository: format!("30617:{}:repo", key.key.pubkey()),
            runner: key.key.pubkey().into(),
            checkout: temp.path().into(),
            token_file: temp.path().join("token"),
            model: "fixture".into(),
            crates: ["kiln".into()].into(),
        };
        let task = Task {
            id: "11".repeat(32),
            objective: "improve-kiln".into(),
            objective_revision: objective.revision.clone(),
            issue: 123,
            base: "22".repeat(20),
            krate: "kiln".into(),
            text: "A bounded fix".into(),
        };
        assert!(validate_objective(&task, &objective).is_ok());
        let mut newer: ObjectiveRecord =
            serde_json::from_value(serde_json::to_value(&objective).unwrap()).unwrap();
        newer.revision = json!({"changed": true});
        assert!(validate_objective(&task, &newer).is_err());
        let p = "crates/kiln/src/lib.rs";
        let mut job = Job {
            task,
            objective,
            agent: Agent::new("test", 8000),
            files: [(p.into(), "new\n".into())].into(),
            original: [(p.into(), "old\n".into())].into(),
            pending: vec![Command::Run("view crates/kiln/src/lib.rs 1 1".into())],
            phase: "tools".into(),
            turns: 1,
            calls: 1,
            until: 1000,
            checked: None,
            check: None,
            patch: None,
            pull: None,
            error: None,
            runner: None,
            profile: profile(&config, &key),
            results: vec![],
        };
        assert!(prepare_patch(&config, &key, &mut job, "Fix", "why").is_err());
        job.checked = Some(fingerprint(&job));
        let path = temp.path().join("job.json");
        save(&path, &job).unwrap();
        let mut resumed: Job = load(&path).unwrap();
        assert_eq!(resumed.pending, job.pending);
        assert_eq!(resumed.agent.task, "test");
        let request = relay::runner_request(
            &key,
            key.key.pubkey(),
            &remote::Job {
                v: remote::NATIVE_JOB_V.into(),
                base: resumed.task.base.clone(),
                args: vec!["test".into(), "-p".into(), "kiln".into()],
                files: BTreeMap::new(),
                deleted: vec![],
                patch: None,
                action: None,
                diff: None,
            },
            &[],
        )
        .unwrap();
        let intent = RunnerIntent {
            event: request.clone(),
            command: Some(Command::Run("cargo test -p kiln".into())),
            publication: false,
            fingerprint: fingerprint(&resumed),
            full_test: true,
            runner: key.key.pubkey().into(),
            relay: config.relay.clone(),
        };
        resumed.runner = Some(intent.clone());
        save(&path, &resumed).unwrap();
        resumed = load(&path).unwrap();
        let outcome = remote::Outcome {
            v: remote::RESULT_V.into(),
            code: Some(0),
            summary: "passed".into(),
            log: String::new(),
            published: None,
            ms: 1,
            check: None,
        };
        let result = key
            .key
            .seal(
                key.key.pubkey(),
                kiln::cj::RESULT_KIND,
                vec![
                    nostr::domain::Tag::new(vec!["p".into(), key.key.pubkey().into()]),
                    nostr::domain::Tag::new(vec!["e".into(), request.id]),
                ],
                &json!({"v":1,"requires":[],"type":"result","text":"passed"}),
                [2; 32],
                102,
            )
            .unwrap();
        finish_runner(&mut resumed, &intent, outcome.clone(), result.clone()).unwrap();
        assert_eq!(resumed.results.len(), 1);
        assert!(finish_runner(&mut resumed, &intent, outcome, result).is_err());
        prepare_patch(&config, &key, &mut resumed, "Fix", "why").unwrap();
        resumed.patch.as_ref().unwrap().validate_crypto().unwrap();
        edit(&config, &mut resumed, p, "different\n".into()).unwrap();
        assert!(resumed.checked.is_none());
        assert!(prepare_patch(&config, &key, &mut resumed, "Fix", "why").is_err());
    }
}
