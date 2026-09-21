//! mc-bridge: the Minecraft bot helper.
//!
//! One bot, line-delimited JSON on stdin and stdout. `crates/voyager` spawns
//! this binary as a supervised child process and never links azalea — the
//! bot framework needs nightly Rust (`simdnbt` uses `portable_simd`), which
//! the workspace's pinned stable toolchain cannot provide. This crate is the
//! Rust analogue of `swift/lev-bridge`: a helper built by
//! `scripts/build-mc-bridge.sh` and supervised by the caller.
//!
//! Requests, one object per line:
//!
//! ```jsonc
//! {"id": 1, "op": "join", "args": {"address": "127.0.0.1:25565", "username": "voyager"}}
//! {"id": 2, "op": "state", "args": {"radius": 16}}
//! {"id": 3, "op": "say", "args": {"text": "hello"}}
//! {"id": 4, "op": "goto", "args": {"x": 12, "z": -40, "seconds": 60}}
//! {"id": 5, "op": "goto", "args": {"x": 12, "y": 65, "z": -40, "radius": 3}}
//! {"id": 6, "op": "explore", "args": {"direction": "north", "distance": 96, "seconds": 30}}
//! {"id": 7, "op": "mine", "args": {"names": ["oak_log", "birch_log"], "count": 2,
//!  "radius": 32, "seconds": 120}}
//! {"id": 8, "op": "wait", "args": {"seconds": 2}}
//! {"id": 9, "op": "disconnect"}
//! {"id": 10, "op": "shutdown"}
//! ```
//!
//! Answers, one object per line:
//!
//! ```jsonc
//! {"id": 1, "ok": true, "result": {"username": "voyager"}}
//! {"id": 7, "ok": false, "code": "no_blocks", "error": "..."}
//! {"event": "chat", "text": "<voyager> hello"}
//! {"event": "feedback", "text": "mining oak_log at (12, 64, -40)"}
//! ```

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::str::FromStr;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use azalea::auto_tool::AutoToolClientExt;
use azalea::ecs::entity::Entity;
use azalea::ecs::prelude::Component;
use azalea::ecs::query::{With, Without};
use azalea::pathfinder::goals::{RadiusGoal, XZGoal};
use azalea::prelude::*;
use azalea::{BlockPos, Vec3};
use azalea_block::BlockStates;
use azalea_client::mining::StopMiningBlockEvent;
use azalea_entity::inventory::Inventory;
use azalea_entity::{EntityKindComponent, LocalEntity, Position};
use azalea_inventory::{ItemStack, Menu};
use azalea_registry::builtin::BlockKind;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

/// How long `join` waits for the first `Spawn` event.
const JOIN_WAIT: Duration = Duration::from_secs(45);
/// Default per-op bound when a request names none.
const DEFAULT_OP_WAIT: Duration = Duration::from_secs(60);
/// Default radius for a `state` scan, in blocks.
const DEFAULT_RADIUS: i32 = 16;
/// The most block names one `state` answer reports.
const BLOCKS_MAX: usize = 64;
/// The most entity names one `state` answer reports.
const ENTITIES_MAX: usize = 32;
/// How close `mine` tries to stand to the block it digs.
const MINE_REACH: f32 = 3.8;
/// A single movement or digging stretch inside `mine` never exceeds this.
const MINE_STEP_WAIT: Duration = Duration::from_secs(30);
/// `mine` ignores candidates this far above the bot — a log in a canopy is
/// nearer by manhattan distance than one at ground level, but the
/// pathfinder cannot climb a trunk to reach it.
const MINE_MAX_ABOVE: i32 = 6;
/// How long a single approach walk inside `mine` gets before the candidate
/// is called unreachable.
const MINE_APPROACH_WAIT: Duration = Duration::from_secs(15);

/// One request line.
#[derive(Debug, serde::Deserialize)]
struct Request {
    id: u64,
    op: String,
    #[serde(default)]
    args: Value,
}

/// The bot's own handle, handed to the request loop on first spawn.
type Handout = StdMutex<Option<oneshot::Sender<Client>>>;

