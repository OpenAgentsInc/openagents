//! Synchronous actor definitions and transaction preparation.
//!
//! Handlers stage commands; the store commits those commands with their state.
//! Handlers must not perform I/O or read clocks or randomness outside [`Ctx`].
//! This is an API contract for trusted Rust code, not a sandbox or a CPU limiter.
//! Payload privacy belongs to each actor and its host. Store opaque references
//! to credentials and customer documents, rather than their contents.

use crate::types::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

pub const MAX_STATE_BYTES: usize = 256 * 1024;
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_REPLY_BYTES: usize = 64 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub const MAX_COMMANDS: usize = 100;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_JSON_NODES: usize = 32_768;
const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_RANDOM_CALLS: u64 = 4096;
const MAX_LEASE_MS: i64 = 86_400_000;
const MAX_EFFECT_TIMEOUT_MS: i64 = 600_000;

/// A state owner whose handlers execute synchronously inside a transaction.
pub trait Actor: Sized + Send + 'static {
    const TYPE: &'static str;
    const STATE_VERSION: u32;
    const PRIVATE: bool = false;
    const CREATE_ACCESS: Access = Access::Member;
    const VIEW_ACCESS: Access = Access::Member;

    type State: Serialize + DeserializeOwned + Send;
    type Input: DeserializeOwned + Send;

    fn create(input: Self::Input, ctx: &mut Ctx) -> Result<Self::State>;
    fn wake(state: &Self::State) -> Result<Self>;
    fn view(state: &Self::State, caller: &Caller) -> Result<Value>;

    /// Upgrade an older document directly to `STATE_VERSION`.
    ///
    /// The store persists the upgrade with the next successful write. Views and
    /// read-only messages use an upgraded copy without changing stored state.
    fn migrate(_from: u32, _state: Value) -> Result<Value> {
        Err(ActorError::new("migration", "This record needs an update."))
    }

    fn input_schema() -> Value {
        Value::Bool(true)
    }
    fn state_schema() -> Value {
        Value::Bool(true)
    }
    fn view_schema() -> Value {
        Value::Bool(true)
    }
    fn events_schema() -> Value {
        json!({})
    }
    fn description() -> &'static str {
        ""
    }
}

/// A versioned message and its access requirements.
pub trait Message: DeserializeOwned + Send + 'static {
    const NAME: &'static str;
    const ACCESS: Access = Access::Member;
    const READ_ONLY: bool = false;
    type Reply: Serialize;

    fn schema() -> Value {
        Value::Bool(true)
    }
    fn reply_schema() -> Value {
        Value::Bool(true)
    }
}

pub trait Handles<M: Message>: Actor {
    fn handle(&mut self, state: &mut Self::State, message: M, ctx: &mut Ctx) -> Result<M::Reply>;
}

/// Inputs and staged commands for one attempted transition.
///
/// Command IDs and random values depend only on the actor UID, the next state
/// version, and their position. Retrying the same transition reproduces them.
pub struct Ctx {
    id: ActorId,
    uid: String,
    caller: Caller,
    next_version: u64,
    now: Timestamp,
    read_only: bool,
    random_index: u64,
    commands: Vec<Command>,
    failure: Option<ActorError>,
}

impl Ctx {
    fn new(
        id: &ActorId,
        uid: &str,
        caller: &Caller,
        next_version: u64,
        now: Timestamp,
        read_only: bool,
    ) -> Self {
        Self {
            id: id.clone(),
            uid: uid.into(),
            caller: caller.clone(),
            next_version,
            now,
            read_only,
            random_index: 0,
            commands: Vec::new(),
            failure: None,
        }
    }

    pub fn id(&self) -> &ActorId {
        &self.id
    }
    pub fn caller(&self) -> &Caller {
        &self.caller
    }
    pub fn now(&self) -> Timestamp {
        self.now
    }

