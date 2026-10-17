//! The Labs in the web chat: Video Lab ([`crate::lab`]), Game Lab
//! ([`crate::game_lab`]), CAD Lab ([`crate::cad_lab`]) and Ship Lab
//! ([`crate::ship_lab`]) behind one door
//! (#11619, #11627).
//!
//! One chat turn is one Gemini 4 Argon conversation that sees both Labs'
//! tools together (`lab_*`, `game_*` and `cad_*`) and `general_chat`, with the
//! chat's current Lab document as context: the last Lab card the chat
//! showed, its kind and id. The model's typed function choice is the
//! route, so a follow-up in a film conversation goes to the film and one
//! in a game conversation goes to the game. No words of the message are
//! matched here.
//!
//! Tools never invent: a tool call naming the other Lab's document is
//! refused here before it reaches a Lab (an exact id check on a bounded
//! field), each Lab refuses versions it doesn't have, and the model is told
//! to say only what the tool results say.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use inference::upstream::vertex;
use openagents_chat::basic_coder::{self, Door, Failure, Reply, Role as TurnRole, Turn};
use openagents_chat::router::{Context, Meta};
use serde_json::{Value, json};

use crate::App;
use crate::chat_vision::{Gemini, gemini_text};
use crate::connections::chat::{HANDOFF, gemini_call};

/// The most model steps (tool rounds) one answer takes.
const MAX_STEPS: usize = 12;
/// How long a Labs turn may take: its tool steps rebuild and check models.
pub(crate) const DEADLINE: std::time::Duration = std::time::Duration::from_secs(300);

/// Which Lab a tool, a document or a card belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Video,
    Game,
    Robot,
    Cad,
    Ship,
    Registered(String),
}

impl Kind {
    /// The Lab a tool name belongs to, by its typed prefix.
    fn of_tool(labs: &[Lab], name: &str) -> Option<Kind> {
        labs.iter()
            .find(|lab| lab.tools.iter().any(|tool| tool["name"] == name))
            .map(|lab| lab.kind.clone())
    }

    /// The Lab a card belongs to, by its card kind.
    fn of_card(card: &Value) -> Option<Kind> {
        if card["schema"] == lab_kit::card::SCHEMA {
            return card["kind"]
                .as_str()
                .map(|kind| Kind::Registered(kind.into()));
        }
        match card["card"].as_str()? {
            crate::lab::CARD => Some(Kind::Video),
            crate::game_lab::CARD => Some(Kind::Game),
            crate::robot_lab::CARD => Some(Kind::Robot),
            crate::cad_lab::CARD => Some(Kind::Cad),
            crate::ship_lab::CARD => Some(Kind::Ship),
            _ => None,
        }
    }

