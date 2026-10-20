//! Using agents socially (#12256): fork one, and talk with one in public
//! rooms and in private.
//!
//! - **Fork.** `POST /v1/published-agents/{id}/fork` forks an agent this
//!   site stores, or one only its public definition (NIP-AGENT kind 30187)
//!   on the relays describes; either way the fork keeps its `upstream`.
//!   `/agent/{id}/fork` is the same from the agent's page, signed in.
//! - **Rooms.** People and agents talk in NIP-29 groups with NIP-C7 chat
//!   (kind 9): the public room [`PUBLIC_ROOM`] and one room per agent
//!   ([`agent_room`]). A message that mentions an agent (a `p` tag naming
//!   its key) gets an answer signed by the agent's key, quoting the message
//!   (`q`) and carrying the owner's NIP-OA tag.
//! - **Private messages.** A NIP-17 message (kind 1059 gift wrap) to an
//!   agent gets a NIP-17 reply to its sender. The owner sets who may start
//!   one (`PUT /v1/published-agents/{id}/messages` `{"from": "owner" |
//!   "anyone"}`); the owner, by default.
//!
//! Answers run only when `OPENAGENTS_WEB_AGENT_REPLIES=on` starts
//! [`spawn`]; nothing listens otherwise. What answers is an [`Answerer`]:
//! the agent's saved instructions and the message, nothing else.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use maud::html;
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::nip17;
use openagents_sdk::agent_lab::{ForkAgent, Forked, PublishedAgent};
use openagents_sdk::agents::AgentDefinition;
use openagents_ui::actions::{Button, ButtonType};
use openagents_ui::content::MarkdownRoot;
use serde::Deserialize;
use serde_json::{Value, json};

use super::owner::{self, Owned};
use super::{Refusal, fork, fork_published, load};
use crate::App;
use crate::chat_store::Error;
use crate::coder_sync::{answer, stored};

/// The room everyone shares.
pub(crate) const PUBLIC_ROOM: &str = "openagents";
/// The longest answer an agent posts.
const ANSWER_MAX: usize = 4_000;
/// The longest message an agent reads.
const QUESTION_MAX: usize = 4_000;

/// The room for published agent `id`.
#[must_use]
pub(crate) fn agent_room(id: &str) -> String {
    format!("agent-{}", &id[..16.min(id.len())])
}

/// Who may start a private conversation with an agent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MessagesFrom {
    #[default]
    Owner,
    Anyone,
}

/// The newest live public definition of published agent `id` on the
/// relays, as a published agent to fork.
async fn from_relays(app: &App, id: &str) -> Option<PublishedAgent> {
    let ours: Vec<String> = app.config.profile_relays.iter().take(3).cloned().collect();
    let mut events = Vec::new();
    for relay in &ours {
        events.extend(
            crate::profiles::relays::read(
                relay,
                json!({"kinds": [nostr::agent_public::DEFINITION_KIND], "#d": [id], "limit": 20}),
                crate::profiles::relays::READ_WAIT,
            )
            .await,
        );
    }
    let definition = nostr::agent_public::newest_definitions(&events)
        .into_iter()
        .find(|d| d.id == id && !d.revoked)?;
    let experiment: openagents_sdk::agent_lab::Experiment =
        serde_json::from_value(definition.document?.get("experiment")?.clone()).ok()?;
    let spec = experiment.draft.agent_spec().ok()?;
    Some(PublishedAgent {
        id: id.to_string(),
        revision: 1,
        definition: AgentDefinition {
            id: id.to_string(),
            revision: 1,
            spec,
            deleted: false,
        },
        template: experiment,
    })
}

/// Fork published agent `id`, stored here or described on the relays.
pub(super) async fn fork_any(
    app: &App,
    owner: &str,
    id: &str,
    request: ForkAgent,
) -> Result<Result<Forked, Refusal>, Error> {
    let store = &app.config.chat_store;
    if load(store, id).await?.is_some() {
        return fork(store, owner, id, request).await;
    }
    match from_relays(app, id).await {
        Some(published) => fork_published(store, owner, id, &published, request).await,
        None => Ok(Err(Refusal::Missing)),
    }
}

