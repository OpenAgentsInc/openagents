//! The Labs in the web chat: Video Lab ([`crate::lab`]), Game Lab
//! ([`crate::game_lab`]) and CAD Lab ([`crate::cad_lab`]) behind one door
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
const MAX_STEPS: usize = 8;

/// Which Lab a tool, a document or a card belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Video,
    Game,
    Robot,
    Cad,
}

impl Kind {
    /// The Lab a tool name belongs to, by its typed prefix.
    fn of_tool(name: &str) -> Option<Kind> {
        if name.starts_with("lab_") {
            Some(Kind::Video)
        } else if name.starts_with("game_") {
            Some(Kind::Game)
        } else if name.starts_with("robot_") {
            Some(Kind::Robot)
        } else if name.starts_with("cad_") {
            Some(Kind::Cad)
        } else {
            None
        }
    }

    /// The Lab a card belongs to, by its card kind.
    fn of_card(card: &Value) -> Option<Kind> {
        match card["card"].as_str()? {
            crate::lab::CARD => Some(Kind::Video),
            crate::game_lab::CARD => Some(Kind::Game),
            crate::robot_lab::CARD => Some(Kind::Robot),
            crate::cad_lab::CARD => Some(Kind::Cad),
            _ => None,
        }
    }

    fn card(self) -> &'static str {
        match self {
            Kind::Video => crate::lab::CARD,
            Kind::Game => crate::game_lab::CARD,
            Kind::Robot => crate::robot_lab::CARD,
            Kind::Cad => crate::cad_lab::CARD,
        }
    }

    /// The argument a tool of this Lab names its document by.
    fn doc_arg(self) -> &'static str {
        match self {
            Kind::Video => "document",
            Kind::Game => "game",
            Kind::Robot => "document",
            Kind::Cad => "doc",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Video => "Video Lab",
            Kind::Game => "Game Lab",
            Kind::Robot => "Robot Lab",
            Kind::Cad => "CAD Lab",
        }
    }
}

/// One Lab this server talks to: where it runs, its tools, its documents.
#[derive(Clone)]
pub(crate) struct Lab {
    pub kind: Kind,
    pub url: String,
    pub tools: Vec<Value>,
    pub documents: Vec<Value>,
}

impl Lab {
    fn has(&self, id: &str) -> bool {
        self.documents.iter().any(|doc| doc["id"] == id)
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
            let doc = card["doc"].as_str().filter(|d| !d.is_empty())?;
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
    pub current: Option<Current>,
    pub fallback: Option<Box<dyn Door>>,
}

/// The Labs door, when at least one Lab is configured and answering and
/// this server has Gemini. Robot Lab also checks its configured owner and
/// project before exposing private robot files and tools.
pub(crate) async fn door(
    _app: &App,
    current: Option<Current>,
    owner: &str,
    project: Option<&str>,
) -> Option<LabsDoor> {
    let (video, game, robot, cad) = tokio::join!(
        crate::lab::manifest_here(),
        crate::game_lab::manifest_here(),
        crate::robot_lab::manifest_here(owner, project),
        crate::cad_lab::manifest_here()
    );
    let labs: Vec<Lab> = [video, game, robot, cad].into_iter().flatten().collect();
    if labs.is_empty() {
        return None;
    }
    // The Labs' turns run on Gemini 4 Argon (Vertex, global); the chat's
    // image path keeps its own model.
    let gemini = Gemini::from_env_for(vertex::ARGON)?;
    // A current document only counts while its Lab answers.
    let current = current.filter(|c| labs.iter().any(|lab| lab.kind == c.kind));
    Some(LabsDoor {
        gemini,
        labs,
        current,
        fallback: None,
    })
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
            Kind::Cad => crate::cad_lab::guidance(&lab.documents),
        });
    }
    out.push_str(
        "\nFilms, cuts, shots, scenes and videos are Video Lab documents (lab_* tools); games are Game Lab documents \
(game_* tools); robot scenes and attempts are Robot Lab documents (robot_* tools); parts, assemblies and CAD models are CAD Lab documents (cad_* tools). Never use one Lab's tools on another Lab's document. When the message is about none of these Labs, call \
general_chat.\nSay only what the tool results in this turn say: every version, time, number and measurement you \
mention comes from a tool result. When a tool refuses (a document or version that doesn't exist, an edit it won't \
make), say so plainly in a sentence and don't guess an answer.\n",
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
        .filter(|tool| tool["name"].as_str().and_then(Kind::of_tool).is_some())
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
    let Some(kind) = Kind::of_tool(name) else {
        return Some(format!("{name} isn't a Lab tool."));
    };
    if !labs.iter().any(|lab| lab.kind == kind) {
        return Some(format!("{} isn't running on this Mac.", kind.name()));
    }
    let named = arguments[kind.doc_arg()].as_str()?.trim();
    let other = labs.iter().find(|lab| lab.kind != kind && lab.has(named))?;
    let own = labs.iter().any(|lab| lab.kind == kind && lab.has(named));
    (!own).then(|| {
        format!(
            "{named} is a {} document, not a {} one; nothing was done. Use its own Lab's tools ({}*).",
            other.kind.name(),
            kind.name(),
            match other.kind {
                Kind::Video => "lab_",
                Kind::Game => "game_",
                Kind::Robot => "robot_",
                Kind::Cad => "cad_",
            }
        )
    })
}

async fn run_tool(labs: &[Lab], name: &str, arguments: &Value) -> Result<Value, String> {
    if let Some(why) = misrouted(labs, name, arguments) {
        return Err(why);
    }
    let lab = labs
        .iter()
        .find(|lab| Some(lab.kind) == Kind::of_tool(name))
        .ok_or("no Lab")?;
    match lab.kind {
        Kind::Video => crate::lab::run_tool(&lab.url, name, arguments).await,
        Kind::Game => crate::game_lab::run_tool(&lab.url, name, arguments).await,
        Kind::Robot => crate::robot_lab::run_tool(&lab.url, name, arguments).await,
        Kind::Cad => crate::cad_lab::run_tool(&lab.url, name, arguments).await,
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
            let response = match run_tool(labs, &name, &arguments).await {
                Ok(mut result) => {
                    let kind = Kind::of_tool(&name);
                    route = kind.or(route);
                    if let Some(found) = result
                        .get("card")
                        .filter(|c| kind.is_some_and(|k| c["card"] == k.card()))
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
            eprintln!("openagents-web: labs: {name}");
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
        Box::pin(async move {
            match converse(&gemini, &labs, &instructions, &turns).await {
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
            },
            Lab {
                kind: Kind::Game,
                url: "http://127.0.0.1:8793".into(),
                tools: vec![json!({"name": "game_compare", "description": "d", "parameters": {}})],
                documents: vec![
                    json!({"id": "the-ice-cream-man", "title": "The Ice Cream Man", "kind": "game.v1", "versions": 3}),
                ],
            },
            Lab {
                kind: Kind::Robot,
                url: "http://127.0.0.1:8794".into(),
                tools: vec![json!({"name":"robot_compare","description":"d","parameters":{}})],
                documents: vec![
                    json!({"id":"robot-table","title":"SO-101 table","kind":"robot-scene.v1","versions":2}),
                ],
            },
        ]
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
        // CAD Lab tools route by their prefix and name their document as doc.
        assert_eq!(Kind::of_tool("cad_add_feature"), Some(Kind::Cad));
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
