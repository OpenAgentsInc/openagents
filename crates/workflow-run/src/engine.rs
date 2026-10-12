//! The JavaScript host for one workflow run.
//!
//! `pipeline` and `parallel` are script. `agent` is the one native call that
//! starts a thread. The script cannot read the clock, draw a random number,
//! or start another run.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use boa_engine::job::{JobExecutor, SimpleJobExecutor};
use boa_engine::native_function::NativeFunction;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, JsValue, Source, js_string};
use serde_json::{Map, Value, json};
use tokio::sync::Semaphore;

use crate::{
    Admit, EndResult, FinishKind, InlineAgent, MAX_IN_FLIGHT, Outcome, Prepared, Session,
    ThreadCall, ThreadResult, ThreadRunner, cut, digest, next_slot,
};

const PRELUDE: &str = r#"
function pipeline(items) {
  const stages = Array.prototype.slice.call(arguments, 1);
  const list = Array.isArray(items) ? items : [];
  return Promise.all(list.map(async function (item, index) {
    let prev = item;
    for (let n = 0; n < stages.length; n += 1) {
      try {
        prev = await stages[n](prev, item, index);
        if (prev === undefined) prev = null;
      } catch (error) {
        return null;
      }
    }
    return prev;
  }));
}

function parallel(thunks) {
  const list = Array.isArray(thunks) ? thunks : [];
  return Promise.all(list.map(async function (thunk) {
    try {
      const value = typeof thunk === "function" ? await thunk() : await thunk;
      return value === undefined ? null : value;
    } catch (error) {
      return null;
    }
  }));
}

const __clockMessage = "A workflow script does not read the clock. Pass a timestamp in args.";
if (typeof Date !== "undefined") {
  const __RealDate = Date;
  function __WorkflowDate(...args) {
    if (args.length === 0) throw new Error(__clockMessage);
    return new __RealDate(...args);
  }
  __WorkflowDate.now = function () { throw new Error(__clockMessage); };
  __WorkflowDate.parse = __RealDate.parse;
  __WorkflowDate.UTC = __RealDate.UTC;
  __WorkflowDate.prototype = __RealDate.prototype;
  globalThis.Date = __WorkflowDate;
}
Math.random = function () {
  throw new Error("A workflow script does not draw random numbers. Put the index in the prompt.");
};

const budget = {
  get total() { return __budget_total(); },
  spent() { return __budget_spent(); },
  remaining() {
    const total = __budget_total();
    if (total === null) return Infinity;
    return Math.max(0, total - __budget_spent());
  },
};
"#;

const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

struct Active {
    session: Session,
    run_id: String,
    script_key: String,
    cap: u32,
    inline_agents: bool,
    predefined: HashSet<String>,
    runner: ThreadRunner,
    semaphore: Arc<Semaphore>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Copy, Clone)]
struct Slot(u64);

fn slots() -> &'static Mutex<HashMap<u64, Arc<Active>>> {
    static SLOTS: std::sync::LazyLock<Mutex<HashMap<u64, Arc<Active>>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    &SLOTS
}

fn lookup(slot: Slot) -> Option<Arc<Active>> {
    slots()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&slot.0)
        .cloned()
}

struct SlotGuard(u64);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        slots()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.0);
    }
}

struct FinishGuard {
    session: Session,
    id: String,
    open: bool,
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        if self.open {
            let _ = self.session.complete(
                &self.id,
                EndResult::error("unknown_error", "The run failed."),
                Value::Null,
            );
        }
    }
}

struct Hold {
    active: Arc<Active>,
    key: String,
    armed: bool,
}

impl Drop for Hold {
    fn drop(&mut self) {
        if self.armed {
            self.active
                .session
                .store(&self.active.script_key, &self.key, ThreadResult::dead());
        }
    }
}

struct ParsedCall {
    prompt: String,
    label: Option<String>,
    phase: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    agent_type: Option<String>,
    agent: Option<InlineAgent>,
    schema: Option<Value>,
    key: String,
}

