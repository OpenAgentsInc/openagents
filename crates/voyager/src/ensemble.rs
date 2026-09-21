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

use std::time::{Duration, Instant};

use atif::document::{Outcome, Session, Source, Step};
use nostr::domain::RelaySigner;
use serde_json::{Map, Value, json};

use crate::bridge::{Bridge, Event};
use crate::decide::Door;
use crate::episode::{Plan, Report, TaskResult};
use crate::error::{Error, Result};
use crate::guild::{self, Channel};
use crate::keys;
use crate::ledger::{Awarded, Ledger};
use crate::relay::Relay;
use crate::server::Server;
use crate::world::{Deposit, Member, World};

/// Seconds one member's `mine` call may run.
const MINE_SECONDS: u64 = 90;
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

/// A runner for a world that enrolls agents.
struct Ensemble<'a> {
    world: &'a World,
    plan: &'a Plan,
    server: Server,
    relay: Option<Relay>,
    door: Option<Door>,
    agents: Vec<AgentHandle<'a>>,
    ledger: Ledger,
    log: atif::log::Log,
    run_dir: std::path::PathBuf,
    trace: std::path::PathBuf,
    actions: usize,
    started: Instant,
    progress: &'a dyn Fn(&str),
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
pub fn run_ensemble(world: &World, plan: &Plan, progress: impl Fn(&str)) -> Result<Report> {
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
            door = Some(Door::local(url, model, run_dir.join("decisions"))?);
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
        world,
        plan,
        server,
        relay,
        door,
        agents: Vec::new(),
        ledger,
        log,
        run_dir,
        trace,
        actions: 0,
        started: Instant::now(),
        progress: &progress,
    };
    ensemble.note(Source::System, "server ready", json!({"port": plan.port}));

    let result = ensemble.episode();
    ensemble.note(Source::System, "episode over", json!({}));
    for handle in &mut ensemble.agents {
        let _ = handle.bridge.shutdown();
    }
    let _ = ensemble.server.stop();
    let _ = ensemble.log.finish(if result.is_ok() {
        "ended"
    } else {
        "interrupted"
    });
    result
}