    fn card(&self) -> &'static str {
        match self {
            Kind::Video => crate::lab::CARD,
            Kind::Game => crate::game_lab::CARD,
            Kind::Robot => crate::robot_lab::CARD,
            Kind::Cad => crate::cad_lab::CARD,
            Kind::Ship => crate::ship_lab::CARD,
            Kind::Registered(_) => lab_kit::card::SCHEMA,
        }
    }

    /// The argument a tool of this Lab names its document by.
    fn doc_arg(&self) -> &'static str {
        match self {
            Kind::Video => "document",
            Kind::Game => "game",
            Kind::Robot => "document",
            Kind::Cad => "doc",
            Kind::Ship => "doc",
            Kind::Registered(_) => "document",
        }
    }

    fn name(&self) -> &str {
        match self {
            Kind::Video => "Video Lab",
            Kind::Game => "Game Lab",
            Kind::Robot => "Robot Lab",
            Kind::Cad => "CAD Lab",
            Kind::Ship => "Ship Lab",
            Kind::Registered(kind) => kind,
        }
    }
    fn id(&self) -> &str {
        match self {
            Self::Video => "video",
            Self::Game => "game",
            Self::Robot => "robot",
            Self::Cad => "cad",
            Self::Ship => "ship",
            Self::Registered(kind) => kind,
        }
    }
    fn accepts(&self, card: &Value) -> bool {
        match self {
            Self::Registered(kind) => {
                card["schema"] == lab_kit::card::SCHEMA && card["kind"] == *kind
            }
            _ => card["card"] == self.card(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Platform {
    manifest: lab_kit::KindManifest,
    client: openagents_sdk::labs::Client,
    space: String,
    owner: String,
}

/// One Lab this server talks to: where it runs, its tools, its documents.
#[derive(Clone)]
pub(crate) struct Lab {
    pub kind: Kind,
    pub url: String,
    pub tools: Vec<Value>,
    pub documents: Vec<Value>,
    /// CAD Lab on this site (#11653): the models a turn works on, in place
    /// of a Lab on a Mac.
    pub hosted: Option<crate::cad_web::Scope>,
    pub platform: Option<Platform>,
}

impl Lab {
    fn has(&self, id: &str) -> bool {
        self.documents.iter().any(|doc| doc["id"] == id)
    }
    fn doc_arg(&self) -> &str {
        self.platform
            .as_ref()
            .map_or_else(|| self.kind.doc_arg(), |p| p.manifest.doc_arg.as_str())
    }
    fn name(&self) -> &str {
        self.platform
            .as_ref()
            .map_or_else(|| self.kind.name(), |p| p.manifest.name.as_str())
    }
}

/// The chat's current Lab document: the last Lab card it showed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Current {
    pub kind: Kind,
    pub doc: String,
    pub title: String,
    pub version: Option<u64>,
}

/// The chat's current Lab document, from its earlier replies' cards (the
/// latest reply with a Lab card wins).
pub(crate) fn current<'a>(replies: impl DoubleEndedIterator<Item = &'a Meta>) -> Option<Current> {
    replies.rev().find_map(|meta| {
        meta.cards.iter().rev().find_map(|card| {
            let kind = Kind::of_card(card)?;
            let doc = card["document"]
                .as_str()
                .or_else(|| card["doc"].as_str())
                .filter(|d| !d.is_empty())?;
            Some(Current {
                kind,
                doc: doc.to_owned(),
                title: card["title"].as_str().unwrap_or(doc).to_owned(),
                version: card["version"].as_u64(),
            })
        })
    })
}

/// A door that works on the Labs' documents, or hands the message to
/// `fallback`.
pub(crate) struct LabsDoor {
    pub gemini: Gemini,
    pub labs: Vec<Lab>,
    /// Where CAD Lab on this site keeps its models.
    pub store: Arc<crate::chat_store::Store>,
    pub current: Option<Current>,
    pub fallback: Option<Box<dyn Door>>,
}

async fn platform_labs(
    app: &App,
    headers: &axum::http::HeaderMap,
    project: Option<&str>,
) -> Result<Vec<Lab>, String> {
    if app.config.lab_backend.is_none() {
        return Ok(vec![]);
    }
    let cloud = app.config.cloud.as_deref().ok_or("Sign in to use Labs.")?;
    let headers = crate::labs_api::caller_headers(app, headers);
    let credential = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or("Sign in to use Labs.")?;
    let deployment =
        openagents_sdk::Deployment::new(&format!("{}/v1", cloud.origin()), cloud.origin())
            .map_err(|_| "Labs aren't available right now.")?;
    let transport =
        openagents_sdk::Client::with_timeout(deployment, std::time::Duration::from_secs(120))
            .map_err(|_| "Labs aren't available right now.")?;
    let session = transport
        .authenticate(credential)
        .await
        .map_err(|_| "Sign in to use Labs.")?;
    let owner = crate::chat_store::account_owner(&session.account().id);
    let client = transport
        .labs(&session)
        .map_err(|_| "Labs aren't available right now.")?;
    let space = project.map_or_else(|| "me".into(), |project| format!("project:{project}"));
    let cancel = openagents_sdk::CancellationToken::new();
    let manifests = client
        .kinds(&space, &cancel)
        .await
        .map_err(|_| "Labs aren't available right now.")?;
    let mut labs = vec![];
    for manifest in manifests.into_iter().take(32) {
        let page = openagents_sdk::labs::PageOptions {
            kind: Some(manifest.kind.clone()),
            limit: 100,
            ..Default::default()
        };
        let documents = client
            .documents(&space, &page, &cancel)
            .await
            .map_err(|_| "We couldn't load your Lab documents.")?;
        let tools = manifest
            .tools
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "We couldn't load the Lab tools.")?;
        labs.push(Lab {
            kind: Kind::Registered(manifest.kind.clone()), url: String::new(), tools,
            documents: documents.data.into_iter().map(|doc| json!({"id":doc.id,"title":doc.title,"kind":doc.schema,"versions":doc.latest})).collect(),
            hosted: None,
            platform: Some(Platform { manifest, client: client.clone(), space: space.clone(), owner: owner.clone() }),
        });
    }
    Ok(labs)
}

