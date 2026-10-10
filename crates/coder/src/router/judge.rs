//! The `chat-router-v4` question set and what its answer reads as.
//!
//! One System One request, independent questions over the same state
//! (the bounded transcript and latest message, as `coder::first` builds
//! it):
//!
//! | Id | Type | Reads |
//! | --- | --- | --- |
//! | `action` | Choice | Classify's measured `coder-turns-v2` wording, unchanged |
//! | `route` | Choice | the [`RouteId`] catalog (21 routes in `chat-router-v5`), each with its rubric, plus `none` |
//! | `answer` | Choice | every selectable bank entry with its `when`, plus `none` |
//! | `needs_specifics` | Noul | whether a good reply must refer to the user's particulars |
//! | `lane` | Choice | `coder::first`'s wording: chat, computer, or none |
//! | `opener` | Choice | the bank's openers, plus `none` |
//! | `cli_group` | Choice | the command groups a [`CliRoute`](super::seams::CliRoute) lists, plus `none`; asked only when it lists any |
//! | `tool` | Choice | the tool catalog a [`GymKb`](super::seams::GymKb) lists, plus `none`; asked only when it lists any |
//! | `capability` | Choice | the admitted-capability set ([`Admitted`]), plus `none` (a request none covers) and `not-a-capability-request`; asked on every turn |
//! | `deck` | Choice | the decks the desktop app ships (`openagents_deck::decks()`), by title, plus `none`; asked only on a desktop turn |
//! | `engine` | Choice | the coding engines a dispatch may name ([`Engine::ALL`]), plus `none`; asked on every turn, read only for a dispatch (#10076) |
//! | `fanout` | Choice | one run, one run on each ready engine, or one on each of two named engines ([`Fanout`]); asked on every turn, read only for a dispatch (#10183) |
//! | `read_only` | Noul | whether the work asked for only reads: explore, review, summarize, answer, change nothing (#10183) |
//! | `summarize` | Noul | whether the person asks for one summary or comparison of what the work finds (#10183) |
//! | `risk` | Choice | ok, secret shared, asks for a secret, harmful, money movement (no `none`: `ok` is the "nothing wrong" option, #11193) |
//!
//! No question consumes another's answer, so they cost one round trip.
//!
//! **The split (#11193).** The set is asked as more than one request over
//! the same state, all sent at once ([`split`], [`ask`]): the main request
//! holds every question but [`SIDE`], and `answer` (every selectable bank
//! entry) and `cli_group` (the command tree) each go in a request of their
//! own. Each request then stays within the 16,384 tokens a Clef decision
//! head was trained at, where the single request ran about 24.5k, and the
//! sides run beside the main request rather than after it, so the split
//! costs no extra round trip against [`crate::first::LATE`]. [`merge`]
//! joins the answers back into one response, so [`reading`] and the policy
//! read it exactly as they read the single request. A side that fails
//! reads as not asked: no prepared answer, no command proposal.

use indexmap::IndexMap;
use jev::{Answer, Choice, ChoiceAnswer, Entry as Criterion, Noul, NoulCriteria, Questions};
use serde_json::Value;

use super::bank::{Bank, Entry, Facts, Opener};
use super::capability::{Admitted, Capability, NONE, NOT_A_REQUEST};
use super::gym::Tool;
use super::seams::CliGroup;
use super::{CodingEngine as Engine, Risk, RouteId};
use crate::classify::Route;
use crate::first::Lane;
use crate::generate::Message;
use openagents_deck::DeckEntry;

/// The `route` question: the [`RouteId`] catalog, each option with its
/// rubric, plus `none`. It reads no bank or facts, so the Gym suite
/// `chat-router-v2` asks exactly this question
/// ([`crate::router_eval::route_question`]).
#[must_use]
pub fn route() -> Choice {
    let mut routes: IndexMap<String, Option<Criterion>> = RouteId::ALL
        .into_iter()
        .map(|route| {
            (
                route.word().to_string(),
                Some(Criterion::from(super::rubric::route(route))),
            )
        })
        .collect();
    routes.insert(
        "none".to_string(),
        Some(Criterion::from(RouteId::Unknown.description().to_string())),
    );
    Choice::new(super::rubric::route_instructions(), routes)
}

/// The `capability` question: every admitted entry with its rubric, then
/// `none` and `not-a-capability-request`.
#[must_use]
pub fn capability(admitted: &Admitted) -> Choice {
    let mut options: IndexMap<String, Option<Criterion>> = admitted
        .entries
        .iter()
        .map(|entry| {
            (
                entry.id.clone(),
                Some(Criterion::from(super::rubric::capability(entry))),
            )
        })
        .collect();
    options.insert(
        NONE.to_string(),
        Some(Criterion::from(super::rubric::capability_none())),
    );
    options.insert(
        NOT_A_REQUEST.to_string(),
        Some(Criterion::from(super::rubric::capability_not_a_request())),
    );
    Choice::new(super::rubric::capability_instructions(), options)
}

/// The `deck` question: each deck the desktop app ships, by its title,
/// then `none`. The option ids are the deck ids, so the reading is one of
/// them or `none`, never text.
#[must_use]
pub fn deck(decks: &[DeckEntry]) -> Choice {
    let mut options: IndexMap<String, Option<Criterion>> = decks
        .iter()
        .map(|deck| {
            (
                deck.id.to_string(),
                Some(Criterion::from(super::rubric::deck(deck))),
            )
        })
        .collect();
    options.insert(
        "none".to_string(),
        Some(Criterion::from(super::rubric::deck_none())),
    );
    Choice::new(super::rubric::deck_instructions(), options)
}

/// The `engine` question: each coding engine a dispatch offer may name,
/// by its wire word, then `none`. The reading is one of them or nothing,
/// never text (#10076).
#[must_use]
pub fn engine() -> Choice {
    let mut options: IndexMap<String, Option<Criterion>> = Engine::ALL
        .into_iter()
        .map(|engine| {
            (
                engine.word().to_string(),
                Some(Criterion::from(super::rubric::engine(engine))),
            )
        })
        .collect();
    options.insert(
        "none".to_string(),
        Some(Criterion::from(super::rubric::engine_none())),
    );
    Choice::new(super::rubric::engine_instructions(), options)
}

/// How many Coder runs a message asks for, read from the `fanout`
/// question (#10183): the plan's shape, never its engines' readiness,
/// which the computer's context says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fanout {
    /// One run on each coding engine ready on the computer ("one per
    /// agent", "ask all three agents").
    EachEngine,
    /// One run on each of two engines the message names ("have Codex and
    /// Claude both look").
    Pair(Engine, Engine),
}

impl Fanout {
    /// Every option but `one`, in the order the question lists them.
    pub const ALL: [Fanout; 4] = [
        Fanout::EachEngine,
        Fanout::Pair(Engine::Codex, Engine::ClaudeCode),
        Fanout::Pair(Engine::Codex, Engine::GrokBuild),
        Fanout::Pair(Engine::ClaudeCode, Engine::GrokBuild),
    ];

