//! The stateful layer above the attempt loop (`docs/inference/gateway.md`,
//! section 3, P2): stored responses, `previous_response_id`, compaction,
//! connection-local state for the WebSocket transport, and hosted tools.
//!
//! [`Sessions::create`] takes an Open Responses request as the caller sent
//! it and returns a [`Turn`]: the committed stream, with every lifecycle
//! response carrying our own id (`resp_` and 32 hex characters), the
//! request's `previous_response_id` and `store`, and the tools the caller
//! declared. Upstreams always see a stateless request: the whole context
//! as input items, `store: false`, no `previous_response_id`.
//!
//! - **Continuing.** `previous_response_id` loads that response's context
//!   and output and puts the new input after them (the spec's
//!   `previous_response.input`, then `previous_response.output`, then
//!   `input`). It is found in the connection's own memory ([`Local`],
//!   WebSocket only) first, then among the owner's stored responses. An id
//!   found in neither, or owned by another tenant, is
//!   `400 previous_response_not_found`. A continuation whose
//!   `function_call_output` answers no earlier call is refused, and a
//!   continuation that fails evicts its id from [`Local`].
//! - **Storing.** Only `store: true` keeps a response (sealed, owner-scoped,
//!   expiring; see [`crate::store`]). A tenant marked zero-retention gets
//!   `400` for `store: true`; its responses live only in a WebSocket
//!   connection's memory while that connection is open.
//! - **Compaction.** [`Sessions::compact`] asks the model for a summary of
//!   the conversation and returns the user's messages plus one
//!   `compaction` item whose `encrypted_content` is the summary sealed to
//!   the owner. Sent back as input, that item opens into a developer
//!   message carrying the summary; for another owner it does not open.
//! - **Hosted tools.** See [`crate::hosted`]: the gateway runs the search
//!   loop between model turns and stitches the turns into one stream.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ErrorType, ResponseError};
use crate::event::{Event, EventBody, ItemEvent, Lifecycle};
use crate::hosted::{self, WebSearch};
use crate::item::{
    Compaction, ContentPart, FunctionCall, FunctionCallOutput, Item, ItemStatus, Message,
    MessageContent, Role, TextPart, ToolOutput,
};
use crate::openagents::{Cost, CostEvent, RequestOptions};
use crate::request::{CreateResponse, Input, Tool};
use crate::response::{Response, ResponseStatus, Usage};
use crate::run::{Caller, Events, Gateway, collect};
use crate::seal::{Sealer, random_id};
use crate::store::{Record, ResponseStore};
use crate::stream::Sequencer;
use crate::upstream::{DEFAULT_MARGIN_BPS, privacy_level};
use crate::wire::Extra;

/// The error code for a `previous_response_id` that cannot be found.
pub const PREVIOUS_NOT_FOUND: &str = "previous_response_not_found";

/// Hosted tool rounds a request gets when it sets no `max_tool_calls`: a
/// guard against a model that searches forever.
pub const DEFAULT_HOSTED_ROUNDS: u64 = 8;

/// The instructions the compaction pass runs under.
const COMPACT_INSTRUCTIONS: &str = "You compact conversations. Write a summary of the \
conversation so far that lets the assistant continue it without the original: the user's \
goals and constraints, decisions made, facts and numbers stated, tool calls and their \
results, open questions, and what was about to happen next. Plain prose, no preamble.";

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// Who a request belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    /// The tenant: the owner of everything the request stores.
    pub tenant: String,
    /// The tenant keeps nothing with us: `store: true` is refused.
    pub zero_retention: bool,
}

/// A WebSocket connection's memory: the most recent response on it, so a
/// `store: false` response can be continued on the same socket without
/// being written anywhere.
#[derive(Clone, Default)]
pub struct Local(Arc<Mutex<Option<Record>>>);

impl Local {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, owner: &str, id: &str) -> Option<Record> {
        self.0
            .lock()
            .ok()?
            .as_ref()
            .filter(|record| record.id == id && record.owner == owner)
            .cloned()
    }

    fn set(&self, record: Record) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(record);
        }
    }

    /// Forgets `id` if it is the response held.
    pub fn evict(&self, id: &str) {
        if let Ok(mut slot) = self.0.lock()
            && slot.as_ref().is_some_and(|record| record.id == id)
        {
            *slot = None;
        }
    }

    /// The id of the response held, if any.
    #[must_use]
    pub fn held(&self) -> Option<String> {
        self.0.lock().ok()?.as_ref().map(|record| record.id.clone())
    }
}

