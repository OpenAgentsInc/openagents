use super::*;
use coder_environment::evidence::Redactor;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
use codex_transport::fake::{call, say};
use codex_transport::{Reply, TokenUsage, TransportError};
use serde_json::json;
use std::collections::VecDeque;
use std::process::Child;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// One script of replies shared by every transport the studio makes.
#[derive(Clone, Default)]
struct Script(Arc<Mutex<VecDeque<Reply>>>);
impl Script {
    fn push(&self, reply: Reply) {
        self.0.lock().unwrap().push_back(reply);
    }
}
impl Transport for Script {
    async fn respond(&self, _r: &codex_transport::Request) -> Result<Reply, TransportError> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(TransportError::Exhausted)
    }
}

fn machines() -> Arc<FakeProvider> {
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let sanitize = coder_environment_build::sanitize::Plan::new(
        &Default::default(),
        "/tmp/oa-commands/sanitize",
    );
    let pin = SourcePin {
        repository: Some("example/repo".into()),
        revision: COMMIT.into(),
        digest: "b".repeat(64),
    };
    provider.on_command(Box::new(move |spec, _env, files| {
        let id = spec.id.as_str();
        let c = spec.command.as_str();
        if id.ends_with("-inventory-before") || id.ends_with("-inventory-after") {
            FakeRun::exit(
                0,
                &format!("oa-inventory git:head {COMMIT}\noa-inventory done\n"),
                "",
            )
        } else if id == "sanitize" {
            FakeRun::exit(0, &format!("sanitized {}\n", sanitize.digest()), "")
        } else if c.contains("oa-source head=") && c.contains("no-checkout") {
            let r = coder_environment_setup::source::Report::verified_for(&pin, false);
            FakeRun::exit(0, &r.render(), "")
        } else if c.contains("oa-source head=") {
            assert!(c.contains("https://github.com/example/repo.git"));
            files.insert("README.md".into(), "# repo".into());
            let r = coder_environment_setup::source::Report::verified_for(&pin, true);
            FakeRun::exit(0, &r.render(), "")
        } else if c.contains("OA-CHECK") {
            FakeRun::exit(0, "OA-CHECK passed=1 failed=0\n", "")
        } else {
            FakeRun::exit(0, "ok\n", "")
        }
    }));
    provider
}

fn config(state: &Path) -> Config {
    Config {
        schema: SCHEMA.into(),
        state: state.into(),
        machines: crate::Config {
            schema: crate::SCHEMA.into(),
            provider: crate::ProviderKind::Boat,
            gce: None,
            workdir: "/home/user/repo".into(),
            template: Some("oa-coder-runtime-20261008".into()),
            credential_names: BTreeSet::new(),
            tick_seconds: 15,
        },
        owner: Principal {
            workspace: "local".into(),
            principal: "owner".into(),
        },
        codex_home: Some("/tmp/codex-home".into()),
        model: None,
        size: None,
        deadline_seconds: None,
        github_token: None,
        claude_key: None,
        model_api: None,
        home_retention_days: None,
    }
}

fn u() -> TokenUsage {
    TokenUsage::default()
}