impl Platform {
    async fn invoke(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let document = arguments[&self.manifest.doc_arg]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Choose a document.")?;
        let reference = self
            .client
            .document_ref(&self.space, document)
            .map_err(|_| "Choose a valid document name.")?;
        let cancel = openagents_sdk::CancellationToken::new();
        let base = match self.client.document(&reference, &cancel).await {
            Ok(meta) if meta.kind == self.manifest.kind => Some(meta.latest),
            Ok(_) => return Err("Choose a document from this Lab.".into()),
            Err(error)
                if error.status() == Some(404)
                    || matches!(
                        error,
                        openagents_sdk::labs::Error::Session(openagents_sdk::Error::NotFound)
                    ) =>
            {
                None
            }
            Err(error) => return Err(error.to_string()),
        };
        let request = self
            .client
            .bind(openagents_sdk::labs::ToolCall {
                space: self.space.clone(),
                chat: None,
                base: arguments["base"].as_u64().or(base),
                arguments: arguments.clone(),
            })
            .map_err(|error| error.to_string())?;
        let mut result = self
            .client
            .invoke(&self.manifest.kind, name, &request, &cancel)
            .await
            .map_err(|error| {
                error
                    .refusal()
                    .map_or_else(|| error.to_string(), |refusal| refusal.message.clone())
            })?
            .result;
        if let Some(card) = result.get_mut("card") {
            if card["schema"] != lab_kit::card::SCHEMA
                || card["kind"] != self.manifest.kind
                || card["document"] != document
            {
                return Err("This Lab returned a different document. Try again.".into());
            }
            card["space"] = json!(self.space);
            card["_lab"] = json!({"space":self.space,"owner":self.owner});
        }
        Ok(result)
    }
}

fn registered_guidance(lab: &Lab) -> String {
    let Some(platform) = &lab.platform else {
        return String::new();
    };
    let manifest = &platform.manifest;
    let mut out = format!(
        "\n{}: kind {}, formats {:?}. Its document argument is {}. Use only its listed typed tools. Documents:\n{}\n",
        manifest.name,
        manifest.kind,
        manifest.schemas,
        manifest.doc_arg,
        serde_json::to_string(&lab.documents).unwrap_or_default()
    );
    if manifest.kind == "cad" {
        let mut example =
            cad::doc::new_part("plate", "Mounting plate", cad::doc::DEFAULT_MATERIAL, None);
        example["features"] =
            json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":3}}]);
        out.push_str(&format!("Use cad_from_json for a new design. Start from this part format and change its id/title/features to suit the request: {}. cad_drag_face takes a picked part, face name, and delta_mm. A tool result alone establishes that a change succeeded.\n", example));
    }
    out
}