const FORK_SCOPE: &str = "agent-fork";

fn fork_target(viewer: &crate::cloud::session::Viewer, id: &str) -> String {
    format!("{}:{id}", viewer.account_id)
}

/// `GET /agent/{id}/fork`: confirm a fork, signed in.
pub(crate) async fn fork_page(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let back = format!("/agent/{id}/fork");
    let (service, viewer) = match crate::settings::viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !nostr::agent_public::valid_id(&id) {
        return crate::not_found().await;
    }
    let csrf = match service.csrf(&headers, &viewer, FORK_SCOPE, &fork_target(&viewer, &id)) {
        Ok(csrf) => csrf,
        Err(error) => return crate::cloud::refused(error),
    };
    let body = html! {
        (MarkdownRoot::new(html! {
            h1 { "Make your own copy" }
            p { "Your copy starts as this agent is now. You can change it, and it keeps a link to the original." }
            p { a href=(format!("/agent/{id}")) { "The original" } }
        }))
        form method="post" action=(back) {
            input type="hidden" name="csrf" value=(csrf);
            p { (Button::new("Make a copy").kind(ButtonType::Submit)) }
        }
    };
    crate::settings::page(&headers, service, &viewer, "Make a copy", &back, body)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ForkForm {
    csrf: String,
}

/// `POST /agent/{id}/fork`.
pub(crate) async fn fork_submit(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    form: Result<Form<ForkForm>, FormRejection>,
) -> Response {
    let back = format!("/agent/{id}/fork");
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(crate::cloud::session::SessionError::InvalidRequest);
    };
    let (service, viewer) = match crate::settings::viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        FORK_SCOPE,
        &fork_target(&viewer, &id),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    if !nostr::agent_public::valid_id(&id) {
        return crate::not_found().await;
    }
    let owner = crate::chat_store::account_owner(&viewer.account_id);
    let request_id = crate::cloud::byo::fresh_request().repeat(2);
    let name = match owner::found(&app, &id).await {
        Some(found) => found.newest.definition.spec.name.clone(),
        None => match from_relays(&app, &id).await {
            Some(published) => published.definition.spec.name,
            None => return crate::not_found().await,
        },
    };
    let copy = format!("{name} (my copy)");
    let forked = fork_any(
        &app,
        &owner,
        &id,
        ForkAgent {
            request_id,
            name: copy,
        },
    )
    .await;
    let forked = match forked {
        Ok(Ok(forked)) => forked,
        Ok(Err(refusal)) => return refusal.response(),
        Err(e) => return stored(&e),
    };
    let body = html! {
        (MarkdownRoot::new(html! {
            h1 { "Your copy is saved" }
            p { strong { (forked.definition.spec.name) } " is in your agents. It came from "
                a href=(format!("/agent/{}", forked.forked_from.published)) { "this agent" } "." }
            p { a href="/labs/agent" { "Open Agent Lab" } }
        }))
    };
    crate::settings::page(&headers, service, &viewer, "Your copy", &back, body)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetMessages {
    from: MessagesFrom,
}

/// `PUT /v1/published-agents/{id}/messages` `{"from"}`, as the publisher.
pub(crate) async fn set_messages(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<SetMessages>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let publisher = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Json(body)) = body else {
        return Refusal::Invalid("Say who may message this agent: owner or anyone.".into())
            .response();
    };
    match owner::set_messages(&app, &publisher, &id, body.from).await {
        Ok(Ok(())) => answer(StatusCode::OK, json!({"from": body.from})),
        Ok(Err(r)) => r.response(),
        Err(e) => stored(&e),
    }
}

/// What writes an agent's answer.
pub(crate) trait Answerer: Send + Sync {
    /// Answer `question` under the agent's `instructions`; `None` when no
    /// answer could be written.
    fn answer(
        &self,
        instructions: String,
        question: String,
    ) -> Pin<Box<dyn Future<Output = Option<String>> + Send>>;
}

