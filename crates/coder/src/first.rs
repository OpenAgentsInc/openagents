//! The first response: one typed judgment the moment a message arrives.
//!
//! A conversation turn on the chat worker used to show nothing until the
//! model's first token, which on the Gemini Flash lane is three to four
//! seconds after the request reaches the worker. This module is what fills
//! that gap. For a turn that asks (`"opener": true`, or `"judge": true` for
//! the judgment alone), the worker asks one System One (Jev) request over
//! the user's message and the bounded transcript, in parallel with the
//! model call and never in front of it, and reads five independent
//! questions from the same state:
//!
//! - `action`: the turn's route, worded exactly as [`crate::classify`]'s
//!   measured `coder-turns-v2` question, so its answer means the same.
//! - `lane`: whether the request can be answered in the chat or needs a
//!   computer — a repository, files, commands, or changes.
//! - `answer`: which of the prepared [`ANSWERS`] (the `chat-answers-v1`
//!   bank) fully answers the message as asked, or `none`.
//! - `needs_specifics`: whether a good reply has to refer to specific
//!   things the user named, which no prepared answer can.
//! - `opener`: which of the [`OPENERS`] the reply should open with, or
//!   `none`.
//!
//! Code, not the judge, decides what is shown ([`Triage::tier`]): a
//! prepared answer that is sure enough ([`ANSWER_CONFIDENCE`]) and needs no
//! specifics ([`SPECIFICS_CEILING`]) is the whole reply, and the worker
//! drops the model call; otherwise an opener that is sure enough
//! ([`OPENER_CONFIDENCE`]) leads the model's reply; otherwise nothing is
//! shown before the model's own words. Every line here speaks as
//! OpenAgents, in the plural, and none is filler: a line that does not
//! say something true and useful about this message is not in the set.
//!
//! Nothing here is keyword matching: every reading is a Choice answer's
//! argmax, or a Noul's probability, over options this module lists, which
//! is the typed semantic selector `AGENTS.md` asks for. This is the first
//! step of the chat router (`docs/coder/design/2026-09-28-chat-router.md`):
//! the bank's ids, its `when` descriptions, its fact slots, and its
//! `sources` follow that design.
//!
//! [`rank_questions`] and [`ranking`] are the same judgment turned to the
//! phone's suggestions: the caller names its candidate repositories or
//! actions, and the answer orders them. Read
//! `docs/coder/measurements/2026-09-28-first-reply.md` for what this saves
//! and `nips/openagents/NIP-CJ.md` for the wire shapes.
//!
//! Everything here is pure: a state in, a request out; an answer in, a
//! reading out. The caller owns the HTTP.

use std::time::Duration;

use indexmap::IndexMap;
use jev::{
    Answer, Choice, ChoiceAnswer, Entry, Noul, NoulCriteria, Questions, RetryPolicy,
    SystemOneResponse,
};
use serde_json::{Value, json};

use crate::classify::{Route, route};
use crate::generate::{DEFAULT_DOOR_URL, Lane as ModelLane, Message};

/// The question set's identity, for evidence and for the wire.
pub const SET: &str = "coder-first-response-v2";

/// The answer bank's identity, for evidence and for the wire. A result
/// written from the bank names this as its `model`, prefixed `bank:`.
pub const BANK: &str = "chat-answers-v1";

/// How long the worker waits for the judgment before it gives up on it.
/// Past this the model's own first words are close, and an opener that
/// arrives after them is not shown at all.
pub const BUDGET: Duration = Duration::from_millis(2_500);

/// The least `answer` probability at which a prepared answer is the whole
/// reply. A wrong prepared answer is the failure a user remembers, so this
/// is high.
pub const ANSWER_CONFIDENCE: f64 = 0.80;

/// The most `needs_specifics` probability at which a prepared answer may
/// stand as the whole reply: above it the user's own words matter, and the
/// model answers.
pub const SPECIFICS_CEILING: f64 = 0.30;

/// The least `opener` probability at which an opener leads the model's
/// reply. Below it nothing is shown before the model's own words.
pub const OPENER_CONFIDENCE: f64 = 0.70;

/// The instruction the worker adds to the caller's, so the model speaks as
/// OpenAgents and does not open with an acknowledgement of its own after
/// the one that may be shown.
pub const MODEL_NOTE: &str = "We are OpenAgents: always speak as \"we\" and \"us\", never \
\"I\" or \"me\". The user may already see a short opening line above your reply, such as \
\"Here's how that works.\" or \"Sorry about that.\", so do not open with an acknowledgement, \
apology, or greeting: begin directly with the substance.";