/// The Labs door, when at least one Lab is configured and answering and
/// this server has Gemini. Robot Lab also checks its configured owner and
/// project before exposing private robot files and tools.
pub(crate) async fn door(
    app: &App,
    current: Option<Current>,
    owner: &str,
    project: Option<&str>,
    _cad: Option<crate::cad_web::Scope>,
    cad_allowed: bool,
    headers: &axum::http::HeaderMap,
) -> Option<LabsDoor> {
    let store = &app.config.chat_store;
    let (video, game, robot, cad, ship, platform) = tokio::join!(
        crate::lab::manifest_here(),
        crate::game_lab::manifest_here(),
        crate::robot_lab::manifest_here(owner, project),
        async {
            if cad_allowed {
                crate::cad_lab::manifest_here().await
            } else {
                None
            }
        },
        crate::ship_lab::manifest_here(),
        platform_labs(app, headers, project)
    );
    let mut labs: Vec<Lab> = [video, game, robot, cad, ship]
        .into_iter()
        .flatten()
        .collect();
    match platform {
        Ok(platform) => {
            labs = with_platform(labs, platform);
        }
        Err(_) => eprintln!("openagents-web: Labs registry is unavailable"),
    }
    if labs.is_empty() {
        return None;
    }
    // The Labs' turns run on Gemini 4 Argon (Vertex, global); the chat's
    // image path keeps its own model.
    let gemini = Gemini::from_env_for(vertex::ARGON)?;
    // A current document only counts while its Lab answers.
    let current = current.filter(|c| labs.iter().any(|lab| lab.kind.id() == c.kind.id()));
    Some(LabsDoor {
        gemini,
        labs,
        store: store.clone(),
        current,
        fallback: None,
    })
}

fn with_platform(mut local: Vec<Lab>, platform: Vec<Lab>) -> Vec<Lab> {
    local.retain(|lab| {
        !platform
            .iter()
            .any(|hosted| hosted.kind.id() == lab.kind.id())
    });
    local.extend(platform);
    local
}

/// What the model is told: each Lab's guidance, the route, the rule that
/// nothing is invented, and the chat's current document.
fn guidance(labs: &[Lab], current: Option<&Current>) -> String {
    let mut out = String::new();
    for lab in labs {
        out.push_str(&match lab.kind {
            Kind::Video => crate::lab::guidance(&lab.documents),
            Kind::Game => crate::game_lab::guidance(&lab.documents),
            Kind::Robot => crate::robot_lab::guidance(&lab.documents),
            Kind::Cad => crate::cad_lab::guidance(&lab.documents, lab.hosted.is_some()),
            Kind::Ship => crate::ship_lab::guidance(&lab.documents),
            Kind::Registered(_) => registered_guidance(lab),
        });
    }
    out.push_str(
        "\nNever use one Lab's tools on another Lab's document. Choose only a tool listed for that Lab. When the message is outside the available Labs, call general_chat.\nSay only what the tool results in this turn say: every version, time, number and measurement comes from a tool result. When a tool refuses, explain why in plain words and don't invent a result.\n",
    );
    match current {
        Some(c) => out.push_str(&format!(
            "\nThe chat's current Lab document (the last card it showed): {} {} \"{}\"{}. A follow-up that doesn't \
name another document (\"the latest\", \"v2\", \"compare with the original\", \"at 0:25\") is about this one, \
with its Lab's tools.\n",
            c.kind.name(),
            c.doc,
            c.title.replace('"', "'"),
            c.version.map(|v| format!(", v{v}")).unwrap_or_default(),
        )),
        None => out.push_str("\nThe chat has no current Lab document yet.\n"),
    }
    out
}

/// The function declarations Gemini gets: every Lab's tools and the
/// hand-off.
fn declarations(labs: &[Lab]) -> Vec<Value> {
    let mut out: Vec<Value> = labs
        .iter()
        .flat_map(|lab| lab.tools.iter())
        .filter(|tool| tool["name"].as_str().is_some_and(|name| !name.is_empty()))
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["parameters"],
            })
        })
        .collect();
    out.push(json!({
        "type": "function",
        "name": HANDOFF,
        "description": "Hand the message to OpenAgents' general chat for topics outside the available Labs.",
        "parameters": {"type": "object", "properties": {}},
    }));
    out
}