    pub fn random(&mut self) -> u64 {
        if self.random_index >= MAX_RANDOM_CALLS {
            self.reject(ActorError::new(
                "limit",
                "This request needs too many random values.",
            ));
            return 0;
        }
        let bytes = digest_id(&self.uid, self.next_version, self.random_index, "random");
        self.random_index += 1;
        let mut value = [0; 8];
        value.copy_from_slice(&bytes[..8]);
        u64::from_be_bytes(value)
    }

    pub fn emit(&mut self, name: impl Into<String>, payload: impl Serialize) -> Result<()> {
        let name = name.into();
        let result = (|| {
            validate_name(&name)?;
            let payload = serialize_value(&payload, MAX_PAYLOAD_BYTES)?;
            Ok(Command::Emit {
                event: Event { name, payload },
            })
        })();
        self.stage(result)
    }

    /// Send a normal inbox message within this workspace.
    ///
    /// The store must check the destination's access rules in the same
    /// transaction, using this transition's caller. Sending cannot grant the
    /// authority to create a destination or submit runtime completion messages.
    pub fn send(&mut self, to: ActorId, message: Envelope) -> Result<String> {
        let key = self.command_id("send");
        let result = (|| {
            validate_actor_id(&to)?;
            validate_envelope(&message)?;
            if to.workspace_id != self.id.workspace_id {
                return Err(not_found());
            }
            if message.origin != Origin::Inbox || internal_name(&message.name) {
                return Err(forbidden());
            }
            Ok(Command::Send {
                to,
                message,
                idempotency_key: key.clone(),
            })
        })();
        self.stage(result)?;
        Ok(key)
    }

    pub fn schedule(&mut self, alarm: AlarmSpec) -> Result<()> {
        let result = (|| {
            validate_name(&alarm.name)?;
            validate_envelope(&alarm.message)?;
            if alarm.message.origin != Origin::Inbox || internal_name(&alarm.message.name) {
                return Err(forbidden());
            }
            if alarm.due_at < 0
                || alarm
                    .interval_ms
                    .is_some_and(|ms| !(1..=31_622_400_000).contains(&ms))
            {
                return Err(ActorError::new("bad_args", "The reminder time is invalid."));
            }
            Ok(Command::Schedule { alarm })
        })();
        self.stage(result)
    }

    pub fn cancel_alarm(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        let result = validate_name(&name).map(|()| Command::CancelAlarm { name });
        self.stage(result)
    }

    /// Stage an effect and replace its ID with the transition's deterministic ID.
    pub fn effect(&mut self, mut effect: EffectSpec) -> Result<String> {
        effect.id = self.command_id("effect");
        let id = effect.id.clone();
        let result = (|| {
            validate_name(&effect.kind)?;
            validate_json(&effect.payload, MAX_PAYLOAD_BYTES)?;
            if !(1..=MAX_EFFECT_TIMEOUT_MS).contains(&effect.timeout_ms)
                || !(1..=100).contains(&effect.max_attempts)
            {
                return Err(ActorError::new(
                    "bad_args",
                    "The operation limits are invalid.",
                ));
            }
            Ok(Command::Effect { effect })
        })();
        self.stage(result)?;
        Ok(id)
    }

    /// Stage work and replace its ID with the transition's deterministic ID.
    pub fn work(&mut self, mut work: WorkSpec) -> Result<String> {
        work.item_id = self.command_id("work");
        let id = work.item_id.clone();
        let result = (|| {
            validate_name(&work.queue)?;
            if let Some(target) = &work.target {
                validate_name(target)?;
            }
            validate_json(&work.payload, MAX_PAYLOAD_BYTES)?;
            if !(1..=MAX_LEASE_MS).contains(&work.lease_ms)
                || !(1..=100).contains(&work.max_attempts)
            {
                return Err(ActorError::new("bad_args", "The work limits are invalid."));
            }
            Ok(Command::Work { work })
        })();
        self.stage(result)?;
        Ok(id)
    }

    pub fn cancel_work(&mut self, item_id: impl Into<String>) -> Result<()> {
        let item_id = item_id.into();
        let result = validate_name(&item_id).map(|()| Command::CancelWork { item_id });
        self.stage(result)
    }

