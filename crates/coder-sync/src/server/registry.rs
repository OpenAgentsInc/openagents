//! Account-private registry rows. Merge semantics come from the pinned fork;
//! one PostgreSQL transaction publishes every changed row in a batch.
use super::{
    Identity, StateData,
    chat::{Failure, identity},
    rooms::Room,
};
use axum::{
    Json,
    body::Bytes,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    sync::Arc,
};
use tokio_postgres::Transaction;
use uuid::Uuid;
use zeron_doc::{OpKind, RegistryRow, RowOp, apply_op};
type Result<T> = std::result::Result<T, Failure>;
fn bad(code: &'static str) -> Failure {
    Failure(StatusCode::BAD_REQUEST, code)
}
fn scope(
    state: &StateData,
    headers: &HeaderMap,
    q: &HashMap<String, String>,
    org: &str,
) -> Result<Identity> {
    let who = identity(state, headers, q, org)?;
    if org != who.account && who.workspace.as_deref() != Some(org) {
        return Err(Failure(StatusCode::FORBIDDEN, "forbidden"));
    }
    if q.get("device").is_some_and(|d| d.len() > 128) {
        return Err(bad("bad_device"));
    }
    Ok(who)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn valid_name(s: &str, first: fn(u8) -> bool, rest: fn(u8) -> bool, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && first(s.as_bytes()[0]) && s.bytes().all(rest)
}
fn letter(b: u8) -> bool {
    b.is_ascii_alphabetic()
}
fn alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}
pub fn valid_hlc(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 22
        && b.len() <= 149
        && b[..13].iter().all(u8::is_ascii_digit)
        && b[13] == b'-'
        && b[14..20].iter().all(u8::is_ascii_digit)
        && b[20] == b'-'
        && b[21..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(b))
}
pub fn validate_op(value: &Value) -> Result<RowOp> {
    if value.get("clocks").is_some_and(|v| !v.is_object()) {
        return Err(bad("invalid_op"));
    }
    let op: RowOp = serde_json::from_value(value.clone()).map_err(|_| bad("invalid_op"))?;
    let field = |s: &str| valid_name(s, letter, alnum, 64);
    if !valid_name(&op.kind, |b| b.is_ascii_lowercase(), alnum, 32)
        || !valid_name(
            &op.id,
            |b| b.is_ascii_alphanumeric() || b"_.:@/-".contains(&b),
            |b| b.is_ascii_alphanumeric() || b"_.:@/-".contains(&b),
            256,
        )
        || !valid_hlc(&op.hlc)
    {
        return Err(bad("invalid_op"));
    }
    if op.op == OpKind::Delete {
        if value.get("set").is_some() {
            return Err(bad("invalid_op"));
        }
    } else if !value.get("set").is_some_and(Value::is_object)
        || op
            .set
            .as_ref()
            .is_none_or(|set| !set.keys().all(|k| field(k)))
    {
        return Err(bad("invalid_op"));
    }
    if op
        .clocks
        .as_ref()
        .is_some_and(|c| c.iter().any(|(k, v)| !field(k) || !valid_hlc(v)))
    {
        return Err(bad("invalid_op"));
    }
    if value.to_string().encode_utf16().count() > 16 * 1024 {
        return Err(bad("invalid_op"));
    }
    if op.kind == "sidebarPins" {
        if op.op == OpKind::Delete {
            return Err(bad("invalid_op"));
        }
        for (k, v) in op.set.as_ref().unwrap() {
            let valid = match k.as_str() {
                "pinned" => v.is_boolean(),
                "orderKey" => v.as_str().is_some_and(|s| {
                    !s.is_empty()
                        && s.len() <= 8192
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || b"abcdef".contains(&b))
                        && s.as_bytes().last() != Some(&b'0')
                }),
                _ => false,
            };
            if !valid {
                return Err(bad("invalid_op"));
            }
        }
    }
    Ok(op)
}
async fn lock(tx: &Transaction<'_>, account: &str) -> Result<(i64, i64)> {
    tx.execute(
        "INSERT INTO registry_heads(account) VALUES($1) ON CONFLICT DO NOTHING",
        &[&account],
    )
    .await?;
    let row = tx
        .query_one(
            "SELECT seq,gc_floor FROM registry_heads WHERE account=$1 FOR UPDATE",
            &[&account],
        )
        .await?;
    Ok((row.get(0), row.get(1)))
}
async fn run<T: Send + 'static, F: Future<Output = Result<T>> + Send + 'static>(
    state: &StateData,
    account: &str,
    job: F,
) -> Result<T> {
    state
        .rooms
        .run(Room::Registry(account.into()), job)
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"))?
}
async fn head(state: &StateData, account: &str) -> Result<i64> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let (seq, _) = lock(&tx, account).await?;
    tx.commit().await?;
    Ok(seq)
}
async fn snapshot(state: &StateData, account: &str, cursor: Option<i64>) -> Result<Value> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let (seq, floor) = lock(&tx, account).await?;
    let full = cursor.is_none_or(|c| c < floor || c > seq);
    let after = if full { 0 } else { cursor.unwrap() };
    let rows=tx.query("SELECT value::text FROM registry_rows WHERE account=$1 AND seq>$2 ORDER BY seq,kind,id",&[&account,&after]).await?.iter().map(|r|serde_json::from_str::<RegistryRow>(&r.get::<_,String>(0)).map_err(|_|Failure(StatusCode::SERVICE_UNAVAILABLE,"unavailable"))).collect::<Result<Vec<_>>>()?;
    tx.commit().await?;
    let presence = state
        .rooms
        .presence(Room::Registry(account.into()), None)
        .await;
    Ok(json!({"t":"state","seq":seq,"full":full,"gcFloor":floor,"rows":rows,"presence":presence}))
}
struct Applied {
    ack: Value,
}
async fn apply(
    state: &StateData,
    account: &str,
    device: &str,
    body: Value,
    sender: Uuid,
) -> Result<Applied> {
    let batch = body["batch"]
        .as_str()
        .filter(|b| !b.is_empty() && b.len() <= 256)
        .ok_or(bad("bad_push"))?;
    let ops = body["ops"]
        .as_array()
        .filter(|o| !o.is_empty() && o.len() <= 500)
        .ok_or(bad("bad_push"))?
        .iter()
        .map(validate_op)
        .collect::<Result<Vec<_>>>()?;
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let (head, _) = lock(&tx, account).await?;
    let next = head.checked_add(1).ok_or(Failure(
        StatusCode::INSUFFICIENT_STORAGE,
        "sequence_exhausted",
    ))?;
    let mut touched = BTreeMap::<(String, String), RegistryRow>::new();
    let mut applied = 0;
    for op in ops {
        let key = (op.kind.clone(), op.id.clone());
        let before = if let Some(row) = touched.get(&key) {
            Some(row.clone())
        } else {
            tx.query_opt(
                "SELECT value::text FROM registry_rows WHERE account=$1 AND kind=$2 AND id=$3",
                &[&account, &op.kind, &op.id],
            )
            .await?
            .map(|r| {
                serde_json::from_str::<RegistryRow>(&r.get::<_, String>(0))
                    .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))
            })
            .transpose()?
        };
        let (row, changed) = apply_op(before.as_ref(), &op);
        if changed {
            if let Some(mut row) = row {
                row.seq = next as u64;
                applied += 1;
                touched.insert(key, row);
            }
        }
    }
    if applied > 0 {
        for row in touched.values() {
            let value = serde_json::to_string(row).expect("registry row serializes");
            tx.execute("INSERT INTO registry_rows(account,kind,id,seq,clock,writer,deleted,value) VALUES($1,$2,$3,$4,0,$5,$6,$7::text::jsonb) ON CONFLICT(account,kind,id) DO UPDATE SET seq=EXCLUDED.seq,writer=EXCLUDED.writer,deleted=EXCLUDED.deleted,value=EXCLUDED.value",&[&account,&row.kind,&row.id,&next,&device,&row.deleted,&value]).await?;
        }
        tx.execute(
            "UPDATE registry_heads SET seq=$2 WHERE account=$1",
            &[&account, &next],
        )
        .await?;
    }
    tx.commit().await?;
    let seq = if applied > 0 { next } else { head };
    let rows = (applied > 0)
        .then(|| json!({"t":"rows","seq":seq,"rows":touched.values().collect::<Vec<_>>()}));
    if let Some(rows) = &rows {
        let _ = state
            .rooms
            .events(Room::Registry(account.into()))
            .await
            .send((Uuid::nil(), serde_json::to_vec(rows).unwrap()));
    }
    let ack = json!({"t":"ack","batch":batch,"seq":seq,"applied":applied});
    if sender != Uuid::nil() {
        // Rows and the sender's acknowledgment share the ordered room bus.
        // A socket handling an incoming push must not let its newer ack
        // overtake another writer's queued rows and skip them on reconnect.
        let _ = state
            .rooms
            .events(Room::Registry(account.into()))
            .await
            .send((sender, serde_json::to_vec(&ack).unwrap()));
    }
    Ok(Applied { ack })
}
fn cursor(q: &HashMap<String, String>) -> Option<i64> {
    q.get("since")
        .and_then(|s| s.parse().ok())
        .filter(|c| *c >= 0)
}
async fn beat(state: &StateData, account: &str, device: &str, source: Uuid, at: i64) {
    if device.is_empty() {
        return;
    }
    state
        .rooms
        .presence(Room::Registry(account.into()), Some((device.into(), at)))
        .await;
    let _ = state
        .rooms
        .events(Room::Registry(account.into()))
        .await
        .send((
            source,
            serde_json::to_vec(&json!({"t":"presence","device":device,"at":at})).unwrap(),
        ));
}
pub async fn rows(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    let s = state.clone();
    let account = who.account.clone();
    let since = cursor(&q);
    let device = if q.get("beat").is_some_and(|v| v == "1") {
        q.get("device").cloned()
    } else {
        None
    };
    let snap = run(&state, &who.account, async move {
        if let Some(device) = device {
            beat(&s, &account, &device, Uuid::nil(), now()).await;
        }
        snapshot(&s, &account, since).await
    })
    .await?;
    Ok(Json(snap))
}
pub async fn push(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    if body.len() > 2 * 1024 * 1024 {
        return Err(Failure(StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    }
    let frame = serde_json::from_slice(&body).map_err(|_| bad("bad_push"))?;
    let s = state.clone();
    let account = who.account.clone();
    let device = q.get("device").cloned().unwrap_or_default();
    let applied = run(&state, &who.account, async move {
        apply(&s, &account, &device, frame, Uuid::nil()).await
    })
    .await?;
    Ok(Json(applied.ack))
}
pub async fn stats(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    let s = state.clone();
    let account = who.account.clone();
    run(&state,&who.account,async move{
        let snap=snapshot(&s,&account,None).await?;
        let db=s.database.connect().await?;
        let targets=db.query("SELECT device,environment,prefs::text FROM registry_push_targets WHERE account=$1 ORDER BY device",&[&account]).await?.iter().map(|r|json!({"device":r.get::<_,String>(0),"environment":r.get::<_,String>(1),"prefs":serde_json::from_str::<Value>(&r.get::<_,String>(2)).unwrap()})).collect::<Vec<_>>();
        let rows=snap["rows"].as_array().unwrap();let connected=s.rooms.events(Room::Registry(account)).await.receiver_count();
        Ok(Json(json!({"seq":snap["seq"],"gcFloor":snap["gcFloor"],"rowCount":rows.len(),"tombstones":rows.iter().filter(|r|r["deleted"]==true).count(),"connectedSockets":connected,"presence":snap["presence"],"pushTargets":targets,"pushConfigured":false,"pushLog":[],"pushOutcomes":{},"lastBackupSeq":0,"lastGcAt":0})))
    }).await
}
pub async fn reset(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    let s = state.clone();
    let account = who.account.clone();
    run(&state, &who.account, async move {
        let mut db = s.database.connect().await?;
        let tx = db.transaction().await?;
        lock(&tx, &account).await?;
        tx.execute("DELETE FROM registry_rows WHERE account=$1", &[&account])
            .await?;
        tx.execute(
            "UPDATE registry_heads SET seq=0,gc_floor=0 WHERE account=$1",
            &[&account],
        )
        .await?;
        tx.commit().await?;
        let _ = s
            .rooms
            .events(Room::Registry(account))
            .await
            .send((Uuid::nil(), br#"{"t":"reset"}"#.to_vec()));
        Ok(Json(json!({"ok":true})))
    })
    .await
}
pub async fn push_target(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    let device = q
        .get("device")
        .filter(|d| !d.is_empty())
        .cloned()
        .ok_or(bad("bad_request"))?;
    let body: Value = serde_json::from_slice(&body).map_err(|_| bad("bad_request"))?;
    let token = body["token"]
        .as_str()
        .filter(|t| (32..=512).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(bad("bad_request"))?
        .to_owned();
    let environment = body["environment"]
        .as_str()
        .filter(|e| matches!(*e, "sandbox" | "production"))
        .ok_or(bad("bad_request"))?
        .to_owned();
    let prefs = serde_json::to_string(&super::notifications::parse_prefs(&body["prefs"]))
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
    let s = state.clone();
    let account = who.account.clone();
    run(&state,&who.account,async move{let mut db=s.database.connect().await?;let tx=db.transaction().await?;lock(&tx,&account).await?;tx.execute("INSERT INTO registry_push_targets(account,device,token,environment,prefs) VALUES($1,$2,$3,$4,$5::text::jsonb) ON CONFLICT(account,device) DO UPDATE SET token=EXCLUDED.token,environment=EXCLUDED.environment,prefs=EXCLUDED.prefs,updated_at=now()",&[&account,&device,&token,&environment,&prefs]).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))}).await
}
pub async fn delete_target(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let who = scope(&state, &headers, &q, &org)?;
    let device = q
        .get("device")
        .filter(|d| !d.is_empty())
        .cloned()
        .ok_or(bad("bad_request"))?;
    let s = state.clone();
    let account = who.account.clone();
    run(&state, &who.account, async move {
        let db = s.database.connect().await?;
        db.execute(
            "DELETE FROM registry_push_targets WHERE account=$1 AND device=$2",
            &[&account, &device],
        )
        .await?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}

pub async fn websocket(
    State(state): State<Arc<StateData>>,
    Path(org): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    upgrade: std::result::Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response> {
    let who = scope(&state, &headers, &q, &org)?;
    let upgrade =
        upgrade.map_err(|_| Failure(StatusCode::UPGRADE_REQUIRED, "expected_websocket"))?;
    let s = state.clone();
    let account = who.account.clone();
    run(
        &state,
        &who.account,
        async move { head(&s, &account).await },
    )
    .await?;
    let events = state
        .rooms
        .events(Room::Registry(who.account.clone()))
        .await
        .subscribe();
    Ok(upgrade
        .max_message_size(1_000_000)
        .on_upgrade(move |socket| {
            session(
                socket,
                state,
                who.account,
                q.get("device").cloned().unwrap_or_default(),
                events,
            )
        }))
}
async fn send(ws: &mut WebSocket, value: Value) -> bool {
    ws.send(Message::Text(value.to_string().into()))
        .await
        .is_ok()
}
async fn session(
    mut ws: WebSocket,
    state: Arc<StateData>,
    account: String,
    mut device: String,
    mut events: tokio::sync::broadcast::Receiver<(Uuid, Vec<u8>)>,
) {
    let sender = Uuid::new_v4();
    let mut ready = false;
    loop {
        tokio::select! {
            event=events.recv()=>match event{
                Ok((source,bytes)) if ready=>{
                    let Ok(frame)=serde_json::from_slice::<Value>(&bytes)else{break;};
                    if (frame["t"]=="ack" && source!=sender) || (frame["t"]=="presence" && source==sender) {continue;}
                    if frame["t"]=="reset"{let _=ws.send(Message::Close(Some(axum::extract::ws::CloseFrame{code:4410,reason:"Registry reset".into()}))).await;break;}
                    if !send(&mut ws,frame).await{break;}
                },
                Ok(_)=>{},Err(_)=>break,
            },
            message=ws.recv()=>{
                let text=match message{
                    Some(Ok(Message::Text(t)))if t=="ping"=>{if ws.send(Message::Text("pong".into())).await.is_err(){break;}continue;},
                    Some(Ok(Message::Text(t)))=>t,
                    Some(Ok(Message::Ping(b)))=>{if ws.send(Message::Pong(b)).await.is_err(){break;}continue;},
                    Some(Ok(Message::Pong(_)))=>continue,
                    _=>break,
                };
                let Ok(frame)=serde_json::from_str::<Value>(&text) else{break;};
                let s=state.clone();let owner=account.clone();
                let result:Result<Vec<Value>>=match frame["t"].as_str(){
                    Some("hello")=>{
                        if let Some(d)=frame["device"].as_str().filter(|d|!d.is_empty()&&d.len()<=128){device=d.into();}
                        ready=true;let cursor=frame["cursor"].as_i64().filter(|c|*c>=0);
                        run(&state,&account,async move{Ok(vec![snapshot(&s,&owner,cursor).await?])}).await
                    },
                    Some("push") if ready=>{
                        let d=device.clone();
                        run(&state,&account,async move{
                            apply(&s,&owner,&d,frame,sender).await?;
                            Ok(vec![])
                        }).await
                    },
                    Some("presence") if ready=>{beat(&state,&account,&device,sender,frame["at"].as_i64().unwrap_or_else(now)).await;Ok(vec![])},
                    Some("probe")=>run(&state,&account,async move{let seq=head(&s,&owner).await?;Ok(vec![json!({"t":"probe-ok","seq":seq})])}).await,
                    Some("push")=>Err(bad("bad_push")),
                    _=>Err(bad("bad_frame")),
                };
                let frames=match result{Ok(frames)=>frames,Err(e)=>vec![json!({"t":"error","code":e.1,"message":"The update could not be saved."})]};
                for frame in frames{if !send(&mut ws,frame).await{return;}}
            }
        }
    }
}
