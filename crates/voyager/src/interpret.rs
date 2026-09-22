//! The interpreter: generated code is runtime data, run bounded.
//!
//! Code-as-action is the paper's core claim — a program represents
//! temporally extended, compositional action in a way primitive
//! commands cannot. The action language is Lua, the embedded engine
//! game scripting grew up on: scripts are data files a model or a
//! skill store authors, never product code, and the engine bounds
//! every one — an instruction budget, a call-depth cap, a memory
//! ceiling, and a wall-clock deadline.
//!
//! A script speaks the environment through host functions that name
//! the bridge's own vocabulary — `state`, `walk`, `explore`, `mine`,
//! `players`, `block_at`, `say`, `wait`, `feedback` — so what may run
//! stays the vocabulary the host owns. The script itself runs on a
//! worker thread; every host call crosses a channel back to the
//! caller's thread, which owns the `&mut` host and so never hands the
//! interpreter a borrow it could outlive. Each crossing is where the
//! caller records its ATIF step.
//!
//! An error is typed for the refinement loop, the way Node's runtime
//! errors were for the paper: [`ScriptError::Parse`] means the source
//! never ran, [`ScriptError::Runtime`] means a host call or a script
//! line refused, [`ScriptError::Exhausted`] means the instruction
//! budget ran out, and [`ScriptError::Timeout`] means the wall clock
//! did.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mlua::LuaSerdeExt;
use serde_json::{Value, json};

use crate::error::{Error, Result};

/// How a script reaches the environment. `Bridge` implements this
/// against the live helper; tests implement it against a stub.
pub trait Host {
    /// One environment call — an op name and its arguments, answered
    /// by the same JSON the bridge would return. An `Err` reaches the
    /// script as a runtime error carrying the op's name.
    fn op(&mut self, op: &str, args: &Value) -> Result<Value>;
    /// The bot's own narration since the last drain — the paper's
    /// `bot.chat` feedback channel. The default is silence.
    fn feedback(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// The bounds one script may spend.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The most Lua instructions the engine may run.
    pub operations: u64,
    /// The deepest a call stack may go.
    pub call_levels: usize,
    /// The most wall seconds a script may run.
    pub wall: Duration,
    /// The most bytes the Lua state may hold.
    pub memory: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            operations: 400_000,
            call_levels: 32,
            wall: Duration::from_secs(120),
            memory: 64 * 1024 * 1024,
        }
    }
}

/// How a script ended badly — the refinement loop's error channel.
#[derive(Clone, Debug)]
pub enum ScriptError {
    /// The source never compiled.
    Parse {
        /// The line and column the parser stopped at.
        position: String,
        /// What the parser said.
        message: String,
    },
    /// A host call or a script line refused.
    Runtime {
        /// The op or line that failed, when known.
        at: String,
        /// What failed.
        message: String,
    },
    /// The instruction budget ran out.
    Exhausted,
    /// The wall clock ran out.
    Timeout,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { position, message } => {
                write!(f, "script does not parse at {position}: {message}")
            }
            Self::Runtime { at, message } => write!(f, "{at}: {message}"),
            Self::Exhausted => write!(f, "the script used its instruction budget"),
            Self::Timeout => write!(f, "the script used its time budget"),
        }
    }
}

/// What a finished script did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// How many host calls it made.
    pub calls: u64,
    /// The chunk's return value, as JSON — a script reports through
    /// `return`.
    pub returned: Value,
}

/// A request from the script thread to the host's thread.
enum Request {
    /// Run an environment op.
    Op {
        op: String,
        args: Value,
        reply: mpsc::Sender<std::result::Result<Value, String>>,
    },
    /// Drain the bot's feedback lines.
    Feedback { reply: mpsc::Sender<Vec<String>> },
}

/// The host the script sees: a channel back to the caller's thread.
#[derive(Clone)]
struct ChannelHost {
    tx: mpsc::Sender<Request>,
}

impl ChannelHost {
    fn op(&self, op: &str, args: &Value) -> std::result::Result<Value, String> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Request::Op {
                op: op.to_string(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| "the host is gone".to_string())?;
        rx.recv().map_err(|_| "the host is gone".to_string())?
    }

    fn feedback(&self) -> Vec<String> {
        let (reply, rx) = mpsc::channel();
        if self.tx.send(Request::Feedback { reply }).is_err() {
            return Vec::new();
        }
        rx.recv().unwrap_or_default()
    }
}

