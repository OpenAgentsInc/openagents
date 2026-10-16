//! Opaque chat updates. PostgreSQL commits rows and their sequence together;
//! immutable bucket objects become visible only through committed metadata.
use super::{Identity, StateData, authenticate, rooms::Room};
use axum::{
    Json,
    body::Bytes,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use tokio_postgres::Transaction;
use uuid::Uuid;
use zeron_sync::chat_frames::{self as wire, frame_type as kind};

const ROW_LIMIT: usize = 1024 * 1024;
const CHECKPOINT_LIMIT: usize = 32 * 1024 * 1024;
type Result<T> = std::result::Result<T, Failure>;
#[derive(Debug)]
pub struct Failure(StatusCode, &'static str);
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<tokio_postgres::Error> for Failure {
    fn from(_: tokio_postgres::Error) -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    }
}
fn bad(code: &'static str) -> Failure {
    Failure(StatusCode::BAD_REQUEST, code)
}
fn identity(
    state: &StateData,
    headers: &HeaderMap,
    query: &HashMap<String, String>,
    id: &str,
) -> Result<Identity> {
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.split_once(' '))
        .filter(|(s, _)| s.eq_ignore_ascii_case("bearer"))
        .map(|(_, t)| t.trim());
    let who = header
        .or_else(|| query.get("token").map(String::as_str))
        .and_then(|t| authenticate(state.auth, t))
        .ok_or(Failure(StatusCode::UNAUTHORIZED, "unauthenticated"))?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(Failure(StatusCode::NOT_FOUND, "not_found"));
    }
    Ok(who)
}
async fn claim(tx: &Transaction<'_>, id: &str, account: &str) -> Result<i64> {
    tx.execute(
        "INSERT INTO chats(chat_id,account) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        &[&id, &account],
    )
    .await?;
    let row = tx
        .query_one(
            "SELECT account,head FROM chats WHERE chat_id=$1 FOR UPDATE",
            &[&id],
        )
        .await?;
    if row.get::<_, String>(0) != account {
        return Err(Failure(StatusCode::FORBIDDEN, "forbidden"));
    }
    Ok(row.get(1))
}

pub fn decode(bytes: &[u8]) -> Option<wire::WireFrame> {
    let frame = wire::decode(bytes)?;
    (kind::HELLO..=kind::ERROR)
        .contains(&frame.kind)
        .then_some(frame)
}
fn frame(kind: u8, header: Value, payload: &[u8]) -> Vec<u8> {
    wire::encode(kind, &header, payload)
}
fn error(code: &str, batch: &str) -> Vec<u8> {
    frame(kind::ERROR, json!({"code":code,"batchId":batch}), &[])
}

