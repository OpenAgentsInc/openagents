//! Dedicated-account simulator coordinator. It owns only the headless child it starts.
use anyhow::{Context, Result, ensure};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use coder_account::openagents::{Client, Deployment};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use zeron_rpc::{RpcClient, methods};

#[path = "custom_agent_fixture/mod.rs"]
mod custom_agent_fixture;

const ACCOUNT: &str = "acct_83d16a35955ef908";
const TITLE: &str = "OpenAgents mobile live fixture";
struct Fixture {
    binary: PathBuf,
    root: PathBuf,
    edge: String,
    credential: String,
    account: String,
    port: u16,
    child: Option<Child>,
    ipc: Option<RpcClient>,
    device: String,
    space: String,
    chat: String,
    nonce: String,
    delivered: bool,
    restarts: usize,
    checks: BTreeMap<String, bool>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Fixture {
    async fn start(&mut self) -> Result<()> {
        ensure!(self.child.is_none(), "Fixture engine is already running");
        let home = self.root.join("home");
        std::fs::create_dir_all(&home)?;
        custom_agent_fixture::install_provider_credentials(self, &home)?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("engine.log"))?;
        let child = Command::new(&self.binary)
            .arg("headless")
            .env_clear()
            .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin")
            .envs(custom_agent_fixture::provider_environment(self, &home)?)
            .env("HOME", &home)
            .env("TMPDIR", &self.root)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_DATA_HOME", home.join("share"))
            .env("ZERON_DATA_DIR", self.root.join("data"))
            .env("ZERON_IPC_PORT", self.port.to_string())
            .env("ZERON_EDGE_URL", &self.edge)
            .env("CODER_DESKTOP_CREDENTIAL", &self.credential)
            .env("ZERON_HARNESS", "mock")
            .env("ZERON_MOCK_REPEAT", "500")
            .env("ZERON_MOCK_DELAY_MS", "250")
            .env("BROWSER", "/usr/bin/true")
            .env("NO_OPEN_BROWSER", "1")
            .current_dir(&home)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        let pid = child.id();
        self.child = Some(child);
        let ipc = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                ensure!(
                    self.child.as_mut().unwrap().try_wait()?.is_none(),
                    "The dedicated engine exited before readiness; inspect its private log"
                );
                // Never probe an IPC listener until OS metadata proves our child owns it.
                let output = Command::new("/usr/sbin/lsof")
                    .args(["-nP", &format!("-iTCP:{}", self.port), "-sTCP:LISTEN", "-t"])
                    .output()?;
                let pids = String::from_utf8_lossy(&output.stdout);
                if pids
                    .lines()
                    .any(|line| line.parse::<u32>().ok() == Some(pid))
                {
                    if let Ok(ipc) =
                        zeron_rpc::connect_ws(&format!("ws://127.0.0.1:{}", self.port)).await
                    {
                        let info = ipc.call(methods::ENGINE_INFO, json!({})).await?;
                        ensure!(
                            info["workspaceScope"] == "synced",
                            "The dedicated SDK account did not enable sync"
                        );
                        ipc.call(methods::ENGINE_READY, json!({})).await?;
                        return Ok::<_, anyhow::Error>(ipc);
                    }
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await??;
        self.device = ipc.call(methods::LOCAL_DEVICE, json!({})).await?["deviceId"]
            .as_str()
            .context("Missing fixture device")?
            .into();
        ipc.call(
            methods::MUTATE,
            json!({"op":"renameDevice","deviceId":self.device,"name":"OpenAgents mobile fixture"}),
        )
        .await?;
        self.ipc = Some(ipc);
        let http = reqwest::Client::new();
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let response = http
                    .get(format!("{}/device/{}/status", self.edge, self.device))
                    .bearer_auth(&self.credential)
                    .send()
                    .await?;
                if response.status().is_success()
                    && response.json::<Value>().await?["hostConnected"] == true
                {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await??;
        Ok(())
    }
    async fn stop(&mut self) -> Result<()> {
        if let Some(ipc) = self.ipc.take() {
            let _ = ipc.call(methods::STOP_ENGINE, json!({})).await;
        }
        if let Some(child) = &mut self.child {
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    if child.try_wait()?.is_some() {
                        return Ok::<_, anyhow::Error>(());
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await??;
        }
        self.child = None;
        Ok(())
    }
    async fn first(&self, method: &str) -> Result<Value> {
        let ipc = self.ipc.as_ref().context("The fixture engine is offline")?;
        let mut watch = ipc.subscribe(method, json!({"chatId":self.chat})).await?;
        tokio::time::timeout(Duration::from_secs(10), watch.recv())
            .await?
            .context("Fixture watch ended")
    }
    async fn seed(&self) -> Result<()> {
        let ipc = self.ipc.as_ref().unwrap();
        let folder = self.root.join("project");
        std::fs::create_dir_all(&folder)?;
        ipc.call(methods::MUTATE, json!({"op":"createSpace","spaceId":self.space,"deviceId":self.device,"path":folder,"name":custom_agent_fixture::project_name(self),"gitDetected":false})).await?;
        ipc.call(methods::MUTATE, json!({"op":"createChat","chatId":self.chat,"spaceId":self.space,"config":{"harness":"mock","sandbox":"workspace-write"}})).await?;
        ipc.call(
            methods::MUTATE,
            json!({"op":"renameChat","chatId":self.chat,"title":TITLE}),
        )
        .await?;
        ipc.call(methods::QUEUE_COMMAND, json!({"chatId":self.chat,"command":{"kind":"run","messageId":uuid::Uuid::new_v4().to_string(),"request":{"prompt":"Desktop started this dedicated fixture session.","harness":"mock","modelOptions":{"mockQuestions":true},"cwd":folder,"sandbox":"workspace-write"}}})).await?;
        Ok(())
    }
    async fn check(&mut self, name: &str) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(65), async {
            loop {
                let messages = self.first(methods::WATCH_DOC_MESSAGES).await?;
                let text = serde_json::to_string(&messages)?;
                let rows = messages["reset"]
                    .as_array()
                    .context("Expected a transcript reset")?;
                let last = rows.iter().rev().find(|row| row["role"] == "assistant");
                let ready = match name {
                    "answers" => {
                        text.contains("Locked in:")
                            && text.contains("Event-driven fold")
                            && text.contains("Unit tests")
                    }
                    "phone" => {
                        text.contains("Phone live message")
                            && last.is_some_and(|row| {
                                row["status"] == "streaming"
                                    && row["parts"]
                                        .as_array()
                                        .is_some_and(|parts| !parts.is_empty())
                            })
                    }
                    "queue" => self.first(methods::WATCH_QUEUE).await?["items"]
                        .as_array()
                        .is_some_and(|rows| {
                            rows.iter().any(|row| row["text"] == "Phone queued message")
                        }),
                    "stop" => last.is_some_and(|row| row["status"] == "aborted"),
                    "foreground" => text.contains("Phone live message"),
                    "reconnect" => self.restarts == 1 && text.contains("Phone reconnected message"),
                    _ => anyhow::bail!("Unknown fixture assertion"),
                };
                if ready {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await??;
        self.checks.insert(name.into(), true);
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<()> {
        if std::env::var_os("OA_CUSTOM_AGENT_UI").is_some() {
            custom_agent_fixture::cleanup_phone(self).await?;
        }
        if self.child.is_none() {
            self.start().await?;
        }
        if let Some(ipc) = &self.ipc {
            let _ = ipc
                .call(
                    methods::QUEUE_COMMAND,
                    json!({"chatId":self.chat,"command":{"kind":"interrupt"}}),
                )
                .await;
            ipc.call(
                methods::MUTATE,
                json!({"op":"deleteSpace","spaceId":self.space}),
            )
            .await?;
        }
        self.stop().await?;
        if std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_PROVIDER_HOME").is_some() {
            for path in [".codex/auth.json", ".claude/.credentials.json"] {
                let _ = std::fs::remove_file(self.root.join("home").join(path));
            }
        }
        let ms = chrono::Utc::now().timestamp_millis() + 10000;
        let ops = [("devices",&self.device),("spaces",&self.space),("chats",&self.chat),("sessions",&self.chat)].map(|(kind,id)|json!({"kind":kind,"id":id,"op":"delete","hlc":format!("{ms:013}-000000-mobile-fixture")}));
        let response = reqwest::Client::new()
            .post(format!("{}/registry/{}/push", self.edge, self.account))
            .bearer_auth(&self.credential)
            .json(&json!({"batch":uuid::Uuid::new_v4().to_string(),"ops":ops}))
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "Could not remove the dedicated registry fixtures"
        );
        Ok(())
    }
}
type Shared = Arc<Mutex<Fixture>>;
async fn fixture_credential(
    State(state): State<Shared>,
    Path(nonce): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut fixture = state.lock().await;
    if nonce != fixture.nonce || fixture.delivered {
        return Err(StatusCode::NOT_FOUND);
    }
    fixture.delivered = true;
    Ok(Json(json!({"credential":fixture.credential})))
}
async fn check(
    State(state): State<Shared>,
    Path(name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let result = state.lock().await.check(&name).await;
    if let Err(error) = result {
        eprintln!("Fixture assertion failed: {name}: {error}");
        return Err(StatusCode::CONFLICT);
    }
    Ok(Json(json!({"passed":true})))
}
async fn offline(State(state): State<Shared>) -> Result<Json<Value>, StatusCode> {
    state
        .lock()
        .await
        .stop()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"offline":true})))
}
async fn online(State(state): State<Shared>) -> Result<Json<Value>, StatusCode> {
    let mut f = state.lock().await;
    f.start()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    f.restarts += 1;
    Ok(Json(json!({"online":true})))
}
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let binary = PathBuf::from(args.next().context("Pass the dedicated headless binary")?);
    let edge = args.next().context("Pass the sync URL")?;
    ensure!(
        [
            "https://coder-sync-staging-157437760789.us-central1.run.app",
            "https://coder-sync-157437760789.us-central1.run.app"
        ]
        .contains(&edge.as_str()),
        "Use a qualified coder-sync deployment"
    );
    let root = PathBuf::from(args.next().context("Pass a fresh scratch directory")?);
    ensure!(!root.exists(), "Use a new fixture directory");
    std::fs::create_dir_all(&root)?;
    let key_file = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("work/.secrets/openagents-test-account.env")
    });
    let expected_account = args.next().unwrap_or_else(|| ACCOUNT.into());
    let file = std::fs::read_to_string(key_file)?;
    let credential = file
        .lines()
        .filter_map(|line| line.trim().strip_prefix("OPENAGENTS_TOKEN="))
        .next()
        .context("Dedicated test credential missing")?
        .trim()
        .trim_matches(['\"', '\''])
        .to_string();
    let account = Client::new(Deployment::production())?
        .authenticate(&credential)
        .await?;
    ensure!(
        account.account().id == expected_account,
        "This is not the documented dedicated SDK test account"
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    ensure!(port != 49777, "Protected IPC port");
    drop(listener);
    let mut fixture = Fixture {
        binary,
        root: root.clone(),
        edge: edge.clone(),
        credential,
        account: expected_account,
        port,
        child: None,
        ipc: None,
        device: String::new(),
        space: uuid::Uuid::new_v4().to_string(),
        chat: uuid::Uuid::new_v4().to_string(),
        nonce: uuid::Uuid::new_v4().simple().to_string(),
        delivered: false,
        restarts: 0,
        checks: BTreeMap::new(),
    };
    if let Some(runner) = std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_RUNNER") {
        let runner = PathBuf::from(runner);
        let data = root.join("data");
        std::fs::create_dir_all(&data)?;
        std::fs::write(
            data.join("sdk-runners.json"),
            serde_json::to_vec(
                &json!({"fixture":{"label":"Fixture Rust agent","sha256":zeron_harness::sdk::executable_digest(std::path::Path::new(&runner))?,"executable":runner,"args":[]}}),
            )?,
        )?;
        if std::env::var_os("OA_CUSTOM_AGENT_UI").is_none() {
            fixture.start().await?;
            return custom_agent_fixture::run(&mut fixture).await;
        }
    }
    if std::env::var_os("OA_CUSTOM_AGENT_UI").is_some() {
        custom_agent_fixture::capture_phone_baseline(&fixture).await?;
    }
    fixture.start().await?;
    fixture.seed().await?;
    let http = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", http.local_addr()?);
    std::fs::write(
        root.join("config.json"),
        serde_json::to_vec_pretty(
            &json!({"control":base,"credential_url":format!("{base}/credential/{}",fixture.nonce),"sync":edge,"title":TITLE,"project":custom_agent_fixture::project_name(&fixture),"chat":fixture.chat,"device":fixture.device}),
        )?,
    )?;
    let state = Arc::new(Mutex::new(fixture));
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let app = Router::new()
        .route("/credential/{nonce}", get(fixture_credential))
        .route("/check/{name}", post(check))
        .route("/offline", post(offline))
        .route("/online", post(online))
        .route(
            "/stop",
            post(move || {
                let stop = stop.clone();
                async move {
                    let _ = stop.send(true);
                    Json(json!({"stopping":true}))
                }
            }),
        )
        .with_state(state.clone());
    println!("READY: dedicated account, isolated headless host, real coder-sync");
    let server = tokio::spawn(async move { axum::serve(http, app).await });
    tokio::select! {_=stopped.changed()=>{},_=tokio::signal::ctrl_c()=>{},_=tokio::time::sleep(Duration::from_secs(900))=>{}}
    server.abort();
    let mut fixture = state.lock().await;
    fixture.cleanup().await?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(
            &json!({"account":fixture.account,"sync":fixture.edge,"checks":fixture.checks,"host_restarts":fixture.restarts,"registry_fixtures_removed":true,"child_stopped":true,"credential_written_to_config":false}),
        )?,
    )?;
    println!(
        "Cleanup: fixture registry rows removed, dedicated headless child stopped; no owner sessions used"
    );
    Ok(())
}