pub(crate) async fn execute(session: Session, prepared: Prepared, runner: ThreadRunner) -> Outcome {
    let id = prepared.id.clone();
    let mut guard = FinishGuard {
        session: session.clone(),
        id: id.clone(),
        open: true,
    };
    let timer_session = session.clone();
    let timer_id = id.clone();
    let wait = prepared.deadline.saturating_duration_since(Instant::now());
    let timer = tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        timer_session.mark_timeout(&timer_id);
    });
    let outcome = drive(&session, prepared, runner).await;
    timer.abort();
    guard.open = false;
    outcome
}

async fn drive(session: &Session, prepared: Prepared, runner: ThreadRunner) -> Outcome {
    let id = prepared.id.clone();
    if Instant::now() >= prepared.deadline {
        return settle(session, &id, Value::Null, None);
    }
    let slot_id = next_slot();
    let active = Arc::new(Active {
        session: session.clone(),
        run_id: id.clone(),
        script_key: prepared.script_key.clone(),
        cap: prepared.cap,
        inline_agents: prepared.inline_agents,
        predefined: prepared.predefined.clone(),
        runner,
        semaphore: Arc::new(Semaphore::new(MAX_IN_FLIGHT as usize)),
        cancel: Arc::clone(&prepared.cancel),
    });
    slots()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(slot_id, Arc::clone(&active));
    let _slot = SlotGuard(slot_id);

    let built = match script_host(&prepared, Slot(slot_id)).await {
        Ok(found) => found,
        Err(message) => return settle(session, &id, Value::Null, Some(message)),
    };
    settle(session, &id, built.0, built.1)
}

fn settle(session: &Session, id: &str, value: Value, error: Option<String>) -> Outcome {
    match session.finish_kind(id) {
        FinishKind::Timeout => session.complete(
            id,
            EndResult::error("timeout_error", "The run reached its lifetime."),
            Value::Null,
        ),
        FinishKind::ThreadLimit => session.complete(
            id,
            EndResult::error("thread_limit_error", "The run reached its thread limit."),
            Value::Null,
        ),
        FinishKind::Stopped => session.complete(id, EndResult::stopped(), Value::Null),
        FinishKind::Budget => session.pause(id, value),
        FinishKind::Completed => match error {
            Some(message) => {
                session.complete(id, EndResult::error("program_error", message), Value::Null)
            }
            None => session.complete(id, EndResult::completed(), value),
        },
    }
}

async fn script_host(prepared: &Prepared, slot: Slot) -> Result<(Value, Option<String>), String> {
    let executor = Rc::new(SimpleJobExecutor::new());
    let mut context = Context::builder()
        .job_executor(Rc::clone(&executor))
        .build()
        .map_err(|_| "The script could not start.".to_string())?;
    context.insert_data(slot);
    let mut limits = context.runtime_limits();
    limits.set_loop_iteration_limit(2_000_000);
    context.set_runtime_limits(limits);
    register(&mut context).map_err(|_| "The script could not start.".to_string())?;
    let source = source_text(&prepared.body, &prepared.args);
    context
        .eval(Source::from_bytes(source.as_bytes()))
        .map_err(|error| js_message(&error))?;
    let jobs = {
        let cell = RefCell::new(&mut context);
        executor.run_jobs_async(&cell).await
    };
    if let Err(error) = jobs {
        return Ok((Value::Null, Some(js_message(&error))));
    }
    read_finish(&mut context)
}

fn register(context: &mut Context) -> JsResult<()> {
    context.register_global_builtin_callable(
        js_string!("agent"),
        2,
        NativeFunction::from_async_fn(agent),
    )?;
    context.register_global_builtin_callable(
        js_string!("phase"),
        1,
        NativeFunction::from_fn_ptr(phase),
    )?;
    context.register_global_builtin_callable(
        js_string!("log"),
        1,
        NativeFunction::from_fn_ptr(log),
    )?;
    context.register_global_builtin_callable(
        js_string!("__budget_total"),
        0,
        NativeFunction::from_fn_ptr(budget_total),
    )?;
    context.register_global_builtin_callable(
        js_string!("__budget_spent"),
        0,
        NativeFunction::from_fn_ptr(budget_spent),
    )?;
    Ok(())
}

