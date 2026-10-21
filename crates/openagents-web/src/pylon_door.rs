//! The `/home` terminal asks the pylons first (#12366).
//!
//! A turn sent from the Grid terminal on `/home` (the lander chat client's
//! `X-OpenAgents-Chat-Surface: home`, [`crate::visitor_api::home_surface`])
//! goes to [`PylonDoor`] before the
//! hosted chat. The door runs the pylon router ([`pylon::router::route`]):
//! the live pylons that can take a free conversation job are ranked in
//! code, and when a pylon serving decisions (Clef) is online, one typed
//! Choice through the Decisions API, asked of a connected Clef pylon first
//! (`X-Decision-Order: pylon`), picks which of them answers or hands the
//! message to the hosted chat. The chosen pylon answers over NIP-CJ, the
//! next one when it can't, and the hosted chat answers when none does.
//!
//! A turn that names DeepSeek V4.1 Flash in `X-OpenAgents-Pylon-Model`
//! ([`GROUP_MODEL`]) goes only to a ready model group serving it, with no
//! decision, under `pylon.deadline.v1` with W14's 600-second budget
//! ([`GROUP_DEADLINE`]). Every other turn routes as above.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use jev::decisions::{DecisionAnswer, DecisionsClient, DecisionsRequest};
use openagents_chat::basic_coder::{self, Door, Failure, Reply, Role, Turn};
use openagents_chat::router::Context;
use pylon::identity::Identity;
use pylon::router::{self, Decide, Deciding, How, Outcome, Question, Settings, Verdict};
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};

use crate::App;

/// Turns the router off when set to `off`.
const SWITCH_VAR: &str = "OPENAGENTS_WEB_PYLON_ROUTER";
/// Where the router records its job receipts.
const HOME_VAR: &str = "OPENAGENTS_WEB_PYLON_HOME";
/// The Decisions API's base URL, as the decision clients read it.
const DECISIONS_URL_VAR: &str = "OPENAGENTS_DECISIONS_URL";
/// The header a Grid terminal turn names a group model in.
const MODEL_HEADER: &str = "x-openagents-pylon-model";
/// The one model a terminal turn may ask for by name: DeepSeek V4.1 Flash,
/// which runs as a model group.
pub(crate) const GROUP_MODEL: &str = pylon::placement::MODEL;
/// The budget for one V4.1 answer: W14's frozen deadline.
pub(crate) const GROUP_DEADLINE: Duration = Duration::from_secs(600);
/// How long a turn that asks for V4.1 may take in all: the deadline, the
/// grace for the answer to cross the relay, and finding the group.
pub(crate) const GROUP_TURN: Duration = Duration::from_secs(600 + 10 + 20);
/// Our decision model: Clef first.
const MODEL: &str = "openagents/decide";
/// What a pylon is told about the turn it answers.
const INSTRUCTIONS: &str = "You are a pylon: a computer on the OpenAgents network. Someone \
typed this at the terminal on the OpenAgents Grid. Answer directly, in plain words, in a few \
sentences unless they ask for more.";

/// The key a person's pylon jobs are sent as: derived from the server's
/// secret and the payer, apart from their worker key, so a pylon's rate
/// limit is per person.
fn buyer(salt: &[u8; 32], payer: &str) -> SecretKey {
    let mut seed: [u8; 32] = Sha256::new()
        .chain_update(b"openagents-web-pylon-buyer-v1")
        .chain_update(salt)
        .chain_update(payer.as_bytes())
        .finalize()
        .into();
    loop {
        if let Ok(key) = SecretKey::from_byte_array(seed) {
            return key;
        }
        seed = Sha256::digest(seed).into();
    }
}

/// Where the router's decision goes: `OPENAGENTS_DECISIONS_URL`, else the
/// inference gateway beside this server, else the public API.
fn decisions_url(app: &App) -> String {
    std::env::var(DECISIONS_URL_VAR)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .or_else(|| app.config.inference.as_ref().map(|up| up.base_url()))
        .unwrap_or_else(|| jev::decisions::OPENAGENTS.to_owned())
}

/// The Decisions API as the router's [`Decide`].
struct Api {
    client: DecisionsClient,
}