    pub fn destroy(&mut self) -> Result<()> {
        self.stage(Ok(Command::Destroy))
    }

    fn reject(&mut self, error: ActorError) -> ActorError {
        self.failure.get_or_insert_with(|| error.clone());
        error
    }

    fn stage(&mut self, command: Result<Command>) -> Result<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.read_only {
            return Err(self.reject(ActorError::new(
                "read_only",
                "This request cannot make changes.",
            )));
        }
        if self.commands.len() >= MAX_COMMANDS {
            return Err(self.reject(ActorError::new(
                "limit",
                "This request makes too many changes.",
            )));
        }
        if self
            .commands
            .iter()
            .any(|command| matches!(command, Command::Destroy))
        {
            return Err(self.reject(ActorError::new(
                "destroyed",
                "This record is being deleted.",
            )));
        }
        match command {
            Ok(command) => {
                self.commands.push(command);
                Ok(())
            }
            Err(error) => Err(self.reject(error)),
        }
    }

    fn finish(self) -> Result<Vec<Command>> {
        match self.failure {
            Some(error) => Err(error),
            None => Ok(self.commands),
        }
    }

    fn command_id(&self, kind: &str) -> String {
        let digest = digest_id(
            &self.uid,
            self.next_version,
            self.commands.len() as u64,
            kind,
        );
        let mut text = String::with_capacity(kind.len() + 65);
        text.push_str(kind);
        text.push('-');
        for byte in digest {
            use std::fmt::Write as _;
            // Formatting into a String cannot fail.
            let _ = write!(&mut text, "{byte:02x}");
        }
        text
    }
}

fn digest_id(uid: &str, version: u64, index: u64, kind: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"openagents.actor.command.v1\0");
    digest.update((uid.len() as u64).to_be_bytes());
    digest.update(uid.as_bytes());
    digest.update(version.to_be_bytes());
    digest.update(index.to_be_bytes());
    digest.update((kind.len() as u64).to_be_bytes());
    digest.update(kind.as_bytes());
    digest.finalize().into()
}

type Handler<A> = fn(&mut A, &mut <A as Actor>::State, Value, &mut Ctx) -> Result<Value>;

struct MessageDefinition<A: Actor> {
    name: &'static str,
    access: Access,
    read_only: bool,
    invoke: Handler<A>,
    schema: fn() -> Value,
    reply_schema: fn() -> Value,
}

/// A typed dispatch table. Register each supported message explicitly.
pub struct Definition<A: Actor> {
    messages: Vec<MessageDefinition<A>>,
    actor: PhantomData<fn() -> A>,
}

impl<A: Actor> Default for Definition<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Actor> Definition<A> {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            actor: PhantomData,
        }
    }

    pub fn message<M: Message>(mut self) -> Self
    where
        A: Handles<M>,
    {
        self.messages.push(MessageDefinition {
            name: M::NAME,
            access: M::ACCESS,
            read_only: M::READ_ONLY,
            invoke: invoke::<A, M>,
            schema: M::schema,
            reply_schema: M::reply_schema,
        });
        self
    }
}

/// Build a definition and its JSON contract from one message list.
#[macro_export]
macro_rules! actor_messages {
    ($actor:ty; $($message:ty),* $(,)?) => {
        $crate::core::Definition::<$actor>::new()$(.message::<$message>())*
    };
}

fn invoke<A, M>(actor: &mut A, state: &mut A::State, value: Value, ctx: &mut Ctx) -> Result<Value>
where
    A: Handles<M>,
    M: Message,
{
    let message = serde_json::from_value(value)?;
    let reply = actor.handle(state, message, ctx)?;
    serialize_value(&reply, MAX_REPLY_BYTES)
}

