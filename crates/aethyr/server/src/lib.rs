//! Bounded authoritative room transport for the owned MMORPG simulation.
use af_mmorpg_core::{
    Class, Controls, DT, PROTOCOL, SOURCE_COMMIT, Surface, V3, World, WorldConfig,
};
use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::SinkExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

pub struct ServerSurface {
    pub height_fn: fn(f32, f32) -> f32,
    pub collision: af_kernel::CollisionScene,
    pub procedural_trees: bool,
}
impl Surface for ServerSurface {
    fn height(&self, x: f32, z: f32) -> f32 {
        (self.height_fn)(x, z)
    }
    fn support(&self, p: V3, max_y: f32) -> f32 {
        self.height(p[0], p[2])
            .max(self.collision.support(p, max_y))
    }
    fn step_support(&self, p: V3, max_y: f32) -> f32 {
        self.support(p, max_y)
            .max(self.collision.step_support(p, max_y))
    }
    fn can_occupy(&self, p: V3, height: f32) -> bool {
        p[1] >= self.height(p[0], p[2]) - 0.001
            && self.collision.can_occupy(p, height)
            && (!self.procedural_trees || !af_mmorpg_core::scenery::tree_blocks(p[0], p[2]))
    }
    fn slide_down(&self, p: V3, height: f32) -> Option<V3> {
        self.collision.slide_down(p, height)
    }
    fn clear(&self, a: V3, b: V3) -> bool {
        self.collision.clear(a, b)
            && af_mmorpg_core::height_line_clear(a, b, |x, z| self.height(x, z))
    }
}
#[derive(Clone)]
pub struct AppState {
    pub world: Arc<Mutex<World>>,
    pub surface: Arc<ServerSurface>,
    origins: Arc<Vec<String>>,
    connections: Arc<Semaphore>,
    pub shutting_down: Arc<AtomicBool>,
}
impl AppState {
    pub fn new(
        config: WorldConfig,
        surface: ServerSurface,
        origins: Vec<String>,
    ) -> Result<Self, &'static str> {
        if origins.is_empty()
            || origins.len() > 16
            || origins.iter().any(|s| s.len() > 256 || s.contains('*'))
        {
            return Err("Choose explicit browser origins.");
        }
        let world = World::new(config, &surface)?;
        Ok(Self {
            world: Arc::new(Mutex::new(world)),
            surface: Arc::new(surface),
            origins: Arc::new(origins),
            connections: Arc::new(Semaphore::new(64)),
            shutting_down: Arc::new(AtomicBool::new(false)),
        })
    }
}
pub fn default_origins() -> Vec<String> {
    [
        "http://127.0.0.1:55316",
        "http://127.0.0.1:55317",
        "http://localhost:55316",
        "http://localhost:55317",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/worlds/open-ground/ws", get(upgrade))
        .with_state(state)
}
async fn health(State(state): State<AppState>) -> Json<Value> {
    let world = state.world.lock().await;
    Json(
        json!({"ok":true,"protocol":PROTOCOL,"tick":world.tick,"tickRate":60,"population":world.population()}),
    )
}
async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    if !origin.is_some_and(|o| state.origins.iter().any(|allowed| allowed == o)) {
        return (
            StatusCode::FORBIDDEN,
            "Open this world from an approved page.",
        )
            .into_response();
    }
    let Ok(permit) = state.connections.clone().try_acquire_owned() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "This world is busy. Try again shortly.",
        )
            .into_response();
    };
    ws.max_message_size(8192)
        .max_frame_size(8192)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            connection(socket, state).await
        })
}
pub async fn run_world(state: AppState, mut stop: tokio::sync::watch::Receiver<bool>) {
    let mut timer = tokio::time::interval(Duration::from_secs_f64(DT as f64));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _=timer.tick()=>{state.world.lock().await.step(state.surface.as_ref());}
            result=stop.changed()=>{if result.is_err() || *stop.borrow(){break;}}
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ClientMessage {
    Join {
        name: String,
        #[serde(default)]
        class: Class,
    },
    Input {
        sequence: u32,
        #[serde(default)]
        forward: f32,
        #[serde(default)]
        turn: f32,
        #[serde(default)]
        sprint: bool,
        #[serde(default)]
        attack: bool,
        #[serde(default)]
        dance: bool,
    },
    Swap {
        #[serde(rename = "requestId")]
        request_id: String,
        version: u64,
        from: u8,
        to: u8,
    },
    Chat {
        #[serde(rename = "requestId")]
        request_id: String,
        text: String,
    },
    Respawn {
        #[serde(rename = "requestId")]
        request_id: String,
    },
}
async fn send(socket: &mut WebSocket, value: &impl serde::Serialize) -> bool {
    let Ok(text) = serde_json::to_string(value) else {
        return false;
    };
    if text.len() > 1024 * 1024 {
        return false;
    }
    matches!(
        tokio::time::timeout(
            Duration::from_secs(2),
            socket.send(Message::Text(text.into()))
        )
        .await,
        Ok(Ok(()))
    )
}
async fn connection(mut socket: WebSocket, state: AppState) {
    let mut player = None;
    let mut known = BTreeSet::new();
    let mut after = 0;
    let mut timer = tokio::time::interval(Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let opened = Instant::now();
    let mut window = Instant::now();
    let mut messages = 0;
    loop {
        tokio::select! {
            message=socket.recv()=>{
                let Some(Ok(message))=message else{break;};
                let text=match message {
                    Message::Text(text)=>text,
                    Message::Close(_)=>break,
                    Message::Ping(bytes)=>{if !matches!(tokio::time::timeout(Duration::from_secs(2),socket.send(Message::Pong(bytes))).await,Ok(Ok(()))){break;}continue;},
                    Message::Pong(_)=>continue,
                    Message::Binary(_)=>{let _=send(&mut socket,&json!({"type":"error","message":"Send a supported world action."})).await;break;},
                };
                if window.elapsed()>=Duration::from_secs(1){window=Instant::now();messages=0;}
                messages+=1;if messages>240{let _=send(&mut socket,&json!({"type":"error","message":"Too many world actions. Reopen the world."})).await;break;}
                let Ok(message)=serde_json::from_str::<ClientMessage>(&text)else{if !send(&mut socket,&json!({"type":"error","message":"Choose a supported world action."})).await{break;}continue;};
                let response={
                    let mut world=state.world.lock().await;
                    match message {
                        ClientMessage::Join{name,class} if player.is_none()=>match world.join(&name,class,state.surface.as_ref()){
                            Ok(id)=>{player=Some(id);let snapshot=world.snapshot(id,&mut known,0).unwrap();after=snapshot.last_event_id;
                                json!({"type":"welcome","protocol":PROTOCOL,"playerId":id,"world":{"seed":world.config.seed,"worldScale":world.config.world_scale,"sourceCommit":SOURCE_COMMIT,"tickRate":60,"snapshotRate":10,"release":world.config.release,"spawn":world.config.spawn,"yaw":world.config.yaw},"snapshot":snapshot})},
                            Err(message)=>json!({"type":"error","message":message}),
                        },
                        ClientMessage::Join{..}=>json!({"type":"error","message":"You have already joined this world."}),
                        other=>if let Some(id)=player {match other {
                            ClientMessage::Input{sequence,forward,turn,sprint,attack,dance}=>{
                                match world.input(id,sequence,Controls{forward,turn,sprint,attack,dance}){Ok(())=>Value::Null,Err(message)=>json!({"type":"error","message":message})}
                            },
                            ClientMessage::Swap{request_id,version,from,to}=>serde_json::to_value(world.swap(id,&request_id,version,from,to)).unwrap(),
                            ClientMessage::Chat{request_id,text}=>serde_json::to_value(world.chat(id,&request_id,&text)).unwrap(),
                            ClientMessage::Respawn{request_id}=>serde_json::to_value(world.respawn(id,&request_id,state.surface.as_ref())).unwrap(),
                            ClientMessage::Join{..}=>unreachable!(),
                        }}else{json!({"type":"error","message":"Enter your name before playing."})},
                    }
                };
                if !response.is_null()&&!send(&mut socket,&response).await{break;}
            }
            _=timer.tick()=>{
                if state.shutting_down.load(Ordering::Relaxed) {break;}
                if let Some(id)=player{
                    let snapshot=state.world.lock().await.snapshot(id,&mut known,after);
                    let Some(snapshot)=snapshot else{break;};after=snapshot.last_event_id;
                    if !send(&mut socket,&snapshot).await{break;}
                }else if opened.elapsed()>Duration::from_secs(10){break;}
            }
        }
    }
    if let Some(id) = player {
        state.world.lock().await.leave(id);
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message as ClientFrame, client::IntoClientRequest},
    };
    type Client = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;
    async fn fixture() -> (AppState, String, tokio::task::JoinHandle<()>) {
        let state = AppState::new(
            WorldConfig {
                spawn: [0.; 3],
                spawners: Some(vec![]),
                ..Default::default()
            },
            ServerSurface {
                height_fn: |_, _| 0.,
                collision: af_kernel::CollisionScene::default(),
                procedural_trees: false,
            },
            default_origins(),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "ws://{}/v1/worlds/open-ground/ws",
            listener.local_addr().unwrap()
        );
        let app = router(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (state, url, task)
    }
    async fn connect(url: &str, origin: &str) -> Client {
        let mut req = url.into_client_request().unwrap();
        req.headers_mut().insert("Origin", origin.parse().unwrap());
        connect_async(req).await.unwrap().0
    }
    async fn send_client(client: &mut Client, value: Value) {
        client
            .send(ClientFrame::Text(value.to_string().into()))
            .await
            .unwrap();
    }
    async fn until(client: &mut Client, predicate: impl Fn(&Value) -> bool) -> Value {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let message = client.next().await.unwrap().unwrap();
                if let ClientFrame::Text(text) = message {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    if predicate(&value) {
                        return value;
                    }
                }
            }
        })
        .await
        .expect("bounded server response")
    }
    #[tokio::test]
    async fn two_real_websockets_share_chat_equipment_input_and_disconnect_cleanup() {
        let (state, url, server) = fixture().await;
        let mut a = connect(&url, "http://127.0.0.1:55316").await;
        let mut b = connect(&url, "http://127.0.0.1:55317").await;
        send_client(&mut a, json!({"type":"join","name":"A"})).await;
        let wa = until(&mut a, |v| v["type"] == "welcome").await;
        send_client(&mut b, json!({"type":"join","name":"B","class":"sorceror"})).await;
        let wb = until(&mut b, |v| v["type"] == "welcome").await;
        assert_ne!(wa["playerId"], wb["playerId"]);
        assert_eq!(wb["snapshot"]["player"]["health"], 100.);
        assert_eq!(state.world.lock().await.population().players, 2);
        send_client(
            &mut a,
            json!({"type":"chat","requestId":"chat-1","text":"Hello B"}),
        )
        .await;
        assert_eq!(
            until(&mut a, |v| v["requestId"] == "chat-1").await["ok"],
            true
        );
        let shared = until(&mut b, |v| {
            v["events"]
                .as_array()
                .is_some_and(|events| events.iter().any(|e| e["text"] == "Hello B"))
        })
        .await;
        assert!(
            shared["actors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["name"] == "A")
        );
        let swap = json!({"type":"swap","requestId":"swap-1","version":0,"from":24,"to":2});
        send_client(&mut a, swap.clone()).await;
        assert_eq!(
            until(&mut a, |v| v["requestId"] == "swap-1").await["inventoryVersion"],
            1
        );
        send_client(&mut a, swap).await;
        assert_eq!(
            until(&mut a, |v| v["requestId"] == "swap-1").await["duplicate"],
            true
        );
        send_client(&mut a,json!({"type":"input","sequence":1,"forward":1,"turn":0,"sprint":false,"attack":false,"dance":false})).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        state.world.lock().await.step(state.surface.as_ref());
        let snap = until(&mut a, |v| v["ackInput"] == 1).await;
        assert!(snap["player"]["position"][2].as_f64().unwrap() < 0.);
        send_client(
            &mut a,
            json!({"type":"input","sequence":2,"health":9999,"position":[1000,0,0]}),
        )
        .await;
        assert_eq!(
            until(&mut a, |v| v["type"] == "error").await["type"],
            "error"
        );
        assert_eq!(
            until(&mut a, |v| v["type"] == "snapshot").await["player"]["health"],
            200.
        );
        a.close(None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while state.world.lock().await.population().players != 1 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let gone = until(&mut b, |v| {
            v["despawned"]
                .as_array()
                .is_some_and(|ids| ids.contains(&wa["playerId"]))
        })
        .await;
        assert_eq!(gone["population"]["players"], 1);
        b.close(None).await.unwrap();
        server.abort();
    }
    #[tokio::test]
    async fn foreign_and_missing_origins_cannot_upgrade() {
        let (_, url, server) = fixture().await;
        let mut request = url.clone().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("Origin", "https://unrelated.example".parse().unwrap());
        assert!(connect_async(request).await.is_err());
        assert!(connect_async(&url).await.is_err());
        server.abort();
    }
    #[test]
    fn client_messages_cannot_claim_another_identity_or_replace_stats_inventory_or_pose() {
        for message in [
            json!({"type":"input","sequence":1,"playerId":42}),
            json!({"type":"world.update","transform":["idle",[0,0,0],[0,0,0,1]]}),
            json!({"type":"world.inventory","slots":["weapon.sword1"]}),
            json!({"type":"join","name":"A","health":999}),
        ] {
            assert!(serde_json::from_value::<ClientMessage>(message).is_err());
        }
    }
}