/// State shared with the azalea handler. `set_handler` requires `Default`;
/// the fallback's channel sends nowhere, which only the swarm path could
/// touch — the bot here always runs with the state `join` installs.
#[derive(Component, Clone)]
struct BridgeState {
    handout: std::sync::Arc<Handout>,
    events: mpsc::UnboundedSender<Value>,
}

impl Default for BridgeState {
    fn default() -> Self {
        let (events, _dropped) = mpsc::unbounded_channel();
        Self {
            handout: std::sync::Arc::new(StdMutex::new(None)),
            events,
        }
    }
}

fn main() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            emit(&json!({"event": "fatal", "error": format!("tokio: {error}")}));
            std::process::exit(2);
        }
    };
    let code = runtime.block_on(run());
    std::process::exit(code);
}

async fn run() -> i32 {
    // stdin is blocking; read it on a thread and forward lines as requests.
    let (lines_tx, mut lines_rx) = mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(std::io::stdin()).lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    if lines_tx.send(line).is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    });

    // Every event the bot side emits funnels through one printer so lines
    // never interleave.
    let (events_tx, mut events_rx) = mpsc::unbounded_channel::<Value>();
    let printer = tokio::spawn(async move {
        while let Some(event) = events_rx.recv().await {
            emit(&event);
        }
    });

    let mut bot: Option<Client> = None;
    while let Some(line) = lines_rx.recv().await {
        let request = match serde_json::from_str::<Request>(&line) {
            Ok(request) => request,
            Err(error) => {
                emit(&json!({"id": 0, "ok": false, "code": "bad_request",
                             "error": format!("not a request: {error}")}));
                continue;
            }
        };
        match dispatch(&mut bot, &events_tx, request).await {
            Some(response) => emit(&response),
            None => break,
        }
    }
    if let Some(bot) = bot.take() {
        bot.disconnect();
    }
    printer.abort();
    0
}

/// One request. Returns the response to print, or `None` for `shutdown`,
/// which prints its own response and ends the process.
async fn dispatch(
    bot: &mut Option<Client>,
    events: &mpsc::UnboundedSender<Value>,
    request: Request,
) -> Option<Value> {
    let id = request.id;
    if request.op == "shutdown" {
        if let Some(bot) = bot.take() {
            bot.disconnect();
        }
        emit(&json!({"id": id, "ok": true, "result": {"bye": true}}));
        return None;
    }
    if request.op == "join" {
        return Some(join(bot, events, id, &request.args).await);
    }
    let Some(client) = bot.as_ref() else {
        return Some(err(id, "not_joined", "no bot is in a world; join first"));
    };
    let response = match request.op.as_str() {
        "state" => state(client, id, &request.args).await,
        "say" => say(client, id, &request.args),
        "goto" => goto(client, events, id, &request.args).await,
        "explore" => explore(client, events, id, &request.args).await,
        "mine" => mine(client, events, id, &request.args).await,
        "wait" => wait(id, &request.args).await,
        "disconnect" => {
            client.disconnect();
            *bot = None;
            ok(id, json!({"disconnected": true}))
        }
        op => err(id, "bad_request", format!("unknown op {op:?}")),
    };
    Some(response)
}

