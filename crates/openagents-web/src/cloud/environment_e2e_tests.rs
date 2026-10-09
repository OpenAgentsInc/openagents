//! ENV-08 code acceptance: one isolated, simulated end-to-end run of
//! repository environment onboarding through the packaged owners and the
//! resident operator, over real HTTP through the web app.
//!
//! Setup with a failed install, an operator restart mid-flow, browser
//! steering, repair, and rerun; source materialized by the real source
//! script with local `sh` and `git`; a clean build; a fresh verification
//! (untouched baseline, then an idempotence fork); a reviewed Save and
//! select; a new task pinned to the saved version; complete evidence with
//! no gaps and no credential anywhere; and every machine's cleanup
//! acknowledged. Machines are the in-memory provider; identities are
//! synthetic.

use super::environment::{
    action_form, confirm_form, encode, enroll, get, link, operator_root, post,
};
use super::*;
use coder_environment::{self as env, digest};
use coder_environment_build::service::BuildRequest;
use coder_environment_operator::{Cadence, Owners, Providers, Service, attach};
use coder_environment_setup::service::{CommandInput, RecipeEdit};
use coder_environment_setup::{SetupRequest, SetupState, ToolResult};
use coder_environment_verify::plan::{
    Assertions, Check, CheckKind, CheckPlan, Idempotence, PLAN_SCHEMA, SourceStep, Startup,
};
use coder_environment_verify::service::VerifyRequest;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
use coder_working_computer::{Health, Phase as ComputerPhase, Principal, ServiceDecl};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

const CLOUD: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud";
const PANEL: &str = "/cloud/app/hosts/resident/cloud/synthetic-cloud/environment";
const SECRET: &str = "ghp_env08_synthetic_secret_0123456789";
const SESSION: &str = "setup-1";
const LOCK: &str = "version = 3\n";
const UNIT: &str = "cargo test -p app --locked";
const BROWSER: &str = "openagents browser run --flow smoke";
const INSTALL_V1: &str = "./install-v1.sh";
const INSTALL_V2: &str = "./install-v2.sh";

type Fake = Arc<FakeProvider>;

fn now_ms() -> u64 {
    coder_environment_operator::now_ms()
}