/// A started response.
pub struct Turn {
    /// Our id for the response.
    pub id: String,
    /// The model and upstream that took the first model turn.
    pub model: String,
    pub upstream: String,
    /// The events, sequence numbers from zero, ending with a terminal
    /// event.
    pub events: Events,
}

impl std::fmt::Debug for Turn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Turn")
            .field("id", &self.id)
            .field("model", &self.model)
            .field("upstream", &self.upstream)
            .finish_non_exhaustive()
    }
}

/// `POST /v1/responses/compact`'s body.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompactRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input: Option<Input>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<RequestOptions>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `POST /v1/responses/compact`'s answer (`CompactResource`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compacted {
    pub id: String,
    pub object: String,
    pub output: Vec<Item>,
    pub created_at: u64,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openagents: Option<crate::openagents::ResponseInfo>,
}

/// What a compaction item seals.
#[derive(Serialize, Deserialize)]
struct Summary {
    v: u8,
    summary: String,
}

/// The stateful layer: the attempt loop plus stored responses, the
/// sealing key, and hosted tool providers.
pub struct Sessions {
    gateway: Arc<Gateway>,
    sealer: Arc<Sealer>,
    store: Option<Arc<dyn ResponseStore>>,
    retention_ms: u64,
    search: Option<Arc<dyn WebSearch>>,
}

impl Sessions {
    /// Sessions over `gateway`, sealing compaction items with `sealer`;
    /// no stored responses and no hosted tools until added.
    #[must_use]
    pub fn new(gateway: Arc<Gateway>, sealer: Arc<Sealer>) -> Self {
        Self {
            gateway,
            sealer,
            store: None,
            retention_ms: crate::store::Config::default().retention_ms(),
            search: None,
        }
    }

    /// The same, keeping `store: true` responses in `store` for
    /// `retention_ms`.
    #[must_use]
    pub fn with_store(mut self, store: Arc<dyn ResponseStore>, retention_ms: u64) -> Self {
        self.store = Some(store);
        self.retention_ms = retention_ms;
        self
    }

    /// The same, with a web search provider for the hosted search tool.
    #[must_use]
    pub fn with_search(mut self, search: Arc<dyn WebSearch>) -> Self {
        self.search = Some(search);
        self
    }

    /// The attempt loop underneath.
    #[must_use]
    pub fn gateway(&self) -> &Arc<Gateway> {
        &self.gateway
    }

    /// Whether `store: true` is served here.
    #[must_use]
    pub fn stores(&self) -> bool {
        self.store.is_some()
    }

    /// The owner's stored response `id`.
    #[must_use]
    pub fn get(&self, owner: &Owner, id: &str) -> Option<Record> {
        self.store.as_ref()?.get(&owner.tenant, id, unix_ms())
    }

    /// Deletes the owner's stored response `id`. Whether there was one.
    #[must_use]
    pub fn delete(&self, owner: &Owner, id: &str) -> bool {
        self.store
            .as_ref()
            .is_some_and(|store| store.delete(&owner.tenant, id))
    }

    /// Removes expired stored responses. How many went.
    #[must_use]
    pub fn sweep(&self) -> usize {
        self.store
            .as_ref()
            .map_or(0, |store| store.sweep(unix_ms()))
    }

    fn find(&self, owner: &Owner, id: &str, local: Option<&Local>) -> Result<Record, ApiError> {
        if let Some(record) = local.and_then(|local| local.get(&owner.tenant, id)) {
            return Ok(record);
        }
        self.get(owner, id).ok_or_else(|| previous_not_found(id))
    }

