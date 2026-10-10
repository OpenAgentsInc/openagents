use super::*;
use serde::Deserialize;

#[derive(Serialize, Deserialize)]
struct Count {
    value: i64,
    nonce: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    value: i64,
}

struct Counter;

impl Actor for Counter {
    const TYPE: &'static str = "test.counter";
    const STATE_VERSION: u32 = 2;
    const PRIVATE: bool = true;
    type State = Count;
    type Input = Start;

    fn create(input: Start, ctx: &mut Ctx) -> Result<Count> {
        ctx.emit("created", json!({ "value": input.value }))?;
        Ok(Count {
            value: input.value,
            nonce: ctx.random(),
        })
    }

    fn wake(_: &Count) -> Result<Self> {
        Ok(Self)
    }

    fn view(state: &Count, _: &Caller) -> Result<Value> {
        Ok(json!({ "value": state.value }))
    }

    fn migrate(from: u32, mut state: Value) -> Result<Value> {
        if from != 1 {
            return Err(ActorError::new(
                "migration",
                "This record cannot be updated.",
            ));
        }
        state["nonce"] = json!(7);
        Ok(state)
    }

    fn input_schema() -> Value {
        json!({"type":"object","required":["value"],"properties":{"value":{"type":"integer"}},"additionalProperties":false})
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Add {
    amount: i64,
}
impl Message for Add {
    const NAME: &'static str = "add@1";
    type Reply = i64;
}
impl Handles<Add> for Counter {
    fn handle(&mut self, state: &mut Count, message: Add, ctx: &mut Ctx) -> Result<i64> {
        state.value = state
            .value
            .checked_add(message.amount)
            .ok_or_else(|| ActorError::new("limit", "The number is too large."))?;
        ctx.emit("changed", json!({"value":state.value}))?;
        Ok(state.value)
    }
}

macro_rules! empty_message {
    ($name:ident, $wire:literal, $access:expr, $read:expr) => {
        #[derive(Deserialize)]
        struct $name {}
        impl Message for $name {
            const NAME: &'static str = $wire;
            const ACCESS: Access = $access;
            const READ_ONLY: bool = $read;
            type Reply = i64;
        }
    };
}

empty_message!(Read, "read@1", Access::Member, true);
empty_message!(ReadMutates, "read-mutates@1", Access::Member, true);
empty_message!(ReadEmits, "read-emits@1", Access::Member, true);
empty_message!(Fails, "fails@1", Access::Member, false);
empty_message!(Panics, "panics@1", Access::Member, false);
empty_message!(Overflows, "overflows@1", Access::Member, false);
empty_message!(WorkDone, "WorkDone", Access::Internal, false);
empty_message!(OwnerOnly, "owner@1", Access::Owner, false);
empty_message!(AdminOnly, "admin@1", Access::Admin, false);
empty_message!(ServiceOnly, "service@1", Access::Service, false);
empty_message!(AccountOnly, "account@1", Access::AccountOwner, false);
empty_message!(ForgedCompletion, "WorkDone@2", Access::Member, false);

macro_rules! reads_count {
    ($($message:ty),+ $(,)?) => {
        $(impl Handles<$message> for Counter {
            fn handle(&mut self, state: &mut Count, _: $message, _: &mut Ctx) -> Result<i64> { Ok(state.value) }
        })+
    };
}
reads_count!(
    Read,
    WorkDone,
    OwnerOnly,
    AdminOnly,
    ServiceOnly,
    AccountOnly,
    ForgedCompletion
);

impl Handles<ReadMutates> for Counter {
    fn handle(&mut self, state: &mut Count, _: ReadMutates, _: &mut Ctx) -> Result<i64> {
        state.value += 1;
        Ok(state.value)
    }
}
impl Handles<ReadEmits> for Counter {
    fn handle(&mut self, state: &mut Count, _: ReadEmits, ctx: &mut Ctx) -> Result<i64> {
        let _ = ctx.emit("ignored", Value::Null);
        Ok(state.value)
    }
}
impl Handles<Fails> for Counter {
    fn handle(&mut self, state: &mut Count, _: Fails, ctx: &mut Ctx) -> Result<i64> {
        state.value = 99;
        ctx.emit("changed", json!(99))?;
        Err(ActorError::new("denied", "This change was rejected."))
    }
}
impl Handles<Panics> for Counter {
    fn handle(&mut self, state: &mut Count, _: Panics, ctx: &mut Ctx) -> Result<i64> {
        state.value = 99;
        ctx.emit("changed", json!(99))?;
        panic!("deliberate test panic")
    }
}
impl Handles<Overflows> for Counter {
    fn handle(&mut self, state: &mut Count, _: Overflows, ctx: &mut Ctx) -> Result<i64> {
        for _ in 0..=MAX_COMMANDS {
            let _ = ctx.emit("changed", Value::Null);
        }
        state.value = 99;
        Ok(state.value)
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(actor_messages!(Counter; Add, Read, ReadMutates, ReadEmits, Fails, Panics, Overflows, WorkDone, OwnerOnly, AdminOnly, ServiceOnly, AccountOnly)).unwrap();
    registry
}
fn caller() -> Caller {
    Caller {
        principal: "principal-a".into(),
        workspace_id: "workspace-a".into(),
        account_id: Some("account-a".into()),
        role: Role::Member,
        executor: None,
    }
}
fn id() -> ActorId {
    ActorId {
        workspace_id: "workspace-a".into(),
        actor_type: Counter::TYPE.into(),
        key: "counter-a".into(),
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        uid: "0123456789abcdef".into(),
        id: id(),
        owner: Some("account-a".into()),
        status: Status::Live,
        state_version: 2,
        version: 4,
        event_seq: 1,
        inbox_seq: 0,
        state: json!({ "value": 3, "nonce": 9 }),
        created_at: 100,
        updated_at: 200,
    }
}
fn message(name: &str) -> Envelope {
    Envelope {
        name: name.into(),
        args: json!({}),
        origin: Origin::Action,
    }
}
fn ctx(version: u64) -> Ctx {
    Ctx::new(&id(), "0123456789abcdef", &caller(), version, 1234, false)
}

#[test]
fn typed_create_apply_and_view_prepare_without_mutating_snapshot() {
    let registry = registry();
    let first = registry
        .create(
            &id(),
            "0123456789abcdef",
            json!({"value":5}),
            &caller(),
            1000,
        )
        .unwrap();
    let retry = registry
        .create(
            &id(),
            "0123456789abcdef",
            json!({"value":5}),
            &caller(),
            1000,
        )
        .unwrap();
    assert_eq!(first.state, retry.state);
    assert_eq!(first.state["value"], 5);
    assert_eq!(first.state_version, 2);
    assert_eq!(first.commands.len(), 1);
    assert!(registry.is_private(Counter::TYPE).unwrap());
    let snapshot = snapshot();
    let envelope = Envelope {
        name: Add::NAME.into(),
        args: json!({"amount":2}),
        origin: Origin::Action,
    };
    let prepared = registry
        .apply(&snapshot, &envelope, &caller(), 2000)
        .unwrap();
    assert_eq!(prepared.state["value"], 5);
    assert_eq!(prepared.reply, 5);
    assert_eq!(snapshot.state["value"], 3);
    assert!(!prepared.read_only);
    assert_eq!(
        registry.view(&snapshot, &caller()).unwrap(),
        json!({"value":3})
    );
    let invalid = Envelope {
        args: json!({"amount":2,"extra":3}),
        ..envelope
    };
    assert_eq!(
        registry
            .apply(&snapshot, &invalid, &caller(), 2000)
            .unwrap_err()
            .code,
        "bad_args"
    );
}

#[test]
fn all_registered_messages_hide_cross_workspace_and_private_state() {
    let registry = registry();
    let snapshot = snapshot();
    for definition in registry.contract()["types"][0]["messages"]
        .as_array()
        .unwrap()
    {
        let message = message(definition["name"].as_str().unwrap());
        let mut foreign = caller();
        foreign.workspace_id = "workspace-b".into();
        assert_eq!(
            registry
                .authorize(&snapshot, &message, &foreign)
                .unwrap_err()
                .code,
            "not_found"
        );
        foreign.workspace_id = snapshot.id.workspace_id.clone();
        foreign.account_id = Some("account-b".into());
        for role in [Role::Member, Role::Owner, Role::Service, Role::Admin] {
            foreign.role = role;
            assert_eq!(
                registry
                    .authorize(&snapshot, &message, &foreign)
                    .unwrap_err()
                    .code,
                "not_found"
            );
            assert_eq!(
                registry.view(&snapshot, &foreign).unwrap_err().code,
                "not_found"
            );
        }
    }
    let mut anonymous = caller();
    anonymous.account_id = None;
    assert_eq!(
        registry
            .create(&id(), "uid", json!({"value":0}), &anonymous, 0)
            .unwrap_err()
            .code,
        "forbidden"
    );
}

#[test]
fn access_and_origin_cannot_be_promoted_by_message_data() {
    let registry = registry();
    let snapshot = snapshot();
    let mut caller = caller();
    for name in [
        OwnerOnly::NAME,
        AdminOnly::NAME,
        ServiceOnly::NAME,
        WorkDone::NAME,
    ] {
        assert_eq!(
            registry
                .authorize(&snapshot, &message(name), &caller)
                .unwrap_err()
                .code,
            "forbidden"
        );
    }
    assert!(
        registry
            .authorize(&snapshot, &message(AccountOnly::NAME), &caller)
            .is_ok()
    );
    caller.role = Role::Owner;
    assert!(
        registry
            .authorize(&snapshot, &message(OwnerOnly::NAME), &caller)
            .is_ok()
    );
    caller.role = Role::Admin;
    assert!(
        registry
            .authorize(&snapshot, &message(AdminOnly::NAME), &caller)
            .is_ok()
    );
    assert!(
        registry
            .authorize(&snapshot, &message(ServiceOnly::NAME), &caller)
            .is_err()
    );
    let mut internal = message(WorkDone::NAME);
    internal.origin = Origin::Internal;
    caller.role = Role::Member;
    assert_eq!(
        registry
            .authorize(&snapshot, &internal, &caller)
            .unwrap_err()
            .code,
        "forbidden"
    );
    caller.role = Role::Service;
    assert!(registry.authorize(&snapshot, &internal, &caller).is_ok());
    assert!(
        registry
            .authorize(&snapshot, &message(ServiceOnly::NAME), &caller)
            .is_ok()
    );
    internal.name = Read::NAME.into();
    assert_eq!(
        registry
            .authorize(&snapshot, &internal, &caller)
            .unwrap_err()
            .code,
        "forbidden"
    );
}

#[test]
fn failures_panics_and_ignored_command_errors_never_prepare_a_commit() {
    let registry = registry();
    let snapshot = snapshot();
    for (name, code) in [
        (Fails::NAME, "denied"),
        (Panics::NAME, "panic"),
        (Overflows::NAME, "limit"),
    ] {
        let error = registry
            .apply(&snapshot, &message(name), &caller(), 1000)
            .unwrap_err();
        assert_eq!(error.code, code);
        assert!(!error.message.contains("deliberate"));
        assert_eq!(snapshot.state["value"], 3);
    }
}

#[test]
fn read_only_state_and_command_mutations_are_rejected() {
    let registry = registry();
    let snapshot = snapshot();
    let prepared = registry
        .apply(&snapshot, &message(Read::NAME), &caller(), 1000)
        .unwrap();
    assert!(prepared.read_only);
    assert_eq!(prepared.reply, 3);
    assert!(prepared.commands.is_empty());
    for name in [ReadMutates::NAME, ReadEmits::NAME] {
        assert_eq!(
            registry
                .apply(&snapshot, &message(name), &caller(), 1000)
                .unwrap_err()
                .code,
            "read_only"
        );
    }
}

#[test]
fn blocked_actors_allow_reads_and_destroyed_actors_allow_nothing() {
    let registry = registry();
    let mut snapshot = snapshot();
    snapshot.status = Status::Blocked;
    assert!(registry.view(&snapshot, &caller()).is_ok());
    assert!(
        registry
            .apply(&snapshot, &message(Read::NAME), &caller(), 1000)
            .is_ok()
    );
    let add = Envelope {
        name: Add::NAME.into(),
        args: json!({"amount":1}),
        origin: Origin::Action,
    };
    assert_eq!(
        registry
            .apply(&snapshot, &add, &caller(), 1000)
            .unwrap_err()
            .code,
        "blocked"
    );
    snapshot.status = Status::Destroyed;
    assert_eq!(
        registry.view(&snapshot, &caller()).unwrap_err().code,
        "not_found"
    );
    assert_eq!(
        registry
            .apply(&snapshot, &message(Read::NAME), &caller(), 1000)
            .unwrap_err()
            .code,
        "not_found"
    );
}

#[test]
fn migration_is_transient_for_reads_and_newer_versions_are_retryable() {
    let registry = registry();
    let mut snapshot = snapshot();
    snapshot.state_version = 1;
    snapshot.state = json!({"value":8});
    let prepared = registry
        .apply(&snapshot, &message(Read::NAME), &caller(), 1000)
        .unwrap();
    assert_eq!(prepared.state, json!({"value":8}));
    assert_eq!(prepared.state_version, 1);
    assert!(prepared.read_only);
    assert_eq!(snapshot.state, json!({"value":8}));
    let add = Envelope {
        name: Add::NAME.into(),
        args: json!({"amount":1}),
        origin: Origin::Action,
    };
    let changed = registry.apply(&snapshot, &add, &caller(), 1000).unwrap();
    assert_eq!(changed.state, json!({"value":9,"nonce":7}));
    assert_eq!(changed.state_version, 2);
    assert_eq!(
        registry.view(&snapshot, &caller()).unwrap(),
        json!({"value":8})
    );
    snapshot.state_version = 3;
    let error = registry
        .apply(&snapshot, &message(Read::NAME), &caller(), 1000)
        .unwrap_err();
    assert_eq!(error.code, "version_ahead");
    assert!(error.retryable);
    snapshot.state_version = 0;
    assert_eq!(
        registry.view(&snapshot, &caller()).unwrap_err().code,
        "migration"
    );
}

#[test]
fn ids_and_randomness_replay_but_differ_across_versions_and_command_positions() {
    fn work() -> WorkSpec {
        WorkSpec {
            item_id: "ignored".into(),
            queue: "mac.build@1".into(),
            target: Some("studio".into()),
            payload: json!({"run":"run-a"}),
            lease_ms: 50,
            max_attempts: 3,
            retry: RetryPolicy::Reconcile,
        }
    }
    let mut first = ctx(1);
    let mut retry = ctx(1);
    assert_eq!(first.random(), retry.random());
    assert_eq!(first.random(), retry.random());
    assert_eq!(first.work(work()).unwrap(), retry.work(work()).unwrap());
    let next_id = first.work(work()).unwrap();
    assert_ne!(next_id, retry.command_id("work-other"));
    let mut next_version = ctx(2);
    assert_ne!(retry.random(), next_version.random());
    assert_ne!(next_id, next_version.work(work()).unwrap());
    assert_ne!(ctx(0).work(work()).unwrap(), ctx(1).work(work()).unwrap());
    let commands = first.finish().unwrap();
    let [Command::Work { work: a }, Command::Work { work: b }] = commands.as_slice() else {
        panic!("expected work");
    };
    assert_ne!(a.item_id, b.item_id);
    assert_ne!(a.item_id, "ignored");
}

#[test]
fn sends_and_alarms_cannot_forge_internal_or_cross_workspace_authority() {
    let mut inbox = message(Read::NAME);
    inbox.origin = Origin::Inbox;
    let mut context = ctx(1);
    assert!(context.send(id(), inbox.clone()).is_ok());
    let mut foreign = id();
    foreign.workspace_id = "workspace-b".into();
    assert_eq!(
        ctx(1).send(foreign, inbox.clone()).unwrap_err().code,
        "not_found"
    );
    inbox.origin = Origin::Internal;
    inbox.name = WorkDone::NAME.into();
    assert_eq!(
        ctx(1).send(id(), inbox.clone()).unwrap_err().code,
        "forbidden"
    );
    let alarm = AlarmSpec {
        name: "wake".into(),
        due_at: 1,
        message: inbox,
        interval_ms: None,
    };
    assert_eq!(ctx(1).schedule(alarm).unwrap_err().code, "forbidden");
    context.destroy().unwrap();
    assert_eq!(
        context.emit("late", Value::Null).unwrap_err().code,
        "destroyed"
    );
    assert_eq!(context.finish().unwrap_err().code, "destroyed");
}

#[test]
fn invalid_limits_poison_the_whole_transition_even_if_ignored() {
    let mut context = ctx(1);
    let effect = EffectSpec {
        id: "ignored".into(),
        kind: "provider.call@1".into(),
        payload: Value::Null,
        timeout_ms: 0,
        max_attempts: 1,
        retry: RetryPolicy::Idempotent,
    };
    let _ = context.effect(effect);
    assert!(context.finish().is_err());
    let mut context = ctx(1);
    for _ in 0..=MAX_RANDOM_CALLS {
        let _ = context.random();
    }
    assert_eq!(context.finish().unwrap_err().code, "limit");
    let mut context = ctx(1);
    let _ = context.emit("too-big", "x".repeat(MAX_PAYLOAD_BYTES));
    assert_eq!(context.finish().unwrap_err().code, "limit");
}

#[test]
fn encoded_sizes_depth_and_node_limits_are_checked_without_an_unbounded_copy() {
    assert!(validate_json(&json!("abc"), 5).is_ok());
    assert_eq!(validate_json(&json!("abc"), 4).unwrap_err().code, "limit");
    assert!(validate_json(&json!("\"\\\n"), 8).is_ok());
    assert!(validate_json(&json!("[]{}\""), 20).is_ok());
    let mut nested = Value::Null;
    for _ in 0..=MAX_JSON_DEPTH {
        nested = json!([nested]);
    }
    assert_eq!(
        validate_json(&nested, MAX_STATE_BYTES).unwrap_err().code,
        "limit"
    );
    assert_eq!(
        serialize_value(&nested, MAX_STATE_BYTES).unwrap_err().code,
        "limit"
    );
    let nodes = Value::Array(vec![Value::Null; MAX_JSON_NODES]);
    assert_eq!(
        validate_json(&nodes, MAX_STATE_BYTES).unwrap_err().code,
        "limit"
    );
    let escaped = json!({"text":"brackets [] {}, escaped quote \\"});
    assert_eq!(serialize_value(&escaped, 1024).unwrap(), escaped);
}

#[test]
fn registration_is_explicit_duplicate_safe_and_clone_isolated() {
    let mut registry = Registry::new();
    let clone = registry.clone();
    assert_eq!(
        registry
            .register(actor_messages!(Counter; ForgedCompletion))
            .unwrap_err()
            .code,
        "definition"
    );
    assert_eq!(
        registry
            .register(actor_messages!(Counter; Add, Add))
            .unwrap_err()
            .code,
        "duplicate_message"
    );
    assert_eq!(registry.contract()["types"], json!([]));
    registry.register(actor_messages!(Counter; Add)).unwrap();
    assert_eq!(
        registry
            .register(actor_messages!(Counter; Read))
            .unwrap_err()
            .code,
        "duplicate_type"
    );
    assert!(clone.is_private(Counter::TYPE).is_err());
    let contract = registry.contract();
    assert_eq!(contract["types"][0]["messages"][0]["name"], Add::NAME);
    assert_eq!(contract["types"][0]["messages"][0]["access"], "member");
    assert_eq!(contract["types"][0]["input"]["type"], "object");
}

#[test]
fn names_bounds_and_unknown_messages_fail_before_handlers() {
    assert!(validate_name("work.finish@1").is_ok());
    for name in ["", "new\nline", "unicode-☃", "a b"] {
        assert!(validate_name(name).is_err());
    }
    let mut invalid = id();
    invalid.key = "k".repeat(257);
    assert!(validate_actor_id(&invalid).is_err());
    let registry = registry();
    let error = registry
        .apply(&snapshot(), &message("new@2"), &caller(), 1000)
        .unwrap_err();
    assert_eq!(error.code, "unknown_message");
    assert!(error.retryable);
    let mut exhausted = snapshot();
    exhausted.version = u64::MAX;
    assert_eq!(
        registry
            .apply(&exhausted, &message(Read::NAME), &caller(), 1000)
            .unwrap_err()
            .code,
        "limit"
    );
}
