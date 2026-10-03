//! A bounded reader and one writer keep packet framing intact across quiet periods.
use crate::{
    emit,
    state::{Entity, State, distance},
};
use anyhow::{Result, bail, ensure};
use benilla_protocol::messages::{self, CharCreateReq, opcode};
use benilla_protocol::{Poll, SessionEvent, WorldSession, WorldWriter};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct Motion {
    started: Instant,
    duration: Duration,
    path: Vec<[f32; 3]>,
    spline: u32,
}

pub struct Live {
    pub writer: WorldWriter,
    pub state: State,
    pub setup: bool,
    pub setup_feedback: Vec<String>,
    join_args: Value,
    events: mpsc::Receiver<Result<Vec<SessionEvent>>>,
    closed: Arc<AtomicBool>,
    last_ping: Instant,
    ping: u32,
    disconnected: bool,
    closing: bool,
    pub target: Option<u64>,
    pub loot_window: Option<(u64, u32, Vec<messages::LootItem>)>,
    pub vendor_seen: Option<u64>,
    pub loot_error: Option<u8>,
    pub qualities: std::collections::BTreeMap<u32, u32>,
    pub cast_result: Option<(u32, bool, Option<u8>)>,
    motions: std::collections::BTreeMap<u64, Motion>,
    kill_lives: KillLives,
}

pub fn credentials(path: &std::path::Path, account: &str) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16384,
        "invalid credential file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "credential file must be private (0600)"
        );
    }
    let accounts: Vec<Value> = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|_| anyhow::anyhow!("invalid credential file"))?;
    accounts
        .iter()
        .find(|a| {
            a["account"]
                .as_str()
                .is_some_and(|u| u.eq_ignore_ascii_case(account))
        })
        .and_then(|a| a["password"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("account not found in credential file"))
}