async fn join(
    bot: &mut Option<Client>,
    events: &mpsc::UnboundedSender<Value>,
    id: u64,
    args: &Value,
) -> Value {
    if bot.is_some() {
        return err(id, "already_joined", "a bot is already in a world");
    }
    let address = match args.get("address").and_then(Value::as_str) {
        Some(address) if !address.is_empty() => address.to_string(),
        _ => return err(id, "bad_request", "join needs an \"address\""),
    };
    let username = args
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("voyager")
        .to_string();
    let (handed, receive) = oneshot::channel::<Client>();
    let state = BridgeState {
        handout: std::sync::Arc::new(StdMutex::new(Some(handed))),
        events: events.clone(),
    };
    let server_events = events.clone();
    let bot_name = username.clone();
    // `ClientBuilder::start` is `!Send` — azalea drives the Bevy app on a
    // `LocalSet` inside it. The bot gets its own thread and current-thread
    // runtime; the `Client` handle it hands back is `Send`.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = server_events.send(json!({
                    "event": "fatal",
                    "error": format!("tokio: {error}"),
                }));
                return;
            }
        };
        runtime.block_on(async move {
            let exit = ClientBuilder::new()
                .set_handler(handle)
                .set_state(state)
                .start(Account::offline(&bot_name), address)
                .await;
            let _ = server_events.send(json!({
                "event": "server_exit",
                "app_exit": format!("{exit:?}"),
            }));
        });
    });
    match tokio::time::timeout(JOIN_WAIT, receive).await {
        Ok(Ok(client)) => {
            *bot = Some(client);
            ok(id, json!({"username": username}))
        }
        Ok(Err(_)) => err(id, "join_failed", "the bot task ended before spawning"),
        Err(_) => err(
            id,
            "join_timeout",
            format!("no spawn within {}s", JOIN_WAIT.as_secs()),
        ),
    }
}

/// The azalea handler: hands the `Client` to the request loop on the first
/// spawn, and forwards chat and lifecycle events to the printer.
async fn handle(bot: Client, event: Event, state: BridgeState) -> anyhow::Result<()> {
    match event {
        Event::Spawn => {
            if let Some(sender) = state
                .handout
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take()
            {
                let _ = sender.send(bot.clone());
            }
            let _ = state.events.send(json!({
                "event": "spawn",
                "position": vec3(bot.position()),
            }));
        }
        Event::Chat(packet) => {
            let _ = state.events.send(json!({
                "event": "chat",
                "text": packet.content(),
            }));
        }
        Event::Death(_) => {
            let _ = state.events.send(json!({"event": "death"}));
        }
        Event::Disconnect(reason) => {
            let _ = state.events.send(json!({
                "event": "disconnect",
                "reason": reason.map(|text| format!("{text:?}")),
            }));
        }
        _ => {}
    }
    Ok(())
}

async fn state(client: &Client, id: u64, args: &Value) -> Value {
    let radius = args
        .get("radius")
        .and_then(Value::as_i64)
        .unwrap_or(i64::from(DEFAULT_RADIUS))
        .clamp(2, 48) as i32;
    let position = client.position();
    let center = BlockPos::from(position);
    let hunger = client.hunger();

    // Inventory: aggregate the player menu's held slots by item name.
    let inventory = {
        let inventory = client.component::<Inventory>();
        let mut items = BTreeMap::<String, i64>::new();
        if let Menu::Player(player) = inventory.menu() {
            for slot in player.inventory.iter() {
                if let ItemStack::Present(stack) = slot {
                    *items.entry(stack.kind.to_string()).or_default() += i64::from(stack.count);
                }
            }
        }
        items
    };

    // Nearby blocks: a bounded cube scan for distinct non-air names.
    let nearby_blocks = {
        let world = client.world();
        let instance = world.read();
        let mut names = Vec::<String>::new();
        'scan: for dx in -radius..=radius {
            for dy in -8..=8 {
                for dz in -radius..=radius {
                    let pos = center + BlockPos::new(dx, dy, dz);
                    let Some(block) = instance.get_block_state(pos) else {
                        continue;
                    };
                    let kind = BlockKind::from(block);
                    if matches!(
                        kind,
                        BlockKind::Air | BlockKind::CaveAir | BlockKind::VoidAir
                    ) {
                        continue;
                    }
                    let name = kind.to_string();
                    if !names.contains(&name) {
                        names.push(name);
                        if names.len() >= BLOCKS_MAX {
                            break 'scan;
                        }
                    }
                }
            }
        }
        names
    };

    // Nearby entities: every positioned entity that is not the bot.
    let nearby_entities = {
        let mut ecs = client.ecs.lock();
        let mut query = ecs.query::<(Entity, &Position, &EntityKindComponent)>();
        let mut names = Vec::<String>::new();
        for (entity, entity_position, kind) in query.iter(&ecs) {
            if entity == client.entity {
                continue;
            }
            if entity_position.distance_to(position) > f64::from(radius) * 2.0 {
                continue;
            }
            names.push(kind.0.to_string());
            if names.len() >= ENTITIES_MAX {
                break;
            }
        }
        names
    };

    ok(
        id,
        json!({
            "position": vec3(position),
            "health": client.health(),
            "food": hunger.food,
            "saturation": hunger.saturation,
            "inventory": inventory,
            "nearby_blocks": nearby_blocks,
            "nearby_entities": nearby_entities,
        }),
    )
}

