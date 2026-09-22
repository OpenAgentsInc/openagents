//! Guild episodes: several enrolled agents in one world.
//!
//! Where [`crate::episode`] runs one bot through a fixed curriculum, an
//! ensemble run puts every manifest-listed member in the world at once —
//! one `mc-bridge` child per agent, each bound to its enrolled username
//! and guild — and works the arena: go to camp, dig the guild's
//! deposits, contest the shared deposit. A block that a bound agent digs
//! out of a registered deposit earns credits in the run's ledger;
//! everything else is honest effort with no reward.
//!
//! The award path is the attribution claim:
//!
//! 1. The host picks the target — only positions the manifest registers
//!    are ever sent.
//! 2. The bridge refuses to dig a position whose block is not the
//!    deposit's declared kind, so placed or swapped ore cannot earn.
//! 3. Only positions the helper reports `dug` are proposed to the
//!    ledger.
//! 4. The ledger dedupes `(deposit, pos)`, so a contested block pays
//!    once no matter how many guilds swing at it.
//!
//! Gifts never enter this path — an item moving between inventories is
//! not a dig — and replays collide with the dedupe key.

use std::sync::{
    Arc, Mutex, mpsc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use atif::document::{Outcome, Session, Source, Step};
use nostr::domain::{RelaySigner, Tag};
use serde_json::{Map, Value, json};

use crate::bridge::{Bridge, Event};
use crate::decide::{Door, Picked};
use crate::episode::{Plan, Report, TaskResult};
use crate::error::{Error, Result};
use crate::guild::{self, Channel};
use crate::keys;
use crate::ledger::{Awarded, Ledger, Reserved};
use crate::relay::Relay;
use crate::server::Server;
use crate::world::{CombatSection, Deposit, Member, Temperament, World};

/// Seconds one `mine` chunk call may run — the roaming loop digs a
/// few blocks per call so scans stay frequent.
const MINE_SECONDS: u64 = 45;
/// Slack on a call's own deadline before the runner's patience ends.
const CALL_SLACK: Duration = Duration::from_secs(20);

/// One enrolled member and its live bridge.
struct AgentHandle<'a> {
    member: &'a Member,
    bridge: Bridge,
    /// The agent's Nostr identity — its derived key, matching the
    /// manifest's `pubkey` binding.
    signer: RelaySigner,
    /// Its guild channel, when the world runs a relay.
    channel: Option<Channel>,
}

/// One question for the door thread: legs post these and keep moving;
/// the answer lands in `Shared.answers` under `key` whenever the model
/// replies.
struct Ask {
    key: String,
    label: String,
    prompt: String,
    state: Value,
    options: Vec<(String, String)>,
}

/// A guildmate's call for help: where enemies were seen, who called
/// it, and when — members out of reach can answer it.
struct Rally {
    caller: String,
    spot: [i32; 3],
    enemies: usize,
    when: Instant,
}

/// What every agent thread shares: the world and plan read-only, the
/// mutable half — server console, ledger, trace, decision door — behind
/// mutexes so concurrent legs interleave without corrupting the record.
struct Shared<'a> {
    world: &'a World,
    plan: &'a Plan,
    /// The server console. `Arc` because each bridge's event hook holds
    /// a handle too — a join line hands out a kit mid-call.
    server: Arc<Mutex<Server>>,
    /// Players already handed a kit this episode — a join line can
    /// reach several bots' event streams, so the set, not the sighting,
    /// decides who gets one. `Arc` for the event hooks.
    kitted: Arc<Mutex<std::collections::HashSet<String>>>,
    ledger: Mutex<Ledger>,
    log: Mutex<atif::log::Log>,
    /// The door lives here until `episode` moves it onto its own
    /// thread — one model, one call at a time, fed by `asks`.
    door: Mutex<Option<Door>>,
    /// The door thread's inbox. `None` means no door: asks no-op.
    asks: Mutex<Option<mpsc::Sender<Ask>>>,
    /// Answered questions, keyed by the ask's `key` until a leg takes
    /// them — `Err` is a refused or failed call, still an answer.
    answers: Arc<Mutex<std::collections::HashMap<String, std::result::Result<Picked, String>>>>,
    /// The latest rally call per guild: a member that spots enemies
    /// posts where, and members out of reach can answer the call.
    intel: Mutex<std::collections::HashMap<String, Rally>>,
    /// The decision model's name, for naming it in chat — `jev-latest`
    /// and friends, never an anonymous "door".
    decision_model: String,
    /// Keys in flight — an ask posted but not yet answered. A leg
    /// never double-books the same question.
    pending: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Task results accumulate here while the legs run in parallel.
    tasks: Mutex<Vec<TaskResult>>,
    /// Skirmish kills per guild — the score every thread adds to.
    kills: Mutex<std::collections::BTreeMap<String, u64>>,
    actions: AtomicUsize,
    started: Instant,
    run_dir: std::path::PathBuf,
    progress: &'a (dyn Fn(&str) + Sync),
}

/// A runner for a world that enrolls agents.
struct Ensemble<'a> {
    relay: Option<Relay>,
    agents: Vec<AgentHandle<'a>>,
    trace: std::path::PathBuf,
    shared: Shared<'a>,
}

/// One agent's program, run on its own thread: its bridge, its guild
/// channel, and the shared record it reports through. `'s` is the
/// borrow of the shared state; `'w` is the world the members belong
/// to — they differ, so a leg never ties the ensemble down.
struct Leg<'s, 'w> {
    shared: &'s Shared<'w>,
    agent: AgentHandle<'w>,
}

/// Runs a multi-agent episode of `world` under `plan`.
///
/// The manifest's `agents` list is the enrollment — a bot joins only as
/// a listed username, under the pubkey the manifest binds to it.
///
/// # Errors
///
/// Returns the first fault the episode hits; the run directory, ledger,
/// and trace are complete up to that point.
pub fn run_ensemble(
    world: &World,
    plan: &Plan,
    progress: impl Fn(&str) + Sync,
) -> Result<Report> {
    if world.agents.is_empty() {
        return Err(Error::episode(format!(
            "world {} enrolls no agents; run it as a solo episode",
            world.name
        )));
    }
    let run_dir = plan.runs.join(format!(
        "{}-{}",
        atif::log::session_id(atif::document::now_ms()),
        world.name
    ));
    let server_dir = run_dir.join("server");
    std::fs::create_dir_all(&server_dir)?;
    let trace = run_dir.join("trace.jsonl");

    progress(&format!(
        "world {} ({}), {} enrolled agents, in {}",
        world.name,
        world.digest,
        world.agents.len(),
        run_dir.display()
    ));
    progress("starting the minecraft server");
    let server = Server::start(world, &plan.jar, &plan.java, &server_dir, plan.port)?;

    // A world with a `relay` section gets guild channels: one local
    // `nostr-relay`, one closed group per guild, every member enrolled
    // through management before the first agent speaks.
    let mut relay = None;
    let mut door = None;
    if let Some(section) = &world.relay {
        progress("starting the nostr relay");
        let started = Relay::start(
            &plan.relay_bin,
            &plan.relay_database,
            section.port,
            &run_dir.join("relay.log"),
        )?;
        let mgmt = RelaySigner::from_secret_hex(&crate::relay::management_secret())
            .map_err(|error| Error::relay(format!("management key: {error}")))?;
        let mut guilds: Vec<&str> = Vec::new();
        for member in &world.agents {
            if !guilds.contains(&member.guild.as_str()) {
                guilds.push(&member.guild);
            }
        }
        for guild_id in &guilds {
            // An earlier episode on this database may have left the
            // group; the new episode owns its guilds, with clean
            // history, so a stale one is dropped before creating.
            let _ = guild::manage(&started.http, &mgmt, "deletegroup", json!([guild_id]));
            guild::manage(
                &started.http,
                &mgmt,
                "creategroup",
                json!([
                    guild_id,
                    format!("{guild_id} guild"),
                    "a voyager arena guild",
                    "",
                    true,
                    mgmt.pubkey(),
                    [guild::CHAT_KIND]
                ]),
            )?;
            for member in world.agents.iter().filter(|m| m.guild == *guild_id) {
                guild::manage(
                    &started.http,
                    &mgmt,
                    "putgroupuser",
                    json!([guild_id, member.pubkey, ["member"]]),
                )?;
            }
        }
        if let Some(url) = &section.decision_url {
            let model = section.decision_model.as_deref().unwrap_or("kev-latest");
            // A loopback endpoint is a local `kev-serve`; anything
            // else is the live TypeSafe API and wants a credential.
            let local = ["127.0.0.1", "localhost", "[::1]"]
                .iter()
                .any(|host| url.contains(host));
            door = Some(if local {
                Door::local(url, model, run_dir.join("decisions"))?
            } else {
                Door::live(url, model, run_dir.join("decisions"))?
            });
        }
        relay = Some(started);
    }

    let session = Session::opening(
        &run_dir
            .file_name()
            .expect("a run dir has a name")
            .to_string_lossy(),
        concat!("voyager/", env!("CARGO_PKG_VERSION")),
        "minecraft-local",
        &world.digest,
        env!("CARGO_PKG_VERSION"),
    );
    let log = atif::log::Log::create_at(&trace, &session)?;
    let ledger = Ledger::open(run_dir.join("ledger.jsonl"))?;
    let mut ensemble = Ensemble {
        relay,
        agents: Vec::new(),
        trace,
        shared: Shared {
            world,
            plan,
            server: Arc::new(Mutex::new(server)),
            kitted: Arc::new(Mutex::new(std::collections::HashSet::new())),
            ledger: Mutex::new(ledger),
            log: Mutex::new(log),
            decision_model: door
                .as_ref()
                .map(|door| {
                    door.model()
                        .split('-')
                        .next()
                        .unwrap_or(door.model())
                        .to_string()
                })
                .unwrap_or_else(|| "the model".to_string()),
            door: Mutex::new(door),
            asks: Mutex::new(None),
            answers: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending: Arc::new(Mutex::new(std::collections::HashSet::new())),
            intel: Mutex::new(std::collections::HashMap::new()),
            tasks: Mutex::new(Vec::new()),
            kills: Mutex::new(std::collections::BTreeMap::new()),
            actions: AtomicUsize::new(0),
            started: Instant::now(),
            run_dir,
            progress: &progress,
        },
    };
    ensemble.note(Source::System, "server ready", json!({"port": plan.port}));

    let result = ensemble.episode();
    ensemble.note(Source::System, "episode over", json!({}));
    for handle in &mut ensemble.agents {
        let _ = handle.bridge.shutdown();
    }
    let _ = ensemble
        .shared
        .server
        .lock()
        .expect("the server lock is not poisoned")
        .stop();
    let _ = ensemble
        .shared
        .log
        .lock()
        .expect("the trace lock is not poisoned")
        .finish(if result.is_ok() {
            "ended"
        } else {
            "interrupted"
        });
    result
}