fn source_text(body: &str, args: &Value) -> String {
    let args = serde_json::to_string(args).unwrap_or_else(|_| "null".into());
    format!(
        "{PRELUDE}\nconst args = {args};\nglobalThis.__workflow_done = false;\nglobalThis.__workflow_failed = false;\nglobalThis.__workflow_error = '';\nglobalThis.__workflow_value = null;\ntry {{\nconst __workflow_promise = (async () => {{\n{body}\n}})();\n__workflow_promise.then((value) => {{\nglobalThis.__workflow_value = value === undefined ? null : value;\nglobalThis.__workflow_done = true;\n}}, (error) => {{\nglobalThis.__workflow_failed = true;\nglobalThis.__workflow_error = String((error && error.message) || error);\nglobalThis.__workflow_done = true;\n}});\n}} catch (error) {{\nglobalThis.__workflow_failed = true;\nglobalThis.__workflow_error = String((error && error.message) || error);\nglobalThis.__workflow_done = true;\n}}\n"
    )
}

fn read_finish(context: &mut Context) -> Result<(Value, Option<String>), String> {
    let failed = global_bool(context, "__workflow_failed")?;
    let done = global_bool(context, "__workflow_done")?;
    if failed {
        let message = global_text(context, "__workflow_error")?.unwrap_or_default();
        let message = if message.is_empty() {
            "The script failed.".to_string()
        } else {
            cut(&message, 400)
        };
        return Ok((Value::Null, Some(message)));
    }
    if !done {
        return Ok((Value::Null, Some("The script did not finish.".into())));
    }
    let value = global_json(context, "__workflow_value")?;
    Ok((value, None))
}

async fn agent(
    _this: &JsValue,
    args: &[JsValue],
    context: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let slot = context
        .borrow()
        .get_data::<Slot>()
        .copied()
        .ok_or_else(|| fail("The run failed."))?;
    let Some(active) = lookup(slot) else {
        return Err(fail("The run failed."));
    };
    let parsed = {
        let mut context = context.borrow_mut();
        parse_call(args, &active, &mut context)?
    };
    let value = run_call(&active, parsed).await;
    let mut context = context.borrow_mut();
    match value {
        None => Ok(JsValue::null()),
        Some(value) => JsValue::from_json(&value, &mut context),
    }
}

async fn run_call(active: &Arc<Active>, call: ParsedCall) -> Option<Value> {
    loop {
        match active
            .session
            .admit(&active.script_key, &call.key, &active.run_id, active.cap)
        {
            Admit::Ready(result) => return result.value,
            Admit::Wait => {
                tokio::select! {
                    _ = active.session.notified() => {}
                    _ = tokio::time::sleep(std::time::Duration::from_millis(15)) => {}
                }
            }
            Admit::Skip => return None,
            Admit::Start => break,
        }
    }
    let mut hold = Hold {
        active: Arc::clone(active),
        key: call.key.clone(),
        armed: true,
    };
    let permit = tokio::select! {
        biased;
        _ = active.session.until_stopped(&active.run_id) => return None,
        permit = Arc::clone(&active.semaphore).acquire_owned() => permit,
    };
    let Ok(_permit) = permit else {
        return None;
    };
    if active.session.is_stopped(&active.run_id) {
        return None;
    }
    let thread = ThreadCall {
        prompt: call.prompt,
        label: call.label,
        phase: call.phase,
        model: call.model,
        effort: call.effort,
        agent_type: call.agent_type,
        agent: call.agent,
        schema: call.schema.clone(),
        cancel: Arc::clone(&active.cancel),
    };
    let runner = Arc::clone(&active.runner);
    let result = tokio::select! {
        biased;
        _ = active.session.until_stopped(&active.run_id) => ThreadResult::dead(),
        result = runner(thread) => result,
    };
    let result = if let Some(schema) = &call.schema {
        match &result.value {
            Some(value) if satisfies(schema, value) => result,
            _ => ThreadResult::dead(),
        }
    } else {
        result
    };
    active.session.add_spent(result.cost_sats);
    let value = result.value.clone();
    active.session.store(&active.script_key, &call.key, result);
    hold.armed = false;
    value
}