fn say(client: &Client, id: u64, args: &Value) -> Value {
    match args.get("text").and_then(Value::as_str) {
        Some(text) if !text.is_empty() && text.len() <= 256 => {
            client.chat(text);
            ok(id, json!({"said": text}))
        }
        _ => err(id, "bad_request", "say needs \"text\" of at most 256 chars"),
    }
}

async fn goto(
    client: &Client,
    events: &mpsc::UnboundedSender<Value>,
    id: u64,
    args: &Value,
) -> Value {
    let seconds = seconds(args, DEFAULT_OP_WAIT);
    let started = client.position();
    let result = match (args.get("x"), args.get("y"), args.get("z")) {
        (Some(x), Some(y), Some(z)) => {
            let target = Vec3::new(
                x.as_f64().unwrap_or(0.0),
                y.as_f64().unwrap_or(0.0),
                z.as_f64().unwrap_or(0.0),
            );
            let radius = args.get("radius").and_then(Value::as_f64).unwrap_or(1.0) as f32;
            let _ = events.send(feedback(format!("going to within {radius} of {target:?}")));
            timed(
                seconds,
                client.goto(RadiusGoal {
                    pos: target,
                    radius,
                }),
            )
            .await
        }
        (Some(x), _, Some(z)) => {
            let (x, z) = (
                x.as_i64().unwrap_or(0) as i32,
                z.as_i64().unwrap_or(0) as i32,
            );
            let _ = events.send(feedback(format!("going to x={x} z={z}")));
            timed(seconds, client.goto(XZGoal { x, z })).await
        }
        _ => {
            return err(
                id,
                "bad_request",
                "goto needs \"x\" and \"z\", optionally \"y\" and \"radius\"",
            );
        }
    };
    match result {
        Ok(()) => ok(
            id,
            json!({
                "from": vec3(started),
                "to": vec3(client.position()),
            }),
        ),
        Err(message) => {
            client.force_stop_pathfinding();
            err(id, "timeout", message)
        }
    }
}

async fn explore(
    client: &Client,
    events: &mpsc::UnboundedSender<Value>,
    id: u64,
    args: &Value,
) -> Value {
    let seconds = seconds(args, Duration::from_secs(30));
    let distance = args
        .get("distance")
        .and_then(Value::as_i64)
        .unwrap_or(96)
        .clamp(8, 512);
    let (dx, dz) = match args
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("north")
    {
        "north" => (0, -1),
        "south" => (0, 1),
        "east" => (1, 0),
        "west" => (-1, 0),
        "northeast" => (1, -1),
        "northwest" => (-1, -1),
        "southeast" => (1, 1),
        "southwest" => (-1, 1),
        other => return err(id, "bad_request", format!("unknown direction {other:?}")),
    };
    let started = client.position();
    let goal = XZGoal {
        x: started.x as i32 + dx * distance as i32,
        z: started.z as i32 + dz * distance as i32,
    };
    let _ = events.send(feedback(format!(
        "exploring toward x={} z={}",
        goal.x, goal.z
    )));
    match timed(seconds, client.goto(goal)).await {
        Ok(()) => ok(
            id,
            json!({
                "from": vec3(started),
                "to": vec3(client.position()),
            }),
        ),
        Err(message) => {
            client.force_stop_pathfinding();
            // An explore that ran out of time still went somewhere; that is
            // a result, not a failure.
            ok(
                id,
                json!({
                    "from": vec3(started),
                    "to": vec3(client.position()),
                    "reached": false,
                    "note": message,
                }),
            )
        }
    }
}