/// Runs `source` against `host` under `limits`.
///
/// The script's own thread runs the engine; this thread serves every
/// host call out of `host`, so the interpreter never holds a borrow.
/// When the script ends — finished or faulted — its request channel
/// closes and the serve loop exits.
///
/// # Errors
///
/// [`ScriptError::Parse`] when the source does not compile;
/// [`ScriptError::Runtime`], [`ScriptError::Exhausted`], or
/// [`ScriptError::Timeout`] when it fails inside its bounds.
pub fn run(
    host: &mut dyn Host,
    source: &str,
    limits: &Limits,
) -> std::result::Result<Outcome, ScriptError> {
    let (tx, rx) = mpsc::channel::<Request>();
    let source = source.to_string();
    let limits = *limits;
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || run_script(ChannelHost { tx }, &source, &limits));
        while let Ok(request) = rx.recv() {
            match request {
                Request::Op { op, args, reply } => {
                    let answer = host
                        .op(&op, &args)
                        .map_err(|error| format!("{op}: {error}"));
                    let _ = reply.send(answer);
                }
                Request::Feedback { reply } => {
                    let _ = reply.send(host.feedback());
                }
            }
        }
        worker.join().unwrap_or_else(|_| {
            Err(ScriptError::Runtime {
                at: "script".to_string(),
                message: "the interpreter thread panicked".to_string(),
            })
        })
    })
}

/// Why a script stopped, set by the hook before it raises — the stop
/// reason rides shared state rather than an error string, so no source
/// text can masquerade as a bound it never hit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    None = 0,
    Exhausted = 1,
    Timeout = 2,
    Depth = 3,
}

/// The hook's shared counters: instructions run, call depth, deadline,
/// and the stop reason it recorded.
struct Gauge {
    instructions: AtomicU64,
    depth: std::sync::atomic::AtomicI64,
    deadline: Instant,
    stop: AtomicU8,
    operations: u64,
    call_levels: i64,
}