/// Gemini on Vertex AI, the model the site's own doors use.
pub(crate) struct Vertex(pub(crate) crate::chat_vision::Gemini);

impl Answerer for Vertex {
    fn answer(
        &self,
        instructions: String,
        question: String,
    ) -> Pin<Box<dyn Future<Output = Option<String>> + Send>> {
        let gemini = self.0.clone();
        Box::pin(async move {
            let body = json!({
                "systemInstruction": {"parts": [{"text": instructions}]},
                "contents": [{"role": "user", "parts": [{"text": question}]}],
            });
            let reply = crate::connections::chat::gemini_call(&gemini, &body)
                .await
                .ok()?;
            crate::chat_vision::gemini_text(&reply)
        })
    }
}

/// An agent that may answer: its key, its owner, and its instructions.
struct Speaker {
    signer: RelaySigner,
    secret: secp256k1::SecretKey,
    owned: Owned,
    instructions: String,
}

async fn speaker(app: &App, agent: &str) -> Option<Speaker> {
    let id = owner::id_of_key(app, agent).await?;
    let found = owner::found(app, &id).await?;
    let owned = found.owner.filter(|o| !o.revoked)?;
    let (signer, secret) = owner::keys(app, &id)?;
    Some(Speaker {
        signer,
        secret,
        owned,
        instructions: found.newest.definition.spec.instructions,
    })
}