fn request(instructions: &str, turns: &[Turn], labs: &[Lab]) -> Value {
    let input: Vec<Value> = turns
        .iter()
        .map(|turn| {
            let role = match turn.role {
                TurnRole::User => "user",
                TurnRole::Assistant => "assistant",
            };
            json!({"type": "message", "role": role, "content": turn.text})
        })
        .collect();
    json!({
        "model": vertex::ARGON,
        "instructions": instructions,
        "input": input,
        "tools": declarations(labs),
        "stream": false,
        "store": false,
    })
}

/// Why a tool call can't go to its Lab: no such Lab here, or its document
/// is the other Lab's. `None` when it may go.
fn misrouted(labs: &[Lab], name: &str, arguments: &Value) -> Option<String> {
    let Some(lab) = labs
        .iter()
        .find(|lab| lab.tools.iter().any(|tool| tool["name"] == name))
    else {
        return Some(format!("The tool {name} isn't running here."));
    };
    let named = arguments[lab.doc_arg()].as_str()?.trim();
    let other = labs
        .iter()
        .find(|other| other.kind.id() != lab.kind.id() && other.has(named))?;
    (!lab.has(named)).then(|| {
        format!(
            "{named} is a {} document; use that Lab's tools.",
            other.name()
        )
    })
}

async fn run_tool(
    store: &crate::chat_store::Store,
    labs: &[Lab],
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    if let Some(why) = misrouted(labs, name, arguments) {
        return Err(why);
    }
    let lab = labs
        .iter()
        .find(|lab| lab.tools.iter().any(|tool| tool["name"] == name))
        .ok_or("no Lab")?;
    if let Some(platform) = &lab.platform {
        return platform.invoke(name, arguments).await;
    }
    match lab.kind {
        Kind::Registered(_) => Err("This Lab is unavailable. Try again later.".into()),
        Kind::Video => crate::lab::run_tool(&lab.url, name, arguments).await,
        Kind::Game => crate::game_lab::run_tool(&lab.url, name, arguments).await,
        Kind::Robot => crate::robot_lab::run_tool(&lab.url, name, arguments).await,
        Kind::Cad => match (&lab.hosted, crate::cad_web::engine()) {
            (Some(scope), Some(engine)) => Ok(engine.run(store, scope, name, arguments).await),
            (Some(_), None) => Err("CAD Lab can't run here right now.".into()),
            (None, _) => crate::cad_lab::run_tool(&lab.url, name, arguments).await,
        },
        Kind::Ship => crate::ship_lab::run_tool(&lab.url, name, arguments).await,
    }
}

/// How a run of the loop ended.
enum Ending {
    Answer {
        text: String,
        card: Option<Value>,
        route: Option<Kind>,
    },
    Handoff,
}

/// How the model may call tools at a step. The first step must call one
/// (a Lab tool or `general_chat`): the route is always a typed choice, and
/// an answer about a Lab document never comes before a tool has read it
/// (#11619, where a game answer came back with no tool called). The last
/// step must answer in words; the steps between choose.
fn tool_config(step: usize) -> Option<Value> {
    if step == 0 {
        Some(json!({"functionCallingConfig": {"mode": "ANY"}}))
    } else if step == MAX_STEPS {
        Some(json!({"functionCallingConfig": {"mode": "NONE"}}))
    } else {
        None
    }
}