impl Ensemble<'_> {
    /// The bounded guild loop: everyone joins in order, then every
    /// member runs its own leg on its own thread — camp, guild
    /// deposits, the contested deposits for the first of each guild —
    /// and the barrier releases them all into the skirmish at once.
    /// The quest and the close run sequentially after: they act on the
    /// shared world, not in it.
    fn episode(&mut self) -> Result<Report> {
        let mut tasks = Vec::new();
        let address = format!("127.0.0.1:{}", self.shared.plan.port);

        // Grant each guild its starting balance, if the manifest
        // declares one — recorded like any other credit.
        let mut granted = std::collections::BTreeSet::new();
        if self.shared.world.economy.starting_credits > 0 {
            for member in &self.shared.world.agents {
                if granted.insert(member.guild.clone()) {
                    self.shared
                        .ledger
                        .lock()
                        .expect("the ledger lock is not poisoned")
                        .award(
                            &member.guild,
                            "genesis",
                            "starting",
                            [0, 0, 0],
                            self.shared.world.economy.starting_credits,
                        )?;
                }
            }
        }

        // Join every member. A join failure stops the episode — an
        // enrolled agent that cannot enter is a broken binding, not a
        // skip.
        for member in &self.shared.world.agents {
            self.shared.bounded()?;
            (self.shared.progress)(&format!("{} of {} joining", member.username, member.guild));
            let bridge = Bridge::start(&self.shared.plan.bridge)?;
            let signer = keys::agent_signer(&member.username)?;
            self.agents.push(AgentHandle {
                member,
                bridge,
                signer,
                channel: None,
            });
            let index = self.agents.len() - 1;
            // The join hook hands a kit to whoever the server reports
            // joining — this bot included — the moment the line lands,
            // inside whatever call is in flight.
            {
                let server = Arc::clone(&self.shared.server);
                let kitted = Arc::clone(&self.shared.kitted);
                let admins = self.shared.world.admins.clone();
                self.agents[index]
                    .bridge
                    .set_event_hook(move |event| {
                        if event.event != "chat" {
                            return;
                        }
                        let text = event.text("text").unwrap_or_default();
                        let Some(name) = text.strip_suffix(" joined the game") else {
                            return;
                        };
                        // `force-gamemode` pins joiners to the world's
                        // default — a listed admin lands in creative
                        // instead, and `op` covers every command.
                        if admins.iter().any(|admin| admin == name) {
                            let mut server =
                                server.lock().expect("the server lock is not poisoned");
                            let _ = server.command(&format!("gamemode creative {name}"));
                        }
                        let _ = Shared::kit(&server, &kitted, name);
                    });
            }
            self.call(
                index,
                "join",
                json!({"address": address, "username": member.username}),
                Duration::from_secs(75),
                "join the world",
            )?;
            self.say(
                index,
                &format!(
                    "{} of guild {} reporting for work — {}, {:.2} aggression.",
                    member.username,
                    member.guild,
                    member.temperament().as_str(),
                    member.aggression()
                ),
            )?;
            self.channel_open(index)?;
        }
        // Workers need tools: ore drops only under a pickaxe, so every
        // member gets a kit at the gate — any join the event stream
        // already caught is skipped by the kitted set.
        for member in &self.shared.world.agents {
            Shared::kit(&self.shared.server, &self.shared.kitted, &member.username)?;
        }
        self.check_membership_boundary(&mut tasks)?;

        // The door gets its own thread before the legs start: legs
        // post asks and collect answers, so a model round trip never
        // freezes a body mid-episode. No door means no thread and no
        // asks — every decide falls back to the caller's default.
        {
            let door = self
                .shared
                .door
                .lock()
                .expect("the door lock is not poisoned")
                .take();
            if door.is_some() {
                let (tx, rx) = mpsc::channel::<Ask>();
                *self.shared.asks.lock().expect("the ask queue lock is not poisoned") =
                    Some(tx);
                let answers = Arc::clone(&self.shared.answers);
                let pending = Arc::clone(&self.shared.pending);
                std::thread::spawn(move || {
                    let mut door = door;
                    while let Ok(ask) = rx.recv() {
                        let reply = match door.as_mut() {
                            Some(door) => door
                                .choose(ask.state, &ask.prompt, &ask.options, &ask.label)
                                .map_err(|error| error.to_string()),
                            None => Err("the model is gone".to_string()),
                        };
                        answers
                            .lock()
                            .expect("the answers lock is not poisoned")
                            .insert(ask.key.clone(), reply);
                        pending
                            .lock()
                            .expect("the pending lock is not poisoned")
                            .remove(&ask.key);
                    }
                });
            }
        }

        // The legs run in parallel: every member on its own thread,
        // driving its own bridge — camp, then the roaming loop of
        // work stretches and enemy scans.
        let shared = &self.shared;
        let agents = std::mem::take(&mut self.agents);
        let results: Vec<(usize, AgentHandle<'_>, Result<()>)> =
            std::thread::scope(|scope| {
                let mut handles = Vec::new();
                for (index, agent) in agents.into_iter().enumerate() {
                    handles.push(scope.spawn(move || {
                        let mut leg = Leg { shared, agent };
                        let result = leg.run();
                        (index, leg.agent, result)
                    }));
                }
                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
                    })
                    .collect()
            });
        let mut back: Vec<Option<AgentHandle>> = Vec::new();
        back.resize_with(results.len(), || None);
        let mut fault = None;
        for (index, agent, result) in results {
            back[index] = Some(agent);
            if let Err(error) = result
                && fault.is_none()
            {
                fault = Some(error);
            }
        }
        self.agents = back
            .into_iter()
            .map(|agent| agent.expect("an agent thread returns its handle"))
            .collect();
        tasks.append(
            &mut self
                .shared
                .tasks
                .lock()
                .expect("the task list lock is not poisoned"),
        );
        if let Some(error) = fault {
            return Err(error);
        }

        // The skirmish's score, once every fighter stands down.
        let guilds: std::collections::BTreeSet<&str> = self
            .shared
            .world
            .agents
            .iter()
            .map(|member| member.guild.as_str())
            .collect();
        if self.shared.world.combat.is_some() && guilds.len() >= 2 {
            let score = {
                let kills = self
                    .shared
                    .kills
                    .lock()
                    .expect("the kill count lock is not poisoned");
                guilds
                    .iter()
                    .map(|guild| {
                        format!("{guild} {}", kills.get(*guild).copied().unwrap_or(0))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = self.say(0, &format!("the skirmish ends — {score}."));
            tasks.push(TaskResult {
                task: "skirmish".to_string(),
                ok: true,
                detail: score,
            });
        }

        // The coding quest, when the world offers one: the first member
        // spends its guild's credits on a bounded, verified patch.
        self.task_quest(0, &mut tasks)?;

        // Report: every guild hears its own close, and an outside reader
        // confirms the channels stayed public for reading.
        let balances =
            crate::ledger::describe(&self.shared.ledger.lock().expect("the ledger lock is not poisoned"));
        self.note(Source::System, "ledger", balances.clone());
        for index in 0..self.agents.len() {
            let guild = self.agents[index].member.guild.clone();
            let balance = self
                .shared
                .ledger
                .lock()
                .expect("the ledger lock is not poisoned")
                .balance(&guild);
            let closing = format!(
                "Guild {} closes with {} available, {} held, {} spent.",
                guild, balance.available, balance.reserved, balance.spent
            );
            let _ = self.say(index, &closing);
            self.channel_chat(index, &closing)?;
            let _ = self.agents[index].bridge.disconnect();
        }
        self.check_public_read(&mut tasks)?;
        tasks.push(TaskResult {
            task: "guild work".to_string(),
            ok: true,
            detail: serde_json::to_string(&balances).unwrap_or_default(),
        });

        Ok(self.report(tasks))
    }


    /// The coding quest: the guild's credits hold while the solver
    /// works, the patch it leaves is verified on a base it never saw,
    /// and only an accepted artifact buys the manifest-named world
    /// effect — which the referee reads back block by block before the
    /// achievement label and the XP record land. Execution,
    /// verification, and integration each keep their own file under
    /// `quest/`.
    fn task_quest(&mut self, index: usize, tasks: &mut Vec<TaskResult>) -> Result<()> {
        let Some(quest) = self.shared.world.quest.clone() else {
            return Ok(());
        };
        let guild = self.agents[index].member.guild.clone();
        let username = self.agents[index].member.username.clone();
        let hold = format!("quest:{}", quest.id);
        match self
            .shared
            .ledger
            .lock()
            .expect("the ledger lock is not poisoned")
            .reserve(&hold, &guild, &quest.id, quest.cost)?
        {
            Reserved::Held => {}
            other => {
                tasks.push(TaskResult {
                    task: format!("{username}: quest {}", quest.id),
                    ok: false,
                    detail: format!("{guild} could not hold {} credits: {other:?}", quest.cost),
                });
                return Ok(());
            }
        }
        (self.shared.progress)(&format!(
            "quest {}: {} credits held for {guild}",
            quest.id, quest.cost
        ));
        let _ = self.say(
            index,
            &format!(
                "{guild} holds {} credits for the {} quest.",
                quest.cost, quest.id
            ),
        );
        self.note(
            Source::System,
            "quest reserved",
            json!({"quest": quest.id, "guild": guild, "cost": quest.cost}),
        );

        let dir = self.shared.run_dir.join("quest");
        let work = dir.join("work");
        let fixture = self.shared.plan.repo.join(&quest.fixture);
        let (base, fixture_digest) = crate::quest::stage(&fixture, &work)?;

        // The solve loop: the bounded solver writes, the public checks
        // answer, and a repair stays inside the shared budget and the
        // attempt cap.
        let mut attempts = Vec::new();
        for n in 1..=quest.attempts {
            let attempt = crate::quest::attempt(
                &work,
                n,
                &|tree: &std::path::Path| {
                    std::fs::write(tree.join("src/lib.rs"), crate::quest::builtin_source())
                        .map_err(|error| Error::episode(format!("solver write: {error}")))
                },
                &dir,
            )?;
            (self.shared.progress)(&format!(
                "quest {}: attempt {} — {}",
                quest.id, n, attempt.ending
            ));
            let passed = attempt.passed;
            attempts.push(attempt);
            if passed {
                break;
            }
        }
        let per_attempt = (quest.cost / u64::from(quest.attempts)).max(1);
        let spent = per_attempt * u64::from(attempts.len() as u32);
        let execution = crate::quest::seal(&work, &dir, &base, &fixture_digest, attempts)?;
        self.shared
            .ledger
            .lock()
            .expect("the ledger lock is not poisoned")
            .settle(&hold, spent)?;
        self.note(
            Source::System,
            &format!(
                "quest executed: {} spent, patch {}",
                spent, execution.patch_digest
            ),
            json!({"spent": spent, "base_commit": execution.base_commit}),
        );
        if !execution.attempts.last().is_some_and(|a| a.passed) {
            let _ = self.say(index, "the patch never passed its own checks.");
            tasks.push(TaskResult {
                task: format!("{username}: quest {}", quest.id),
                ok: false,
                detail: format!("{} attempts inside the budget, none passed", quest.attempts),
            });
            return Ok(());
        }
        let _ = self.say(
            index,
            &format!("patch sealed, {spent} spent — the referee is checking it."),
        );

        // Verification is the referee's: the artifact applied to a
        // fresh base, with the cases the solver never saw.
        let verification =
            crate::quest::verify(&fixture, &dir.join("verify"), &execution.patch, &dir)?;
        self.note(
            Source::System,
            &format!("quest verified: {}", verification.ending),
            json!({"accepted": verification.accepted}),
        );
        if !verification.accepted {
            let _ = self.say(index, "the referee refused the patch.");
            tasks.push(TaskResult {
                task: format!("{username}: quest {}", quest.id),
                ok: false,
                detail: "the protected cases refused the patch".to_string(),
            });
            return Ok(());
        }
        let _ = self.say(index, "referee accepted — opening the bridge.");

        // Integration: the effect is a manifest name, never a command
        // the quest supplied; another guild's member reads the blocks
        // back.
        let command = self
            .shared
            .world
            .effects
            .get(&quest.effect)
            .cloned()
            .unwrap_or_default();
        self.shared
            .server
            .lock()
            .expect("the server lock is not poisoned")
            .command(&command)?;
        let referee = self.agents.len() - 1;
        let mut blocks = Vec::new();
        for pos in &quest.verify_blocks {
            // The console write returns before the server ticks; the
            // referee polls until the block update reaches its client
            // or the bound runs out — the last read is the record.
            let mut answer = Value::Null;
            for _ in 0..20 {
                answer = self.call(
                    referee,
                    "block_at",
                    json!({"position": pos}),
                    Duration::from_secs(15),
                    "reconcile the world effect",
                )?;
                let kind = answer["kind"].as_str().unwrap_or_default();
                if kind.strip_prefix("minecraft:").unwrap_or(kind) == quest.verify_kind {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            let kind = answer["kind"].as_str().unwrap_or_default();
            let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
            blocks.push(json!({
                "position": pos,
                "kind": answer["kind"],
                "expected": quest.verify_kind,
                "ok": kind == quest.verify_kind,
            }));
        }
        let reconciled = blocks
            .iter()
            .all(|check| check["ok"].as_bool().unwrap_or(false));
        let mut label = json!({"published": false, "why": "reconciliation failed"});
        if reconciled {
            self.shared
                .ledger
                .lock()
                .expect("the ledger lock is not poisoned")
                .xp(&guild, &username, &quest.id, quest.xp)?;
            let _ = self.say(
                index,
                &format!("bridge confirmed — {} xp to {username}.", quest.xp),
            );
            label = self.publish_label(&quest, index, &execution.patch_digest)?;
        } else {
            let _ = self.say(index, "the bridge did not come out right.");
        }
        let integration = json!({
            "effect": quest.effect,
            "command": command,
            "reconciled": reconciled,
            "blocks": blocks,
            "label": label,
            "xp": reconciled.then_some(quest.xp),
        });
        std::fs::write(
            dir.join("integration.json"),
            serde_json::to_vec_pretty(&integration)?,
        )?;
        self.note(
            Source::System,
            &format!("quest integrated: reconciled={reconciled}"),
            integration,
        );
        tasks.push(TaskResult {
            task: format!("{username}: quest {}", quest.id),
            ok: reconciled,
            detail: if reconciled {
                format!("patch accepted; {} opened; {} xp", quest.effect, quest.xp)
            } else {
                "the world effect did not reconcile".to_string()
            },
        });
        Ok(())
    }

    /// The NIP-32 achievement label: `kind:1985`, `openagents.voyager`
    /// namespace, targeting the member's pubkey, signed by the host's
    /// relay-management key — the trusted identity the guilds already
    /// accept — and published to the episode relay. `label.json` keeps
    /// the event and the verdict.
    fn publish_label(
        &mut self,
        quest: &crate::world::QuestSection,
        index: usize,
        patch_digest: &str,
    ) -> Result<Value> {
        let Some(relay) = &self.relay else {
            return Ok(json!({"published": false, "why": "the world runs no relay"}));
        };
        let signer = RelaySigner::from_secret_hex(&crate::relay::management_secret())
            .map_err(|error| Error::relay(format!("label signer: {error}")))?;
        let url = relay.url.clone();
        let mut channel = Channel::connect(&url, &signer)?;
        let event = signer.sign(
            guild::unix_now(),
            1985,
            vec![
                Tag::new(vec!["L".into(), "openagents.voyager".into()]),
                Tag::new(vec![
                    "l".into(),
                    "quest-complete".into(),
                    "openagents.voyager".into(),
                ]),
                Tag::new(vec!["p".into(), self.agents[index].member.pubkey.clone()]),
                Tag::new(vec!["t".into(), quest.id.clone()]),
            ],
            json!({"quest": quest.id, "patch": patch_digest}).to_string(),
        );
        let verdict = channel.publish(&event)?;
        let record = json!({
            "published": verdict.accepted,
            "message": verdict.message,
            "signer": signer.pubkey(),
            "event": serde_json::to_value(&event).unwrap_or_default(),
        });
        let path = self.shared.run_dir.join("quest").join("label.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&record)?)?;
        self.note(
            Source::System,
            &format!("quest label: {}", describe_verdict(&verdict)),
            record.clone(),
        );
        Ok(record)
    }

    /// Opens the member's guild channel: connect, answer AUTH as the
    /// enrolled key, post the join line. A world without a relay skips
    /// this — the member still plays.
    fn channel_open(&mut self, index: usize) -> Result<()> {
        let Some(relay) = &self.relay else {
            return Ok(());
        };
        let url = relay.url.clone();
        let signer = self.agents[index].signer.clone();
        let group = self.agents[index].member.guild.clone();
        let username = self.agents[index].member.username.clone();
        let mut channel = Channel::connect(&url, &signer)?;
        let verdict = channel.chat(&signer, &group, &format!("{username} online."))?;
        self.note(
            Source::System,
            &format!("{username} on {group}: {}", describe_verdict(&verdict)),
            json!({"accepted": verdict.accepted, "message": verdict.message}),
        );
        if !verdict.accepted {
            return Err(Error::episode(format!(
                "{username} could not speak in {group}: {}",
                verdict.message
            )));
        }
        self.agents[index].channel = Some(channel);
        Ok(())
    }

    /// One kind-9 line into the member's own guild channel.
    fn channel_chat(&mut self, index: usize, text: &str) -> Result<()> {
        self.shared.channel_chat(&mut self.agents[index], text)
    }

    /// The membership boundary, exercised once: a ferro member writes
    /// to lumen's channel and the relay must refuse it. A pass is a
    /// recorded `restricted:` verdict, not silence.
    fn check_membership_boundary(&mut self, tasks: &mut Vec<TaskResult>) -> Result<()> {
        let Some(_relay) = &self.relay else {
            return Ok(());
        };
        let mut pair = None;
        'find: for (index, handle) in self.agents.iter().enumerate() {
            for other in &self.shared.world.agents {
                if other.guild != handle.member.guild {
                    pair = Some((index, other.guild.clone()));
                    break 'find;
                }
            }
        }
        let Some((index, foreign)) = pair else {
            return Ok(());
        };
        let username = self.agents[index].member.username.clone();
        let signer = self.agents[index].signer.clone();
        let Some(channel) = self.agents[index].channel.as_mut() else {
            return Ok(());
        };
        let verdict = channel.chat(
            &signer,
            &foreign,
            &format!("{username} knocking where it does not belong."),
        )?;
        let ok = !verdict.accepted;
        self.note(
            Source::System,
            &format!(
                "{username} writing {foreign}: {}",
                describe_verdict(&verdict)
            ),
            json!({"accepted": verdict.accepted, "message": verdict.message}),
        );
        tasks.push(TaskResult {
            task: format!("{username}: restricted write to {foreign}"),
            ok,
            detail: if ok {
                verdict.message
            } else {
                "the relay accepted a nonmember write".to_string()
            },
        });
        Ok(())
    }

    /// Public read: an unauthenticated observer — a fresh key with no
    /// membership anywhere — must still read the guilds' kind-9
    /// history.
    fn check_public_read(&mut self, tasks: &mut Vec<TaskResult>) -> Result<()> {
        let Some(relay) = &self.relay else {
            return Ok(());
        };
        let observer = RelaySigner::from_secret_hex(&observer_secret())
            .map_err(|error| Error::relay(format!("observer key: {error}")))?;
        let mut channel = Channel::connect(&relay.url, &observer)?;
        for guild_id in self
            .shared
            .world
            .agents
            .iter()
            .map(|m| m.guild.clone())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let events = channel.read(serde_json::json!({
                "kinds": [guild::CHAT_KIND],
                "#h": [guild_id],
            }))?;
            self.note(
                Source::System,
                &format!("observer read {guild_id}: {} messages", events.len()),
                json!({"count": events.len()}),
            );
            tasks.push(TaskResult {
                task: format!("observer reads {guild_id}"),
                ok: !events.is_empty(),
                detail: format!("{} kind-9 events visible", events.len()),
            });
        }
        Ok(())
    }

    /// One bridge exchange on agent `index`'s bridge.
    fn call(
        &mut self,
        index: usize,
        op: &str,
        args: Value,
        deadline: Duration,
        task: &str,
    ) -> Result<Value> {
        self.shared
            .call(&mut self.agents[index], op, args, deadline, task)
    }

    /// A bot says something in the world.
    fn say(&mut self, index: usize, text: &str) -> Result<()> {
        self.shared.say(&mut self.agents[index], text)
    }

    /// A lifecycle or observation step.
    fn note(&self, source: Source, message: &str, extra: Value) {
        self.shared.note(source, message, extra)
    }

    /// What the episode leaves behind.
    fn report(&self, tasks: Vec<TaskResult>) -> Report {
        Report {
            world: self.shared.world.name.clone(),
            digest: self.shared.world.digest.clone(),
            tasks,
            actions: self.shared.actions.load(Ordering::Relaxed),
            run_dir: self.shared.run_dir.clone(),
            trace: self.trace.clone(),
        }
    }
}

impl Leg<'_, '_> {
    /// The work program every member runs on its own thread: to camp,
    /// then the roaming loop — digging in small stretches with enemy
    /// scans between them, so combat breaks out wherever paths cross.
    fn run(&mut self) -> Result<()> {
        self.shared.task_camp(&mut self.agent)?;
        self.shared.task_work(&mut self.agent)
    }
}

impl Shared<'_> {
    /// The episode bounds, checked before every exchange.
    fn bounded(&self) -> Result<()> {
        if self.actions.load(Ordering::Relaxed) >= self.world.episode.max_actions as usize {
            return Err(Error::episode(format!(
                "the episode used its {} actions",
                self.world.episode.max_actions
            )));
        }
        if self.started.elapsed() > Duration::from_secs(self.world.episode.max_seconds) {
            return Err(Error::episode(format!(
                "the episode used its {} seconds",
                self.world.episode.max_seconds
            )));
        }
        Ok(())
    }

    /// Walk a member to its manifest camp. No camp means the task is a
    /// pass — the member works from spawn.
    fn task_camp(&self, agent: &mut AgentHandle<'_>) -> Result<()> {
        let Some([x, _, z]) = agent.member.camp else {
            return Ok(());
        };
        let username = agent.member.username.clone();
        let (x, z) = (x.round() as i64, z.round() as i64);
        self.bounded()?;
        let result = self.call(
            agent,
            "goto",
            json!({"x": x, "z": z, "seconds": 45}),
            Duration::from_secs(65),
            "walk to camp",
        );
        match result {
            Ok(value) => {
                let to = &value["to"];
                self.tasks
                    .lock()
                    .expect("the task list lock is not poisoned")
                    .push(TaskResult {
                        task: format!("{username}: reach camp"),
                        ok: true,
                        detail: format!("at ({}, {}, {})", to["x"], to["y"], to["z"]),
                    });
            }
            Err(Error::Refused { .. }) => {
                self.tasks
                    .lock()
                    .expect("the task list lock is not poisoned")
                    .push(TaskResult {
                        task: format!("{username}: reach camp"),
                        ok: false,
                        detail: "the walk did not complete".to_string(),
                    });
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// The roaming loop every member runs: dig the guild's deposits a
    /// few blocks at a time — the contested deposits too, for the
    /// first member of each guild — and between every stretch scan for
    /// enrolled enemies inside the aggro radius. The door decides
    /// whether a sighting becomes a hunt, so combat breaks out
    /// wherever paths cross, not on a schedule. When the work runs
    /// out, the member keeps patrolling the contested ground for the
    /// round budget.
    fn task_work(&self, agent: &mut AgentHandle<'_>) -> Result<()> {
        let guild = agent.member.guild.clone();
        let username = agent.member.username.clone();
        let combat = self.world.combat.clone();
        let first = self
            .world
            .agents
            .iter()
            .position(|member| member.guild == guild)
            == self
                .world
                .agents
                .iter()
                .position(|member| member.username == username);
        let mut contested: Vec<&Deposit> = if first {
            self.world
                .deposits
                .iter()
                .filter(|deposit| deposit.guild.is_none())
                .collect()
        } else {
            Vec::new()
        };
        // The contested ordering ask goes out now — the answer lands
        // while the guild work runs, so the contested phase never
        // waits on the model.
        let order_key = if contested.len() > 1 && self.has_door() {
            self.ask_order(agent, &contested)
        } else {
            None
        };
        // Per-member combat memory: who it has already announced a
        // hunt on, and when it last asked the door — the door call is
        // a real round trip shared by every leg.
        let mut engaged = std::collections::HashSet::new();
        let mut asked = Instant::now() - Duration::from_secs(60);
        let mut answered = Instant::now();
        for deposit in self
            .world
            .deposits
            .iter()
            .filter(|deposit| deposit.guild.as_deref() == Some(guild.as_str()))
        {
            self.work_deposit(agent, deposit, &combat, &mut engaged, &mut asked, &mut answered)?;
        }
        if first && !contested.is_empty() {
            if let Some(key) = order_key {
                contested = self.await_order(agent, contested, &key)?;
            }
            for deposit in contested {
                self.work_deposit(agent, deposit, &combat, &mut engaged, &mut asked, &mut answered)?;
            }
        }
        // Work's done — patrol for the round budget: scan, and let
        // the door turn sightings into hunts. Fighters hold the
        // contested ground; a skittish member watches its own camp; a
        // worker keeps moving between the guild's claims — idle is
        // not a personality this world allows.
        if let Some(combat) = &combat {
            // Patrols ring the whole arena, seeded per member so the
            // eight spread out — a rally then pulls distant members
            // into one spot, and convergence is what coordination
            // looks like.
            const RING: [[i32; 2]; 8] = [
                [-24, -12],
                [-8, -14],
                [8, -14],
                [24, -12],
                [26, 4],
                [16, 14],
                [0, 16],
                [-16, 14],
            ];
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            agent.member.username.hash(&mut hasher);
            let seed = hasher.finish() as usize;
            let ring = |i: usize| -> [i32; 3] {
                let [x, z] = RING[(seed + i) % RING.len()];
                [x, combat.ground[1], z]
            };
            let mut rounds = Vec::new();
            match agent.member.temperament() {
                Temperament::Worker | Temperament::Skittish => {
                    // The far edge, then home claims — cross-map legs
                    // between every waypoint.
                    rounds.push(ring(0));
                    rounds.extend(
                        self.world
                            .deposits
                            .iter()
                            .filter(|deposit| {
                                deposit.guild.as_deref() == Some(guild.as_str())
                            })
                            .filter_map(|deposit| deposit.blocks.first().copied()),
                    );
                    if let Some([x, y, z]) = agent.member.camp {
                        rounds.push([x as i32, y as i32, z as i32]);
                    }
                    if agent.member.temperament() == Temperament::Worker {
                        rounds.push(ring(4));
                    }
                }
                _ => {
                    // Fighters alternate the ground and their stretch
                    // of the rim — the war stays theirs, but it is
                    // never all in one clump.
                    rounds.push(combat.ground);
                    rounds.push(ring(0));
                    rounds.push(ring(2));
                }
            }
            if rounds.is_empty() {
                rounds.push(combat.ground);
            }
            // Patrol until the episode's own bound ends it — a member
            // whose work is done keeps moving, never parks.
            let mut round = 0usize;
            while self.bounded().is_ok() {
                if self
                    .sweep(
                        agent,
                        Some(combat),
                        &mut engaged,
                        &mut asked,
                        &mut answered,
                        "patrolling",
                    )
                    .is_err()
                {
                    break;
                }
                self.to_ground(agent, rounds[round % rounds.len()]);
                round += 1;
            }
        }
        Ok(())
    }

    /// One deposit, dug a few blocks at a time with an enemy scan
    /// between stretches. Awards land per chunk — a fight mid-deposit
    /// doesn't forfeit what was already dug.
    fn work_deposit(
        &self,
        agent: &mut AgentHandle<'_>,
        deposit: &Deposit,
        combat: &Option<CombatSection>,
        engaged: &mut std::collections::HashSet<String>,
        asked: &mut Instant,
        answered: &mut Instant,
    ) -> Result<()> {
        let guild = agent.member.guild.clone();
        let username = agent.member.username.clone();
        let mut mined = 0i64;
        let mut awarded = 0u64;
        let mut duplicates = 0u64;
        for (piece, chunk) in deposit.blocks.chunks(3).enumerate() {
            self.bounded()?;
            self.sweep(agent, combat.as_ref(), engaged, asked, answered, "working")?;
            let names: Vec<&str> = if deposit.kind.is_empty() {
                Vec::new()
            } else {
                vec![deposit.kind.as_str()]
            };
            if piece == 0 {
                self.say(agent, &format!("{username}: digging {} now.", deposit.id))?;
            }
            let result = self.call(
                agent,
                "mine",
                json!({
                    "names": names,
                    "positions": chunk,
                    "count": chunk.len() as u32,
                    "seconds": MINE_SECONDS,
                }),
                Duration::from_secs(MINE_SECONDS) + CALL_SLACK,
                &format!("mine {}", deposit.id),
            );
            let result = match result {
                Ok(value) => value,
                Err(Error::Refused { code, message }) => {
                    self.tasks
                        .lock()
                        .expect("the task list lock is not poisoned")
                        .push(TaskResult {
                            task: format!("{username}: mine {}", deposit.id),
                            ok: false,
                            detail: format!("{code}: {message}"),
                        });
                    break;
                }
                Err(error) => return Err(error),
            };
            mined += result.get("mined").and_then(Value::as_i64).unwrap_or(0);
            let (earned, dupes) = self.award_dug(&guild, &username, &result)?;
            awarded += earned;
            duplicates += dupes;
        }
        let detail =
            format!("dug {mined}, earned {awarded} blocks, {duplicates} already claimed");
        self.note(
            Source::System,
            &format!("{username} at {}: {detail}", deposit.id),
            json!({"deposit": deposit.id}),
        );
        if awarded > 0 || duplicates > 0 {
            let line = if duplicates > 0 {
                format!(
                    "{username} dug {awarded} at {}, {duplicates} already claimed.",
                    deposit.id
                )
            } else {
                format!("{username} dug {awarded} at {} for {guild}.", deposit.id)
            };
            // The world hears the result too — the guild channel
            // is Nostr-side and invisible to anyone watching in
            // game.
            let _ = self.say(agent, &line);
            if awarded > 0 {
                let _ = self.channel_chat(agent, &line);
            }
        }
        self.tasks
            .lock()
            .expect("the task list lock is not poisoned")
            .push(TaskResult {
                task: format!("{username}: mine {}", deposit.id),
                ok: awarded > 0,
                detail,
            });
        Ok(())
    }

    /// The award half of a dig result: only positions the helper
    /// reports `dug`, that the manifest registers, earn — contested
    /// dedupe lives in the ledger.
    fn award_dug(&self, guild: &str, username: &str, result: &Value) -> Result<(u64, u64)> {
        let mut awarded = 0u64;
        let mut duplicates = 0u64;
        if let Some(list) = result.get("dug").and_then(Value::as_array) {
            for pos in list {
                let Some(pos) = pos.as_array() else {
                    continue;
                };
                let (Some(x), Some(y), Some(z)) = (
                    pos.first().and_then(Value::as_i64),
                    pos.get(1).and_then(Value::as_i64),
                    pos.get(2).and_then(Value::as_i64),
                ) else {
                    continue;
                };
                let pos = [x as i32, y as i32, z as i32];
                let Some(hit) = self.world.deposit_at(pos) else {
                    continue;
                };
                if let Some(owner) = &hit.guild
                    && *owner != guild
                {
                    continue;
                }
                match self
                    .ledger
                    .lock()
                    .expect("the ledger lock is not poisoned")
                    .award(guild, username, &hit.id, pos, hit.award)?
                {
                    Awarded::Recorded => awarded += 1,
                    Awarded::Duplicate => duplicates += 1,
                }
            }
        }
        Ok((awarded, duplicates))
    }

    /// One look around: enrolled enemies inside the aggro radius turn
    /// into a door decision — hunt, keep working, or retreat — sampled
    /// by the model's probabilities so the same sighting does not
    /// always end the same way. The door answers at most once a
    /// stretch per member — its call is a real round trip shared by
    /// every leg; between asks the member fights on.
    fn sweep(
        &self,
        agent: &mut AgentHandle<'_>,
        combat: Option<&CombatSection>,
        engaged: &mut std::collections::HashSet<String>,
        asked: &mut Instant,
        answered: &mut Instant,
        context: &str,
    ) -> Result<()> {
        let Some(combat) = combat else {
            return Ok(());
        };
        let username = agent.member.username.clone();
        let guild = agent.member.guild.clone();
        let temperament = agent.member.temperament();
        let aggression = agent.member.aggression();
        // Aggression is the member's nose for a fight: 0 smells an
        // enemy at half the radius, 1 at half again beyond it.
        let aggro = combat.aggro_radius * (0.5 + aggression);
        let seen = match self.call(
            agent,
            "players",
            json!({}),
            Duration::from_secs(20),
            "scan for enemies",
        ) {
            Ok(value) => value,
            Err(Error::Refused { .. }) => return Ok(()),
            Err(error) => return Err(error),
        };
        let mut enemies: Vec<(String, f64, [i32; 3])> = seen
            .get("players")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|player| {
                let name = player.get("username")?.as_str()?.to_string();
                let distance = player.get("distance")?.as_f64()?;
                let position = player.get("position")?;
                let spot = [
                    position.get("x").and_then(Value::as_f64).unwrap_or(0.0).round() as i32,
                    position.get("y").and_then(Value::as_f64).unwrap_or(0.0).round() as i32,
                    position.get("z").and_then(Value::as_f64).unwrap_or(0.0).round() as i32,
                ];
                let member = self
                    .world
                    .agents
                    .iter()
                    .find(|member| member.username == name)?;
                (member.guild != guild && distance <= aggro)
                    .then_some((name, distance, spot))
            })
            .collect();
        enemies.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if enemies.is_empty() {
            // Nobody in reach — maybe a guildmate already found the
            // fight and called a rally. Fresh calls only; a member
            // never answers its own and never answers twice.
            let rally = {
                let intel = self.intel.lock().expect("the intel lock is not poisoned");
                intel.get(&guild).and_then(|rally| {
                    (rally.when.elapsed() < Duration::from_secs(45)
                        && rally.caller != username
                        && rally.when > *answered
                        && answered.elapsed() > Duration::from_secs(20))
                    .then_some((rally.caller.clone(), rally.spot, rally.enemies, rally.when))
                })
            };
            if let Some((caller, spot, count, when)) = rally {
                // The door weighs answering like anything else —
                // posted now, collected on the next sighting — while
                // temperament answers until the model has spoken.
                let rally_key = format!("{username}:rally");
                self.ask(
                    &rally_key,
                    &format!("{username} heard {caller}'s rally"),
                    "A guildmate called a rally where enemies are. Do you answer the call?",
                    json!({
                        "agent": username,
                        "temperament": temperament.as_str(),
                        "aggression": aggression,
                        "doing": context,
                        "caller": caller,
                        "enemies_seen": count,
                    }),
                    vec![
                        ("answer".to_string(), "run to the rally and fight".to_string()),
                        ("ignore".to_string(), "stay on your own work".to_string()),
                    ],
                );
                let answers_call = match self.take_answer(&rally_key) {
                    Some(Ok(picked)) => {
                        let sampled = sample(&picked);
                        self.note(
                            Source::System,
                            &format!(
                                "{username} on {caller}'s rally: {} of {} ({:.2})",
                                sampled, picked.choice, picked.confidence
                            ),
                            json!({
                                "picked": picked.choice,
                                "sampled": sampled,
                                "probabilities": picked.probabilities,
                                "record": picked.record,
                            }),
                        );
                        let _ = self.say(
                            agent,
                            &format!(
                                "{username}: {} picked {} of {} ({:.2})",
                                self.decision_model,
                                sampled,
                                picked.choice,
                                picked.confidence
                            ),
                        );
                        sampled == "answer"
                    }
                    _ => {
                        matches!(
                            temperament,
                            Temperament::Berserker | Temperament::Hunter
                        ) || aggression >= 0.75
                    }
                };
                if answers_call {
                    *answered = when;
                    let _ = self.say(
                        agent,
                        &format!(
                            "{username}: answering {caller}'s rally at ({}, {})!",
                            spot[0], spot[2]
                        ),
                    );
                    let _ = self.call(
                        agent,
                        "goto",
                        json!({"x": spot[0], "y": spot[1], "z": spot[2],
                               "radius": 2.5, "seconds": 12}),
                        Duration::from_secs(20),
                        "answer the rally",
                    );
                    return Ok(());
                }
            }
            // Nobody in reach — the fighters and the simply furious go
            // looking for the fight; everyone else holds their ground.
            if !matches!(temperament, Temperament::Berserker | Temperament::Hunter)
                && aggression < 0.7
            {
                return Ok(());
            }
            let nearest = seen
                .get("players")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|player| {
                    let Some(name) = player.get("username").and_then(Value::as_str)
                    else {
                        return false;
                    };
                    self.world
                        .agents
                        .iter()
                        .any(|member| member.username == name && member.guild != guild)
                })
                .min_by(|a, b| {
                    let distance = |p: &&Value| {
                        p.get("distance").and_then(Value::as_f64).unwrap_or(f64::MAX)
                    };
                    distance(a).partial_cmp(&distance(b)).unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(position) = nearest.and_then(|player| player.get("position")) {
                let (x, z) = (
                    position.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                    position.get("z").and_then(Value::as_f64).unwrap_or(0.0),
                );
                let _ = self.call(
                    agent,
                    "goto",
                    json!({"x": x.round() as i64, "z": z.round() as i64, "seconds": 8}),
                    Duration::from_secs(15),
                    "close on the enemy",
                );
            }
            return Ok(());
        }
        // Enemies in reach — call it out for the guild. A rally
        // refreshes every fifteen seconds at most, so a brawl does
        // not spam the channel with sightings of the same fight.
        {
            let mut intel = self.intel.lock().expect("the intel lock is not poisoned");
            let stale = intel
                .get(&guild)
                .map(|rally| rally.when.elapsed() > Duration::from_secs(15))
                .unwrap_or(true);
            if stale {
                let spot = enemies[0].2;
                intel.insert(
                    guild.clone(),
                    Rally {
                        caller: username.clone(),
                        spot,
                        enemies: enemies.len(),
                        when: Instant::now(),
                    },
                );
                drop(intel);
                let line = format!(
                    "{username}: rally at ({}, {}) — {} hostiles!",
                    spot[0],
                    spot[2],
                    enemies.len()
                );
                let _ = self.say(agent, &line);
                let _ = self.channel_chat(agent, &line);
            }
        }
        // Aggression is also impatience: 0 asks the door every thirty
        // seconds, 1 every ten.
        let cooldown = Duration::from_secs_f64(30.0 - 20.0 * aggression);
        // A door answer may have landed since the last sweep — the ask
        // went out sight-unseen and the reply applies here. Otherwise
        // post this sighting and run on temperament until the model
        // replies; the leg never waits on the model.
        let engage_key = format!("{username}:engage");
        let answered = match self.take_answer(&engage_key) {
            Some(Ok(picked)) => {
                let sampled = sample(&picked);
                self.note(
                    Source::System,
                    &format!(
                        "{username} engages {}: {} of {} ({:.2})",
                        enemies[0].0, sampled, picked.choice, picked.confidence
                    ),
                    json!({
                        "picked": picked.choice,
                        "sampled": sampled,
                        "probabilities": picked.probabilities,
                        "record": picked.record,
                    }),
                );
                let _ = self.say(
                    agent,
                    &format!(
                        "{username}: {} picked {} of {} ({:.2})",
                        self.decision_model,
                        sampled,
                        picked.choice,
                        picked.confidence
                    ),
                );
                Some(sampled)
            }
            _ => None,
        };
        let action = if let Some(action) = answered {
            action
        } else {
            if asked.elapsed() >= cooldown {
                self.ask(
                    &engage_key,
                    &format!("{username} spots {}", enemies[0].0),
                    "An enrolled enemy is inside your aggro range while you work. What do you do?",
                    json!({
                        "agent": username,
                        "guild": guild,
                        "temperament": temperament.as_str(),
                        "aggression": aggression,
                        "doing": context,
                        "enemies": enemies.iter().map(|(name, distance, _)| json!({
                            "username": name, "distance": distance,
                        })).collect::<Vec<_>>(),
                    }),
                    vec![
                        ("attack".to_string(), "hunt the nearest enemy now".to_string()),
                        ("keep_working".to_string(), "ignore them and keep working".to_string()),
                        ("retreat".to_string(), "fall back toward your camp".to_string()),
                    ],
                );
                *asked = Instant::now();
            }
            // Until the door answers the member is itself —
            // temperament sets the style, and enough aggression
            // overrides it: furious workers still swing, timid
            // berserkers still back off.
            if aggression >= 0.8 {
                "attack".to_string()
            } else if aggression <= 0.2 {
                "retreat".to_string()
            } else {
                temperament.fallback().to_string()
            }
        };
        match action.as_str() {
            "retreat" => {
                if let Some([x, _, z]) = agent.member.camp {
                    let (x, z) = (x.round() as i64, z.round() as i64);
                    let _ = self.call(
                        agent,
                        "goto",
                        json!({"x": x, "z": z, "seconds": 12}),
                        Duration::from_secs(20),
                        "fall back to camp",
                    );
                }
            }
            "attack" => {
                // Several enemies in range — the door picks the hunt.
                // Same fire-and-forget shape as the weapon ask: post
                // this sighting, use whatever answer has landed, and
                // take the nearest when the door has not spoken yet.
                let target = if enemies.len() > 1 {
                    let target_key = format!("{username}:target");
                    self.ask(
                        &target_key,
                        &format!("{username} picks a target"),
                        "Several enrolled enemies are in range. Which do you hunt?",
                        json!({
                            "agent": username,
                            "enemies": enemies.iter().map(|(name, distance, _)| json!({
                                "username": name, "distance": distance,
                            })).collect::<Vec<_>>(),
                        }),
                        enemies
                            .iter()
                            .map(|(name, distance, _)| {
                                (name.clone(), format!("{distance:.0} blocks away"))
                            })
                            .collect(),
                    );
                    match self.take_answer(&target_key) {
                        Some(Ok(picked)) => {
                            let sampled = sample(&picked);
                            self.note(
                                Source::System,
                                &format!(
                                    "{username} targets: {} of {} ({:.2})",
                                    sampled, picked.choice, picked.confidence
                                ),
                                json!({
                                    "picked": picked.choice,
                                    "sampled": sampled,
                                    "probabilities": picked.probabilities,
                                    "record": picked.record,
                                }),
                            );
                            let _ = self.say(
                                agent,
                                &format!(
                                    "{username}: {} picked {} of {} ({:.2})",
                                    self.decision_model,
                                    sampled,
                                    picked.choice,
                                    picked.confidence
                                ),
                            );
                            if enemies.iter().any(|(name, _, _)| *name == sampled) {
                                sampled
                            } else {
                                enemies[0].0.clone()
                            }
                        }
                        _ => enemies[0].0.clone(),
                    }
                } else {
                    enemies[0].0.clone()
                };
                if engaged.insert(target.clone()) {
                    let _ = self.say(agent, &format!("{username} hunts {target}!"));
                }
                let distance = enemies
                    .iter()
                    .find(|(name, _, _)| *name == target)
                    .map(|(_, distance, _)| *distance)
                    .unwrap_or(0.0);
                self.hunt(agent, &target, distance, combat)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Run one enemy down: the door picks the weapon — the sword for a
    /// chase, the bow at range — then the bridge fights until the
    /// target falls or escapes. A kill is XP, never credits.
    fn hunt(
        &self,
        agent: &mut AgentHandle<'_>,
        target: &str,
        distance: f64,
        combat: &CombatSection,
    ) -> Result<()> {
        let username = agent.member.username.clone();
        let guild = agent.member.guild.clone();
        let temperament = agent.member.temperament();
        // Weapon choice is fire-and-forget like the engagement ask:
        // post this hunt's question, arm with whatever answer has
        // landed from an earlier one, and run on temperament when the
        // door has not spoken yet. A hunt never waits on the model.
        let weapon_key = format!("{username}:weapon");
        self.ask(
            &weapon_key,
            &format!("{username} arms against {target}"),
            "You are hunting an enemy. Which weapon?",
            json!({
                "agent": username,
                "target": target,
                "distance": distance,
                "temperament": temperament.as_str(),
                "aggression": agent.member.aggression(),
            }),
            vec![
                ("sword".to_string(), "close the distance and cut".to_string()),
                ("bow".to_string(), "loose arrows from range".to_string()),
            ],
        );
        let weapon = match self.take_answer(&weapon_key) {
            Some(Ok(picked)) => {
                let sampled = sample(&picked);
                self.note(
                    Source::System,
                    &format!(
                        "{username} arms: {} of {} ({:.2})",
                        sampled, picked.choice, picked.confidence
                    ),
                    json!({
                        "picked": picked.choice,
                        "sampled": sampled,
                        "probabilities": picked.probabilities,
                        "record": picked.record,
                    }),
                );
                let _ = self.say(
                    agent,
                    &format!(
                        "{username}: {} picked {} of {} ({:.2})",
                        self.decision_model,
                        sampled,
                        picked.choice,
                        picked.confidence
                    ),
                );
                sampled
            }
            _ => temperament.weapon(distance).to_string(),
        };
        let (op, args, seconds) = if weapon == "bow" {
            (
                "shoot",
                json!({"username": target, "max_shots": 4}),
                60u64,
            )
        } else {
            (
                "attack",
                json!({"username": target, "max_swings": 10}),
                90u64,
            )
        };
        let fought = self.call(agent, op, args, Duration::from_secs(seconds), "fight");
        let killed = fought
            .as_ref()
            .ok()
            .and_then(|value| value.get("killed"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if killed {
            *self
                .kills
                .lock()
                .expect("the kill count lock is not poisoned")
                .entry(guild.clone())
                .or_default() += 1;
            let line = format!("{target} fell to {username} on the contested claim.");
            let _ = self.say(agent, &line);
            let _ = self.channel_chat(agent, &line);
            self.ledger
                .lock()
                .expect("the ledger lock is not poisoned")
                .xp(&guild, &username, "skirmish", combat.kill_xp)?;
        }
        Ok(())
    }

    /// Whether a door is wired for this episode — the ask queue only
    /// exists when the door thread does.
    fn has_door(&self) -> bool {
        self.asks
            .lock()
            .expect("the ask queue lock is not poisoned")
            .is_some()
    }

    /// Queue a question for the door thread and keep moving — the
    /// answer lands in `answers` under `key`. A key already in flight
    /// is not re-posted.
    fn ask(
        &self,
        key: &str,
        label: &str,
        prompt: &str,
        state: Value,
        options: Vec<(String, String)>,
    ) {
        let Some(sender) = self
            .asks
            .lock()
            .expect("the ask queue lock is not poisoned")
            .clone()
        else {
            return;
        };
        if !self
            .pending
            .lock()
            .expect("the pending lock is not poisoned")
            .insert(key.to_string())
        {
            return;
        }
        let _ = sender.send(Ask {
            key: key.to_string(),
            label: label.to_string(),
            prompt: prompt.to_string(),
            state,
            options,
        });
    }

    /// An answered question, removed — `Some(Err)` is a refused call,
    /// still an answer.
    fn take_answer(&self, key: &str) -> Option<std::result::Result<Picked, String>> {
        self.answers
            .lock()
            .expect("the answers lock is not poisoned")
            .remove(key)
    }

    /// Walk a fighter onto the combat ground.
    fn to_ground(&self, agent: &mut AgentHandle<'_>, ground: [i32; 3]) {
        // Every member patrols its own patch of the ground — hashing
        // the username spreads the targets so patrols don't stack on
        // one block.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        agent.member.username.hash(&mut hasher);
        let seed = hasher.finish();
        let (x, z) = (
            ground[0] + (seed % 7) as i32 - 3,
            ground[2] + ((seed / 7) % 7) as i32 - 3,
        );
        let _ = self.call(
            agent,
            "goto",
            json!({"x": x, "y": ground[1], "z": z,
                   "radius": 1.2, "seconds": 12}),
            Duration::from_secs(20),
            "join the fight",
        );
    }

    /// Hands the work kit to one player: a diamond pickaxe so ore
    /// actually drops, a diamond sword so nobody punches bare-handed,
    /// and a bow with arrows for range. The `kitted` set makes it once
    /// per episode — the same join line reaches several bots' event
    /// streams.
    fn kit(
        server: &Mutex<Server>,
        kitted: &Mutex<std::collections::HashSet<String>>,
        player: &str,
    ) -> Result<()> {
        if !kitted
            .lock()
            .expect("the kit set lock is not poisoned")
            .insert(player.to_string())
        {
            return Ok(());
        }
        let mut server = server.lock().expect("the server lock is not poisoned");
        server.command(&format!("give {player} minecraft:diamond_pickaxe"))?;
        server.command(&format!("give {player} minecraft:diamond_sword"))?;
        server.command(&format!("give {player} minecraft:bow"))?;
        server.command(&format!("give {player} minecraft:arrow 64"))
    }

    /// The decision door orders contested work: one `choice` question
    /// per guild, whose answer picks the first deposit to dig. The
    /// rest keep manifest order. No door means manifest order — the
    /// call is evidence, not a prerequisite.
    /// Post the contested-order question — `await_order` collects it
    /// once guild work has run, so the model's latency overlaps
    /// digging rather than blocking it.
    fn ask_order(&self, agent: &mut AgentHandle<'_>, deposits: &[&Deposit]) -> Option<String> {
        let guild = agent.member.guild.clone();
        let balance = self
            .ledger
            .lock()
            .expect("the ledger lock is not poisoned")
            .balance(&guild);
        let options: Vec<(String, String)> = deposits
            .iter()
            .map(|deposit| {
                (
                    deposit.id.clone(),
                    format!(
                        "{} blocks of {} at {} credits each",
                        deposit.blocks.len(),
                        deposit.kind,
                        deposit.award
                    ),
                )
            })
            .collect();
        let state = serde_json::json!({
            "guild": guild,
            "balance": {
                "available": balance.available,
                "awarded": balance.awarded,
            },
            "contested_deposits": deposits.iter().map(|deposit| {
                serde_json::json!({
                    "id": deposit.id,
                    "kind": deposit.kind,
                    "blocks": deposit.blocks.len(),
                    "award_per_block": deposit.award,
                })
            }).collect::<Vec<_>>(),
        });
        let key = format!("{guild}:contested-order");
        self.ask(
            &key,
            &format!("{guild} contested order"),
            "Which contested deposit should this guild work first for the best value?",
            state,
            options,
        );
        Some(key)
    }

    /// Collect the contested-order answer and sort by it. A door that
    /// never answered leaves the manifest's order — the deposit list
    /// goes through either way.
    fn await_order<'d>(
        &self,
        agent: &mut AgentHandle<'_>,
        mut deposits: Vec<&'d Deposit>,
        key: &str,
    ) -> Result<Vec<&'d Deposit>> {
        let guild = agent.member.guild.clone();
        // The ask went out when work started; whatever has landed by
        // now is the answer. Nothing is waited on — a slow door just
        // leaves the manifest's order.
        let Some(Ok(picked)) = self.take_answer(key) else {
            return Ok(deposits);
        };
        self.note(
            Source::System,
            &format!(
                "{guild} decision: {} first (confidence {:.2})",
                picked.choice, picked.confidence
            ),
            serde_json::json!({
                "picked": picked.choice,
                "confidence": picked.confidence,
                "probabilities": picked.probabilities,
                "record": picked.record,
            }),
        );
        let _ = self.say(agent, &format!("{} says {} first.", self.decision_model, picked.choice));
        deposits.sort_by_key(|deposit| usize::from(deposit.id != picked.choice));
        Ok(deposits)
    }

    /// A bot says something in the world, tagged with its username so
    /// overlapping guilds stay readable.
    fn say(&self, agent: &mut AgentHandle<'_>, text: &str) -> Result<()> {
        let mut chars = text.chars();
        let cut: String = chars.by_ref().take(240).collect();
        let text = if chars.next().is_some() {
            format!("{cut}…")
        } else {
            cut
        };
        (self.progress)(&format!("{}: {text}", agent.member.username));
        self.call(
            agent,
            "say",
            json!({"text": text}),
            Duration::from_secs(30),
            "say",
        )?;
        Ok(())
    }

    /// One kind-9 line into the member's own guild channel. A refused
    /// post is recorded and returned as an error — a member who cannot
    /// speak in its own channel is a broken binding.
    fn channel_chat(&self, agent: &mut AgentHandle<'_>, text: &str) -> Result<()> {
        let signer = agent.signer.clone();
        let group = agent.member.guild.clone();
        let username = agent.member.username.clone();
        let Some(channel) = agent.channel.as_mut() else {
            return Ok(());
        };
        let verdict = channel.chat(&signer, &group, text)?;
        self.note(
            Source::System,
            &format!("{username} to {group}: {}", describe_verdict(&verdict)),
            json!({"text": text, "accepted": verdict.accepted}),
        );
        if !verdict.accepted {
            return Err(Error::episode(format!(
                "{username} was refused in {group}: {}",
                verdict.message
            )));
        }
        Ok(())
    }

    /// One bridge exchange on an agent's bridge: recorded as a call
    /// step, then that bridge's events recorded as steps of their own.
    fn call(
        &self,
        agent: &mut AgentHandle<'_>,
        op: &str,
        args: Value,
        deadline: Duration,
        task: &str,
    ) -> Result<Value> {
        self.bounded()?;
        let name = agent.member.username.clone();
        let started = Instant::now();
        let outcome = agent.bridge.call(op, args.clone(), deadline);
        let milliseconds = started.elapsed().as_millis() as u64;
        let action = self.actions.fetch_add(1, Ordering::Relaxed) + 1;
        let (output, outcome_result) = match &outcome {
            Ok(value) => (value.to_string(), Outcome::Completed),
            Err(error) => (error.to_string(), Outcome::Failed),
        };
        self.log
            .lock()
            .expect("the trace lock is not poisoned")
            .append(
                &Step::called(atif::document::Call {
                    id: format!("a{action}"),
                    name: format!("mc-bridge:{op}"),
                    arguments: args,
                    output,
                    outcome: outcome_result,
                    milliseconds,
                    purpose: Some(format!("{name}: {task}")),
                    extra: Map::new(),
                })
                .by(concat!("mc-bridge/", env!("CARGO_PKG_VERSION"))),
            )?;
        let events = agent.bridge.drain_events();
        for event in events {
            self.record_event(&name, &event)?;
        }
        outcome
    }

    /// A lifecycle or observation step.
    fn note(&self, source: Source, message: &str, extra: Value) {
        let _ = self
            .log
            .lock()
            .expect("the trace lock is not poisoned")
            .append(&Step::said(source, message).noting("detail", extra));
        (self.progress)(message);
    }

    /// What one bot reported on its own.
    fn record_event(&self, agent: &str, event: &Event) -> Result<()> {
        let message = match event.event.as_str() {
            "chat" => format!("chat: {}", event.text("text").unwrap_or_default()),
            "feedback" => format!("{agent}: {}", event.text("text").unwrap_or_default()),
            other => format!(
                "{agent} event {other}: {}",
                Value::from(event.fields.clone())
            ),
        };
        self.log
            .lock()
            .expect("the trace lock is not poisoned")
            .append(&Step::said(Source::System, &message))?;
        (self.progress)(&message);
        Ok(())
    }
}

/// A verdict as a progress line: `accepted`, or the refusal message.
fn describe_verdict(verdict: &guild::Verdict) -> String {
    if verdict.accepted {
        "accepted".to_string()
    } else {
        format!("refused: {}", verdict.message)
    }
}

/// A weighted pick over the model's own probabilities — the answer
/// stays a model judgment, just not always the argmax. Falls back to
/// the argmax when the map is missing or empty.
fn sample(picked: &Picked) -> String {
    let roll = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (std::process::id(), Instant::now()).hash(&mut hasher);
        (hasher.finish() as f64) / (u64::MAX as f64)
    };
    let mut cumulative = 0.0;
    let mut last = picked.choice.clone();
    for (name, probability) in &picked.probabilities {
        let Some(probability) = probability.as_f64() else {
            continue;
        };
        cumulative += probability;
        last = name.clone();
        if roll <= cumulative {
            return name.clone();
        }
    }
    last
}

/// The observer key — an identity with no membership anywhere, used to
/// prove guild reads are public.
fn observer_secret() -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b"voyager-relay-key:observer")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