/// The engine half of [`run`]: compile, bind, evaluate.
fn run_script(
    host: ChannelHost,
    source: &str,
    limits: &Limits,
) -> std::result::Result<Outcome, ScriptError> {
    let lua = mlua::Lua::new();
    // The cap must hold — a state that cannot take one is refused,
    // never run unbounded.
    lua.set_memory_limit(limits.memory)
        .map_err(|error| ScriptError::Runtime {
            at: "memory limit".to_string(),
            message: error.to_string(),
        })?;
    let gauge = Arc::new(Gauge {
        instructions: AtomicU64::new(0),
        depth: std::sync::atomic::AtomicI64::new(0),
        deadline: Instant::now() + limits.wall,
        stop: AtomicU8::new(Stop::None as u8),
        operations: limits.operations,
        call_levels: limits.call_levels as i64,
    });
    let calls = Arc::new(AtomicU64::new(0));

    // One crossing: build the args JSON at the call site, count the
    // call, ask the host, and turn a refusal into a script error
    // naming the op.
    let globals = lua.globals();
    for name in VOCABULARY {
        let host = host.clone();
        let calls = Arc::clone(&calls);
        let function = lua
            .create_function(move |lua, args: mlua::MultiValue| {
                // The feedback channel drains rather than acts.
                if *name == "feedback" {
                    return lua
                        .create_sequence_from(host.feedback())
                        .map(mlua::Value::Table);
                }
                let json_args: Vec<Value> = args
                    .iter()
                    .map(|arg| lua.from_value::<Value>(arg.clone()).unwrap_or(Value::Null))
                    .collect();
                let Some((op, op_args)) = op_call(name, &json_args) else {
                    return Err(mlua::Error::external(format!("{name}: bad arguments")));
                };
                calls.fetch_add(1, Ordering::Relaxed);
                match host.op(op, &op_args) {
                    Ok(value) => lua.to_value(&value),
                    Err(message) => Err(mlua::Error::external(format!("{op}: {message}"))),
                }
            })
            .map_err(|error| ScriptError::Runtime {
                at: "create_function".to_string(),
                message: error.to_string(),
            })?;
        globals
            .set(*name, function)
            .map_err(|error| ScriptError::Runtime {
                at: format!("globals.set {name}"),
                message: error.to_string(),
            })?;
    }

    // The bounds hook installs after binding — the vocabulary's own
    // setup never spends the script's instruction budget.
    {
        let gauge = Arc::clone(&gauge);
        lua.set_hook(
            mlua::HookTriggers::new()
                .on_calls()
                .on_returns()
                .every_nth_instruction(128),
            move |_lua, debug| {
                use mlua::DebugEvent;
                match debug.event() {
                    DebugEvent::Call => {
                        let depth = gauge.depth.fetch_add(1, Ordering::Relaxed) + 1;
                        if depth > gauge.call_levels {
                            gauge.stop.store(Stop::Depth as u8, Ordering::Relaxed);
                            return Err(mlua::Error::external("call depth"));
                        }
                    }
                    DebugEvent::Ret => {
                        gauge.depth.fetch_sub(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
                let spent = gauge.instructions.fetch_add(128, Ordering::Relaxed) + 128;
                if spent > gauge.operations {
                    gauge.stop.store(Stop::Exhausted as u8, Ordering::Relaxed);
                    return Err(mlua::Error::external("instructions"));
                }
                if Instant::now() >= gauge.deadline {
                    gauge.stop.store(Stop::Timeout as u8, Ordering::Relaxed);
                    return Err(mlua::Error::external("wall clock"));
                }
                Ok(mlua::VmState::Continue)
            },
        )
        .map_err(|error| ScriptError::Runtime {
            at: "hook".to_string(),
            message: error.to_string(),
        })?;
    }

    let before = calls.load(Ordering::Relaxed);
    let result = lua
        .load(source)
        .call::<mlua::MultiValue>(mlua::MultiValue::new());
    let made = calls.load(Ordering::Relaxed) - before;
    let stop = match gauge.stop.load(Ordering::Relaxed) {
        x if x == Stop::Exhausted as u8 => Some(ScriptError::Exhausted),
        x if x == Stop::Timeout as u8 => Some(ScriptError::Timeout),
        x if x == Stop::Depth as u8 => Some(ScriptError::Runtime {
            at: "script".to_string(),
            message: "the script used its call-depth budget".to_string(),
        }),
        _ => None,
    };
    match result {
        Ok(values) => {
            if let Some(stop) = stop {
                return Err(stop);
            }
            let returned = values
                .front()
                .map(|value| {
                    lua.from_value::<Value>(value.clone())
                        .unwrap_or(Value::Null)
                })
                .unwrap_or(Value::Null);
            Ok(Outcome {
                calls: made,
                returned,
            })
        }
        Err(error) => {
            if let Some(stop) = stop {
                return Err(stop);
            }
            if Instant::now() >= gauge.deadline {
                return Err(ScriptError::Timeout);
            }
            Err(match error {
                mlua::Error::SyntaxError { message, .. } => ScriptError::Parse {
                    position: lua_position(&message),
                    message: lua_message(&message),
                },
                other => ScriptError::Runtime {
                    at: "script".to_string(),
                    message: other.to_string(),
                },
            })
        }
    }
}

/// One script call as a bridge op and its JSON args — the shape the
/// host and the ATIF record already speak. `None` means the call's
/// arguments are wrong, a script error the refinement loop can repair.
fn op_call(name: &str, args: &[Value]) -> Option<(&'static str, Value)> {
    let number = |index: usize| args.get(index).and_then(Value::as_i64);
    let text = |index: usize| args.get(index).and_then(Value::as_str);
    Some(match name {
        "say" => ("say", json!({"text": text(0)?.to_string()})),
        "walk" => {
            let seconds = 45;
            if args.len() >= 3 {
                (
                    "goto",
                    json!({"x": number(0)?, "y": number(1)?, "z": number(2)?, "seconds": seconds}),
                )
            } else {
                (
                    "goto",
                    json!({"x": number(0)?, "z": number(1)?, "seconds": seconds}),
                )
            }
        }
        "explore" => (
            "explore",
            json!({"direction": text(0)?, "distance": number(1)?, "seconds": 45}),
        ),
        "mine" => (
            "mine",
            json!({"names": args.first()?.clone(), "count": number(1)?, "radius": 32, "seconds": 60}),
        ),
        "mine_at" => {
            let positions = args.first()?.clone();
            let count = positions.as_array().map(Vec::len).unwrap_or(0);
            (
                "mine",
                json!({"positions": positions, "count": count, "seconds": 60}),
            )
        }
        "players" => ("players", json!({})),
        "state" => ("state", json!({"radius": 24})),
        "block_at" => (
            "block_at",
            json!({"position": [number(0)?, number(1)?, number(2)?]}),
        ),
        "wait" => ("wait", json!({"seconds": number(0)?.clamp(0, 30)})),
        _ => return None,
    })
}

/// Where a Lua syntax error stopped — the `[string "..."]:LINE:`
/// prefix, kept as the record's position.
fn lua_position(message: &str) -> String {
    let mut out = message.split(": ").next().unwrap_or("script").to_string();
    if let Some(line) = message.split(": ").nth(1).and_then(|r| r.split(':').next())
        && line.chars().all(|c| c.is_ascii_digit())
    {
        out = format!("line {line}");
    }
    out
}

/// What a Lua syntax error said, without the position prefix.
fn lua_message(message: &str) -> String {
    match message.rsplit_once(": ") {
        Some((_, tail)) if tail.len() < message.len() => tail.to_string(),
        _ => message.to_string(),
    }
}

/// The ops a script may call, in one place for the prompt and the
/// docs. Anything not listed is not reachable — there is no escape
/// hatch to the filesystem, the network, or the process.
pub const VOCABULARY: &[&str] = &[
    "say", "walk", "explore", "mine", "mine_at", "players", "state", "block_at", "wait", "feedback",
];

/// The host behind a live script: every op is a bridge call with its
/// own deadline, and feedback drains the helper's event stream.
pub struct BridgeHost<'a> {
    /// The live helper.
    pub bridge: &'a mut crate::bridge::Bridge,
    /// The feedback lines drained so far — kept for the record.
    pub drained: VecDeque<String>,
}

impl Host for BridgeHost<'_> {
    fn op(&mut self, op: &str, args: &Value) -> Result<Value> {
        let seconds = match op {
            "state" | "say" | "block_at" | "players" => 30,
            "goto" | "explore" => 75,
            "mine" => 90,
            "wait" => 45,
            other => {
                return Err(Error::episode(format!("unknown op {other:?}")));
            }
        };
        self.bridge
            .call(op, args.clone(), Duration::from_secs(seconds))
    }

    fn feedback(&mut self) -> Vec<String> {
        let lines: Vec<String> = self
            .bridge
            .drain_events()
            .iter()
            .filter_map(|event| event.text("text"))
            .collect();
        self.drained.extend(lines.iter().cloned());
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A host that answers canned JSON and records what was asked.
    struct Stub {
        ops: Vec<(String, Value)>,
        answers: VecDeque<Value>,
        lines: Vec<String>,
    }

    impl Stub {
        fn new() -> Self {
            Stub {
                ops: Vec::new(),
                answers: VecDeque::new(),
                lines: Vec::new(),
            }
        }
    }

    impl Host for Stub {
        fn op(&mut self, op: &str, args: &Value) -> Result<Value> {
            self.ops.push((op.to_string(), args.clone()));
            Ok(self
                .answers
                .pop_front()
                .unwrap_or_else(|| json!({"ok": true})))
        }
        fn feedback(&mut self) -> Vec<String> {
            std::mem::take(&mut self.lines)
        }
    }

    #[test]
    fn a_script_calls_the_host_vocabulary() {
        let mut stub = Stub::new();
        stub.answers
            .push_back(json!({"to": {"x": 5, "y": 0, "z": -3}}));
        let outcome = run(
            &mut stub,
            r#"
                say("heading out")
                walk(5, -3)
                local p = players()
                wait(1)
            "#,
            &Limits::default(),
        )
        .expect("the script runs");
        assert_eq!(outcome.calls, 4);
        assert_eq!(stub.ops[0].0, "say");
        assert_eq!(stub.ops[1].0, "goto");
        assert_eq!(stub.ops[2].0, "players");
        assert_eq!(stub.ops[3].0, "wait");
    }

    #[test]
    fn state_answers_reach_the_script_as_tables() {
        let mut stub = Stub::new();
        stub.answers.push_back(json!({
            "position": {"x": 1.5, "y": 0.0, "z": -2.0},
            "inventory": {"oak_log": 3},
        }));
        let outcome = run(
            &mut stub,
            r#"
                local s = state()
                return s["inventory"]["oak_log"]
            "#,
            &Limits::default(),
        )
        .expect("the script runs");
        assert_eq!(outcome.returned, json!(3));
    }

    #[test]
    fn a_parse_error_is_typed() {
        let mut stub = Stub::new();
        let error = run(&mut stub, "local x = ", &Limits::default()).unwrap_err();
        assert!(matches!(error, ScriptError::Parse { .. }));
        assert!(stub.ops.is_empty());
    }

    #[test]
    fn a_host_refusal_is_a_runtime_error() {
        struct Refusing;
        impl Host for Refusing {
            fn op(&mut self, op: &str, _args: &Value) -> Result<Value> {
                Err(Error::episode(format!("{op} refused")))
            }
        }
        let mut host = Refusing;
        let error = run(&mut host, "walk(0, 0)", &Limits::default()).unwrap_err();
        match error {
            ScriptError::Runtime { message, .. } => assert!(message.contains("goto")),
            other => panic!("expected a runtime error, got {other:?}"),
        }
    }

    #[test]
    fn the_instruction_budget_ends_a_loop() {
        let mut stub = Stub::new();
        let error = run(
            &mut stub,
            "local x = 0 while true do x = x + 1 end",
            &Limits {
                operations: 5_000,
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, ScriptError::Exhausted), "{error:?}");
    }

    #[test]
    fn the_wall_clock_ends_a_busy_script() {
        let mut stub = Stub::new();
        let error = run(
            &mut stub,
            "local x = 0 while true do x = x + 1 end",
            &Limits {
                operations: u64::MAX,
                wall: Duration::from_millis(150),
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, ScriptError::Timeout));
    }

    #[test]
    fn feedback_lines_reach_the_script() {
        let mut stub = Stub::new();
        stub.lines = vec!["mined 2 oak_log".to_string()];
        let outcome =
            run(&mut stub, "return #feedback()", &Limits::default()).expect("the script runs");
        assert_eq!(outcome.returned, json!(1));
    }
}