/// One prepared answer: a complete reply in the OpenAgents voice, for the
/// messages its `when` describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canned {
    /// The id, as the chat router names it; never reused.
    pub id: &'static str,
    /// Bumped whenever `text` changes.
    pub version: u32,
    /// The router route it belongs to.
    pub route: &'static str,
    /// The messages it answers, written for the judge, with what it does
    /// not cover when a neighbor is close.
    pub when: &'static str,
    /// What the user sees. `{slot}`s are filled from [`Facts`]; an entry
    /// with a slot the worker cannot fill is not offered.
    pub text: &'static str,
    /// Where each factual claim in `text` comes from, as repository paths.
    pub sources: &'static [&'static str],
}

impl Canned {
    /// `id@version`, as the wire names an answer.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    /// The text with its slots filled, or `None` when a slot has no value.
    #[must_use]
    pub fn render(&self, facts: &Facts) -> Option<String> {
        let mut text = self.text.to_string();
        for (slot, value) in [
            ("{chat_model}", facts.chat_model.as_deref()),
            ("{chat_model_host}", facts.chat_model_host.as_deref()),
        ] {
            if text.contains(slot) {
                text = text.replace(slot, value?);
            }
        }
        (!text.contains('{')).then_some(text)
    }
}

/// The prepared answers the judgment chooses from: the `chat-answers-v1`
/// bank. Each is short, true before any work happens, promises nothing
/// the turn may not do, and speaks as "we". There is deliberately no
/// pricing or privacy answer yet: those claims need a tested invariant to
/// cite first (see the chat router design).
pub const ANSWERS: &[Canned] = &[
    Canned {
        id: "meta.who",
        version: 1,
        route: "meta",
        when: "The user asks who or what we are, who made or built us, what this assistant or \
               app is, or asks us to introduce ourselves; not which AI model powers the chat, \
               and not a detailed question about what we can do",
        text: "We are OpenAgents. In this chat we answer questions, explain things, and help \
               you plan and write. When something needs a computer, like reading or changing \
               a repository or running commands, we dispatch Coder, our coding agent, to a \
               computer you've connected.",
        sources: &[
            "crates/openagents-mobile/src/basic_coder.rs",
            "docs/deployment/chat-worker.md",
        ],
    },
    Canned {
        id: "meta.model",
        version: 1,
        route: "meta",
        when: "The user asks what AI model or LLM powers this chat, or whether we are ChatGPT, \
               Claude, Gemini, or another named assistant; not who made or built us",
        text: "Our chat runs on {chat_model} through {chat_model_host}. A small, fast model, \
               Jev from TypeSafe, reads each message first to choose how we answer.",
        sources: &[
            "crates/coder/src/generate.rs",
            "crates/coder/src/first.rs",
            "docs/deployment/chat-worker.md",
        ],
    },
    Canned {
        id: "meta.capabilities",
        version: 2,
        route: "meta",
        when: "The user asks in general what we can do, how we can help, or whether we can \
               write code, without naming a project, file, error, or task of their own",
        text: "In this chat we can answer questions, explain code and concepts, and help you \
               plan and write. We can't run code, read files, or reach your computer from \
               here: work on code and repositories goes to Coder, our coding agent, which we \
               dispatch to a computer you connect, with this conversation as its task.",
        sources: &["crates/openagents-mobile/src/basic_coder.rs"],
    },
    Canned {
        id: "meta.limits_chat",
        version: 2,
        route: "meta",
        when: "The user asks whether we can see their files or screen, run code, browse, or \
               reach their computer or repository from this chat, without asking for a \
               specific task to be done",
        text: "In this chat we can't run code, read files, or reach your computer or accounts. \
               That work goes to Coder, our coding agent, which we dispatch to a computer you \
               connect.",
        sources: &["crates/openagents-mobile/src/basic_coder.rs"],
    },
    Canned {
        id: "meta.coder",
        version: 2,
        route: "meta",
        when: "The user asks what Coder is or how Coder works, in general",
        text: "Coder is our coding agent. When a task needs a computer, we dispatch Coder to a \
               computer you connect, with this conversation as its task, and it works there \
               with that computer's own git and GitHub login.",
        sources: &["crates/openagents-mobile/src/basic_coder.rs", "README.md"],
    },
    Canned {
        id: "meta.github",
        version: 1,
        route: "meta",
        when: "The user asks us to connect to, sign in to, or link their GitHub account, or \
               asks how we work with GitHub, without asking for a specific change",
        text: "We work with GitHub through a computer you connect: Coder, our coding agent, \
               runs there and uses that computer's own git and GitHub login.",
        sources: &["crates/openagents-mobile/src/basic_coder.rs"],
    },
    Canned {
        id: "meta.open_source",
        version: 1,
        route: "meta",
        when: "The user asks whether we are open source or where our code is",
        text: "The OpenAgents app and the worker that answers this chat are open source, at \
               github.com/OpenAgentsInc/openagents.",
        sources: &["README.md", "LICENSE"],
    },
    Canned {
        id: "smalltalk.hello",
        version: 1,
        route: "smalltalk",
        when: "A greeting with no question or request in it",
        text: "Hi! We're OpenAgents. What are we working on today?",
        sources: &[],
    },
    Canned {
        id: "smalltalk.how_are_you",
        version: 1,
        route: "smalltalk",
        when: "The user asks how we are doing, with no other question or request",
        text: "Doing well, thanks for asking! What are we working on today?",
        sources: &[],
    },
    Canned {
        id: "smalltalk.test",
        version: 1,
        route: "smalltalk",
        when: "The user checks whether the chat works (a test message, or asks if we are \
               there), with no request in it",
        text: "We're here and working. What can we help with?",
        sources: &[],
    },
    Canned {
        id: "smalltalk.thanks",
        version: 1,
        route: "smalltalk",
        when: "Thanks or praise, with no new question or request in it",
        text: "You're welcome! Anything else we can help with?",
        sources: &[],
    },
    Canned {
        id: "smalltalk.bye",
        version: 1,
        route: "smalltalk",
        when: "A goodbye or sign-off, with no new question or request in it",
        text: "Anytime. Talk soon!",
        sources: &[],
    },
];

