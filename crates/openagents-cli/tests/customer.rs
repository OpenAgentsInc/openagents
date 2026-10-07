//! Commercial customer commands use private scratch state and a loopback gateway.
use receipts::purchase::{Context, PriceReference};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        path.to_str().unwrap().into()
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(["--json", "customer"])
            .args(args)
            .arg("--root")
            .arg(self.dir.path().join("customer"))
            .env("HOME", self.dir.path())
            .env("OPENAGENTS_API_KEY", "oak_unrelated-ambient.key")
            .env("OPENAGENTS_TASKS", self.dir.path().join("tasks"))
            .current_dir(self.dir.path())
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("oak_fixture."));
        assert!(!text.contains("private-refusal"));
        serde_json::from_str(&text).unwrap()
    }
    fn select(&self, origin: &str, alias: &str, account: &str, workspace: &str) -> Value {
        self.ok(&[
            "select",
            "--origin",
            origin,
            "--alias",
            alias,
            "--account",
            account,
            "--workspace",
            workspace,
            "--door",
            "decision-a",
        ])
    }
}
fn context(account: &str, workspace: &str, key: &str) -> Value {
    serde_json::to_value(Context {
        schema: receipts::purchase::SCHEMA.into(),
        account: account.into(),
        workspace: workspace.into(),
        payer_workspace: workspace.into(),
        tenant: workspace.into(),
        credential_reference: key.into(),
        membership_epoch: 1,
        workspace_members_epoch: 1,
        role: "owner".into(),
        door: "decision-a".into(),
        registry_digest: hash('a'),
        artifact_digest: hash('b'),
        price: PriceReference {
            version: "price-1".into(),
            currency: "USD".into(),
            policy: "observed-usage-v1".into(),
            terms_digest: hash('c'),
            maximum_usage_digest: hash('d'),
            maximum_charge: 100,
        },
        can_invoke: true,
        commercial: None,
        team_policy: None,
    })
    .unwrap()
}
fn hash(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}
struct Reply {
    route: String,
    token: &'static str,
    status: u16,
    body: Value,
}
fn read_context(workspace: &str, token: &'static str, body: Value) -> Reply {
    Reply {
        route: format!("GET /v1/workspaces/{workspace}/purchase-context/decision-a "),
        token,
        status: 200,
        body,
    }
}
fn server(replies: Vec<Reply>) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let effects = Arc::new(AtomicUsize::new(0));
    let count = effects.clone();
    let thread = std::thread::spawn(move || {
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut socket = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("Fixture request missing: {}: {e}", reply.route),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut block = [0; 2048];
            loop {
                let n = socket.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8(bytes).unwrap();
            assert!(
                text.starts_with(&reply.route),
                "Unexpected route: {}",
                text.lines().next().unwrap()
            );
            assert!(
                text.to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {}", reply.token))
            );
            assert!(!text.contains("oak_unrelated-ambient.key"));
            if text.lines().next().is_some_and(|line| {
                line.contains("/v1/account/referrers") || line.contains("/v1/account/acquisition")
            }) {
                assert!(
                    text.to_ascii_lowercase()
                        .contains("x-openagents-referral-account: ada")
                );
            }
            if text.starts_with("POST ") || text.starts_with("DELETE ") {
                count.fetch_add(1, Ordering::SeqCst);
            }
            let body = serde_json::to_vec(&reply.body).unwrap();
            write!(socket, "HTTP/1.1 {} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", reply.status, body.len()).unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    (origin, effects, thread)
}

#[test]
fn shipped_customer_commands_isolate_accounts_preserve_rotated_history_and_retired_liability() {
    let f = Fixture::new();
    for (alias, key) in [("ada", "oak_fixture.ada"), ("grace", "oak_fixture.grace")] {
        let path = f.file(alias, key.as_bytes());
        f.ok(&["import", "--alias", alias, "--input", &path]);
    }
    let ada = context("ada", "ada-personal", "ada");
    let replacement = context("ada", "ada-personal", "replacement");
    let account = || json!({"account":{"id":"ada","principals":[{"private":"oak_fixture.ada"}]},"workspaces":[]});
    let mut replies = vec![
        read_context("ada-personal", "oak_fixture.ada", ada.clone()),
        read_context("ada-personal", "oak_fixture.ada", ada.clone()),
        read_context("ada-personal", "oak_fixture.ada", ada.clone()),
        read_context("ada-personal", "oak_fixture.ada", ada.clone()),
        read_context(
            "grace-personal",
            "oak_fixture.grace",
            context("grace", "grace-personal", "grace"),
        ),
        read_context(
            "ada-team",
            "oak_fixture.ada",
            context("ada", "ada-team", "ada"),
        ),
        read_context("ada-personal", "oak_fixture.ada", ada),
        Reply {
            route: "GET /v1/account ".into(),
            token: "oak_fixture.ada",
            status: 200,
            body: account(),
        },
        Reply {
            route: "POST /v1/workspaces/ada-personal/keys/ada/rotate ".into(),
            token: "oak_fixture.ada",
            status: 201,
            body: json!({"key":{"id":"replacement","tenant":"ada-personal"},"key_token":"oak_fixture.replacement"}),
        },
        Reply {
            route: "GET /v1/account ".into(),
            token: "oak_fixture.replacement",
            status: 200,
            body: account(),
        },
        Reply {
            route: "GET /v1/workspaces/ada-personal ".into(),
            token: "oak_fixture.replacement",
            status: 200,
            body: json!({"workspace":{"id":"ada-personal","tenant":"ada-personal","members_epoch":1},"role":"owner"}),
        },
        read_context(
            "ada-personal",
            "oak_fixture.replacement",
            replacement.clone(),
        ),
        read_context(
            "ada-personal",
            "oak_fixture.replacement",
            replacement.clone(),
        ),
        read_context(
            "ada-personal",
            "oak_fixture.replacement",
            replacement.clone(),
        ),
        read_context(
            "ada-personal",
            "oak_fixture.replacement",
            replacement.clone(),
        ),
        read_context("ada-personal", "oak_fixture.replacement", replacement),
        Reply {
            route: "POST /v1/systemone ".into(),
            token: "oak_fixture.replacement",
            status: 503,
            body: json!({"private":"private-refusal"}),
        },
        Reply {
            route: "GET /v1/account ".into(),
            token: "oak_fixture.replacement",
            status: 200,
            body: account(),
        },
        Reply {
            route: "DELETE /v1/workspaces/ada-personal/keys/replacement ".into(),
            token: "oak_fixture.replacement",
            status: 200,
            body: json!({}),
        },
    ];
    for _ in 0..2 {
        replies.push(Reply {
            route: "GET /v1/workspaces/ada-personal/purchase-context/decision-a ".into(),
            token: "oak_fixture.replacement",
            status: 403,
            body: json!({"private":"private-refusal"}),
        });
    }
    let (origin, effects, thread) = server(replies);
    assert!(
        !f.run(&[
            "select",
            "--origin",
            &origin,
            "--alias",
            "ada",
            "--account",
            "grace",
            "--workspace",
            "ada-personal",
            "--door",
            "decision-a"
        ])
        .status
        .success()
    );
    f.select(&origin, "ada", "ada", "ada-personal");
    let input = f.file("request.json", &serde_json::to_vec(&json!({"model":"decision-a","state":"Private synthetic input","questions":{"ready":{"type":"noul","instructions":"Is the task ready?"}}})).unwrap());
    let quote = f.ok(&["quote", "--purchase", "original", "--input", &input]);
    assert_eq!(quote["quote"]["context"]["payer_workspace"], "ada-personal");
    let digest = quote["quote_digest"].as_str().unwrap();
    f.ok(&["approve", "--purchase", "original", "--digest", digest]);
    f.select(&origin, "grace", "grace", "grace-personal");
    assert!(!f.run(&["show", "--purchase", "original"]).status.success());
    assert_eq!(f.ok(&["history"])["purchases"], json!([]));
    f.select(&origin, "ada", "ada", "ada-team");
    assert!(!f.run(&["show", "--purchase", "original"]).status.success());
    f.select(&origin, "ada", "ada", "ada-personal");
    let intent = f.file("rotate.json", &serde_json::to_vec(&json!({"id":"rotate","origin":origin,"account":"ada","credential_alias":"ada","action":{"kind":"rotate","workspace":"ada-personal","key":"ada","output_alias":"replacement"}})).unwrap());
    assert_eq!(f.ok(&["change", "--input", &intent])["status"], "applied");
    assert_eq!(f.ok(&["credentials"])["operations"][0]["selected"], false);
    f.select(&origin, "replacement", "ada", "ada-personal");
    assert_eq!(
        f.ok(&["show", "--purchase", "original"])["quote"],
        quote["quote"]
    );
    assert!(
        !f.run(&["invoke", "--purchase", "original"])
            .status
            .success()
    );
    let q = f.ok(&["quote", "--purchase", "next", "--input", &input]);
    f.ok(&[
        "approve",
        "--purchase",
        "next",
        "--digest",
        q["quote_digest"].as_str().unwrap(),
    ]);
    assert!(!f.run(&["invoke", "--purchase", "next"]).status.success());
    let revoke = f.file("revoke.json", &serde_json::to_vec(&json!({"id":"revoke","origin":origin,"account":"ada","credential_alias":"replacement","action":{"kind":"revoke","workspace":"ada-personal","key":"replacement"}})).unwrap());
    assert_eq!(f.ok(&["change", "--input", &revoke])["status"], "applied");
    let unavailable = f.run(&["current"]);
    assert!(!unavailable.status.success());
    let v: Value = serde_json::from_slice(&unavailable.stdout).unwrap();
    assert_eq!(v["status"], "unavailable");
    assert_eq!(v["stored"]["context"]["account"], "ada");
    let retained = f.ok(&["show", "--purchase", "next"]);
    assert_eq!(retained["status"], "unknown");
    assert_eq!(retained["unresolved_ceiling"], 100);
    assert!(
        !f.run(&["quote", "--purchase", "forbidden", "--input", &input])
            .status
            .success()
    );
    assert_eq!(effects.load(Ordering::SeqCst), 3);
    thread.join().unwrap();
    let history = fs::read_to_string(f.dir.path().join("customer/state.json")).unwrap();
    assert!(!history.contains("oak_fixture."));
    assert_eq!(f.ok(&["credentials"])["operations"][1]["selected"], true);
}

#[test]
fn customer_commands_refuse_secret_flags_shared_inputs_and_host_authority() {
    let f = Fixture::new();
    let host = f.file("host-key", b"npub_unrelated-host");
    assert!(
        !f.run(&["import", "--alias", "host", "--input", &host])
            .status
            .success()
    );
    assert!(
        !f.run(&[
            "account",
            "--origin",
            "https://fixture.invalid",
            "--alias",
            "host",
            "--token",
            "raw-secret"
        ])
        .status
        .success()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = f.file("shared", b"oak_fixture.shared");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            !f.run(&["import", "--alias", "shared", "--input", &path])
                .status
                .success()
        );
    }
    let output = f.run(&["approve", "--purchase", "nothing", "--digest", &hash('a')]);
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("raw-secret"));
}