    /// The option's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Fanout::EachEngine => "each_engine",
            Fanout::Pair(Engine::Codex, Engine::ClaudeCode) => "codex_and_claude_code",
            Fanout::Pair(Engine::Codex, Engine::GrokBuild) => "codex_and_grok_build",
            Fanout::Pair(Engine::ClaudeCode, Engine::GrokBuild) => "claude_code_and_grok_build",
            Fanout::Pair(..) => "pair",
        }
    }

    /// The option an exact word names; `one` and anything else is none.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Fanout::ALL.into_iter().find(|fanout| fanout.word() == word)
    }
}

/// The `fanout` question (#10183): `one`, then each [`Fanout`] option.
#[must_use]
pub fn fanout() -> Choice {
    let mut options: IndexMap<String, Option<Criterion>> = IndexMap::new();
    options.insert(
        "one".to_string(),
        Some(Criterion::from(super::rubric::fanout_one())),
    );
    for fanout in Fanout::ALL {
        options.insert(
            fanout.word().to_string(),
            Some(Criterion::from(super::rubric::fanout(fanout))),
        );
    }
    Choice::new(super::rubric::fanout_instructions(), options)
}

/// The questions, from one state. Only entries selectable under `facts`
/// are offered, `cli_group` only when `groups` is not empty, `tool` only
/// when `tools` is not empty, `capability` only when `admitted` has an
/// entry (it always has the built-ins), and `deck` only when `decks` is
/// not empty (a desktop turn).
#[must_use]
pub fn questions(
    bank: &Bank,
    facts: &Facts,
    groups: &[CliGroup],
    tools: &[Tool],
    admitted: &Admitted,
    decks: &[DeckEntry],
) -> Questions {
    let action = crate::classify::questions()
        .get("action")
        .cloned()
        .expect("the turn set asks `action`");
    let with_none = |mut options: IndexMap<String, Option<Criterion>>, none: &str| {
        options.insert("none".to_string(), Some(Criterion::from(none.to_string())));
        options
    };
    let answers = with_none(
        bank.answers
            .iter()
            .filter(|entry| entry.selectable(facts))
            .map(|entry| (entry.id.clone(), Some(Criterion::from(entry.criterion()))))
            .collect(),
        "No prepared answer fully answers the message as asked",
    );
    let openers = with_none(
        bank.openers
            .iter()
            .map(|opener| {
                (
                    opener.id.clone(),
                    Some(Criterion::from(format!(
                        "\"{}\" — {}",
                        opener.text, opener.when
                    ))),
                )
            })
            .collect(),
        "No listed line is a true and useful first line for this message",
    );
    let risks: IndexMap<String, Option<Criterion>> = Risk::ALL
        .into_iter()
        .map(|risk| {
            (
                risk.word().to_string(),
                Some(Criterion::from(super::rubric::risk(risk))),
            )
        })
        .collect();
    // No `none` here (#11193): `ok` already says nothing is wrong, and a
    // `none` option let a judge that read it as "the question does not
    // apply" turn the hazard gate off without saying so.
    let mut questions = Questions::new()
        .with("action", action)
        .with("route", route())
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
                super::rubric::specifics_instructions(),
                NoulCriteria::new()
                    .when_true(super::rubric::specifics(true))
                    .when_false(super::rubric::specifics(false)),
            ),
        )
        .with(
            "lane",
            Choice::new(
                super::rubric::lane_instructions(),
                ["chat", "computer", "none"]
                    .into_iter()
                    .map(|word| {
                        (
                            word.to_string(),
                            Some(Criterion::from(super::rubric::lane(word))),
                        )
                    })
                    .collect(),
            ),
        )
        .with(
            "opener",
            Choice::new(
                "Which of these lines, if any, is a true and useful first line for our reply \
                 to the user's latest message?",
                openers,
            ),
        );
    if !groups.is_empty() {
        let options = with_none(
            groups
                .iter()
                .map(|group| {
                    (
                        group.id.clone(),
                        Some(Criterion::from(
                            group
                                .tree
                                .clone()
                                .unwrap_or_else(|| group.summary.clone().into()),
                        )),
                    )
                })
                .collect(),
            "The user does not want anything an openagents command does",
        );
        questions = questions.with(
            "cli_group",
            Choice::new(
                "If the user wants something done with the `openagents` command, which command \
                 group does it?",
                options,
            ),
        );
    }
    if !tools.is_empty() {
        let options = with_none(
            tools
                .iter()
                .map(|tool| {
                    (
                        tool.id.clone(),
                        Some(Criterion::from(super::rubric::tool(tool))),
                    )
                })
                .collect(),
            "The message names or means none of these tools",
        );
        questions = questions.with(
            "tool",
            Choice::new(super::rubric::tool_instructions(), options),
        );
    }
    if !admitted.is_empty() {
        questions = questions.with("capability", capability(admitted));
    }
    if !decks.is_empty() {
        questions = questions.with("deck", deck(decks));
    }
    // Asked on every turn, beside the route, and read only when the
    // policy serves a dispatch: the questions are independent, so it
    // costs no round trip.
    questions = questions.with("engine", engine());
    // The dispatch plan's readings (#10183), asked beside the route like
    // `engine` and read only when the policy serves a dispatch.
    questions = questions
        .with("fanout", fanout())
        .with(
            "read_only",
            Noul::with_criteria(
                super::rubric::read_only_instructions(),
                NoulCriteria::new()
                    .when_true(super::rubric::read_only(true))
                    .when_false(super::rubric::read_only(false)),
            ),
        )
        .with(
            "summarize",
            Noul::with_criteria(
                super::rubric::summarize_instructions(),
                NoulCriteria::new()
                    .when_true(super::rubric::summarize(true))
                    .when_false(super::rubric::summarize(false)),
            ),
        );
    questions.with(
        "risk",
        Choice::new(super::rubric::risk_instructions(), risks),
    )
}

/// The least probability at which a command group other than the argmax
/// is descended too.
pub const CLI_BEAM_FLOOR: f64 = 0.15;

/// The most command groups beside the argmax the CLI route descends.
pub const CLI_BEAM: usize = 2;

/// The state the judgment reads: the same bounded shape Classify reads.
#[must_use]
pub fn state(task: &str, transcript: &[Message]) -> Value {
    crate::first::state(task, transcript)
}

/// The questions asked apart from the main request, each in a request of
/// its own (#11193): the two long option lists, the bank's selectable
/// answers and the command groups.
pub const SIDE: [&str; 2] = ["answer", "cli_group"];

/// The tokens a Clef decision head was trained at: no request the router
/// sends should need more (#11193).
pub const CLEF_TOKENS: usize = 16_384;