fn phase(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(slot) = context.get_data::<Slot>().copied() else {
        return Ok(JsValue::undefined());
    };
    let Some(active) = lookup(slot) else {
        return Ok(JsValue::undefined());
    };
    let title = args.first().and_then(js_text).unwrap_or_default();
    active.session.begin_phase(&active.run_id, &title);
    Ok(JsValue::undefined())
}

fn log(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(slot) = context.get_data::<Slot>().copied() else {
        return Ok(JsValue::undefined());
    };
    let Some(active) = lookup(slot) else {
        return Ok(JsValue::undefined());
    };
    let message = args.first().and_then(js_text).unwrap_or_default();
    active
        .session
        .log_line(&active.run_id, cut(&message, 2_000));
    Ok(JsValue::undefined())
}

fn budget_total(_this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(active) = context.get_data::<Slot>().copied().and_then(lookup) else {
        return Ok(JsValue::null());
    };
    match active.session.budget_total() {
        Some(total) => Ok(JsValue::from(total as f64)),
        None => Ok(JsValue::null()),
    }
}

fn budget_spent(_this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(active) = context.get_data::<Slot>().copied().and_then(lookup) else {
        return Ok(JsValue::from(0));
    };
    Ok(JsValue::from(active.session.spent() as f64))
}

fn parse_call(args: &[JsValue], active: &Active, context: &mut Context) -> JsResult<ParsedCall> {
    let prompt = match args.first().and_then(js_text) {
        Some(prompt) if !prompt.is_empty() => prompt,
        _ => return Err(fail("agent expects a text prompt.")),
    };
    let options = match args.get(1) {
        None => Map::new(),
        Some(value) if value.is_undefined() || value.is_null() => Map::new(),
        Some(value) => value
            .to_json(context)?
            .and_then(|value| value.as_object().cloned())
            .ok_or_else(|| fail("Pass agent options as an object."))?,
    };
    let label = optional_text(options.get("label"), 64);
    let phase = optional_text(options.get("phase"), 64);
    let model = optional_text(options.get("model"), 200);
    let effort = match options.get("effort") {
        None | Some(Value::Null) => None,
        Some(Value::String(effort)) if EFFORTS.contains(&effort.as_str()) => Some(effort.clone()),
        Some(_) => return Err(fail("Effort must be low, medium, high, xhigh, or max.")),
    };
    let agent_type = optional_text(options.get("agentType"), 128);
    let agent = match options.get("agent") {
        None | Some(Value::Null) => None,
        Some(value) => Some(inline_agent(value)?),
    };
    if agent.is_some() && agent_type.is_some() {
        return Err(fail("Pass agent or agentType, not both."));
    }
    if agent.is_some() && !active.inline_agents {
        return Err(fail("This run cannot define an agent."));
    }
    if let Some(kind) = &agent_type
        && !active.predefined.contains(kind)
    {
        return Err(fail("That agent is not available in this run."));
    }
    let schema = match options.get("schema") {
        None | Some(Value::Null) => None,
        Some(schema) => {
            check_schema(schema)?;
            Some(schema.clone())
        }
    };
    let key = call_key(
        &prompt,
        &label,
        &phase,
        &model,
        &effort,
        &agent_type,
        &agent,
        &schema,
    );
    Ok(ParsedCall {
        prompt,
        label,
        phase,
        model,
        effort,
        agent_type,
        agent,
        schema,
        key,
    })
}