fn git(home: &Path, cwd: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Synthetic")
        .env("GIT_AUTHOR_EMAIL", "synthetic@example.invalid")
        .env("GIT_COMMITTER_NAME", "Synthetic")
        .env("GIT_COMMITTER_EMAIL", "synthetic@example.invalid")
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// A scratch origin repository: the project's source.
fn origin(scratch: &Path) -> env::SourcePin {
    let home = scratch.join("home");
    let origin = scratch.join("origin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(origin.join("src")).unwrap();
    git(&home, &origin, &["init", "-q", "."]);
    std::fs::write(origin.join("Cargo.lock"), LOCK).unwrap();
    std::fs::write(origin.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(origin.join(".gitignore"), "target/\n").unwrap();
    git(&home, &origin, &["add", "."]);
    git(&home, &origin, &["commit", "-q", "-m", "synthetic source"]);
    env::SourcePin {
        repository: Some(format!("file://{}", origin.display())),
        revision: git(&home, &origin, &["rev-parse", "HEAD"]),
        digest: "b".repeat(64),
    }
}

fn plan() -> CheckPlan {
    let check = |name: &str, kind, script: &str, assertions| Check {
        name: name.into(),
        kind,
        script: digest(script.as_bytes()),
        cwd: ".".into(),
        timeout_seconds: 600,
        assertions,
    };
    CheckPlan {
        schema: PLAN_SCHEMA.into(),
        profile: "rust-service".into(),
        source: SourceStep::Contained,
        offline: true,
        startup: Startup::Services {
            services: vec![ServiceDecl {
                name: "web".into(),
                command: "target/release/app serve".into(),
                cwd: ".".into(),
                health: Health::Http {
                    port: 8080,
                    path: "/health".into(),
                },
                ready_within_seconds: 30,
            }],
            readiness: vec![check(
                "browser",
                CheckKind::Browser,
                BROWSER,
                Assertions::Marker { min_passed: 1 },
            )],
        },
        checks: vec![check(
            "unit",
            CheckKind::Behavior,
            UNIT,
            Assertions::CargoTest { min_passed: 1 },
        )],
        idempotence: Idempotence {
            inventory: vec!["target".into(), "~/.cargo".into()],
        },
    }
}
fn plan_bytes(plan: &CheckPlan) -> Vec<u8> {
    serde_json::to_vec_pretty(plan).unwrap()
}

fn recipe() -> env::Recipe {
    env::Recipe {
        schema: env::RECIPE_SCHEMA.into(),
        base: env::ImagePin {
            provider: env::Provider::Boat,
            image_id: "oa-coder-runtime-20261008".into(),
            digest: "a".repeat(64),
        },
        runtime: env::ArtifactPin {
            revision: "rt-1".into(),
            digest: "c".repeat(64),
        },
        platform: env::Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: env::Script {
            cwd: ".".into(),
            digest: "e".repeat(64),
        },
        start: Default::default(),
        inputs: Default::default(),
        credential_names: Default::default(),
        qualification: env::Qualification {
            profile: "unset".into(),
            plan_digest: "f".repeat(64),
        },
        limits: env::Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 8,
            output_bytes: 1 << 24,
        },
        capture: Default::default(),
    }
}

/// The admitted Coder/Codex Boat profile the setup session runs under.
fn profile(pin: &env::SourcePin, cwd: &Path) -> coder_cloud::operator::Profile {
    coder_cloud::operator::Profile {
        workspace: "checkout".into(),
        project: operator_fixture::PROJECT.into(),
        cwd: cwd.to_path_buf(),
        source_revision: pin.revision.clone(),
        source_digest: pin.digest.clone(),
        repository: pin.repository.clone(),
        branch: None,
        paths: vec![],
        include: vec![],
        pool: "synthetic-local-pool".into(),
        placement: coder_cloud::Placement::Boat,
        mode: coder_cloud::Mode::Coder,
        executor: "codex".into(),
        model: None,
        reasoning: None,
        max_timeout_seconds: 3600,
        size: "small".into(),
        template: None,
        credentials: BTreeMap::from([
            ("GH_TOKEN".into(), cwd.join("synthetic-gh-token")),
            ("OA_CODEX_AUTH".into(), cwd.join("synthetic-codex-auth")),
        ]),
        adapter: coder_cloud::operator::Adapter::Boat {
            origin: "https://boat.example.invalid".into(),
            token_file: cwd.join("synthetic-boat-token"),
        },
    }
}

fn owner() -> Principal {
    Principal {
        workspace: "checkout".into(),
        principal: "alice".into(),
    }
}

fn inventory(files: &BTreeMap<String, String>, head: &str) -> String {
    let mut out = String::new();
    for p in ["target", "~/.cargo"] {
        let under: Vec<_> = files
            .iter()
            .filter(|(k, _)| k.as_str() == p || k.starts_with(&format!("{p}/")))
            .collect();
        if under.is_empty() {
            out.push_str(&format!("oa-inventory {p} absent\n"));
        } else {
            let d = digest(&serde_json::to_vec(&under).unwrap());
            out.push_str(&format!("oa-inventory {p} {d}\n"));
        }
    }
    out.push_str(&format!(
        "oa-inventory git:head {head}\noa-inventory done\n"
    ));
    out
}

/// One machine's command behavior. The source step runs the real source
/// script with local `sh` and `git` in a scratch checkout; the rest is
/// scripted.
fn machines(scratch: &Path, pin: &env::SourcePin) -> Fake {
    let provider = Arc::new(FakeProvider::new(
        BTreeMap::from([("GH_TOKEN".into(), SECRET.into())]),
        true,
    ));
    let sanitize = coder_environment_build::sanitize::Plan::new(
        &recipe().capture,
        "/tmp/oa-commands/sanitize",
    );
    let scratch = scratch.to_path_buf();
    let pin = pin.clone();
    let runs = AtomicUsize::new(0);
    provider.on_command(Box::new(move |spec, env, files| {
        let id = spec.id.as_str();
        let c = spec.command.as_str();
        let offline = env.get("CARGO_NET_OFFLINE").map(String::as_str) == Some("true");
        if id.ends_with("-locks") {
            let ok = files.get("Cargo.lock").map(String::as_str) == Some(LOCK);
            let line = if ok { "ok" } else { "mismatch" };
            FakeRun::exit(
                if ok { 0 } else { 3 },
                &format!("oa-lock {line} Cargo.lock\noa-lock done\n"),
                "",
            )
        } else if id.ends_with("-check-unit") {
            if !offline || !files.contains_key("target/release/app") {
                return FakeRun::exit(101, "", "error: no network\n");
            }
            FakeRun::exit(
                0,
                "running 5 tests\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
                "",
            )
        } else if id.ends_with("-check-browser") {
            FakeRun::exit(0, "OA-CHECK passed=2 failed=0\n", "")
        } else if id.ends_with("-inventory-before") || id.ends_with("-inventory-after") {
            FakeRun::exit(0, &inventory(files, &pin.revision), "")
        } else if id == "sanitize" {
            FakeRun::exit(0, &format!("sanitized {}\n", sanitize.digest()), "")
        } else if c.contains("oa-source head=") && c.contains("no-checkout") {
            // Verify mode on a booted image: the checkout is in the image.
            if !files.contains_key("src/main.rs") {
                return FakeRun::exit(3, "oa-source error=no-checkout\n", "");
            }
            let report = coder_environment_setup::source::Report::verified_for(&pin, false);
            FakeRun::exit(0, &report.render(), "")
        } else if c.contains("oa-source head=") {
            // Materialize: the real script, local sh and git, a fresh
            // checkout per machine.
            let work = scratch.join(format!("checkout-{}", runs.fetch_add(1, Ordering::SeqCst)));
            std::fs::create_dir_all(&work).unwrap();
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(c)
                .current_dir(&work)
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", scratch.join("home"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .envs(env)
                .output()
                .unwrap();
            if out.status.success() {
                for f in ["Cargo.lock", "src/main.rs", ".git/config"] {
                    files.insert(f.into(), std::fs::read_to_string(work.join(f)).unwrap());
                }
            }
            FakeRun::exit(
                out.status.code().unwrap_or(-1).into(),
                &String::from_utf8_lossy(&out.stdout),
                &String::from_utf8_lossy(&out.stderr),
            )
        } else if c.contains("install-v1") {
            FakeRun::exit(1, "resolving\n", "error: libfoo is missing\n")
        } else if c.contains("install-v2") {
            files.insert("target/release/app".into(), "binary".into());
            files.insert("~/.cargo/registry/dep".into(), "vendored".into());
            FakeRun::exit(0, "built\n", "")
        } else if c == "echo $GH_TOKEN" {
            let v = env.get("GH_TOKEN").cloned().unwrap_or_default();
            FakeRun::exit(0, &format!("{v}\n"), "")
        } else {
            FakeRun::exit(0, "", "")
        }
    }));
    provider
}

fn custody() -> coder_environment_operator::Custody {
    Arc::new(|names: &BTreeSet<String>| {
        let mut r = coder_environment::evidence::Redactor::new();
        if names.contains("GH_TOKEN") {
            r.select(SECRET).map_err(|e| e.to_string())?;
        }
        Ok(r)
    })
}

/// The resident host's durable identity: what survives a restart.
struct Boot {
    private: PathBuf,
    relay_url: String,
    relay: tokio::task::JoinHandle<()>,
    device: String,
    access: PathBuf,
    secret: PathBuf,
    workspaces: BTreeMap<String, PathBuf>,
}

/// One running resident: the host, its operator, and the packaged owners.
struct Running {
    host: Option<coder_host::Running>,
    service: Arc<Service>,
    owners: Arc<Owners<Fake>>,
}
impl Running {
    /// Stop the owners' loop and the host, as an operator shutdown does.
    async fn stop(mut self) {
        self.service.stop();
        assert!(!self.service.running());
        if let Some(host) = self.host.take() {
            host.shutdown().await;
        }
    }
}

async fn boot(fixture: &Fixture) -> Boot {
    let private = fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let checkout = private.join("resident-checkout");
    std::fs::create_dir(&checkout).unwrap();
    let workspaces = BTreeMap::from([("checkout".into(), checkout)]);
    let device = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let (relay_url, relay, _) = task_relay::start().await;
    let state = private.join("resident-access");
    let authority = coder_access::host::Host::new(&state, coder_access::RelayPolicy::LoopbackTest);
    let owner = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    authority
        .init(&coder_access::protocol::pubkey(&owner))
        .unwrap();
    let rights =
        coder_access::Rights::new([coder_access::Right::Observe, coder_access::Right::Operate])
            .unwrap();
    let invitation = authority
        .invite(&relay_url, rights, now(), now() + 3600)
        .unwrap();
    let parsed = coder_access::protocol::HostInvitation::parse(
        &invitation.code,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let pending = coder_access::client::prepare_redeem(
        &parsed,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let answer = authority
        .handle_redemption(&pending.event, || Ok(now()))
        .unwrap();
    let redeemed = coder_access::client::finish_redeem(
        &parsed,
        &pending,
        &answer,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let secret = private.join("resident-device.key");
    let access = private.join("resident-device.access");
    private_file(&secret, &device.secret_bytes());
    private_file(&access, &serde_json::to_vec(&redeemed).unwrap());
    let journal = private.join("browser-controls");
    std::fs::create_dir(&journal).unwrap();
    std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o700)).unwrap();
    Boot {
        private,
        relay_url,
        relay,
        device: coder_access::protocol::pubkey(&device),
        access,
        secret,
        workspaces,
    }
}

/// Start (or restart) the resident host with the operator loaded from its
/// retained policy and state, and the packaged owners composed with it.
async fn start(fixture: &mut Fixture, boot: &Boot, provider: &Fake, first: bool) -> Running {
    let private = &boot.private;
    let standing = coder_host::cloud::authority(coder_access::host::Host::new(
        private.join("resident-access"),
        coder_access::RelayPolicy::LoopbackTest,
    ))
    .unwrap();
    let operator = if first {
        operator_fixture::operator(private, standing, &boot.device, &boot.workspaces).unwrap()
    } else {
        operator_fixture::reload(private, standing).unwrap()
    };
    let owners = Arc::new(
        Owners::open(
            &private.join("operator-cloud"),
            Providers {
                setup: provider.clone(),
                build: provider.clone(),
                verify: provider.clone(),
            },
            custody(),
        )
        .unwrap(),
    );
    // Long ticks: the restart sweep runs at start; wake-ups drive the rest.
    let (operator, service) = attach(
        owners.clone(),
        operator,
        Cadence {
            tick: std::time::Duration::from_secs(3600),
        },
    )
    .unwrap();
    assert!(service.running());
    let inbox =
        coder::task::remote::Inbox::new(private.join("resident-tasks"), boot.workspaces.clone())
            .with_settings(private.join("unused-resident-settings"))
            .with_cloud(Arc::new(operator));
    let mut host = coder_host::config::Config::new(
        private.join("resident-access"),
        vec![boot.relay_url.clone()],
        7,
    );
    host.policy = coder_access::RelayPolicy::LoopbackTest;
    host.workspaces = boot.workspaces.clone();
    host.telemetry = false;
    let running = coder_host::start(host, Arc::new(inbox)).await.unwrap();
    let config = private.join("hosts.json");
    let document = json!({"schema":"openagents.cloud.host-bindings.v1","bindings":[{"id":"resident","account":"alice","workspace":"alice-personal","members_epoch":3,"host_workspace":"checkout","host_generation":7,"route":format!("tcp://{}",running.local_addr()),"access_file":boot.access,"device_secret":boot.secret}],"controls":{"directory":private.join("browser-controls"),"bindings":["resident"]}});
    if config.exists() {
        std::fs::remove_file(&config).unwrap();
    }
    private_file(&config, &serde_json::to_vec(&document).unwrap());
    fixture.config.cloud_hosts = Some(Arc::new(crate::cloud::hosts::Hosts::load(&config).unwrap()));
    fixture.site = crate::router(fixture.config.clone());
    Running {
        host: Some(running),
        service,
        owners,
    }
}

fn started(r: ToolResult) -> String {
    match r {
        ToolResult::CommandStarted { command } => command,
        other => panic!("expected a command, got {other:?}"),
    }
}
fn edit(script: &str, plan_digest: Option<&str>) -> RecipeEdit {
    RecipeEdit {
        install_script: script.into(),
        install_cwd: ".".into(),
        start: None,
        inputs: plan_digest.map(|_| env::Inputs {
            toolchain: None,
            locks: BTreeMap::from([("Cargo.lock".to_string(), digest(LOCK.as_bytes()))]),
        }),
        credential_names: None,
        qualification: plan_digest.map(|d| env::Qualification {
            profile: "rust-service".into(),
            plan_digest: d.into(),
        }),
        limits: None,
        capture: None,
    }
}
/// Run one setup command to its end and return what it printed.
async fn finished(
    o: &Owners<Fake>,
    command: String,
) -> coder_environment_setup::service::CommandView {
    for _ in 0..50 {
        let v = o.setup.poll(SESSION, &command, now_ms()).await.unwrap();
        if !matches!(
            v.command.run,
            coder_environment_setup::Run::Running { .. } | coder_environment_setup::Run::Requested
        ) {
            return v;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("command {command} did not finish");
}
/// Stage and confirm one panel action through the request book.
async fn act(fixture: &Fixture, cookies: &Cookies, action: &str, extra: &[(&str, &str)]) {
    let page = get(fixture, cookies, PANEL).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let mut fields = action_form(&page.body, action);
    for (k, v) in extra {
        fields.push(((*k).into(), (*v).into()));
    }
    let id = fields
        .iter()
        .find(|(k, _)| k == "request")
        .unwrap()
        .1
        .clone();
    let staged = post(fixture, cookies, PANEL, &encode(&fields)).await;
    assert_eq!(staged.status, StatusCode::SEE_OTHER, "{}", staged.body);
    let (target, input) = confirm_form(fixture, cookies, &id).await;
    let done = post(fixture, cookies, &target, &input).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(done.body.contains("Return to the project environment"));
}
fn holds(dir: &Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut found = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&p) else {
            continue;
        };
        for e in entries {
            let path = e.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file()
                && std::fs::read(&path)
                    .unwrap()
                    .windows(needle.len())
                    .any(|w| w == needle)
            {
                found.push(path);
            }
        }
    }
    found
}
/// Keyboard and narrow-screen basics of one page.
fn accessible(html: &str) {
    assert!(html.contains("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1"));
    assert!(!html.contains("user-scalable=no") && !html.contains("maximum-scale"));
    assert!(html.contains("href=\"#content\""), "no skip link");
    // Only the skip link's target leaves the tab order; no mouse-only
    // handlers.
    assert_eq!(html.matches("tabindex=\"-").count(), 1);
    let main = html.split("<main id=\"content\"").nth(1).unwrap();
    assert!(main.split('>').next().unwrap().contains("tabindex=\"-1\""));
    assert!(!html.contains("onclick="));
    assert!(
        !html.contains("style=\"width:"),
        "a fixed width breaks narrow screens"
    );
    // Every control has a programmatic label, and every form submits with
    // a real button.
    let labels: Vec<&str> = html
        .split("<label")
        .skip(1)
        .map(|tag| tag.split('>').next().unwrap())
        .collect();
    for tag in html.split("<textarea").skip(1) {
        let tag = tag.split('>').next().unwrap();
        let id = tag
            .split(" id=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("a textarea without an id: {tag}"));
        assert!(
            labels
                .iter()
                .any(|label| label.contains(&format!(" for=\"{id}\""))),
            "unlabelled {id}"
        );
    }
    for form in html.split("<form ").skip(1) {
        let form = form.split("</form>").next().unwrap();
        assert!(form.contains("<button"), "a form without a button: {form}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn packaged_onboarding_runs_end_to_end_with_restart_and_browser_acceptance() {
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let mut fixture = fixture().await;
    let scratch = tempfile::tempdir().unwrap();
    let scratch = scratch.path().canonicalize().unwrap();
    let pin = origin(&scratch);
    let provider = machines(&scratch, &pin);
    let boot = boot(&fixture).await;
    let root = operator_root(&fixture);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;

    // Packaged: the panel reads setup sessions instead of "unavailable".
    let running = start(&mut fixture, &boot, &provider, true).await;
    let o = running.owners.clone();
    assert_eq!(
        o.layout.verify_evidence(),
        root.join(coder_cloud::operator::VERIFY_EVIDENCE)
    );
    let e = env::Environment::new(
        "env-1",
        env::ProjectLink {
            workspace: "checkout".into(),
            project: operator_fixture::PROJECT.into(),
        },
        pin.clone(),
        recipe(),
        now_ms(),
    )
    .unwrap();
    o.setup.environments.create(&e).unwrap();
    let page = get(&fixture, &cookies, PANEL).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body
            .contains("No setup session has run for this environment yet.")
    );
    assert!(!page.body.contains("No setup owner is composed"));

    // Setup: source, a named-credential discovery command, a failed install.
    let profile = profile(&pin, &scratch);
    let request = SetupRequest {
        session: SESSION.into(),
        environment: "env-1".into(),
        owner: owner(),
        profile: operator_fixture::PROFILE.into(),
        objective: "Set up the synthetic Rust service.".into(),
        credential_names: ["GH_TOKEN".to_string()].into(),
        git_credential: Some("GH_TOKEN".into()),
        deadline_seconds: 600,
    };
    let s = o.setup.open(&request, &profile, now_ms()).await.unwrap();
    assert_eq!(s.state, SetupState::Discovering);
    let src = started(
        o.setup
            .materialize_source(SESSION, "q-src", 600, now_ms())
            .await
            .unwrap(),
    );
    let v = finished(&o, src).await;
    assert_eq!(
        v.command.run,
        coder_environment_setup::Run::Exited { code: 0 },
        "{v:?}"
    );
    let report = coder_environment_setup::source::Report::parse(&v.stdout).unwrap();
    assert!(report.verified(&pin) && report.fetched, "{report:?}");
    let probe = started(
        o.setup
            .run_command(
                SESSION,
                "q-probe",
                &CommandInput {
                    command: "echo $GH_TOKEN".into(),
                    cwd: ".".into(),
                    credential_names: ["GH_TOKEN".to_string()].into(),
                    git_auth: false,
                    timeout_seconds: 60,
                },
                now_ms(),
            )
            .await
            .unwrap(),
    );
    let v = finished(&o, probe).await;
    assert!(
        !v.stdout.contains(SECRET),
        "a credential reached the session view"
    );
    o.setup
        .update_recipe(SESSION, "q-r1", 1, &edit(INSTALL_V1, None), now_ms())
        .await
        .unwrap();
    let first = started(
        o.setup
            .run_install(SESSION, "q-i1", 2, 600, now_ms())
            .await
            .unwrap(),
    );
    let v = finished(&o, first).await;
    assert_eq!(
        v.command.run,
        coder_environment_setup::Run::Exited { code: 1 }
    );
    assert_eq!(v.stderr, "error: libfoo is missing\n");
    o.setup
        .pause(
            SESSION,
            "libfoo is missing. May I add it to the install?",
            now_ms(),
        )
        .await
        .unwrap();
    let before = o.setup.sessions().read(SESSION).unwrap();
    assert!(matches!(before.state, SetupState::AwaitingInput { .. }));
    let page = get(&fixture, &cookies, PANEL).await;
    assert!(page.body.contains("Waiting for your input:"));
    drop(o);

    // Operator restart mid-flow: the same records come back.
    running.stop().await;
    let running = start(&mut fixture, &boot, &provider, false).await;
    let o = running.owners.clone();
    let after = o.setup.sessions().read(SESSION).unwrap();
    assert_eq!(after.commands, before.commands);
    assert_eq!(after.state, before.state);
    assert_eq!(
        o.setup.environments.read("env-1").unwrap(),
        coder_environment::store::Store::under(root.join("environments"))
            .read("env-1")
            .unwrap()
    );
    let page = get(&fixture, &cookies, PANEL).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("Waiting for your input:"));
    accessible(&page.body);

    // Browser steering through the request book wakes the session.
    enroll(&fixture, &cookies).await;
    act(
        &fixture,
        &cookies,
        "steer",
        &[("text", "Yes, add libfoo and rerun the install.")],
    )
    .await;
    let mut woke = false;
    for _ in 0..200 {
        let s = o.setup.sessions().read(SESSION).unwrap();
        if !matches!(s.state, SetupState::AwaitingInput { .. }) {
            assert_eq!(s.steering.len(), 1);
            // The restart is disclosed: the first evidence segment was
            // interrupted, and the restarted owner opened a second.
            assert_eq!(s.segments.len(), 2);
            assert!(s.segments[0].interrupted && s.segments[0].sealed.is_none());
            woke = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(woke, "the packaged loop did not wake the steered session");

    // Repair and rerun, bound to the new revision; the plan is frozen.
    let plan = plan();
    for bytes in [UNIT.as_bytes(), BROWSER.as_bytes()] {
        o.seal_artifact(bytes).unwrap();
    }
    let plan_digest = o.seal_artifact(&plan_bytes(&plan)).unwrap();
    o.setup
        .update_recipe(
            SESSION,
            "q-r2",
            2,
            &edit(INSTALL_V2, Some(&plan_digest)),
            now_ms(),
        )
        .await
        .unwrap();
    let second = started(
        o.setup
            .run_install(SESSION, "q-i2", 3, 600, now_ms())
            .await
            .unwrap(),
    );
    let v = finished(&o, second).await;
    assert_eq!(
        v.command.run,
        coder_environment_setup::Run::Exited { code: 0 }
    );
    let s = o.setup.sessions().read(SESSION).unwrap();
    assert_eq!(s.state, SetupState::Installed { recipe_revision: 3 });
    assert_eq!(s.recipe_revisions, vec![2, 3]);
    let s = o.setup.end(SESSION, now_ms()).await.unwrap();
    assert!(s.state.terminal());
    assert!(
        s.segments[1].sealed.is_some(),
        "the live segment is sealed at the end"
    );

    // A clean build of the repaired revision on a fresh builder.
    let build = o
        .build(
            &BuildRequest {
                request_id: "build-req".into(),
                environment: "env-1".into(),
                owner: owner(),
                expected_draft_revision: 3,
                size: "small".into(),
            },
            now_ms(),
        )
        .await
        .unwrap();
    assert_eq!(
        build.phase,
        coder_environment_build::Phase::Ready,
        "{:?}",
        build.reason
    );
    let image = o
        .setup
        .environments
        .read("env-1")
        .unwrap()
        .builds
        .last()
        .unwrap()
        .image
        .clone()
        .unwrap();

    // A fresh verification: untouched baseline, then the idempotence fork.
    let job = o
        .verify(
            &VerifyRequest {
                request_id: "verify-req".into(),
                environment: "env-1".into(),
                build_id: build.inputs.build_id.clone(),
                owner: owner(),
                plan_digest: plan_digest.clone(),
                size: "small".into(),
            },
            now_ms(),
        )
        .await
        .unwrap();
    assert_eq!(
        job.verdict,
        Some(coder_environment_verify::Verdict::Passed),
        "{job:?}"
    );
    use coder_working_computer::VerifyRole;
    for role in [VerifyRole::Baseline, VerifyRole::Fork] {
        assert!(
            job.steps.iter().any(|s| s.role == role),
            "{role:?} did not run"
        );
    }

    // Reviewed Save and select of the exact displayed candidate.
    let page = get(&fixture, &cookies, PANEL).await;
    assert!(page.body.contains("Candidate for review"), "{}", page.body);
    assert!(page.body.contains(&image.image_id));
    accessible(&page.body);
    act(&fixture, &cookies, "promote", &[]).await;
    let saved = o.setup.environments.read("env-1").unwrap();
    assert_eq!(saved.versions.len(), 1);
    assert_eq!(saved.selection.active.as_deref(), Some("v1"));
    assert_eq!(saved.versions[0].image, image);
    let page = get(&fixture, &cookies, PANEL).await;
    assert!(page.body.contains("Version 1") && page.body.contains("data-state=\"selected\""));
    accessible(&page.body);

    // Complete evidence with no gaps, paged over HTTP.
    let summary = get(
        &fixture,
        &cookies,
        &link(&page.body, "/environment/evidence?q="),
    )
    .await;
    assert_eq!(summary.status, StatusCode::OK, "{}", summary.body);
    assert!(summary.body.contains("None disclosed"));
    let export = get(&fixture, &cookies, &link(&summary.body, "download=export")).await;
    let export: Value = serde_json::from_str(&export.body).unwrap();
    assert_eq!(export["summary"]["complete"], true, "{export}");
    assert_eq!(export["summary"]["gaps"], json!([]));
    assert_eq!(export["summary"]["children"].as_array().unwrap().len(), 2);

    // A new task run pins the saved version, started from its exact image.
    let new = format!("{CLOUD}/new");
    let compose = get(&fixture, &cookies, &new).await;
    let html = compose
        .body
        .split("<form ")
        .skip(1)
        .map(|f| f.split("</form>").next().unwrap())
        .find(|f| f.contains("name=\"request\""))
        .unwrap();
    let mut fields: Vec<(String, String)> = ["csrf", "request", "profile", "basis"]
        .iter()
        .map(|n| ((*n).to_string(), field(html, n)))
        .collect();
    fields.push((
        "task:prompt".into(),
        "Synthetic task on the saved environment".into(),
    ));
    fields.push(("timeout".into(), "60".into()));
    let task = fields[1].1.clone();
    assert_eq!(
        post(&fixture, &cookies, &new, &encode(&fields))
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    let (target, input) = confirm_form(&fixture, &cookies, &task).await;
    assert_eq!(
        post(&fixture, &cookies, &target, &input).await.status,
        StatusCode::OK
    );
    let pinned = coder_cloud::Store::under(root.join("jobs"))
        .read(&task)
        .unwrap()
        .environment
        .unwrap();
    assert_eq!((pinned.version_id.as_str(), pinned.number), ("v1", 1));
    assert_eq!(pinned.image, image);
    assert_eq!(pinned.source.revision, pin.revision);
    let held = provider.state.lock().unwrap().images[&image.image_id]
        .0
        .clone();
    assert_eq!(
        held.state,
        coder_working_computer::provider::ImageState::Ready
    );
    let view = get(&fixture, &cookies, &format!("{CLOUD}/jobs/{task}")).await;
    assert!(
        view.body.contains("Started from version 1"),
        "{}",
        view.body
    );

    // Every machine's cleanup is acknowledged.
    let store = coder_working_computer::store::Store::under(o.layout.computers());
    let ids = [
        s.computer.clone(),
        build.computer.clone(),
        job.baseline.computer.clone(),
        job.fork.computer.clone(),
    ];
    let records = std::fs::read_dir(o.layout.computers())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "json")
        })
        .count();
    assert_eq!(records, ids.len(), "setup, builder, baseline, fork");
    for id in &ids {
        let c = store.read(id).unwrap();
        assert_eq!(c.phase, ComputerPhase::Deleted, "{}", c.id);
        assert!(c.deletion.as_ref().is_some_and(|f| f.is_done()), "{}", c.id);
    }
    let build = o.builder.jobs.read(&build.id).unwrap();
    assert!(matches!(
        build.cleanup,
        coder_environment_build::Cleanup::Complete { .. }
    ));
    for m in [&job.baseline, &job.fork] {
        assert!(matches!(
            m.cleanup,
            coder_environment_verify::Cleanup::Complete { .. }
        ));
    }
    // A recovery visit over settled records changes nothing.
    let r = o.recover(now_ms()).await;
    assert!(
        r.builds.is_empty() && r.verifications.is_empty() && r.errors.is_empty(),
        "{r:?}"
    );

    // No credential value anywhere the operator, owners, or host retained.
    assert_eq!(
        holds(&boot.private, SECRET.as_bytes()),
        Vec::<PathBuf>::new()
    );

    running.stop().await;
    boot.relay.abort();
}
