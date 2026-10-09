//! Isolated native operator lifecycle checks.

use super::*;
use coder_host::cloud::Cloud;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[test]
fn private_readers_refuse_links_and_special_files() {
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{PermissionsExt, symlink},
    };
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let source = root.join("credential");
    fs::write(&source, b"synthetic-private-value").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        read_private(&source, 128).unwrap(),
        b"synthetic-private-value"
    );
    let link = root.join("link");
    symlink(&source, &link).unwrap();
    assert!(read_private(&link, 128).is_err());
    let hardlink = root.join("hardlink");
    fs::hard_link(&source, &hardlink).unwrap();
    assert_eq!(read_private(&source, 128), Err(Code::Forbidden));
    fs::remove_file(hardlink).unwrap();
    let alias = root.join("alias");
    symlink(&root, &alias).unwrap();
    assert!(read_private(&alias.join("credential"), 128).is_err());
    let fifo = root.join("fifo");
    let path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: the path is a valid scratch-only C string.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert_eq!(read_private(&fifo, 128), Err(Code::Forbidden));
}

#[test]
fn real_adapters_require_selected_credentials_and_qualified_executor_identity() {
    let f = fixture(Fake::default());
    let mut profile = f.policy.profiles["fixture"].clone();
    let token = f.owner.0.root.join("synthetic-boat-token");
    write(&token, b"synthetic-provider-token").unwrap();
    profile.adapter = Adapter::Boat {
        origin: "https://127.0.0.1:9".into(),
        token_file: token,
    };
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_none()
    );
    let credential = f.owner.0.root.join("synthetic-codex-credential");
    write(&credential, b"synthetic-engine-credential").unwrap();
    profile
        .credentials
        .insert("OPENAI_API_KEY".into(), credential.clone());
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_some()
    );
    for executor in ["claude", "microcoder", "other"] {
        let mut unqualified = profile.clone();
        unqualified.executor = executor.into();
        assert!(
            crate::operator_adapters::configured(&unqualified)
                .unwrap()
                .is_none()
        );
    }
    let mut integrated = profile.clone();
    integrated.mode = Mode::Integrated;
    assert!(
        crate::operator_adapters::configured(&integrated)
            .unwrap()
            .is_none()
    );
    let mut ambient_home = profile.clone();
    ambient_home
        .credentials
        .insert("HOME".into(), credential.clone());
    assert!(
        crate::operator_adapters::configured(&ambient_home)
            .unwrap()
            .is_none()
    );
    write(&credential, b" \n").unwrap();
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_none()
    );
    write(&credential, b"synthetic-engine-credential").unwrap();
    profile.credentials.clear();
    profile
        .credentials
        .insert("OA_CODEX_AUTH".into(), credential);
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_none()
    );
    write(
        &profile.credentials["OA_CODEX_AUTH"],
        br#"{"tokens":{"access_token":"synthetic-explicit-auth"}}"#,
    )
    .unwrap();
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_some()
    );
}

#[test]
fn claude_code_is_an_admitted_engine_that_never_takes_a_claude_login() {
    let f = fixture(Fake::default());
    let mut profile = f.policy.profiles["fixture"].clone();
    let token = f.owner.0.root.join("synthetic-boat-token");
    write(&token, b"synthetic-provider-token").unwrap();
    profile.adapter = Adapter::Boat {
        origin: "https://127.0.0.1:9".into(),
        token_file: token,
    };
    profile.executor = crate::claude::ENGINE.into();
    // No engine credential: the login lives inside the user's computer.
    assert!(
        crate::operator_adapters::configured(&profile)
            .unwrap()
            .is_some()
    );
    // The user's own API key is a separate custody class and is admitted.
    let key = f.owner.0.root.join("synthetic-anthropic-key");
    write(&key, b"synthetic-api-key").unwrap();
    let mut keyed = profile.clone();
    keyed
        .credentials
        .insert(crate::claude::API_KEY.into(), key.clone());
    assert!(
        crate::operator_adapters::configured(&keyed)
            .unwrap()
            .is_some()
    );
    // A claude.ai login under the API key's name is refused outright.
    write(&key, format!("sk-ant-oat01-{}", "s4".repeat(40)).as_bytes()).unwrap();
    assert!(crate::operator_adapters::configured(&keyed).is_err());
    // The subscription-token variable is never an injectable credential.
    let mut oauth = profile.clone();
    oauth
        .credentials
        .insert("CLAUDE_CODE_OAUTH_TOKEN".into(), key.clone());
    assert!(
        crate::operator_adapters::configured(&oauth)
            .unwrap()
            .is_none()
    );
    let mut policy = f.policy.clone();
    policy.profiles.insert("fixture".into(), oauth);
    assert!(policy.validate().is_err());
    let mut codex_key = profile.clone();
    codex_key.credentials.insert("OPENAI_API_KEY".into(), key);
    assert!(
        crate::operator_adapters::configured(&codex_key)
            .unwrap()
            .is_none()
    );
}