async fn until(studio: &Studio, id: &str, want: Status) -> View {
    for _ in 0..2000 {
        let v = studio.view(id).unwrap();
        if v.summary.status == want {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let v = studio.view(id).unwrap();
    panic!("{:?} never became {want:?}: {:?}", v.phase, v.records);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_environment_goes_from_a_repository_to_a_saved_version_and_back_to_work() {
    let dir = tempfile::tempdir().unwrap();
    let provider = machines();
    let script = Script::default();
    for reply in [
        say("I'll look at how this repository builds.", u()),
        call(
            "c1",
            "write_recipe",
            &json!({"install_script":"set -euo pipefail\nmake deps"}),
            u(),
        ),
        call(
            "c2",
            "set_checks",
            &json!({"checks":[{"name":"build","command":"make"}],"offline":false}),
            u(),
        ),
        call("c3", "run_install", &json!({}), u()),
        call("c4", "finish", &json!({"summary":"Builds with make."}), u()),
    ] {
        script.push(reply);
    }
    let transports: Transports<Script> = {
        let script = script.clone();
        Arc::new(move |_env: &str| Ok(script.clone()))
    };
    let studio = Studio::start(
        config(dir.path()),
        "oa-coder-runtime-20261008",
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider.clone(),
        },
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
        transports,
        Duration::from_millis(1),
    )
    .unwrap();
    assert!(studio.list().is_empty());
    assert!(!studio.claude_ready());
    let id = studio
        .create_for(
            &github::Resolved {
                repository: github::RepoName::parse("example/repo").unwrap(),
                branch: "main".into(),
                commit: COMMIT.into(),
                private: false,
            },
            Some("acct_alice"),
        )
        .unwrap();
    // A reply with no tool call is a question for the person.
    let v = until(&studio, &id, Status::NeedsInput).await;
    assert_eq!(
        v.question.as_deref(),
        Some("I'll look at how this repository builds.")
    );
    assert!(studio.steer(&id, "").is_err());
    studio.steer(&id, "Go ahead.").unwrap();
    let v = until(&studio, &id, Status::ReadyToSave).await;
    let candidate = v.candidate.clone().unwrap();
    assert_eq!(candidate.commit, COMMIT);
    assert_eq!(candidate.checks[0].command, "make");
    assert_eq!(v.recipe.as_deref(), Some("set -euo pipefail\nmake deps"));
    assert_eq!(v.summary.repository, "example/repo");

    // An earlier build whose image was never saved (its check failed),
    // and an image some other code made.
    let jobs =
        coder_environment_build::store::Store::under(crate::Layout::under(dir.path()).build_jobs());
    let mut earlier = jobs.list().unwrap().remove(0);
    earlier.id = "build-earlier".into();
    earlier.inputs.build_id = "earlier".into();
    earlier.inputs.image_name =
        coder_environment_build::image_name(&id, "earlier", &earlier.inputs.recipe_digest);
    earlier.created_ms -= 1;
    let name = earlier.inputs.image_name.clone();
    if let Some(capture) = &mut earlier.capture {
        capture.name = name.clone();
    }
    if let Some(image) = &mut earlier.image {
        image.image_id = name;
    }
    jobs.lease(&earlier.id).unwrap().create(&earlier).unwrap();
    let stale = earlier.inputs.image_name.clone();
    {
        let mut state = provider.state.lock().unwrap();
        let (record, files) = state.images.values().next().unwrap().clone();
        state
            .images
            .insert(stale.clone(), (record.clone(), files.clone()));
        state.images.insert("someone-elses".into(), (record, files));
    }
    assert_eq!(provider.state.lock().unwrap().images.len(), 3);

    // Saving names the exact candidate the person saw.
    assert!(studio.save(&id, "stale").await.is_err());
    assert_eq!(studio.save(&id, &candidate.digest).await.unwrap(), 1);
    let (_, pin) = studio.computer_pin("acct_alice", &id, 1).unwrap();
    assert_eq!(pin.source.revision, COMMIT);
    assert_eq!(pin.image.image_id, candidate.image);
    assert!(studio.computer_pin("acct_bob", &id, 1).is_err());
    assert!(studio.computer_pin("acct_alice", &id, 2).is_err());
    // The unsaved image goes; the saved version's and the other one stay.
    for _ in 0..400 {
        if !provider.state.lock().unwrap().images.contains_key(&stale) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let images = provider.state.lock().unwrap().images.clone();
    assert!(!images.contains_key(&stale), "{:?}", images.keys());
    assert!(images.contains_key(&candidate.image) && images.contains_key("someone-elses"));
    let v = studio.view(&id).unwrap();
    assert_eq!(v.summary.status, Status::Saved);
    assert_eq!(v.summary.saved, Some(1));
    assert!(v.versions[0].selected);
    assert!(matches!(
        v.records.last().unwrap().entry,
        Entry::Saved { number: 1 }
    ));
    // No key: Claude Code runs are not offered.
    assert_eq!(
        studio.run_claude(&id, "Fix the tests", None),
        Err("Add your Claude key in Settings to run Claude Code here.".into())
    );

    // A message after saving starts a revision on a new setup computer.
    script.push(say("What should the new version add?", u()));
    studio.steer(&id, "Also install jq.").unwrap();
    until(&studio, &id, Status::NeedsInput).await;
    let rows = studio.list();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].saved, Some(1));
}

#[test]
fn the_config_is_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let good = config(dir.path());
    good.validate().unwrap();
    for bad in [
        Config {
            schema: "other".into(),
            ..good.clone()
        },
        Config {
            state: "relative".into(),
            ..good.clone()
        },
        Config {
            deadline_seconds: Some(0),
            ..good.clone()
        },
    ] {
        assert!(bad.validate().is_err());
    }
    assert_eq!(project_id("my.repo"), "my-repo");
    assert_eq!(
        crate::boat::newest_runtime(
            [
                ("oa-coder-runtime-20261007", "ready"),
                ("oa-coder-runtime-20261008", "pending"),
                ("oa-coder-main-1", "ready"),
            ]
            .into_iter()
        )
        .as_deref(),
        Some("oa-coder-runtime-20261007")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_environment_remembers_the_account_it_was_made_for() {
    let dir = tempfile::tempdir().unwrap();
    let provider = machines();
    let transports: Transports<Script> = Arc::new(|_env: &str| Ok(Script::default()));
    let studio = Studio::start(
        config(dir.path()),
        "oa-coder-runtime-20261008",
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider,
        },
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
        transports,
        Duration::from_millis(1),
    )
    .unwrap();
    let resolved = github::Resolved {
        repository: github::RepoName::parse("example/repo").unwrap(),
        branch: "main".into(),
        commit: COMMIT.into(),
        private: false,
    };
    let mine = studio.create_for(&resolved, Some("acct_alice")).unwrap();
    let unowned = studio.create(&resolved).unwrap();
    assert_ne!(mine, unowned);
    assert_eq!(studio.account(&mine).as_deref(), Some("acct_alice"));
    assert_eq!(studio.account(&unowned), None);
    let rows = studio.list();
    let account = |id: &str| rows.iter().find(|r| r.id == id).unwrap().account.clone();
    assert_eq!(account(&mine).as_deref(), Some("acct_alice"));
    assert_eq!(account(&unowned), None);
}

#[test]
fn a_model_api_needs_an_http_url_and_an_absolute_key_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut good = config(dir.path());
    good.model_api = Some(ModelApi {
        url: "http://127.0.0.1:8791/v1/responses".into(),
        key_file: "/tmp/private/service.key".into(),
    });
    good.validate().unwrap();
    for (url, key_file) in [
        ("file:///v1/responses", "/tmp/k"),
        ("http://127.0.0.1:8791/v1/responses", "relative.key"),
    ] {
        let mut bad = good.clone();
        bad.model_api = Some(ModelApi {
            url: url.into(),
            key_file: key_file.into(),
        });
        assert!(bad.validate().is_err(), "{url} {key_file}");
    }
}