/// The openers the judgment chooses from: `(id, what the user sees, when it
/// fits)`. Each is a first line that is true for any good reply to such a
/// message and tells the user what kind of answer is coming; a line that
/// says nothing ("Sure.", "On it.") is not here. The model's reply follows
/// it.
pub const OPENERS: &[(&str, &str, &str)] = &[
    (
        "explain",
        "Here's how that works.",
        "A request to explain how a concept, tool, protocol, or piece of code works",
    ),
    (
        "compare",
        "Here's how the options compare.",
        "A question asking us to choose between two or more named tools, libraries, or \
         approaches",
    ),
    (
        "plan",
        "Here's a plan.",
        "A request to plan a multi-step feature, migration, or project",
    ),
    (
        "draft",
        "Here's a draft.",
        "A request to write a message, document, or short piece of text in the chat",
    ),
    (
        "summary",
        "Here's the short version.",
        "A request to summarize or condense text the user gave",
    ),
    (
        "sorry",
        "Sorry about that.",
        "A complaint that our previous answer was wrong or unhelpful",
    ),
];

/// The facts a prepared answer's slots are filled from: the worker's own
/// configuration, never text someone typed once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The chat model, for a person: "Google's Gemini 3.8 Flash".
    pub chat_model: Option<String>,
    /// Where the model is reached: "the Vercel AI Gateway".
    pub chat_model_host: Option<String>,
}

impl Facts {
    /// The facts of a door serving `model` at `url` (`None` for a door
    /// that is not a gateway door). A model or a host this does not know
    /// how to name is left out, and the answers that need it with it.
    #[must_use]
    pub fn of(model: &str, url: Option<&str>) -> Self {
        let chat_model = ModelLane::ALL
            .into_iter()
            .find(|lane| lane.model() == model)
            .map(|lane| match lane {
                ModelLane::Gemini => "Google's Gemini 3.8 Flash".to_string(),
                ModelLane::Glm => "Z.ai's GLM 5.3 Flash".to_string(),
            });
        let chat_model_host = url
            .filter(|url| url.trim_end_matches('/') == DEFAULT_DOOR_URL)
            .map(|_| "the Vercel AI Gateway".to_string());
        Self {
            chat_model,
            chat_model_host,
        }
    }
}

/// Where the judgment says the request belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Answerable in the chat: no files, repository, or commands.
    Chat,
    /// Needs a computer: a repository, files, commands, tests, or changes.
    Computer,
    /// The judgment has no read.
    Unknown,
}

impl Lane {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Lane::Chat => "chat",
            Lane::Computer => "computer",
            Lane::Unknown => "unknown",
        }
    }

    fn parse(choice: &str) -> Self {
        match choice {
            "chat" => Lane::Chat,
            "computer" => Lane::Computer,
            _ => Lane::Unknown,
        }
    }
}