fn inline_agent(value: &Value) -> JsResult<InlineAgent> {
    let Some(object) = value.as_object() else {
        return Err(fail(
            "An inline agent needs a name, a description, and a system prompt.",
        ));
    };
    let name = required_field(object, "name")?;
    let description = required_field(object, "description")?;
    let system = required_field(object, "system")?;
    Ok(InlineAgent {
        name,
        description,
        system,
    })
}

fn required_field(object: &Map<String, Value>, key: &str) -> JsResult<String> {
    match object.get(key).and_then(Value::as_str) {
        Some(text) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        _ => Err(fail(
            "An inline agent needs a name, a description, and a system prompt.",
        )),
    }
}

fn optional_text(value: Option<&Value>, max: usize) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(cut(text, max))
    }
}

fn check_schema(schema: &Value) -> JsResult<()> {
    let message =
        "The schema must be an object with properties, and required must name those properties.";
    let Some(object) = schema.as_object() else {
        return Err(fail(message));
    };
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return Err(fail(message));
    }
    let Some(properties) = object.get("properties").and_then(Value::as_object) else {
        return Err(fail(message));
    };
    if let Some(required) = object.get("required") {
        let Some(required) = required.as_array() else {
            return Err(fail(message));
        };
        for item in required {
            let Some(name) = item.as_str() else {
                return Err(fail(message));
            };
            if !properties.contains_key(name) {
                return Err(fail(message));
            }
        }
    }
    Ok(())
}

fn satisfies(schema: &Value, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        return true;
    };
    required
        .iter()
        .all(|item| item.as_str().is_some_and(|name| object.contains_key(name)))
}

fn call_key(
    prompt: &str,
    label: &Option<String>,
    phase: &Option<String>,
    model: &Option<String>,
    effort: &Option<String>,
    agent_type: &Option<String>,
    agent: &Option<InlineAgent>,
    schema: &Option<Value>,
) -> String {
    let mut map = Map::new();
    map.insert("prompt".into(), Value::String(prompt.to_string()));
    insert_some(&mut map, "label", label);
    insert_some(&mut map, "phase", phase);
    insert_some(&mut map, "model", model);
    insert_some(&mut map, "effort", effort);
    insert_some(&mut map, "agentType", agent_type);
    if let Some(agent) = agent {
        map.insert(
            "agent".into(),
            json!({
                "name": agent.name,
                "description": agent.description,
                "system": agent.system,
            }),
        );
    }
    if let Some(schema) = schema {
        map.insert("schema".into(), schema.clone());
    }
    digest(&Value::Object(map).to_string())
}

fn insert_some(map: &mut Map<String, Value>, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        map.insert(key.into(), Value::String(value.clone()));
    }
}

fn js_text(value: &JsValue) -> Option<String> {
    value.as_string().map(|text| text.to_std_string_lossy())
}

fn fail(message: &str) -> JsError {
    JsNativeError::error()
        .with_message(message.to_owned())
        .into()
}

fn js_message(error: &JsError) -> String {
    cut(&error.to_string(), 400)
}

fn global_value(context: &mut Context, name: &str) -> Result<JsValue, String> {
    let global = context.global_object().clone();
    global
        .get(JsString::from(name), context)
        .map_err(|error| js_message(&error))
}

fn global_bool(context: &mut Context, name: &str) -> Result<bool, String> {
    Ok(global_value(context, name)?.as_boolean().unwrap_or(false))
}

fn global_text(context: &mut Context, name: &str) -> Result<Option<String>, String> {
    let value = global_value(context, name)?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    Ok(js_text(&value))
}

fn global_json(context: &mut Context, name: &str) -> Result<Value, String> {
    let value = global_value(context, name)?;
    if value.is_undefined() {
        return Ok(Value::Null);
    }
    value
        .to_json(context)
        .map_err(|error| js_message(&error))?
        .ok_or_else(|| "The script did not finish.".to_string())
}
