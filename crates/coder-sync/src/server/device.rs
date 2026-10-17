//! Content-blind device relay. Authentication binds connections to an owner
//! or an explicit viewer grant; only routing headers change in transit.
use super::{
    StateData,
    chat::{Failure, identity},
    rooms::Room,
};
use axum::{
    Json,
    body::Bytes,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use openagents_rpc::device_room::{
    CLIENT_CLOSED, CLIENT_GONE, HOST_CLOSED, HOST_OFFLINE, NUDGE_KIND, RELAY_KIND,
};
use openagents_rpc::{DeviceFrameHeader, decode_device_frame, encode_device_frame};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
};
use tokio::sync::{Mutex, mpsc, watch};
use tokio_postgres::Transaction;
use uuid::Uuid;
type Result<T> = std::result::Result<T, Failure>;
type QueryMap = HashMap<String, String>;
const LIVE_MS: i64 = 75_000;
const NUDGE_PAGE: usize = 64;
const NUDGE_CAP: i64 = 4096;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Host,
    Client,
}
#[derive(Clone)]
struct Peer {
    conn: String,
    account: String,
    role: Role,
    seen: i64,
    closing: bool,
    tx: mpsc::Sender<Vec<u8>>,
    close: watch::Sender<Option<(u16, &'static str)>>,
    wake: watch::Sender<u64>,
}
#[derive(Default)]
struct Device {
    peers: HashMap<Uuid, Peer>,
}
#[derive(Clone, Default)]
pub struct Devices(Arc<Mutex<HashMap<String, Device>>>);
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn bad(code: &'static str) -> Failure {
    Failure(StatusCode::BAD_REQUEST, code)
}
/// Freshest live host, excluding the connection being closed.
pub fn pick_live_host<T: Copy + PartialEq>(
    hosts: &[(T, i64)],
    at: i64,
    exclude: Option<T>,
) -> Option<T> {
    let mut best = None;
    for &(id, seen) in hosts {
        if exclude == Some(id) {
            continue;
        }
        if best.is_none_or(|(_, last)| seen > last) {
            best = Some((id, seen));
        }
    }
    best.filter(|(_, seen)| at.saturating_sub(*seen) <= LIVE_MS)
        .map(|(id, _)| id)
}
impl Device {
    fn host(&self) -> Option<Uuid> {
        pick_live_host(
            &self
                .peers
                .iter()
                .filter(|(_, p)| p.role == Role::Host && !p.closing && !p.tx.is_closed())
                .map(|(id, p)| (*id, p.seen))
                .collect::<Vec<_>>(),
            now(),
            None,
        )
    }
    fn deliver(&mut self, id: Uuid, header: DeviceFrameHeader, payload: &[u8]) -> bool {
        let Some(peer) = self.peers.get_mut(&id).filter(|p| !p.closing) else {
            return false;
        };
        let Ok(frame) = encode_device_frame(&header, payload) else {
            return false;
        };
        if peer.tx.try_send(frame).is_ok() {
            true
        } else {
            peer.closing = true;
            let _ = peer
                .close
                .send(Some((1013, "The connection is busy. Reconnect.")));
            false
        }
    }
    fn error(&mut self, id: Uuid, header: DeviceFrameHeader, code: &str) {
        self.deliver(
            id,
            header,
            &serde_json::to_vec(&json!({"error":code})).unwrap(),
        );
    }
}
impl Devices {
    async fn register(&self, device: &str, id: Uuid, peer: Peer) -> Result<()> {
        let mut all = self.0.lock().await;
        all.retain(|_, room| {
            room.peers.retain(|_, p| !p.tx.is_closed());
            !room.peers.is_empty()
        });
        if !all.contains_key(device) && all.len() >= 4096 {
            return Err(Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"));
        }
        let room = all.entry(device.into()).or_default();
        room.peers.retain(|_, p| !p.tx.is_closed());
        if room.peers.len() >= 256 {
            return Err(Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"));
        }
        if peer.role == Role::Client
            && room
                .peers
                .values()
                .any(|p| p.role == Role::Client && p.conn == peer.conn && !p.closing)
        {
            return Err(Failure(StatusCode::CONFLICT, "connection_exists"));
        }
        if peer.role == Role::Host {
            for p in room.peers.values_mut().filter(|p| p.role == Role::Host) {
                p.closing = true;
                let _ = p.close.send(Some((4409, "A new host connected.")));
            }
        }
        room.peers.insert(id, peer);
        Ok(())
    }
    async fn touch(&self, device: &str, id: Uuid) {
        if let Some(p) = self
            .0
            .lock()
            .await
            .get_mut(device)
            .and_then(|d| d.peers.get_mut(&id))
        {
            p.seen = now();
        }
    }
    async fn route(&self, device: &str, id: Uuid, header: DeviceFrameHeader, payload: &[u8]) {
        let mut all = self.0.lock().await;
        let Some(room) = all.get_mut(device) else {
            return;
        };
        let Some(peer) = room.peers.get(&id).filter(|p| !p.closing).cloned() else {
            return;
        };
        if peer.role == Role::Client {
            if let Some(host) = room.host() {
                let stamped = DeviceFrameHeader {
                    s: header.s.clone(),
                    k: header.k,
                    to: None,
                    from: Some(peer.conn),
                };
                if room.deliver(host, stamped, payload) {
                    return;
                }
            }
            room.error(
                id,
                DeviceFrameHeader::new(header.s, RELAY_KIND),
                HOST_OFFLINE,
            );
            return;
        }
        let Some(to) = header.to else {
            return;
        };
        let target = room
            .peers
            .iter()
            .find(|(_, p)| p.role == Role::Client && p.conn == to && !p.closing)
            .map(|(id, _)| *id);
        if let Some(target) = target {
            room.deliver(target, DeviceFrameHeader::new(header.s, header.k), payload);
        } else {
            room.error(
                id,
                DeviceFrameHeader::new(header.s, RELAY_KIND).with_to(to),
                CLIENT_GONE,
            );
        }
    }
    pub(crate) async fn wake_host(&self, device: &str) -> bool {
        let all = self.0.lock().await;
        let Some(room) = all.get(device) else {
            return false;
        };
        let Some(host) = room.host().and_then(|id| room.peers.get(&id)) else {
            return false;
        };
        host.wake.send_modify(|n| *n = n.wrapping_add(1));
        true
    }
    async fn deliver_nudge(&self, device: &str, host: Uuid, chat: &str, token: &str) -> bool {
        let mut all = self.0.lock().await;
        let Some(room) = all.get_mut(device) else {
            return false;
        };
        if room.host() != Some(host) {
            return false;
        }
        room.deliver(
            host,
            DeviceFrameHeader::new(chat, NUDGE_KIND),
            &serde_json::to_vec(&json!({"chatId":chat,"token":token})).unwrap(),
        )
    }
    async fn disconnected(&self, device: &str, id: Uuid) {
        let mut all = self.0.lock().await;
        let Some(room) = all.get_mut(device) else {
            return;
        };
        let Some(peer) = room.peers.remove(&id) else {
            return;
        };
        if peer.role == Role::Client {
            if let Some(host) = room.host() {
                let mut header = DeviceFrameHeader::new("", RELAY_KIND);
                header.from = Some(peer.conn);
                room.error(host, header, CLIENT_CLOSED);
            }
        } else if room.host().is_none() {
            let clients = room
                .peers
                .iter()
                .filter(|(_, p)| p.role == Role::Client)
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            for client in clients {
                room.error(client, DeviceFrameHeader::new("", RELAY_KIND), HOST_CLOSED);
            }
        }
        if room.peers.is_empty() {
            all.remove(device);
        }
    }
    async fn counts(&self, device: &str) -> (bool, usize) {
        let all = self.0.lock().await;
        let Some(room) = all.get(device) else {
            return (false, 0);
        };
        (
            room.host().is_some(),
            room.peers
                .values()
                .filter(|p| p.role == Role::Host && !p.tx.is_closed())
                .count(),
        )
    }
    async fn revoke(&self, device: &str, account: &str) {
        if let Some(room) = self.0.lock().await.get_mut(device) {
            for p in room
                .peers
                .values_mut()
                .filter(|p| p.role == Role::Client && p.account == account)
            {
                p.closing = true;
                let _ = p.close.send(Some((4403, "Access was removed.")));
            }
        }
    }
}
async fn run<T: Send + 'static, F: Future<Output = Result<T>> + Send + 'static>(
    state: &StateData,
    id: &str,
    job: F,
) -> Result<T> {
    state
        .rooms
        .run(Room::Device(id.into()), job)
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"))?
}
async fn authorize(
    tx: &Transaction<'_>,
    device: &str,
    account: &str,
    claim: bool,
    owner_only: bool,
) -> Result<String> {
    if claim {
        tx.execute(
            "INSERT INTO devices(device_id,account) VALUES($1,$2) ON CONFLICT DO NOTHING",
            &[&device, &account],
        )
        .await?;
    }
    let owner = tx
        .query_opt(
            "SELECT account FROM devices WHERE device_id=$1 FOR UPDATE",
            &[&device],
        )
        .await?;
    let Some(owner) = owner else {
        return Err(Failure(
            if claim {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::NOT_FOUND
            },
            "forbidden",
        ));
    };
    let owner = owner.get::<_, String>(0);
    if owner == account {
        return Ok(owner);
    }
    if !owner_only
        && tx
            .query_opt(
                "SELECT 1 FROM device_grants WHERE device_id=$1 AND account=$2",
                &[&device, &account],
            )
            .await?
            .is_some()
    {
        return Ok(owner);
    }
    Err(Failure(StatusCode::FORBIDDEN, "forbidden"))
}
async fn check(state: &StateData, device: &str, account: &str, owner_only: bool) -> Result<()> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    authorize(&tx, device, account, false, owner_only).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn status(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    let s = state.clone();
    let device = id.clone();
    run(&state, &id, async move {
        check(&s, &device, &who.account, false).await?;
        let (live, count) = s.devices.counts(&device).await;
        Ok(Json(json!({"hostConnected":live,"hostSockets":count})))
    })
    .await
}
fn valid_sidecar(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
pub async fn sidecar(
    State(state): State<Arc<StateData>>,
    Path((id, name)): Path<(String, String)>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    if !valid_sidecar(&name) {
        return Err(bad("bad_sidecar"));
    }
    let s = state.clone();
    let device = id.clone();
    run(&state, &id, async move {
        let mut db = s.database.connect().await?;
        let tx = db.transaction().await?;
        authorize(&tx, &device, &who.account, false, false).await?;
        let row = tx
            .query_one(
                "SELECT sidecars::text FROM devices WHERE device_id=$1",
                &[&device],
            )
            .await?;
        let sidecars: Value = serde_json::from_str(&row.get::<_, String>(0))
            .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
        let value = sidecars
            .get(&name)
            .cloned()
            .ok_or(Failure(StatusCode::NOT_FOUND, "not_found"))?;
        tx.commit().await?;
        Ok(Json(value))
    })
    .await
}
pub async fn publish_sidecar(
    State(state): State<Arc<StateData>>,
    Path((id, name)): Path<(String, String)>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    if !valid_sidecar(&name) {
        return Err(bad("bad_sidecar"));
    }
    let value: Value = serde_json::from_slice(&body).map_err(|_| bad("bad_sidecar"))?;
    let value = value.to_string();
    let s = state.clone();
    let device = id.clone();
    run(&state,&id,async move{let mut db=s.database.connect().await?;let tx=db.transaction().await?;authorize(&tx,&device,&who.account,false,true).await?;tx.execute("UPDATE devices SET sidecars=jsonb_set(sidecars,ARRAY[$2::text],$3::text::jsonb,true) WHERE device_id=$1",&[&device,&name,&value]).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))}).await
}
pub async fn grant(
    State(state): State<Arc<StateData>>,
    Path((id, account)): Path<(String, String)>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    if account.is_empty() || account.len() > 256 || account.chars().any(char::is_control) {
        return Err(bad("bad_account"));
    }
    let s = state.clone();
    let device = id.clone();
    run(&state, &id, async move {
        let mut db = s.database.connect().await?;
        let tx = db.transaction().await?;
        authorize(&tx, &device, &who.account, false, true).await?;
        tx.execute(
            "INSERT INTO device_grants(device_id,account) VALUES($1,$2) ON CONFLICT DO NOTHING",
            &[&device, &account],
        )
        .await?;
        tx.commit().await?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
pub async fn revoke(
    State(state): State<Arc<StateData>>,
    Path((id, account)): Path<(String, String)>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    let s = state.clone();
    let device = id.clone();
    run(&state, &id, async move {
        let mut db = s.database.connect().await?;
        let tx = db.transaction().await?;
        authorize(&tx, &device, &who.account, false, true).await?;
        tx.execute(
            "DELETE FROM device_grants WHERE device_id=$1 AND account=$2",
            &[&device, &account],
        )
        .await?;
        tx.commit().await?;
        s.devices.revoke(&device, &account).await;
        Ok(Json(json!({"ok":true})))
    })
    .await
}

/// Account-scoped receipts can precede the host's first claim. They are
/// consumed only after that account proves ownership of the host connection.
pub(crate) async fn enqueue_nudge(
    tx: &Transaction<'_>,
    account: &str,
    device: &str,
    chat: &str,
) -> Result<bool> {
    let key = format!("{account}:{device}");
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,11845))",
        &[&key],
    )
    .await?;
    let exists = tx
        .query_opt(
            "SELECT 1 FROM device_nudges WHERE account=$1 AND device_id=$2 AND chat_id=$3",
            &[&account, &device, &chat],
        )
        .await?
        .is_some();
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM device_nudges WHERE account=$1 AND device_id=$2",
            &[&account, &device],
        )
        .await?
        .get(0);
    let token = Uuid::new_v4().to_string();
    if !exists && count >= NUDGE_CAP {
        tx.execute("INSERT INTO device_reconcile(account,device_id,token) VALUES($1,$2,$3) ON CONFLICT(account,device_id) DO UPDATE SET token=EXCLUDED.token",&[&account,&device,&token]).await?;
        return Ok(false);
    }
    tx.execute("INSERT INTO device_nudges(account,device_id,chat_id,token) VALUES($1,$2,$3,$4) ON CONFLICT(account,device_id,chat_id) DO UPDATE SET token=EXCLUDED.token,queued_at=now()",&[&account,&device,&chat,&token]).await?;
    Ok(true)
}
pub async fn nudge(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &id)?;
    let body: Value = serde_json::from_slice(&body).map_err(|_| bad("bad_chat_id"))?;
    let chat = body["chatId"]
        .as_str()
        .filter(|c| {
            !c.is_empty()
                && c.len() <= 64
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
        .ok_or(bad("bad_chat_id"))?
        .to_owned();
    let s = state.clone();
    let device = id.clone();
    run(&state, &id, async move {
        let mut db = s.database.connect().await?;
        let tx = db.transaction().await?;
        let owner = authorize(&tx, &device, &who.account, false, false).await?;
        let queued = enqueue_nudge(&tx, &owner, &device, &chat).await?;
        tx.commit().await?;
        let delivered = s.devices.wake_host(&device).await;
        Ok(if queued {
            Json(json!({"delivered":delivered,"queued":true})).into_response()
        } else {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":"nudge_queue_full","retryable":true})),
            )
                .into_response()
        })
    })
    .await
}
async fn replay(
    state: &StateData,
    device: &str,
    account: &str,
    host: Uuid,
    acked: bool,
    mut inflight: HashSet<String>,
) -> Result<HashSet<String>> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    authorize(&tx, device, account, false, true).await?;
    let mut pending = Vec::<(String, String)>::new();
    if let Some(row) = tx
        .query_opt(
            "SELECT token FROM device_reconcile WHERE account=$1 AND device_id=$2",
            &[&account, &device],
        )
        .await?
    {
        pending.push(("*".into(), row.get(0)));
    }
    pending.extend(tx.query("SELECT chat_id,token FROM device_nudges WHERE account=$1 AND device_id=$2 ORDER BY queued_at,chat_id LIMIT 128",&[&account,&device]).await?.iter().map(|r|(r.get::<_,String>(0),r.get::<_,String>(1))));
    let mut sent = 0;
    for (chat, token) in pending {
        if (acked && inflight.contains(&token)) || (!acked && chat == "*") {
            continue;
        }
        if (acked && inflight.len() >= NUDGE_PAGE) || (!acked && sent >= NUDGE_PAGE) {
            break;
        }
        if !state
            .devices
            .deliver_nudge(device, host, &chat, &token)
            .await
        {
            break;
        }
        if acked {
            inflight.insert(token);
        } else {
            tx.execute("DELETE FROM device_nudges WHERE account=$1 AND device_id=$2 AND chat_id=$3 AND token=$4",&[&account,&device,&chat,&token]).await?;
        }
        sent += 1;
    }
    tx.commit().await?;
    Ok(inflight)
}
async fn acknowledge(
    state: &StateData,
    device: &str,
    account: &str,
    payload: &[u8],
) -> Result<Option<String>> {
    let Ok(value) = serde_json::from_slice::<Value>(payload) else {
        return Ok(None);
    };
    let (Some(chat), Some(token)) = (value["chatId"].as_str(), value["token"].as_str()) else {
        return Ok(None);
    };
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    authorize(&tx, device, account, false, true).await?;
    if chat == "*" {
        tx.execute(
            "DELETE FROM device_reconcile WHERE account=$1 AND device_id=$2 AND token=$3",
            &[&account, &device, &token],
        )
        .await?;
    } else {
        tx.execute("DELETE FROM device_nudges WHERE account=$1 AND device_id=$2 AND chat_id=$3 AND token=$4",&[&account,&device,&chat,&token]).await?;
    }
    tx.commit().await?;
    Ok(Some(token.into()))
}
pub async fn websocket(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<QueryMap>,
    headers: HeaderMap,
    upgrade: std::result::Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &id)?;
    let lease = super::auth::socket_lease(&state, &headers, &q, &who);
    let upgrade =
        upgrade.map_err(|_| Failure(StatusCode::UPGRADE_REQUIRED, "expected_websocket"))?;
    let role = if q.get("role").is_some_and(|r| r == "host") {
        Role::Host
    } else {
        Role::Client
    };
    let peerid = Uuid::new_v4();
    let conn = q
        .get("connId")
        .cloned()
        .unwrap_or_else(|| peerid.to_string());
    if conn.is_empty() || conn.len() > 128 || conn.chars().any(char::is_control) {
        return Err(bad("bad_connection"));
    }
    let acked = q.get("nudgeAck").is_some_and(|v| v == "1");
    let (tx, rx) = mpsc::channel(64);
    let (close, closing) = watch::channel(None);
    let (wake, waking) = watch::channel(0);
    let peer = Peer {
        conn,
        account: who.account.clone(),
        role,
        seen: now(),
        closing: false,
        tx,
        close,
        wake,
    };
    let s = state.clone();
    let device = id.clone();
    let account = who.account.clone();
    run(&state,&id,async move{
        let mut db=s.database.connect().await?;let txn=db.transaction().await?;
        authorize(&txn,&device,&account,role==Role::Host,role==Role::Host).await.map_err(|e|if e.0==StatusCode::NOT_FOUND{Failure(StatusCode::FORBIDDEN,"forbidden")}else{e})?;
        // Drop expired diagnostic sessions left by a process crash.
        txn.execute("DELETE FROM device_sessions WHERE device_id=$1 AND last_seen_at<now()-interval '75 seconds'",&[&device]).await?;
        let key=peerid.to_string();let role_name=if role==Role::Host{"host"}else{"client"};
        txn.execute("INSERT INTO device_sessions(device_id,connection_id,role) VALUES($1,$2,$3)",&[&device,&key,&role_name]).await?;
        txn.commit().await?;s.devices.register(&device,peerid,peer).await
    }).await?;
    Ok(upgrade
        .max_message_size(1024 * 1024 + 8192)
        .on_upgrade(move |socket| {
            session(
                socket,
                state,
                id,
                who.account,
                peerid,
                role,
                acked,
                rx,
                closing,
                waking,
                lease,
            )
        }))
}
async fn flush_nudges(
    state: &StateData,
    device: &str,
    account: &str,
    host: Uuid,
    acked: bool,
    inflight: HashSet<String>,
) -> Result<HashSet<String>> {
    let s = state.clone();
    let id = device.to_owned();
    let account = account.to_owned();
    run(state, device, async move {
        replay(&s, &id, &account, host, acked, inflight).await
    })
    .await
}
async fn session(
    mut socket: WebSocket,
    state: Arc<StateData>,
    id: String,
    account: String,
    peer: Uuid,
    role: Role,
    acked: bool,
    mut outbound: mpsc::Receiver<Vec<u8>>,
    mut closing: watch::Receiver<Option<(u16, &'static str)>>,
    mut waking: watch::Receiver<u64>,
    lease: Option<super::auth::SocketLease>,
) {
    let mut inflight = HashSet::new();
    if role == Role::Host {
        if let Ok(pending) =
            flush_nudges(&state, &id, &account, peer, acked, inflight.clone()).await
        {
            inflight = pending;
        }
    }
    let mut retry = tokio::time::interval(std::time::Duration::from_secs(5));
    retry.tick().await;
    let mut last_pulse = std::time::Instant::now();
    let ended = super::auth::socket_ended(lease);
    tokio::pin!(ended);
    loop {
        tokio::select! {
            code=&mut ended=>{let reason=if code==4401{"Log in again to continue."}else{"Account service is unavailable. Try again."};let _=socket.send(Message::Close(Some(axum::extract::ws::CloseFrame{code,reason:reason.into()}))).await;break;},
            change=closing.changed()=>{
                if change.is_err(){break;}let command=*closing.borrow();if let Some((code,reason))=command{let _=socket.send(Message::Close(Some(CloseFrame{code,reason:reason.into()}))).await;break;}
            },
            frame=outbound.recv()=>{let Some(frame)=frame else{break;};if !matches!(tokio::time::timeout(std::time::Duration::from_secs(5),socket.send(Message::Binary(frame.into()))).await,Ok(Ok(()))){break;}},
            changed=waking.changed(),if role==Role::Host=>{if changed.is_err(){break;}if let Ok(pending)=flush_nudges(&state,&id,&account,peer,acked,inflight.clone()).await{inflight=pending;}},
            _=retry.tick(),if role==Role::Host=>{inflight.clear();if let Ok(pending)=flush_nudges(&state,&id,&account,peer,acked,inflight.clone()).await{inflight=pending;}},
            incoming=socket.recv()=>{
                if incoming.as_ref().is_some_and(|m| m.is_ok()) && last_pulse.elapsed() >= std::time::Duration::from_secs(10) {
                    if let Ok(db) = state.database.connect().await {
                        let _ = db.execute("UPDATE device_sessions SET last_seen_at=now() WHERE device_id=$1 AND connection_id=$2", &[&id, &peer.to_string()]).await;
                    }
                    last_pulse = std::time::Instant::now();
                }
                let frame=match incoming{
                    Some(Ok(Message::Text(t)))=>{state.devices.touch(&id,peer).await;if t=="ping"&&socket.send(Message::Text("pong".into())).await.is_err(){break;}continue;},
                    Some(Ok(Message::Ping(b)))=>{state.devices.touch(&id,peer).await;if socket.send(Message::Pong(b)).await.is_err(){break;}continue;},
                    Some(Ok(Message::Pong(_)))=>{state.devices.touch(&id,peer).await;continue;},
                    Some(Ok(Message::Binary(frame)))=>frame,
                    _=>break,
                };
                let Ok((header,payload))=decode_device_frame(&frame)else{let _=socket.send(Message::Close(Some(CloseFrame{code:1002,reason:"Invalid frame.".into()}))).await;break;};
                if header.s.len()>1024||header.k.len()>128||header.to.as_ref().is_some_and(|v|v.len()>128)||header.from.as_ref().is_some_and(|v|v.len()>128){break;}
                state.devices.touch(&id,peer).await;
                if role==Role::Host&&acked&&header.k=="nudgeAck"{
                    let s=state.clone();let device=id.clone();let owner=account.clone();
                    if let Ok(Some(token))=run(&state,&id,async move{acknowledge(&s,&device,&owner,&payload).await}).await{inflight.remove(&token);}
                    if let Ok(pending)=flush_nudges(&state,&id,&account,peer,acked,inflight.clone()).await{inflight=pending;}
                }else{let s=state.clone();let device=id.clone();let _=run(&state,&id,async move{s.devices.route(&device,peer,header,&payload).await;Ok(())}).await;}
            }
        }
    }
    let s = state.clone();
    let device = id.clone();
    let _ = run(&state, &id, async move {
        s.devices.disconnected(&device, peer).await;
        let db = s.database.connect().await?;
        let key = peer.to_string();
        db.execute(
            "DELETE FROM device_sessions WHERE device_id=$1 AND connection_id=$2",
            &[&device, &key],
        )
        .await?;
        Ok(())
    })
    .await;
}
