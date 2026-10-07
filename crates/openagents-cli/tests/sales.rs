//! Private sales operations through the shipped command, with scratch home/state.
use serde_json::{Value, json};
use std::fs;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn run(&self, credential: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(["--json", "sales"])
            .args(args)
            .arg("--root")
            .arg(self.dir.path().join("host"))
            .arg("--credential")
            .arg(self.dir.path().join(credential))
            .env("HOME", self.dir.path())
            .env("OPENAGENTS_TASKS", self.dir.path().join("tasks"))
            .current_dir(self.dir.path())
            .output()
            .unwrap()
    }
    fn ok(&self, credential: &str, args: &[&str]) -> Value {
        let output = self.run(credential, args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn apply(
        &self,
        credential: &str,
        name: &str,
        lead: Option<&str>,
        revision: u64,
        operation: Value,
    ) -> Value {
        let path = self.dir.path().join(format!("{name}.json"));
        fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema":"openagents.sales.pipeline-command.v1", "id":name,
                "lead":lead, "expected_revision":revision,"operation":operation
            }))
            .unwrap(),
        )
        .unwrap();
        self.ok(credential, &["apply", "--input", path.to_str().unwrap()])
    }
}

#[test]
fn private_pipeline_restart_handoff_authorization_replay_and_suppression() {
    let f = Fixture {
        dir: tempfile::tempdir().unwrap(),
    };
    f.ok("owner-token", &["init", "--owner", "founder"]);
    let collaborator = f.dir.path().join("collaborator-token");
    f.ok(
        "owner-token",
        &[
            "issue",
            "--human",
            "collaborator",
            "--role",
            "writer",
            "--new-credential",
            collaborator.to_str().unwrap(),
        ],
    );
    let stranger = f.dir.path().join("stranger-token");
    f.ok(
        "owner-token",
        &[
            "issue",
            "--human",
            "stranger",
            "--role",
            "reader",
            "--new-credential",
            stranger.to_str().unwrap(),
        ],
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let created = f.apply("owner-token", "synthetic-lead", None, 0, json!({
        "kind":"create", "ownership_acceptance":"founder accepted synthetic responsibility",
        "input":{"contact":"email:synthetic@pipeline.invalid", "source":"synthetic introduction", "source_at":now,
            "details":{"account":"synthetic account", "jurisdiction":"synthetic jurisdiction",
                "permission":{"state":"granted","reference":"synthetic-consent","recorded_at":now,"expires_at":now+3600,"channels":["email"]},
                "workflow":"synthetic maintenance", "baseline_reference":"synthetic private baseline",
                "data":{"recipients":["human:founder","human:collaborator"],"permitted_use":"one synthetic pilot", "retain_until":now+7200},
                "stage":"qualified","next":{"description":"review synthetic scope","due_at":now+1800},"customer_decision":null,"readers":[]}}
    }));
    let lead = created["lead"].as_str().unwrap();
    let original = f.ok("owner-token", &["show", "--lead", lead]);
    assert_eq!(original["responsible_human"], "founder");
    assert_eq!(
        original["details"]["permission"]["reference"],
        "synthetic-consent"
    );
    assert_eq!(original["details"]["next"]["due_at"], now + 1800);
    let bytes = fs::read(f.dir.path().join("synthetic-lead.json")).unwrap();
    assert_eq!(
        f.ok("owner-token", &["apply", "--input", "synthetic-lead.json"]),
        created
    );
    assert_eq!(
        fs::read(f.dir.path().join("synthetic-lead.json")).unwrap(),
        bytes
    );
    assert!(
        !f.run("stranger-token", &["show", "--lead", lead])
            .status
            .success()
    );
    let unauthorized = f.dir.path().join("forbidden-export.json");
    assert!(
        !f.run(
            "stranger-token",
            &[
                "export",
                "--lead",
                lead,
                "--output",
                unauthorized.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    assert!(!unauthorized.exists());
    f.apply(
        "owner-token",
        "propose",
        Some(lead),
        1,
        json!({"kind":"propose_handoff","target":"collaborator","reference":"synthetic proposal"}),
    );
    assert_eq!(
        f.ok("collaborator-token", &["show", "--lead", lead])["responsible_human"],
        "founder"
    );
    let accepted = f.apply(
        "collaborator-token",
        "accept",
        Some(lead),
        2,
        json!({"kind":"accept_handoff","reference":"collaborator accepted synthetic scope"}),
    );
    assert_eq!(
        f.ok("collaborator-token", &["show", "--lead", lead])["responsible_human"],
        "collaborator"
    );
    assert_eq!(
        f.ok("collaborator-token", &["apply", "--input", "accept.json"]),
        accepted
    );
    f.apply(
        "collaborator-token",
        "delete",
        Some(lead),
        3,
        json!({"kind":"delete","reference":"synthetic contact asked to stop"}),
    );
    assert_eq!(
        f.ok(
            "owner-token",
            &[
                "suppressed",
                "--contact",
                "email:SYNTHETIC@pipeline.invalid"
            ]
        )["suppressed"],
        true
    );
    assert!(
        !fs::read_to_string(f.dir.path().join("host/sales/state.json"))
            .unwrap()
            .contains("synthetic@pipeline.invalid")
    );
    assert_eq!(f.ok("owner-token", &["list"])["records"], json!([]));
    let token = fs::read_to_string(f.dir.path().join("owner-token")).unwrap();
    assert!(!original.to_string().contains(&token));
    let invalid = f.run("owner-token", &["list", "--limit", "101"]);
    assert_eq!(invalid.status.code(), Some(64));
    let state_path = f.dir.path().join("host/sales/state.json");
    let before = fs::read(&state_path).unwrap();
    for command in ["show", "export"] {
        for other in ["--sale", "--assignment"] {
            let denied = f.run(
                "owner-token",
                &[
                    command,
                    "--lead",
                    lead,
                    "--journey",
                    "private-journey",
                    other,
                    "private-record",
                ],
            );
            assert_eq!(denied.status.code(), Some(64));
            let output = format!(
                "{}{}",
                String::from_utf8_lossy(&denied.stdout),
                String::from_utf8_lossy(&denied.stderr)
            );
            assert!(output.contains("Select one sales record scope."));
            assert!(!output.contains("private-journey"));
            assert_eq!(fs::read(&state_path).unwrap(), before);
        }
    }
}