struct Snapshot {
    head: i64,
    floor: i64,
    object: Option<String>,
    size: i64,
    frontier: Vec<u8>,
    rows: Vec<Vec<u8>>,
    count: i64,
    bytes: i64,
    more: bool,
}
impl Snapshot {
    fn state(&self) -> Vec<u8> {
        frame(
            kind::STATE,
            json!({"headSeq":self.head,"seqFloor":self.floor,"checkpointSeq":self.floor,"checkpointSize":self.size,"rowCount":self.count,"rowBytes":self.bytes}),
            &self.frontier,
        )
    }
}
async fn snapshot(
    state: &StateData,
    id: &str,
    account: &str,
    after: i64,
    exclude: &str,
) -> Result<Snapshot> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let head = claim(&tx, id, account).await?;
    let cp = tx
        .query_opt(
            "SELECT seq,object,size,frontier FROM chat_checkpoints WHERE chat_id=$1",
            &[&id],
        )
        .await?;
    let (floor, object, size, frontier) = match cp {
        Some(r) => (
            r.get(0),
            Some(r.get(1)),
            r.get(2),
            STANDARD
                .decode(r.get::<_, String>(3))
                .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?,
        ),
        None => (0, None, 0, vec![]),
    };
    let stats=tx.query_one("SELECT count(*),coalesce(sum(octet_length(bytes)),0)::bigint FROM chat_rows WHERE chat_id=$1",&[&id]).await?;
    // Bound both row count and bytes; probe for another page without loading it.
    let records=tx.query("WITH page AS (SELECT seq,device,batch_id,bytes,sum(octet_length(bytes)+512) OVER(ORDER BY seq) AS budget FROM chat_rows WHERE chat_id=$1 AND seq>$2 AND ($3='' OR device<>$3)) SELECT seq,device,batch_id,bytes FROM page WHERE budget<=4194304 ORDER BY seq LIMIT 256",&[&id,&after,&exclude]).await?;
    let last = records.last().map(|r| r.get::<_, i64>(0)).unwrap_or(after);
    let more=tx.query_one("SELECT EXISTS(SELECT 1 FROM chat_rows WHERE chat_id=$1 AND seq>$2 AND ($3='' OR device<>$3))",&[&id,&last,&exclude]).await?.get::<_,bool>(0);
    let rows=records.into_iter().map(|r|frame(kind::ROW,json!({"seq":r.get::<_,i64>(0),"device":r.get::<_,String>(1),"batchId":r.get::<_,String>(2)}),&r.get::<_,Vec<u8>>(3))).collect();
    tx.commit().await?;
    Ok(Snapshot {
        head,
        floor,
        object,
        size,
        frontier,
        rows,
        count: stats.get(0),
        bytes: stats.get(1),
        more,
    })
}
async fn append(
    state: &StateData,
    id: &str,
    account: &str,
    device: &str,
    batch: &str,
    bytes: &[u8],
    sender: Uuid,
) -> Result<Value> {
    if batch.is_empty() || batch.len() > 128 {
        return Err(bad("bad_push"));
    }
    if bytes.is_empty() {
        return Err(bad("empty"));
    }
    if bytes.len() > ROW_LIMIT {
        return Err(Failure(StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    }
    if !state
        .rooms
        .admit_push(Room::Chat(id.into()), device, bytes.len())
        .await
    {
        return Err(Failure(StatusCode::TOO_MANY_REQUESTS, "quota"));
    }
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let head = claim(&tx, id, account).await?;
    if let Some(row) = tx
        .query_opt(
            "SELECT seq FROM chat_batches WHERE chat_id=$1 AND batch_id=$2",
            &[&id, &batch],
        )
        .await?
    {
        let seq: i64 = row.get(0);
        tx.commit().await?;
        return Ok(json!({"batchId":batch,"seq":seq,"dup":true}));
    }
    let seq = head.checked_add(1).ok_or(Failure(
        StatusCode::INSUFFICIENT_STORAGE,
        "sequence_exhausted",
    ))?;
    tx.execute(
        "INSERT INTO chat_rows(chat_id,seq,batch_id,bytes,device) VALUES($1,$2,$3,$4,$5)",
        &[&id, &seq, &batch, &bytes, &device],
    )
    .await?;
    tx.execute(
        "INSERT INTO chat_batches(chat_id,batch_id,seq) VALUES($1,$2,$3)",
        &[&id, &batch, &seq],
    )
    .await?;
    tx.execute("UPDATE chats SET head=$2 WHERE chat_id=$1", &[&id, &seq])
        .await?;
    tx.commit().await?;
    let events = state.rooms.events(Room::Chat(id.into())).await;
    let _ = events.send((
        sender,
        frame(
            kind::ROW,
            json!({"seq":seq,"device":device,"batchId":batch}),
            bytes,
        ),
    ));
    Ok(json!({"batchId":batch,"seq":seq,"dup":false}))
}
async fn in_room<T: Send + 'static, F: std::future::Future<Output = Result<T>> + Send + 'static>(
    state: &StateData,
    id: &str,
    job: F,
) -> Result<T> {
    state
        .rooms
        .run(Room::Chat(id.into()), job)
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"))?
}
fn after(q: &HashMap<String, String>) -> i64 {
    q.get("after")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(0)
}

pub async fn rows(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &id)?;
    let device = if q.get("excludeOwn").is_some_and(|v| v == "1") {
        q.get("device").cloned().unwrap_or_default()
    } else {
        String::new()
    };
    let s = state.clone();
    let chat = id.clone();
    let cursor = after(&q);
    let snap = in_room(&state, &id, async move {
        snapshot(&s, &chat, &who.account, cursor, &device).await
    })
    .await?;
    let mut output = Vec::new();
    fn pack(out: &mut Vec<u8>, f: Vec<u8>) {
        out.extend_from_slice(&(f.len() as u32).to_le_bytes());
        out.extend(f);
    }
    pack(&mut output, snap.state());
    let mut truncated = false;
    let more = snap.more;
    for row in snap.rows {
        if output.len() + row.len() + 4 > 4 * 1024 * 1024 {
            truncated = true;
            break;
        }
        pack(&mut output, row);
    }
    if !truncated && !more {
        pack(
            &mut output,
            frame(kind::ROWS_DONE, json!({"headSeq":snap.head}), &[]),
        );
    }
    Ok(([("content-type", "application/octet-stream")], output).into_response())
}
pub async fn push(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    let s = state.clone();
    let chat = id.clone();
    in_room(&state, &id, async move {
        append(
            &s,
            &chat,
            &who.account,
            q.get("device").map(String::as_str).unwrap_or(""),
            q.get("batchId").map(String::as_str).unwrap_or(""),
            &body,
            Uuid::nil(),
        )
        .await
    })
    .await
    .map(Json)
}
pub async fn checkpoint(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &id)?;
    let s = state.clone();
    let chat = id.clone();
    let snap = in_room(&state, &id, async move {
        snapshot(&s, &chat, &who.account, i64::MAX, "").await
    })
    .await?;
    let key = snap
        .object
        .ok_or(Failure(StatusCode::NOT_FOUND, "not_found"))?;
    let object = state
        .bucket
        .get(&key)
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?
        .ok_or(Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
    let range = headers
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .and_then(|v| v.strip_suffix('-'))
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0);
    let total = object.bytes.len();
    if range.is_some_and(|n| n >= total) {
        return Ok((
            StatusCode::RANGE_NOT_SATISFIABLE,
            [("content-range", format!("bytes */{total}"))],
        )
            .into_response());
    }
    let start = range.unwrap_or(0);
    let mut response = (
        if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        },
        object.bytes.slice(start..),
    )
        .into_response();
    let h = response.headers_mut();
    h.insert("content-type", "application/octet-stream".parse().unwrap());
    h.insert("accept-ranges", "bytes".parse().unwrap());
    h.insert(
        "x-chat2-checkpoint-seq",
        snap.floor.to_string().parse().unwrap(),
    );
    if range.is_some() {
        h.insert(
            "content-range",
            format!("bytes {start}-{}/{total}", total - 1)
                .parse()
                .unwrap(),
        );
    }
    Ok(response)
}
pub async fn publish_checkpoint(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let who = identity(&state, &headers, &q, &id)?;
    let covered = q
        .get("seqCovered")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .ok_or(bad("bad_seq_covered"))?;
    let frontier = headers
        .get("x-chat2-frontier")
        .map(|h| h.to_str().map_err(|_| bad("bad_frontier")))
        .transpose()?
        .unwrap_or("");
    let frontier = STANDARD.decode(frontier).map_err(|_| bad("bad_frontier"))?;
    if covered > 0 && frontier.is_empty() {
        return Err(bad("bad_frontier"));
    }
    if body.is_empty() {
        return Err(bad("empty"));
    }
    if body.len() > CHECKPOINT_LIMIT {
        return Err(Failure(StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    }
    let s = state.clone();
    let chat = id.clone();
    in_room(&state,&id,async move {
        let mut db=s.database.connect().await?;let tx=db.transaction().await?;
        let head=claim(&tx,&chat,&who.account).await?;
        let prior=tx.query_opt("SELECT seq FROM chat_checkpoints WHERE chat_id=$1",&[&chat]).await?.map(|r|r.get::<_,i64>(0)).unwrap_or(0);
        if covered<prior {return Err(Failure(StatusCode::CONFLICT,"floor_regression"));}
        if covered>head {return Err(Failure(StatusCode::CONFLICT,"ahead_of_head"));}
        let key=format!("chat2/{chat}/checkpoints/{}",Uuid::new_v4());
        // Upload first. A crash leaves an unreferenced object, never metadata
        // pointing to a partial or missing checkpoint. Old objects stay valid
        // for downloads that raced this commit.
        s.bucket.put(&key,body.clone(),"application/octet-stream").await.map_err(|_|Failure(StatusCode::SERVICE_UNAVAILABLE,"unavailable"))?;
        let size=body.len() as i64;let encoded=STANDARD.encode(frontier);
        tx.execute("INSERT INTO chat_checkpoints(chat_id,seq,object,size,frontier) VALUES($1,$2,$3,$4,$5) ON CONFLICT(chat_id) DO UPDATE SET seq=EXCLUDED.seq,object=EXCLUDED.object,size=EXCLUDED.size,frontier=EXCLUDED.frontier",&[&chat,&covered,&key,&size,&encoded]).await?;
        let pruned=tx.execute("DELETE FROM chat_rows WHERE chat_id=$1 AND seq<=$2",&[&chat,&covered]).await?;
        tx.commit().await?;
        Ok(Json(json!({"ok":true,"seqFloor":covered,"pruned":pruned})))
    }).await
}

pub async fn websocket(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    upgrade: std::result::Result<
        WebSocketUpgrade,
        axum::extract::ws::rejection::WebSocketUpgradeRejection,
    >,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &id)?;
    let upgrade =
        upgrade.map_err(|_| Failure(StatusCode::UPGRADE_REQUIRED, "expected_websocket"))?;
    let s = state.clone();
    let chat = id.clone();
    let account = who.account.clone();
    in_room(&state, &id, async move {
        snapshot(&s, &chat, &account, i64::MAX, "").await
    })
    .await?;
    let events = state.rooms.events(Room::Chat(id.clone())).await.subscribe();
    Ok(upgrade
        .max_message_size(ROW_LIMIT + 8192)
        .on_upgrade(move |socket| {
            session(
                socket,
                state,
                id,
                who.account,
                q.get("device").cloned().unwrap_or_default(),
                events,
            )
        }))
}
async fn send(socket: &mut WebSocket, bytes: Vec<u8>) -> bool {
    socket.send(Message::Binary(bytes.into())).await.is_ok()
}
async fn session(
    mut socket: WebSocket,
    state: Arc<StateData>,
    id: String,
    account: String,
    mut device: String,
    mut events: tokio::sync::broadcast::Receiver<(Uuid, Vec<u8>)>,
) {
    let sender = Uuid::new_v4();
    let mut ready = false;
    loop {
        tokio::select! {
            event=events.recv()=>match event {
                Ok((source,bytes)) if ready && source!=sender=>{if !send(&mut socket,bytes).await{break;}},
                Ok(_)=>{},
                // Disconnect on overflow; the client's persisted cursor repairs
                // the gap on reconnect instead of silently dropping updates.
                Err(_)=>break,
            },
            message=socket.recv()=> {
                let bytes=match message {
                    Some(Ok(Message::Binary(b)))=>b,
                    Some(Ok(Message::Text(t))) if t=="ping"=>{if socket.send(Message::Text("pong".into())).await.is_err(){break;}continue;},
                    Some(Ok(Message::Ping(b)))=>{if socket.send(Message::Pong(b)).await.is_err(){break;}continue;},
                    Some(Ok(Message::Pong(_)))=>continue,
                    _=>break,
                };
                let Some(f)=decode(&bytes) else {if !send(&mut socket,error("bad_frame","")).await{break;}continue;};
                if f.kind == kind::ROWS_REQ && ready {
                    let mut cursor=f.header["after"].as_i64().filter(|v|*v>=0).unwrap_or(0);
                    let exclude=if f.header["excludeOwn"]==true{device.clone()}else{String::new()};
                    loop {
                        let s=state.clone();let chat=id.clone();let owner=account.clone();let e=exclude.clone();
                        let page=in_room(&state,&id,async move{snapshot(&s,&chat,&owner,cursor,&e).await}).await;
                        let Ok(page)=page else{return;};
                        let more=page.more;
                        for row in page.rows {
                            cursor=decode(&row).and_then(|f|f.header["seq"].as_i64()).unwrap_or(cursor);
                            if !send(&mut socket,row).await{return;}
                        }
                        if !more {
                            if !send(&mut socket,frame(kind::ROWS_DONE,json!({"headSeq":page.head}),&[])).await{return;}
                            break;
                        }
                    }
                    continue;
                }
                let s=state.clone();let chat=id.clone();let owner=account.clone();
                let responses:Result<Vec<Vec<u8>>>=match f.kind {
                    kind::HELLO=>{
                        if let Some(d)=f.header["device"].as_str().filter(|v|!v.is_empty()&&v.len()<=128){device=d.into();}
                        ready=true;
                        in_room(&state,&id,async move{Ok(vec![snapshot(&s,&chat,&owner,i64::MAX,"").await?.state()])}).await
                    },
                    kind::PUSH if ready=>{
                        let batch=f.header["batchId"].as_str().unwrap_or("").to_owned();let d=device.clone();
                        let result=in_room(&state,&id,async move{append(&s,&chat,&owner,&d,&batch,&f.payload,sender).await}).await;
                        match result {Ok(ack)=>Ok(vec![frame(kind::ACK,ack,&[])]),Err(e)=>Ok(vec![error(e.1,f.header["batchId"].as_str().unwrap_or(""))])}
                    },
                    kind::PRESENCE if ready=>{
                        let _=state.rooms.events(Room::Chat(id.clone())).await.send((sender,frame(kind::PRESENCE,json!({"device":device,"at":f.header["at"]}),&f.payload)));
                        Ok(vec![])
                    },
                    kind::PROBE=>in_room(&state,&id,async move{Ok(vec![frame(kind::PROBE_OK,json!({"headSeq":snapshot(&s,&chat,&owner,i64::MAX,"").await?.head}),&[])])}).await,
                    kind::ROWS_REQ=>Ok(vec![error("hello_first","")]),
                    kind::PUSH=>Ok(vec![error("bad_push",f.header["batchId"].as_str().unwrap_or(""))]),
                    _=>Ok(vec![error("bad_frame","")]),
                };
                match responses {Ok(frames)=>{for f in frames{if !send(&mut socket,f).await{return;}}},Err(_)=>break}
            }
        }
    }
}
