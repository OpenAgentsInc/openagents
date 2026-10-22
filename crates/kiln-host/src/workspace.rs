//! A bounded workspace adapter for the shared Kiln agent. Checks stay with the caller.
use crate::{Result, load, now, save, worker::git};
use kiln::agent::{Agent, Command, Observation, Step, clip, split};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

const INSTRUCTIONS: &str = r#"You are Kiln, editing a bounded repository workspace. Reply only with one JSON object:
{"thought":"one short sentence","commands":[...]}
Commands: {"run":"ls PATH"}, {"run":"view PATH START END"}, {"run":"grep TEXT PATH"}, {"write":"PATH","content":"full text"}, {"patch":"PATH","search":"exact unique old text","replace":"new text"}, {"run":"check"}, {"done":"summary"}.
Read before editing. Use small exact patches, preserve existing style, and write meaningful regression tests. View at most 20 lines at a time. No shell or arbitrary programs are available. check asks the coordinator to run the task's real checks outside this workspace. Read failures, repair, and check again. Say done only after check passed for your current files. Never change paths outside the supplied scope. Your edits remain private until a reviewer accepts them."#;

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
        })
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
        if !self.original.contains_key(path) {
            let entries = git(
                &self.config.repo,
                &["ls-tree", &self.config.base, "--", path],
            )?;
            if !entries.is_empty() {
                self.original.insert(path.into(), self.read(path)?);
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
        json!({"phase":self.phase,"turns":self.turns,"calls":self.calls,"base":self.config.base,"pylon":self.config.pylon,"model":self.config.model,"profile":self.config.profile,"files":self.files.keys().collect::<Vec<_>>(),"digest":self.digest(),"checked":self.checked,"done":self.agent.done,"latest":self.agent.steps.last()})
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
                    ["view", path, start, end] => {
                        let start: usize = start.parse().map_err(|_| "invalid start line")?;
                        let end: usize = end.parse().map_err(|_| "invalid end line")?;
                        if start == 0 || end < start || end - start >= 20 {
                            return Err("view requires 1 to 20 lines".into());
                        }
                        Ok(self
                            .read(path)?
                            .lines()
                            .enumerate()
                            .skip(start - 1)
                            .take(end - start + 1)
                            .map(|(n, l)| format!("{}:{l}", n + 1))
                            .collect::<Vec<_>>()
                            .join("\n"))
                    }
                    ["grep", text, path] => {
                        let mut hits = Vec::new();
                        for name in self.names(path)? {
                            if let Ok(body) = self.read(&name) {
                                for (n, line) in body.lines().enumerate() {
                                    if line.contains(text) {
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
                    _ => Err(
                        "available commands: ls PATH, view PATH START END, grep TEXT PATH, check"
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
    pub async fn advance(&mut self, path: &Path) -> Result<()> {
        if matches!(self.phase.as_str(), "awaiting-check" | "review" | "stopped") {
            return Ok(());
        }
        if now() >= self.until || self.turns >= 96 || self.calls >= 256 {
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
            let answered = pylon::client::ask_model_within(
                &buyer,
                &ask,
                &want,
                &pylon::client::Within::new(ask.wait),
                &[],
                Some(&self.agent.instructions),
            )
            .await;
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
            "kiln-workspace init|advance|status|export|feedback --state FILE\ninit --config FILE\nfeedback --input FILE (digest, code, output)"
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
        let c = config(root.path());
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
        w.edit("apps/haunt-preview/new.ts", "new".into()).unwrap();
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