/// The five questions, from one state. Only the prepared answers whose
/// slots `facts` fills are offered.
#[must_use]
pub fn questions(facts: &Facts) -> Questions {
    let action = crate::classify::questions()
        .get("action")
        .cloned()
        .expect("the turn set asks `action`");
    let mut answers: IndexMap<String, Option<Entry>> = ANSWERS
        .iter()
        .filter(|canned| canned.render(facts).is_some())
        .map(|canned| (canned.id.to_string(), Some(Entry::from(canned.when))))
        .collect();
    answers.insert(
        "none".to_string(),
        Some(Entry::from(
            "No prepared answer fully answers the message as asked",
        )),
    );
    let mut openers: IndexMap<String, Option<Entry>> = OPENERS
        .iter()
        .map(|(id, text, fits)| {
            (
                (*id).to_string(),
                Some(Entry::from(format!("\"{text}\" — {fits}"))),
            )
        })
        .collect();
    openers.insert(
        "none".to_string(),
        Some(Entry::from(
            "No listed line is a true and useful first line for this message",
        )),
    );
    Questions::new()
        .with("action", action)
        .with(
            "lane",
            Choice::new(
                "Can the user's latest message be answered in a chat reply, or does it need \
                 work on a computer?",
                IndexMap::from([
                    (
                        "chat".to_string(),
                        Some(Entry::from(
                            "Answer in the chat: a question, explanation, advice, or a short \
                             snippet that needs none of the user's repositories, files, \
                             commands, or accounts",
                        )),
                    ),
                    (
                        "computer".to_string(),
                        Some(Entry::from(
                            "Needs a computer: connecting to or using the user's GitHub or \
                             other accounts, looking at, cloning, or changing a repository or \
                             files, running code, commands, or tests, or opening a pull \
                             request",
                        )),
                    ),
                    (
                        "none".to_string(),
                        Some(Entry::from("Neither fits the message")),
                    ),
                ]),
            ),
        )
        .with(
            "answer",
            Choice::new(
                "We are OpenAgents, an assistant in a chat app. Which prepared answer, if any, \
                 fully and correctly answers the user's latest message as asked, on its own?",
                answers,
            ),
        )
        .with(
            "needs_specifics",
            Noul::with_criteria(
                "Would a good reply to the user's latest message need to refer to specific \
                 things the user named, such as a file, repository, error, product, feature, \
                 or goal of their own, beyond a fixed prepared answer?",
                NoulCriteria::new()
                    .when_true("Yes: the reply has to address the particulars the user gave")
                    .when_false(
                        "No: the message is a general question or small talk that one fixed \
                         answer serves",
                    ),
            ),
        )
        .with(
            "opener",
            Choice::new(
                "Which of these lines, if any, is a true and useful first line for our reply \
                 to the user's latest message?",
                openers,
            ),
        )
}

/// The state the judgment reads: the same bounded shape Classify reads.
#[must_use]
pub fn state(task: &str, transcript: &[Message]) -> Value {
    crate::classify::state_of(task, transcript, &[])
}

/// A retry policy for a call that is only worth anything fast: one
/// attempt, bounded by [`BUDGET`].
#[must_use]
pub fn retry() -> RetryPolicy {
    RetryPolicy {
        max_retries: 0,
        budget: Some(BUDGET),
        ..RetryPolicy::default()
    }
}

/// The request the worker sends.
#[must_use]
pub fn request(task: &str, transcript: &[Message], facts: &Facts) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(task, transcript), questions(facts))
        .retry(retry())
        .timeout(BUDGET)
}

/// What the judgment read.
#[derive(Clone, Debug)]
pub struct Triage {
    pub route: Route,
    pub lane: Lane,
    /// The argmax prepared answer, its filled text, and its probability;
    /// `None` for `none`.
    pub answer: Option<(&'static Canned, String, f64)>,
    /// The probability that a reply needs the user's specifics; 1 when
    /// the judgment did not say, so a missing reading never shows a
    /// prepared answer.
    pub needs_specifics: f64,
    /// The argmax opener's id and display text, or `None` for `none`.
    pub opener: Option<(&'static str, &'static str)>,
    /// The opener choice's confidence.
    pub confidence: f64,
}

/// What code decides to show, from a [`Triage`].
#[derive(Clone, Debug, PartialEq)]
pub enum Tier {
    /// A prepared answer is the whole reply; the model is not needed.
    Canned {
        answer: &'static Canned,
        text: String,
    },
    /// An opener leads the model's reply.
    Opener {
        id: &'static str,
        text: &'static str,
    },
    /// Nothing before the model's own words.
    Model,
}

impl Tier {
    /// The word the wire carries.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Tier::Canned { .. } => "canned",
            Tier::Opener { .. } => "opener",
            Tier::Model => "model",
        }
    }
}

fn choice<'a>(response: &'a SystemOneResponse, id: &str) -> Option<&'a ChoiceAnswer> {
    match response.answers.get(id) {
        Some(Answer::Choice(choice)) => Some(choice),
        _ => None,
    }
}