impl Decide for Api {
    fn decide<'a>(&'a self, question: &'a Question) -> Deciding<'a> {
        Box::pin(async move {
            let request = DecisionsRequest::new(MODEL, question.input.clone())
                .choice(
                    question.name.clone(),
                    question.instructions.clone(),
                    question.choices.clone(),
                )
                .metadata("question_set", router::QUESTION_SET);
            let response = self
                .client
                .create_with(&request, None, &[("x-decision-order", "pylon")])
                .await
                .map_err(|e| e.to_string())?;
            match response.answer(&question.name) {
                Some(DecisionAnswer::Choice {
                    choice, confidence, ..
                }) => Ok(Verdict {
                    choice: choice.clone(),
                    confidence: *confidence,
                    door: response.openagents.door.clone(),
                    model: Some(response.model.clone()),
                }),
                Some(DecisionAnswer::Refusal { code, .. }) => Err(format!("refused: {code}")),
                _ => Err("the answer has no choice".into()),
            }
        })
    }
}

/// A door that asks the live pylons, or hands the message to `fallback`.
pub(crate) struct PylonDoor {
    pub settings: Settings,
    pub buyer: Identity,
    pub decide: Arc<dyn Decide>,
    pub fallback: Option<Box<dyn Door>>,
}

/// The pylon door for a turn, when it came from the Grid terminal, this
/// server watches a pylon relay, and the router isn't switched off.
pub(crate) fn door(app: &App, headers: &HeaderMap, payer: &str) -> Option<PylonDoor> {
    if !crate::visitor_api::home_surface(headers)
        || std::env::var(SWITCH_VAR).is_ok_and(|v| v.trim().eq_ignore_ascii_case("off"))
    {
        return None;
    }
    let relay = app.config.pylons_relay.clone()?;
    let mut settings = Settings::new(
        relay,
        std::env::var(HOME_VAR).map_or_else(
            |_| std::env::temp_dir().join("openagents-web-pylons"),
            PathBuf::from,
        ),
    );
    if asks_for_group(headers) {
        settings.want = Some(pylon::group::Want::Model(GROUP_MODEL.into()));
        settings.deadline = Some(GROUP_DEADLINE);
    }
    settings.checkers = std::env::var("OPENAGENTS_PYLON_CHECKERS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|key| pylon::identity::hex_pubkey(key.trim()))
        .collect();
    let client = DecisionsClient::new(decisions_url(app))
        .ok()?
        .timeout(settings.decide);
    Some(PylonDoor {
        buyer: Identity::from_secret(buyer(&app.config.ask_salt, payer)).ok()?,
        settings,
        decide: Arc::new(Api { client }),
        fallback: None,
    })
}

/// Whether the turn names [`GROUP_MODEL`] in [`MODEL_HEADER`]; any other
/// value is ignored, and the turn routes as a default one.
fn asks_for_group(headers: &HeaderMap) -> bool {
    headers
        .get(MODEL_HEADER)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == GROUP_MODEL)
}

impl PylonDoor {
    /// Whether this door asks a model group, whose answer may take
    /// [`GROUP_TURN`].
    pub(crate) fn asks_group(&self) -> bool {
        self.settings.want.is_some()
    }
}

/// The reply's text: one line naming who answered, over the answer.
/// A V4.1 group reads "DeepSeek V4.1 · group w16-g5 (4 parts)"; any other
/// pylon reads its name and its model, exactly as its beacon gives them.
fn shown(label: &str, model: &str, group: Option<&router::AnsweredGroup>, text: &str) -> String {
    let source = match group {
        Some(group) => format!(
            "**{GROUP_NAME}** · group {}{}",
            literal(&group.id),
            match group.stages {
                0 => String::new(),
                1 => " (1 part)".into(),
                n => format!(" ({n} parts)"),
            }
        ),
        None => format!("**{}** · {}", literal(label), literal(model)),
    };
    let body = match without_source(text).trim() {
        "" => text.trim(),
        body => body,
    };
    format!("{source}\n\n{body}")
}

/// What a person calls [`GROUP_MODEL`].
const GROUP_NAME: &str = "DeepSeek V4.1";

