//! The real service binary restarts over simulated Boat and wallet servers.
//! All files, keys, invoices, and resources belong to this fixture.

use pay_ledger::{
    Ledger,
    compute::{Binding, PrincipalKind, Rights, credential_digest},
};
use retail_qualify::sim::{FakeBoat, Resident, SimNetwork};
use retail_service::types::{Config, RetailGrant, SCHEMA};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn private(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn start(config: &Path, home: &Path) -> Process {
    Process(
        Command::new(env!("CARGO_BIN_EXE_retail-service"))
            .args(["--config", config.to_str().unwrap()])
            .env_clear()
            .env("HOME", home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}
async fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(15);
    while !ready() {
        assert!(
            Instant::now() < until,
            "synthetic service did not advance: {label}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
async fn post(client: &reqwest::Client, url: &str, value: Value) -> Value {
    let response = client
        .post(url)
        .header("x-retail-principal", "cli:alice")
        .bearer_auth("alice")
        .json(&value)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let result = response.json::<Value>().await.unwrap();
    assert!(status.is_success(), "{status}: {result}");
    result["result"].clone()
}
#[tokio::test]
async fn killed_service_resumes_original_resource_task_and_settlement() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let network = SimNetwork::new();
    let wallet_home = root.join("wallet");
    let _resident = Resident::serve(&wallet_home, network.receiver()).unwrap();
    let boat = FakeBoat::start("synthetic-retail-boat-key").unwrap();
    boat.lose_next_create_reply();
    boat.set_usage_seconds(0);
    let ledger_path = root.join("ledger.sqlite");
    private(&ledger_path, "");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    let now = retail_service::http::now();
    ledger.create_compute_account("alice", now).unwrap();
    ledger
        .bind_principal(&Binding {
            principal: "cli:alice".into(),
            account: "alice".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("alice"),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: now,
        })
        .unwrap();
    drop(ledger);
    let mut receipt = retail_qualify::qualify::run_fake(&retail_qualify::qualify::fixture());
    receipt.mode = retail_qualify::qualify::Mode::Funded;
    receipt.label = "Synthetic owner-record gate fixture, not real funded qualification.".into();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    drop(socket);
    let key_path = root.join("boat.key");
    private(&key_path, "synthetic-retail-boat-key");
    let config_path = root.join("host.json");
    private(&config_path,&serde_json::to_string(&json!({"mode":"development","customer":Config {schema:SCHEMA.into(),state:root.join("service"),ledger:ledger_path.clone(),template:"oa-coder-main-2026-10-07".into(),grants:vec![RetailGrant {principal:"cli:alice".into(),account:"alice".into(),generation:1,observe:true,execute:true,disclose:true}],contract_confirmed:true,qualification:Some(receipt.clone()),supported_plan:receipt.plan.clone(),plan_starts_left:Some(4),environments:None},"listen":address,"boat_api_base":boat.base(),"boat_org":null,"boat_key_file":key_path,"wallet_home":wallet_home,"poll_seconds":1})).unwrap());
    let operator_key = root.join("operator.key");
    private(&operator_key, "synthetic-operator-key");
    let ingress = root.join("Caddyfile");
    private(
        &ingress,
        "synthetic ingress fixture; not a deployed TLS origin",
    );
    let mut host: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    host["operations"] = json!({
        "schema":retail_service::package::OPERATIONS_SCHEMA,
        "public_origin":"https://retail.example",
        "ingress_file":ingress,
        "operator_key_file":operator_key,
        "approved_identity":route_contract::Digest::of_bytes(b"unapproved-synthetic"),
        "plan_file":root.join("absent-plan.json"),
        "qualification_file":root.join("absent-live-receipt.json"),
        "qualification_digest":route_contract::Digest::of_bytes(b"absent"),
        "qualification_state":root.join("absent-qualification"),
        "qualification_ledger":root.join("absent-qualification/ledger.sqlite")
    });
    private(&config_path, &serde_json::to_string(&host).unwrap());
    let client = reqwest::Client::new();
    let url = format!("http://{address}/v1/retail");
    let home = root.join("empty-home");
    fs::create_dir(&home).unwrap();
    let mut process = start(&config_path, &home);
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .get(format!("http://{address}/healthz"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < until);
        assert!(
            process.0.try_wait().unwrap().is_none(),
            "service failed to start"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let topup = post(
        &client,
        &url,
        json!({"op":"top_up","idempotency":"funding","amount_sats":1000}),
    )
    .await;
    let status_url = format!("http://{address}/operator/status");
    assert_eq!(client.get(&status_url).send().await.unwrap().status(), 403);
    assert_eq!(
        client
            .get(&status_url)
            .bearer_auth("alice")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(&status_url)
            .bearer_auth("synthetic-operator-key")
            .header("Origin", "https://untrusted.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let status: Value = client
        .get(&status_url)
        .bearer_auth("synthetic-operator-key")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        status["identity"]["commit"],
        retail_service::package::COMMIT
    );
    assert_eq!(status["synthetic"], true);
    assert_eq!(
        status["identity"]["service"],
        retail_qualify::launch::SERVICE
    );
    private(&operator_key, "withdrawn-synthetic-key");
    assert_eq!(
        client
            .get(&status_url)
            .bearer_auth("synthetic-operator-key")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    private(&operator_key, "synthetic-operator-key");
    network
        .payer()
        .pay_invoice(topup["invoice"].as_str().unwrap())
        .unwrap();
    loop {
        let account = post(&client, &url, json!({"op":"account"})).await;
        if account["balance"]["credited_msat"] == 1_000_000 {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let made=post(&client,&url,json!({"op":"offer","idempotency":"work","task":{"source":{"repository":"https://github.com/OpenAgentsInc/example","commit":"c".repeat(40)},"task":"Fix the parser.","checks":["cargo test -p parser"],"max_seconds":600,"ceiling_sats":null}})).await;
    let confirm = json!({"op":"confirm","offer":made["offer"]["id"],"digest":made["offer"]["digest"],"admission":route_contract::digest_of(&made["admission"]),"custody":made["custody"]["digest"],"credential":{"provider":"openai","key":"synthetic-customer-model-key","service_custody":true}});
    let accepted = post(&client, &url, confirm.clone()).await;
    let execution = accepted["execution"].as_str().unwrap();
    wait("create", || boat.sandboxes_created() == 1).await;
    process.0.kill().unwrap();
    process.0.wait().unwrap();
    drop(process);
    let mut process = start(&config_path, &home);
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .get(format!("http://{address}/healthz"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < until);
        assert!(process.0.try_wait().unwrap().is_none());
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let repeated = post(&client, &url, confirm).await;
    assert_eq!(repeated["execution"], execution);
    wait("dispatch", || boat.executors_started() == 1).await;
    boat.set_usage_seconds(17);
    post(&client, &url, json!({"op":"cancel","execution":execution})).await;
    wait("cleanup", || boat.active() == 0).await;
    let until = Instant::now() + Duration::from_secs(10);
    let settled = loop {
        let receipt = post(&client, &url, json!({"op":"receipt","execution":execution})).await;
        if !receipt["settlement"].is_null() {
            break receipt;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(settled["settlement"]["held_msat"], 0);
    assert_eq!(boat.sandboxes_created(), 1);
    assert_eq!(boat.executors_started(), 1);
    assert_eq!(boat.unauthorized(), 0);
    assert_eq!(network.issued(), 1);
    let b = post(&client, &url, json!({"op":"account"})).await["balance"].clone();
    assert_eq!(
        b["credited_msat"].as_i64().unwrap(),
        b["available_msat"].as_i64().unwrap()
            + b["held_msat"].as_i64().unwrap()
            + b["settled_msat"].as_i64().unwrap()
    );
    assert!(
        fs::read_dir(root.join("service/credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(
        boat.files_containing("synthetic-customer-model-key")
            .is_empty()
    );
    // A second process restart retains the sealed settlement and balance.
    process.0.kill().unwrap();
    process.0.wait().unwrap();
    drop(process);
    let mut final_process = start(&config_path, &home);
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .get(format!("http://{address}/healthz"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(
        post(&client, &url, json!({"op":"account"})).await["balance"],
        b
    );
    assert!(fs::read_dir(&home).unwrap().next().is_none());
    final_process.0.kill().unwrap();
    final_process.0.wait().unwrap();
    drop(final_process);
    // The actual packaged command snapshots only the stopped runtime. It
    // checks rollback against unchanged live accounting, then restores to
    // absent paths instead of replacing the original money files.
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_retail-service"))
            .args(args)
            .env_clear()
            .env("HOME", &home)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let checkpoint = root.join("checkpoint");
    let inspection = run(&["inspect", "--config", config_path.to_str().unwrap()]);
    assert_eq!(
        inspection["identity"]["commit"],
        retail_service::package::COMMIT
    );
    run(&[
        "snapshot",
        "--config",
        config_path.to_str().unwrap(),
        "--out",
        checkpoint.to_str().unwrap(),
    ]);
    let binary = env!("CARGO_BIN_EXE_retail-service");
    let digest = route_contract::Digest::of_bytes(&fs::read(binary).unwrap());
    let rollback = run(&[
        "rollback-check",
        "--config",
        config_path.to_str().unwrap(),
        "--snapshot",
        checkpoint.to_str().unwrap(),
        "--candidate",
        binary,
        "--digest",
        digest.as_str(),
    ]);
    assert_eq!(rollback["accounting_preserved"], true);
    let original_config = fs::read(&config_path).unwrap();
    let mut changed_config: Value = serde_json::from_slice(&original_config).unwrap();
    changed_config["customer"]["plan_starts_left"] = json!(0);
    private(
        &config_path,
        &serde_json::to_string(&changed_config).unwrap(),
    );
    let refused = Command::new(binary)
        .args(["rollback-check", "--config"])
        .arg(&config_path)
        .arg("--snapshot")
        .arg(&checkpoint)
        .args(["--candidate", binary, "--digest", digest.as_str()])
        .env_clear()
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "rollback cannot approve changed configuration"
    );
    fs::write(&config_path, original_config).unwrap();
    let restored = root.join("restored");
    let restored_ledger = root.join("restored-ledger.sqlite");
    run(&[
        "restore",
        "--snapshot",
        checkpoint.to_str().unwrap(),
        "--state",
        restored.to_str().unwrap(),
        "--ledger",
        restored_ledger.to_str().unwrap(),
    ]);
    let mut changed: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    changed["customer"]["state"] = json!(restored);
    changed["customer"]["ledger"] = json!(restored_ledger);
    let recovered_config = root.join("recovered.json");
    private(&recovered_config, &serde_json::to_string(&changed).unwrap());
    let _recovered = start(&recovered_config, &home);
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .get(format!("http://{address}/healthz"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(
        post(&client, &url, json!({"op":"account"})).await["balance"],
        b
    );
    assert_eq!(boat.sandboxes_created(), 1);
    assert_eq!(boat.executors_started(), 1);
    assert!(fs::read_dir(&home).unwrap().next().is_none());
}