#[tokio::test]
async fn original_studios_cannot_discover_or_resume_the_public_owners_records() {
    let dir = tempfile::tempdir().unwrap();
    let open = |root: &Path| {
        let provider = machines();
        let transports: Transports<Script> = Arc::new(|_| Ok(Script::default()));
        Studio::start(
            config(root),
            "oa-coder-runtime-20261008",
            Providers {
                setup: provider.clone(),
                build: provider.clone(),
                verify: provider,
            },
            Arc::new(|_| Ok(Redactor::new())),
            transports,
            Duration::from_millis(1),
        )
        .unwrap()
    };
    let parent = open(dir.path());
    let child = open(&dir.path().join("public-computers"));
    parent.attach_public(child.clone()).unwrap();
    let resolved = github::Resolved {
        repository: github::RepoName {
            owner: "example".into(),
            name: "repo".into(),
        },
        branch: "main".into(),
        commit: COMMIT.into(),
        private: false,
    };
    let mut authorized = false;
    let id = parent
        .create_for_bound(&resolved, "account-a", |id| {
            assert!(id.starts_with("envc-"));
            authorized = true;
            Ok(())
        })
        .unwrap();
    assert!(authorized);
    assert!(parent.list().iter().any(|row| row.id == id));
    assert_eq!(parent.account(&id).as_deref(), Some("account-a"));
    assert!(parent.view(&id).is_some());
    assert_eq!(parent.computer_state(), Some(child.config.state.as_path()));
    // This instance models an older process: it opens only the original
    // directory and has no knowledge of the attached public owners.
    let older = open(dir.path());
    assert!(older.list().is_empty());
    assert!(older.account(&id).is_none());
    assert!(older.view(&id).is_none());
    assert!(
        coder_environment::store::Store::under(parent.layout.environments())
            .list()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        coder_environment::store::Store::under(child.layout.environments())
            .list()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn shared_private_setup_admits_a_broker_reference_without_operator_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let configuration = config(dir.path());
    assert!(configuration.machines.credential_names.is_empty());
    let mut recipe = crate::tests::recipe();
    recipe.credential_names.insert("GH_TOKEN".into());
    let source = SourcePin {
        repository: Some("example/repo".into()),
        revision: COMMIT.into(),
        digest: "b".repeat(64),
    };
    let env = Environment::new(
        "envc-private",
        ProjectLink {
            workspace: configuration.owner.workspace.clone(),
            project: "repo".into(),
        },
        source,
        recipe,
        now_ms(),
    )
    .unwrap();
    let meta = Meta {
        repository: "example/repo".into(),
        branch: "main".into(),
        created_ms: now_ms(),
        account: Some("alice".into()),
        private: true,
        public_computers: true,
    };
    let b = brief(&configuration, &env, Some(&meta));
    assert_eq!(b.git_credential.as_deref(), Some("GH_TOKEN"));
    assert_eq!(b.credential_names, BTreeSet::from(["GH_TOKEN".into()]));
    assert_eq!(
        b.profile.credentials["GH_TOKEN"],
        PathBuf::from("/credential-references/GH_TOKEN")
    );
    assert!((crate::computers::custody(true))(&b.credential_names).is_ok());
    let mut public = meta;
    public.private = false;
    let b = brief(&configuration, &env, Some(&public));
    assert!(b.git_credential.is_none());
    assert!(!b.profile.credentials.contains_key("GH_TOKEN"));
}

const FENCE_ENV: &str = "env-held";

struct CountTransport(Arc<AtomicUsize>);
impl Transport for CountTransport {
    async fn respond(&self, _request: &codex_transport::Request) -> Result<Reply, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Unsent("gateway down".into()))
    }
}