fn bounded(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The answer to one event the relays delivered, ready to publish: a
/// room reply for a kind 9 mention, a gift-wrapped reply for a private
/// message. `None` when nothing should answer it.
pub(crate) async fn respond(
    app: &App,
    answerer: &dyn Answerer,
    event: &Event,
    now: u64,
) -> Option<Event> {
    if event.validate_crypto().is_err() {
        return None;
    }
    match event.kind {
        9 => {
            let room = event.tag_values("h").next()?.to_string();
            let mut agent = None;
            for key in event.tag_values("p") {
                if key != event.pubkey
                    && let Some(found) = speaker(app, key).await
                {
                    agent = Some(found);
                    break;
                }
            }
            let agent = agent?;
            if agent.signer.pubkey() == event.pubkey {
                return None;
            }
            let text = answerer
                .answer(
                    agent.instructions.clone(),
                    bounded(&event.content, QUESTION_MAX),
                )
                .await?;
            let relay = app
                .config
                .profile_relays
                .first()
                .cloned()
                .unwrap_or_default();
            let tags = vec![
                Tag::new(vec!["h".into(), room]),
                Tag::new(vec![
                    "q".into(),
                    event.id.clone(),
                    relay,
                    event.pubkey.clone(),
                ]),
                Tag::new(vec!["p".into(), event.pubkey.clone()]),
                Tag::new(agent.owned.tag.clone()),
            ];
            Some(agent.signer.sign(now, 9, tags, bounded(&text, ANSWER_MAX)))
        }
        nip17::GIFT_WRAP_KIND => {
            let agent_key = event.tag_values("p").next()?.to_string();
            let agent = speaker(app, &agent_key).await?;
            let rumor = nip17::open_direct_message(event, &agent.secret).ok()?;
            if rumor.kind != nip17::CHAT_KIND || rumor.pubkey == agent_key {
                return None;
            }
            let allowed = match owner::messages_from(&agent.owned) {
                MessagesFrom::Anyone => true,
                MessagesFrom::Owner => rumor.pubkey == agent.owned.pubkey,
            };
            if !allowed {
                return None;
            }
            let text = answerer
                .answer(
                    agent.instructions.clone(),
                    bounded(&rumor.content, QUESTION_MAX),
                )
                .await?;
            let reader: secp256k1::XOnlyPublicKey = rumor.pubkey.parse().ok()?;
            let reply = nip17::chat_rumor(
                agent.signer.pubkey(),
                now,
                &bounded(&text, ANSWER_MAX),
                vec![
                    Tag::new(vec!["p".into(), rumor.pubkey.clone()]),
                    Tag::new(vec!["e".into(), rumor.id.clone()]),
                ],
            )
            .ok()?;
            let nonce = || secp256k1::rand::random::<[u8; 32]>();
            let sealed = nip17::seal(&reply, &agent.secret, &reader, now, nonce(), None).ok()?;
            let wrapper = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
            nip17::gift_wrap(&sealed, &wrapper, &reader, now, nonce(), None).ok()
        }
        _ => None,
    }
}

/// Publish `event` to `relay`, authenticating as `signer` with the owner's
/// tag (NIP-42, NIP-AA) when the relay asks for it.
async fn publish_as(
    relay: &str,
    secret: &secp256k1::SecretKey,
    owner_tag: &Tag,
    event: &Event,
) -> Result<(), String> {
    match crate::profiles::relays::publish(relay, event).await {
        Ok(()) => Ok(()),
        Err(why) if why.contains("auth-required") => {
            let mut connection = nostr_transport::Connection::connect_as(
                relay,
                secret,
                Some(owner_tag),
                Duration::from_secs(10),
            )
            .await?;
            connection.send(json!(["EVENT", event])).await?;
            loop {
                let frame = connection.next().await?;
                if frame.get(0).and_then(Value::as_str) == Some("OK")
                    && frame.get(1) == Some(&json!(event.id))
                {
                    let _ = connection.close().await;
                    return if frame.get(2) == Some(&json!(true)) {
                        Ok(())
                    } else {
                        Err("refused".into())
                    };
                }
            }
        }
        Err(why) => Err(why),
    }
}

/// Listen for mentions and private messages to this site's agents and
/// answer them, until the process ends.
pub(crate) fn spawn(app: App, answerer: Arc<dyn Answerer>) {
    tokio::spawn(async move {
        let mut since = crate::cloud::session::now();
        loop {
            let keys = owner::agent_keys(&app).await;
            let Some(relay) = app.config.profile_relays.first().cloned() else {
                return;
            };
            if keys.is_empty() {
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            }
            let filter = json!({"kinds": [9, nip17::GIFT_WRAP_KIND], "#p": keys, "since": since.saturating_sub(5)});
            let Ok(connection) =
                nostr_transport::Connection::connect_open(&relay, Duration::from_secs(110)).await
            else {
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            };
            let mut connection = connection.with_frame_budget(4_096);
            if connection
                .send(json!(["REQ", "agents", filter]))
                .await
                .is_err()
            {
                continue;
            }
            let mut answered = std::collections::BTreeSet::new();
            while let Ok(frame) = connection.next().await {
                if frame.get(0).and_then(Value::as_str) != Some("EVENT") {
                    continue;
                }
                let Some(event) = frame
                    .get(2)
                    .and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
                else {
                    continue;
                };
                if !answered.insert(event.id.clone()) {
                    continue;
                }
                since = since.max(event.created_at);
                let now = crate::cloud::session::now();
                if let Some(reply) = respond(&app, answerer.as_ref(), &event, now).await {
                    send_reply(&app, &reply).await;
                }
            }
        }
    });
}

/// Publish an answer the agent signed, or a gift wrap carrying one.
pub(crate) async fn send_reply(app: &App, reply: &Event) {
    for relay in &app.config.profile_relays {
        let _ = match reply.kind {
            9 => match speaker(app, &reply.pubkey).await {
                Some(agent) => {
                    publish_as(
                        relay,
                        &agent.secret,
                        &Tag::new(agent.owned.tag.clone()),
                        reply,
                    )
                    .await
                }
                None => Err(String::new()),
            },
            _ => crate::profiles::relays::publish(relay, reply).await,
        };
    }
}

/// `/agent/{id}/fork` both ways.
pub(crate) fn fork_route() -> axum::routing::MethodRouter<App> {
    axum::routing::get(fork_page).post(fork_submit)
}