async fn converse(
    gemini: &Gemini,
    store: &crate::chat_store::Store,
    labs: &[Lab],
    instructions: &str,
    turns: &[Turn],
) -> Result<Ending, String> {
    let mut body = gemini.body(&request(instructions, turns, labs))?;
    let mut card: Option<Value> = None;
    let mut route: Option<Kind> = None;
    for step in 0..=MAX_STEPS {
        match tool_config(step) {
            Some(config) => body["toolConfig"] = config,
            None => {
                if let Some(object) = body.as_object_mut() {
                    object.remove("toolConfig");
                }
            }
        }
        let reply = gemini_call(gemini, &body).await?;
        let content = reply["candidates"][0]["content"].clone();
        let calls: Vec<Value> = content["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["functionCall"].is_object())
            .map(|part| part["functionCall"].clone())
            .collect();
        if calls.is_empty() {
            let text = gemini_text(&reply).ok_or("Gemini answered nothing")?;
            return Ok(Ending::Answer { text, card, route });
        }
        if calls.iter().any(|call| call["name"] == HANDOFF) && route.is_none() {
            return Ok(Ending::Handoff);
        }
        let mut responses = Vec::new();
        for call in &calls {
            let name = call["name"].as_str().unwrap_or_default().to_owned();
            let arguments = match &call["args"] {
                Value::Object(_) => call["args"].clone(),
                _ => json!({}),
            };
            let response = match run_tool(store, labs, &name, &arguments).await {
                Ok(mut result) => {
                    let kind = Kind::of_tool(labs, &name);
                    route = kind.clone().or(route);
                    if let Some(found) = result
                        .get("card")
                        .filter(|c| kind.as_ref().is_some_and(|k| k.accepts(c)))
                    {
                        card = Some(found.clone());
                    }
                    // The model reads the words; the card is for the person.
                    if let Some(object) = result.as_object_mut() {
                        object.remove("card");
                        object.remove("documents");
                    }
                    json!({"result": result})
                }
                Err(why) => json!({"error": why}),
            };
            match response["result"]["error"]
                .as_str()
                .or(response["error"].as_str())
            {
                // A refusal, so a failed turn can be read back from the log.
                Some(why) => eprintln!(
                    "openagents-web: labs: {name} refused: {}",
                    why.chars().take(300).collect::<String>()
                ),
                None => eprintln!("openagents-web: labs: {name}"),
            }
            let mut part = json!({"functionResponse": {"name": name, "response": response}});
            if let Some(id) = call["id"].as_str() {
                part["functionResponse"]["id"] = json!(id);
            }
            responses.push(part);
        }
        let contents = body["contents"]
            .as_array_mut()
            .ok_or("the request has no contents")?;
        contents.push(json!({"role": "model", "parts": content["parts"].clone()}));
        contents.push(json!({"role": "user", "parts": responses}));
    }
    Err("Gemini kept calling tools".into())
}

impl Door for LabsDoor {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let instructions = format!(
            "{}{}",
            basic_coder::instructions(&context),
            guidance(&self.labs, self.current.as_ref())
        );
        let fallback = self
            .fallback
            .as_ref()
            .map(|door| door.ask(turns.clone(), context.clone(), reply.clone()));
        let gemini = self.gemini.clone();
        let labs = self.labs.clone();
        let store = self.store.clone();
        Box::pin(async move {
            match converse(&gemini, &store, &labs, &instructions, &turns).await {
                Ok(Ending::Answer { text, card, route }) => {
                    let route = card
                        .as_ref()
                        .and_then(Kind::of_card)
                        .or(route)
                        .map_or("labs", |kind| kind.card());
                    let mut reply = basic_coder::lock(&reply);
                    reply.text = text;
                    reply.model = Some(gemini.row.id.clone());
                    reply.meta.tier = Some("model".into());
                    reply.meta.route = Some(route.into());
                    reply.meta.cards = card.into_iter().collect();
                    reply.done = true;
                }
                other => {
                    if let Err(why) = &other {
                        eprintln!("openagents-web: labs: {why}; the next chat answers");
                    }
                    match fallback {
                        Some(job) => job.await,
                        None => {
                            basic_coder::lock(&reply).failure = Some(Failure::Transport(
                                other.err().unwrap_or_else(|| "no general chat".into()),
                            ));
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labs() -> Vec<Lab> {
        vec![
            Lab {
                kind: Kind::Video,
                url: "http://127.0.0.1:8792".into(),
                tools: vec![json!({"name": "lab_compare", "description": "d", "parameters": {}})],
                documents: vec![
                    json!({"id": "episode-290", "title": "Episode 290", "kind": "cut.v1", "versions": 15}),
                ],
                hosted: None,
                platform: None,
            },
            Lab {
                kind: Kind::Game,
                url: "http://127.0.0.1:8793".into(),
                tools: vec![json!({"name": "game_compare", "description": "d", "parameters": {}})],
                documents: vec![
                    json!({"id": "the-ice-cream-man", "title": "The Ice Cream Man", "kind": "game.v1", "versions": 3}),
                ],
                hosted: None,
                platform: None,
            },
            Lab {
                kind: Kind::Robot,
                url: "http://127.0.0.1:8794".into(),
                tools: vec![json!({"name":"robot_compare","description":"d","parameters":{}})],
                documents: vec![
                    json!({"id":"robot-table","title":"SO-101 table","kind":"robot-scene.v1","versions":2}),
                ],
                hosted: None,
                platform: None,
            },
        ]
    }

    #[test]
    fn a_registered_kind_replaces_its_loopback_adapter() {
        let mut hosted = labs().remove(0);
        hosted.kind = Kind::Registered("video".into());
        hosted.url.clear();
        let combined = with_platform(labs(), vec![hosted]);
        assert_eq!(combined.len(), 3);
        let video = combined
            .iter()
            .find(|lab| lab.kind.id() == "video")
            .unwrap();
        assert!(matches!(video.kind, Kind::Registered(_)));
        assert!(video.url.is_empty());
        assert!(combined.iter().any(|lab| lab.kind == Kind::Game));
    }

    #[tokio::test]
    async fn a_registered_film_followup_cannot_run_a_game_tool() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let router = axum::Router::new().route(
            "/v1/account",
            axum::routing::get(move || {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    axum::Json(
                        json!({"account":{"id":"fixture-alice","label":"Alice"},"workspaces":[]}),
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = openagents_sdk::Client::new(
            openagents_sdk::Deployment::new(&format!("{origin}/v1"), &origin).unwrap(),
        )
        .unwrap();
        let session = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        let client = client.labs(&session).unwrap();
        let mut registered = labs();
        for lab in &mut registered {
            use lab_kit::LabKind;
            let mut manifest = lab_kit::echo::Echo::manifest();
            manifest.kind = lab.kind.id().into();
            manifest.name = lab.kind.name().into();
            manifest.doc_arg = lab.kind.doc_arg().into();
            lab.kind = Kind::Registered(manifest.kind.clone());
            lab.platform = Some(Platform {
                manifest,
                client: client.clone(),
                space: "me".into(),
                owner: crate::chat_store::account_owner("fixture-alice"),
            });
            lab.url.clear();
        }
        let replies = [
            meta(vec![
                json!({"schema":lab_kit::card::SCHEMA,"kind":"video","document":"episode-290","title":"Episode 290","version":15}),
            ]),
            meta(vec![]),
        ];
        let current = current(replies.iter()).unwrap();
        assert_eq!(current.kind, Kind::Registered("video".into()));
        assert_eq!(current.doc, "episode-290");
        assert_eq!(
            Kind::of_tool(&registered, "lab_compare"),
            Some(current.kind.clone())
        );
        assert!(
            misrouted(
                &registered,
                "lab_compare",
                &json!({"document":"episode-290"})
            )
            .is_none()
        );
        let root = tempfile::tempdir().unwrap();
        let store = crate::chat_store::Store::local(root.path().to_path_buf());
        let before = calls.load(Ordering::SeqCst);
        let why = run_tool(
            &store,
            &registered,
            "game_compare",
            &json!({"game":"episode-290","a":2}),
        )
        .await
        .unwrap_err();
        assert!(why.contains("Video Lab document"), "{why}");
        assert_eq!(calls.load(Ordering::SeqCst), before);
        server.abort();
    }

    fn meta(cards: Vec<Value>) -> Meta {
        Meta {
            cards,
            ..Default::default()
        }
    }

    #[test]
    fn one_turn_sees_each_labs_tools_and_the_hand_off() {
        let names: Vec<String> = declarations(&labs())
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            ["lab_compare", "game_compare", "robot_compare", HANDOFF]
        );
    }

    #[test]
    fn the_current_document_is_the_last_lab_card_shown() {
        let replies = [
            meta(vec![
                json!({"card": "video_lab", "doc": "episode-290", "title": "Episode 290", "version": 15}),
            ]),
            meta(vec![json!({"card": "tool"})]),
            meta(vec![]),
        ];
        let found = current(replies.iter()).unwrap();
        assert_eq!(found.kind, Kind::Video);
        assert_eq!(found.doc, "episode-290");
        assert_eq!(found.version, Some(15));
        let text = guidance(&labs(), Some(&found));
        assert!(text.contains("current Lab document"), "{text}");
        assert!(
            text.contains("Video Lab episode-290 \"Episode 290\", v15"),
            "{text}"
        );

        let replies = [
            meta(vec![
                json!({"card": "video_lab", "doc": "episode-290", "title": "Episode 290", "version": 15}),
            ]),
            meta(vec![
                json!({"card": "game_lab", "doc": "the-ice-cream-man", "title": "The Ice Cream Man", "version": 2}),
            ]),
        ];
        let found = current(replies.iter()).unwrap();
        assert_eq!(found.kind, Kind::Game);
        assert_eq!(found.doc, "the-ice-cream-man");
        assert!(current([meta(vec![])].iter()).is_none());
    }

    #[test]
    fn a_tool_on_the_other_labs_document_is_refused_before_it_runs() {
        let labs = labs();
        let why = misrouted(
            &labs,
            "game_compare",
            &json!({"game": "episode-290", "a": 2}),
        )
        .unwrap();
        assert!(why.contains("episode-290 is a Video Lab document"), "{why}");
        let why = misrouted(
            &labs,
            "lab_compare",
            &json!({"document": "the-ice-cream-man"}),
        )
        .unwrap();
        assert!(why.contains("Game Lab document"), "{why}");
        assert!(
            misrouted(&labs, "game_compare", &json!({"game":"robot-table"}))
                .unwrap()
                .contains("Robot Lab document")
        );
        assert!(
            misrouted(&labs, "robot_compare", &json!({"document":"episode-290"}))
                .unwrap()
                .contains("Video Lab document")
        );
        assert!(misrouted(&labs, "robot_compare", &json!({"document":"robot-table"})).is_none());
        assert_eq!(
            misrouted(
                &labs,
                "game_compare",
                &json!({"game": "the-ice-cream-man", "a": 1})
            ),
            None
        );
        assert_eq!(
            misrouted(
                &labs,
                "lab_compare",
                &json!({"document": "episode-290", "a": "2"})
            ),
            None
        );
        // An unknown id goes to its Lab, which refuses it by name.
        assert_eq!(
            misrouted(&labs, "game_compare", &json!({"game": "nope"})),
            None
        );
        assert!(misrouted(&labs, "shell_run", &json!({})).is_some());
        // Prefix-like names never add a tool that the registry did not list.
        assert_eq!(Kind::of_tool(&labs, "cad_add_feature"), None);
        assert!(
            misrouted(&labs, "cad_open", &json!({"doc": "rover"}))
                .unwrap()
                .contains("isn't running")
        );
        assert!(
            misrouted(&labs[..1], "game_compare", &json!({}))
                .unwrap()
                .contains("isn't running")
        );
    }

    #[test]
    fn the_model_is_told_to_say_only_what_tools_say() {
        let text = guidance(&labs(), None);
        assert!(text.contains("Say only what the tool results in this turn say"));
        assert!(text.contains("Never use one Lab's tools on another Lab's document"));
        assert!(text.contains("no current Lab document"));
    }

    #[test]
    fn the_first_step_must_call_a_tool_and_the_last_must_answer() {
        assert_eq!(
            tool_config(0).unwrap()["functionCallingConfig"]["mode"],
            "ANY"
        );
        assert_eq!(tool_config(1), None);
        assert_eq!(
            tool_config(MAX_STEPS).unwrap()["functionCallingConfig"]["mode"],
            "NONE"
        );
    }
}