impl Ensemble<'_> {
    /// The bounded guild loop: everyone joins, goes to camp, digs what
    /// their guild owns, then both guilds swing at the contested
    /// deposit — the ledger's dedupe decides who it paid.
    fn episode(&mut self) -> Result<Report> {
        let mut tasks = Vec::new();
        let address = format!("127.0.0.1:{}", self.plan.port);

        // Grant each guild its starting balance, if the manifest
        // declares one — recorded like any other credit.
        let mut granted = std::collections::BTreeSet::new();
        if self.world.economy.starting_credits > 0 {
            for member in &self.world.agents {
                if granted.insert(member.guild.clone()) {
                    self.ledger.award(
                        &member.guild,
                        "genesis",
                        "starting",
                        [0, 0, 0],
                        self.world.economy.starting_credits,
                    )?;
                }
            }
        }

        // Join every member. A join failure stops the episode — an
        // enrolled agent that cannot enter is a broken binding, not a
        // skip.
        for member in &self.world.agents {
            self.bounded()?;
            (self.progress)(&format!("{} of {} joining", member.username, member.guild));
            let bridge = Bridge::start(&self.plan.bridge)?;
            let signer = keys::agent_signer(&member.username)?;
            self.agents.push(AgentHandle {
                member,
                bridge,
                signer,
                channel: None,
            });
            let index = self.agents.len() - 1;
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
                    "{} of guild {} reporting for work.",
                    member.username, member.guild
                ),
            )?;
            self.channel_open(index)?;
        }
        self.check_membership_boundary(&mut tasks)?;

        // To camp, then to work. Each member walks to its manifest camp
        // and digs its guild's deposits; the last leg sends each guild's
        // first member at the contested deposit so the dedupe is
        // exercised, not just believed.
        for index in 0..self.agents.len() {
            self.bounded()?;
            self.task_camp(index, &mut tasks)?;
            self.task_mine(index, false, &mut tasks)?;
        }
        let contested: Vec<usize> = {
            let mut seen = std::collections::HashSet::new();
            let mut picks = Vec::new();
            for (index, handle) in self.agents.iter().enumerate() {
                if seen.insert(handle.member.guild.clone()) {
                    picks.push(index);
                }
            }
            picks
        };
        for index in contested {
            self.bounded()?;
            self.task_mine(index, true, &mut tasks)?;
        }

        // Report: every guild hears its own close, and an outside reader
        // confirms the channels stayed public for reading.
        let balances = crate::ledger::describe(&self.ledger);
        self.note(Source::System, "ledger", balances.clone());
        for index in 0..self.agents.len() {
            let guild = self.agents[index].member.guild.clone();
            let balance = self.ledger.balance(&guild);
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

    /// Walk a member to its manifest camp. No camp means the task is a
    /// pass — the member works from spawn.
    fn task_camp(&mut self, index: usize, tasks: &mut Vec<TaskResult>) -> Result<()> {
        let Some([x, _, z]) = self.agents[index].member.camp else {
            return Ok(());
        };
        let username = self.agents[index].member.username.clone();
        let (x, z) = (x.round() as i64, z.round() as i64);
        self.bounded()?;
        let result = self.call(
            index,
            "goto",
            json!({"x": x, "z": z, "seconds": 45}),
            Duration::from_secs(65),
            "walk to camp",
        );
        match result {
            Ok(value) => {
                let to = &value["to"];
                tasks.push(TaskResult {
                    task: format!("{username}: reach camp"),
                    ok: true,
                    detail: format!("at ({}, {}, {})", to["x"], to["y"], to["z"]),
                });
            }
            Err(Error::Refused { .. }) => {
                tasks.push(TaskResult {
                    task: format!("{username}: reach camp"),
                    ok: false,
                    detail: "the walk did not complete".to_string(),
                });
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// Dig a deposit. `contested` picks every deposit without a guild;
    /// otherwise only the member's own guild's deposits. Awards land in
    /// the ledger for dug positions the manifest registered.
    fn task_mine(
        &mut self,
        index: usize,
        contested: bool,
        tasks: &mut Vec<TaskResult>,
    ) -> Result<()> {
        let guild = self.agents[index].member.guild.clone();
        let username = self.agents[index].member.username.clone();
        let deposits: Vec<&Deposit> = self
            .world
            .deposits
            .iter()
            .filter(|deposit| {
                if contested {
                    deposit.guild.is_none()
                } else {
                    deposit.guild.as_deref() == Some(guild.as_str())
                }
            })
            .collect();
        // A decision door, when the world names one, orders contested
        // work — the answer picks which deposit this member digs first.
        let mut deposits = deposits;
        if contested && deposits.len() > 1 && self.door.is_some() {
            deposits = self.decide_order(index, deposits)?;
        }
        for deposit in deposits {
            self.bounded()?;
            let want = deposit.blocks.len().min(64) as u32;
            let names: Vec<&str> = if deposit.kind.is_empty() {
                Vec::new()
            } else {
                vec![deposit.kind.as_str()]
            };
            self.say(index, &format!("{username}: digging {} now.", deposit.id))?;
            let result = self.call(
                index,
                "mine",
                json!({
                    "names": names,
                    "positions": deposit.blocks,
                    "count": want,
                    "seconds": MINE_SECONDS,
                }),
                Duration::from_secs(MINE_SECONDS) + CALL_SLACK,
                &format!("mine {}", deposit.id),
            );
            let result = match result {
                Ok(value) => value,
                Err(Error::Refused { code, message }) => {
                    tasks.push(TaskResult {
                        task: format!("{username}: mine {}", deposit.id),
                        ok: false,
                        detail: format!("{code}: {message}"),
                    });
                    continue;
                }
                Err(error) => return Err(error),
            };
            // Award only what the helper reports dug and the manifest
            // registers — the ledger dedupe covers contested overlap.
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
                        .award(&guild, &username, &hit.id, pos, hit.award)?
                    {
                        Awarded::Recorded => awarded += 1,
                        Awarded::Duplicate => duplicates += 1,
                    }
                }
            }
            let mined = result.get("mined").and_then(Value::as_i64).unwrap_or(0);
            let detail =
                format!("dug {mined}, earned {awarded} blocks, {duplicates} already claimed");
            self.note(
                Source::System,
                &format!("{username} at {}: {detail}", deposit.id),
                json!({"deposit": deposit.id, "dug": result.get("dug").cloned()}),
            );
            if awarded > 0 {
                self.channel_chat(
                    index,
                    &format!("{username} dug {awarded} at {} for {guild}.", deposit.id),
                )?;
            }
            tasks.push(TaskResult {
                task: format!("{username}: mine {}", deposit.id),
                ok: awarded > 0,
                detail,
            });
        }
        Ok(())
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

    /// One kind-9 line into the member's own guild channel. A refused
    /// post is recorded and returned as an error — a member who cannot
    /// speak in its own channel is a broken binding.
    fn channel_chat(&mut self, index: usize, text: &str) -> Result<()> {
        let signer = self.agents[index].signer.clone();
        let group = self.agents[index].member.guild.clone();
        let username = self.agents[index].member.username.clone();
        let Some(channel) = self.agents[index].channel.as_mut() else {
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

    /// The membership boundary, exercised once: a ferro member writes
    /// to lumen's channel and the relay must refuse it. A pass is a
    /// recorded `restricted:` verdict, not silence.
    fn check_membership_boundary(&mut self, tasks: &mut Vec<TaskResult>) -> Result<()> {
        let Some(_relay) = &self.relay else {
            return Ok(());
        };
        let mut pair = None;
        'find: for (index, handle) in self.agents.iter().enumerate() {
            for other in &self.world.agents {
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

    /// The decision door orders contested work: one `choice` question
    /// per guild, whose answer picks the first deposit to dig. The
    /// rest keep manifest order. No door means manifest order — the
    /// call is evidence, not a prerequisite.
    fn decide_order<'d>(
        &mut self,
        index: usize,
        mut deposits: Vec<&'d Deposit>,
    ) -> Result<Vec<&'d Deposit>> {
        let Some(door) = &mut self.door else {
            return Ok(deposits);
        };
        let guild = self.agents[index].member.guild.clone();
        let balance = self.ledger.balance(&guild);
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
        let picked = door.choose(
            state,
            "Which contested deposit should this guild work first for the best value?",
            &options,
            &format!("{guild} contested order"),
        )?;
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
        deposits.sort_by_key(|deposit| usize::from(deposit.id != picked.choice));
        Ok(deposits)
    }

    /// A bot says something in the world, tagged with its username so
    /// overlapping guilds stay readable.
    fn say(&mut self, index: usize, text: &str) -> Result<()> {
        let mut chars = text.chars();
        let cut: String = chars.by_ref().take(240).collect();
        let text = if chars.next().is_some() {
            format!("{cut}…")
        } else {
            cut
        };
        (self.progress)(&format!("{}: {text}", self.agents[index].member.username));
        self.call(
            index,
            "say",
            json!({"text": text}),
            Duration::from_secs(30),
            "say",
        )?;
        Ok(())
    }

    /// One bridge exchange on agent `index`'s bridge: recorded as a call
    /// step, then that bridge's events recorded as steps of their own.
    fn call(
        &mut self,
        index: usize,
        op: &str,
        args: Value,
        deadline: Duration,
        task: &str,
    ) -> Result<Value> {
        self.bounded()?;
        let agent = self.agents[index].member.username.clone();
        let started = Instant::now();
        let outcome = self.agents[index].bridge.call(op, args.clone(), deadline);
        let milliseconds = started.elapsed().as_millis() as u64;
        self.actions += 1;
        let (output, outcome_result) = match &outcome {
            Ok(value) => (value.to_string(), Outcome::Completed),
            Err(error) => (error.to_string(), Outcome::Failed),
        };
        self.log.append(
            &Step::called(atif::document::Call {
                id: format!("a{}", self.actions),
                name: format!("mc-bridge:{op}"),
                arguments: args,
                output,
                outcome: outcome_result,
                milliseconds,
                purpose: Some(format!("{agent}: {task}")),
                extra: Map::new(),
            })
            .by(concat!("mc-bridge/", env!("CARGO_PKG_VERSION"))),
        )?;
        let events = self.agents[index].bridge.drain_events();
        for event in events {
            self.record_event(&agent, &event)?;
        }
        outcome
    }

    /// A lifecycle or observation step.
    fn note(&mut self, source: Source, message: &str, extra: Value) {
        let _ = self
            .log
            .append(&Step::said(source, message).noting("detail", extra));
        (self.progress)(message);
    }

    /// What one bot reported on its own.
    fn record_event(&mut self, agent: &str, event: &Event) -> Result<()> {
        let message = match event.event.as_str() {
            "chat" => format!("chat: {}", event.text("text").unwrap_or_default()),
            "feedback" => format!("{agent}: {}", event.text("text").unwrap_or_default()),
            other => format!(
                "{agent} event {other}: {}",
                Value::from(event.fields.clone())
            ),
        };
        self.log.append(&Step::said(Source::System, &message))?;
        (self.progress)(&message);
        Ok(())
    }

    /// The episode bounds, checked before every exchange.
    fn bounded(&mut self) -> Result<()> {
        if self.actions >= self.world.episode.max_actions as usize {
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

    /// What the episode leaves behind.
    fn report(&self, tasks: Vec<TaskResult>) -> Report {
        Report {
            world: self.world.name.clone(),
            digest: self.world.digest.clone(),
            tasks,
            actions: self.actions,
            run_dir: self.run_dir.clone(),
            trace: self.trace.clone(),
        }
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

/// The observer key — an identity with no membership anywhere, used to
/// prove guild reads are public.
fn observer_secret() -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b"voyager-relay-key:observer")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