impl Live {
    pub fn join(args: &Value, closed: Arc<AtomicBool>) -> Result<Self> {
        let account = args["account"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("account required"))?;
        let setup = std::env::var("WOW_BRIDGE_SETUP").as_deref() == Ok("1");
        ensure!(
            setup || !account.eq_ignore_ascii_case("GYMSETUP"),
            "setup account requires the setup helper"
        );
        let file = std::env::var_os("VOYAGER_WOW_ACCOUNTS").ok_or_else(|| {
            anyhow::anyhow!("VOYAGER_WOW_ACCOUNTS must name a private credential file")
        })?;
        let password = credentials(std::path::Path::new(&file), account)?;
        let auth = args["auth"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("auth address required"))?;
        let logon = benilla_protocol::logon(auth, account, &password)?;
        let address = args["world"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| logon.realms.first().map(|r| r.address.clone()))
            .ok_or_else(|| anyhow::anyhow!("no realm available"))?;
        let mut session = WorldSession::connect(address, account, logon.session_key)?;
        let name = args["character"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("character required"))?;
        ensure!(
            name.len() >= 2 && name.len() <= 12 && name.bytes().all(|b| b.is_ascii_alphabetic()),
            "character name must contain 2–12 letters"
        );
        let mut roster = session.char_enum()?;
        if args["reset"].as_bool() == Some(true) {
            if let Some(c) = roster.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
                ensure!(
                    session.delete_character(c.guid)? == messages::CHAR_DELETE_SUCCESS,
                    "character reset refused"
                );
                let end = Instant::now() + Duration::from_secs(15);
                loop {
                    roster = session.char_enum()?;
                    if !roster.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
                        break;
                    }
                    ensure!(
                        Instant::now() < end,
                        "character deletion has not reached the roster"
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        if !roster.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
            ensure!(roster.len() < 10, "account character limit reached");
            let create = &args["create"];
            let race = create["race"].as_u64().unwrap_or(1);
            let class = create["class"].as_u64().unwrap_or(1);
            ensure!(
                (1..=8).contains(&race) && (1..=9).contains(&class),
                "invalid character template"
            );
            let req = CharCreateReq {
                name: name.to_owned(),
                race: race as u8,
                class: class as u8,
                gender: 0,
                skin: 0,
                face: 0,
                hair_style: 0,
                hair_color: 0,
                facial_hair: 0,
            };
            ensure!(
                session.create_character(&req)? == messages::CHAR_CREATE_SUCCESS,
                "character creation refused"
            );
            let end = Instant::now() + Duration::from_secs(15);
            loop {
                roster = session.char_enum()?;
                if roster.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
                    break;
                }
                ensure!(
                    Instant::now() < end,
                    "created character has not reached the roster"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let character = roster
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("character not found"))?;
        let state = State::new(character);
        session.player_login(character.guid)?;
        session.set_active_mover(character.guid)?;
        let (mut reader, writer) = session.into_split()?;
        let (tx, events) = mpsc::sync_channel(64);
        std::thread::spawn(move || {
            loop {
                let value = match reader.poll() {
                    Ok(Poll::Events { events, .. }) => Ok(events),
                    Ok(Poll::Skipped { .. }) => continue,
                    Err(e) => Err(e),
                };
                let failed = value.is_err();
                if tx.send(value).is_err() || failed {
                    break;
                }
            }
        });
        let mut live = Self {
            writer,
            state,
            setup,
            setup_feedback: Vec::new(),
            join_args: args.clone(),
            events,
            closed,
            last_ping: Instant::now(),
            ping: 0,
            disconnected: false,
            closing: false,
            target: None,
            loot_window: None,
            loot_error: None,
            vendor_seen: None,
            cast_result: None,
            qualities: Default::default(),
            kill_lives: Default::default(),
            motions: Default::default(),
        };
        live.pump(Duration::from_secs(2))?;
        ensure!(live.state.fields().is_some(), "player state did not arrive");
        emit(
            json!({"event":"spawn","name":name,"map":live.state.map,"position":live.state.position}),
        );
        Ok(live)
    }

    fn apply(&mut self, event: SessionEvent) -> Result<()> {
        match event {
            SessionEvent::ObjectCreate {
                guid,
                kind,
                position,
                orientation,
                fields,
                speeds,
                ..
            } => {
                ensure!(
                    self.state.entities.len() < 4096 || self.state.entities.contains_key(&guid),
                    "entity bound exceeded"
                );
                if fields.unit_health().is_some_and(|h| h > 0) {
                    self.kill_lives.alive(guid);
                }
                if guid == self.state.guid {
                    self.state.position = position;
                    self.state.orientation = orientation;
                    if let Some(s) = speeds {
                        self.state.speed = s.run;
                    }
                }
                self.state.entities.insert(
                    guid,
                    Entity {
                        position,
                        orientation,
                        fields,
                        kind: format!("{kind:?}"),
                    },
                );
            }
            SessionEvent::ItemCreate { guid, fields, .. } => {
                ensure!(
                    self.state.items.len() < 1024 || self.state.items.contains_key(&guid),
                    "item bound exceeded"
                );
                self.state.items.insert(guid, fields);
            }
            SessionEvent::ObjectValues { guid, fields } => {
                if let Some(e) = self.state.entities.get_mut(&guid) {
                    let was_alive = e.fields.unit_health().unwrap_or(0) > 0;
                    e.fields.merge(fields);
                    if !was_alive && e.fields.unit_health().is_some_and(|h| h > 0) {
                        self.kill_lives.alive(guid);
                    }
                    if guid == self.state.guid && was_alive && e.fields.unit_health() == Some(0) {
                        self.state.deaths += 1;
                        emit(json!({"event":"death"}));
                    }
                } else if let Some(item) = self.state.items.get_mut(&guid) {
                    item.merge(fields);
                }
            }
            SessionEvent::ObjectMove {
                guid,
                position,
                orientation,
            }
            | SessionEvent::UnitMove {
                guid,
                position,
                orientation,
                ..
            } => {
                if let Some(e) = self.state.entities.get_mut(&guid) {
                    e.position = position;
                    e.orientation = orientation;
                }
                if guid == self.state.guid {
                    self.state.position = position;
                    self.state.orientation = orientation;
                }
            }
            SessionEvent::MonsterMove {
                guid,
                start,
                path,
                duration_ms,
                spline_id,
                stop,
                transport,
                ..
            } if transport.is_none() => {
                if stop || path.is_empty() || duration_ms == 0 {
                    self.motions.remove(&guid);
                    if let Some(e) = self.state.entities.get_mut(&guid) {
                        e.position = start;
                    }
                } else if self.motions.len() < 4096 {
                    let mut points = vec![start];
                    points.extend(path);
                    self.motions.insert(
                        guid,
                        Motion {
                            started: Instant::now(),
                            duration: Duration::from_millis(u64::from(duration_ms)),
                            path: points,
                            spline: spline_id,
                        },
                    );
                }
            }
            SessionEvent::ObjectsRemoved(guids) => {
                for g in guids {
                    self.motions.remove(&g);
                    self.motions.remove(&g);
                    self.state.entities.remove(&g);
                    self.state.items.remove(&g);
                }
            }
            SessionEvent::ObjectDestroyed(g) => {
                self.motions.remove(&g);
                self.state.entities.remove(&g);
                self.state.items.remove(&g);
            }
            SessionEvent::CinematicTriggered { .. } => self.writer.complete_cinematic()?,
            SessionEvent::Worldport {
                map_id,
                position,
                orientation,
                needs_ack,
            } => {
                self.state.map = map_id;
                self.state.position = position;
                self.state.orientation = orientation;
                if needs_ack {
                    self.state.entities.clear();
                    self.state.items.clear();
                    self.writer.worldport_ack()?;
                }
            }
            SessionEvent::Teleport {
                guid,
                counter,
                position,
                orientation,
            } => {
                if guid == self.state.guid {
                    self.state.position = position;
                    self.state.orientation = orientation;
                }
                self.writer.teleport_ack(guid, counter)?;
            }
            SessionEvent::MoveMode {
                guid,
                counter,
                mode,
                apply,
            } => {
                self.writer.move_mode_ack(
                    guid,
                    counter,
                    mode,
                    apply,
                    if apply { mode.flag() } else { 0 },
                    (self.state.position, self.state.orientation),
                )?;
            }
            SessionEvent::ClientControl { mover, allow_move } => {
                if allow_move && mover == self.state.guid {
                    self.writer.set_active_mover(mover)?;
                }
            }
            SessionEvent::ForceSpeedChange {
                guid,
                kind,
                counter,
                speed,
            } => {
                self.writer.force_speed_change_ack(
                    kind,
                    guid,
                    counter,
                    speed,
                    0,
                    self.state.position,
                    self.state.orientation,
                    0.0,
                    0,
                    None,
                    None,
                )?;
                if guid == self.state.guid && kind == messages::SpeedKind::Run {
                    self.state.speed = speed;
                }
            }
            SessionEvent::Chat(chat) => {
                if self.setup && chat.chat_type == 0x0A && self.setup_feedback.len() < 32 {
                    self.setup_feedback.push(chat.text.clone());
                }
                emit(json!({"event":"chat","text":chat.text}));
            }
            SessionEvent::LevelUp(info) => emit(json!({"event":"level_up","level":info.level})),
            SessionEvent::QuestComplete(q) => {
                if self.state.turned_in.len() < 1024 && !self.state.turned_in.contains(&q.quest_id)
                {
                    self.state.turned_in.push(q.quest_id);
                }
                emit(json!({"event":"quest_complete","quest":q.quest_id}));
            }
            SessionEvent::PartyKillLog(k) if k.killer == self.state.guid => {
                if self.kill_lives.credit(k.victim) {
                    if let Some(entry) = self
                        .state
                        .entities
                        .get(&k.victim)
                        .and_then(|e| e.fields.object_entry())
                    {
                        *self.state.killed.entry(entry).or_default() += 1;
                        emit(json!({"event":"kill","entry":entry,"guid":k.victim.to_string()}));
                    }
                }
            }
            SessionEvent::VendorInventory { vendor, .. } => self.vendor_seen = Some(vendor),
            SessionEvent::ItemTemplate { entry, info } => {
                if let Some(info) = info {
                    if self.qualities.len() < 1024 {
                        self.qualities.insert(entry, info.quality);
                    }
                }
            }
            SessionEvent::LootResponse {
                guid, gold, items, ..
            } => self.loot_window = Some((guid, gold, items)),
            SessionEvent::LootError { error, .. } => self.loot_error = Some(error),
            SessionEvent::CastResult {
                spell_id,
                success,
                reason,
                ..
            } => self.cast_result = Some((spell_id, success, reason)),
            SessionEvent::QuestFailed { quest_id, .. } => {
                emit(json!({"event":"feedback","text":format!("quest {quest_id} failed")}))
            }
            SessionEvent::XpGain(xp) => self.state.earned_xp += u64::from(xp.total),
            SessionEvent::ItemPushResult(item) => {
                emit(json!({"event":"loot","entry":item.item_entry,"count":item.count}))
            }
            SessionEvent::LoggedOut => {
                self.disconnected = true;
                emit(json!({"event":"disconnect"}));
            }
            SessionEvent::CharacterLoginFailed { .. } => bail!("character login refused"),
            _ => {}
        }
        Ok(())
    }

    pub fn pump(&mut self, duration: Duration) -> Result<()> {
        let end = Instant::now() + duration;
        loop {
            if self.last_ping.elapsed() >= Duration::from_secs(30) && !self.disconnected {
                self.ping += 1;
                self.writer.ping(self.ping, 0)?;
                self.last_ping = Instant::now();
            }
            for _ in 0..64 {
                match self.events.try_recv() {
                    Ok(packet) => {
                        for e in packet? {
                            self.apply(e)?;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(_) => bail!("world stream closed"),
                }
            }
            let mut finished = Vec::new();
            for (guid, m) in &self.motions {
                let progress =
                    (m.started.elapsed().as_secs_f32() / m.duration.as_secs_f32()).min(1.0);
                let total = m.path.windows(2).map(|p| distance(p[0], p[1])).sum::<f32>();
                let mut left = total * progress;
                let mut pos = *m.path.last().unwrap();
                for pair in m.path.windows(2) {
                    let length = distance(pair[0], pair[1]);
                    if left <= length && length > 0.0 {
                        pos = std::array::from_fn(|i| {
                            pair[0][i] + (pair[1][i] - pair[0][i]) * left / length
                        });
                        break;
                    }
                    left -= length;
                }
                if let Some(e) = self.state.entities.get_mut(guid) {
                    e.position = pos;
                }
                if *guid == self.state.guid {
                    self.state.position = pos;
                }
                if progress >= 1.0 {
                    finished.push((*guid, m.spline));
                }
            }
            for (guid, spline) in finished {
                self.motions.remove(&guid);
                if guid == self.state.guid {
                    self.writer.move_spline_done(
                        0,
                        self.state.position,
                        self.state.orientation,
                        spline,
                    )?;
                }
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            ensure!(
                self.closing || !self.closed.load(Ordering::Relaxed),
                "input closed; operation cancelled"
            );
            match self
                .events
                .recv_timeout(left.min(Duration::from_millis(50)))
            {
                Ok(packet) => {
                    for e in packet? {
                        self.apply(e)?;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("world stream closed"),
            }
        }
    }

    pub fn connected(&self) -> bool {
        !self.disconnected
    }

    pub fn goto(&mut self, args: &Value, seconds: u64) -> Result<Value> {
        let points = if let Some(p) = args["waypoints"].as_array() {
            ensure!(p.len() <= 32 && !p.is_empty(), "need 1–32 waypoints");
            p.clone()
        } else {
            vec![json!([args["x"], args["y"], args["z"]])]
        };
        let end = Instant::now() + Duration::from_secs(seconds);
        let origin = self.state.position;
        let result = (|| -> Result<()> {
            for point in points {
                let list = point
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("waypoint must be a coordinate triple"))?;
                ensure!(list.len() == 3, "waypoint must be a coordinate triple");
                let mut dest = [0.0; 3];
                for i in 0..3 {
                    let n = list[i]
                        .as_f64()
                        .ok_or_else(|| anyhow::anyhow!("coordinate must be numeric"))?;
                    ensure!(n.is_finite() && n.abs() <= 50000.0, "invalid coordinate");
                    dest[i] = n as f32;
                }
                while distance(self.state.position, dest) > 0.75 {
                    ensure!(
                        Instant::now() < end,
                        "movement timed out; route needs waypoints"
                    );
                    let a = self.state.position;
                    let d = distance(a, dest);
                    let step = (self.state.speed * 0.1).min(d);
                    let orientation = (dest[1] - a[1]).atan2(dest[0] - a[0]);
                    self.writer.send_movement(
                        opcode::MSG_MOVE_START_FORWARD,
                        1,
                        a,
                        orientation,
                        0.0,
                        0,
                        None,
                        None,
                    )?;
                    self.pump(Duration::from_millis(100))?;
                    let next = std::array::from_fn(|i| a[i] + (dest[i] - a[i]) * step / d);
                    self.writer.send_movement(
                        opcode::MSG_MOVE_HEARTBEAT,
                        1,
                        next,
                        orientation,
                        0.0,
                        0,
                        None,
                        None,
                    )?;
                    self.state.position = next;
                    self.state.orientation = orientation;
                }
            }
            Ok(())
        })();
        let stop = self.writer.send_movement(
            opcode::MSG_MOVE_STOP,
            0,
            self.state.position,
            self.state.orientation,
            0.0,
            0,
            None,
            None,
        );
        result?;
        stop?;
        self.pump(Duration::from_millis(100))?;
        Ok(
            json!({"reached":true,"position":self.state.position,"distance":distance(origin,self.state.position)}),
        )
    }

    pub fn cleanup(&mut self) -> Result<Value> {
        ensure!(!self.setup, "setup character is retained");
        self.disconnect()?;
        let args = &self.join_args;
        let account = args["account"].as_str().unwrap();
        let file = std::env::var_os("VOYAGER_WOW_ACCOUNTS")
            .ok_or_else(|| anyhow::anyhow!("credential file required"))?;
        let password = credentials(std::path::Path::new(&file), account)?;
        let logon = benilla_protocol::logon(args["auth"].as_str().unwrap(), account, &password)?;
        let address = args["world"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| logon.realms.first().map(|r| r.address.clone()))
            .ok_or_else(|| anyhow::anyhow!("no realm"))?;
        let mut session = WorldSession::connect(address, account, logon.session_key)?;
        let name = args["character"].as_str().unwrap();
        let roster = session.char_enum()?;
        if let Some(c) = roster.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
            ensure!(
                session.delete_character(c.guid)? == messages::CHAR_DELETE_SUCCESS,
                "cleanup deletion refused"
            );
        }
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            if !session
                .char_enum()?
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(name))
            {
                break;
            }
            ensure!(Instant::now() < end, "character survived cleanup");
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(json!({"deleted":true,"character":name}))
    }

    pub fn disconnect(&mut self) -> Result<()> {
        if self.disconnected {
            return Ok(());
        }
        self.closing = true;
        self.writer.logout_request()?;
        let end = Instant::now() + Duration::from_secs(25);
        while !self.disconnected && Instant::now() < end {
            self.pump(Duration::from_millis(50))?;
        }
        ensure!(self.disconnected, "logout did not complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_refuse_public_permissions_and_missing_accounts() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "[{{\"account\":\"GYM1\",\"password\":\"fixture\"}}]").unwrap();
        assert_eq!(credentials(f.path(), "gym1").unwrap(), "fixture");
        assert!(credentials(f.path(), "GYM2").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(f.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(credentials(f.path(), "GYM1").is_err());
        }
    }
}

/// A creature GUID survives respawns; duplicate protection lasts one life.
#[derive(Default)]
struct KillLives(std::collections::BTreeSet<u64>);
impl KillLives {
    fn alive(&mut self, guid: u64) {
        self.0.remove(&guid);
    }
    fn credit(&mut self, guid: u64) -> bool {
        self.0.len() < 4096 && self.0.insert(guid)
    }
}
#[cfg(test)]
mod kill_tests {
    use super::*;
    #[test]
    fn a_respawn_can_earn_new_credit_but_a_duplicate_cannot() {
        let mut lives = KillLives::default();
        lives.alive(17);
        assert!(lives.credit(17));
        assert!(!lives.credit(17));
        lives.alive(17);
        assert!(lives.credit(17));
        assert!(!lives.credit(17));
        assert!(lives.credit(18));
    }
}