trait ErasedActor: Send + Sync {
    fn private(&self) -> bool;
    fn create(
        &self,
        id: &ActorId,
        uid: &str,
        input: Value,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared>;
    fn apply(
        &self,
        snapshot: &Snapshot,
        message: &Envelope,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared>;
    fn view(&self, snapshot: &Snapshot, caller: &Caller) -> Result<Value>;
    fn authorize(&self, snapshot: &Snapshot, message: &Envelope, caller: &Caller) -> Result<()>;
    fn contract(&self) -> &Value;
}

struct Registered<A: Actor> {
    messages: BTreeMap<&'static str, MessageDefinition<A>>,
    contract: Value,
    actor: PhantomData<fn() -> A>,
}

impl<A: Actor> Registered<A> {
    fn load(&self, snapshot: &Snapshot) -> Result<A::State> {
        validate_json(&snapshot.state, MAX_STATE_BYTES)?;
        if snapshot.state_version > A::STATE_VERSION {
            return Err(ActorError::retry(
                "version_ahead",
                "Try again after the update finishes.",
            ));
        }
        if snapshot.state_version == 0 {
            return Err(ActorError::new(
                "migration",
                "This record has an invalid version.",
            ));
        }
        let state = if snapshot.state_version < A::STATE_VERSION {
            let state = A::migrate(snapshot.state_version, snapshot.state.clone())?;
            validate_json(&state, MAX_STATE_BYTES)?;
            state
        } else {
            snapshot.state.clone()
        };
        serde_json::from_value(state)
            .map_err(|_| ActorError::new("state", "This record could not be opened."))
    }

    fn check_snapshot(&self, snapshot: &Snapshot, caller: &Caller) -> Result<()> {
        check_workspace(&snapshot.id, caller)?;
        if snapshot.id.actor_type != A::TYPE || snapshot.status == Status::Destroyed {
            return Err(not_found());
        }
        if A::PRIVATE && !account_owner(snapshot.owner.as_deref(), caller) {
            return Err(not_found());
        }
        Ok(())
    }
}

impl<A: Actor> ErasedActor for Registered<A> {
    fn private(&self) -> bool {
        A::PRIVATE
    }

    fn create(
        &self,
        id: &ActorId,
        uid: &str,
        input: Value,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared> {
        check_workspace(id, caller)?;
        if A::PRIVATE && !account_owner(caller.account_id.as_deref(), caller) {
            return Err(forbidden());
        }
        require_access(
            A::CREATE_ACCESS,
            Origin::Action,
            caller.account_id.as_deref(),
            caller,
        )?;
        validate_json(&input, MAX_INPUT_BYTES)?;
        let input = serde_json::from_value(input)?;
        let mut ctx = Ctx::new(id, uid, caller, 0, now, false);
        let state = A::create(input, &mut ctx)?;
        let state = serialize_value(&state, MAX_STATE_BYTES)?;
        let commands = ctx.finish()?;
        Ok(Prepared {
            state,
            state_version: A::STATE_VERSION,
            reply: Value::Null,
            commands,
            read_only: false,
        })
    }

    fn apply(
        &self,
        snapshot: &Snapshot,
        message: &Envelope,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared> {
        self.authorize(snapshot, message, caller)?;
        let definition = self
            .messages
            .get(message.name.as_str())
            .ok_or_else(unknown_message)?;
        let mut state = self.load(snapshot)?;
        let before = if definition.read_only {
            Some(serialize_value(&state, MAX_STATE_BYTES)?)
        } else {
            None
        };
        let mut actor = A::wake(&state)?;
        let version = snapshot
            .version
            .checked_add(1)
            .ok_or_else(|| ActorError::new("limit", "This record has reached its update limit."))?;
        let mut ctx = Ctx::new(
            &snapshot.id,
            &snapshot.uid,
            caller,
            version,
            now,
            definition.read_only,
        );
        let reply = (definition.invoke)(&mut actor, &mut state, message.args.clone(), &mut ctx)?;
        let state = serialize_value(&state, MAX_STATE_BYTES)?;
        let commands = ctx.finish()?;
        if before.as_ref().is_some_and(|before| before != &state) {
            return Err(ActorError::new(
                "read_only",
                "This request cannot make changes.",
            ));
        }
        let (state, state_version) = if definition.read_only {
            // Read-only replies may use an upgraded copy, but their prepared
            // result must not ask the store to persist the upgrade.
            (snapshot.state.clone(), snapshot.state_version)
        } else {
            (state, A::STATE_VERSION)
        };
        Ok(Prepared {
            state,
            state_version,
            reply,
            commands,
            read_only: definition.read_only,
        })
    }

    fn view(&self, snapshot: &Snapshot, caller: &Caller) -> Result<Value> {
        self.check_snapshot(snapshot, caller)?;
        require_access(
            A::VIEW_ACCESS,
            Origin::Action,
            snapshot.owner.as_deref(),
            caller,
        )?;
        let state = self.load(snapshot)?;
        let view = A::view(&state, caller)?;
        validate_json(&view, MAX_REPLY_BYTES)?;
        Ok(view)
    }

    fn authorize(&self, snapshot: &Snapshot, message: &Envelope, caller: &Caller) -> Result<()> {
        self.check_snapshot(snapshot, caller)?;
        validate_envelope(message)?;
        if internal_name(&message.name) && message.origin != Origin::Internal {
            return Err(forbidden());
        }
        let definition = self
            .messages
            .get(message.name.as_str())
            .ok_or_else(unknown_message)?;
        require_access(
            definition.access,
            message.origin,
            snapshot.owner.as_deref(),
            caller,
        )?;
        if snapshot.status == Status::Blocked && !definition.read_only {
            return Err(ActorError::retry("blocked", "This record is paused."));
        }
        Ok(())
    }

    fn contract(&self) -> &Value {
        &self.contract
    }
}

/// A cheaply cloned registry shared by adapters and dispatchers.
#[derive(Clone, Default)]
pub struct Registry {
    actors: Arc<BTreeMap<String, Arc<dyn ErasedActor>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<A: Actor>(&mut self, definition: Definition<A>) -> Result<()> {
        let registered = guard(|| prepare_definition(definition))?;
        if self.actors.contains_key(A::TYPE) {
            return Err(ActorError::new(
                "duplicate_type",
                "This record type is already registered.",
            ));
        }
        Arc::make_mut(&mut self.actors).insert(A::TYPE.into(), Arc::new(registered));
        Ok(())
    }

    pub fn is_private(&self, actor_type: &str) -> Result<bool> {
        Ok(self.actor(actor_type)?.private())
    }

    pub fn create(
        &self,
        id: &ActorId,
        uid: &str,
        input: Value,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared> {
        check_workspace(id, caller)?;
        validate_actor_id(id)?;
        validate_token(uid, 256)?;
        validate_time(now)?;
        let actor = self.actor(&id.actor_type)?;
        guard(|| actor.create(id, uid, input, caller, now))
    }

    pub fn apply(
        &self,
        snapshot: &Snapshot,
        message: &Envelope,
        caller: &Caller,
        now: Timestamp,
    ) -> Result<Prepared> {
        check_workspace(&snapshot.id, caller)?;
        validate_actor_id(&snapshot.id)?;
        validate_token(&snapshot.uid, 256)?;
        validate_time(now)?;
        let actor = self.actor(&snapshot.id.actor_type)?;
        guard(|| actor.apply(snapshot, message, caller, now))
    }

    pub fn view(&self, snapshot: &Snapshot, caller: &Caller) -> Result<Value> {
        check_workspace(&snapshot.id, caller)?;
        validate_actor_id(&snapshot.id)?;
        let actor = self.actor(&snapshot.id.actor_type)?;
        guard(|| actor.view(snapshot, caller))
    }

    pub fn authorize(
        &self,
        snapshot: &Snapshot,
        message: &Envelope,
        caller: &Caller,
    ) -> Result<()> {
        check_workspace(&snapshot.id, caller)?;
        validate_actor_id(&snapshot.id)?;
        self.actor(&snapshot.id.actor_type)?
            .authorize(snapshot, message, caller)
    }

    pub fn contract(&self) -> Value {
        json!({
            "format": "openagents.actors.contract.v1",
            "types": self.actors.values().map(|actor| actor.contract().clone()).collect::<Vec<_>>(),
        })
    }

    fn actor(&self, actor_type: &str) -> Result<&dyn ErasedActor> {
        self.actors
            .get(actor_type)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                ActorError::retry("unknown_type", "This record type is not available here.")
            })
    }
}

fn prepare_definition<A: Actor>(definition: Definition<A>) -> Result<Registered<A>> {
    validate_name(A::TYPE)?;
    if A::TYPE.len() > 64
        || A::TYPE.contains('@')
        || A::STATE_VERSION == 0
        || definition.messages.len() > 256
    {
        return Err(ActorError::new(
            "definition",
            "The record type definition is invalid.",
        ));
    }
    if A::CREATE_ACCESS == Access::Internal || A::VIEW_ACCESS == Access::Internal {
        return Err(ActorError::new(
            "definition",
            "Internal access is only available for messages.",
        ));
    }
    if A::description().len() > 4096 {
        return Err(ActorError::new(
            "definition",
            "The record description is too long.",
        ));
    }
    let mut messages = BTreeMap::new();
    let mut contract_messages = Vec::new();
    for message in definition.messages {
        validate_name(message.name)?;
        if internal_name(message.name) && message.access != Access::Internal {
            return Err(ActorError::new(
                "definition",
                "Runtime messages require internal access.",
            ));
        }
        let schema = (message.schema)();
        let reply_schema = (message.reply_schema)();
        validate_json(&schema, MAX_SCHEMA_BYTES)?;
        validate_json(&reply_schema, MAX_SCHEMA_BYTES)?;
        contract_messages.push(json!({
            "name": message.name, "access": message.access, "read_only": message.read_only,
            "args": schema, "reply": reply_schema,
        }));
        if messages.insert(message.name, message).is_some() {
            return Err(ActorError::new(
                "duplicate_message",
                "This message is already registered.",
            ));
        }
    }
    contract_messages.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let input = A::input_schema();
    let state = A::state_schema();
    let view = A::view_schema();
    let events = A::events_schema();
    for schema in [&input, &state, &view, &events] {
        validate_json(schema, MAX_SCHEMA_BYTES)?;
    }
    let contract = json!({
        "type": A::TYPE, "state_version": A::STATE_VERSION, "private": A::PRIVATE,
        "create_access": A::CREATE_ACCESS, "view_access": A::VIEW_ACCESS,
        "description": A::description(), "input": input, "state": state,
        "view": view, "events": events, "messages": contract_messages,
    });
    Ok(Registered {
        messages,
        contract,
        actor: PhantomData,
    })
}

fn require_access(
    access: Access,
    origin: Origin,
    owner: Option<&str>,
    caller: &Caller,
) -> Result<()> {
    if (origin == Origin::Internal) != (access == Access::Internal) {
        return Err(forbidden());
    }
    let allowed = match access {
        Access::Member => true,
        Access::Owner => matches!(caller.role, Role::Owner | Role::Admin),
        Access::AccountOwner => account_owner(owner, caller),
        Access::Service => caller.role == Role::Service,
        Access::Admin => caller.role == Role::Admin,
        Access::Internal => matches!(caller.role, Role::Service | Role::Admin),
    };
    if allowed { Ok(()) } else { Err(forbidden()) }
}

fn account_owner(owner: Option<&str>, caller: &Caller) -> bool {
    owner.is_some_and(|owner| !owner.is_empty() && caller.account_id.as_deref() == Some(owner))
}

fn check_workspace(id: &ActorId, caller: &Caller) -> Result<()> {
    if id.workspace_id != caller.workspace_id
        || caller.workspace_id.is_empty()
        || caller.principal.is_empty()
    {
        return Err(not_found());
    }
    Ok(())
}

fn internal_name(name: &str) -> bool {
    name.starts_with("runtime.")
        || matches!(
            name.split('@').next(),
            Some("WorkDone" | "WorkExpired" | "EffectDone" | "EffectFailed")
        )
}

fn forbidden() -> ActorError {
    ActorError::new("forbidden", "You do not have access to this action.")
}
fn not_found() -> ActorError {
    ActorError::new("not_found", "This record was not found.")
}
fn unknown_message() -> ActorError {
    ActorError::retry("unknown_message", "This action is not available here.")
}
fn guard<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or_else(|_| {
        Err(ActorError::new(
            "panic",
            "This action could not be completed.",
        ))
    })
}

/// Validate an identity before using it in storage or routing.
pub fn validate_actor_id(id: &ActorId) -> Result<()> {
    validate_token(&id.workspace_id, 128)?;
    validate_token(&id.actor_type, 64)?;
    validate_token(&id.key, 256)?;
    if id.workspace_id.contains('@') || id.actor_type.contains('@') || id.key.contains('@') {
        return Err(ActorError::new(
            "bad_args",
            "A name or identifier is invalid.",
        ));
    }
    Ok(())
}

pub fn validate_name(name: &str) -> Result<()> {
    validate_token(name, 128)
}

fn validate_token(value: &str, max_bytes: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
    {
        Err(ActorError::new(
            "bad_args",
            "A name or identifier is invalid.",
        ))
    } else {
        Ok(())
    }
}

pub fn validate_envelope(message: &Envelope) -> Result<()> {
    validate_name(&message.name)?;
    validate_json(&message.args, MAX_MESSAGE_BYTES)
}

fn validate_time(now: Timestamp) -> Result<()> {
    if now < 0 {
        Err(ActorError::new("bad_args", "The request time is invalid."))
    } else {
        Ok(())
    }
}

/// Bound JSON depth, node count, and encoded size without creating a full copy.
pub fn validate_json(value: &Value, max_bytes: usize) -> Result<()> {
    let mut nodes = 0;
    check_shape(value, 0, &mut nodes)?;
    let mut output = BoundedWriter::new(max_bytes, false);
    serde_json::to_writer(&mut output, value).map_err(|_| output.error())
}

fn check_shape(value: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > MAX_JSON_DEPTH || *nodes > MAX_JSON_NODES {
        return Err(ActorError::new(
            "limit",
            "The data is too deeply nested or has too many values.",
        ));
    }
    match value {
        Value::Array(values) => {
            for value in values {
                check_shape(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                check_shape(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn serialize_value(value: &impl Serialize, max_bytes: usize) -> Result<Value> {
    let mut output = BoundedWriter::new(max_bytes, true);
    serde_json::to_writer(&mut output, value).map_err(|_| output.error())?;
    let value: Value = serde_json::from_slice(&output.bytes)?;
    let mut nodes = 0;
    check_shape(&value, 0, &mut nodes)?;
    Ok(value)
}

/// Limit output as the serializer writes it, including nesting before descent.
struct BoundedWriter {
    bytes: Vec<u8>,
    retain: bool,
    max: usize,
    written: usize,
    depth: usize,
    quoted: bool,
    escaped: bool,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(max: usize, retain: bool) -> Self {
        Self {
            bytes: Vec::new(),
            retain,
            max,
            written: 0,
            depth: 0,
            quoted: false,
            escaped: false,
            exceeded: false,
        }
    }

    fn error(&self) -> ActorError {
        if self.exceeded {
            ActorError::new("limit", "The data is too large or too deeply nested.")
        } else {
            ActorError::new("bad_args", "The data has an invalid format.")
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(io::Error::other("actor size limit"));
        }
        for &byte in bytes {
            if self.quoted {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.quoted = false;
                }
            } else {
                match byte {
                    b'"' => self.quoted = true,
                    b'[' | b'{' => {
                        self.depth += 1;
                        if self.depth > MAX_JSON_DEPTH {
                            self.exceeded = true;
                            return Err(io::Error::other("actor depth limit"));
                        }
                    }
                    b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        self.written += bytes.len();
        if self.retain {
            self.bytes.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
