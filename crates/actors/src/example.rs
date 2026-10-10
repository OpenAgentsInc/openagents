//! A small example actor for local demonstrations and integration tests.
//! Applications register their own types; this counter has no product integration.
use crate::{
    core::{Actor, Ctx, Definition, Handles, Message, Registry},
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct Counter;
#[derive(Serialize, Deserialize)]
pub struct CounterState {
    pub value: i64,
    pub completed: u64,
}
#[derive(Serialize, Deserialize)]
pub struct CounterInput {
    #[serde(default)]
    pub initial: i64,
}
impl Actor for Counter {
    const TYPE: &'static str = "example.counter";
    const STATE_VERSION: u32 = 1;
    const PRIVATE: bool = true;
    type State = CounterState;
    type Input = CounterInput;
    fn create(input: CounterInput, _ctx: &mut Ctx) -> Result<CounterState> {
        Ok(CounterState {
            value: input.initial,
            completed: 0,
        })
    }
    fn wake(_state: &CounterState) -> Result<Self> {
        Ok(Self)
    }
    fn view(state: &CounterState, _caller: &Caller) -> Result<Value> {
        Ok(json!({"value":state.value,"completed":state.completed}))
    }
    fn description() -> &'static str {
        "A private counter that demonstrates durable commands."
    }
    fn input_schema() -> Value {
        json!({"type":"object","properties":{"initial":{"type":"integer"}}})
    }
    fn state_schema() -> Value {
        json!({"type":"object","required":["value","completed"],"properties":{"value":{"type":"integer"},"completed":{"type":"integer"}}})
    }
    fn view_schema() -> Value {
        Self::state_schema()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Add {
    pub delta: i64,
}
impl Message for Add {
    const NAME: &'static str = "add@1";
    type Reply = i64;
    fn schema() -> Value {
        json!({"type":"object","required":["delta"],"properties":{"delta":{"type":"integer"}},"additionalProperties":false})
    }
}
impl Handles<Add> for Counter {
    fn handle(&mut self, state: &mut CounterState, msg: Add, ctx: &mut Ctx) -> Result<i64> {
        state.value = state
            .value
            .checked_add(msg.delta)
            .ok_or_else(|| ActorError::new("bad_args", "The total is too large."))?;
        ctx.emit("changed@1", json!({"value":state.value}))?;
        Ok(state.value)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {}
impl Message for Read {
    const NAME: &'static str = "read@1";
    const READ_ONLY: bool = true;
    type Reply = i64;
}
impl Handles<Read> for Counter {
    fn handle(&mut self, state: &mut CounterState, _msg: Read, _ctx: &mut Ctx) -> Result<i64> {
        Ok(state.value)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Later {
    pub delta: i64,
    pub delay_ms: i64,
}
impl Message for Later {
    const NAME: &'static str = "later@1";
    type Reply = ();
}
impl Handles<Later> for Counter {
    fn handle(&mut self, _state: &mut CounterState, msg: Later, ctx: &mut Ctx) -> Result<()> {
        if !(0..=86_400_000).contains(&msg.delay_ms) {
            return Err(ActorError::new(
                "bad_args",
                "Choose a delay of at most one day.",
            ));
        }
        ctx.schedule(AlarmSpec {
            name: "add".into(),
            due_at: ctx.now().saturating_add(msg.delay_ms),
            message: Envelope {
                name: Add::NAME.into(),
                args: json!({"delta":msg.delta}),
                origin: Origin::Inbox,
            },
            interval_ms: None,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub queue: String,
    #[serde(default)]
    pub target: Option<String>,
}
impl Message for Offer {
    const NAME: &'static str = "offer@1";
    type Reply = String;
}
impl Handles<Offer> for Counter {
    fn handle(&mut self, _state: &mut CounterState, msg: Offer, ctx: &mut Ctx) -> Result<String> {
        ctx.work(WorkSpec {
            item_id: String::new(),
            queue: msg.queue,
            target: msg.target,
            payload: json!({"task":"example"}),
            lease_ms: 1000,
            max_attempts: 3,
            retry: RetryPolicy::Reconcile,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkDone(pub Value);
impl Message for WorkDone {
    const NAME: &'static str = "WorkDone";
    const ACCESS: Access = Access::Internal;
    type Reply = u64;
}
impl Handles<WorkDone> for Counter {
    fn handle(&mut self, state: &mut CounterState, _msg: WorkDone, _ctx: &mut Ctx) -> Result<u64> {
        state.completed = state.completed.saturating_add(1);
        Ok(state.completed)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkExpired(pub Value);
impl Message for WorkExpired {
    const NAME: &'static str = "WorkExpired";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<WorkExpired> for Counter {
    fn handle(
        &mut self,
        _state: &mut CounterState,
        _msg: WorkExpired,
        _ctx: &mut Ctx,
    ) -> Result<()> {
        Ok(())
    }
}
pub fn registry() -> Result<Registry> {
    let mut registry = Registry::new();
    registry.register(
        Definition::<Counter>::new()
            .message::<Add>()
            .message::<Read>()
            .message::<Later>()
            .message::<Offer>()
            .message::<WorkDone>()
            .message::<WorkExpired>(),
    )?;
    Ok(registry)
}
