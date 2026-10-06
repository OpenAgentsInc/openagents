//! Bounded operational input replay over the owned world authority.
//!
//! The recorder owns its game. All mutations in this profile pass through
//! `apply`; transport authentication and realm services remain separate.
use crate::{
    Command, Controller,
    movement::frames::Frame,
    play::{Ability, Game},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use verse_engine::core::{FixedSchedule, LifeId};

pub const SCHEMA: &str = "verse.input-replay.v1";
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 512;
const SHUTDOWN_RESERVE: usize = 2 * 1024 * 1024 + 8192;
pub const RNG: &str = "splitmix64-rejection-v1; caster seed xor actor*0x9e3779b97f4a7c15";

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| format!("Cannot encode input replay: {e}"))
}

/// Exact build and operator-declared runtime compatibility, not attestation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub executable: [u8; 32],
    pub compiler: String,
    pub target: String,
    pub build: String,
    /// Declared OS, CPU, and runtime profile; exact equality is required.
    pub runtime: String,
    pub configuration: [u8; 32],
    pub content: [u8; 32],
    pub rules: String,
    pub rng: String,
}
impl Execution {
    /// Portable adapters supply the digest of their deployed executable artifact.
    pub fn declared(
        executable: [u8; 32],
        runtime: String,
        configuration: [u8; 32],
        content: [u8; 32],
    ) -> Result<Self, String> {
        let result = Self {
            executable,
            compiler: env!("VERSE_REPLAY_COMPILER").into(),
            target: env!("VERSE_REPLAY_TARGET").into(),
            build: env!("VERSE_REPLAY_BUILD").into(),
            runtime,
            configuration,
            content,
            rules: crate::play::RULES_REVISION.into(),
            rng: RNG.into(),
        };
        result.validate()?;
        Ok(result)
    }
    /// Hashes the native executable from its current path without retaining it.
    pub fn native(
        runtime: String,
        configuration: [u8; 32],
        content: [u8; 32],
    ) -> Result<Self, String> {
        let mut file = File::open(std::env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        let mut bytes = [0u8; 64 * 1024];
        let mut total = 0usize;
        loop {
            let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total = total
                .checked_add(n)
                .ok_or("Executable byte budget exceeded")?;
            if total > 1024 * 1024 * 1024 {
                return Err("Executable byte budget exceeded".into());
            }
            digest.update(&bytes[..n]);
        }
        Self::declared(digest.finalize().into(), runtime, configuration, content)
    }
    fn validate(&self) -> Result<(), String> {
        if [self.executable, self.configuration, self.content].contains(&[0; 32])
            || self.compiler != env!("VERSE_REPLAY_COMPILER")
            || self.target != env!("VERSE_REPLAY_TARGET")
            || self.build != env!("VERSE_REPLAY_BUILD")
            || self.rules != crate::play::RULES_REVISION
            || self.rng != RNG
            || self.runtime.is_empty()
            || self.runtime.len() > 256
            || self.runtime.bytes().any(|b| !(32..=126).contains(&b))
        {
            return Err("Unsupported input replay execution profile".into());
        }
        Ok(())
    }
}

/// Local authority operations. These values confer no network admission.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Command {
        controller: Controller,
        command: Command<Ability>,
    },
    Handoff {
        life: LifeId,
        controller: Controller,
    },
    Join {
        controller: Controller,
        spawn: [f32; 3],
    },
    BeginFrames {
        controller: Controller,
        life: LifeId,
    },
    Frame {
        controller: Controller,
        frame: Frame,
    },
    Social {
        controller: Controller,
        input: crate::play::social::Input,
    },
    Respawn {
        controller: Controller,
        life: LifeId,
    },
    /// The same 30 Hz, three-step catch-up schedule as the native chamber host.
    Elapsed { seconds: f64 },
    /// Pins an in-memory world checkpoint at the next logical commit revision.
    Commit {},
    /// Restores the committed checkpoint and resets the host elapsed-time clock.
    Restore {},
    /// Fences all controllers, commits, and seals this segment.
    Shutdown {},
}
impl Operation {
    fn envelope(&self) -> Option<(LifeId, u64, u64)> {
        match self {
            Self::Command { command, .. } => Some((command.actor, command.epoch, command.sequence)),
            Self::Frame { frame, .. } => Some((frame.life, frame.epoch, frame.sequence)),
            Self::Social { input, .. } => Some((input.life, input.epoch, input.sequence)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Applied { value: Value },
    Refused { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    tick: u64,
    physics_step: u64,
    revision: u64,
    schedule: Value,
    world: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    sequence: u64,
    operation: Operation,
    admission_consumed: Option<bool>,
    outcome: Outcome,
    observations: Vec<Observation>,
    chain: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    schema: String,
    execution: Execution,
    initial: String,
    initial_digest: [u8; 32],
    initial_rng: Value,
}
/// A complete, closed input segment. Presentation trajectories use another format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    header: Header,
    entries: Vec<Entry>,
    count: usize,
    seal: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Divergence {
    pub sequence: u64,
    pub tick: u64,
    /// JSON pointer into the outcome or authoritative observation.
    pub field: String,
    pub expected: Value,
    pub actual: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub entries: usize,
    pub ticks: u64,
    pub commit_revision: u64,
    pub dropped_seconds: f64,
    pub final_world: [u8; 32],
    pub divergence: Option<Divergence>,
}

#[derive(Clone)]
struct State {
    game: Game,
    schedule: FixedSchedule,
    revision: u64,
    committed: Vec<u8>,
    closed: bool,
}
impl State {
    fn new(game: Game) -> Result<Self, String> {
        let committed = game.checkpoint()?;
        Ok(Self {
            game,
            schedule: FixedSchedule::new(30, 3)?,
            revision: 0,
            committed,
            closed: false,
        })
    }
    fn observe(&self) -> Result<Observation, String> {
        Ok(Observation {
            tick: self.game.authority_tick,
            physics_step: self.game.physics_steps,
            revision: self.revision,
            schedule: serde_json::to_value(&self.schedule).map_err(|e| e.to_string())?,
            world: self.game.checkpoint_parts()?.0,
        })
    }
    fn commit(&mut self) -> Result<(), String> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Replay commit revision exhausted")?;
        self.committed = self.game.checkpoint()?;
        self.revision = revision;
        Ok(())
    }
    fn execute(&mut self, operation: &Operation) -> Result<(Outcome, Vec<Observation>), String> {
        if self.closed {
            return Err("Input replay segment is closed".into());
        }
        let mut observations = vec![];
        let result: Result<Value, String> = match operation {
            Operation::Command {
                controller,
                command,
            } => self
                .game
                .submit(*controller, command.clone())
                .map(|()| Value::Null),
            Operation::Handoff { life, controller } => self
                .game
                .handoff_player(*life, *controller)
                .map(|()| Value::Null),
            Operation::Join { controller, spawn } => self
                .game
                .add_player(*controller, (*spawn).into())
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
            Operation::BeginFrames { controller, life } => self
                .game
                .begin_movement_frames(*controller, *life)
                .map(|()| Value::Null),
            Operation::Frame { controller, frame } => self
                .game
                .submit_movement_frame(*controller, frame.clone())
                .map(|()| Value::Null),
            Operation::Social { controller, input } => self
                .game
                .submit_social(*controller, input.clone())
                .map(|()| Value::Null),
            Operation::Respawn { controller, life } => self
                .game
                .respawn_controlled_player(*controller, *life)
                .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
            Operation::Elapsed { seconds } => match self.schedule.advance(*seconds) {
                Err(error) => Err(error),
                Ok(batch) => {
                    // Each tick remains observable even within a catch-up batch.
                    for _ in 0..batch.steps {
                        self.game.tick(batch.seconds, [0.; 2])?;
                        observations.push(self.observe()?);
                    }
                    Ok(
                        serde_json::json!({"steps":batch.steps,"dropped_seconds":batch.dropped_seconds}),
                    )
                }
            },
            Operation::Commit {} => {
                self.commit()?;
                Ok(Value::Null)
            }
            Operation::Restore {} => {
                self.game = Game::restore(&self.committed)?;
                self.schedule = FixedSchedule::new(30, 3)?;
                Ok(Value::Null)
            }
            Operation::Shutdown {} => {
                let lives: Vec<_> = self
                    .game
                    .controlled_effects()
                    .map(|(life, _, _)| life)
                    .collect();
                for life in lives {
                    self.game.handoff_player(life, Controller(0))?;
                }
                self.commit()?;
                self.closed = true;
                Ok(Value::Null)
            }
        };
        if observations.is_empty() {
            observations.push(self.observe()?);
        }
        Ok((
            match result {
                Ok(value) => Outcome::Applied { value },
                Err(reason) => Outcome::Refused { reason },
            },
            observations,
        ))
    }
}
fn rng(game: &Game) -> Result<Value, String> {
    Ok(serde_json::json!({"primary":game.spells.dice,"casters":game.spells.caster_dice}))
}
fn chain(previous: [u8; 32], entry: &Entry) -> Result<[u8; 32], String> {
    Ok(hash(&encode(&(
        SCHEMA,
        previous,
        entry.sequence,
        &entry.operation,
        entry.admission_consumed,
        &entry.outcome,
        &entry.observations,
    ))?))
}
fn consumed(before: &Game, after: &Game, operation: &Operation) -> Option<bool> {
    operation.envelope().map(|(life, epoch, sequence)| {
        before
            .player_admission(life.actor)
            .zip(after.player_admission(life.actor))
            .is_some_and(|(a, b)| {
                a.actor() == life
                    && b.actor() == life
                    && a.epoch() == epoch
                    && b.epoch() == epoch
                    && a.accepted_sequence() < sequence
                    && b.accepted_sequence() >= sequence
            })
    })
}
fn seal(previous: [u8; 32], count: usize) -> Result<[u8; 32], String> {
    Ok(hash(&encode(&(
        "verse.input-replay.end.v1",
        previous,
        count,
    ))?))
}

/// Owns a bounded authority segment; budget errors leave the game unchanged.
pub struct Recorder {
    state: State,
    trace: Trace,
    previous: [u8; 32],
    bytes: usize,
}
impl Recorder {
    pub fn new(game: Game, execution: Execution) -> Result<Self, String> {
        execution.validate()?;
        let initial = game.checkpoint()?;
        let restored = Game::restore(&initial)?;
        if restored.checkpoint()? != initial {
            return Err("Input replay requires a current canonical checkpoint".into());
        }
        let header = Header {
            schema: SCHEMA.into(),
            execution,
            initial_digest: hash(&initial),
            initial: String::from_utf8(initial).map_err(|e| e.to_string())?,
            initial_rng: rng(&game)?,
        };
        let previous = hash(&encode(&header)?);
        let bytes = encode(&header)?.len() + 1024;
        Ok(Self {
            state: State::new(game)?,
            trace: Trace {
                header,
                entries: vec![],
                count: 0,
                seal: [0; 32],
            },
            previous,
            bytes,
        })
    }
    pub fn game(&self) -> &Game {
        &self.state.game
    }
    /// Rule refusals are recorded outcomes; recording errors apply no effects.
    pub fn apply(&mut self, operation: Operation) -> Result<Outcome, String> {
        if self.trace.entries.len() >= MAX_ENTRIES
            || (!matches!(operation, Operation::Shutdown {})
                && self.trace.entries.len() >= MAX_ENTRIES - 1)
        {
            return Err("Input replay entry budget exceeded".into());
        }
        // Non-finite API values cannot be represented by the closed JSON profile.
        let _: Operation = serde_json::from_slice(&encode(&operation)?)
            .map_err(|_| "Input replay operation is not representable")?;
        let mut next = self.state.clone();
        let (outcome, observations) = next.execute(&operation)?;
        let mut entry = Entry {
            sequence: self.trace.entries.len() as u64 + 1,
            admission_consumed: consumed(&self.state.game, &next.game, &operation),
            operation,
            outcome: outcome.clone(),
            observations,
            chain: [0; 32],
        };
        entry.chain = chain(self.previous, &entry)?;
        let bytes = self
            .bytes
            .checked_add(encode(&entry)?.len() + 1)
            .ok_or("Input replay byte budget exceeded")?;
        let reserve = if matches!(entry.operation, Operation::Shutdown {}) {
            0
        } else {
            SHUTDOWN_RESERVE
        };
        if bytes > MAX_BYTES - reserve {
            return Err("Input replay byte budget exceeded".into());
        }
        self.previous = entry.chain;
        self.bytes = bytes;
        self.state = next;
        self.trace.entries.push(entry);
        Ok(outcome)
    }
    pub fn finish(mut self) -> Result<Trace, String> {
        if !self.state.closed {
            return Err("Input replay requires a recorded shutdown".into());
        }
        self.trace.count = self.trace.entries.len();
        self.trace.seal = seal(self.previous, self.trace.count)?;
        self.trace.to_json()?;
        Ok(self.trace)
    }
}

fn difference(expected: &Value, actual: &Value, path: &str) -> Option<(String, Value, Value)> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let pointer = key.replace('~', "~0").replace('/', "~1");
                if let Some(v) = difference(
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{pointer}"),
                ) {
                    return Some(v);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for i in 0..a.len().max(b.len()) {
                if let Some(v) = difference(
                    a.get(i).unwrap_or(&Value::Null),
                    b.get(i).unwrap_or(&Value::Null),
                    &format!("{path}/{i}"),
                ) {
                    return Some(v);
                }
            }
        }
        _ => {}
    }
    Some((path.into(), expected.clone(), actual.clone()))
}
impl Trace {
    pub fn execution(&self) -> &Execution {
        &self.header.execution
    }
    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        let bytes = encode(self)?;
        if bytes.len() > MAX_BYTES {
            return Err("Input replay byte budget exceeded".into());
        }
        Ok(bytes)
    }
    pub fn digest(&self) -> Result<[u8; 32], String> {
        Ok(hash(&self.to_json()?))
    }
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err("Input replay byte budget exceeded".into());
        }
        let trace: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid input replay: {e}"))?;
        if trace.to_json()? != bytes {
            return Err("Input replay is not canonical".into());
        }
        trace.validate()?;
        Ok(trace)
    }
    fn validate(&self) -> Result<(), String> {
        self.header.execution.validate()?;
        if self.header.schema != SCHEMA
            || self.entries.is_empty()
            || self.entries.len() > MAX_ENTRIES
            || self.count != self.entries.len()
            || hash(self.header.initial.as_bytes()) != self.header.initial_digest
        {
            return Err("Invalid or incomplete input replay".into());
        }
        let initial = Game::restore(self.header.initial.as_bytes())?;
        if initial.checkpoint()? != self.header.initial.as_bytes()
            || rng(&initial)? != self.header.initial_rng
        {
            return Err("Input replay initial state differs".into());
        }
        let mut previous = hash(&encode(&self.header)?);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.sequence != i as u64 + 1
                || entry.observations.is_empty()
                || entry.observations.len() > 3
                || entry.chain != chain(previous, entry)?
                || matches!(entry.operation, Operation::Shutdown {})
                    != (i + 1 == self.entries.len())
            {
                return Err("Invalid input replay order or chain".into());
            }
            previous = entry.chain;
        }
        if self.seal != seal(previous, self.count)? {
            return Err("Input replay end seal differs".into());
        }
        Ok(())
    }
    /// Compares every recorded result and authority tick on the exact profile.
    pub fn replay(&self, expected: &Execution) -> Result<Report, String> {
        expected.validate()?;
        self.validate()?;
        if &self.header.execution != expected {
            return Err("Input replay execution identity differs".into());
        }
        let mut state = State::new(Game::restore(self.header.initial.as_bytes())?)?;
        let mut report = Report {
            entries: 0,
            ticks: 0,
            commit_revision: 0,
            dropped_seconds: 0.,
            final_world: self.header.initial_digest,
            divergence: None,
        };
        for entry in &self.entries {
            let before = state.game.authority_tick;
            let before_game = state.game.clone();
            let (outcome, observations) = state.execute(&entry.operation)?;
            let expected_outcome = serde_json::json!({"outcome":entry.outcome,"admission_consumed":entry.admission_consumed});
            let actual_outcome = serde_json::json!({"outcome":outcome,"admission_consumed":consumed(&before_game,&state.game,&entry.operation)});
            let mut mismatch =
                difference(&expected_outcome, &actual_outcome, "/entry").map(|d| (before, d));
            if mismatch.is_none() {
                for i in 0..entry.observations.len().max(observations.len()) {
                    let a = entry
                        .observations
                        .get(i)
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .unwrap_or(Value::Null);
                    let b = observations
                        .get(i)
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .unwrap_or(Value::Null);
                    if let Some(d) = difference(&a, &b, "/observation") {
                        let tick = observations.get(i).map_or(before, |o| o.tick);
                        mismatch = Some((tick, d));
                        break;
                    }
                }
            }
            report.entries += 1;
            if matches!(entry.operation, Operation::Elapsed { .. })
                && state.game.authority_tick != before
            {
                report.ticks += observations.len() as u64;
            }
            report.commit_revision = state.revision;
            if let Outcome::Applied { value } = &outcome {
                if matches!(entry.operation, Operation::Elapsed { .. }) {
                    report.dropped_seconds += value["dropped_seconds"]
                        .as_f64()
                        .ok_or("Replay schedule outcome is missing")?;
                }
            }
            report.final_world = hash(&state.game.checkpoint()?);
            if let Some((tick, (field, expected, actual))) = mismatch {
                report.divergence = Some(Divergence {
                    sequence: entry.sequence,
                    tick,
                    field,
                    expected,
                    actual,
                });
                break;
            }
        }
        Ok(report)
    }
    /// Creates a private immutable retained segment and syncs its directory.
    pub fn write_new(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = self.to_json()?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
            return Err("Input replay byte budget exceeded".into());
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::from_json(&bytes)
    }
}

#[cfg(test)]
mod tests;
