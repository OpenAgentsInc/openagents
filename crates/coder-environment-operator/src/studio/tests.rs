use super::*;
use coder_environment::evidence::Redactor;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
use codex_transport::fake::{call, say};
use codex_transport::{Reply, TokenUsage, TransportError};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Mutex;

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
        .create(&github::Resolved {
            repository: github::RepoName::parse("example/repo").unwrap(),
            branch: "main".into(),
            commit: COMMIT.into(),
            private: false,
        })
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

    // Saving names the exact candidate the person saw.
    assert!(studio.save(&id, "stale").await.is_err());
    assert_eq!(studio.save(&id, &candidate.digest).await.unwrap(), 1);
    let v = studio.view(&id).unwrap();
    assert_eq!(v.summary.status, Status::Saved);
    assert_eq!(v.summary.saved, Some(1));
    assert!(v.versions[0].selected);
    assert!(matches!(
        v.records.last().unwrap().entry,
        Entry::Saved { number: 1 }
    ));
    // No key: Claude Code runs are not offered.
    assert!(studio.run_claude(&id, "Fix the tests").is_err());

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