    /// The context with every compaction item opened into a developer
    /// message.
    fn expand(&self, owner: &Owner, items: &[Item]) -> Result<Vec<Item>, ApiError> {
        items
            .iter()
            .map(|item| match item {
                Item::Compaction(compaction) => {
                    let summary = self
                        .sealer
                        .open(
                            &compaction_aad(&owner.tenant),
                            &compaction.encrypted_content,
                        )
                        .and_then(|plain| serde_json::from_slice::<Summary>(&plain).ok())
                        .ok_or_else(|| {
                            ApiError::invalid_request(
                                "input",
                                "A compaction item can't be read here: it was made for another \
                                 account or by another server.",
                            )
                            .with_code("invalid_compaction")
                        })?;
                    Ok(Item::Message(Message::text(
                        Role::Developer,
                        format!(
                            "A summary of the earlier conversation, which was compacted:\n\n{}",
                            summary.summary
                        ),
                    )))
                }
                other => Ok(other.clone()),
            })
            .collect()
    }

    /// Starts a response for `request` as the caller sent it.
    ///
    /// # Errors
    ///
    /// `400` for a request the stateful rules refuse (`store` where it is
    /// not allowed, an unknown `previous_response_id`, a tool output with
    /// no call, an unreadable compaction item, a hosted tool not set up),
    /// or the attempt loop's refusal. A failed continuation evicts its id
    /// from `local`.
    pub async fn create(
        &self,
        request: CreateResponse,
        owner: &Owner,
        caller: &Caller,
        local: Option<&Local>,
    ) -> Result<Turn, ApiError> {
        let continuing = request.previous_response_id.clone();
        let started = self.start(request, owner, caller, local).await;
        if started.is_err()
            && let (Some(id), Some(local)) = (&continuing, local)
        {
            local.evict(id);
        }
        started
    }

    async fn start(
        &self,
        request: CreateResponse,
        owner: &Owner,
        caller: &Caller,
        local: Option<&Local>,
    ) -> Result<Turn, ApiError> {
        let store = request.store == Some(true);
        if store {
            if self.store.is_none() {
                return Err(ApiError::invalid_request(
                    "store",
                    "Stored responses aren't available here. Send `store: false` and the whole \
                     conversation.",
                ));
            }
            if owner.zero_retention {
                return Err(ApiError::invalid_request(
                    "store",
                    "This account keeps nothing with us (zero retention), so responses can't \
                     be stored. Send `store: false`.",
                )
                .with_code("store_not_allowed"));
            }
        }
        let new_items = request.input_items();
        let mut context = match &request.previous_response_id {
            Some(id) => {
                let prior = self.find(owner, id, local)?;
                let earlier = prior.continued();
                check_outputs(&earlier, &new_items)?;
                earlier
            }
            None => Vec::new(),
        };
        context.extend(new_items);
        let expanded = self.expand(owner, &context)?;
        let declared = request.tools.clone().unwrap_or_default();
        let (hosted_tools, model_tools) = hosted::split(&declared)
            .map_err(|(param, message)| ApiError::invalid_request(param, message))?;
        let search = match hosted_tools.web_search {
            None => None,
            Some(results) => {
                let Some(provider) = self.search.clone() else {
                    return Err(ApiError::invalid_request(
                        "tools",
                        "Web search isn't available here.",
                    ));
                };
                if !hosted::allowed(&*provider, &privacy_level(&request)) {
                    return Err(ApiError::invalid_request(
                        "tools",
                        "Web search under `strict` privacy needs a search provider that keeps \
                         nothing, and none is set up. Send `openagents.privacy: \"standard\"` \
                         to search anyway.",
                    ));
                }
                Some((provider, results))
            }
        };

        let mut upstream_request = request.clone();
        upstream_request.input = Some(Input::Items(expanded.clone()));
        upstream_request.previous_response_id = None;
        upstream_request.store = Some(false);
        if request.tools.is_some() {
            upstream_request.tools = Some(model_tools);
        }
        let routed = self.gateway.run(&upstream_request, caller).await?;
        let id = random_id("resp_");
        let model = routed.model.clone();
        let upstream = routed.upstream.clone();
        let events = match search {
            None => routed.events,
            Some((provider, results)) => {
                let rounds = request.max_tool_calls.unwrap_or(DEFAULT_HOSTED_ROUNDS);
                hosted_loop(HostedLoop {
                    gateway: self.gateway.clone(),
                    caller: caller.clone(),
                    request: upstream_request,
                    input: expanded,
                    first: routed.events,
                    search: provider,
                    results,
                    rounds,
                })
            }
        };
        let finish = Finish {
            id: id.clone(),
            previous: request.previous_response_id.clone(),
            store,
            tools: declared,
            context,
            owner: owner.tenant.clone(),
            local: local.cloned(),
            keep: if store { self.store.clone() } else { None },
            retention_ms: self.retention_ms,
            sequencer: Sequencer::new(),
        };
        Ok(Turn {
            id,
            model,
            upstream,
            events: finish.wrap(events),
        })
    }