#[test]
fn a_quote_cannot_be_approved_or_dispatched_after_customer_becomes_read_only() {
    let f = Fixture::new();
    let path = f.file("reader", b"oak_fixture.reader");
    f.ok(&["import", "--alias", "reader", "--input", &path]);
    let mut ctx = context("ada", "ada-personal", "reader");
    ctx["role"] = json!("member");
    ctx["can_invoke"] = json!(false);
    let allowed = context("ada", "ada-personal", "reader");
    let (origin, effects, thread) = server(vec![
        read_context("ada-personal", "oak_fixture.reader", allowed.clone()),
        read_context("ada-personal", "oak_fixture.reader", allowed),
        read_context("ada-personal", "oak_fixture.reader", ctx.clone()),
        read_context("ada-personal", "oak_fixture.reader", ctx),
    ]);
    f.select(&origin, "reader", "ada", "ada-personal");
    let input = f.file("input.json", &serde_json::to_vec(&json!({"model":"decision-a", "state":"Private reader input", "questions":{"ready":{"type":"noul", "instructions":"Is this ready?"}}})).unwrap());
    let quote = f.ok(&["quote", "--purchase", "read-only", "--input", &input]);
    assert_eq!(quote["quote"]["context"]["payer_workspace"], "ada-personal");
    assert!(
        !f.run(&[
            "approve",
            "--purchase",
            "read-only",
            "--digest",
            quote["quote_digest"].as_str().unwrap()
        ])
        .status
        .success()
    );
    assert!(
        !f.run(&["invoke", "--purchase", "read-only"])
            .status
            .success()
    );
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.ok(&["show", "--purchase", "read-only"])["status"],
        "quoted"
    );
    thread.join().unwrap();
}