async fn mine(
    client: &Client,
    events: &mpsc::UnboundedSender<Value>,
    id: u64,
    args: &Value,
) -> Value {
    let seconds = seconds(args, Duration::from_secs(120));
    let want = args
        .get("count")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 64);
    let radius = args
        .get("radius")
        .and_then(Value::as_i64)
        .unwrap_or(32)
        .clamp(4, 64) as i32;
    let names: Vec<String> = match args.get("names").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        None => args
            .get("name")
            .and_then(Value::as_str)
            .map(|name| vec![name.to_string()])
            .unwrap_or_default(),
    };
    // `positions` names exact blocks to dig — a registered deposit — and
    // beats `names`: the host has already decided what may be dug. When
    // both are given the dug block must still be a listed kind.
    let explicit: Vec<BlockPos> = args
        .get("positions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_array)
                .filter_map(|pos| {
                    Some(BlockPos::new(
                        pos.first()?.as_i64()? as i32,
                        pos.get(1)?.as_i64()? as i32,
                        pos.get(2)?.as_i64()? as i32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut kinds = HashSet::<BlockKind>::new();
    for name in &names {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        match BlockKind::from_str(name) {
            Ok(kind) => {
                kinds.insert(kind);
            }
            Err(_) => return err(id, "bad_request", format!("unknown block {name:?}")),
        }
    }
    if kinds.is_empty() && explicit.is_empty() {
        return err(id, "bad_request", "mine needs \"name\", \"names\", or \"positions\"");
    }
    let states = BlockStates::from(&kinds);
    let deadline = tokio::time::Instant::now() + seconds;
    let mut mined = 0i64;
    let mut attempted = 0i64;
    let mut dug = Vec::<[i32; 3]>::new();
    let mut tried = HashSet::<BlockPos>::new();
    while mined < want {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        // Explicit positions are consumed in order; otherwise the nearest
        // matching block the bot has not already failed on, preferring
        // the one closest to the bot's own height — the base of a trunk
        // is both reachable and drops its item onto ground the
        // pathfinder can walk, while a canopy log does neither.
        let target = if explicit.is_empty() {
            let world = client.world();
            let instance = world.read();
            let center = BlockPos::from(client.position());
            instance
                .find_blocks(center, &states)
                .filter(|pos| pos.distance_to(center) <= f64::from(radius))
                .filter(|pos| pos.y <= center.y + MINE_MAX_ABOVE)
                .filter(|pos| !tried.contains(pos))
                .min_by_key(|pos| ((pos.y - center.y).abs(), pos.distance_to(center) as i64))
        } else {
            explicit.iter().find(|pos| !tried.contains(pos)).copied()
        };
        let Some(target) = target else {
            return if mined > 0 {
                ok(
                    id,
                    json!({"mined": mined, "attempted": attempted, "dug": dug,
                           "note": "no more candidates"}),
                )
            } else {
                err(
                    id,
                    "no_blocks",
                    format!("no matching block within {radius} blocks"),
                )
            };
        };
        attempted += 1;
        tried.insert(target);
        // An explicit position earns only if the block still holds a
        // listed kind — a deposit's ore cannot be pre-broken or swapped.
        if !explicit.is_empty() && !kinds.is_empty() {
            let matches = {
                let world = client.world();
                let instance = world.read();
                instance
                    .get_block_state(target)
                    .is_some_and(|state| states.contains(&state))
            };
            if !matches {
                let _ = events.send(feedback(format!(
                    "{target:?} is not a listed kind; skipping"
                )));
                continue;
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let _ = events.send(feedback(format!(
            "mining {} at {target:?}",
            if names.is_empty() {
                "a listed block".to_string()
            } else {
                names.join("/")
            }
        )));
        // Stand near it, then dig.
        if timed(
            remaining.min(MINE_APPROACH_WAIT),
            client.goto(RadiusGoal {
                pos: target.center(),
                radius: MINE_REACH,
            }),
        )
        .await
        .is_err()
        {
            let _ = events.send(feedback(format!("cannot reach {target:?}")));
            client.force_stop_pathfinding();
            continue;
        }
        let _ = events.send(feedback(format!("digging {target:?}")));
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if timed(left.min(MINE_STEP_WAIT), client.mine_with_auto_tool(target))
            .await
            .is_err()
        {
            let _ = events.send(feedback(format!("dig at {target:?} timed out")));
            stop_mining(client);
            continue;
        }
        mined += 1;
        dug.push([target.x, target.y, target.z]);
        let _ = events.send(feedback(format!("dug {target:?}")));
        // The drop falls where the block was and the pickup radius is
        // under a block — chase the item entity itself rather than the
        // block position, which can sit inside the next trunk segment.
        pickup_drops(client, deadline, events, target).await;
    }
    ok(id, json!({"mined": mined, "attempted": attempted, "dug": dug}))
}

/// Walk onto dropped-item entities near `near` until none are left or the
/// deadline is close. Items land where a mined block was, but they settle
/// on leaves or drift, so the entity's live position is the only honest
/// place to stand. Vanilla pickup needs the bot nearly on the drop, and a
/// drop that fell far away — down a ravine, say — is not worth the walk.
async fn pickup_drops(
    client: &Client,
    deadline: tokio::time::Instant,
    events: &mpsc::UnboundedSender<Value>,
    near: BlockPos,
) {
    let near = near.center();
    // The drop entity takes a tick to spawn and fall.
    tokio::time::sleep(Duration::from_millis(400)).await;
    for _ in 0..6 {
        let drops: Vec<Vec3> = client
            .nearest_entities_by::<(), (With<azalea_entity::metadata::Item>, Without<LocalEntity>)>(
                |_: ()| true,
            )
            .into_iter()
            .map(|entity| *client.entity_component::<Position>(entity))
            .filter(|pos| pos.distance_to(near) <= 14.0)
            .collect();
        let Some(&pos) = drops.first() else {
            let _ = events.send(feedback("no drops near the dug block".into()));
            return;
        };
        let _ = events.send(feedback(format!(
            "chasing a drop at ({:.1}, {:.1}, {:.1}), {} near",
            pos.x,
            pos.y,
            pos.z,
            drops.len()
        )));
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left < Duration::from_millis(500) {
            break;
        }
        let _ = timed(
            left.min(Duration::from_secs(6)),
            client.goto(RadiusGoal { pos, radius: 0.8 }),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

async fn wait(id: u64, args: &Value) -> Value {
    let seconds = seconds(args, Duration::from_secs(1));
    tokio::time::sleep(seconds).await;
    ok(id, json!({"waited": seconds.as_secs_f64()}))
}

fn feedback(text: String) -> Value {
    json!({"event": "feedback", "text": text})
}

fn vec3(position: Vec3) -> Value {
    json!({
        "x": (position.x * 100.0).round() / 100.0,
        "y": (position.y * 100.0).round() / 100.0,
        "z": (position.z * 100.0).round() / 100.0,
    })
}

fn seconds(args: &Value, default: Duration) -> Duration {
    args.get("seconds")
        .and_then(Value::as_f64)
        .map(Duration::from_secs_f64)
        .unwrap_or(default)
        .clamp(Duration::from_secs(1), Duration::from_secs(600))
}

async fn timed<T>(
    deadline: Duration,
    future: impl std::future::Future<Output = T>,
) -> Result<T, String> {
    match tokio::time::timeout(deadline, future).await {
        Ok(value) => Ok(value),
        Err(_) => Err(format!("timed out after {}s", deadline.as_secs())),
    }
}

fn stop_mining(client: &Client) {
    client.ecs.lock().write_message(StopMiningBlockEvent {
        entity: client.entity,
    });
}

fn ok(id: u64, result: Value) -> Value {
    json!({"id": id, "ok": true, "result": result})
}

fn err(id: u64, code: &str, error: impl std::fmt::Display) -> Value {
    json!({"id": id, "ok": false, "code": code, "error": error.to_string()})
}

fn emit(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, value);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}
