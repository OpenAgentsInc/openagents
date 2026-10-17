use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, device::pick_live_host, rooms::Rooms,
    storage::Bucket,
};
use futures::{SinkExt, StreamExt};
use openagents_rpc::{DeviceFrameHeader as Header, decode_device_frame, encode_device_frame};
use serde_json::{Value, json};
use std::time::Duration;
trait FromHeader {
    fn with_from(self, from: &str) -> Self;
}
impl FromHeader for Header {
    fn with_from(mut self, from: &str) -> Self {
        self.from = Some(from.into());
        self
    }
}
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
struct Server {
    base: String,
    account: String,
    device: String,
    db: Database,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start() -> Self {
        let db = Database::new(
            &std::env::var("CODER_SYNC_TEST_DATABASE_URL").expect("Run scripts/coder-sync/test.sh"),
        )
        .unwrap();
        db.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = server::router(StateData {
            database: db.clone(),
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Dev,
            accounts: None,
            revision: "device-test".into(),
            push: None,
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            account: format!("device-{}", uuid::Uuid::new_v4()),
            device: uuid::Uuid::new_v4().to_string(),
            db,
            task,
        }
    }
    fn url(&self, role: &str, conn: &str, account: &str) -> String {
        format!(
            "{}/device/{}/ws?role={role}&connId={conn}&token={account}&nudgeAck=1",
            self.base.replacen("http:", "ws:", 1),
            self.device
        )
    }
    async fn ws(&self, role: &str, conn: &str) -> Socket {
        tokio_tungstenite::connect_async(self.url(role, conn, &self.account))
            .await
            .unwrap()
            .0
    }
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        account: &str,
    ) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .request(
                method,
                format!("{}/device/{}{path}", self.base, self.device),
            )
            .bearer_auth(account)
    }
    async fn get(&self, path: &str) -> Value {
        self.request(reqwest::Method::GET, path, &self.account)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    async fn nudge(&self, chat: &str) -> Value {
        self.request(reqwest::Method::POST, "/nudge", &self.account)
            .json(&json!({"chatId":chat}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    async fn pending(&self) -> i64 {
        self.db
            .connect()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM device_nudges WHERE account=$1 AND device_id=$2",
                &[&self.account, &self.device],
            )
            .await
            .unwrap()
            .get(0)
    }
}
async fn send(ws: &mut Socket, h: Header, p: &[u8]) {
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(
        encode_device_frame(&h, p).unwrap(),
    ))
    .await
    .unwrap();
}
async fn next(ws: &mut Socket) -> (Header, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match ws.next().await.unwrap().unwrap() {
                tokio_tungstenite::tungstenite::Message::Binary(b) => {
                    return decode_device_frame(&b).unwrap();
                }
                m => panic!("Expected binary frame, got {m:?}"),
            }
        }
    })
    .await
    .unwrap()
}
async fn closed(ws: &mut Socket) -> u16 {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match ws.next().await {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Close(Some(c)))) => {
                    return u16::from(c.code);
                }
                Some(Ok(_)) => {}
                m => panic!("Expected close, got {m:?}"),
            }
        }
    })
    .await
    .unwrap()
}
async fn ack(ws: &mut Socket, body: &[u8]) {
    send(ws, Header::new("", "nudgeAck"), body).await;
}
async fn drained(s: &Server) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while s.pending().await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[test]
fn edge_device_frame_vectors() {
    for (h, p) in [
        (
            Header::new("term-42", "term").with_to("conn-9"),
            vec![1, 2, 3, 250, 255],
        ),
        (
            Header::new("x".repeat(200), "rpc").with_from("conn-1"),
            vec![],
        ),
    ] {
        let frame = encode_device_frame(&h, &p).unwrap();
        let (a, b) = decode_device_frame(&frame).unwrap();
        assert_eq!(a, h);
        assert_eq!(b, p);
    }
    for bad in [vec![], vec![128], vec![255; 12], vec![2, b'{', b'{']] {
        assert!(decode_device_frame(&bad).is_err());
    }
}
#[test]
fn edge_host_liveness_vectors() {
    let now = 1_000_000_000_000;
    let fresh = now - 10_000;
    let corpse = now - 600_000;
    for (hosts, exclude, want) in [
        (
            vec![("corpse", corpse), ("live", fresh)],
            None,
            Some("live"),
        ),
        (
            vec![("live", fresh), ("corpse", corpse)],
            None,
            Some("live"),
        ),
        (
            vec![
                ("older", now - 40_000),
                ("newest", now - 1000),
                ("middle", now - 20_000),
            ],
            None,
            Some("newest"),
        ),
        (vec![("a", corpse), ("b", now - 76_000)], None, None),
        (vec![("legacy", 0)], None, None),
        (vec![("joining", now)], None, Some("joining")),
        (vec![], None, None),
        (vec![("leaving", fresh)], Some("leaving"), None),
        (
            vec![("leaving", now), ("successor", fresh)],
            Some("leaving"),
            Some("successor"),
        ),
    ] {
        assert_eq!(pick_live_host(&hosts, now, exclude), want);
    }
}
#[tokio::test]
async fn opaque_relay_supersedes_host_and_reports_disconnects() {
    let s = Server::start().await;
    let mut host = s.ws("host", "host-a").await;
    let mut client = s.ws("client", "viewer").await;
    assert_eq!(s.get("/status").await["hostConnected"], true);
    let p = vec![0, 255, 3, 128, 1];
    send(
        &mut client,
        Header::new("q", "rpc").with_from("spoof").with_to("spoof"),
        &p,
    )
    .await;
    let (h, b) = next(&mut host).await;
    assert_eq!(b, p);
    assert_eq!(h.from.as_deref(), Some("viewer"));
    assert!(h.to.is_none());
    send(
        &mut host,
        Header::new("q", "rpc").with_to("viewer").with_from("spoof"),
        &b,
    )
    .await;
    let (h, b) = next(&mut client).await;
    assert_eq!(b, p);
    assert!(h.from.is_none() && h.to.is_none());
    let mut successor = s.ws("host", "host-b").await;
    assert_eq!(closed(&mut host).await, 4409);
    send(&mut client, Header::new("next", "echo"), b"live").await;
    let (h, b) = next(&mut successor).await;
    assert_eq!(h.s, "next");
    assert_eq!(b, b"live");
    send(
        &mut successor,
        Header::new("missing", "rpc").with_to("missing"),
        b"data",
    )
    .await;
    let (h, b) = next(&mut successor).await;
    assert_eq!(h.k, " relay");
    assert_eq!(
        serde_json::from_slice::<Value>(&b).unwrap()["error"],
        "client_gone"
    );
    successor.close(None).await.unwrap();
    let (_, b) = next(&mut client).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&b).unwrap()["error"],
        "host_closed"
    );
    send(&mut client, Header::new("q", "rpc"), b"again").await;
    let (_, b) = next(&mut client).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&b).unwrap()["error"],
        "host_offline"
    );
    assert_eq!(s.get("/status").await["hostConnected"], false);
}
#[tokio::test]
async fn ownership_grants_revocation_and_sidecars() {
    let s = Server::start().await;
    let _host = s.ws("host", "h").await;
    let other = format!("other-{}", uuid::Uuid::new_v4());
    for role in ["host", "client"] {
        let e = tokio_tungstenite::connect_async(s.url(role, "stranger", &other))
            .await
            .unwrap_err();
        assert!(matches!(e,tokio_tungstenite::tungstenite::Error::Http(r) if r.status()==403));
    }
    for path in ["/status", "/sidecar/repos"] {
        assert_eq!(
            s.request(reqwest::Method::GET, path, &other)
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        s.request(reqwest::Method::POST, "/nudge", &other)
            .json(&json!({"chatId":"x"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let body = json!({"repos":[{"path":"/scratch/example","branches":["main"]}]});
    assert!(
        s.request(reqwest::Method::POST, "/sidecar/repos", &s.account)
            .json(&body)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(s.get("/sidecar/repos").await, body);
    let path = format!("/grants/{other}");
    assert!(
        s.request(reqwest::Method::PUT, &path, &s.account)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let mut viewer = tokio_tungstenite::connect_async(s.url("client", "granted", &other))
        .await
        .unwrap()
        .0;
    assert_eq!(
        s.request(reqwest::Method::GET, "/sidecar/repos", &other)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        s.request(reqwest::Method::POST, "/sidecar/repos", &other)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(
        s.request(reqwest::Method::DELETE, &path, &s.account)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(closed(&mut viewer).await, 4403);
    assert_eq!(
        s.request(reqwest::Method::GET, "/status", &other)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}
#[tokio::test]
async fn durable_wakeup_replays_and_old_ack_cannot_erase_new_work() {
    let s = Server::start().await;
    let mut host = s.ws("host", "h").await;
    host.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(s.nudge("chat").await["queued"], true);
    assert_eq!(s.pending().await, 1);
    let mut host = s.ws("host", "reconnected").await;
    let (h, old) = next(&mut host).await;
    assert_eq!(h.k, "nudge");
    assert_eq!(
        serde_json::from_slice::<Value>(&old).unwrap()["chatId"],
        "chat"
    );
    assert_eq!(s.pending().await, 1);
    s.nudge("chat").await;
    let (_, new) = next(&mut host).await;
    assert_ne!(old, new);
    ack(&mut host, &old).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(s.pending().await, 1);
    ack(&mut host, &new).await;
    drained(&s).await;
    s.nudge("again").await;
    let (_, unacked) = next(&mut host).await;
    host.close(None).await.unwrap();
    let mut successor = s.ws("host", "after-loss").await;
    let (_, replayed) = next(&mut successor).await;
    assert_eq!(unacked, replayed);
    ack(&mut successor, &replayed).await;
    drained(&s).await;
}
#[tokio::test]
async fn cold_host_hint_is_atomic_and_account_scoped() {
    let s = Server::start().await;
    let chat = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "{}/chat2/{chat}/rows?batchId=first&device=phone&hostDevice={}",
        s.base, s.device
    );
    let push = || {
        reqwest::Client::new()
            .post(&url)
            .bearer_auth(&s.account)
            .body(vec![1, 2, 3])
    };
    assert!(push().send().await.unwrap().status().is_success());
    assert_eq!(s.pending().await, 1);
    let mut host = s.ws("host", "cold").await;
    let (_, old) = next(&mut host).await;
    assert!(push().send().await.unwrap().status().is_success());
    let (_, new) = next(&mut host).await;
    assert_ne!(old, new);
    ack(&mut host, &old).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(s.pending().await, 1);
    ack(&mut host, &new).await;
    drained(&s).await;
    let db = s.db.connect().await.unwrap();
    assert_eq!(
        db.query_one("SELECT head FROM chats WHERE chat_id=$1", &[&chat])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

struct Engine(std::process::Child);
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
#[ignore = "Requires OPENAGENTS_TEST_HEADLESS_BIN built from apps/openagents"]
async fn pinned_headless_engine_folder_listing_and_queue_roundtrip() {
    let binary = std::env::var("OPENAGENTS_TEST_HEADLESS_BIN")
        .or_else(|_| std::env::var("ZERON_TEST_HEADLESS_BIN"))
        .expect("Build pinned openagents, set OPENAGENTS_TEST_HEADLESS_BIN");
    let s = Server::start().await;
    let scratch = tempfile::Builder::new()
        .prefix("device-engine-")
        .tempdir_in(std::env::var("OPENAGENTS_SCRATCH").unwrap())
        .unwrap();
    let home = scratch.path().join("home");
    let folders = scratch.path().join("folders");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(folders.join("alpha")).unwrap();
    std::fs::write(folders.join("file.txt"), "fixture").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_ne!(port, 49777);
    drop(listener);
    let log = std::fs::File::create(scratch.path().join("engine.log")).unwrap();
    let mut engine = Engine(
        std::process::Command::new(binary)
            .arg("headless")
            .env_clear()
            .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &home)
            .env("TMPDIR", scratch.path())
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_DATA_HOME", home.join("share"))
            .env("OPENAGENTS_DATA_DIR", scratch.path().join("data"))
            .env("OPENAGENTS_IPC_PORT", port.to_string())
            .env("OPENAGENTS_EDGE_URL", &s.base)
            .env("OPENAGENTS_EDGE_TOKEN", &s.account)
            .env("OPENAGENTS_ORG_ID", &s.account)
            .env("OPENAGENTS_WORKOS_CLIENT_ID", "")
            .env("OPENAGENTS_HARNESS", "mock")
            .env("BROWSER", "/usr/bin/true")
            .env("NO_OPEN_BROWSER", "1")
            .current_dir(&home)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap(),
    );
    let ipc = tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            if let Ok(c) = openagents_rpc::connect_ws(&format!("ws://127.0.0.1:{port}")).await {
                break c;
            }
            assert!(
                engine.0.try_wait().unwrap().is_none(),
                "Headless engine exited; inspect {}",
                scratch.path().display()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let device = ipc
        .call(openagents_rpc::methods::LOCAL_DEVICE, json!({}))
        .await
        .unwrap()["deviceId"]
        .as_str()
        .unwrap()
        .to_owned();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status = reqwest::Client::new()
                .get(format!("{}/device/{device}/status", s.base))
                .bearer_auth(&s.account)
                .send()
                .await
                .unwrap();
            if status.status().is_success()
                && status.json::<Value>().await.unwrap()["hostConnected"] == true
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let url = openagents_rpc::device_room::device_room_ws_url(
        &s.base,
        &device,
        "client",
        Some("phone"),
        &s.account,
    );
    let link = openagents_rpc::DeviceLink::connect(&url).await.unwrap();
    let client = link.client();
    let listing = tokio::time::timeout(
        Duration::from_secs(10),
        client.call(
            openagents_rpc::methods::LIST_FOLDERS,
            json!({"path":folders}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        listing["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "alpha" && e["isDir"] == true)
    );
    let space = uuid::Uuid::new_v4().to_string();
    let chat = uuid::Uuid::new_v4().to_string();
    client.call(openagents_rpc::methods::MUTATE,json!({"op":"createSpace","spaceId":space,"deviceId":device,"path":folders,"name":"Relay fixture","gitDetected":false})).await.unwrap();
    client
        .call(
            openagents_rpc::methods::MUTATE,
            json!({"op":"createChat","chatId":chat,"spaceId":space}),
        )
        .await
        .unwrap();
    let mut queue = client
        .subscribe(openagents_rpc::methods::WATCH_QUEUE, json!({"chatId":chat}))
        .await
        .unwrap();
    // Wait for subscription setup before queuing; writes and subscriptions run concurrently.
    let initial = tokio::time::timeout(Duration::from_secs(10), queue.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(initial["items"].as_array().unwrap().is_empty());
    // A pending attachment keeps this fixture queued while the idle mock engine runs.
    let hold = format!("pending://{}/hold.png", uuid::Uuid::new_v4());
    let queued=client.call(openagents_rpc::methods::QUEUE_MESSAGE,json!({"chatId":chat,"text":"Relay fixture message","attachments":[hold],"holdForTurnEnd":true})).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = queue.recv().await.unwrap();
            if state["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == queued["id"] && item["text"] == "Relay fixture message")
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    let other = format!("stranger-{}", uuid::Uuid::new_v4());
    assert!(
        matches!(tokio_tungstenite::connect_async(openagents_rpc::device_room::device_room_ws_url(&s.base,&device,"client",Some("intruder"),&other)).await.unwrap_err(),tokio_tungstenite::tungstenite::Error::Http(r) if r.status()==403)
    );
    client
        .call(
            openagents_rpc::methods::REMOVE_QUEUED_MESSAGE,
            json!({"chatId":chat,"id":queued["id"]}),
        )
        .await
        .unwrap();
    // These are the same engine-free client and image bytes the phone uses.
    let phone = openagents_client::Client::new(
        openagents_client::ClientConfig::new(&s.base, scratch.path().join("phone")),
        openagents_client::Credentials::Dev {
            user_id: s.account.clone(),
            org_id: s.account.clone(),
        },
        std::sync::Arc::new(openagents_client::events::NullListener),
    )
    .unwrap();
    let mut second_config =
        openagents_client::ClientConfig::new(&s.base, scratch.path().join("second-viewer"));
    second_config.device_id = "second-viewer".into();
    let second = openagents_client::Client::new(
        second_config,
        openagents_client::Credentials::Dev {
            user_id: s.account.clone(),
            org_id: s.account.clone(),
        },
        std::sync::Arc::new(openagents_client::events::NullListener),
    )
    .unwrap();
    let mut random = 0x12345678u32;
    let pixels = image::RgbaImage::from_fn(768, 512, |x, y| {
        if y < 256 {
            image::Rgba(match x / 192 {
                0 => [240, 50, 60, 255],
                1 => [40, 190, 100, 255],
                2 => [30, 110, 240, 255],
                _ => [250, 210, 30, 255],
            })
        } else {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            image::Rgba([random as u8, (random >> 8) as u8, (random >> 16) as u8, 255])
        }
    });
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels.clone())
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    assert!(png.len() > 510 * 1024, "Fixture must cross upload chunks");
    let c = phone.clone();
    let device_id = device.clone();
    let uploaded = png.clone();
    let path = openagents_client::runtime::run(async move {
        c.upload_attachment(&device_id, "fixture image.png", uploaded, None)
            .await
    })
    .await
    .unwrap();
    let queued_image=client.call(openagents_rpc::methods::QUEUE_MESSAGE,json!({"chatId":chat,"text":"Image fixture","attachments":[path,hold],"holdForTurnEnd":true})).await.unwrap();
    let attachment = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = queue.recv().await.unwrap();
            if let Some(item) = state["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["id"] == queued_image["id"])
            {
                break item["attachments"][0].as_str().unwrap().to_owned();
            }
        }
    })
    .await
    .unwrap();
    let c = second.clone();
    let device_id = device.clone();
    let downloaded =
        openagents_client::runtime::run(
            async move { c.read_attachment(&device_id, &attachment).await },
        )
        .await
        .unwrap();
    assert_eq!(&**downloaded, &png);
    let rendered = image::load_from_memory(&downloaded).unwrap().to_rgba8();
    assert_eq!(rendered, pixels);
    rendered
        .save(
            std::path::Path::new(&std::env::var("OPENAGENTS_SCRATCH").unwrap())
                .join("11846-rendered.png"),
        )
        .unwrap();
    let c = phone.clone();
    let device_id = device.clone();
    assert!(
        openagents_client::runtime::run(async move {
            c.upload_attachment(
                &device_id,
                "too-large.png",
                vec![0; 24 * 1024 * 1024 + 1],
                None,
            )
            .await
        })
        .await
        .is_err()
    );
    phone.shutdown();
    second.shutdown();
    client
        .call(
            openagents_rpc::methods::REMOVE_QUEUED_MESSAGE,
            json!({"chatId":chat,"id":queued_image["id"]}),
        )
        .await
        .unwrap();
    client
        .call(
            openagents_rpc::methods::MUTATE,
            json!({"op":"deleteSpace","spaceId":space}),
        )
        .await
        .unwrap();
    ipc.call(openagents_rpc::methods::STOP_ENGINE, json!({}))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(exit) = engine.0.try_wait().unwrap() {
                assert!(exit.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn protocol_errors_keepalives_and_client_departure() {
    let s = Server::start().await;
    let mut host = s.ws("host", "h").await;
    let mut viewer = s.ws("client", "v").await;
    assert!(
        matches!(tokio_tungstenite::connect_async(s.url("client","v",&s.account)).await.unwrap_err(),tokio_tungstenite::tungstenite::Error::Http(r) if r.status()==409)
    );
    viewer
        .send(tokio_tungstenite::tungstenite::Message::Text("ping".into()))
        .await
        .unwrap();
    assert_eq!(
        viewer.next().await.unwrap().unwrap(),
        tokio_tungstenite::tungstenite::Message::Text("pong".into())
    );
    viewer
        .send(tokio_tungstenite::tungstenite::Message::Binary(vec![128]))
        .await
        .unwrap();
    assert_eq!(closed(&mut viewer).await, 1002);
    let (h, b) = next(&mut host).await;
    assert_eq!(h.from.as_deref(), Some("v"));
    assert_eq!(
        serde_json::from_slice::<Value>(&b).unwrap()["error"],
        "client_closed"
    );
}
#[tokio::test]
async fn unknown_host_receipt_is_never_delivered_to_another_account() {
    let s = Server::start().await;
    let chat = uuid::Uuid::new_v4().to_string();
    let result = reqwest::Client::new()
        .post(format!(
            "{}/chat2/{chat}/rows?batchId=b&device=phone&hostDevice={}",
            s.base, s.device
        ))
        .bearer_auth(&s.account)
        .body(vec![1])
        .send()
        .await
        .unwrap();
    assert!(result.status().is_success());
    let other = format!("unrelated-{}", uuid::Uuid::new_v4());
    let mut host = tokio_tungstenite::connect_async(s.url("host", "other-owner", &other))
        .await
        .unwrap()
        .0;
    assert!(
        tokio::time::timeout(Duration::from_millis(150), host.next())
            .await
            .is_err()
    );
    assert_eq!(s.pending().await, 1);
    assert_eq!(
        s.request(reqwest::Method::POST, "/nudge", &s.account)
            .json(&json!({"chatId":chat}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}
#[tokio::test]
async fn rejected_chat_transaction_cannot_publish_a_wakeup() {
    let s = Server::start().await;
    let chat = uuid::Uuid::new_v4().to_string();
    let name = format!("device_fail_{}", uuid::Uuid::new_v4().simple());
    let db = s.db.connect().await.unwrap();
    db.batch_execute(&format!("CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.chat_id='{chat}' THEN RAISE EXCEPTION 'fixture failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER {name} BEFORE INSERT ON chat_rows FOR EACH ROW EXECUTE FUNCTION {name}();")).await.unwrap();
    let response = reqwest::Client::new()
        .post(format!(
            "{}/chat2/{chat}/rows?batchId=b&device=phone&hostDevice={}",
            s.base, s.device
        ))
        .bearer_auth(&s.account)
        .body(vec![1])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(s.pending().await, 0);
    assert!(
        db.query_opt("SELECT 1 FROM chats WHERE chat_id=$1", &[&chat])
            .await
            .unwrap()
            .is_none()
    );
    db.batch_execute(&format!(
        "DROP TRIGGER {name} ON chat_rows; DROP FUNCTION {name}();"
    ))
    .await
    .unwrap();
}
#[tokio::test]
async fn wakeup_overflow_preserves_receipts_and_requests_a_full_scan() {
    let s = Server::start().await;
    let mut host = s.ws("host", "h").await;
    host.close(None).await.unwrap();
    let db = s.db.connect().await.unwrap();
    db.execute("INSERT INTO device_nudges(account,device_id,chat_id,token) SELECT $1,$2,'chat-'||i::text,'token-'||i::text FROM generate_series(1,4096) i",&[&s.account,&s.device]).await.unwrap();
    let response = s
        .request(reqwest::Method::POST, "/nudge", &s.account)
        .json(&json!({"chatId":"overflow"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(s.pending().await, 4096);
    assert_eq!(response.json::<Value>().await.unwrap()["retryable"], true);
    let mut host = s.ws("host", "full-scan").await;
    let (h, b) = next(&mut host).await;
    assert_eq!(h.s, "*");
    assert_eq!(serde_json::from_slice::<Value>(&b).unwrap()["chatId"], "*");
    ack(&mut host, &b).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if db
                .query_opt(
                    "SELECT 1 FROM device_reconcile WHERE account=$1 AND device_id=$2",
                    &[&s.account, &s.device],
                )
                .await
                .unwrap()
                .is_none()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(s.pending().await, 4096);
}