struct LogMachines {
    deleted: Arc<AtomicUsize>,
}
impl sign_in::Machines for LogMachines {
    async fn create(&self, _image: &str, _size: &str) -> Result<String, String> {
        Ok("machine".into())
    }
    async fn resume(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    async fn run(&self, _id: &str, _script: &str) -> Result<String, String> {
        Ok(String::new())
    }
    async fn put(&self, _id: &str, _path: &str, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }
    async fn stop(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    async fn state(&self, _id: &str) -> Result<Option<String>, String> {
        Ok(Some("stopped".into()))
    }
    async fn delete(&self, _id: &str) -> Result<(), String> {
        self.deleted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn plant_unfinished(state: &Path) {
    let source = SourcePin {
        repository: Some("example/repo".into()),
        revision: COMMIT.into(),
        digest: "b".repeat(64),
    };
    let mut recipe = crate::tests::recipe();
    recipe.limits.deadline_seconds = DEFAULT_DEADLINE_SECONDS;
    let env = Environment::new(
        FENCE_ENV,
        ProjectLink {
            workspace: "local".into(),
            project: "repo".into(),
        },
        source,
        recipe,
        1,
    )
    .unwrap();
    coder_environment::store::Store::under(crate::Layout::under(state).environments())
        .create(&env)
        .unwrap();
    let dir = state.join("environment-studio").join(FENCE_ENV);
    fs::create_dir_all(&dir).unwrap();
    let meta = Meta {
        repository: "example/repo".into(),
        branch: "main".into(),
        created_ms: 1,
        account: None,
        private: true,
        public_computers: false,
    };
    fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    fs::write(
        dir.join("agent.json"),
        serde_json::to_vec(&State::new(FENCE_ENV)).unwrap(),
    )
    .unwrap();
    sign_in::Homes::under(sign_in::root(state))
        .save(
            FENCE_ENV,
            &sign_in::Home {
                sandbox: "sb-old".into(),
                version: 1,
                signed_in: false,
                updated_ms: 1,
                retired: vec![],
            },
        )
        .unwrap();
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel.ends_with("-report") {
                continue;
            }
            out.insert(rel, fs::read(&path).unwrap_or_default());
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn providers(provider: Arc<FakeProvider>) -> Providers<Arc<FakeProvider>> {
    Providers {
        setup: provider.clone(),
        build: provider.clone(),
        verify: provider,
    }
}

fn wait_report(child: &mut Child, path: &Path) -> String {
    let start = Instant::now();
    loop {
        if path.is_file() {
            let text = fs::read_to_string(path).unwrap_or_default();
            if !text.is_empty() {
                return text;
            }
        }
        if let Some(status) = child.try_wait().unwrap() {
            let mut err = String::new();
            if let Some(stderr) = child.stderr.as_mut() {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("fence child exited {status}: {err}");
        }
        if start.elapsed() > Duration::from_secs(25) {
            panic!("timed out waiting for {}", path.display());
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

struct StopChild(Option<Child>);
impl Drop for StopChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test]
async fn environment_fence_child() {
    let Ok(dir) = std::env::var("OA_ENV_FENCE_DIR") else {
        return;
    };
    let role = std::env::var("OA_ENV_FENCE_ROLE").unwrap_or_default();
    if role.starts_with("lease-") {
        crate::fence::lease_child(Path::new(&dir), &role);
        return;
    }
    match role.as_str() {
        "held" => held_candidate(Path::new(&dir)).await,
        "serving" => serving_candidate(Path::new(&dir)).await,
        "unavailable" => unavailable_model(Path::new(&dir)).await,
        other => panic!("unknown fence role {other}"),
    }
}

async fn held_candidate(dir: &Path) {
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let calls = Arc::new(AtomicUsize::new(0));
    let transports: Transports<CountTransport> = {
        let calls = calls.clone();
        Arc::new(move |_| Ok(CountTransport(calls.clone())))
    };
    let studio = Studio::start_admitted(
        config(dir),
        "oa-coder-runtime-20261008",
        providers(provider.clone()),
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
        transports,
        Duration::from_millis(1),
        crate::fence::Admission::serving("held", Arc::new(|| false)),
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let deleted = Arc::new(AtomicUsize::new(0));
    let swept = studio
        .sweep_homes(&LogMachines {
            deleted: deleted.clone(),
        })
        .await;
    let refused = studio.steer(FENCE_ENV, "keep going").unwrap_err();
    assert_eq!(refused, crate::fence::REFUSAL);
    fs::write(
        dir.join("held-report"),
        format!(
            "calls={} commands={} deleted={} listed={}",
            calls.load(Ordering::SeqCst),
            provider.calls().len(),
            swept + deleted.load(Ordering::SeqCst),
            studio.list().len()
        ),
    )
    .unwrap();
}

async fn serving_candidate(dir: &Path) {
    let provider = machines();
    let script = Script::default();
    script.push(say("The repository is ready to set up.", u()));
    let transports: Transports<Script> = {
        let script = script.clone();
        Arc::new(move |_| Ok(script.clone()))
    };
    let studio = Studio::start_admitted(
        config(dir),
        "oa-coder-runtime-20261008",
        providers(provider.clone()),
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
        transports,
        Duration::from_millis(1),
        crate::fence::Admission::serving("serving", Arc::new(|| true)),
    )
    .unwrap();
    let mut status = Status::Starting;
    for _ in 0..3000 {
        if let Some(view) = studio.view(FENCE_ENV) {
            status = view.summary.status;
            if status != Status::Starting {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let commands = provider.calls().len();
    assert!(
        commands > 0 && status != Status::Starting,
        "admitted worker made {commands} provider calls and stayed {status:?}"
    );
    fs::write(
        dir.join("serving-report"),
        format!("progress calls={commands} status={status:?}"),
    )
    .unwrap();
}

async fn unavailable_model(dir: &Path) {
    let provider = machines();
    let calls = Arc::new(AtomicUsize::new(0));
    let transports: Transports<CountTransport> = {
        let calls = calls.clone();
        Arc::new(move |_| Ok(CountTransport(calls.clone())))
    };
    let studio = Studio::start_admitted(
        config(dir),
        "oa-coder-runtime-20261008",
        providers(provider),
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
        transports,
        Duration::from_millis(1),
        crate::fence::Admission::serving("unavailable", Arc::new(|| true)),
    )
    .unwrap();
    let agent_path = dir
        .join("environment-studio")
        .join(FENCE_ENV)
        .join("agent.json");
    let started = Instant::now();
    let phase = loop {
        let text = fs::read_to_string(&agent_path).unwrap_or_default();
        if text.contains("\"failed\"") {
            panic!("unavailable model failed the setup: {text}");
        }
        if calls.load(Ordering::SeqCst) > 0 && text.contains("\"working\"") {
            break text;
        }
        if started.elapsed() > Duration::from_secs(15) {
            panic!(
                "setup did not stay open after {} model calls: {text}",
                calls.load(Ordering::SeqCst)
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let mut cancelled = false;
    if let Ok(entries) = fs::read_dir(dir.join("environment-setup").join("sessions")) {
        for entry in entries.flatten() {
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            if String::from_utf8_lossy(&bytes).contains("cancelled") {
                cancelled = true;
            }
        }
    }
    assert!(!cancelled, "unavailable model cancelled the setup computer");
    assert!(phase.contains("\"working\""));
    fs::write(
        dir.join("unavailable-report"),
        format!("kept calls={} cancelled=0", calls.load(Ordering::SeqCst)),
    )
    .unwrap();
    let _ = studio;
}

#[test]
fn a_held_candidate_does_no_environment_work_until_it_has_traffic() {
    if std::env::var_os("OA_ENV_FENCE_ROLE").is_some() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    plant_unfinished(dir.path());
    let before = snapshot(dir.path());
    let child = crate::fence::tests::spawn(dir.path(), "held", &[]);
    let mut child = StopChild(Some(child));
    let report = wait_report(child.0.as_mut().unwrap(), &dir.path().join("held-report"));
    assert_eq!(
        report.trim(),
        "calls=0 commands=0 deleted=0 listed=1",
        "held report: {report}"
    );
    assert!(
        !dir.path()
            .join("environment-studio")
            .join("execution.json")
            .exists(),
        "a candidate with no traffic took the environment lease"
    );
    drop(child);
    assert_eq!(
        before,
        snapshot(dir.path()),
        "a held candidate changed environment files"
    );

    let child = crate::fence::tests::spawn(dir.path(), "serving", &[]);
    let mut child = StopChild(Some(child));
    let report = wait_report(
        child.0.as_mut().unwrap(),
        &dir.path().join("serving-report"),
    );
    assert!(
        report.starts_with("progress calls=") && !report.contains("calls=0"),
        "serving report: {report}"
    );
}

#[test]
fn an_unavailable_model_leaves_the_setup_in_place() {
    if std::env::var_os("OA_ENV_FENCE_ROLE").is_some() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    plant_unfinished(dir.path());
    let child = crate::fence::tests::spawn(dir.path(), "unavailable", &[]);
    let mut child = StopChild(Some(child));
    let report = wait_report(
        child.0.as_mut().unwrap(),
        &dir.path().join("unavailable-report"),
    );
    assert!(
        report.starts_with("kept calls=") && !report.contains("calls=0"),
        "unavailable report: {report}"
    );
    assert!(report.contains("cancelled=0"));
}