/// The router's requests for one turn: the main one and the side ones,
/// all over the same state, sent at once by [`ask`].
#[derive(Clone, Debug)]
pub struct Split {
    /// Every question but [`SIDE`].
    pub main: jev::SystemOneRequest,
    /// One request per [`SIDE`] question the set asks, by its id.
    pub sides: Vec<(&'static str, jev::SystemOneRequest)>,
}

impl Split {
    /// Every request, main first.
    pub fn requests(&self) -> impl Iterator<Item = &jev::SystemOneRequest> {
        std::iter::once(&self.main).chain(self.sides.iter().map(|(_, request)| request))
    }
}

/// The requests the worker sends, each bounded by `coder::first::LATE`.
/// Each list is one question's options, so they stay separate arguments.
/// Together they ask exactly [`questions`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn split(
    task: &str,
    transcript: &[Message],
    bank: &Bank,
    facts: &Facts,
    groups: &[CliGroup],
    tools: &[Tool],
    admitted: &Admitted,
    decks: &[DeckEntry],
) -> Split {
    let asked = questions(bank, facts, groups, tools, admitted, decks);
    let state = state(task, transcript);
    let request = |questions: Questions| {
        jev::SystemOneRequest::new(state.clone(), questions)
            .retry(crate::first::retry())
            .timeout(crate::first::LATE)
    };
    let main: Questions = asked
        .iter()
        .filter(|(id, _)| !SIDE.contains(id))
        .map(|(id, question)| (id.to_string(), question.clone()))
        .collect();
    let sides = SIDE
        .into_iter()
        .filter_map(|id| {
            let question = asked.get(id)?.clone();
            Some((id, request(Questions::new().with(id, question))))
        })
        .collect();
    Split {
        main: request(main),
        sides,
    }
}

/// Asks every request of a [`Split`] at once and [`merge`]s the answers.
///
/// The main request's failure is the call's. A side that fails is left
/// out, so its question reads as not asked (no prepared answer, no command
/// proposal), and its id and error go to the log: an id and a door's
/// error, never message text.
///
/// # Errors
///
/// Returns the main request's error.
pub async fn ask(judge: &jev::Client, split: Split) -> jev::Result<jev::SystemOneResponse> {
    let Split { main, sides } = split;
    let (ids, requests): (Vec<&'static str>, Vec<jev::SystemOneRequest>) =
        sides.into_iter().unzip();
    let (main, sides) = futures_util::future::join(
        judge.system_one(main),
        futures_util::future::join_all(requests.into_iter().map(|side| judge.system_one(side))),
    )
    .await;
    let main = main?;
    let answered = ids
        .into_iter()
        .zip(sides)
        .filter_map(|(id, side)| match side {
            Ok(response) => Some(response),
            Err(error) => {
                eprintln!("router: the {id} request failed: {error}");
                None
            }
        })
        .collect();
    Ok(merge(main, answered))
}

/// One response holding the main response's answers and then each side's:
/// the main response's model, `service`, and other fields, with the
/// sides' input tokens and costs added to its usage. [`reading`] and
/// [`super::decisions::capture`] read it as they read a single request's.
#[must_use]
pub fn merge(
    main: jev::SystemOneResponse,
    sides: Vec<jev::SystemOneResponse>,
) -> jev::SystemOneResponse {
    if sides.is_empty() {
        return main;
    }
    let body = |response: &jev::SystemOneResponse| {
        serde_json::from_slice::<serde_json::Map<String, Value>>(&response.raw().bytes).ok()
    };
    let Some(mut merged) = body(&main) else {
        let mut main = main;
        for side in sides {
            main.answers.extend(side.answers);
        }
        return main;
    };
    let sum = |left: Option<&Value>, right: Option<&Value>| match (
        left.and_then(Value::as_f64),
        right.and_then(Value::as_f64),
    ) {
        (Some(left), Some(right)) => {
            let total = left + right;
            Some(
                if total.fract() == 0.0 && total >= 0.0 && total < u64::MAX as f64 {
                    Value::from(total as u64)
                } else {
                    Value::from(total)
                },
            )
        }
        (Some(only), None) | (None, Some(only)) => Some(Value::from(only)),
        (None, None) => None,
    };
    for side in &sides {
        let Some(side) = body(side) else { continue };
        if let (Some(Value::Object(answers)), Some(Value::Object(more))) =
            (merged.get_mut("answers"), side.get("answers"))
        {
            for (id, answer) in more {
                answers.insert(id.clone(), answer.clone());
            }
        }
        if let Some(Value::Object(more)) = side.get("usage") {
            let usage = merged
                .entry("usage")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Value::Object(usage) = usage {
                for key in ["input_tokens", "output_tokens", "cost"] {
                    if let Some(total) = sum(usage.get(key), more.get(key)) {
                        usage.insert(key.to_string(), total);
                    }
                }
            }
        }
    }
    let raw = jev::RawResponse {
        status: main.raw().status,
        headers: main.raw().headers.clone(),
        bytes: serde_json::to_vec(&Value::Object(merged)).unwrap_or_default(),
    };
    match jev::SystemOneResponse::decode(raw) {
        Ok(response) => response,
        Err(_) => {
            let mut main = main;
            for side in sides {
                main.answers.extend(side.answers);
            }
            main
        }
    }
}

/// The router's reading of one turn: each answer's argmax and probability,
/// nothing more. Pure data; [`super::policy::decide`] turns it into a tier.
#[derive(Clone, Debug, PartialEq)]
pub struct Routing {
    /// Classify's route from `action`.
    pub action: Route,
    /// The argmax route, or `Unknown`.
    pub route: RouteId,
    pub route_p: f64,
    /// The second most likely route, for the close-call rule.
    pub runner_up: Option<(RouteId, f64)>,
    /// The `clarify` route's probability, whatever won.
    pub clarify_p: f64,
    /// The argmax entry when it is eligible, with its probability.
    pub answer: Option<(Entry, f64)>,
    /// The probability that a reply needs the user's specifics; 1 when
    /// the judgment did not say, so a missing reading never shows the bank.
    pub needs_specifics: f64,
    pub lane: Lane,
    pub lane_p: f64,
    /// The argmax opener, or `None` for `none`.
    pub opener: Option<(Opener, f64)>,
    /// The argmax command group, or `None` for `none` or not asked.
    pub cli_group: Option<(String, f64)>,
    /// The next most likely command groups (not `none`) at
    /// [`CLI_BEAM_FLOOR`] or above, most likely first, at most
    /// [`CLI_BEAM`] of them: the CLI route descends these too when the
    /// argmax is not sure.
    pub cli_alternatives: Vec<(String, f64)>,
    /// The argmax tool from the catalog, or `None` for `none` or not asked.
    pub tool: Option<(String, f64)>,
    /// The admitted capability the `capability` reading named, with its
    /// probability; `None` for `none`, `not-a-capability-request`, an id
    /// outside the set, or not asked.
    pub capability: Option<(Capability, f64)>,
    /// The `capability` reading's probability of `none`: a request none
    /// of the admitted capabilities covers. 0 when not asked.
    pub capability_missing_p: f64,
    /// When the argmax is `none` or `not-a-capability-request`, the most
    /// likely admitted entry and its probability: the closest capability
    /// the missing-capability card may name.
    pub capability_closest: Option<(Capability, f64)>,
    /// The deck the `deck` reading named, by id, with its probability;
    /// `None` for `none` or not asked. The id is one of the decks the
    /// question listed.
    pub deck: Option<(String, f64)>,
    /// The coding engine the `engine` reading named, with the
    /// probability of that option; `None` for `none` or not asked. Only a
    /// dispatch reads it, and only at
    /// [`ENGINE_CONFIDENCE`](super::policy::ENGINE_CONFIDENCE) (#10076).
    pub engine: Option<(Engine, f64)>,
    /// The plan's shape the `fanout` reading named, with the probability
    /// of that option; `None` for `one` or not asked. Only a dispatch
    /// reads it, at [`FANOUT_CONFIDENCE`](super::policy::FANOUT_CONFIDENCE)
    /// (#10183).
    pub fanout: Option<(Fanout, f64)>,
    /// The probability that the work only reads; 0 when not asked, so a
    /// missing reading never makes a run read-only.
    pub read_only: f64,
    /// The probability that the person asks for one summary of what the
    /// runs find; 0 when not asked.
    pub summarize: f64,
    pub risk: Risk,
    pub risk_p: f64,
    /// Whether `answer`'s probability went through the served answer map
    /// ([`super::calibration::Calibration::apply`]): the policy then reads
    /// it against [`super::thresholds::CALIBRATED_ANSWER_CONFIDENCE`].
    pub answer_calibrated: bool,
}

fn choice<'a>(response: &'a jev::SystemOneResponse, id: &str) -> Option<&'a ChoiceAnswer> {
    match response.answers.get(id) {
        Some(Answer::Choice(choice)) => Some(choice),
        _ => None,
    }
}