    /// Compacts a conversation (`POST /v1/responses/compact`).
    ///
    /// # Errors
    ///
    /// `400` without `model` or input, for an unknown
    /// `previous_response_id`, or an unreadable compaction item; the
    /// attempt loop's refusal; `502` when the model wrote no summary.
    pub async fn compact(
        &self,
        body: CompactRequest,
        owner: &Owner,
        caller: &Caller,
        local: Option<&Local>,
    ) -> Result<Compacted, ApiError> {
        let model = body
            .model
            .clone()
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| ApiError::invalid_request("model", "`model` is required."))?;
        let mut context = match &body.previous_response_id {
            Some(id) => self.find(owner, id, local)?.continued(),
            None => Vec::new(),
        };
        context.extend(
            body.input
                .clone()
                .map(Input::into_items)
                .unwrap_or_default(),
        );
        if context.is_empty() {
            return Err(ApiError::invalid_request(
                "input",
                "There is nothing to compact: send `input` or `previous_response_id`.",
            ));
        }
        let mut expanded = self.expand(owner, &context)?;
        expanded.push(Item::Message(Message::text(
            Role::User,
            "Write the summary of the conversation above now.",
        )));
        let instructions = match &body.instructions {
            Some(extra) if !extra.trim().is_empty() => {
                format!(
                    "{COMPACT_INSTRUCTIONS}\n\nThe conversation's own instructions were:\n{extra}"
                )
            }
            _ => COMPACT_INSTRUCTIONS.to_owned(),
        };
        let summarize = CreateResponse {
            model: Some(model),
            instructions: Some(instructions),
            input: Some(Input::Items(expanded)),
            stream: Some(false),
            store: Some(false),
            prompt_cache_key: body.prompt_cache_key.clone(),
            openagents: body.openagents.clone(),
            ..CreateResponse::default()
        };
        let routed = self.gateway.run(&summarize, caller).await?;
        let folded = collect(routed.events).await.ok_or_else(|| {
            ApiError::new(ErrorType::UpstreamFailed, "The model sent no summary.")
        })?;
        let summary = folded.output_text();
        if folded.status == ResponseStatus::Failed || summary.trim().is_empty() {
            return Err(ApiError::new(
                ErrorType::UpstreamFailed,
                "The model wrote no summary.",
            ));
        }
        let sealed = serde_json::to_vec(&Summary { v: 1, summary })
            .map_err(|_| ApiError::new(ErrorType::ServerError, "The summary can't be sealed."))
            .and_then(|plain| {
                self.sealer
                    .seal(&compaction_aad(&owner.tenant), &plain)
                    .map_err(|_| {
                        ApiError::new(ErrorType::ServerError, "The summary can't be sealed.")
                    })
            })?;
        let mut output: Vec<Item> = context
            .iter()
            .filter_map(|item| match item {
                Item::Message(message) if message.role == Role::User => {
                    Some(Item::Message(kept_user_message(message)))
                }
                _ => None,
            })
            .collect();
        output.push(Item::Compaction(Compaction {
            id: Some(random_id("cmp_")),
            encrypted_content: sealed,
            created_by: None,
            extra: Extra::new(),
        }));
        Ok(Compacted {
            id: random_id("resp_"),
            object: "response.compaction".to_owned(),
            output,
            created_at: unix_ms() / 1_000,
            usage: folded.usage.clone().unwrap_or_default(),
            openagents: folded.openagents.clone(),
        })
    }
}

/// What a compaction item is sealed to.
fn compaction_aad(owner: &str) -> Vec<u8> {
    format!("openagents:compaction\0{owner}").into_bytes()
}

