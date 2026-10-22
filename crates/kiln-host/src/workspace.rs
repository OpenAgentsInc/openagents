//! A bounded workspace adapter for the shared Kiln agent. Checks stay with the caller.
use crate::{Result, load, now, save, worker::git};
use kiln::agent::{Agent, Command, Observation, Step, clip, split};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_TURNS: usize = 96;
const MAX_GUIDANCE_BYTES: usize = 2000;
const MAX_INSTRUCTION_BYTES: usize = INSTRUCTIONS.len() + 4000;
const MAX_VIEW_LINES: usize = 80;
const MAX_VIEW_BYTES: usize = 1400;

const INSTRUCTIONS: &str = r#"You are Kiln, editing a bounded repository workspace. Reply only with one JSON object:
{"thought":"one short sentence","commands":[...]}
Commands: {"run":"ls PATH"}, {"run":"view PATH START END"}, {"run":"grep [-n] [-i] TEXT PATH"}, {"run":"copy SOURCE DEST"}, {"write":"PATH","content":"full text"}, {"patch":"PATH","search":"exact unique old text","replace":"new text"}, {"run":"check"}, {"done":"summary"}.
grep matches literal text; separate alternatives with \|. -i ignores case, and -n includes the line numbers that grep always shows. Other regular expressions and shell commands are unavailable.
copy duplicates an existing scoped text file in the private workspace without printing or regenerating it.
Read before editing. Use small exact patches, preserve existing style, and write meaningful regression tests. View at most 80 lines at a time. No shell or arbitrary programs are available. check asks the coordinator to run the task's real checks outside this workspace. Read failures, repair, and check again. Say done only after check passed for your current files. Never change paths outside the supplied scope. Your edits remain private until a reviewer accepts them."#;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub repo: PathBuf,
    pub base: String,
    pub paths: Vec<String>,
    pub key: PathBuf,
    pub relay: String,
    pub model: String,
    pub profile: String,
    pub pylon: String,
    pub task: String,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !self.repo.is_absolute()
            || !self.key.is_absolute()
            || self.base.len() != 40
            || !self.base.bytes().all(|b| b.is_ascii_hexdigit())
            || self.paths.is_empty()
            || self.paths.len() > 8
            || self.paths.iter().any(|p| normalized(p).is_err())
            || !self.relay.starts_with("wss://")
            || self.model.is_empty()
            || !nostr::pylon::GROUP_PROFILES.contains(&self.profile.as_str())
            || self.pylon.parse::<secp256k1::XOnlyPublicKey>().is_err()
            || self.task.trim().is_empty()
            || self.task.len() > 12000
        {
            return Err("invalid workspace configuration".into());
        }
        Ok(())
    }
    fn allows(&self, path: &str) -> bool {
        normalized(path).is_ok()
            && self
                .paths
                .iter()
                .any(|p| path == p || path.starts_with(&format!("{p}/")))
    }
}
fn normalized(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 1024
        || path.split('/').count() > 32
        || path.starts_with('/')
        || path.contains(['\\', '\0', ':'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p == ".git")
    {
        return Err("path must name a file or directory inside the configured scope".into());
    }
    Ok(())
}

fn view_page(text: &str, start: usize, end: usize) -> String {
    let total = text.lines().count();
    if start > total {
        return format!("EOF: {total} lines; start {start} is past the end.");
    }
    let next = |last| {
        if last < total {
            format!("Next line: {}.", last + 1)
        } else {
            "EOF.".into()
        }
    };
    let header = |last| format!("Lines {start}-{last} of {total}. {}\n", next(last));
    let end = end.min(start.saturating_add(MAX_VIEW_LINES - 1)).min(total);
    let mut body = String::new();
    let mut last = None;
    for (n, line) in text
        .lines()
        .enumerate()
        .skip(start - 1)
        .take(end - start + 1)
    {
        let numbered = format!("{}:{line}", n + 1);
        let separator = usize::from(!body.is_empty());
        if header(n + 1).len() + body.len() + separator + numbered.len() > MAX_VIEW_BYTES {
            break;
        }
        if separator != 0 {
            body.push('\n');
        }
        body.push_str(&numbered);
        last = Some(n + 1);
    }
    match last {
        Some(last) => format!("{}{body}", header(last)),
        None => format!(
            "Line {start} is too long for a {MAX_VIEW_BYTES}-byte page; no source lines shown. {}",
            next(start)
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerEvidence {
    pub turn: usize,
    pub request: Option<String>,
    pub pylon: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub lossy: Option<bool>,
    pub outcome: String,
    pub error: bool,
    pub error_sha256: Option<String>,
    pub usage: Option<Value>,
    pub timing: AnswerTiming,
    pub prompt_sha256: String,
    pub reply_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerTiming {
    pub elapsed_ms: u64,
    pub discover_ms: Option<u64>,
    pub contact_ms: Option<u64>,
    pub answer_ms: Option<u64>,
}

impl AnswerEvidence {
    fn capture(
        turn: usize,
        prompt: &str,
        result: &Result<pylon::client::Answer>,
        elapsed_ms: u64,
    ) -> Self {
        let answer = result.as_ref().ok();
        let reply = answer.and_then(|a| a.text.as_deref());
        let error = match result {
            Ok(answer) => answer.error.as_deref(),
            Err(error) => Some(error.as_str()),
        };
        Self {
            turn,
            request: answer.map(|a| clip(&a.request, 128)),
            pylon: answer.map(|a| clip(&a.pylon, 512)),
            model: answer.map(|a| clip(&a.model, 512)),
            profile: answer.map(|a| clip(&a.profile, 64)),
            lossy: answer.map(|a| a.lossy),
            outcome: answer.map_or_else(|| "error".into(), |a| clip(&a.outcome, 64)),
            error: error.is_some(),
            error_sha256: error.map(|text| kiln::sha256_hex(text.as_bytes())),
            usage: answer
                .and_then(|a| a.usage.as_ref())
                .and_then(usage_counters),
            timing: AnswerTiming {
                elapsed_ms,
                discover_ms: answer.map(|a| a.discover_ms),
                contact_ms: answer.and_then(|a| a.contact_ms),
                answer_ms: answer.and_then(|a| a.answer_ms),
            },
            prompt_sha256: kiln::sha256_hex(answer.map_or(prompt, |a| &a.prompt).as_bytes()),
            reply_sha256: reply.map(|text| kiln::sha256_hex(text.as_bytes())),
        }
    }
}

fn usage_counters(usage: &Value) -> Option<Value> {
    let mut counters = serde_json::Map::new();
    for name in [
        "input",
        "output",
        "total",
        "input_tokens",
        "output_tokens",
        "total_tokens",
        "prompt_tokens",
        "completion_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ] {
        if let Some(count) = usage[name].as_u64() {
            counters.insert(name.into(), count.into());
        }
    }
    (!counters.is_empty()).then(|| counters.into())
}

#[derive(Serialize, Deserialize)]
pub struct Workspace {
    pub config: Config,
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
    pub answers: Vec<AnswerEvidence>,
}
impl Workspace {
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;
        if git(
            &config.repo,
            &["rev-parse", &format!("{}^{{commit}}", config.base)],
        )?
        .trim()
            != config.base
        {
            return Err("base commit was not found".into());
        }
        let mut agent = Agent::new(&config.task, 12000);
        agent.instructions = INSTRUCTIONS.into();
        Ok(Self {
            config,
            agent,
            files: BTreeMap::new(),
            original: BTreeMap::new(),
            pending: vec![],
            phase: "model".into(),
            turns: 0,
            calls: 0,
            until: now() + 21600,
            checked: None,
            answers: vec![],
        })
    }
    fn record_answer(
        &mut self,
        prompt: &str,
        result: &Result<pylon::client::Answer>,
        elapsed_ms: u64,
    ) {
        let evidence = AnswerEvidence::capture(self.turns, prompt, result, elapsed_ms);
        self.answers.drain(
            ..self
                .answers
                .len()
                .saturating_add(1)
                .saturating_sub(MAX_TURNS),
        );
        self.answers.push(evidence);
    }
    fn names(&self, path: &str) -> Result<Vec<String>> {
        if !self.config.allows(path) {
            return Err("directory is outside the configured scope".into());
        }
        let found = git(
            &self.config.repo,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                &self.config.base,
                "--",
                path,
            ],
        )?;
        let mut names: Vec<_> = found
            .lines()
            .map(str::to_owned)
            .chain(
                self.files
                    .keys()
                    .filter(|p| *p == path || p.starts_with(&format!("{path}/")))
                    .cloned(),
            )
            .collect();
        names.sort();
        names.dedup();
        if names.len() > 2000 {
            return Err("narrow the directory".into());
        }
        Ok(names)
    }
    fn read(&self, path: &str) -> Result<String> {
        if !self.config.allows(path) {
            return Err("file is outside the configured scope".into());
        }
        if let Some(text) = self.files.get(path) {
            return Ok(text.clone());
        }
        // Read Git objects, never the host's working files or symlink targets.
        let entry = git(
            &self.config.repo,
            &["ls-tree", &self.config.base, "--", path],
        )?;
        if !entry.starts_with("100644 blob ") && !entry.starts_with("100755 blob ") {
            return Err("file is not a tracked regular text file".into());
        }
        let text = git(
            &self.config.repo,
            &["show", &format!("{}:{path}", self.config.base)],
        )?;
        if text.len() > 250_000 || text.contains('\0') {
            return Err("file exceeds the text limit".into());
        }
        Ok(text)
    }
    fn edit(&mut self, path: &str, text: String) -> Result<String> {
        if !self.config.allows(path)
            || text.len() > 250_000
            || text.contains('\0')
            || path.split('/').any(|p| p.starts_with(".env"))
            || !kiln::patch::secret_findings(&text).is_empty()
        {
            return Err("edit is outside the configured text-file scope".into());
        }
        if self
            .files
            .keys()
            .any(|p| p.starts_with(&format!("{path}/")))
        {
            return Err("a directory containing proposed files cannot become a file".into());
        }
        let mut ancestor = path;
        while let Some((parent, _)) = ancestor.rsplit_once('/') {
            if self.files.contains_key(parent) {
                return Err("a file cannot contain another file".into());
            }
            let entry = git(
                &self.config.repo,
                &["ls-tree", &self.config.base, "--", parent],
            )?;
            if !entry.is_empty() && !entry.starts_with("040000 tree ") {
                return Err("edit has a non-directory ancestor at the pinned base".into());
            }
            ancestor = parent;
        }
        if !self.original.contains_key(path) {
            let entries = git(
                &self.config.repo,
                &["ls-tree", &self.config.base, "--", path],
            )?;
            if !entries.is_empty() {
                let original = self.read(path)?;
                if self.original.values().map(String::len).sum::<usize>() + original.len()
                    > 2_000_000
                {
                    return Err("original files exceed the workspace limit".into());
                }
                self.original.insert(path.into(), original);
            }
        }
        let next_size = self
            .files
            .iter()
            .filter(|(p, _)| p.as_str() != path)
            .map(|(_, text)| text.len())
            .sum::<usize>()
            + text.len();
        if next_size > 2_000_000 {
            return Err("workspace exceeds the edit limit".into());
        }
        self.files.insert(path.into(), text);
        self.checked = None;
        Ok("Saved in the private workspace.".into())
    }
    pub fn digest(&self) -> String {
        kiln::sha256_hex(&serde_json::to_vec(&(&self.config.base, &self.files)).unwrap_or_default())
    }
    pub fn summary(&self) -> Value {
        json!({"phase":self.phase,"turns":self.turns,"calls":self.calls,"base":self.config.base,"pylon":self.config.pylon,"model":self.config.model,"profile":self.config.profile,"files":self.files.keys().collect::<Vec<_>>(),"digest":self.digest(),"checked":self.checked,"done":self.agent.done,"latest":self.agent.steps.last(),"answers":self.answers})
    }
    fn grep(&self, mut args: &[String]) -> Result<String> {
        let mut ignore_case = false;
        while let Some(option) = args.first() {
            if option == "--" {
                args = &args[1..];
                break;
            }
            if !option.starts_with('-') || option == "-" {
                break;
            }
            if !option[1..].chars().all(|c| matches!(c, 'n' | 'i')) {
                return Err("grep accepts only -n and -i options".into());
            }
            ignore_case |= option.contains('i');
            args = &args[1..];
        }
        let [text, path] = args else {
            return Err("grep requires text and one scoped path".into());
        };
        if text.len() > 1024 {
            return Err("grep text exceeds 1024 bytes".into());
        }
        let alternatives: Vec<_> = text.split("\\|").collect();
        if alternatives.len() > 16 || alternatives.iter().any(|part| part.is_empty()) {
            return Err("grep requires 1 to 16 nonempty literal alternatives".into());
        }
        let alternatives: Vec<_> = alternatives
            .into_iter()
            .map(|part| {
                if ignore_case {
                    part.to_lowercase()
                } else {
                    part.to_owned()
                }
            })
            .collect();
        let mut hits = Vec::new();
        for name in self.names(path)? {
            if let Ok(body) = self.read(&name) {
                for (n, line) in body.lines().enumerate() {
                    let folded = ignore_case.then(|| line.to_lowercase());
                    let candidate = folded.as_deref().unwrap_or(line);
                    if alternatives.iter().any(|text| candidate.contains(text)) {
                        hits.push(format!("{name}:{}:{}", n + 1, clip(line, 250)));
                    }
                    if hits.len() >= 40 {
                        break;
                    }
                }
            }
            if hits.len() >= 40 {
                break;
            }
        }
        Ok(hits.join("\n"))
    }
    fn execute(&mut self, command: &Command) -> Result<String> {
        match command {
            Command::Write { path, content } => self.edit(path, content.clone()),
            Command::Patch {
                path,
                search,
                replace,
            } => {
                let old = self.read(path)?;
                let count = old
                    .char_indices()
                    .filter(|(i, _)| old[*i..].starts_with(search))
                    .count();
                if search.is_empty() || count != 1 {
                    return Err(
                        "search text must match exactly once, including overlapping matches".into(),
                    );
                }
                self.edit(path, old.replacen(search, replace, 1))
            }
            Command::Run(line) if line == "check" => {
                self.phase = "awaiting-check".into();
                Ok(format!(
                    "Coordinator must check this exact workspace: {}",
                    self.digest()
                ))
            }
            Command::Run(line) => {
                let args = split(line)?;
                match args
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .as_slice()
                {
                    ["ls", path] => Ok(self.names(path)?.join("\n")),
                    ["copy", source, destination] => {
                        let text = self.read(source)?;
                        self.edit(destination, text)
                    }
                    ["view", path, start, end] => {
                        let start: usize = start.parse().map_err(|_| "invalid start line")?;
                        let end: usize = end.parse().map_err(|_| "invalid end line")?;
                        if start == 0 || end < start {
                            return Err("view requires a positive start and an end at or after it".into());
                        }
                        let text = self.read(path)?;
                        Ok(view_page(&text, start, end))
                    }
                    ["grep", ..] => self.grep(&args[1..]),
                    _ => Err(
                        "available commands: ls PATH, view PATH START END, grep [-n] [-i] TEXT PATH, copy SOURCE DEST, check"
                            .into(),
                    ),
                }
            }
            Command::Done(_) => {
                if self.checked.as_deref() != Some(self.digest().as_str()) || self.files.is_empty()
                {
                    return Err("request check and repair failures before finishing".into());
                }
                self.phase = "review".into();
                Ok("Ready for review.".into())
            }
            _ => Err(
                "the coordinator opens a draft PR after checking and reviewing the exported patch"
                    .into(),
            ),
        }
    }
    pub fn feedback(&mut self, digest: &str, code: i32, output: &str) -> Result<()> {
        if self.phase != "awaiting-check" || digest != self.digest() || output.len() > 12000 {
            return Err("check result does not match the waiting workspace".into());
        }
        self.checked = (code == 0).then(|| digest.into());
        self.agent.steps.push(Step {
            thought: "Coordinator check result".into(),
            observations: vec![Observation {
                command: "check".into(),
                code,
                output: output.into(),
            }],
            problem: None,
        });
        self.phase = "model".into();
        Ok(())
    }
    fn guide(&mut self, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_GUIDANCE_BYTES || text.contains('\0') {
            return Err("guidance must contain 1 to 2000 bytes of text".into());
        }
        let addition = format!("\n\nCoordinator guidance:\n{text}");
        if self.agent.instructions.len() + addition.len() > MAX_INSTRUCTION_BYTES {
            return Err("workspace guidance limit reached".into());
        }
        self.agent.instructions.push_str(&addition);
        Ok(())
    }
    pub async fn advance(&mut self, path: &Path) -> Result<()> {
        if matches!(self.phase.as_str(), "awaiting-check" | "review" | "stopped") {
            return Ok(());
        }
        if now() >= self.until || self.turns >= MAX_TURNS || self.calls >= 256 {
            self.phase = "stopped".into();
            return save(path, self);
        }
        if self.pending.is_empty() {
            self.turns += 1;
            save(path, self)?;
            let key = kiln_git::keys::load(&self.config.key)?;
            let buyer = pylon::identity::Identity::from_secret(key.secret)?;
            let names = self
                .config
                .paths
                .iter()
                .map(|p| self.names(p).map(|n| n.join("\n")))
                .collect::<Result<Vec<_>>>()?
                .join("\n");
            let ask = pylon::client::Ask {
                relay: self.config.relay.clone(),
                pylon: Some(self.config.pylon.clone()),
                prompt: self.agent.prompt(&names),
                wait: Duration::from_secs(600.min(self.until.saturating_sub(now()))),
                publish_receipt: false,
                home: path
                    .parent()
                    .ok_or("state parent missing")?
                    .join("receipts"),
                checkers: Default::default(),
                pay: None,
            };
            let want = pylon::group::Want::Profiled(
                Box::new(pylon::group::Want::Model(self.config.model.clone())),
                self.config.profile.clone(),
            );
            let started = Instant::now();
            let answered = pylon::client::ask_model_within(
                &buyer,
                &ask,
                &want,
                &pylon::client::Within::new(ask.wait),
                &[],
                Some(&self.agent.instructions),
            )
            .await;
            self.record_answer(
                &ask.prompt,
                &answered,
                started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            );
            match answered {
                Ok(answer) if answer.text.is_some() => {
                    let text = answer.text.unwrap_or_default();
                    let (_, reply) = kiln::transcript::split_thinking(&text);
                    self.pending = self.agent.plan(&reply);
                }
                result => {
                    self.agent.steps.push(Step {
                        problem: Some(clip(
                            &match result {
                                Ok(a) => format!("{:?}", a.error),
                                Err(e) => e,
                            },
                            1000,
                        )),
                        ..Default::default()
                    });
                }
            }
            save(path, self)?;
        }
        while !self.pending.is_empty()
            && self.calls < 256
            && !matches!(self.phase.as_str(), "awaiting-check" | "review")
        {
            let command = self.pending.remove(0);
            self.calls += 1;
            let (code, output) = match self.execute(&command) {
                Ok(s) => (0, s),
                Err(e) => (1, e),
            };
            if code == 0 || !matches!(command, Command::Done(_)) {
                self.agent.observe(&command, code, output);
            } else {
                self.agent.steps.push(Step {
                    problem: Some(output),
                    ..Default::default()
                });
            }
            save(path, self)?;
        }
        Ok(())
    }
}

pub async fn run(args: &[String]) -> Result<()> {
    let option = |name: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .ok_or_else(|| format!("missing {name}"))
    };
    let action = args.first().map(String::as_str).unwrap_or("help");
    if action == "help" || args.iter().any(|s| s == "--help") {
        println!(
            "kiln-workspace init|advance|status|export|feedback|guide|focus --state FILE\ninit --config FILE\nfeedback --input FILE (digest, code, output)\nguide --input FILE (up to 2000 bytes of UTF-8 text)\nfocus --input FILE (up to 2000 bytes for the current step; resets added guidance)"
        );
        return Ok(());
    }
    let path = PathBuf::from(option("--state")?);
    let parent = path.parent().ok_or("state parent missing")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "workspace is already running")?;
    if action == "init" {
        if path.exists() {
            return Err("workspace already initialized".into());
        }
        let workspace = Workspace::new(load(&PathBuf::from(option("--config")?))?)?;
        save(&path, &workspace)?;
        println!("{}", workspace.summary());
        return Ok(());
    }
    let mut workspace: Workspace = load(&path)?;
    workspace.config.validate()?;
    match action {
        "advance" => workspace.advance(&path).await?,
        "guide" | "focus" => {
            let bytes = workspace_host::artifact::read(
                &PathBuf::from(option("--input")?),
                MAX_GUIDANCE_BYTES,
            )?;
            let text = String::from_utf8(bytes).map_err(|_| "input must be UTF-8 text")?;
            if action == "guide" {
                workspace.guide(&text)?;
            } else {
                let text = text.trim();
                if text.is_empty() || text.contains('\0') {
                    return Err("current step must contain 1 to 2000 bytes of text".into());
                }
                workspace.agent.task = text.into();
                workspace.agent.instructions = INSTRUCTIONS.into();
            }
            save(&path, &workspace)?;
        }
        "feedback" => {
            let result: Value = load(&PathBuf::from(option("--input")?))?;
            workspace.feedback(
                result["digest"].as_str().ok_or("missing digest")?,
                result["code"]
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or("missing code")?,
                result["output"].as_str().ok_or("missing output")?,
            )?;
            save(&path, &workspace)?;
        }
        "export" => {
            println!(
                "{}",
                json!({"base":workspace.config.base,"original":workspace.original,"files":workspace.files,"digest":workspace.digest()})
            );
            return Ok(());
        }
        "status" => (),
        _ => return Err("unknown command".into()),
    }
    println!("{}", workspace.summary());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(repo: &Path) -> Config {
        Config {
            repo: repo.into(),
            base: "a".repeat(40),
            paths: vec!["apps/haunt-preview".into()],
            key: repo.join("key"),
            relay: "wss://relay.example".into(),
            model: "exact-model".into(),
            profile: "consumer-q".into(),
            pylon: kiln_git::keys::load(&repo.join("key"))
                .unwrap()
                .key
                .pubkey()
                .to_string(),
            task: "Repair a shader".into(),
        }
    }
    fn legacy_workspace(repo: &Path) -> Value {
        json!({
            "config": config(repo), "agent": Agent::new("task", 12000),
            "files": {}, "original": {}, "pending": [], "phase": "model",
            "turns": 0, "calls": 0, "until": now() + 60, "checked": null,
        })
    }
    #[test]
    fn view_clamps_real_ranges_and_reports_progress_and_eof() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let dir = root.path().join("apps/haunt-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let source = (1..=150)
            .map(|n| format!("source {n} é\n"))
            .collect::<String>();
        std::fs::write(dir.join("runtime.ts"), source).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path().join("key"), dir.join("link")).unwrap();
        git(root.path(), &["init", "-q"]).unwrap();
        git(root.path(), &["add", "apps"]).unwrap();
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let mut c = config(root.path());
        c.base = git(root.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        let mut workspace = Workspace::new(c).unwrap();
        std::fs::write(dir.join("runtime.ts"), "uncommitted host content").unwrap();
        workspace.checked = Some(workspace.digest());
        let checked = workspace.checked.clone();
        for (start, requested_end, displayed_end, next) in [
            (1, 40, 40, "Next line: 41."),
            (40, 75, 75, "Next line: 76."),
            (1, usize::MAX, 80, "Next line: 81."),
            (100, 140, 140, "Next line: 141."),
            (80, 140, 140, "Next line: 141."),
            (2, 2, 2, "Next line: 3."),
            (145, 200, 150, "EOF."),
            (149, usize::MAX, 150, "EOF."),
        ] {
            let output = workspace
                .execute(&Command::Run(format!(
                    "view apps/haunt-preview/runtime.ts {start} {requested_end}"
                )))
                .unwrap();
            let mut lines = output.lines();
            assert_eq!(
                lines.next().unwrap(),
                format!("Lines {start}-{displayed_end} of 150. {next}")
            );
            let displayed = lines.collect::<Vec<_>>();
            assert_eq!(displayed.len(), displayed_end - start + 1);
            assert!(displayed.len() <= 80);
            assert!(output.len() <= 1400);
            assert_eq!(
                displayed.first().unwrap(),
                &format!("{start}:source {start} é")
            );
            assert_eq!(
                displayed.last().unwrap(),
                &format!("{displayed_end}:source {displayed_end} é")
            );
            workspace.agent.steps.push(Step::default());
            workspace.agent.observe(
                &Command::Run(format!(
                    "view apps/haunt-preview/runtime.ts {start} {requested_end}"
                )),
                0,
                output.clone(),
            );
            assert!(workspace.agent.prompt("runtime.ts").contains(&output));
        }
        assert_eq!(
            workspace
                .execute(&Command::Run(
                    "view apps/haunt-preview/runtime.ts 151 200".into()
                ))
                .unwrap(),
            "EOF: 150 lines; start 151 is past the end."
        );
        assert_eq!(workspace.checked, checked);
        for line in [
            "view apps/haunt-preview/runtime.ts 0 20",
            "view apps/haunt-preview/runtime.ts 20 10",
            "view apps/haunt-preview/runtime.ts no 10",
            "view ../key 1 40",
            "view key 1 40",
            "view apps/haunt-preview-evil/a.ts 1 40",
        ] {
            assert!(
                workspace.execute(&Command::Run(line.into())).is_err(),
                "{line}"
            );
        }
        #[cfg(unix)]
        assert!(
            workspace
                .execute(&Command::Run("view apps/haunt-preview/link 1 40".into()))
                .is_err()
        );
        workspace
            .edit("apps/haunt-preview/runtime.ts", "overlay é\n".into())
            .unwrap();
        assert_eq!(
            workspace
                .execute(&Command::Run(
                    "view apps/haunt-preview/runtime.ts 1 40".into()
                ))
                .unwrap(),
            "Lines 1-1 of 1. EOF.\n1:overlay é"
        );
        workspace
            .edit("apps/haunt-preview/empty.ts", String::new())
            .unwrap();
        assert_eq!(
            workspace
                .execute(&Command::Run(
                    "view apps/haunt-preview/empty.ts 1 40".into()
                ))
                .unwrap(),
            "EOF: 0 lines; start 1 is past the end."
        );
    }
    #[test]
    fn view_pages_keep_whole_utf8_lines_and_survive_the_real_prompt_limit() {
        let source = (1..=80)
            .map(|n| format!("source {n} {}", "界é".repeat(12)))
            .collect::<Vec<_>>();
        let text = source.join("\n");
        let mut agent = Agent::new("Read every source line", 12000);
        agent.instructions = INSTRUCTIONS.into();
        for _ in 0..30 {
            agent.steps.push(Step {
                observations: vec![Observation {
                    command: "view old.ts 1 80".into(),
                    code: 0,
                    output: "old history\n".repeat(200),
                }],
                ..Default::default()
            });
        }
        let mut start = 1;
        let mut seen = Vec::new();
        while start <= source.len() {
            let page = view_page(&text, start, usize::MAX);
            assert!(page.len() <= MAX_VIEW_BYTES);
            let (header, body) = page.split_once('\n').unwrap();
            let rows = body.lines().collect::<Vec<_>>();
            assert!(!rows.is_empty() && rows.len() <= MAX_VIEW_LINES);
            let last = start + rows.len() - 1;
            let next = if last == source.len() {
                "EOF.".into()
            } else {
                format!("Next line: {}.", last + 1)
            };
            assert_eq!(header, format!("Lines {start}-{last} of 80. {next}"));
            for (n, row) in (start..=last).zip(rows) {
                assert_eq!(row, format!("{n}:{}", source[n - 1]));
                seen.push(n);
            }
            agent.steps.push(Step::default());
            agent.observe(
                &Command::Run(format!("view runtime.ts {start} 80")),
                0,
                page.clone(),
            );
            assert!(agent.prompt("runtime.ts").contains(&page));
            assert_eq!(agent.budget, 12000);
            start = last + 1;
        }
        assert_eq!(seen, (1..=80).collect::<Vec<_>>());
    }
    #[test]
    fn view_byte_ceiling_includes_the_header_and_reports_oversized_lines() {
        let header = "Lines 1-1 of 2. Next line: 2.\n";
        let line = "x".repeat(MAX_VIEW_BYTES - header.len() - "1:".len());
        let text = format!("{line}\nsecond line");
        let page = view_page(&text, 1, 80);
        assert_eq!(page.len(), MAX_VIEW_BYTES);
        assert_eq!(page, format!("{header}1:{line}"));
        assert_eq!(
            view_page(&text, 2, 80),
            "Lines 2-2 of 2. EOF.\n2:second line"
        );

        let large = "界".repeat(500);
        let text = format!("small\n{large}\ntail");
        assert_eq!(
            view_page(&text, 1, 80),
            "Lines 1-1 of 3. Next line: 2.\n1:small"
        );
        let diagnostic = view_page(&text, 2, 80);
        assert_eq!(
            diagnostic,
            "Line 2 is too long for a 1400-byte page; no source lines shown. Next line: 3."
        );
        assert!(diagnostic.len() <= MAX_VIEW_BYTES);
        assert!(!diagnostic.contains('界'));
        assert_eq!(view_page(&text, 3, 80), "Lines 3-3 of 3. EOF.\n3:tail");
        assert_eq!(
            view_page(&large, 1, 80),
            "Line 1 is too long for a 1400-byte page; no source lines shown. EOF."
        );
    }
    #[test]
    fn grep_supports_observed_options_and_literal_alternatives_with_existing_bounds() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let dir = root.path().join("apps/haunt-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let source =
            "dispose();\nstate();\nqueueScene();\nedit(project);\nDISPOSE();\nunrelated();\n";
        std::fs::write(dir.join("runtime.ts"), source).unwrap();
        std::fs::write(dir.join("oversize.ts"), "dispose".repeat(40_000)).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path().join("key"), dir.join("link")).unwrap();
        git(root.path(), &["init", "-q"]).unwrap();
        git(root.path(), &["add", "apps"]).unwrap();
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let mut c = config(root.path());
        c.base = git(root.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        let mut workspace = Workspace::new(c).unwrap();
        std::fs::write(dir.join("runtime.ts"), "uncommitted host text").unwrap();
        let execute =
            |workspace: &mut Workspace, line: &str| workspace.execute(&Command::Run(line.into()));
        let expected = [
            "apps/haunt-preview/runtime.ts:1:dispose();",
            "apps/haunt-preview/runtime.ts:2:state();",
            "apps/haunt-preview/runtime.ts:3:queueScene();",
            "apps/haunt-preview/runtime.ts:4:edit(project);",
        ]
        .join("\n");
        assert_eq!(
            execute(
                &mut workspace,
                r#"grep "dispose\|state()\|queueScene\|edit(" apps/haunt-preview/runtime.ts"#,
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            execute(
                &mut workspace,
                "grep -n 'state()' apps/haunt-preview/runtime.ts"
            )
            .unwrap(),
            "apps/haunt-preview/runtime.ts:2:state();"
        );
        for options in ["-n -i", "-in", "-ni"] {
            let output = execute(
                &mut workspace,
                &format!(
                    r#"grep {options} "dispose\|state()\|queueScene\|edit(" apps/haunt-preview"#
                ),
            )
            .unwrap();
            assert_eq!(
                output,
                format!("{expected}\napps/haunt-preview/runtime.ts:5:DISPOSE();")
            );
        }
        assert!(
            execute(&mut workspace, "grep 'queue.*' apps/haunt-preview")
                .unwrap()
                .is_empty()
        );
        assert!(
            execute(&mut workspace, "grep 'dispose|state' apps/haunt-preview")
                .unwrap()
                .is_empty()
        );
        workspace
            .edit(
                "apps/haunt-preview/runtime.ts",
                "-needle\nSTATE();\n".into(),
            )
            .unwrap();
        workspace.checked = Some(workspace.digest());
        let checked = workspace.checked.clone();
        assert_eq!(
            execute(
                &mut workspace,
                "grep -- -needle apps/haunt-preview/runtime.ts"
            )
            .unwrap(),
            "apps/haunt-preview/runtime.ts:1:-needle"
        );
        assert_eq!(
            execute(
                &mut workspace,
                "grep -i 'state()' apps/haunt-preview/runtime.ts"
            )
            .unwrap(),
            "apps/haunt-preview/runtime.ts:2:STATE();"
        );
        assert_eq!(workspace.checked, checked);
        for invalid in [
            "grep -r dispose apps/haunt-preview",
            "grep -E dispose apps/haunt-preview",
            "grep -xni dispose apps/haunt-preview",
            "grep dispose ../key",
            "grep dispose key",
            "grep dispose apps/haunt-preview-evil",
            "grep dispose apps/haunt-preview ; touch owned",
            "grep '' apps/haunt-preview",
            r#"grep "dispose\|" apps/haunt-preview"#,
        ] {
            assert!(execute(&mut workspace, invalid).is_err(), "{invalid}");
        }
        let many = std::iter::repeat_n("no", 17)
            .collect::<Vec<_>>()
            .join("\\|");
        assert!(execute(&mut workspace, &format!("grep '{many}' apps/haunt-preview")).is_err());
        assert!(
            execute(
                &mut workspace,
                &format!("grep '{}' apps/haunt-preview", "x".repeat(1025))
            )
            .is_err()
        );
        let line = format!("hit{}", "x".repeat(1000));
        workspace
            .edit(
                "apps/haunt-preview/capped.ts",
                std::iter::repeat_n(line, 100)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
        let output = execute(&mut workspace, "grep hit apps/haunt-preview/capped.ts").unwrap();
        assert_eq!(
            output
                .lines()
                .filter(|line| line.starts_with("apps/haunt-preview/capped.ts:"))
                .count(),
            40
        );
        assert!(output.lines().all(|line| line.chars().count() < 300));
        assert!(!root.path().join("owned").exists());
    }
    #[test]
    fn copy_uses_scoped_text_snapshots_and_invalidates_the_check() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let dir = root.path().join("apps/haunt-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let source = (0..750)
            .map(|line| format!("export const value_{line} = {line};\n"))
            .collect::<String>();
        assert!(source.len() > 16000);
        std::fs::write(dir.join("worker.ts"), &source).unwrap();
        std::fs::write(dir.join("oversize.ts"), "x".repeat(250001)).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path().join("key"), dir.join("link")).unwrap();
        git(root.path(), &["init", "-q"]).unwrap();
        git(root.path(), &["add", "apps"]).unwrap();
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let mut c = config(root.path());
        c.base = git(root.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        let mut workspace = Workspace::new(c).unwrap();
        workspace.execute(&Command::Run("check".into())).unwrap();
        workspace
            .feedback(&workspace.digest(), 0, "checks passed")
            .unwrap();
        let output = workspace
            .execute(&Command::Run(
                "copy apps/haunt-preview/worker.ts apps/haunt-preview/runtime.ts".into(),
            ))
            .unwrap();
        assert!(output.len() < 100);
        assert_eq!(workspace.files["apps/haunt-preview/runtime.ts"], source);
        assert!(workspace.checked.is_none());
        assert!(workspace.execute(&Command::Done("done".into())).is_err());
        assert!(!dir.join("runtime.ts").exists());
        workspace
            .edit("apps/haunt-preview/worker.ts", "updated overlay".into())
            .unwrap();
        workspace
            .execute(&Command::Run(
                "copy apps/haunt-preview/worker.ts apps/haunt-preview/overlay.ts".into(),
            ))
            .unwrap();
        assert_eq!(
            workspace.files["apps/haunt-preview/overlay.ts"],
            "updated overlay"
        );
        let before = workspace.digest();
        for line in [
            "guide apps/haunt-preview/worker.ts apps/haunt-preview/x.ts",
            "copy ../key apps/haunt-preview/x.ts",
            "copy key apps/haunt-preview/x.ts",
            "copy apps/haunt-preview/worker.ts ../x.ts",
            "copy apps/haunt-preview/worker.ts apps/haunt-preview-evil/x.ts",
            "copy apps/haunt-preview/oversize.ts apps/haunt-preview/x.ts",
            "copy apps/haunt-preview/worker.ts apps/haunt-preview/runtime.ts/x.ts",
            "copy apps/haunt-preview/worker.ts apps/haunt-preview",
            "copy apps/haunt-preview apps/haunt-preview/x.ts",
        ] {
            assert!(
                workspace.execute(&Command::Run(line.into())).is_err(),
                "{line}"
            );
        }
        #[cfg(unix)]
        for line in [
            "copy apps/haunt-preview/link apps/haunt-preview/x.ts",
            "copy apps/haunt-preview/link/x.ts apps/haunt-preview/x.ts",
            "copy apps/haunt-preview/worker.ts apps/haunt-preview/link",
            "copy apps/haunt-preview/worker.ts apps/haunt-preview/link/x.ts",
        ] {
            assert!(
                workspace.execute(&Command::Run(line.into())).is_err(),
                "{line}"
            );
        }
        assert_eq!(workspace.digest(), before);
        assert_eq!(
            std::fs::read_to_string(dir.join("worker.ts")).unwrap(),
            source
        );
    }
    #[tokio::test]
    async fn guidance_is_bounded_persisted_and_uses_the_workspace_lock() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let mut workspace: Workspace =
            serde_json::from_value(legacy_workspace(root.path())).unwrap();
        workspace.agent.instructions = INSTRUCTIONS.into();
        workspace.phase = "awaiting-check".into();
        workspace.checked = Some(workspace.digest());
        workspace.pending = vec![Command::Run("check".into())];
        workspace.record_answer("fixture prompt", &Err("fixture refusal".into()), 1);
        let before = serde_json::to_value(&workspace).unwrap();
        let state = root.path().join("workspace.json");
        let note = root.path().join("note.txt");
        let guidance = "Use copy SOURCE DEST to duplicate a scoped file without regenerating it.";
        std::fs::write(&note, guidance).unwrap();
        save(&state, &workspace).unwrap();
        let args = vec![
            "guide".into(),
            "--state".into(),
            state.to_string_lossy().into_owned(),
            "--input".into(),
            note.to_string_lossy().into_owned(),
        ];
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.with_extension("lock"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        assert_eq!(
            run(&args).await.unwrap_err(),
            "workspace is already running"
        );
        assert_eq!(
            serde_json::to_value(load::<Workspace>(&state).unwrap()).unwrap(),
            before
        );
        drop(lock);
        run(&args).await.unwrap();
        let restored: Workspace = load(&state).unwrap();
        assert!(restored.agent.instructions.ends_with(guidance));
        let mut after = serde_json::to_value(&restored).unwrap();
        after["agent"]["instructions"] = before["agent"]["instructions"].clone();
        assert_eq!(after, before);
        for bad in [
            vec![b'x'; MAX_GUIDANCE_BYTES + 1],
            vec![0xff],
            vec![0],
            vec![b'\n'],
        ] {
            let unchanged = std::fs::read(&state).unwrap();
            std::fs::write(&note, bad).unwrap();
            assert!(run(&args).await.is_err());
            assert_eq!(std::fs::read(&state).unwrap(), unchanged);
        }
        std::fs::write(&note, "x".repeat(MAX_GUIDANCE_BYTES)).unwrap();
        run(&args).await.unwrap();
        let unchanged = std::fs::read(&state).unwrap();
        assert_eq!(
            run(&args).await.unwrap_err(),
            "workspace guidance limit reached"
        );
        assert_eq!(std::fs::read(&state).unwrap(), unchanged);
        let restored: Workspace = load(&state).unwrap();
        assert!(restored.agent.instructions.len() <= MAX_INSTRUCTION_BYTES);
    }
    #[tokio::test]
    async fn focus_replaces_only_the_current_brief_under_the_workspace_lock() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let mut workspace: Workspace =
            serde_json::from_value(legacy_workspace(root.path())).unwrap();
        workspace.agent.instructions = format!("{INSTRUCTIONS}\nOld coordinator guidance");
        workspace.agent.task = "Broad task with earlier progress".into();
        workspace.agent.done = Some("Earlier summary".into());
        workspace.agent.budget = 8250;
        workspace
            .files
            .insert("apps/haunt-preview/a.ts".into(), "new text".into());
        workspace
            .original
            .insert("apps/haunt-preview/a.ts".into(), "old text".into());
        workspace.phase = "awaiting-check".into();
        workspace.checked = Some(workspace.digest());
        workspace.pending = vec![Command::Run("check".into())];
        workspace.turns = 95;
        workspace.calls = 255;
        workspace.until = now() - 1;
        workspace.record_answer("fixture prompt", &Err("fixture refusal".into()), 1);
        assert!(
            workspace
                .execute(&Command::Run("focus next step".into()))
                .is_err()
        );
        let before = serde_json::to_value(&workspace).unwrap();
        let state = root.path().join("workspace.json");
        let note = root.path().join("note.txt");
        let focused = "Keep the copied file. Replace its shared state with explicit input é.";
        std::fs::write(&note, focused).unwrap();
        save(&state, &workspace).unwrap();
        let args = vec![
            "focus".into(),
            "--state".into(),
            state.to_string_lossy().into_owned(),
            "--input".into(),
            note.to_string_lossy().into_owned(),
        ];
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.with_extension("lock"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        assert_eq!(
            run(&args).await.unwrap_err(),
            "workspace is already running"
        );
        assert_eq!(
            serde_json::to_value(load::<Workspace>(&state).unwrap()).unwrap(),
            before
        );
        drop(lock);
        run(&args).await.unwrap();
        let restored: Workspace = load(&state).unwrap();
        assert_eq!(restored.agent.task, focused);
        assert_eq!(restored.agent.instructions, INSTRUCTIONS);
        let mut after = serde_json::to_value(&restored).unwrap();
        after["agent"]["task"] = before["agent"]["task"].clone();
        after["agent"]["instructions"] = before["agent"]["instructions"].clone();
        assert_eq!(after, before);
        for invalid in [
            vec![b'x'; MAX_GUIDANCE_BYTES + 1],
            vec![0xff],
            vec![0],
            vec![b'\n'],
        ] {
            let unchanged = std::fs::read(&state).unwrap();
            std::fs::write(&note, invalid).unwrap();
            assert!(run(&args).await.is_err());
            assert_eq!(std::fs::read(&state).unwrap(), unchanged);
        }
        let full = "é".repeat(MAX_GUIDANCE_BYTES / 2);
        std::fs::write(&note, &full).unwrap();
        run(&args).await.unwrap();
        assert_eq!(load::<Workspace>(&state).unwrap().agent.task, full);
    }
    #[test]
    fn legacy_workspace_loads_without_answer_evidence() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let legacy = legacy_workspace(root.path());
        assert!(legacy.get("answers").is_none());
        let workspace: Workspace = serde_json::from_value(legacy).unwrap();
        assert!(workspace.answers.is_empty());
        assert_eq!(workspace.phase, "model");
        assert_eq!(workspace.config.profile, "consumer-q");
    }
    #[test]
    fn answer_evidence_retains_actual_metadata_without_private_text() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let mut workspace: Workspace =
            serde_json::from_value(legacy_workspace(root.path())).unwrap();
        let digest = workspace.digest();
        let answer = pylon::client::Answer {
            request: "a".repeat(64),
            pylon: "30200:actual-producer:maya-consumer-q-1-head".into(),
            pylon_npub: "actual-producer-npub".into(),
            label: "Maya".into(),
            model: "deepseek-v4.1-flash@sha256:actual-artifact".into(),
            profile: "consumer-q".into(),
            lossy: true,
            prompt: "private prompt".into(),
            text: Some("private reply".into()),
            reasoning: Some("private reasoning".into()),
            outcome: "accepted".into(),
            error: None,
            discover_ms: 7,
            contact_ms: Some(11),
            answer_ms: Some(123),
            usage: Some(json!({"input": 12, "output": 0, "prompt": "private prompt"})),
            receipt: None,
            receipt_error: None,
            receipt_event: None,
            paid_msat: None,
            declared: None,
        };
        workspace.turns = 1;
        workspace.record_answer("requested prompt", &Ok(answer.clone()), 130);
        let evidence = &workspace.answers[0];
        assert_eq!(evidence.request.as_deref(), Some(answer.request.as_str()));
        assert_eq!(evidence.pylon.as_deref(), Some(answer.pylon.as_str()));
        assert_eq!(evidence.model.as_deref(), Some(answer.model.as_str()));
        assert_eq!(evidence.profile.as_deref(), Some("consumer-q"));
        assert_eq!(evidence.lossy, Some(true));
        assert_eq!(evidence.outcome, "accepted");
        assert!(!evidence.error);
        assert_eq!(evidence.usage, Some(json!({"input": 12, "output": 0})));
        assert_eq!(
            evidence.timing,
            AnswerTiming {
                elapsed_ms: 130,
                discover_ms: Some(7),
                contact_ms: Some(11),
                answer_ms: Some(123),
            }
        );
        assert_eq!(evidence.prompt_sha256, kiln::sha256_hex(b"private prompt"));
        assert_eq!(
            evidence.reply_sha256,
            Some(kiln::sha256_hex(b"private reply"))
        );
        let mut refused = answer;
        refused.text = None;
        refused.outcome = "rejected".into();
        refused.error = Some("refused private prompt excerpt".into());
        refused.usage = None;
        workspace.turns = 2;
        workspace.record_answer("requested prompt", &Ok(refused), 140);
        assert_eq!(workspace.answers[1].outcome, "rejected");
        assert!(workspace.answers[1].error);
        assert_eq!(
            workspace.answers[1].error_sha256,
            Some(kiln::sha256_hex(b"refused private prompt excerpt"))
        );
        assert!(workspace.answers[1].reply_sha256.is_none());
        assert!(workspace.answers[1].usage.is_none());
        workspace.turns = 3;
        workspace.record_answer("requested prompt", &Err("connection failed".into()), 5);
        assert!(workspace.answers[2].request.is_none());
        assert!(workspace.answers[2].pylon.is_none());
        assert!(workspace.answers[2].timing.discover_ms.is_none());
        assert_eq!(workspace.answers[2].outcome, "error");
        assert_eq!(
            workspace.answers[2].prompt_sha256,
            kiln::sha256_hex(b"requested prompt")
        );
        assert_eq!(workspace.digest(), digest);
        let state = root.path().join("workspace.json");
        save(&state, &workspace).unwrap();
        let restored: Workspace = load(&state).unwrap();
        assert_eq!(restored.answers, workspace.answers);
        let stored = std::fs::read_to_string(state).unwrap();
        for private in [
            "private prompt",
            "private reply",
            "private reasoning",
            "connection failed",
        ] {
            assert!(!stored.contains(private), "{private}");
        }
        assert_eq!(restored.summary()["answers"].as_array().unwrap().len(), 3);
        for turn in 4..=MAX_TURNS + 1 {
            workspace.turns = turn;
            workspace.record_answer("prompt", &Err("connection failed".into()), 1);
        }
        assert_eq!(workspace.answers.len(), MAX_TURNS);
        assert_eq!(workspace.answers.first().unwrap().turn, 2);
        assert_eq!(workspace.answers.last().unwrap().turn, MAX_TURNS + 1);
    }
    #[test]
    fn scope_rejects_traversal_and_adjacent_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let c = config(root.path());
        c.validate().unwrap();
        assert!(c.allows("apps/haunt-preview/src/project.ts"));
        for path in [
            "apps/haunt-preview-evil/a",
            "apps/haunt-preview/../secret",
            "apps/haunt-preview/.git/config",
            "apps/haunt-preview//x",
            "/apps/haunt-preview/x",
        ] {
            assert!(!c.allows(path), "{path}");
        }
    }
    #[test]
    fn edits_invalidate_checks_and_overlapping_patches_fail() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        git(root.path(), &["init", "-q"]).unwrap();
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let mut c = config(root.path());
        c.base = git(root.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        let mut w = Workspace {
            agent: Agent::new("task", 12000),
            config: c,
            files: BTreeMap::from([("apps/haunt-preview/a.ts".into(), "ééé".into())]),
            original: BTreeMap::from([("apps/haunt-preview/a.ts".into(), "original".into())]),
            pending: vec![],
            phase: "model".into(),
            turns: 0,
            calls: 0,
            until: now() + 60,
            checked: None,
            answers: vec![],
        };
        assert!(
            w.execute(&Command::Patch {
                path: "apps/haunt-preview/a.ts".into(),
                search: "éé".into(),
                replace: "x".into()
            })
            .is_err()
        );
        assert!(w.execute(&Command::Done("done".into())).is_err());
        w.execute(&Command::Run("check".into())).unwrap();
        let digest = w.digest();
        assert!(w.feedback("wrong", 0, "pass").is_err());
        w.feedback(&digest, 0, "pass").unwrap();
        assert!(w.execute(&Command::Done("done".into())).is_ok());
        w.edit("apps/haunt-preview/a.ts", "new".into()).unwrap();
        assert!(w.checked.is_none());
        assert!(w.execute(&Command::Done("done".into())).is_err());
        assert!(w.execute(&Command::Run("sh -c anything".into())).is_err());
    }
    #[test]
    fn reads_use_the_pinned_commit_and_refuse_symlink_targets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("key"), "11".repeat(32)).unwrap();
        let dir = root.path().join("apps/haunt-preview");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("source.ts"), "committed").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path().join("key"), dir.join("link")).unwrap();
        git(root.path(), &["init", "-q"]).unwrap();
        git(root.path(), &["add", "apps"]).unwrap();
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let mut c = config(root.path());
        c.base = git(root.path(), &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .into();
        let mut w = Workspace::new(c).unwrap();
        std::fs::write(dir.join("source.ts"), "uncommitted").unwrap();
        assert_eq!(w.read("apps/haunt-preview/source.ts").unwrap(), "committed");
        #[cfg(unix)]
        assert!(w.read("apps/haunt-preview/link").is_err());
        #[cfg(unix)]
        assert!(
            w.edit("apps/haunt-preview/link/new.ts", "must not escape".into())
                .is_err()
        );
        assert!(
            w.edit(
                "apps/haunt-preview/source.ts/nested.ts",
                "invalid ancestor".into()
            )
            .is_err()
        );
        w.edit("apps/haunt-preview/new.ts", "new".into()).unwrap();
        w.edit("apps/haunt-preview/newdir/child.ts", "child".into())
            .unwrap();
        assert!(
            w.edit(
                "apps/haunt-preview/newdir",
                "invalid directory replacement".into()
            )
            .is_err()
        );
        assert!(!w.files.contains_key("apps/haunt-preview/newdir"));
        assert!(
            w.edit(
                "apps/haunt-preview/new.ts/nested.ts",
                "invalid overlay ancestor".into()
            )
            .is_err()
        );
        w.edit("apps/haunt-preview/source.ts", "changed".into())
            .unwrap();
        assert_eq!(w.original["apps/haunt-preview/source.ts"], "committed");
        assert!(!w.original.contains_key("apps/haunt-preview/new.ts"));
        assert_eq!(
            std::fs::read_to_string(dir.join("source.ts")).unwrap(),
            "uncommitted"
        );
    }
}