#[test]
fn first_cancellation_evidence_survives_distinct_and_legacy_retries() {
    let f = fixture(Fake::default());
    let store = f.owner.store().unwrap();
    let lease = store.lease("first").unwrap();
    let r = Record::new(
        "first",
        spec(&f.policy.profiles["fixture"], "Synthetic", 60),
    )
    .unwrap();
    lease.save(&r).unwrap();
    let raw =
        workspace::read_bounded(&store.root().join("first.json"), crate::MAX_RECORD_BYTES).unwrap();
    let pin = workspace::digest(&raw);
    let original = json!({"request":"first-cancel","reason":"Synthetic first reason"});
    store
        .cancel_exact_evidence("first", &pin, &original)
        .unwrap();
    let bytes = store.cancellation_evidence("first").unwrap().unwrap();
    store
        .cancel_exact_evidence("first", &pin, &original)
        .unwrap();
    assert!(
        store
            .cancel_exact_evidence(
                "first",
                &pin,
                &json!({"request":"second-cancel","reason":"Synthetic changed reason"})
            )
            .is_err()
    );
    store.cancel_exact("first", &pin).unwrap();
    store.cancel("first").unwrap();
    assert_eq!(
        store.cancellation_evidence("first").unwrap().unwrap(),
        bytes
    );
}