/// Reads a response into a [`Triage`]: each answer's argmax or
/// probability, nothing more.
#[must_use]
pub fn triage_of(response: &SystemOneResponse, facts: &Facts) -> Triage {
    let judgment = crate::classify::Judgment {
        action: choice(response, "action").cloned(),
    };
    let lane = choice(response, "lane").map_or(Lane::Unknown, |lane| Lane::parse(&lane.choice));
    let answer = choice(response, "answer").and_then(|answer| {
        let canned = ANSWERS.iter().find(|canned| canned.id == answer.choice)?;
        let text = canned.render(facts)?;
        Some((canned, text, answer.confidence))
    });
    let needs_specifics = match response.answers.get("needs_specifics") {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul,
        _ => 1.0,
    };
    let opener = choice(response, "opener");
    Triage {
        route: route(&judgment),
        lane,
        answer,
        needs_specifics,
        opener: opener.and_then(|opener| {
            OPENERS
                .iter()
                .find(|(id, _, _)| *id == opener.choice)
                .map(|(id, text, _)| (*id, *text))
        }),
        confidence: opener.map_or(0.0, |opener| opener.confidence),
    }
}

impl Triage {
    /// The NIP-CJ verdict word.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match self.route {
            Route::Respond => "respond",
            Route::Clarify => "clarify",
            Route::End => "end_conversation",
            Route::Halt(_) => "unrouted",
        }
    }

    /// What to show: a sure prepared answer that needs no specifics, else
    /// a sure opener, else nothing.
    #[must_use]
    pub fn tier(&self) -> Tier {
        if let Some((canned, text, p)) = &self.answer
            && *p >= ANSWER_CONFIDENCE
            && self.needs_specifics < SPECIFICS_CEILING
        {
            return Tier::Canned {
                answer: canned,
                text: text.clone(),
            };
        }
        match self.opener {
            Some((id, text)) if self.confidence >= OPENER_CONFIDENCE => Tier::Opener { id, text },
            _ => Tier::Model,
        }
    }

    /// The display line: what is shown first, else the verdict.
    #[must_use]
    pub fn line(&self) -> String {
        match self.tier() {
            Tier::Canned { text, .. } => text,
            Tier::Opener { text, .. } => text.to_string(),
            Tier::Model => self.verdict().to_string(),
        }
    }
}

/// The `27000` judgment feedback for `triage`, at payload `version`.
///
/// `verdict` and `line` are NIP-CJ's; `set`, `lane`, `opener`,
/// `confidence`, `bank`, `answer`, `answer_p`, `needs_specifics`, and
/// `tier` are this set's typed additions, each optional to a reader.
/// `opener` names the opener shown, so it is null unless the tier is
/// `opener`.
#[must_use]
pub fn feedback(version: u64, triage: &Triage) -> Value {
    let tier = triage.tier();
    json!({
        "v": version,
        "requires": [],
        "type": "judgment",
        "verdict": triage.verdict(),
        "line": triage.line(),
        "set": SET,
        "lane": triage.lane.word(),
        "opener": match tier { Tier::Opener { id, .. } => Some(id), _ => None },
        "confidence": triage.confidence,
        "bank": BANK,
        "answer": triage.answer.as_ref().map(|(canned, _, _)| canned.tag()),
        "answer_p": triage.answer.as_ref().map_or(0.0, |(_, _, p)| *p),
        "needs_specifics": triage.needs_specifics,
        "tier": tier.word(),
    })
}

/// The most candidates one ranking takes.
pub const MAX_CANDIDATES: usize = 16;
/// The longest candidate id, in bytes.
pub const MAX_ID_BYTES: usize = 64;
/// The longest candidate label, in bytes.
pub const MAX_LABEL_BYTES: usize = 200;

/// One suggestion a caller wants ranked: a repository or an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub label: String,
}

/// Reads `candidates` from a request payload: an array of
/// `{ "id", "label" }`, bounded.
///
/// # Errors
///
/// Names what is wrong: not an array, too many, an empty, long, repeated,
/// or reserved id, or a long label.
pub fn candidates_of(value: &Value) -> Result<Vec<Candidate>, String> {
    let list = value
        .as_array()
        .ok_or("candidates is not an array of { id, label }")?;
    if list.is_empty() || list.len() > MAX_CANDIDATES {
        return Err(format!("candidates must name 1 to {MAX_CANDIDATES}"));
    }
    let mut out: Vec<Candidate> = Vec::with_capacity(list.len());
    for item in list {
        let id = item["id"].as_str().unwrap_or_default();
        let label = item["label"].as_str().unwrap_or(id);
        if id.is_empty() || id.len() > MAX_ID_BYTES || id == "none" {
            return Err(format!(
                "a candidate id must be 1 to {MAX_ID_BYTES} bytes and not `none`"
            ));
        }
        if label.len() > MAX_LABEL_BYTES {
            return Err(format!(
                "a candidate label must be at most {MAX_LABEL_BYTES} bytes"
            ));
        }
        if out.iter().any(|seen| seen.id == id) {
            return Err(format!("the candidate id {id} is repeated"));
        }
        out.push(Candidate {
            id: id.to_string(),
            label: label.to_string(),
        });
    }
    Ok(out)
}