/// `text` on one line, at most 64 characters, with Markdown's marks
/// escaped so it shows as written: `q8_0` stays `q8_0`.
fn literal(text: &str) -> String {
    let mut out = String::new();
    for c in text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(64)
    {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `text` without the source lines [`shown`] puts over an answer: what an
/// earlier answer in the conversation carries, and what a small model may
/// copy from one. Earlier turns go to a pylon without them (#12421).
pub(crate) fn without_source(text: &str) -> &str {
    let mut rest = text.trim_start();
    while let Some(line) = rest.strip_prefix("**") {
        let (first, after) = line.split_once('\n').unwrap_or((line, ""));
        let Some((name, from)) = first.split_once("** · ") else {
            break;
        };
        if name.is_empty() || from.trim().is_empty() || name.len() > 160 || from.len() > 160 {
            break;
        }
        rest = after.trim_start();
    }
    rest
}

/// Whether `answer` repeats an earlier answer in `turns` word for word.
fn repeats(turns: &[pylon::engine::Turn], answer: &str) -> bool {
    let answer = without_source(answer).trim();
    !answer.is_empty()
        && turns
            .iter()
            .any(|t| t.role == "assistant" && without_source(&t.content).trim() == answer)
}

/// How routing went, for the log: never the message or the answer.
fn log_line(routed: &router::Routed, ms: u128) -> String {
    let outcome = match &routed.outcome {
        Outcome::Answered(answer) => format!("answered by {}", answer.pylon),
        Outcome::Declined => "handed to the hosted chat".into(),
        Outcome::Unavailable(why) if why == "no pylon answered" => why.clone(),
        Outcome::Unavailable(why) => format!("no pylon answered ({why})"),
    };
    let how = match &routed.how {
        How::Decided {
            choice,
            confidence,
            door,
            ms,
            ..
        } => format!(
            "decided {choice} at {confidence:.2} by {} in {ms} ms",
            door.as_deref().unwrap_or("an unnamed door")
        ),
        How::Ranked { why } => format!("ranked ({why})"),
    };
    let mut line = format!(
        "openagents-web: pylon router: {outcome}; {how}; {} candidates, {} deciders, {} failed, {ms} ms",
        routed.candidates,
        routed.deciders,
        routed.failed.len()
    );
    // Each pylon that gave no answer, by its slug, and the reason it gave
    // (#12416), one line, bounded: a pylon writes the reason.
    for (address, why) in &routed.failed {
        let slug = address.rsplit(':').next().unwrap_or_default();
        let why: String = why
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(160)
            .collect();
        line.push_str(&format!("; {slug}: {why}"));
    }
    line
}

impl Door for PylonDoor {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let turns: Vec<Turn> = turns
            .into_iter()
            .map(|mut turn| {
                if turn.role == Role::Assistant {
                    turn.text = without_source(&turn.text).to_owned();
                }
                turn
            })
            .collect();
        let fallback = self
            .fallback
            .as_ref()
            .map(|door| door.ask(turns.clone(), context, reply.clone()));
        let (settings, buyer, decide) = (
            self.settings.clone(),
            self.buyer.clone(),
            self.decide.clone(),
        );
        Box::pin(async move {
            // Earlier answers go without the line naming who answered, which
            // a small model otherwise copies, with its answer, into the next
            // one (#12421).
            let jobs: Vec<pylon::engine::Turn> = turns
                .iter()
                .map(|turn| match turn.role {
                    Role::User => pylon::engine::Turn {
                        role: "user".into(),
                        content: turn.text.clone(),
                    },
                    Role::Assistant => pylon::engine::Turn {
                        role: "assistant".into(),
                        content: without_source(&turn.text).to_owned(),
                    },
                })
                .filter(|turn| !turn.content.trim().is_empty())
                .collect();
            let started = Instant::now();
            let mut routed = router::route(
                &buyer,
                &settings,
                decide.as_ref(),
                &jobs,
                Some(INSTRUCTIONS),
            )
            .await;
            eprintln!("{}", log_line(&routed, started.elapsed().as_millis()));
            // A small model can answer a new message with its last answer,
            // word for word. Ask once more with only what the person wrote,
            // which it doesn't copy.
            if let Outcome::Answered(answer) = &routed.outcome
                && repeats(&jobs, answer.text.as_deref().unwrap_or_default())
            {
                let theirs: Vec<pylon::engine::Turn> =
                    jobs.iter().filter(|t| t.role == "user").cloned().collect();
                let again = Instant::now();
                routed = router::route(
                    &buyer,
                    &settings,
                    decide.as_ref(),
                    &theirs,
                    Some(INSTRUCTIONS),
                )
                .await;
                eprintln!(
                    "{} (asked again: the first answer repeated an earlier one)",
                    log_line(&routed, again.elapsed().as_millis())
                );
            }
            if let Outcome::Answered(answer) = routed.outcome {
                let slug = answer.pylon.rsplit(':').next().unwrap_or_default();
                let mut reply = basic_coder::lock(&reply);
                reply.text = shown(
                    &answer.label,
                    &answer.model,
                    routed.group.as_ref(),
                    answer.text.as_deref().unwrap_or_default(),
                );
                reply.model = Some(answer.model.clone());
                reply.meta.tier = Some("model".into());
                reply.meta.route = Some(format!("pylon:{slug}"));
                reply.done = true;
                return;
            }
            match fallback {
                Some(job) => job.await,
                None => {
                    basic_coder::lock(&reply).failure =
                        Some(Failure::Transport("no pylon answered".into()));
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "pylon_door_tests.rs"]
mod tests;