#[derive(Clone, Default)]
struct Fake {
    dispatches: Arc<AtomicU32>,
    recoveries: Arc<AtomicU32>,
    restarts: Arc<AtomicU32>,
    lost: bool,
    running: bool,
    cleanup_unknown: bool,
    /// The saved image each provision started from (ENV-06).
    provisioned: Arc<Mutex<Vec<Option<String>>>>,
    image_missing: Arc<AtomicBool>,
}
impl Backend for Fake {
    async fn resolve(&self, r: &mut Record) -> crate::Result<()> {
        if r.environment.is_some() && self.image_missing.load(Ordering::SeqCst) {
            return Err("Synthetic saved image is missing.".into());
        }
        Ok(())
    }
    async fn provision(&self, r: &mut Record) -> crate::Result<String> {
        self.provisioned
            .lock()
            .unwrap()
            .push(r.environment.as_ref().map(|p| p.image.image_id.clone()));
        Ok("synthetic-resource".into())
    }
    async fn dispatch(&self, _: &Record) -> crate::Result<crate::Task> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        if self.lost {
            return Err("Synthetic lost dispatch response.".into());
        }
        Ok(crate::Task {
            id: "synthetic-task".into(),
            conversation: Some("synthetic-session".into()),
        })
    }
    async fn recover(&self, _: &Record) -> crate::Result<Option<crate::Task>> {
        self.recoveries.fetch_add(1, Ordering::SeqCst);
        Ok(Some(crate::Task {
            id: "synthetic-task".into(),
            conversation: Some("synthetic-session".into()),
        }))
    }
    async fn poll(&self, _: &Record) -> crate::Result<crate::Observation> {
        if self.running {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(crate::Observation {
            events: if self.running {
                vec![]
            } else {
                vec![json!({"event":"delta","text":"original synthetic result"})]
            },
            cursor: None,
            end: (!self.running).then(|| {
                Ok(json!({"reply":"original synthetic result","model":"synthetic-served"}))
            }),
        })
    }
    async fn cancel(&self, _: &Record) -> crate::Result<()> {
        Ok(())
    }
    async fn restart(&self, _: &Record) -> crate::Result<()> {
        self.restarts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn collect(&self, _: &Record) -> crate::Result<Option<Value>> {
        use base64::Engine;
        Ok(Some(
            json!({"patch":base64::engine::general_purpose::STANDARD.encode(b"original synthetic patch\n")}),
        ))
    }
    async fn cleanup(&self, _: &Record) -> crate::Result<Option<Value>> {
        if self.cleanup_unknown {
            Err("Synthetic cleanup unknown.".into())
        } else {
            Ok(Some(json!({"cost_usd":0.002,"provider":"synthetic"})))
        }
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    owner: Operator,
    principal: Principal,
    backend: Fake,
    authority: Arc<AtomicBool>,
    source: PathBuf,
    policy: Policy,
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", root)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "Synthetic Git setup refused.");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn fixture(backend: Fake) -> Fixture {
    fixture_with(backend, |_, _| {})
}
fn fixture_with(backend: Fake, edit: impl FnOnce(&Path, &mut Profile)) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap();
    let source = path.join("source");
    directory(&source).unwrap();
    git(&source, &["init", "-q"]);
    fs::write(source.join("fixture.txt"), "Synthetic source\n").unwrap();
    git(&source, &["add", "fixture.txt"]);
    git(
        &source,
        &[
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=synthetic@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Synthetic source",
        ],
    );
    let revision = git(&source, &["rev-parse", "HEAD"]);
    let source_digest = workspace::source_identity(&source, &revision, &[], &[]).unwrap();
    let principal = Principal {
        device: "d".repeat(64),
        grant: Some("a".repeat(64)),
        epoch: Some(1),
    };
    let profile = Profile {
        workspace: "checkout".into(),
        project: "synthetic".into(),
        cwd: source.clone(),
        source_revision: revision,
        source_digest,
        repository: None,
        branch: None,
        paths: vec![],
        include: vec![],
        pool: "fixture-pool".into(),
        placement: Placement::Boat,
        mode: Mode::Coder,
        executor: "codex".into(),
        model: Some("synthetic-requested".into()),
        reasoning: None,
        max_timeout_seconds: 600,
        size: "small".into(),
        template: None,
        credentials: BTreeMap::new(),
        adapter: Adapter::Unavailable,
    };
    let mut profile = profile;
    edit(&path, &mut profile);
    let policy = Policy {
        schema: "openagents.coder.cloud-operator.v1".into(),
        operators: vec![Assignment {
            device: principal.device.clone(),
            workspace: "checkout".into(),
            project: "synthetic".into(),
            profiles: vec!["fixture".into()],
        }],
        profiles: BTreeMap::from([("fixture".into(), profile)]),
    };
    let authority = Arc::new(AtomicBool::new(true));
    let check = authority.clone();
    let owner = Operator::new(
        path.join("operator"),
        policy.clone(),
        Arc::new(move |_| check.load(Ordering::SeqCst)),
    )
    .unwrap()
    .with_backend("fixture", backend.clone())
    .unwrap();
    Fixture {
        _root: root,
        owner,
        principal,
        backend,
        authority,
        source,
        policy,
    }
}
fn submit(f: &Fixture) -> Operation {
    let p = &f.policy.profiles["fixture"];
    Operation::CloudSubmit {
        intent: dto::Submit {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            profile: "fixture".into(),
            profile_revision: Operator::profile_revision(p).unwrap(),
            source_digest: p.source_digest.clone(),
            prompt: "Original explicit operator task".into(),
            timeout_seconds: 600,
        },
    }
}
fn accepted(outcome: Outcome) -> dto::Accepted {
    match outcome {
        Outcome::CloudAccepted { accepted } => accepted,
        _ => panic!("Expected a native operator receipt."),
    }
}
fn read(f: &Fixture, id: &str) -> dto::Job {
    match f
        .owner
        .execute(
            "read",
            &f.principal,
            &Operation::CloudRead {
                query: dto::ReadQuery {
                    workspace: "checkout".into(),
                    project: "synthetic".into(),
                    job: id.into(),
                    revision: None,
                },
            },
        )
        .unwrap()
    {
        Outcome::CloudRead { job } => *job,
        _ => panic!("Expected a native job."),
    }
}
fn wait(f: &Fixture, id: &str, state: &str) -> dto::Job {
    for _ in 0..500 {
        let job = read(f, id);
        if job.state == state
            && (!matches!(state, "completed" | "failed" | "cancelled")
                || job.cleanup == "confirmed")
        {
            return job;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let job = read(f, id);
    panic!(
        "Synthetic native job did not reach expected state: {} / {} / {:?}",
        job.state, job.cleanup, job.error
    )
}

#[test]
fn explicit_operator_membership_gates_reads_effects_and_source_changes() {
    let f = fixture(Fake::default());
    let mut stranger = f.principal.clone();
    stranger.device = "e".repeat(64);
    let query = Operation::CloudProjects {
        workspace: "checkout".into(),
    };
    assert_eq!(
        f.owner.execute("read", &stranger, &query),
        Err(Code::Forbidden)
    );
    assert_eq!(
        f.owner.execute("first", &stranger, &submit(&f)),
        Err(Code::Forbidden)
    );
    let projects = f.owner.execute("read", &f.principal, &query).unwrap();
    assert!(
        matches!(projects,Outcome::CloudProjects {projects} if projects.projects==["synthetic"])
    );
    fs::write(f.source.join("fixture.txt"), "Changed source\n").unwrap();
    assert_eq!(
        f.owner.execute("first", &f.principal, &submit(&f)),
        Err(Code::Stale)
    );
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 0);
}
#[test]
fn exact_request_retries_return_original_job_without_a_second_dispatch() {
    let f = fixture(Fake::default());
    let op = submit(&f);
    let first = accepted(f.owner.execute("first", &f.principal, &op).unwrap());
    let job = wait(&f, "first", "completed");
    assert_eq!(job.model.as_deref(), Some("synthetic-requested"));
    assert_eq!(job.served_model.as_deref(), Some("synthetic-served"));
    assert_eq!(job.cleanup, "confirmed");
    assert_eq!(
        accepted(f.owner.execute("first", &f.principal, &op).unwrap()),
        first
    );
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 1);
    let request_bytes = fs::read(f.owner.0.root.join("requests/first.json")).unwrap();
    assert_eq!(
        accepted(f.owner.execute("first", &f.principal, &op).unwrap()),
        first
    );
    assert_eq!(
        fs::read(f.owner.0.root.join("requests/first.json")).unwrap(),
        request_bytes
    );
    let mut changed = op.clone();
    if let Operation::CloudSubmit { intent } = &mut changed {
        intent.prompt.push_str(" changed");
    }
    assert_eq!(
        f.owner.execute("first", &f.principal, &changed),
        Err(Code::Conflict)
    );
}
#[test]
fn lost_provider_dispatch_follows_original_task_without_resubmission() {
    let f = fixture(Fake {
        lost: true,
        ..Fake::default()
    });
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    let job = wait(&f, "first", "dispatching");
    assert_eq!(job.cleanup, "unknown");
    for _ in 0..500 {
        if f.owner.store().unwrap().lease("first").is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    accepted(
        f.owner
            .execute(
                "follow",
                &f.principal,
                &Operation::CloudFollow {
                    intent: dto::Follow {
                        scope: read(&f, "first").scope,
                    },
                },
            )
            .unwrap(),
    );
    wait(&f, "first", "completed");
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(f.backend.recoveries.load(Ordering::SeqCst), 1);
}
#[test]
fn continuation_preserves_original_attempt_and_patch_bytes() {
    let f = fixture(Fake::default());
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    let first = wait(&f, "first", "completed");
    for _ in 0..500 {
        if f.owner.store().unwrap().lease("first").is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let store = f.owner.store().unwrap();
    let raw =
        workspace::read_bounded(&store.root().join("first.json"), crate::MAX_RECORD_BYTES).unwrap();
    let cancellation =
        json!({"request":"original-cancel","reason":"Synthetic retained cancellation reason"});
    store
        .cancel_exact_evidence("first", &workspace::digest(&raw), &cancellation)
        .unwrap();
    accepted(
        f.owner
            .execute(
                "continue",
                &f.principal,
                &Operation::CloudContinue {
                    intent: dto::Continue {
                        scope: read(&f, "first").scope,
                        prompt: "Explicit next operator turn".into(),
                    },
                },
            )
            .unwrap(),
    );
    let second = wait(&f, "first", "completed");
    assert_eq!(second.scope.attempt, 2);
    assert_eq!(f.backend.restarts.load(Ordering::SeqCst), 1);
    assert!(!store.cancellation_requested("first").unwrap());
    let cancellation_source = second
        .originals
        .iter()
        .find(|o| o.source == "attempt:1:cancellation")
        .unwrap()
        .clone();
    let Outcome::CloudOriginal { chunk } = f
        .owner
        .execute(
            "original-cancellation",
            &f.principal,
            &Operation::CloudOriginal {
                query: dto::OriginalQuery {
                    scope: second.scope.clone(),
                    original: cancellation_source,
                    cursor: None,
                    limit: 16384,
                },
            },
        )
        .unwrap()
    else {
        panic!("Expected original cancellation bytes.")
    };
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(chunk.data)
            .unwrap(),
        serde_json::to_vec(&cancellation).unwrap()
    );
    let original = second
        .originals
        .iter()
        .find(|o| o.source == "attempt:1:artifact:changes.patch")
        .unwrap()
        .clone();
    let answer = f
        .owner
        .execute(
            "original",
            &f.principal,
            &Operation::CloudOriginal {
                query: dto::OriginalQuery {
                    scope: second.scope.clone(),
                    original,
                    cursor: None,
                    limit: 16384,
                },
            },
        )
        .unwrap();
    match answer {
        Outcome::CloudOriginal { chunk } => assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(chunk.data)
                .unwrap(),
            b"original synthetic patch\n"
        ),
        _ => panic!("Expected original cloud bytes."),
    };
    assert_eq!(
        f.owner.execute(
            "stale",
            &f.principal,
            &Operation::CloudCancel {
                intent: dto::Cancel {
                    scope: first.scope,
                    reason: "Stale attempt".into()
                }
            }
        ),
        Err(Code::Stale)
    );
}
#[test]
fn cancellation_fences_the_record_while_its_worker_holds_the_job_lease() {
    let f = fixture(Fake {
        running: true,
        ..Fake::default()
    });
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    let current = wait(&f, "first", "running");
    let op = Operation::CloudCancel {
        intent: dto::Cancel {
            scope: current.scope,
            reason: "Synthetic cancellation".into(),
        },
    };
    let first = accepted(f.owner.execute("cancel", &f.principal, &op).unwrap());
    assert_eq!(first.state, "cancellation_requested");
    let job = wait(&f, "first", "cancelled");
    assert_eq!(job.cancellation, "requested");
    assert_eq!(job.cleanup, "confirmed");
    assert_eq!(
        accepted(f.owner.execute("cancel", &f.principal, &op).unwrap()),
        first
    );
}
#[test]
fn revoked_operator_policy_blocks_retained_job_and_recovery_disclosure() {
    let f = fixture(Fake::default());
    let first = accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    wait(&f, "first", "completed");
    let admission = dto::Admission::from(&first.scope);
    f.owner
        .admit_recovery(&f.principal.device, &admission)
        .unwrap();
    let mut revoked = f.policy.clone();
    revoked.operators.clear();
    f.owner.replace_policy(revoked).unwrap();
    assert_eq!(
        f.owner.admit_recovery(&f.principal.device, &admission),
        Err(Code::Forbidden)
    );
    assert_eq!(
        f.owner.execute("first", &f.principal, &submit(&f)),
        Err(Code::Forbidden)
    );
}
#[test]
fn fresh_native_standing_refuses_deferred_provider_effects() {
    let f = fixture(Fake::default());
    f.authority.store(false, Ordering::SeqCst);
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    std::thread::sleep(Duration::from_millis(100));
    let job = read(&f, "first");
    assert_eq!(job.state, "created");
    assert_eq!(job.cleanup, "unknown");
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 0);
}
#[test]
fn gce_integrated_mode_and_arbitrary_original_paths_refuse() {
    let f = fixture(Fake::default());
    let mut policy = f.policy.clone();
    let p = policy.profiles.get_mut("fixture").unwrap();
    p.placement = Placement::Gce;
    p.mode = Mode::Integrated;
    assert!(policy.validate().is_err());
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    let job = wait(&f, "first", "completed");
    let mut original = job.originals[0].clone();
    original.source = "artifact:../../private".into();
    assert_eq!(
        f.owner.execute(
            "original",
            &f.principal,
            &Operation::CloudOriginal {
                query: dto::OriginalQuery {
                    scope: job.scope,
                    original,
                    cursor: None,
                    limit: 16384
                }
            }
        ),
        Err(Code::Malformed)
    );
}
#[test]
fn changed_profile_cannot_run_an_old_registered_driver() {
    let mut f = fixture(Fake::default());
    let p = f.policy.profiles.get_mut("fixture").unwrap();
    p.model = Some("changed-requested".into());
    f.owner.replace_policy(f.policy.clone()).unwrap();
    let catalog = f
        .owner
        .execute(
            "catalog",
            &f.principal,
            &Operation::CloudCatalog {
                query: dto::CatalogQuery {
                    workspace: "checkout".into(),
                    project: "synthetic".into(),
                },
            },
        )
        .unwrap();
    assert!(
        matches!(catalog,Outcome::CloudCatalog{catalog} if catalog.profiles[0].availability=="unavailable")
    );
    assert_eq!(
        f.owner.execute("first", &f.principal, &submit(&f)),
        Err(Code::Unavailable)
    );
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn profile_display_metadata_is_projected_and_changes_the_admission_pin() {
    let mut f = fixture(Fake::default());
    let original = f.policy.profiles["fixture"].clone();
    let old_pin = Operator::profile_revision(&original).unwrap();
    let profile = f.policy.profiles.get_mut("fixture").unwrap();
    profile.repository = Some("OpenAgentsInc/openagents".into());
    let repository_pin = Operator::profile_revision(profile).unwrap();
    assert_ne!(repository_pin, old_pin);
    profile.branch = Some("codex/environment-setup".into());
    assert_ne!(Operator::profile_revision(profile).unwrap(), repository_pin);
    profile.template = Some("oa-project-fixture-v1".into());
    assert_ne!(Operator::profile_revision(profile).unwrap(), old_pin);
    assert_eq!(profile.source_revision, original.source_revision);
    assert_eq!(profile.source_digest, original.source_digest);
    f.owner.replace_policy(f.policy.clone()).unwrap();
    let Outcome::CloudCatalog { catalog } = f
        .owner
        .execute(
            "catalog",
            &f.principal,
            &Operation::CloudCatalog {
                query: dto::CatalogQuery {
                    workspace: "checkout".into(),
                    project: "synthetic".into(),
                },
            },
        )
        .unwrap()
    else {
        panic!("The operator did not return its catalog.");
    };
    catalog.validate().unwrap();
    let projected = &catalog.profiles[0];
    assert_eq!(
        projected.repository.as_deref(),
        Some("OpenAgentsInc/openagents")
    );
    assert_eq!(projected.branch.as_deref(), Some("codex/environment-setup"));
    assert_eq!(projected.template.as_deref(), Some("oa-project-fixture-v1"));
    assert_eq!(projected.size, "small");
    assert_eq!(projected.source_revision, original.source_revision);
    assert_eq!(projected.source_digest, original.source_digest);
    assert_eq!(projected.availability, "unavailable");
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn older_operator_profiles_keep_their_encoding_and_refuse_invalid_labels() {
    let f = fixture(Fake::default());
    let original = &f.policy.profiles["fixture"];
    let encoded = serde_json::to_value(original).unwrap();
    assert!(encoded.get("repository").is_none());
    assert!(encoded.get("branch").is_none());
    let decoded: Profile = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded.repository, None);
    assert_eq!(decoded.branch, None);
    assert_eq!(serde_json::to_value(&decoded).unwrap(), encoded);
    assert_eq!(
        Operator::profile_revision(&decoded).unwrap(),
        Operator::profile_revision(original).unwrap()
    );
    let mut policy = f.policy.clone();
    policy.profiles.get_mut("fixture").unwrap().repository =
        Some("https://github.com/OpenAgentsInc/openagents".into());
    assert!(policy.validate().is_err());
    let profile = policy.profiles.get_mut("fixture").unwrap();
    profile.repository = Some("OpenAgentsInc/openagents".into());
    profile.branch = Some("../other".into());
    assert!(policy.validate().is_err());
}

#[test]
fn same_size_private_credential_replacement_changes_the_profile_pin() {
    let mut f = fixture(Fake::default());
    let path = f.owner.0.root.join("synthetic.credential");
    write(&path, b"synthetic-value-one").unwrap();
    f.policy
        .profiles
        .get_mut("fixture")
        .unwrap()
        .credentials
        .insert("SYNTHETIC_FIXTURE".into(), path.clone());
    let original = Operator::profile_revision(&f.policy.profiles["fixture"]).unwrap();
    write(&path, b"synthetic-value-two").unwrap();
    assert_ne!(
        Operator::profile_revision(&f.policy.profiles["fixture"]).unwrap(),
        original
    );
}
#[test]
fn original_chunks_preserve_large_bytes_and_refuse_media_or_cursor_substitution() {
    let f = fixture(Fake::default());
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    wait(&f, "first", "completed");
    let store = f.owner.store().unwrap();
    let lease = store.lease("first").unwrap();
    let mut r = lease.read("first").unwrap();
    r.result = Some(json!({"reply":"large original response ".repeat(4000)}));
    lease.save(&r).unwrap();
    drop(lease);
    let job = read(&f, "first");
    let original = job
        .originals
        .iter()
        .find(|o| o.source == "result")
        .unwrap()
        .clone();
    assert!(original.bytes > 16384);
    let mut query = dto::OriginalQuery {
        scope: job.scope.clone(),
        original: original.clone(),
        cursor: None,
        limit: 16384,
    };
    let mut bytes = Vec::new();
    loop {
        let outcome = f
            .owner
            .execute(
                "original",
                &f.principal,
                &Operation::CloudOriginal {
                    query: query.clone(),
                },
            )
            .unwrap();
        use base64::Engine;
        let Outcome::CloudOriginal { chunk } = outcome else {
            panic!("Expected original native bytes.")
        };
        bytes.extend(
            base64::engine::general_purpose::STANDARD
                .decode(&chunk.data)
                .unwrap(),
        );
        if let Some(next) = chunk.next {
            query.cursor = Some(next);
        } else {
            break;
        }
    }
    assert_eq!(digest(&bytes), original.digest);
    assert_eq!(bytes.len() as u64, original.bytes);
    query.cursor = None;
    query.original.media_type = "text/html".into();
    assert_eq!(
        f.owner.execute(
            "original",
            &f.principal,
            &Operation::CloudOriginal { query }
        ),
        Err(Code::Stale)
    );
    let path = store.root().join("first.artifacts/changes.patch");
    fs::remove_file(path).unwrap();
    assert_eq!(read(&f, "first").artifact_state, "damaged");
}

#[test]
fn a_claude_plan_login_runs_one_turn_while_own_keys_run_in_parallel() {
    let running = Fake {
        running: true,
        ..Fake::default()
    };
    let plan = fixture_with(running.clone(), |_, p| {
        p.executor = crate::claude::ENGINE.into();
    });
    accepted(
        plan.owner
            .execute("first", &plan.principal, &submit(&plan))
            .unwrap(),
    );
    assert_eq!(
        plan.owner
            .execute("second", &plan.principal, &submit(&plan)),
        Err(Code::Conflict)
    );
    let keyed = fixture_with(running, |root, p| {
        p.executor = crate::claude::ENGINE.into();
        directory(&root.join("keys")).unwrap();
        let key = root.join("keys/synthetic-anthropic-key");
        write(&key, b"synthetic-own-api-key").unwrap();
        p.credentials.insert(crate::claude::API_KEY.into(), key);
    });
    accepted(
        keyed
            .owner
            .execute("first", &keyed.principal, &submit(&keyed))
            .unwrap(),
    );
    accepted(
        keyed
            .owner
            .execute("second", &keyed.principal, &submit(&keyed))
            .unwrap(),
    );
}

#[path = "operator_environment_tests.rs"]
mod environment;