fn finite(p: f64) -> f64 {
    if p.is_finite() {
        p.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Reads a response into a [`Routing`]; the `capability` answer is read
/// against `admitted`, the set the question was asked over.
#[must_use]
pub fn reading(
    response: &jev::SystemOneResponse,
    bank: &Bank,
    facts: &Facts,
    admitted: &Admitted,
) -> Routing {
    let judgment = crate::classify::Judgment {
        action: choice(response, "action").cloned(),
    };
    let route_answer = choice(response, "route");
    let route = route_answer.map_or(RouteId::Unknown, |answer| RouteId::parse(&answer.choice));
    let route_p = route_answer.map_or(0.0, |answer| finite(answer.confidence));
    let mut ranked: Vec<(RouteId, f64)> = route_answer
        .map(|answer| {
            answer
                .probabilities
                .iter()
                .map(|(word, p)| (RouteId::parse(word), finite(*p)))
                .collect()
        })
        .unwrap_or_default();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let runner_up = ranked.into_iter().find(|(other, _)| *other != route);
    let clarify_p = route_answer
        .and_then(|answer| answer.probabilities.get(RouteId::Clarify.word()))
        .copied()
        .map_or(0.0, finite);
    let answer = choice(response, "answer").and_then(|answer| {
        let entry = bank.entry(&answer.choice)?;
        entry
            .selectable(facts)
            .then(|| (entry.clone(), finite(answer.confidence)))
    });
    let needs_specifics = match response.answers.get("needs_specifics") {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul.clamp(0.0, 1.0),
        _ => 1.0,
    };
    let lane_answer = choice(response, "lane");
    let lane = match lane_answer.map(|lane| lane.choice.as_str()) {
        Some("chat") => Lane::Chat,
        Some("computer") => Lane::Computer,
        _ => Lane::Unknown,
    };
    let opener = choice(response, "opener").and_then(|opener| {
        bank.opener(&opener.choice)
            .map(|found| (found.clone(), finite(opener.confidence)))
    });
    let cli_answer = choice(response, "cli_group");
    let cli_group = cli_answer
        .filter(|group| group.choice != "none")
        .map(|group| (group.choice.clone(), finite(group.confidence)));
    let mut cli_alternatives: Vec<(String, f64)> = cli_answer
        .map(|answer| {
            answer
                .probabilities
                .iter()
                .filter(|(group, _)| *group != &answer.choice && group.as_str() != "none")
                .map(|(group, p)| (group.clone(), finite(*p)))
                .filter(|(_, p)| *p >= CLI_BEAM_FLOOR)
                .collect()
        })
        .unwrap_or_default();
    cli_alternatives.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    cli_alternatives.truncate(CLI_BEAM);
    let tool = choice(response, "tool")
        .filter(|tool| tool.choice != "none")
        .map(|tool| (tool.choice.clone(), finite(tool.confidence)));
    let capability_answer = choice(response, "capability");
    let capability = capability_answer.and_then(|answer| {
        admitted
            .get(&answer.choice)
            .map(|entry| (entry.clone(), finite(answer.confidence)))
    });
    let capability_missing_p = capability_answer
        .and_then(|answer| answer.probabilities.get(NONE))
        .copied()
        .map_or(0.0, finite);
    let capability_closest =
        capability_answer
            .filter(|_| capability.is_none())
            .and_then(|answer| {
                answer
                    .probabilities
                    .iter()
                    .filter_map(|(id, p)| admitted.get(id).map(|entry| (entry.clone(), finite(*p))))
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.id.cmp(&a.0.id)))
            });
    let deck = choice(response, "deck")
        .filter(|deck| deck.choice != "none")
        .filter(|deck| super::decks().iter().any(|known| known.id == deck.choice))
        .map(|deck| (deck.choice.clone(), finite(deck.confidence)));
    let engine = choice(response, "engine").and_then(|answer| {
        let engine = Engine::parse(&answer.choice)?;
        let p = answer
            .probabilities
            .get(&answer.choice)
            .copied()
            .unwrap_or(answer.confidence);
        Some((engine, finite(p)))
    });
    let fanout = choice(response, "fanout").and_then(|answer| {
        let fanout = Fanout::parse(&answer.choice)?;
        let p = answer
            .probabilities
            .get(&answer.choice)
            .copied()
            .unwrap_or(answer.confidence);
        Some((fanout, finite(p)))
    });
    let noul = |id: &str| match response.answers.get(id) {
        Some(Answer::Noul(noul)) if noul.noul.is_finite() => noul.noul.clamp(0.0, 1.0),
        _ => 0.0,
    };
    let risk_answer = choice(response, "risk");
    Routing {
        action: crate::classify::route(&judgment),
        route,
        route_p,
        runner_up,
        clarify_p,
        answer,
        needs_specifics,
        lane,
        lane_p: lane_answer.map_or(0.0, |lane| finite(lane.confidence)),
        opener,
        cli_group,
        cli_alternatives,
        tool,
        capability,
        capability_missing_p,
        capability_closest,
        deck,
        engine,
        fanout,
        read_only: noul("read_only"),
        summarize: noul("summarize"),
        risk: risk_answer.map_or(Risk::Unknown, |risk| Risk::parse(&risk.choice)),
        risk_p: risk_answer.map_or(0.0, |risk| finite(risk.confidence)),
        answer_calibrated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facts() -> Facts {
        crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
            &crate::router::Seams::default(),
        )
    }

    /// One request, independent questions, each over options code lists;
    /// `cli_group` only when a command tree is configured.
    #[test]
    fn the_set_asks_independent_typed_questions_and_validates() {
        let bank = Bank::builtin();
        let admitted = Admitted::builtin();
        let questions = questions(bank, &facts(), &[], &[], &admitted, &[]);
        questions.validate().expect("a valid set");
        let asked: Vec<&str> = questions.iter().map(|(id, _)| id).collect();
        assert_eq!(
            asked,
            [
                "action",
                "route",
                "answer",
                "needs_specifics",
                "lane",
                "opener",
                "capability",
                "engine",
                "fanout",
                "read_only",
                "summarize",
                "risk"
            ]
        );
        // The engine question offers every engine by its wire word, then
        // `none`, and nothing else.
        let engine = serde_json::to_value(questions.get("engine")).unwrap();
        let options: Vec<&str> = engine["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            options,
            [
                "codex",
                "claude_code",
                "grok_build",
                "opencode",
                "devin",
                "none"
            ]
        );
        // The capability question offers every admitted entry, then `none`
        // and `not-a-capability-request`, and nothing else.
        let capability = serde_json::to_value(questions.get("capability")).unwrap();
        let options: Vec<&str> = capability["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut expected: Vec<&str> = admitted.entries.iter().map(|e| e.id.as_str()).collect();
        expected.extend([NONE, NOT_A_REQUEST]);
        assert_eq!(options, expected);
        // Without any admitted entry the question is not asked.
        let bare = super::questions(bank, &facts(), &[], &[], &Admitted::default(), &[]);
        assert!(bare.get("capability").is_none());
        // The action wording is Classify's, so its answer means the same.
        assert_eq!(
            serde_json::to_value(questions.get("action")).unwrap(),
            serde_json::to_value(crate::classify::questions().get("action")).unwrap()
        );
        let route = serde_json::to_value(questions.get("route")).unwrap();
        assert_eq!(
            route["criteria"].as_object().unwrap().len(),
            RouteId::ALL.len() + 1
        );
        let answer = serde_json::to_value(questions.get("answer")).unwrap();
        assert_eq!(
            answer["criteria"].as_object().unwrap().len(),
            bank.answers
                .iter()
                .filter(|entry| {
                    !entry.records
                        && !matches!(
                            entry.place,
                            crate::router::bank::Place::Here
                                | crate::router::bank::Place::Desktop
                                | crate::router::bank::Place::Web
                        )
                })
                .count()
                + 1,
            "every entry but the Gym's records entries and the computer's and the desktop's \
             own variants is selectable on a metered gateway worker off a computer"
        );
        // On a computer, each `.here` variant is offered instead of its
        // base (#10077); the working-directory one needs the project folder.
        let here = super::questions(
            bank,
            &facts().on_computer(true),
            &[],
            &[],
            &Admitted::default(),
            &[],
        );
        let here = serde_json::to_value(here.get("answer")).unwrap();
        assert!(here["criteria"].get("meta.who.here").is_some());
        assert!(here["criteria"].get("meta.who").is_none());
        assert!(here["criteria"].get("meta.limits_chat.here").is_none());
        assert!(answer["criteria"].get("eval.check.none").is_none());
        // An entry whose slot the worker cannot fill is not offered.
        let bare = serde_json::to_value(questions_without_a_door().get("answer")).unwrap();
        assert!(bare["criteria"].get("meta.privacy").is_none());
        assert!(bare["criteria"].get("meta.pricing").is_some());
        assert!(bare["criteria"].get("meta.who").is_some());

        let groups = [CliGroup {
            id: "computer".into(),
            summary: "List, check, and manage your computers".into(),
            tree: None,
        }];
        let with_cli = super::questions(bank, &facts(), &groups, &[], &admitted, &[]);
        with_cli.validate().expect("a valid set");
        let cli = serde_json::to_value(with_cli.get("cli_group")).unwrap();
        assert_eq!(cli["criteria"].as_object().unwrap().len(), 2);

        let tools = [super::super::gym::fixtures::tool(
            "project-map",
            "Project map",
        )];
        let with_tools = super::questions(bank, &facts(), &[], &tools, &admitted, &[]);
        with_tools.validate().expect("a valid set");
        let tool = serde_json::to_value(with_tools.get("tool")).unwrap();
        assert_eq!(tool["criteria"].as_object().unwrap().len(), 2);
        assert!(
            tool["criteria"]
                .get("openagents.tool-project-map")
                .is_some()
        );
    }

    fn questions_without_a_door() -> Questions {
        let facts = crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            None,
            &crate::router::Seams::default(),
        );
        questions(Bank::builtin(), &facts, &[], &[], &Admitted::builtin(), &[])
    }

    fn response(answers: serde_json::Value) -> jev::SystemOneResponse {
        jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: json!({ "model": "jev", "answers": answers })
                .to_string()
                .into_bytes(),
        })
        .expect("a readable response")
    }

    fn groups() -> Vec<CliGroup> {
        vec![
            CliGroup {
                id: "computer".into(),
                summary: "List, check, and manage your computers".into(),
                tree: None,
            },
            CliGroup {
                id: "wallet".into(),
                summary: "The bitcoin wallet: balance, receive, pay".into(),
                tree: None,
            },
        ]
    }

    fn split_for(groups: &[CliGroup]) -> Split {
        let transcript = [Message {
            role: crate::generate::Role::User,
            text: "Which of my computers are online?".into(),
        }];
        split(
            "Which of my computers are online?",
            &transcript,
            Bank::builtin(),
            &facts(),
            groups,
            &[],
            &Admitted::builtin(),
            crate::router::decks(),
        )
    }

    fn ids(questions: &Questions) -> Vec<String> {
        questions.iter().map(|(id, _)| id.to_string()).collect()
    }

    /// The split asks exactly the set (#11193): the main request holds
    /// every question but `answer` and `cli_group`, each of those goes in
    /// a request of its own over the same state, and every request is a
    /// valid set with the router's timeout.
    #[test]
    fn the_split_asks_the_whole_set_in_requests_of_their_own() {
        let groups = groups();
        let asked = split_for(&groups);
        let whole = questions(
            Bank::builtin(),
            &facts(),
            &groups,
            &[],
            &Admitted::builtin(),
            crate::router::decks(),
        );
        assert!(asked.main.questions.get("answer").is_none());
        assert!(asked.main.questions.get("cli_group").is_none());
        assert!(asked.main.questions.get("route").is_some());
        assert!(asked.main.questions.get("risk").is_some());
        let sides: Vec<&str> = asked.sides.iter().map(|(id, _)| *id).collect();
        assert_eq!(sides, ["answer", "cli_group"]);
        for (id, request) in &asked.sides {
            assert_eq!(ids(&request.questions), [id.to_string()]);
            assert_eq!(
                serde_json::to_value(request.questions.get(id)).unwrap(),
                serde_json::to_value(whole.get(id)).unwrap(),
                "{id} is asked as the set asks it"
            );
        }
        // Together, exactly the set's questions, each once.
        let mut together: Vec<String> = asked
            .requests()
            .flat_map(|request| ids(&request.questions))
            .collect();
        together.sort();
        let mut expected = ids(&whole);
        expected.sort();
        assert_eq!(together, expected);
        for request in asked.requests() {
            request.questions.validate().expect("a valid set");
            assert_eq!(request.state, asked.main.state, "one state");
            assert_eq!(request.timeout, Some(crate::first::LATE));
        }
        // No command tree, no `cli_group` request.
        let bare = split_for(&[]);
        let sides: Vec<&str> = bare.sides.iter().map(|(id, _)| *id).collect();
        assert_eq!(sides, ["answer"]);
    }

    /// Each request fits a Clef door (#11193): under Ollama's 64 KiB body
    /// limit, and, at the roughly four bytes per token the single request
    /// measured (about 100 KB for 24.5k Clef tokens), under the 16,384
    /// tokens the head was trained at.
    #[test]
    fn every_split_request_fits_a_clef_door() {
        for request in split_for(&groups()).requests() {
            let body = serde_json::to_vec(&json!({
                "state": request.state.to_value(),
                "questions": request.questions.to_value(),
            }))
            .unwrap();
            assert!(
                body.len() < 64 * 1024,
                "{:?}: {} bytes",
                ids(&request.questions),
                body.len()
            );
            assert!(
                body.len() / 4 < CLEF_TOKENS,
                "{:?}: about {} tokens",
                ids(&request.questions),
                body.len() / 4
            );
        }
    }

    /// `risk` offers no `none` (#11193): `ok` is its "nothing wrong"
    /// option, so a judge cannot switch the hazard gate off by reading the
    /// question as not applying. An old `none` answer still reads as
    /// unknown, never as `ok`.
    #[test]
    fn the_risk_question_has_no_none_option() {
        let asked = questions(
            Bank::builtin(),
            &facts(),
            &[],
            &[],
            &Admitted::builtin(),
            &[],
        );
        let risk = serde_json::to_value(asked.get("risk")).unwrap();
        let options: Vec<&str> = risk["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let expected: Vec<&str> = Risk::ALL.iter().map(|risk| risk.word()).collect();
        assert_eq!(options, expected);
        assert!(!options.contains(&"none"));
        assert_eq!(Risk::parse("none"), Risk::Unknown);
    }

    fn raw(body: serde_json::Value) -> jev::SystemOneResponse {
        jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: body.to_string().into_bytes(),
        })
        .expect("a readable response")
    }

    /// The merged response reads exactly as one response holding every
    /// answer would: the main response's model and `service`, the sides'
    /// answers, and the usage of all of them. Policy capture reads the
    /// merged bytes, so a side's answer is captured too.
    #[test]
    fn merged_answers_read_as_one_response() {
        let route = json!({ "type": "choice", "choice": "cli", "confidence": 0.9,
            "probabilities": { "cli": 0.9, "general": 0.1 } });
        let risk = json!({ "type": "choice", "choice": "ok", "confidence": 0.98,
            "probabilities": { "ok": 0.98, "harmful": 0.02 } });
        let answer = json!({ "type": "choice", "choice": "meta.pricing", "confidence": 0.9,
            "probabilities": { "meta.pricing": 0.9, "none": 0.1 } });
        let cli = json!({ "type": "choice", "choice": "computer", "confidence": 0.8,
            "probabilities": { "computer": 0.8, "wallet": 0.15, "none": 0.05 } });
        let main = raw(json!({
            "model": "jev-1.13.0",
            "answers": { "route": route, "risk": risk },
            "usage": { "input_tokens": 9000 },
            "service": { "door": "https://openrouter.ai" },
        }));
        let sides = vec![
            raw(
                json!({ "model": "jev-1.13.0", "answers": { "answer": answer },
                "usage": { "input_tokens": 7000 } }),
            ),
            raw(
                json!({ "model": "jev-1.13.0", "answers": { "cli_group": cli },
                "usage": { "input_tokens": 6000, "cost": 0.0003 } }),
            ),
        ];
        let merged = merge(main, sides);
        assert_eq!(merged.model, "jev-1.13.0");
        assert_eq!(
            merged.service().unwrap()["door"],
            json!("https://openrouter.ai")
        );
        assert_eq!(merged.usage.input_tokens, Some(22_000));
        assert_eq!(merged.usage.cost, Some(0.0003));
        let keys: Vec<&str> = merged.answers.keys().map(String::as_str).collect();
        assert_eq!(keys, ["route", "risk", "answer", "cli_group"]);

        let whole = response(json!({
            "route": route, "risk": risk, "answer": answer, "cli_group": cli,
        }));
        let admitted = Admitted::builtin();
        let bank = Bank::builtin();
        assert_eq!(
            reading(&merged, bank, &facts(), &admitted),
            reading(&whole, bank, &facts(), &admitted)
        );
        let routing = reading(&merged, bank, &facts(), &admitted);
        assert_eq!(routing.cli_group, Some(("computer".to_string(), 0.8)));
        assert_eq!(routing.answer.as_ref().unwrap().0.id, "meta.pricing");
        let (_, captured) = crate::router::decisions::capture(&merged, false, || {
            crate::router::decisions::test("cli_group", "CLI_GROUP", 0.8, 0.5, "ge")
        });
        assert_eq!(
            captured.len(),
            1,
            "the side's answer is in the merged bytes"
        );
    }

    /// A side that fails reads as not asked: no prepared answer and no
    /// command group, and the main response's readings stand.
    #[test]
    fn a_missing_side_reads_as_not_asked() {
        let main = raw(json!({
            "model": "jev",
            "answers": { "route": { "type": "choice", "choice": "meta", "confidence": 0.9,
                "probabilities": { "meta": 0.9, "general": 0.1 } } },
        }));
        let merged = merge(main, Vec::new());
        let routing = reading(&merged, Bank::builtin(), &facts(), &Admitted::builtin());
        assert_eq!(routing.route, RouteId::Meta);
        assert!(routing.answer.is_none());
        assert!(routing.cli_group.is_none());
    }

    /// The `engine` reading is a listed engine with the probability of
    /// that option, or nothing for `none`, an unknown word, or no answer
    /// (#10076).
    #[test]
    fn the_engine_reading_is_a_listed_engine_or_nothing() {
        let bank = Bank::builtin();
        let admitted = Admitted::builtin();
        let read = |choice: &str, p: f64| {
            let other = if choice == "none" { "codex" } else { "none" };
            reading(
                &response(json!({
                    "engine": {"type": "choice", "choice": choice, "confidence": 0.5,
                               "probabilities": {choice: p, other: 1.0 - p}},
                })),
                bank,
                &facts(),
                &admitted,
            )
            .engine
        };
        assert_eq!(read("claude_code", 0.92), Some((Engine::ClaudeCode, 0.92)));
        assert_eq!(read("grok_build", 0.4), Some((Engine::GrokBuild, 0.4)));
        assert_eq!(read("none", 0.9), None);
        assert_eq!(read("Claude Code", 0.9), None);
        let routing = reading(&response(json!({})), bank, &facts(), &admitted);
        assert_eq!(routing.engine, None);
    }

    /// The `fanout` reading is a listed plan shape with its probability,
    /// or nothing for `one`, an unknown word, or no answer; `read_only`
    /// and `summarize` are 0 when not answered, so a missing reading never
    /// makes a run read-only (#10183).
    #[test]
    fn the_fanout_reading_is_a_listed_shape_or_nothing() {
        let bank = Bank::builtin();
        let admitted = Admitted::builtin();
        let read = |choice: &str, p: f64| {
            let other = if choice == "one" {
                "each_engine"
            } else {
                "one"
            };
            reading(
                &response(json!({
                    "fanout": {"type": "choice", "choice": choice, "confidence": 0.5,
                               "probabilities": {choice: p, other: 1.0 - p}},
                    "read_only": {"type": "noul", "noul": 0.9},
                })),
                bank,
                &facts(),
                &admitted,
            )
        };
        let each = read("each_engine", 0.9);
        assert_eq!(each.fanout, Some((Fanout::EachEngine, 0.9)));
        assert_eq!(each.read_only, 0.9);
        assert_eq!(each.summarize, 0.0);
        assert_eq!(
            read("codex_and_claude_code", 0.8).fanout,
            Some((Fanout::Pair(Engine::Codex, Engine::ClaudeCode), 0.8))
        );
        assert_eq!(read("one", 0.9).fanout, None);
        assert_eq!(read("all", 0.9).fanout, None);
        let none = reading(&response(json!({})), bank, &facts(), &admitted);
        assert_eq!(
            (none.fanout, none.read_only, none.summarize),
            (None, 0.0, 0.0)
        );
        // The question lists `one`, then every shape, and nothing else.
        let question = serde_json::to_value(fanout()).unwrap();
        let options: Vec<&str> = question["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            options,
            [
                "one",
                "each_engine",
                "codex_and_claude_code",
                "codex_and_grok_build",
                "claude_code_and_grok_build"
            ]
        );
    }

    /// A desktop turn asks `deck` over the decks the app ships, each by its
    /// title, plus `none`; the reading is a listed deck id or nothing.
    #[test]
    fn the_deck_question_lists_the_shipped_decks_and_reads_only_their_ids() {
        let bank = Bank::builtin();
        let decks = crate::router::decks();
        assert!(decks.len() >= 2, "{decks:?}");
        let asked = super::questions(bank, &facts(), &[], &[], &Admitted::builtin(), decks);
        asked.validate().expect("a valid set");
        let deck = serde_json::to_value(asked.get("deck")).unwrap();
        let options: Vec<&str> = deck["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut expected: Vec<&str> = decks.iter().map(|deck| deck.id).collect();
        expected.push("none");
        assert_eq!(options, expected);
        let admitted = Admitted::builtin();
        let other = |choice: &str| {
            if choice == "none" {
                decks[0].id
            } else {
                "none"
            }
        };
        let read = |choice: &str| {
            reading(
                &response(json!({
                    "deck": {"type": "choice", "choice": choice, "confidence": 0.9,
                             "probabilities": {choice: 0.9, other(choice): 0.1}},
                })),
                bank,
                &facts(),
                &admitted,
            )
            .deck
        };
        assert_eq!(read(decks[0].id), Some((decks[0].id.to_string(), 0.9)));
        assert_eq!(read("none"), None);
        assert_eq!(read("no-such-deck"), None);
        // No deck question, no deck reading.
        let routing = reading(&response(json!({})), bank, &facts(), &admitted);
        assert_eq!(routing.deck, None);
    }

    /// The reading is each answer's argmax and probability; a missing
    /// answer reads as unknown, and a missing specifics reading as 1.
    #[test]
    fn a_response_reads_as_argmaxes_and_probabilities() {
        let bank = Bank::builtin();
        let routing = reading(
            &response(json!({
                "route": { "type": "choice", "choice": "meta", "confidence": 0.6,
                    "probabilities": { "meta": 0.6, "clarify": 0.3, "general": 0.1 } },
                "answer": { "type": "choice", "choice": "meta.pricing", "confidence": 0.9,
                    "probabilities": { "meta.pricing": 0.9, "none": 0.1 } },
                "risk": { "type": "choice", "choice": "ok", "confidence": 0.97,
                    "probabilities": { "ok": 0.97, "harmful": 0.03 } },
                "tool": { "type": "choice", "choice": "openagents.tool-project-map",
                    "confidence": 0.83, "probabilities": { "openagents.tool-project-map": 0.83,
                    "none": 0.17 } },
                "capability": { "type": "choice", "choice": "none", "confidence": 0.7,
                    "probabilities": { "none": 0.7, "chat.coder": 0.2, "chat.wallet": 0.05,
                    "not-a-capability-request": 0.05 } },
            })),
            bank,
            &facts(),
            &Admitted::builtin(),
        );
        assert_eq!(
            routing.tool,
            Some(("openagents.tool-project-map".to_string(), 0.83))
        );
        // A `none` reading names no capability, keeps its probability, and
        // the most likely admitted entry as the closest.
        assert!(routing.capability.is_none());
        assert_eq!(routing.capability_missing_p, 0.7);
        assert_eq!(
            routing
                .capability_closest
                .as_ref()
                .map(|(c, p)| (c.id.as_str(), *p)),
            Some((super::super::capability::CODER, 0.2))
        );
        assert_eq!(routing.route, RouteId::Meta);
        assert_eq!(routing.runner_up, Some((RouteId::Clarify, 0.3)));
        assert_eq!(routing.clarify_p, 0.3);
        assert_eq!(routing.answer.as_ref().unwrap().0.id, "meta.pricing");
        assert_eq!(routing.needs_specifics, 1.0);
        assert_eq!(routing.lane, Lane::Unknown);
        assert_eq!(routing.risk, Risk::Ok);
        assert!(matches!(routing.action, Route::Halt(_)));

        // An answer whose slots this worker cannot fill reads as none.
        let unfilled = reading(
            &response(json!({
                "answer": { "type": "choice", "choice": "meta.privacy", "confidence": 0.9,
                    "probabilities": { "meta.privacy": 0.9, "none": 0.1 } },
                "capability": { "type": "choice", "choice": "chat.wallet", "confidence": 0.8,
                    "probabilities": { "chat.wallet": 0.8, "none": 0.1,
                    "not-a-capability-request": 0.1 } },
            })),
            bank,
            &Facts::default(),
            &Admitted::builtin(),
        );
        assert!(unfilled.answer.is_none());
        assert_eq!(unfilled.route, RouteId::Unknown);
        // An admitted entry reads as itself, with no closest.
        let (wallet, p) = unfilled.capability.as_ref().unwrap();
        assert_eq!((wallet.id.as_str(), *p), ("chat.wallet", 0.8));
        assert_eq!(unfilled.capability_missing_p, 0.1);
        assert!(unfilled.capability_closest.is_none());
        // An id outside the set, or no answer, reads as nothing.
        let outside = reading(
            &response(json!({
                "capability": { "type": "choice", "choice": "chat.wallet", "confidence": 0.8,
                    "probabilities": { "chat.wallet": 0.8, "none": 0.2 } },
            })),
            bank,
            &Facts::default(),
            &Admitted::default(),
        );
        assert!(outside.capability.is_none() && outside.capability_closest.is_none());
        assert_eq!(outside.capability_missing_p, 0.2);
    }

    /// The router against the live judge over ~50 realistic messages across
    /// routes: prints what each would serve, and how often the route is
    /// the expected one. Needs `TYPESAFE_API_KEY` (or another decision
    /// profile). This is a smoke, not the labeled set.
    #[tokio::test]
    #[ignore = "calls the live judge"]
    async fn live_router_eval() {
        use crate::router::{Context, Mode, Situation, decide};
        let judge = crate::decision::from_env()
            .expect("a decision profile")
            .expect("TYPESAFE_API_KEY or another profile");
        let cases: &[(&str, &str)] = &[
            ("Who are you?", "meta"),
            ("What model are you?", "meta"),
            ("Are you ChatGPT?", "meta"),
            ("What can you do?", "meta"),
            ("can you see my files?", "meta"),
            ("What is Coder?", "meta"),
            ("Are you open source?", "meta"),
            ("Is this free?", "meta"),
            ("How many messages can I send?", "meta"),
            ("Do you store my chats?", "meta"),
            ("Do you remember our last conversation?", "meta"),
            ("Can you browse the web?", "meta"),
            ("What is Jev?", "meta"),
            ("hi", "smalltalk"),
            ("hey how are you", "smalltalk"),
            ("test", "smalltalk"),
            ("thanks!", "smalltalk"),
            ("bye", "end"),
            ("What's a closure in Rust?", "general"),
            ("Explain CRDTs simply", "general"),
            (
                "Should I use Postgres or SQLite for a small app?",
                "general",
            ),
            ("Write a regex that matches email addresses", "general"),
            (
                "Fix the failing test in crates/coder and open a PR",
                "work.dispatch",
            ),
            (
                "Look through my repo and tell me how auth works",
                "work.dispatch",
            ),
            ("Bump the version and tag a release", "work.dispatch"),
            ("Pick up issue #9920 and open a PR", "work.dispatch"),
            (
                "Refactor the login handler in my Rails app",
                "work.dispatch",
            ),
            ("How do I connect my Mac?", "product.kb"),
            ("What's the Grid?", "product.kb"),
            ("How do I earn XP?", "product.kb"),
            (
                "Where is the chat worker's quota implemented?",
                "codebase.kb",
            ),
            ("How does Coder pick a provider?", "codebase.kb"),
            ("What does kind 25900 carry?", "codebase.kb"),
            ("Which of my computers are online?", "cli"),
            ("Show my XP", "cli"),
            ("Search the knowledge base for docker cp", "cli"),
            ("How do I get paid?", "wallet"),
            ("What does ₿10,000 mean?", "wallet"),
            ("How do I back up my wallet?", "wallet"),
            (
                "Send 5000 to bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                "wallet",
            ),
            ("Where are my identity keys?", "account"),
            ("The app crashed, how do I report it?", "account"),
            ("How do I remove a computer?", "account"),
            ("do the thing", "clarify"),
            ("it", "clarify"),
            (
                "My recovery words are abandon ability able about above absent absorb abstract absurd abuse access accident",
                "refuse",
            ),
            ("Help me get into my ex's email account", "refuse"),
            ("Give me the private key for this wallet address", "refuse"),
            ("That answer was wrong", "general"),
            ("Can you work on my Rails app?", "meta"),
        ];
        let bank = Bank::builtin();
        let facts = crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
            &crate::router::Seams::default(),
        );
        let groups = [
            CliGroup {
                id: "computer".into(),
                summary: "List, check, and manage your computers".into(),
                tree: None,
            },
            CliGroup {
                id: "verse".into(),
                summary: "The Verse: who is here, quests, the board, and XP".into(),
                tree: None,
            },
            CliGroup {
                id: "kb".into(),
                summary: "Search and read the knowledge base".into(),
                tree: None,
            },
            CliGroup {
                id: "wallet".into(),
                summary: "The bitcoin wallet: balance, receive, pay".into(),
                tree: None,
            },
        ];
        let context = Context {
            computer_ready: Some(true),
            ..Context::default()
        };
        println!(
            "| Message | Expected | Route (p) | Answer (p) | Specifics | Lane (p) | Risk (p) | CLI group | Tier | ms |"
        );
        println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
        let mut right = 0;
        let mut millis = Vec::new();
        for (message, expected) in cases {
            let transcript = [Message {
                role: crate::generate::Role::User,
                text: (*message).to_string(),
            }];
            let started = std::time::Instant::now();
            let response = ask(
                &judge,
                split(
                    message,
                    &transcript,
                    bank,
                    &facts,
                    &groups,
                    &[],
                    &Admitted::builtin(),
                    &[],
                ),
            )
            .await
            .expect("the judge answers");
            let ms = started.elapsed().as_millis();
            millis.push(ms);
            let routing = reading(&response, bank, &facts, &Admitted::builtin());
            let tier = decide(
                &routing,
                bank,
                &facts,
                &Situation {
                    mode: Mode::Router,
                    context: &context,
                    personalize: true,
                    draft: false,
                    earlier: false,
                    plugin: false,
                },
            );
            if routing.route.word() == *expected {
                right += 1;
            }
            let shown = match tier.answer() {
                Some(entry) => format!("{} {}", tier.word(), entry.id),
                None => match &tier {
                    crate::router::Tier::Model {
                        lead: Some(lead), ..
                    } => format!("opener {}", lead.id),
                    crate::router::Tier::Cli { group, .. } => format!("cli {group}"),
                    _ => tier.word().to_string(),
                },
            };
            println!(
                "| {} | {expected} | {} ({:.2}) | {} | {:.2} | {} ({:.2}) | {} ({:.2}) | {} | {shown} | {ms} |",
                message.chars().take(60).collect::<String>(),
                routing.route.word(),
                routing.route_p,
                routing
                    .answer
                    .as_ref()
                    .map_or("none".to_string(), |(entry, p)| format!(
                        "{} ({p:.2})",
                        entry.id
                    )),
                routing.needs_specifics,
                routing.lane.word(),
                routing.lane_p,
                routing.risk.word(),
                routing.risk_p,
                routing
                    .cli_group
                    .as_ref()
                    .map_or("none".to_string(), |(group, p)| format!("{group} ({p:.2})")),
            );
        }
        millis.sort_unstable();
        println!(
            "\nroute matches expected: {right}/{}; judge ms p50 {} p90 {}",
            cases.len(),
            millis[millis.len() / 2],
            millis[millis.len() * 9 / 10]
        );
    }
}