/// The ranking question: which candidate the user most likely wants next.
#[must_use]
pub fn rank_questions(candidates: &[Candidate]) -> Questions {
    let mut options: IndexMap<String, Option<Entry>> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.id.clone(),
                Some(Entry::from(candidate.label.clone())),
            )
        })
        .collect();
    options.insert(
        "none".to_string(),
        Some(Entry::from("None of these fits what the user is doing")),
    );
    Questions::new().with(
        "next",
        Choice::new(
            "Given the conversation so far, which of these repositories or actions is the user \
             most likely to want next?",
            options,
        ),
    )
}

/// The ranking request over a draft (possibly empty) and the transcript.
#[must_use]
pub fn rank_request(
    draft: &str,
    transcript: &[Message],
    candidates: &[Candidate],
) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(draft, transcript), rank_questions(candidates))
        .retry(retry())
        .timeout(BUDGET)
}

/// The candidates in the answer's order, most likely first, each with its
/// probability. A candidate the answer left out ranks last at zero, in the
/// caller's order; `none` is not a candidate and is dropped.
#[must_use]
pub fn ranking(response: &SystemOneResponse, candidates: &[Candidate]) -> Vec<(String, f64)> {
    let probabilities = choice(response, "next").map(|next| &next.probabilities);
    let mut ranked: Vec<(usize, String, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let p = probabilities
                .and_then(|p| p.get(&candidate.id))
                .copied()
                .filter(|p| p.is_finite())
                .unwrap_or(0.0);
            (index, candidate.id.clone(), p)
        })
        .collect();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(_, id, p)| (id, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(answers: Value) -> SystemOneResponse {
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: json!({ "model": "jev", "answers": answers })
                .to_string()
                .into_bytes(),
        })
        .expect("a readable response")
    }

    /// A choice answer that puts `p` on `choice` and the rest on the
    /// first other option.
    fn picked_at(choice: &str, p: f64, options: &[&str]) -> Value {
        let other = options.iter().find(|option| **option != choice).copied();
        let probabilities: serde_json::Map<String, Value> = options
            .iter()
            .map(|option| {
                let q = if *option == choice {
                    p
                } else if Some(*option) == other {
                    1.0 - p
                } else {
                    0.0
                };
                ((*option).to_string(), json!(q))
            })
            .collect();
        json!({ "type": "choice", "choice": choice, "confidence": p, "probabilities": probabilities })
    }

    fn picked(choice: &str, options: &[&str]) -> Value {
        picked_at(choice, 1.0, options)
    }

    fn gateway() -> Facts {
        Facts::of("google/gemini-3.8-flash", Some(DEFAULT_DOOR_URL))
    }

    fn answer_ids() -> Vec<&'static str> {
        ANSWERS
            .iter()
            .map(|canned| canned.id)
            .chain(["none"])
            .collect()
    }

    fn opener_ids() -> Vec<&'static str> {
        OPENERS
            .iter()
            .map(|(id, _, _)| *id)
            .chain(["none"])
            .collect()
    }

    /// A judgment: respond in the chat, `answer` at `answer_p`, the
    /// specifics probability, and `opener` at `opener_p`.
    fn judged(answer: &str, answer_p: f64, specifics: f64, opener: &str, opener_p: f64) -> Triage {
        triage_of(
            &response(json!({
                "action": picked("respond", &["respond", "clarify", "end_conversation", "none"]),
                "lane": picked("chat", &["chat", "computer", "none"]),
                "answer": picked_at(answer, answer_p, &answer_ids()),
                "needs_specifics": { "type": "noul", "noul": specifics },
                "opener": picked_at(opener, opener_p, &opener_ids()),
            })),
            &gateway(),
        )
    }

    #[test]
    fn the_set_asks_five_independent_questions_and_validates() {
        let questions = questions(&gateway());
        questions.validate().expect("a valid set");
        let asked: Vec<&str> = questions.iter().map(|(id, _)| id).collect();
        assert_eq!(
            asked,
            ["action", "lane", "answer", "needs_specifics", "opener"]
        );
        // The action wording is Classify's, so its answer means the same.
        assert_eq!(
            serde_json::to_value(questions.get("action")).unwrap(),
            serde_json::to_value(crate::classify::questions().get("action")).unwrap()
        );
        let answer = serde_json::to_value(questions.get("answer")).unwrap();
        assert_eq!(
            answer["criteria"].as_object().unwrap().len(),
            ANSWERS.len() + 1
        );
        let opener = serde_json::to_value(questions.get("opener")).unwrap();
        assert_eq!(
            opener["criteria"].as_object().unwrap().len(),
            OPENERS.len() + 1
        );
    }

    /// "Who are you?" is answered whole, from the bank, in the plural.
    #[test]
    fn a_sure_identity_judgment_is_the_whole_reply() {
        let triage = judged("meta.who", 0.93, 0.05, "explain", 0.4);
        let Tier::Canned { answer, text } = triage.tier() else {
            panic!("expected a prepared answer, got {:?}", triage.tier());
        };
        assert_eq!(answer.id, "meta.who");
        assert!(text.starts_with("We are OpenAgents."), "{text}");
        assert_eq!(triage.line(), text);
        let body = feedback(2, &triage);
        assert_eq!(body["tier"], "canned");
        assert_eq!(body["answer"], "meta.who@1");
        assert_eq!(body["bank"], BANK);
        assert_eq!(body["set"], SET);
        assert!(body["opener"].is_null());
    }

    /// The model answer's slots come from the door, and an entry whose
    /// slot the worker cannot fill is never offered or shown.
    #[test]
    fn the_model_answer_names_the_configured_model_or_is_not_offered() {
        let text = ANSWERS
            .iter()
            .find(|canned| canned.id == "meta.model")
            .unwrap()
            .render(&gateway())
            .unwrap();
        assert!(
            text.starts_with(
                "Our chat runs on Google's Gemini 3.8 Flash through the Vercel AI Gateway."
            ),
            "{text}"
        );
        let elsewhere = Facts::of("google/gemini-3.8-flash", Some("http://127.0.0.1:9"));
        assert_eq!(elsewhere.chat_model_host, None);
        let offered = serde_json::to_value(questions(&elsewhere).get("answer")).unwrap();
        assert!(offered["criteria"].get("meta.model").is_none());
        assert!(offered["criteria"].get("meta.who").is_some());
        assert_eq!(Facts::of("some/other-model", None), Facts::default());
    }

    /// Below the answer threshold, or with specifics the user named, the
    /// bank stays quiet; below the opener threshold nothing is shown.
    #[test]
    fn low_confidence_shows_nothing_before_the_model() {
        let unsure = judged("meta.capabilities", 0.6, 0.1, "explain", 0.5);
        assert_eq!(unsure.tier(), Tier::Model);
        assert_eq!(unsure.line(), "respond");
        assert!(feedback(2, &unsure)["opener"].is_null());
        assert_eq!(feedback(2, &unsure)["tier"], "model");

        let specific = judged("meta.capabilities", 0.95, 0.7, "explain", 0.2);
        assert_eq!(specific.tier(), Tier::Model);

        let opener = judged("none", 0.9, 0.8, "explain", 0.85);
        assert_eq!(
            opener.tier(),
            Tier::Opener {
                id: "explain",
                text: "Here's how that works."
            }
        );
        assert_eq!(feedback(2, &opener)["opener"], "explain");

        // A judgment without the specifics reading never shows the bank.
        let missing = triage_of(
            &response(json!({
                "answer": picked("smalltalk.hello", &answer_ids()),
                "opener": picked("none", &opener_ids()),
            })),
            &gateway(),
        );
        assert_eq!(missing.needs_specifics, 1.0);
        assert_eq!(missing.tier(), Tier::Model);
    }

    fn singular_words(text: &str) -> Vec<String> {
        text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
            .filter(|word| {
                let lower = word.to_lowercase().replace('’', "'");
                matches!(
                    lower.as_str(),
                    "i" | "i'll" | "i'm" | "i've" | "i'd" | "me" | "my" | "mine" | "myself"
                ) || (*word == "I")
            })
            .map(str::to_string)
            .collect()
    }

    /// Every line the user can see speaks as OpenAgents, in the plural,
    /// and says something: no bare acknowledgements.
    #[test]
    fn every_canned_line_is_plural_and_says_something() {
        assert!(singular_words("I'll look into that. Let me check.").len() == 2);
        for canned in ANSWERS {
            let text = canned.render(&gateway()).unwrap();
            assert!(singular_words(&text).is_empty(), "{}: {text}", canned.id);
            assert!(text.len() <= 600, "{} is too long", canned.id);
            // The app shows the right action itself, and may not show a
            // button a line names, so no line names one.
            for control in ["Run Coder", "tap ", "button", "Tap "] {
                assert!(!text.contains(control), "{} names `{control}`", canned.id);
            }
            assert!(!canned.when.is_empty());
        }
        for (id, text, _) in OPENERS {
            assert!(singular_words(text).is_empty(), "{id}: {text}");
            assert!(
                text.split_whitespace().count() >= 3,
                "{id}: `{text}` is filler"
            );
        }
        for filler in [
            "Sure.",
            "On it.",
            "Hi!",
            "Good question.",
            "Let me think about that.",
        ] {
            assert!(OPENERS.iter().all(|(_, text, _)| *text != filler));
        }
        let mut ids: Vec<&str> = ANSWERS.iter().map(|canned| canned.id).collect();
        ids.extend(OPENERS.iter().map(|(id, _, _)| *id));
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "ids are unique");
    }

    /// Every source an answer cites exists in the repository.
    #[test]
    fn every_answer_cites_sources_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for canned in ANSWERS {
            if canned.route == "meta" {
                assert!(!canned.sources.is_empty(), "{} cites nothing", canned.id);
            }
            for source in canned.sources {
                assert!(root.join(source).exists(), "{}: {source}", canned.id);
            }
        }
    }

    #[test]
    fn candidates_are_bounded_and_ranked_by_probability() {
        assert!(candidates_of(&json!("repo")).is_err());
        assert!(candidates_of(&json!([])).is_err());
        assert!(candidates_of(&json!([{ "id": "none" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "a" }, { "id": "a" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "x".repeat(65) }])).is_err());
        let many: Vec<Value> = (0..17).map(|i| json!({ "id": format!("r{i}") })).collect();
        assert!(candidates_of(&Value::Array(many)).is_err());

        let candidates = candidates_of(&json!([
            { "id": "openagents", "label": "OpenAgentsInc/openagents" },
            { "id": "psionic", "label": "OpenAgentsInc/psionic" },
            { "id": "run_tests", "label": "Run the tests" },
        ]))
        .unwrap();
        rank_questions(&candidates).validate().expect("valid");
        let ranked = ranking(
            &response(json!({ "next": {
                "type": "choice", "choice": "psionic", "confidence": 0.6,
                "probabilities": { "openagents": 0.3, "psionic": 0.6, "run_tests": 0.0, "none": 0.1 }
            }})),
            &candidates,
        );
        let order: Vec<&str> = ranked.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(order, ["psionic", "openagents", "run_tests"]);
    }

    /// The first-response set against the live judge, over realistic
    /// first messages: prints what each would show. Needs
    /// `TYPESAFE_API_KEY` (or another decision profile).
    #[tokio::test]
    #[ignore = "calls the live judge"]
    async fn live_first_response_eval() {
        let judge = crate::decision::from_env()
            .expect("a decision profile")
            .expect("TYPESAFE_API_KEY or another profile");
        let messages = [
            "Who are you?",
            "what are you",
            "What model are you?",
            "Are you ChatGPT?",
            "which LLM is this",
            "What can you do?",
            "Can you code?",
            "can you see my files?",
            "What is Coder?",
            "Are you open source?",
            "hi",
            "Hello!",
            "hey how are you",
            "test",
            "thanks!",
            "Thank you, that helped",
            "bye",
            "Fix my repo",
            "Fix the failing test in crates/coder and open a PR",
            "Explain how Nostr relays work",
            "What's a closure in Rust?",
            "Should I use Postgres or SQLite for a small app?",
            "Write a commit message for a change that adds retries to the relay client",
            "Plan a migration from REST to gRPC for our API",
            "How much does this cost?",
            "Is this free?",
            "Do you store my chats?",
            "That answer was wrong",
            "Why does my build fail on CI but not locally?",
            "Can you work on my Rails app?",
            "summarize this: Rust ownership means each value has one owner, and when the owner goes out of scope the value is dropped.",
            "who made you",
            "Connect to my GitHub",
            "Look at my repo",
            "Open a PR for this",
        ];
        let facts = gateway();
        println!(
            "| Message | Lane | Tier | Shown first | answer (p) | specifics | opener (p) | ms |"
        );
        println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
        for message in messages {
            let transcript = [Message {
                role: crate::generate::Role::User,
                text: message.to_string(),
            }];
            let started = std::time::Instant::now();
            let response = judge
                .system_one(request(message, &transcript, &facts))
                .await
                .expect("the judge answers");
            let ms = started.elapsed().as_millis();
            let triage = triage_of(&response, &facts);
            let tier = triage.tier();
            let shown = match &tier {
                Tier::Canned { answer, .. } => format!("{} (whole reply)", answer.id),
                Tier::Opener { text, .. } => format!("\"{text}\""),
                Tier::Model => "nothing".to_string(),
            };
            let answer = choice(&response, "answer")
                .map(|a| format!("{} ({:.2})", a.choice, a.confidence))
                .unwrap_or_default();
            let opener = choice(&response, "opener")
                .map(|a| format!("{} ({:.2})", a.choice, a.confidence))
                .unwrap_or_default();
            println!(
                "| {message} | {} | {} | {shown} | {answer} | {:.2} | {opener} | {ms} |",
                triage.lane.word(),
                tier.word(),
                triage.needs_specifics
            );
        }
    }
}