/// `400 previous_response_not_found` for `id`.
#[must_use]
pub fn previous_not_found(id: &str) -> ApiError {
    let shown: String = id.chars().take(80).collect();
    ApiError::invalid_request(
        "previous_response_id",
        format!("Previous response with id '{shown}' not found."),
    )
    .with_code(PREVIOUS_NOT_FOUND)
}

/// A user message as compaction output keeps it: an item with an id and
/// `input_text` parts.
fn kept_user_message(message: &Message) -> Message {
    let content = match &message.content {
        MessageContent::Text(text) => {
            MessageContent::Parts(vec![ContentPart::InputText(TextPart::new(text.clone()))])
        }
        parts @ MessageContent::Parts(_) => parts.clone(),
    };
    Message {
        id: Some(message.id.clone().unwrap_or_else(|| random_id("msg_"))),
        status: Some(ItemStatus::Completed),
        role: Role::User,
        content,
        phase: None,
        extra: Extra::new(),
    }
}

/// Refuses a continuation whose tool output answers no call: every
/// `function_call_output` in `new_items` needs a `function_call` with its
/// `call_id` earlier in the conversation.
fn check_outputs(earlier: &[Item], new_items: &[Item]) -> Result<(), ApiError> {
    let mut calls: HashSet<&str> = HashSet::new();
    for item in earlier.iter().chain(new_items) {
        match item {
            Item::FunctionCall(call) => {
                calls.insert(call.call_id.as_str());
            }
            Item::FunctionCallOutput(output) if !calls.contains(output.call_id.as_str()) => {
                return Err(ApiError::invalid_request(
                    "input",
                    format!(
                        "No tool call found for function call output with call_id '{}'.",
                        output.call_id.chars().take(80).collect::<String>()
                    ),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Stamps our id and the request's settings on every lifecycle response,
/// renumbers the events, and at the end keeps the response: in the
/// connection's memory, and in the store when the request said `store`.
struct Finish {
    id: String,
    previous: Option<String>,
    store: bool,
    tools: Vec<Tool>,
    context: Vec<Item>,
    owner: String,
    local: Option<Local>,
    keep: Option<Arc<dyn ResponseStore>>,
    retention_ms: u64,
    sequencer: Sequencer,
}

impl Finish {
    fn wrap(mut self, events: Events) -> Events {
        Box::pin(events.map(move |event| self.event(event)))
    }

    fn event(&mut self, event: Event) -> Event {
        let mut body = event.body;
        let terminal = body.is_terminal();
        if let Some(response) = lifecycle_response(&mut body) {
            response.id.clone_from(&self.id);
            response.previous_response_id.clone_from(&self.previous);
            response.store = self.store;
            response.tools.clone_from(&self.tools);
            if terminal {
                self.end(response);
            }
        }
        self.sequencer.stamp(body)
    }

    fn end(&mut self, response: &mut Response) {
        if response.status == ResponseStatus::Failed {
            if let (Some(previous), Some(local)) = (&self.previous, &self.local) {
                local.evict(previous);
            }
            return;
        }
        let now = unix_ms();
        let record = Record {
            id: self.id.clone(),
            owner: self.owner.clone(),
            created_at_ms: now,
            expires_at_ms: now.saturating_add(self.retention_ms),
            context: std::mem::take(&mut self.context),
            response: response.clone(),
        };
        if let Some(store) = &self.keep
            && store.put(&record).is_err()
        {
            // Not kept: say so on the response the caller gets.
            response.store = false;
        }
        if let Some(local) = &self.local {
            local.set(record);
        }
    }
}

fn lifecycle_response(body: &mut EventBody) -> Option<&mut Response> {
    match body {
        EventBody::Created(event)
        | EventBody::Queued(event)
        | EventBody::InProgress(event)
        | EventBody::Completed(event)
        | EventBody::Incomplete(event)
        | EventBody::Failed(event) => Some(&mut event.response),
        _ => None,
    }
}

/// Micro-dollars in a decimal dollar string (`"0.000578"`), rounding
/// anything below a micro-dollar away.
fn micros_of(usd: &str) -> u64 {
    let (whole, fraction) = usd.split_once('.').unwrap_or((usd, ""));
    let whole: u64 = whole.parse().unwrap_or(0);
    let mut digits: String = fraction.chars().take(6).collect();
    while digits.len() < 6 {
        digits.push('0');
    }
    whole
        .saturating_mul(1_000_000)
        .saturating_add(digits.parse().unwrap_or(0))
}

/// The margin on `cost` micro-dollars at the default margin, rounded up.
fn margin_of(cost: u64) -> u64 {
    let margin = (u128::from(cost) * u128::from(DEFAULT_MARGIN_BPS)).div_ceil(10_000);
    u64::try_from(margin).unwrap_or(u64::MAX)
}

fn add_usage(total: &mut Usage, more: &Usage) {
    *total = Usage::new(
        total.input_tokens + more.input_tokens,
        total.input_tokens_details.cached_tokens + more.input_tokens_details.cached_tokens,
        total.output_tokens + more.output_tokens,
        total.output_tokens_details.reasoning_tokens + more.output_tokens_details.reasoning_tokens,
    );
}

/// The hosted tool loop's inputs.
struct HostedLoop {
    gateway: Arc<Gateway>,
    caller: Caller,
    /// The request as upstreams see it (hosted tools as functions).
    request: CreateResponse,
    /// The input of the first model turn.
    input: Vec<Item>,
    first: Events,
    search: Arc<dyn WebSearch>,
    results: u64,
    /// Searches allowed in all.
    rounds: u64,
}

/// Runs model turns and searches until the model answers without
/// searching, as one stream: one `response.created` and
/// `response.in_progress`, every item renumbered in one sequence, search
/// items between the turns, and one terminal event whose response holds
/// every item, the summed usage, and the summed cost.
fn hosted_loop(job: HostedLoop) -> Events {
    let (tx, rx) = tokio::sync::mpsc::channel::<EventBody>(64);
    tokio::spawn(drive(job, tx));
    Box::pin(futures_util::stream::unfold(
        (rx, Sequencer::new()),
        |(mut rx, mut sequencer)| async move {
            let body = rx.recv().await?;
            Some((sequencer.stamp(body), (rx, sequencer)))
        },
    ))
}

/// One model turn's events, read with items renumbered and the hosted
/// function's calls held back.
struct TurnRead {
    /// The turn's terminal event, if one came.
    terminal: Option<EventBody>,
    /// Calls to the hosted function, in order.
    calls: Vec<FunctionCall>,
    cost_upstream: u64,
    cost_margin: u64,
}

async fn read_turn(
    events: &mut Events,
    first_turn: bool,
    offset: &mut u64,
    tx: &tokio::sync::mpsc::Sender<EventBody>,
) -> Option<TurnRead> {
    // The turn's output index, to ours (`None`: a hosted call, held back).
    let mut map: HashMap<u64, Option<u64>> = HashMap::new();
    let mut read = TurnRead {
        terminal: None,
        calls: Vec::new(),
        cost_upstream: 0,
        cost_margin: 0,
    };
    while let Some(event) = events.next().await {
        let mut body = event.body;
        match &mut body {
            EventBody::Created(_) | EventBody::Queued(_) | EventBody::InProgress(_) => {
                if !first_turn {
                    continue;
                }
            }
            EventBody::Route(_) => {
                if !first_turn {
                    continue;
                }
            }
            EventBody::Cost(CostEvent { cost, .. }) => {
                read.cost_upstream += micros_of(&cost.upstream_usd);
                read.cost_margin += micros_of(&cost.margin_usd);
                continue;
            }
            terminal if terminal.is_terminal() => {
                read.terminal = Some(body);
                return Some(read);
            }
            EventBody::OutputItemAdded(added) => {
                if is_hosted_call(&added.item) {
                    map.insert(added.output_index, None);
                    continue;
                }
                map.insert(added.output_index, Some(*offset));
                added.output_index = *offset;
                *offset += 1;
            }
            EventBody::OutputItemDone(done) => {
                if is_hosted_call(&done.item) {
                    if let Item::FunctionCall(call) = &done.item {
                        read.calls.push(call.clone());
                    }
                    map.insert(done.output_index, None);
                    continue;
                }
                let global = match map.get(&done.output_index) {
                    Some(Some(global)) => *global,
                    Some(None) => continue,
                    None => {
                        let global = *offset;
                        *offset += 1;
                        map.insert(done.output_index, Some(global));
                        global
                    }
                };
                done.output_index = global;
            }
            other => {
                if let Some(index) = output_index_mut(other) {
                    match map.get(index) {
                        Some(Some(global)) => *index = *global,
                        Some(None) => continue,
                        None => {}
                    }
                }
            }
        }
        if tx.send(body).await.is_err() {
            return None;
        }
    }
    Some(read)
}

fn is_hosted_call(item: &Item) -> bool {
    matches!(item, Item::FunctionCall(call) if call.name == hosted::FUNCTION)
}

fn output_index_mut(body: &mut EventBody) -> Option<&mut u64> {
    match body {
        EventBody::ContentPartAdded(event) | EventBody::ContentPartDone(event) => {
            Some(&mut event.output_index)
        }
        EventBody::ReasoningSummaryPartAdded(event)
        | EventBody::ReasoningSummaryPartDone(event) => Some(&mut event.output_index),
        EventBody::OutputTextDelta(event)
        | EventBody::RefusalDelta(event)
        | EventBody::ReasoningDelta(event)
        | EventBody::ReasoningTextDelta(event) => Some(&mut event.output_index),
        EventBody::OutputTextDone(event)
        | EventBody::ReasoningDone(event)
        | EventBody::ReasoningTextDone(event) => Some(&mut event.output_index),
        EventBody::OutputTextAnnotationAdded(event) => Some(&mut event.output_index),
        EventBody::RefusalDone(event) => Some(&mut event.output_index),
        EventBody::ReasoningSummaryTextDelta(event) => Some(&mut event.output_index),
        EventBody::ReasoningSummaryTextDone(event) => Some(&mut event.output_index),
        EventBody::FunctionCallArgumentsDelta(event) => Some(&mut event.output_index),
        EventBody::FunctionCallArgumentsDone(event) => Some(&mut event.output_index),
        _ => None,
    }
}

async fn drive(job: HostedLoop, tx: tokio::sync::mpsc::Sender<EventBody>) {
    let HostedLoop {
        gateway,
        caller,
        request,
        mut input,
        first,
        search,
        results,
        rounds,
    } = job;
    let mut events = first;
    let mut offset = 0u64;
    let mut output: Vec<Item> = Vec::new();
    let mut usage = Usage::default();
    let mut upstream_cost = 0u64;
    let mut margin = 0u64;
    let mut searched = 0u64;
    let mut first_turn = true;
    loop {
        let Some(read) = read_turn(&mut events, first_turn, &mut offset, &tx).await else {
            return;
        };
        first_turn = false;
        upstream_cost += read.cost_upstream;
        margin += read.cost_margin;
        let Some(mut terminal) = read.terminal else {
            return;
        };
        let Some(response) = lifecycle_response(&mut terminal) else {
            return;
        };
        if let Some(turn_usage) = &response.usage {
            add_usage(&mut usage, turn_usage);
        }
        let turn_output = std::mem::take(&mut response.output);
        let mut calls = read.calls;
        for item in &turn_output {
            if let Item::FunctionCall(call) = item
                && call.name == hosted::FUNCTION
                && !calls.iter().any(|seen| seen.call_id == call.call_id)
            {
                calls.push(call.clone());
            }
        }
        output.extend(
            turn_output
                .iter()
                .filter(|item| !is_hosted_call(item))
                .cloned(),
        );
        let finished = response.status != ResponseStatus::Completed || calls.is_empty();
        if finished {
            response.output = output;
            response.usage = Some(usage);
            let cost = Cost::from_micros(upstream_cost, margin, None);
            if let Some(info) = response.openagents.as_mut() {
                info.cost = Some(cost.clone());
            }
            let _ = tx
                .send(EventBody::Cost(CostEvent {
                    cost,
                    extra: Extra::new(),
                }))
                .await;
            let _ = tx.send(terminal).await;
            return;
        }
        // Search, show each search as an item, and hand the results back.
        input.extend(turn_output);
        for call in calls {
            let query = hosted::query_of(&call.arguments);
            let outcome = match &query {
                None => Err("no query".to_owned()),
                Some(_) if searched >= rounds => Err("out of searches".to_owned()),
                Some(query) => {
                    searched += 1;
                    let price = search.price_micros();
                    upstream_cost += price;
                    margin += margin_of(price);
                    search.search(query, results).await
                }
            };
            let id = random_id("ws_");
            let shown = query.clone().unwrap_or_default();
            let mut started = hosted::call_item(&id, &shown, &Ok(Vec::new()));
            started["status"] = "in_progress".into();
            let item = Item::Unknown(hosted::call_item(&id, &shown, &outcome));
            for (body, event_item) in [(true, Item::Unknown(started)), (false, item.clone())] {
                let event = ItemEvent {
                    output_index: offset,
                    item: event_item,
                    extra: Extra::new(),
                };
                let body = if body {
                    EventBody::OutputItemAdded(event)
                } else {
                    EventBody::OutputItemDone(event)
                };
                if tx.send(body).await.is_err() {
                    return;
                }
            }
            offset += 1;
            output.push(item);
            input.push(Item::FunctionCallOutput(FunctionCallOutput {
                id: None,
                status: None,
                call_id: call.call_id.clone(),
                output: ToolOutput::Text(hosted::function_output(&outcome)),
                extra: Extra::new(),
            }));
        }
        let mut next = request.clone();
        next.input = Some(Input::Items(input.clone()));
        if searched >= rounds
            && let Some(tools) = next.tools.as_mut()
        {
            tools.retain(|tool| !matches!(tool, Tool::Function(f) if f.name == hosted::FUNCTION));
        }
        match gateway.run(&next, &caller).await {
            Ok(routed) => events = routed.events,
            Err(refusal) => {
                let mut failed = response.clone();
                failed.status = ResponseStatus::Failed;
                failed.output = output;
                failed.usage = Some(usage);
                failed.error = Some(ResponseError {
                    code: refusal
                        .code
                        .clone()
                        .unwrap_or_else(|| refusal.kind.as_str().to_owned()),
                    message: refusal.message.clone(),
                    extra: Extra::new(),
                });
                let _ = tx
                    .send(EventBody::lifecycle(Lifecycle::Failed, failed))
                    .await;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollar_strings_become_micros() {
        assert_eq!(micros_of("0.000578"), 578);
        assert_eq!(micros_of("1"), 1_000_000);
        assert_eq!(micros_of("2.5"), 2_500_000);
        assert_eq!(micros_of("0.0000001"), 0);
        assert_eq!(margin_of(10_000), 500);
        assert_eq!(margin_of(1), 1);
    }

    #[test]
    fn tool_outputs_need_their_calls() {
        let call = Item::FunctionCall(FunctionCall {
            id: None,
            status: None,
            call_id: "call_1".into(),
            name: "f".into(),
            arguments: "{}".into(),
            extra: Extra::new(),
        });
        let output = |id: &str| {
            Item::FunctionCallOutput(FunctionCallOutput {
                id: None,
                status: None,
                call_id: id.into(),
                output: ToolOutput::Text("x".into()),
                extra: Extra::new(),
            })
        };
        assert!(check_outputs(std::slice::from_ref(&call), &[output("call_1")]).is_ok());
        assert!(check_outputs(&[call], &[output("call_2")]).is_err());
        assert!(check_outputs(&[], &[output("call_1")]).is_err());
    }

    #[test]
    fn local_memory_holds_one_response_and_evicts_it() {
        let local = Local::new();
        let record = Record {
            id: "resp_1".into(),
            owner: "acme".into(),
            created_at_ms: 0,
            expires_at_ms: 0,
            context: Vec::new(),
            response: Response::from_request("resp_1", 0, "m", &CreateResponse::default()),
        };
        local.set(record);
        assert!(local.get("acme", "resp_1").is_some());
        assert!(local.get("other", "resp_1").is_none());
        local.evict("resp_2");
        assert_eq!(local.held().as_deref(), Some("resp_1"));
        local.evict("resp_1");
        assert!(local.held().is_none());
    }
}