#[test]
fn installed_referral_commands_use_selected_account_and_private_input() {
    let f = Fixture::new();
    let key = f.file("referrer-key", b"oak_fixture.referrer");
    f.ok(&["import", "--alias", "referrer", "--input", &key]);
    let id = format!("ref_{}", "a".repeat(32));
    let token = format!("rfr_{}", "b".repeat(64));
    let ctx = context("ada", "ada-personal", "referrer");
    let record = json!({"schema":"openagents.referral.source.v1","id":id,"version":1,"owner":"ada","kind":"person","label":"Private referrer label","source_only":false,"pending_owner":null});
    let source = json!({"schema":"openagents.referral.source.v1","account":"ada","request":"referral-input","outcome":"captured","referrer":{"id":id,"version":2,"kind":"person","source_only":false},"consent_version":"openagents.referral.consent.v1","captured_at":1000});
    let replies = vec![
        read_context("ada-personal", "oak_fixture.referrer", ctx.clone()),
        read_context("ada-personal", "oak_fixture.referrer", ctx.clone()),
        Reply {
            route: "POST /v1/account/referrers ".into(),
            token: "oak_fixture.referrer",
            status: 200,
            body: json!({"v":"openagents.accounts.v1","referral":record}),
        },
        read_context("ada-personal", "oak_fixture.referrer", ctx.clone()),
        Reply {
            route: format!("POST /v1/account/referrers/{id}/link "),
            token: "oak_fixture.referrer",
            status: 200,
            body: json!({"v":"openagents.accounts.v1","referral":{"referrer":id,"token":token,"path":format!("/join?ref={token}")}}),
        },
        read_context("ada-personal", "oak_fixture.referrer", ctx.clone()),
        Reply {
            route: "POST /v1/account/acquisition ".into(),
            token: "oak_fixture.referrer",
            status: 200,
            body: json!({"v":"openagents.accounts.v1","referral":source}),
        },
        read_context("ada-personal", "oak_fixture.referrer", ctx.clone()),
        Reply {
            route: "GET /v1/account/acquisition ".into(),
            token: "oak_fixture.referrer",
            status: 200,
            body: json!({"v":"openagents.accounts.v1","referral":source}),
        },
        read_context("ada-personal", "oak_fixture.referrer", ctx),
        Reply {
            route: format!("DELETE /v1/account/referrers/{id}/link "),
            token: "oak_fixture.referrer",
            status: 200,
            body: json!({"v":"openagents.accounts.v1","referral":{"disabled":true}}),
        },
    ];
    let (origin, effects, thread) = server(replies);
    f.select(&origin, "referrer", "ada", "ada-personal");
    let input = f.file(
        "referrer.json",
        &serde_json::to_vec(&json!({"kind":"person","label":"Private referrer label"})).unwrap(),
    );
    assert_eq!(f.ok(&["referral", "create", "--input", &input])["id"], id);
    let link = f.ok(&["referral", "link", "--referrer", &id]);
    assert_eq!(link["url"], format!("{origin}/join?ref={token}"));
    assert!(!link["url"].as_str().unwrap().contains("ada"));
    let input=f.file("capture.json",&serde_json::to_vec(&json!({"request":"referral-input","token":token,"consent":true,"consent_version":"openagents.referral.consent.v1"})).unwrap());
    assert_eq!(f.ok(&["referral", "capture", "--input", &input]), source);
    assert_eq!(f.ok(&["referral", "source"]), source);
    assert_eq!(
        f.ok(&["referral", "disable", "--referrer", &id])["disabled"],
        true
    );
    assert!(
        !f.run(&["referral", "capture", "--token", "private-flag"])
            .status
            .success()
    );
    assert_eq!(effects.load(Ordering::SeqCst), 4);
    thread.join().unwrap();
}
