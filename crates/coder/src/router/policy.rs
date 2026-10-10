//! The policy table: code, not the judge, decides what a turn shows.
//!
//! [`decide`] maps a [`Routing`] and the turn's situation to a [`Tier`].
//! Changing a threshold here never reruns inference and never changes a
//! question's meaning. The thresholds are the design's starting values;
//! the labeled `chat-router-v2` set is what moves them.
//!
//! The rules, in the order they apply:
//!
//! 0. **An open interview.** A request that carries the authoring
//!    interview's draft continues it ([`Tier::Author`]) unless the route
//!    reading is sure of a route in no way part of it
//!    ([`AUTHOR_CONTINUES`]); risk is still read first. A plugin being made
//!    on this computer (#10177) continues the same way, and also on a
//!    reply that reads as more work or a command ([`PLUGIN_CONTINUES`]).
//! 1. **Risk.** `secret_shared`, `asks_for_secret`, or `harmful` at
//!    [`RISK_REFUSE`], or at [`RISK_WARN`] when `route` is also `refuse` at
//!    [`ROUTE_CONFIDENCE`], answers with the bank's refusal and nothing
//!    else. In
//!    the warn band ([`RISK_WARN`] up to the refusal), no prepared answer,
//!    stem, or offer is used; a possible secret gets the bank's warning as
//!    the lead line, and the model answers under its own safety behavior.
//!    `money_movement` at [`RISK_WARN`] answers `wallet.send` with the
//!    wallet screen offered and no amount carried.
//! 2. **Close call.** When the second route is within [`CLOSE_MARGIN`] of
//!    the first, the router does less: a clarify at [`CLARIFY_WINS`], else
//!    the model (with a Run Coder offer only when `work.dispatch` is one
//!    of the two, the other is in [`LANE_ROUTES`], and the lane says
//!    computer).
//! 3. **T0 canned final.** `route` at [`ROUTE_CONFIDENCE`], `answer` at
//!    [`ANSWER_CONFIDENCE`] on an entry of that route with text,
//!    `needs_specifics` below [`SPECIFICS_CEILING`]. Dispatch and CLI
//!    entries are never picked this way.
//! 4. **End.** `route` = `end` at [`ROUTE_CONFIDENCE`]: `smalltalk.bye`.
//! 5. **T1 stem.** `answer` at [`STEM_CONFIDENCE`] on a stemmed entry of
//!    the argmax route, with `needs_specifics` at the ceiling or above.
//! 6. **T4 dispatch.** `route` = `work.dispatch` at [`DISPATCH_ROUTE`]:
//!    with no computer ready, `dispatch.no_computer` and the computers
//!    screen; otherwise a `dispatch.*` stem (the one `answer` chose, else
//!    `dispatch.stem`) and a Run Coder offer.
//! 7. **T4 CLI.** `route` = `cli` at [`CLI_ROUTE`] and `cli_group` at
//!    [`CLI_GROUP`]; or `cli` at [`CLI_ROUTE_SURE`] and a group at
//!    [`CLI_GROUP_BEAM`], with the next likely groups descended beside it;
//!    or `cli` at [`GROUNDED_ROUTE`] and a group at [`CLI_GROUP_SURE`]:
//!    the CLI seam proposes, and the gate decides.
//! 8. **T2 grounded.** `route` = `product.kb`, `meta`, or `codebase.kb`
//!    at [`GROUNDED_ROUTE`]. Below it, and in a close call (rule 2), a
//!    question about us (`product.kb` or `meta` the argmax, or a close
//!    runner-up of `general`, `clarify`, or `none`) is still grounded,
//!    never left to the model alone (#10135, #10137).
//! 9. **Gym and eval.** `gym.news` at [`GROUNDED_ROUTE`], or another
//!    `eval.*` route at [`EVAL_ROUTE`]: `eval.author` is a step of the
//!    interview ([`Tier::Author`]), `eval.credit` the bank's
//!    `eval.credit.mine`, and the rest read the Gym's verified records
//!    ([`Tier::Gym`]), which [`super::gym::reply`] turns into a bank line,
//!    a card, and an offer, or a grounded reply for news.
//!
//!    **A deck** (#10058, rule 9b in the code). `route` =
//!    `presentation.open` at [`PRESENTATION_ROUTE`]: off the desktop, the
//!    bank's `presentation.elsewhere` line; on it, when the `deck` reading
//!    names one of the decks the app ships at [`DECK_CONFIDENCE`], the
//!    bank's `presentation.open` line with an `open_presentation` offer for
//!    that deck, and otherwise the plain `presentation.unknown` refusal,
//!    which lists the decks. The deck is an id the reading chose from the
//!    list, never text from the message.
//! 10. **Missing capability** (#9960). `route` = `capability.missing` at
//!     [`CAPABILITY_ROUTE`] and, independently, the `capability` reading
//!     names no admitted entry and reads `none` at [`CAPABILITY_MISSING`]:
//!     the bank's `capability.missing` line ([`Tier::Capability`]), or
//!     `capability.missing_near` naming the closest admitted capability
//!     when the reading puts one at [`CAPABILITY_CLOSEST`] or more. Two
//!     readings agree before the card shows, so a lone route reading never
//!     says a capability is missing. When the route reads missing but the
//!     `capability` reading names an admitted entry usable only in a Coder
//!     run at [`CAPABILITY_CONFIDENCE`], and the lane says computer, that
//!     entry answers: a dispatch offer naming it (rule 6's stem).
//! 11. **T4 dispatch by lane.** `lane` = computer at [`DISPATCH_LANE`],
//!     after the CLI and knowledge routes, which read such a message more
//!     precisely, and only on a route in [`LANE_ROUTES`]: an offer loses to
//!     a route with its own answer.
//! 12. **Clarify.** `route` = `clarify` at [`CLARIFY_ROUTE`]: the
//!     model told to answer a plain yes-or-no in a few words or ask one
//!     natural question ([`CLARIFY_NOTE`]); but an `answer` reading at
//!     [`ANSWER_CONFIDENCE`] on an entry that answers in the chat, with
//!     `needs_specifics` below [`SPECIFICS_CEILING`], serves that entry
//!     whole instead (#10138). On a later turn, a clarify (here or in
//!     rule 2) is the model told [`LATER_CLARIFY_NOTE`]: the earlier
//!     messages may already say what the latest one means (#10138).
//! 13. **T3 model**, led by the argmax opener at [`OPENER_CONFIDENCE`];
//!     when the route or the runner-up is a Gym or eval route, the model is
//!     told it has no verified records ([`super::gym::NO_RECORDS_NOTE`]).
//!
//! A dispatch offer (rules 2, 6, 10, and 11) names the capability the
//! `capability` reading found at [`CAPABILITY_CONFIDENCE`] when it is one
//! usable only in a Coder run (a catalog tool or an adoption): the
//! `dispatch.capability_stem` stem, with the entry's name as its slot.
//!
//! A request that asks only for `opener` or `judge` (the phones before the
//! router) is decided in [`Mode::Legacy`]: rule 3 for entries with no
//! offer, then rule 13, which is what `coder-first-response-v2` showed.

use super::bank::{Bank, Entry, Facts};
use super::capability::{Capability, Reach};
use super::judge::{RepositoryAsk, Routing};
use super::{Context, Corpus, Offer, Risk, RouteFamily, RouteId, Surface};
use crate::first::Lane;

/// The least `route` probability for a whole prepared answer or `end`.
pub const ROUTE_CONFIDENCE: f64 = 0.80;
/// The least raw `answer` probability for a whole prepared answer. A
/// reading that went through the answer map is read against
/// [`super::thresholds::CALIBRATED_ANSWER_CONFIDENCE`] instead
/// ([`answer_confidence`]).
pub const ANSWER_CONFIDENCE: f64 = 0.80;

/// The `answer` threshold this reading is held to, with its name for the
/// decision record: the cost-derived calibrated one when the answer map
/// applied (#10386), else the raw one.
fn answer_confidence(routing: &Routing) -> (&'static str, f64) {
    if routing.answer_calibrated {
        (
            "CALIBRATED_ANSWER_CONFIDENCE",
            super::thresholds::CALIBRATED_ANSWER_CONFIDENCE,
        )
    } else {
        ("ANSWER_CONFIDENCE", ANSWER_CONFIDENCE)
    }
}
/// The most `needs_specifics` at which a prepared answer may stand whole.
pub const SPECIFICS_CEILING: f64 = 0.30;
/// The least `answer` probability for a stem.
pub const STEM_CONFIDENCE: f64 = 0.70;
/// The least `opener` probability for an opener to lead the model.
pub const OPENER_CONFIDENCE: f64 = 0.70;
/// The least `work.dispatch` probability for a dispatch offer.
pub const DISPATCH_ROUTE: f64 = 0.70;
/// The least `lane` = computer probability for a dispatch offer.
pub const DISPATCH_LANE: f64 = 0.75;
/// The least `cli` probability for a CLI proposal.
pub const CLI_ROUTE: f64 = 0.75;
/// The least `cli_group` probability for a CLI proposal.
pub const CLI_GROUP: f64 = 0.60;
/// A `cli` route this sure proposes from a less sure group, with the other
/// likely groups descended beside it.
pub const CLI_ROUTE_SURE: f64 = 0.90;
/// The least `cli_group` probability for a beam from a sure route.
pub const CLI_GROUP_BEAM: f64 = 0.25;
/// A group this sure proposes from a `cli` route at [`GROUNDED_ROUTE`].
pub const CLI_GROUP_SURE: f64 = 0.75;
/// The routes whose messages a `lane` reading alone may offer to Coder:
/// work, and routes with no answer of their own. Every other route has its
/// own answer, which an offer loses to.
pub const LANE_ROUTES: [RouteId; 4] = [
    RouteId::WorkDispatch,
    RouteId::General,
    RouteId::Clarify,
    RouteId::Unknown,
];
/// The least risk probability at which the router warns.
pub const RISK_WARN: f64 = 0.60;
/// The least risk probability at which the router refuses.
pub const RISK_REFUSE: f64 = 0.85;
/// The least knowledge-route probability for a grounded reply.
pub const GROUNDED_ROUTE: f64 = 0.60;
/// Two routes this close are a close call.
pub const CLOSE_MARGIN: f64 = 0.15;
/// In a close call, a clarify this likely wins.
pub const CLARIFY_WINS: f64 = 0.40;
/// The least `clarify` probability, as the argmax, to ask a question.
pub const CLARIFY_ROUTE: f64 = 0.60;
/// The least `eval.*` route probability for a card, an offer, or an
/// interview step: like a dispatch offer, a wrong one costs an ignored
/// card.
pub const EVAL_ROUTE: f64 = 0.70;
/// The least `tool` probability at which the reading names the tool a
/// Gym reply is about.
pub const TOOL_CONFIDENCE: f64 = 0.60;
/// The least `capability.missing` route probability for the
/// missing-capability card: like an eval card, a wrong one costs an
/// ignored card, so it sits with [`EVAL_ROUTE`].
pub const CAPABILITY_ROUTE: f64 = 0.70;
/// The least `capability` = `none` probability for the card, read
/// independently of the route: the second reading that has to agree.
pub const CAPABILITY_MISSING: f64 = 0.60;
/// The least `capability` probability at which the reading names the
/// admitted capability a reply is about, or a dispatch offer names.
pub const CAPABILITY_CONFIDENCE: f64 = 0.60;
/// The least probability of an admitted entry, beside a `none` argmax,
/// for the missing-capability line to name it as the closest one.
pub const CAPABILITY_CLOSEST: f64 = 0.20;
/// The least `presentation.open` probability for a deck line: like an
/// eval card, a wrong one costs a viewer the person closes.
pub const PRESENTATION_ROUTE: f64 = 0.70;
/// The least `deck` probability at which the reading names the deck to
/// open.
pub const DECK_CONFIDENCE: f64 = 0.60;

/// The `standing.rule` probability at which a message is served as a
/// background rule (#10157): in a terminal, the computer then compiles it
/// and shows the rule; elsewhere, the line says where rules are made.
pub const STANDING_ROUTE: f64 = 0.60;
/// The least `engine` probability at which a dispatch offer names the
/// engine the person asked for (#10076). An unsure reading is no
/// preference: a wrong engine put first costs more than none.
pub const ENGINE_CONFIDENCE: f64 = 0.70;
/// The least `fanout` probability at which a dispatch starts one run on
/// each of several engines (#10183). An unsure reading is one run, as
/// before.
pub const FANOUT_CONFIDENCE: f64 = 0.70;
/// The least `read_only` probability at which a plan's runs are
/// read-only. A missed reading leaves the runs as a single run would be.
pub const READ_ONLY_CONFIDENCE: f64 = 0.70;
/// The least `summarize` probability at which the chat writes one
/// combined summary once a plan's runs end.
pub const SUMMARIZE_CONFIDENCE: f64 = 0.60;
/// The routes a message may take and still continue an open authoring
/// interview: the interview's own, running or reading its pilot, and the
/// short replies ("looks good", "change it") that answer its questions.
pub const AUTHOR_CONTINUES: [RouteId; 7] = [
    RouteId::EvalAuthor,
    RouteId::EvalRun,
    RouteId::EvalResult,
    RouteId::Smalltalk,
    RouteId::Clarify,
    RouteId::General,
    RouteId::Unknown,
];

/// The routes a message may take and still continue a plugin being made
/// on this computer (#10177): the interview's, and a reply that reads as
/// more work for Coder ("make the tests harder") or as a command ("turn it
/// on"), which the flow's own typed readings then decide.
pub const PLUGIN_CONTINUES: [RouteId; 2] = [RouteId::WorkDispatch, RouteId::Cli];

/// The instruction the model gets when the router wants one question.
pub const CLARIFY_NOTE: &str = "The user's message is short or unclear. If it is a simple yes-or-no \
question or a remark, just answer it the way a friendly person would, in a few words (\"Yep.\", \
\"Yes, it does.\", \"Oh yes.\"). Otherwise reply with one short, natural question that would let \
us answer or act, and nothing else. No preamble.";

/// The instruction the model gets when the router wants one question on a
/// later turn: the earlier messages may already say what a short message
/// means (#10138).
pub const LATER_CLARIFY_NOTE: &str = "The user's latest message is short or unclear on its \
own. Read it with the earlier messages: when they make clear what it asks, such as to try the \
last answer again or to go on, do that. Only when they do not, reply with one short question \
that would let us answer or act, and nothing else.";

/// How a turn asked for its first response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `"router": "chat-router-v2"` (or `chat-router-v1`): every tier.
    Router,
    /// `opener` or `judge` only: a whole prepared answer with no offer, an
    /// opener, or nothing, as `coder-first-response-v2` did.
    Legacy,
}

/// What the router knows about the turn beyond the judgment.
#[derive(Clone, Copy, Debug)]
pub struct Situation<'a> {
    pub mode: Mode,
    pub context: &'a Context,
    /// Whether a personalization provider is configured.
    pub personalize: bool,
    /// Whether the request carries an authoring interview's draft that
    /// passed [`super::card::draft`]: a bounded field, never text.
    pub draft: bool,
    /// Whether the conversation has a message before the latest one, which
    /// the latest can refer to: a count, never text (#10138).
    pub earlier: bool,
    /// Whether a plugin is being made on this computer (#10177): our last
    /// message ended at an open step's fixed line
    /// (`crate::eval_author::plugin::open`), an exact comparison.
    pub plugin: bool,
}

/// A line above the model's reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lead {
    /// The opener's or bank entry's id (`explain`, `warn.secret_shared`).
    pub id: String,
    /// The line, as shown.
    pub text: String,
}

/// What the turn shows. Pure data; the worker acts on it.
#[derive(Clone, Debug, PartialEq)]
pub enum Tier {
    /// T0 (T4 for a dispatch entry): a whole bank answer; the model call
    /// is cancelled.
    CannedFinal {
        answer: Entry,
        text: String,
        offer: Option<Offer>,
    },
    /// T1 (T4 with a Run Coder offer): a bank stem now, then a validated
    /// continuation or the stem's generic end; the model call is
    /// cancelled.
    CannedStem {
        answer: Entry,
        stem: String,
        generic_end: String,
        offer: Option<Offer>,
        /// Whether a continuation may be asked for (never after a possible
        /// secret).
        personalize: bool,
    },
    /// T2: retrieve from `corpus`, then the model answers from what was
    /// found; falls back to T3 when nothing is.
    Grounded { corpus: Corpus, lead: Option<Lead> },
    /// T3: the model, optionally led by a line and told one more thing.
    Model {
        lead: Option<Lead>,
        note: Option<&'static str>,
    },
    /// T4 CLI: ask the CLI seam for a command in `group` (and in `also`,
    /// the other likely groups, when the seam keeps a beam); falls back to
    /// T3.
    Cli {
        group: String,
        also: Vec<String>,
        lead: Option<Lead>,
    },
    /// T0: a bank refusal; never model text.
    Refuse { answer: Entry, text: String },
    /// A Gym or eval route: read the Gym's verified records, then
    /// [`super::gym::reply`] decides; falls back to T3 told it has no
    /// records.
    Gym {
        route: RouteId,
        /// The tool the `tool` reading named at [`TOOL_CONFIDENCE`].
        tool: Option<String>,
        lead: Option<Lead>,
    },
    /// `eval.author`: one step of the authoring interview through the
    /// author seam; falls back to the bank's `eval.author.soon`. The model
    /// call is dropped: the interview's words come from the seam, checked
    /// ([`super::gym::check_step`]), or from the bank.
    Author,
    /// T0 (#9960): a request that calls for a capability none of the
    /// admitted ones covers. The bank's `capability.missing` line (or
    /// `capability.missing_near`, naming `closest`), with the
    /// `capability` card beside it; the worker adds how to add one and
    /// the Gym offer. The model call is cancelled.
    Capability {
        answer: Entry,
        text: String,
        /// The closest admitted capability, when the reading put one at
        /// [`CAPABILITY_CLOSEST`] or more.
        closest: Option<Capability>,
    },
}

impl Tier {
    /// The word the wire carries in `tier`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Tier::CannedFinal { answer, .. } | Tier::CannedStem { answer, .. }
                if dispatches(answer) =>
            {
                "offer"
            }
            Tier::CannedFinal { .. } | Tier::Capability { .. } => "canned",
            Tier::CannedStem { .. } => "stem",
            Tier::Grounded { .. } => "grounded",
            Tier::Model { lead: Some(_), .. } => "opener",
            Tier::Model { lead: None, .. } => "model",
            Tier::Cli { .. } => "cli",
            Tier::Refuse { .. } => "refuse",
            Tier::Gym { .. } => "gym",
            Tier::Author => "author",
        }
    }

    /// The design's tier number, 0 to 4.
    #[must_use]
    pub fn number(&self) -> u8 {
        match self {
            Tier::CannedFinal { answer, .. } | Tier::CannedStem { answer, .. }
                if dispatches(answer) =>
            {
                4
            }
            Tier::CannedFinal { .. } | Tier::Refuse { .. } | Tier::Capability { .. } => 0,
            Tier::CannedStem { .. } => 1,
            Tier::Grounded { .. } => 2,
            Tier::Model { .. } => 3,
            Tier::Gym {
                route: RouteId::GymNews,
                ..
            } => 2,
            Tier::Cli { .. } | Tier::Gym { .. } | Tier::Author => 4,
        }
    }

    /// Whether this tier keeps the speculative model call running.
    #[must_use]
    pub fn keeps_model(&self) -> bool {
        matches!(
            self,
            Tier::Model { .. } | Tier::Grounded { .. } | Tier::Cli { .. } | Tier::Gym { .. }
        )
    }

    /// The bank entry that supplied the text, when one did.
    #[must_use]
    pub fn answer(&self) -> Option<&Entry> {
        match self {
            Tier::CannedFinal { answer, .. }
            | Tier::CannedStem { answer, .. }
            | Tier::Refuse { answer, .. }
            | Tier::Capability { answer, .. } => Some(answer),
            _ => None,
        }
    }
}

/// The admitted capability a dispatch offer names: one the `capability`
/// reading found at [`CAPABILITY_CONFIDENCE`] that is usable only in a
/// Coder run (a catalog tool or an adoption, never Coder itself).
fn named_capability(routing: &Routing) -> Option<&Capability> {
    routing
        .capability
        .as_ref()
        .filter(|(entry, p)| {
            super::decisions::test(
                "capability",
                "CAPABILITY_CONFIDENCE",
                *p,
                CAPABILITY_CONFIDENCE,
                "ge",
            ) && entry.reach == Reach::Coder
        })
        .map(|(entry, _)| entry)
}

fn dispatches(entry: &Entry) -> bool {
    entry.answers(RouteId::WorkDispatch) || entry.answers(RouteId::Cli)
}

/// The whole answer code picks by `id`, in the chat's place: on a
/// computer, its `.here` variant when it has one ([`Bank::placed`]).
fn final_of(bank: &Bank, facts: &Facts, id: &str) -> Option<Tier> {
    let entry = bank.placed(id, facts)?;
    Some(Tier::CannedFinal {
        text: entry.render(facts)?,
        offer: entry.offer(),
        answer: entry.clone(),
    })
}

fn stem_of(entry: &Entry, facts: &Facts, personalize: bool) -> Option<Tier> {
    let (stem, generic_end) = entry.stem(facts)?;
    Some(Tier::CannedStem {
        answer: entry.clone(),
        stem,
        generic_end,
        offer: entry.offer(),
        personalize,
    })
}

fn refuse_of(bank: &Bank, facts: &Facts, id: &str) -> Option<Tier> {
    let entry = bank.entry(id)?;
    Some(Tier::Refuse {
        text: entry.render(facts)?,
        answer: entry.clone(),
    })
}

fn opener_lead(routing: &Routing) -> Option<Lead> {
    routing
        .opener
        .as_ref()
        .filter(|(_, p)| {
            super::decisions::test("opener", "OPENER_CONFIDENCE", *p, OPENER_CONFIDENCE, "ge")
        })
        .map(|(opener, _)| Lead {
            id: opener.id.clone(),
            text: opener.text.clone(),
        })
}

/// T3: the model, led by the opener; told it has no Gym records when the
/// route or the runner-up is a Gym or eval route, so it states no result.
fn model(routing: &Routing) -> Tier {
    let near = |is: fn(RouteId) -> bool| {
        is(routing.route)
            || routing.runner_up.is_some_and(|(route, p)| {
                is(route)
                    && super::decisions::test(
                        "route",
                        "CLOSE_MARGIN",
                        routing.route_p - p,
                        CLOSE_MARGIN,
                        "lt",
                    )
            })
    };
    let note = if near(RouteId::is_gym) {
        Some(super::gym::NO_RECORDS_NOTE)
    } else if near(|route| route == RouteId::Wallet) {
        Some(WALLET_NOTE)
    } else {
        None
    };
    Tier::Model {
        lead: opener_lead(routing),
        note,
    }
}

/// The note the model gets on the wallet route (#10170): the wallet is
/// always the user's own OpenAgents wallet. An instruction, never a
/// router: it reads nothing.
pub const WALLET_NOTE: &str = "A wallet in this chat is always the user's built-in OpenAgents \
wallet, which holds bitcoin over Lightning and on-chain. Never ask which wallet, app, provider, or \
exchange they mean, and never name another wallet, app, or exchange. In a terminal on their \
computer, `openagents wallet balance` says their balance and `openagents wallet address` gives an \
address to get paid at; in the OpenAgents app, the Wallet screen shows both. A message that only \
mentions their balance, sats, or bitcoin asks for their balance: answer that, without asking what \
they want. Say amounts in plain words and never mention nodes, networks, servers, channels, or \
liquidity. We never move money from this chat: sending happens in the wallet, where the user \
confirms it.";

/// Rule 3a: a wallet request in a terminal descends the `wallet` command
/// group: the typed `route` reading is sure the message is about the
/// built-in wallet, whose commands are that group, unless the `cli_group`
/// reading is sure of another group. Nothing reads words in the message;
/// the descent's own `none` leaves a question no command answers to the
/// model, told [`WALLET_NOTE`].
fn terminal_wallet(routing: &Routing, situation: &Situation) -> Option<Tier> {
    let elsewhere = routing.cli_group.as_ref().is_some_and(|(group, p)| {
        group != WALLET_GROUP
            && super::decisions::test("cli_group", "CLI_GROUP", *p, CLI_GROUP, "ge")
    });
    (situation.context.surface() == Surface::Terminal
        && routing.route == RouteId::Wallet
        && super::decisions::test("route", "CLI_ROUTE", routing.route_p, CLI_ROUTE, "ge")
        && !elsewhere)
        .then(|| Tier::Cli {
            group: WALLET_GROUP.to_owned(),
            also: Vec::new(),
            lead: opener_lead(routing),
        })
}

/// The built-in wallet's read-only balance, `openagents wallet balance`,
/// one plain sentence: what a sure wallet request in a terminal runs when
/// the descent picks no command, so the chat checks the wallet instead of
/// asking what the user wants (#10170).
#[must_use]
pub fn wallet_overview() -> super::seams::CliProposal {
    super::seams::CliProposal {
        argv: vec![WALLET_GROUP.to_owned(), "balance".to_owned()],
        effect: super::Effect::ReadOnly,
        runs_on: super::RunsOn::ThisDevice,
    }
}

/// The command group of the built-in wallet.
pub const WALLET_GROUP: &str = "wallet";

/// Rule 8b: the Gym and eval routes.
fn gym(routing: &Routing, bank: &Bank, facts: &Facts) -> Option<Tier> {
    let floor = if routing.route == RouteId::GymNews {
        GROUNDED_ROUTE
    } else {
        EVAL_ROUTE
    };
    if !routing.route.is_gym()
        || super::decisions::test("route", "gym.floor", routing.route_p, floor, "lt")
    {
        return None;
    }
    match routing.route {
        RouteId::EvalAuthor => Some(Tier::Author),
        // Credit answers are general ("how XP from tests works", "where
        // yours shows"), so a sure `answer` reading of one serves it even
        // when the message names specifics; otherwise where credit shows.
        RouteId::EvalCredit => {
            let id = routing
                .answer
                .as_ref()
                .filter(|(entry, p)| {
                    entry.answers(RouteId::EvalCredit)
                        && super::decisions::test(
                            "answer",
                            "STEM_CONFIDENCE",
                            *p,
                            STEM_CONFIDENCE,
                            "ge",
                        )
                })
                .map_or("eval.credit.mine", |(entry, _)| entry.id.as_str());
            final_of(bank, facts, id)
        }
        route => Some(Tier::Gym {
            route,
            tool: routing
                .tool
                .as_ref()
                .filter(|(_, p)| {
                    super::decisions::test("tool", "TOOL_CONFIDENCE", *p, TOOL_CONFIDENCE, "ge")
                })
                .map(|(tool, _)| tool.clone()),
            lead: opener_lead(routing),
        }),
    }
}

/// Rule 3: a whole prepared answer, if the judgment is sure of one.
fn canned(routing: &Routing, facts: &Facts, offers: bool) -> Option<Tier> {
    let (entry, p) = routing.answer.as_ref()?;
    if super::decisions::test(
        "route",
        "ROUTE_CONFIDENCE",
        routing.route_p,
        ROUTE_CONFIDENCE,
        "lt",
    ) || {
        let (name, at) = answer_confidence(routing);
        super::decisions::test("answer", name, *p, at, "lt")
    } || super::decisions::test(
        "needs_specifics",
        "SPECIFICS_CEILING",
        routing.needs_specifics,
        SPECIFICS_CEILING,
        "ge",
    ) || !entry.answers(routing.route)
        || dispatches(entry)
        || entry.answers(RouteId::Refuse)
        || (!offers && entry.offer.is_some())
    {
        return None;
    }
    Some(Tier::CannedFinal {
        text: entry.render(facts)?,
        offer: entry.offer(),
        answer: entry.clone(),
    })
}

/// Rule 6: the dispatch offer, or `None` when no dispatch entry renders.
/// A capability the reading named that is usable only in a Coder run
/// is named in the stem ([`named_capability`]).
fn dispatch(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Option<Tier> {
    if situation.context.computer_ready == Some(false) {
        return final_of(bank, facts, "dispatch.no_computer");
    }
    if let Some(plan) = fan_out(routing, situation.context)
        && let Some(tier) = fan_out_tier(bank, facts, plan)
    {
        return Some(tier);
    }
    let engine = requested_engine(routing);
    let mut tier = dispatch_stem(routing, bank, facts, situation, engine)?;
    if let Tier::CannedStem {
        offer: Some(Offer::RunCoder { engine: named, .. }),
        ..
    } = &mut tier
    {
        *named = engine;
    }
    Some(tier)
}

/// The dispatch plan for several runs (#10183): one run on each engine the
/// `fanout` reading asks for at [`FANOUT_CONFIDENCE`], read-only at
/// [`READ_ONLY_CONFIDENCE`], with a combined summary at
/// [`SUMMARIZE_CONFIDENCE`]. Only a terminal on the computer starts
/// several runs (the `openagents chat` client), so any other surface gets
/// one run, as before. "Each engine" is each the computer's context names
/// ready, in its order; with no engines named, Codex, Claude Code, and
/// Grok Build. `None` when fewer than two engines remain.
#[must_use]
pub fn fan_out(routing: &Routing, context: &super::Context) -> Option<super::DispatchPlan> {
    use super::judge::Fanout;
    use super::{CodingEngine as Engine, Computer, EngineState};
    if context.surface() != Surface::Terminal {
        return None;
    }
    let (fanout, _) = routing.fanout.filter(|(_, p)| {
        super::decisions::test("fanout", "FANOUT_CONFIDENCE", *p, FANOUT_CONFIDENCE, "ge")
    })?;
    let runs: Vec<Engine> = match fanout {
        Fanout::EachEngine => {
            let listed = match &context.computer {
                Some(Computer::Here { engines, .. } | Computer::Paired { engines, .. }) => {
                    engines.as_slice()
                }
                None => &[],
            };
            if listed.is_empty() {
                vec![Engine::Codex, Engine::ClaudeCode, Engine::GrokBuild]
            } else {
                let mut ready: Vec<Engine> = listed
                    .iter()
                    .filter(|engine| engine.state == EngineState::Ready)
                    .filter_map(|engine| super::coding_engine(&engine.engine))
                    .collect();
                ready.dedup();
                ready
            }
        }
        Fanout::Pair(first, second) => vec![first, second],
    };
    let plan = super::DispatchPlan {
        runs,
        read_only: super::decisions::test(
            "read_only",
            "READ_ONLY_CONFIDENCE",
            routing.read_only,
            READ_ONLY_CONFIDENCE,
            "ge",
        ),
        summarize: super::decisions::test(
            "summarize",
            "SUMMARIZE_CONFIDENCE",
            routing.summarize,
            SUMMARIZE_CONFIDENCE,
            "ge",
        ),
    };
    (plan.runs.len() >= 2 && plan.valid()).then_some(plan)
}

/// The reply for a plan: the bank's `dispatch.fan_out`, saying plainly
/// what starts, verb first ("Exploring the repo with Codex, Claude Code,
/// and Grok Build." for a read-only plan, #10212), with the `run_coder`
/// offer carrying the plan. No continuation: a continuation could restate
/// the request as its topic.
fn fan_out_tier(bank: &Bank, facts: &Facts, plan: super::DispatchPlan) -> Option<Tier> {
    let doing = if plan.read_only {
        "Exploring the repo"
    } else {
        "Working on this"
    };
    let engines: Vec<String> = plan
        .runs
        .iter()
        .map(|engine| engine.name().to_string())
        .collect();
    let facts = facts
        .clone()
        .set("fanout.doing", doing)
        .set("fanout.engines", super::series(&engines));
    let mut tier = final_of(bank, &facts, "dispatch.fan_out")?;
    if let Tier::CannedFinal {
        offer: Some(Offer::RunCoder { plan: planned, .. }),
        ..
    } = &mut tier
    {
        *planned = plan;
    } else {
        return None;
    }
    Some(tier)
}

/// The engine a dispatch offer names: the `engine` reading at
/// [`ENGINE_CONFIDENCE`], else none (#10076).
#[must_use]
pub fn requested_engine(routing: &Routing) -> Option<super::CodingEngine> {
    routing
        .engine
        .filter(|(_, p)| {
            super::decisions::test("engine", "ENGINE_CONFIDENCE", *p, ENGINE_CONFIDENCE, "ge")
        })
        .map(|(engine, _)| engine)
}

/// The dispatch stem: one naming a capability the reading found, else
/// one naming the engine the person asked for, else the chosen or plain
/// stem.
fn dispatch_stem(
    routing: &Routing,
    bank: &Bank,
    facts: &Facts,
    situation: &Situation,
    engine: Option<super::CodingEngine>,
) -> Option<Tier> {
    if let Some(capability) = named_capability(routing)
        && let Some(entry) = bank.entry("dispatch.capability_stem")
        && let Some(tier) = stem_of(
            entry,
            &facts
                .clone()
                .set("capability.name", capability.name.clone()),
            situation.personalize,
        )
    {
        return Some(tier);
    }
    // The engine stem closes with its generic end: a continuation would
    // echo the engine's name back as the task ("to claude to do ...").
    if let Some(engine) = engine
        && let Some(entry) = bank.entry("dispatch.engine_stem")
        && let Some(tier) = stem_of(
            entry,
            &facts.clone().set("engine.name", engine.name()),
            false,
        )
    {
        return Some(tier);
    }
    let chosen = routing
        .answer
        .as_ref()
        .map(|(entry, _)| entry)
        .filter(|entry| entry.answers(RouteId::WorkDispatch) && entry.stem.is_some());
    let entry = chosen.or_else(|| bank.entry("dispatch.stem"))?;
    stem_of(entry, facts, situation.personalize)
}

/// Rule 9b: a deck. Off the desktop, where no slide viewer is, the line
/// that says where decks open; on it, the deck the `deck` reading named
/// with an `open_presentation` offer, or a plain refusal that lists the
/// decks there are.
fn presentation(
    routing: &Routing,
    bank: &Bank,
    facts: &Facts,
    situation: &Situation,
) -> Option<Tier> {
    if routing.route != RouteId::PresentationOpen
        || super::decisions::test(
            "route",
            "PRESENTATION_ROUTE",
            routing.route_p,
            PRESENTATION_ROUTE,
            "lt",
        )
    {
        return None;
    }
    if situation.context.surface() != Surface::Desktop {
        return final_of(bank, facts, "presentation.elsewhere");
    }
    let decks = super::decks();
    let named = routing
        .deck
        .as_ref()
        .filter(|(_, p)| {
            super::decisions::test("deck", "DECK_CONFIDENCE", *p, DECK_CONFIDENCE, "ge")
        })
        .and_then(|(id, _)| decks.iter().find(|deck| deck.id == id));
    let Some(deck) = named else {
        let titles: Vec<&str> = decks.iter().map(|deck| deck.title.as_str()).collect();
        let list = match titles.as_slice() {
            [] => return None,
            [one] => (*one).to_string(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        return final_of(
            bank,
            &facts.clone().set("deck.list", list),
            "presentation.unknown",
        );
    };
    let entry = bank.entry("presentation.open")?;
    let label = format!("Open {}", deck.title);
    let label = if label.chars().count() <= nostr::cj_conversation::MAX_LABEL_CHARS {
        label
    } else {
        "Open the deck".to_string()
    };
    Some(Tier::CannedFinal {
        text: entry.render(&facts.clone().set("deck.title", deck.title.clone()))?,
        offer: Some(Offer::OpenPresentation {
            deck: deck.id.to_string(),
            label,
        }),
        answer: entry.clone(),
    })
}

/// Rule 2b: a standing rule. In a terminal, `standing.rule`, which the
/// terminal's computer follows with the compiled rule; on every other
/// surface, `standing.elsewhere`.
fn standing(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Option<Tier> {
    if routing.route != RouteId::StandingRule
        || super::decisions::test(
            "route",
            "STANDING_ROUTE",
            routing.route_p,
            STANDING_ROUTE,
            "lt",
        )
    {
        return None;
    }
    let id = if situation.context.surface() == Surface::Terminal {
        "standing.rule"
    } else {
        "standing.elsewhere"
    };
    final_of(bank, facts, id)
}

/// Rule 10: the missing-capability line, when both readings agree that
/// the request calls for a capability none of the admitted ones covers.
fn missing(routing: &Routing, bank: &Bank, facts: &Facts) -> Option<Tier> {
    if routing.route != RouteId::CapabilityMissing
        || super::decisions::test(
            "route",
            "CAPABILITY_ROUTE",
            routing.route_p,
            CAPABILITY_ROUTE,
            "lt",
        )
        || routing.capability.is_some()
        || super::decisions::test(
            "capability",
            "CAPABILITY_MISSING",
            routing.capability_missing_p,
            CAPABILITY_MISSING,
            "lt",
        )
    {
        return None;
    }
    let closest = routing
        .capability_closest
        .as_ref()
        .filter(|(_, p)| {
            super::decisions::test(
                "capability",
                "CAPABILITY_CLOSEST",
                *p,
                CAPABILITY_CLOSEST,
                "ge",
            )
        })
        .map(|(entry, _)| entry.clone());
    let (id, facts) = match &closest {
        Some(entry) => (
            "capability.missing_near",
            facts
                .clone()
                .set("capability.name", entry.name.clone())
                .set("capability.line", entry.line.clone()),
        ),
        None => ("capability.missing", facts.clone()),
    };
    let entry = bank.entry(id)?;
    Some(Tier::Capability {
        text: entry.render(&facts)?,
        answer: entry.clone(),
        closest,
    })
}

/// What the model is told on the website when the turn asked for
/// something only the OpenAgents app does (#10106).
pub const WEB_NOTE: &str = "This chat is on the openagents.com website, which only answers \
questions about OpenAgents. The visitor asked for something the website cannot do (work on \
code or a computer, a command, a screen, the wallet, or the Gym). Say in one or two sentences \
that Coder, our coding agent, does work on code in their terminal on their own computer, and \
give the one command that installs it, in a code span: \
`curl -fsSL https://openagents.com/cli/install.sh | bash` on macOS or Linux, or \
`irm https://openagents.com/cli/install.ps1 | iex` in PowerShell on Windows (every download is \
at https://openagents.com/download). When it helps, add that `coder login`, approving the code \
at https://openagents.com/device, and `/sync on` in Coder connect that computer to their \
account. For the wallet, the Gym, or a screen, say our OpenAgents apps have it. Answer any \
question in the message about OpenAgents itself.";

/// The bank entry the website answers `eval.run` with: which plugins there
/// are, as cards (`docs/web/plugin-card.md`), since nothing on the website
/// can start a test.
pub const WEB_PLUGINS: &str = "plugins.web";

/// The tier for `routing`. See the module documentation for the rules.
/// On the website ([`Surface::Web`]) it is then held to [`for_web`], except
/// that a wish to try or test a plugin (the typed `eval.run` route, never
/// the message's words) gets [`WEB_PLUGINS`] with the built-in plugins' cards
/// instead of a model reply that could name plugins we don't have.
#[must_use]
pub fn decide(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Tier {
    let tier = decide_anywhere(routing, bank, facts, situation);
    if situation.context.surface() == Surface::Web {
        if routing.route == RouteId::EvalRun
            && !matches!(tier, Tier::Refuse { .. })
            && let Some(plugins) = final_of(bank, facts, WEB_PLUGINS)
        {
            return plugins;
        }
        let held = for_web(routing, tier);
        if situation.context.repository.is_some() {
            in_project(routing, held)
        } else {
            held
        }
    } else {
        tier
    }
}

/// How sure the `repository` reading must be that a message asks about
/// the chat's repository before the model answers it from the repository
/// whatever the route read ([`in_project`]).
pub const REPOSITORY_READ_CONFIDENCE: f64 = 0.5;

/// What the model is told on the website when the chat is in a project
/// whose GitHub repository the website read for this turn
/// (`context.repository`): the repository itself is in its instructions
/// ([`super::Context::repository_note`]), so it answers from that.
pub const REPO_NOTE: &str = "This chat is on the openagents.com website, in the user's \
project, and the repository's contents read for this message are in these instructions. \
Answer the message from them: what the repository is and does, how it is laid out, its \
languages, key files, and recent changes. When the user asks for a change to its code \
(editing files, running commands or tests), answer what you can from what was read and say in \
one sentence that Coder, our coding agent, makes changes: `curl -fsSL \
https://openagents.com/cli/install.sh | bash` installs it. Never send them to install Coder \
only to read or explain this repository.";

/// The website's tier in a project chat with its repository read
/// (`context.repository`): a turn that would have told the visitor to
/// install Coder ([`WEB_NOTE`]), or that would have answered from the
/// OpenAgents codebase (the typed `codebase.kb` route), is answered by the
/// model from the repository instead ([`REPO_NOTE`]). Every other tier
/// stands: a prepared answer, a product note, a GitHub command card, a
/// refusal.
#[must_use]
pub fn in_project(routing: &Routing, tier: Tier) -> Tier {
    let repo = || Tier::Model {
        lead: None,
        note: Some(REPO_NOTE),
    };
    // The `repository` reading (asked only in a project chat) says the
    // message asks about the repository, which reading it answers: the
    // model answers from it, whatever the route read, unless refused.
    if let Some((RepositoryAsk::Read, p)) = routing.repository
        && p >= REPOSITORY_READ_CONFIDENCE
        && !matches!(tier, Tier::Refuse { .. })
    {
        return repo();
    }
    match tier {
        Tier::Model {
            note: Some(note), ..
        } if note == WEB_NOTE => repo(),
        Tier::Grounded {
            corpus: Corpus::Codebase,
            ..
        } if routing.route == RouteId::CodebaseKb => repo(),
        tier => tier,
    }
}

/// The command groups the website's chat proposes from (#11167): the
/// groups of `github_actions::COMMANDS`, whose changes the website makes
/// itself ([`crate::cli_route::gate::WEB_COMMANDS`]).
pub const WEB_GROUPS: [&str; 3] = ["issue", "project", "pr"];

/// The website's tiers (#10106): refusals, knowledge, the model, and
/// prepared answers on the answer routes, each without an offer, and the
/// GitHub commands of [`WEB_GROUPS`]. Work, other commands, screens, the
/// Gym, decks, and capabilities become the model
/// told [`WEB_NOTE`], since nothing on the website can act on them. An
/// account question reads the product knowledge: the website has
/// accounts (GitHub sign-in, Settings, the Claude key, Coder's sign-in),
/// so "the app does that" would be wrong there.
#[must_use]
pub fn for_web(routing: &Routing, tier: Tier) -> Tier {
    let answers = matches!(
        routing.route.family(),
        Some(RouteFamily::Answers | RouteFamily::Boundaries) | None
    ) && routing.route != RouteId::CapabilityMissing;
    let web_model = || Tier::Model {
        lead: None,
        note: Some(WEB_NOTE),
    };
    match tier {
        Tier::Refuse { .. } => tier,
        // A GitHub change (#11167): the website makes it itself after a
        // signed confirm card, so the command route stays, held to the
        // GitHub groups (the typed `cli_group` reading, never the words).
        Tier::Cli { group, also, lead } if WEB_GROUPS.contains(&group.as_str()) => Tier::Cli {
            group,
            also: also
                .into_iter()
                .filter(|group| WEB_GROUPS.contains(&group.as_str()))
                .collect(),
            lead,
        },
        _ if routing.route == RouteId::Account => Tier::Grounded {
            corpus: Corpus::Product,
            lead: None,
        },
        _ if !answers => web_model(),
        Tier::Grounded { .. } | Tier::Model { .. } => tier,
        Tier::CannedFinal { answer, text, .. } if !dispatches(&answer) => Tier::CannedFinal {
            answer,
            text,
            offer: None,
        },
        Tier::CannedStem {
            answer,
            stem,
            generic_end,
            personalize,
            ..
        } if !dispatches(&answer) => Tier::CannedStem {
            answer,
            stem,
            generic_end,
            offer: None,
            personalize,
        },
        _ => web_model(),
    }
}

fn decide_anywhere(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Tier {
    if situation.mode == Mode::Legacy {
        return canned(routing, facts, false).unwrap_or_else(|| model(routing));
    }

    // 1. Risk.
    let risky = matches!(
        routing.risk,
        Risk::SecretShared | Risk::AsksForSecret | Risk::Harmful
    );
    // Two independent readings agreeing, the `refuse` route and a risk in
    // the warn band, are as sure as one risk reading at the refusal bar.
    let agreed = routing.route == RouteId::Refuse
        && super::decisions::test(
            "route",
            "ROUTE_CONFIDENCE",
            routing.route_p,
            ROUTE_CONFIDENCE,
            "ge",
        );
    if risky
        && (super::decisions::test("risk", "RISK_REFUSE", routing.risk_p, RISK_REFUSE, "ge")
            || (agreed
                && super::decisions::test("risk", "RISK_WARN", routing.risk_p, RISK_WARN, "ge")))
    {
        let id = match routing.risk {
            Risk::SecretShared => "refuse.secret_shared",
            Risk::AsksForSecret => "refuse.asks_for_secret",
            _ => "refuse.harmful",
        };
        if let Some(tier) = refuse_of(bank, facts, id) {
            return tier;
        }
    }
    // 0. An open interview continues, unless the reading is sure of a
    // route that is no part of it. Refusals above still come first.
    if situation.draft
        && (AUTHOR_CONTINUES.contains(&routing.route)
            || super::decisions::test(
                "route",
                "ROUTE_CONFIDENCE",
                routing.route_p,
                ROUTE_CONFIDENCE,
                "lt",
            ))
        && !(risky && super::decisions::test("risk", "RISK_WARN", routing.risk_p, RISK_WARN, "ge"))
    {
        return Tier::Author;
    }
    // 0b. A plugin being made on this computer continues the same way.
    if situation.plugin
        && (AUTHOR_CONTINUES.contains(&routing.route)
            || PLUGIN_CONTINUES.contains(&routing.route)
            || super::decisions::test(
                "route",
                "ROUTE_CONFIDENCE",
                routing.route_p,
                ROUTE_CONFIDENCE,
                "lt",
            ))
        && !(risky && super::decisions::test("risk", "RISK_WARN", routing.risk_p, RISK_WARN, "ge"))
    {
        return Tier::Author;
    }
    if risky && super::decisions::test("risk", "RISK_WARN", routing.risk_p, RISK_WARN, "ge") {
        let lead = (routing.risk == Risk::SecretShared)
            .then(|| bank.entry("warn.secret_shared"))
            .flatten()
            .and_then(|entry| {
                Some(Lead {
                    id: entry.id.clone(),
                    text: entry.render(facts)?,
                })
            });
        return Tier::Model { lead, note: None };
    }
    if routing.risk == Risk::MoneyMovement
        && super::decisions::test("risk", "RISK_WARN", routing.risk_p, RISK_WARN, "ge")
        && let Some(tier) = final_of(bank, facts, "wallet.send")
    {
        return tier;
    }

    // 2. Close call.
    if let Some((second, second_p)) = routing.runner_up
        && routing.route != RouteId::Unknown
        && super::decisions::test(
            "route",
            "CLOSE_MARGIN",
            routing.route_p - second_p,
            CLOSE_MARGIN,
            "lt",
        )
    {
        // A question about us is answered from what we documented, not
        // asked back or left to the model (#10137).
        if let Some(tier) = knowledge(routing) {
            return tier;
        }
        if super::decisions::test(
            "route",
            "CLARIFY_WINS",
            routing.clarify_p,
            CLARIFY_WINS,
            "ge",
        ) {
            return clarify(situation);
        }
        // An offer loses to an answer: work and a route with its own answer
        // this close is not a dispatch.
        let other = if routing.route == RouteId::WorkDispatch {
            second
        } else {
            routing.route
        };
        let dispatch_pair = (routing.route == RouteId::WorkDispatch
            || second == RouteId::WorkDispatch)
            && LANE_ROUTES.contains(&other);
        if dispatch_pair
            && routing.lane == Lane::Computer
            && super::decisions::test("lane", "DISPATCH_LANE", routing.lane_p, DISPATCH_LANE, "ge")
            && let Some(tier) = dispatch(routing, bank, facts, situation)
        {
            return tier;
        }
        return answered(routing);
    }

    // 2b. A standing rule (#10157): something to keep happening on the
    // user's computer. A terminal compiles it on the computer beside this
    // line; elsewhere the line says where rules are made.
    if let Some(tier) = standing(routing, bank, facts, situation) {
        return tier;
    }

    // 3. T0.
    if let Some(tier) = canned(routing, facts, true) {
        return tier;
    }

    // 3a. The wallet in a terminal (#10170): a request about the built-in
    // wallet that no prepared answer serves descends the wallet's
    // commands, so a balance request runs the read-only command on this
    // computer and shows what it printed, before the model could ask
    // which wallet.
    if let Some(tier) = terminal_wallet(routing, situation) {
        return tier;
    }

    // 4. End.
    if routing.route == RouteId::End
        && super::decisions::test(
            "route",
            "ROUTE_CONFIDENCE",
            routing.route_p,
            ROUTE_CONFIDENCE,
            "ge",
        )
        && let Some(tier) = final_of(bank, facts, "smalltalk.bye")
    {
        return tier;
    }

    // 5. T1.
    if let Some((entry, p)) = &routing.answer
        && super::decisions::test("answer", "STEM_CONFIDENCE", *p, STEM_CONFIDENCE, "ge")
        && super::decisions::test(
            "needs_specifics",
            "SPECIFICS_CEILING",
            routing.needs_specifics,
            SPECIFICS_CEILING,
            "ge",
        )
        && entry.answers(routing.route)
        && !dispatches(entry)
        && let Some(tier) = stem_of(entry, facts, situation.personalize)
    {
        return tier;
    }

    // 6. T4 dispatch, by route.
    if routing.route == RouteId::WorkDispatch
        && super::decisions::test(
            "route",
            "DISPATCH_ROUTE",
            routing.route_p,
            DISPATCH_ROUTE,
            "ge",
        )
        && let Some(tier) = dispatch(routing, bank, facts, situation)
    {
        return tier;
    }

    // 7. T4 CLI: the route is sure enough, and so is a group, or the
    // route is sure and the seam descends the likely groups as a beam.
    if routing.route == RouteId::Cli
        && let Some((group, group_p)) = &routing.cli_group
        && ((super::decisions::test("route", "CLI_ROUTE", routing.route_p, CLI_ROUTE, "ge")
            && super::decisions::test("cli_group", "CLI_GROUP", *group_p, CLI_GROUP, "ge"))
            || (super::decisions::test(
                "route",
                "CLI_ROUTE_SURE",
                routing.route_p,
                CLI_ROUTE_SURE,
                "ge",
            ) && super::decisions::test(
                "cli_group",
                "CLI_GROUP_BEAM",
                *group_p,
                CLI_GROUP_BEAM,
                "ge",
            ))
            || (super::decisions::test(
                "route",
                "GROUNDED_ROUTE",
                routing.route_p,
                GROUNDED_ROUTE,
                "ge",
            ) && super::decisions::test(
                "cli_group",
                "CLI_GROUP_SURE",
                *group_p,
                CLI_GROUP_SURE,
                "ge",
            )))
    {
        let also = if super::decisions::test(
            "cli_group",
            "CLI_GROUP_SURE",
            *group_p,
            CLI_GROUP_SURE,
            "ge",
        ) {
            Vec::new()
        } else {
            routing
                .cli_alternatives
                .iter()
                .map(|(group, _)| group.clone())
                .collect()
        };
        return Tier::Cli {
            group: group.clone(),
            also,
            lead: opener_lead(routing),
        };
    }

    // 8. T2.
    if super::decisions::test(
        "route",
        "GROUNDED_ROUTE",
        routing.route_p,
        GROUNDED_ROUTE,
        "ge",
    ) && let Some(corpus) = corpus_of(routing.route)
    {
        return Tier::Grounded {
            corpus,
            lead: opener_lead(routing),
        };
    }

    // 9. Gym and eval.
    if let Some(tier) = gym(routing, bank, facts) {
        return tier;
    }

    // 9b. A deck.
    if let Some(tier) = presentation(routing, bank, facts, situation) {
        return tier;
    }

    // 10. A missing capability, when the route and the capability reading
    // agree; or the admitted Coder-run capability the route missed.
    if let Some(tier) = missing(routing, bank, facts) {
        return tier;
    }
    if routing.route == RouteId::CapabilityMissing
        && super::decisions::test(
            "route",
            "CAPABILITY_ROUTE",
            routing.route_p,
            CAPABILITY_ROUTE,
            "ge",
        )
        && named_capability(routing).is_some()
        && routing.lane == Lane::Computer
        && super::decisions::test("lane", "DISPATCH_LANE", routing.lane_p, DISPATCH_LANE, "ge")
        && let Some(tier) = dispatch(routing, bank, facts, situation)
    {
        return tier;
    }

    // 11. T4 dispatch, by lane: work the route did not name, when the lane is sure it
    // needs a computer. It comes after the CLI and knowledge routes, which
    // read a computer question more precisely, and only when the route is
    // not one with its own answer: an offer loses to an answer.
    if LANE_ROUTES.contains(&routing.route)
        && routing.lane == Lane::Computer
        && super::decisions::test("lane", "DISPATCH_LANE", routing.lane_p, DISPATCH_LANE, "ge")
        && let Some(tier) = dispatch(routing, bank, facts, situation)
    {
        return tier;
    }

    // 12. Clarify.
    if routing.route == RouteId::Clarify
        && super::decisions::test(
            "route",
            "CLARIFY_ROUTE",
            routing.route_p,
            CLARIFY_ROUTE,
            "ge",
        )
    {
        return clarify_or_answer(routing, bank, facts, situation);
    }

    // 13. T3, grounded when the question is about us.
    answered(routing)
}

/// The corpus a knowledge route answers from: product documentation for
/// `product.kb` and for `meta` (our limits, pricing, privacy, and model,
/// once no prepared answer fits), the repository for `codebase.kb`.
fn corpus_of(route: RouteId) -> Option<Corpus> {
    match route {
        RouteId::ProductKb | RouteId::Meta => Some(Corpus::Product),
        RouteId::CodebaseKb => Some(Corpus::Codebase),
        _ => None,
    }
}

/// A grounded reply for a question about us that no surer rule served:
/// the argmax route is a knowledge route at any probability, or one is
/// the close runner-up of a route with no answer of its own (`general`,
/// `clarify`, `none`). A follow-up that leaves the prepared answers reads
/// the documentation rather than letting the model invent plans, limits,
/// accounts, or platforms (#10135, #10136, #10137).
fn knowledge(routing: &Routing) -> Option<Tier> {
    let corpus = corpus_of(routing.route).or_else(|| {
        let (second, second_p) = routing.runner_up?;
        (matches!(
            routing.route,
            RouteId::General | RouteId::Clarify | RouteId::Unknown
        ) && super::decisions::test(
            "route",
            "CLOSE_MARGIN",
            routing.route_p - second_p,
            CLOSE_MARGIN,
            "lt",
        ))
        .then(|| corpus_of(second))
        .flatten()
    })?;
    Some(Tier::Grounded {
        corpus,
        lead: opener_lead(routing),
    })
}

/// Rule 2's and rule 13's last answer: [`knowledge`], else the model.
fn answered(routing: &Routing) -> Tier {
    knowledge(routing).unwrap_or_else(|| model(routing))
}

/// Rule 12: a clarify loses to a sure prepared answer (#10138).
/// When the `answer` reading is sure of an entry that answers in the chat
/// (an [`RouteFamily::Answers`] route) and the reply needs none of the
/// user's particulars, the message is clear enough that the entry answers
/// it: "what is this?" on the website reads as asking who we are.
fn clarify_or_answer(
    routing: &Routing,
    _bank: &Bank,
    facts: &Facts,
    situation: &Situation,
) -> Tier {
    if let Some((entry, p)) = &routing.answer
        && {
            let (name, at) = answer_confidence(routing);
            super::decisions::test("answer", name, *p, at, "ge")
        }
        && super::decisions::test(
            "needs_specifics",
            "SPECIFICS_CEILING",
            routing.needs_specifics,
            SPECIFICS_CEILING,
            "lt",
        )
        && !dispatches(entry)
        && !entry.answers(RouteId::Refuse)
        && entry
            .routes
            .iter()
            .any(|word| RouteId::parse(word).family() == Some(RouteFamily::Answers))
        && let Some(text) = entry.render(facts)
    {
        return Tier::CannedFinal {
            text,
            offer: entry.offer(),
            answer: entry.clone(),
        };
    }
    clarify(situation)
}

/// Rules 2 and 12's question. On a later turn the model reads the earlier
/// messages first and asks only when they leave the latest one unclear
/// ([`LATER_CLARIFY_NOTE`], #10138); a first message gets the model told
/// [`CLARIFY_NOTE`], never a canned stem: a stem read stiffly ("To make
/// sure we get this right: this works.") on a plain yes-or-no question.
fn clarify(situation: &Situation) -> Tier {
    if situation.earlier {
        return Tier::Model {
            lead: None,
            note: Some(LATER_CLARIFY_NOTE),
        };
    }
    Tier::Model {
        lead: None,
        note: Some(CLARIFY_NOTE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{Screen, Surface};

    fn facts() -> Facts {
        crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
            &crate::router::Seams::default(),
        )
    }

    /// A routing: `route` at `route_p` (the rest on `general`), `answer` at
    /// `answer_p`, the specifics probability, and everything else quiet.
    fn routed(
        route: RouteId,
        route_p: f64,
        answer: &str,
        answer_p: f64,
        specifics: f64,
    ) -> Routing {
        let bank = Bank::builtin();
        Routing {
            action: crate::classify::Route::Respond,
            route,
            route_p,
            runner_up: Some((
                if route == RouteId::General {
                    RouteId::Meta
                } else {
                    RouteId::General
                },
                1.0 - route_p,
            )),
            clarify_p: 0.0,
            answer: bank.entry(answer).map(|entry| (entry.clone(), answer_p)),
            needs_specifics: specifics,
            lane: Lane::Chat,
            lane_p: 0.9,
            opener: None,
            cli_group: None,
            cli_alternatives: Vec::new(),
            tool: None,
            capability: None,
            capability_missing_p: 0.0,
            capability_closest: None,
            deck: None,
            repository: None,
            engine: None,
            fanout: None,
            read_only: 0.0,
            summarize: 0.0,
            risk: Risk::Ok,
            risk_p: 0.95,
            answer_calibrated: false,
        }
    }

    fn decided(routing: &Routing, context: &Context, personalize: bool) -> Tier {
        decide(
            routing,
            Bank::builtin(),
            &facts(),
            &Situation {
                mode: Mode::Router,
                context,
                personalize,
                draft: false,
                earlier: false,
                plugin: false,
            },
        )
    }

    fn router(routing: &Routing) -> Tier {
        decided(routing, &Context::default(), false)
    }

    fn web() -> Context {
        Context::of(&serde_json::json!({ "surface": "web" }))
    }

    /// #10170: a wallet request in a terminal descends the wallet's
    /// commands, so "balance and wallet address" runs `wallet balance`
    /// rather than the model asking which wallet; elsewhere the model is
    /// told the wallet is ours ([`WALLET_NOTE`]).
    #[test]
    fn a_wallet_request_in_a_terminal_descends_the_wallet_commands() {
        let terminal = Context::of(&serde_json::json!({ "surface": "terminal" }));
        // Thread 7cd10ec8…'s last turn, as the worker judged it.
        let mut wallet = routed(RouteId::Wallet, 0.91, "clarify.generic", 0.0, 0.88);
        wallet.lane = Lane::Computer;
        wallet.cli_group = Some(("wallet".into(), 0.89));
        assert_eq!(
            decided(&wallet, &terminal, false),
            Tier::Cli {
                group: "wallet".into(),
                also: Vec::new(),
                lead: None,
            }
        );
        // On the phone the wallet is a screen: the model, told the wallet
        // is the built-in one.
        assert_eq!(
            router(&wallet),
            Tier::Model {
                lead: None,
                note: Some(WALLET_NOTE),
            }
        );
        // A message that moves money is the wallet's own answer, never a
        // command, in a terminal too.
        let mut send = wallet.clone();
        send.risk = Risk::MoneyMovement;
        assert!(matches!(
            decided(&send, &terminal, false),
            Tier::CannedFinal { answer, .. } if answer.id == "wallet.send"
        ));
        // A group reading for another group is not the wallet's rule.
        let mut other = wallet.clone();
        other.cli_group = Some(("verse".into(), 0.9));
        assert!(!matches!(
            decided(&other, &terminal, false),
            Tier::Cli { group, .. } if group == "wallet"
        ));
        for word in ["MetaMask", "Phantom", "Coinbase"] {
            assert!(!WALLET_NOTE.contains(word));
        }
        assert!(WALLET_NOTE.contains("built-in OpenAgents wallet"));
        assert!(WALLET_NOTE.contains("Never ask which wallet"));
        // The note names the plain commands, never the x402 node's.
        assert!(WALLET_NOTE.contains("openagents wallet balance"));
        for word in [
            "wallet info",
            "wallet fund",
            "node id",
            "Esplora",
            "testnet",
        ] {
            assert!(!WALLET_NOTE.contains(word), "{word}");
        }
        // The overview is a read-only command of this build's tree.
        let overview = wallet_overview();
        assert_eq!(
            crate::cli_route::gate::effect_here(&overview.argv),
            Some(crate::cli_route::tree::Effect::ReadOnly)
        );
        assert_eq!(overview.effect, crate::router::Effect::ReadOnly);
    }

    /// On the website a GitHub command group keeps the command route
    /// (#11167): the website makes the change itself after a signed
    /// confirm card. Other groups in the beam are dropped there.
    #[test]
    fn the_website_proposes_github_changes_from_the_github_groups_only() {
        let mut cli = routed(RouteId::Cli, 0.95, "cli.offer", 0.9, 0.9);
        cli.cli_group = Some(("issue".to_string(), 0.95));
        cli.cli_alternatives = vec![("project".to_string(), 0.4), ("session".to_string(), 0.3)];
        match decided(&cli, &web(), false) {
            Tier::Cli { group, also, .. } => {
                assert_eq!(group, "issue");
                assert!(
                    also.iter().all(|g| WEB_GROUPS.contains(&g.as_str())),
                    "{also:?}"
                );
            }
            other => panic!("expected the command route, got {other:?}"),
        }
        for group in WEB_GROUPS {
            assert!(
                crate::cli_route::gate::WEB_COMMANDS
                    .iter()
                    .any(|command| command.split(' ').next() == Some(group)),
                "{group}"
            );
        }
    }

    fn in_a_project() -> Context {
        Context::of(&serde_json::json!({
            "surface": "web",
            "repository": {
                "name": "AtlantisPleb/finances",
                "branch": "main",
                "snapshot": "Repository: AtlantisPleb/finances\nREADME.md:\nPersonal finance scripts."
            }
        }))
    }

    /// A project chat on the website with its repository read: "Summarize
    /// this repo" (read as work, or as the OpenAgents codebase) is answered
    /// by the model from the repository, never with the install text;
    /// answers, product notes, and GitHub command cards stand.
    #[test]
    fn a_project_chat_answers_about_its_repository_from_it() {
        let project = in_a_project();
        assert!(project.repository.is_some());
        let note = project.repository_note().unwrap();
        assert!(note.contains("AtlantisPleb/finances"));
        assert!(note.contains("Personal finance scripts."));
        assert!(!REPO_NOTE.contains("coder login"));
        let repo = Tier::Model {
            lead: None,
            note: Some(REPO_NOTE),
        };
        let work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.9, 0.9);
        assert_eq!(decided(&work, &project, true), repo);
        let code = routed(RouteId::CodebaseKb, 0.9, "none", 0.0, 0.5);
        assert_eq!(decided(&code, &project, false), repo);
        // Without a repository the website still says what Coder does.
        assert_eq!(
            decided(&work, &web(), true),
            Tier::Model {
                lead: None,
                note: Some(WEB_NOTE)
            }
        );
        let kb = routed(RouteId::ProductKb, 0.95, "none", 0.0, 0.9);
        assert!(matches!(
            decided(&kb, &project, false),
            Tier::Grounded {
                corpus: Corpus::Product,
                ..
            }
        ));
        let mut cli = routed(RouteId::Cli, 0.95, "cli.offer", 0.9, 0.9);
        cli.cli_group = Some(("issue".to_string(), 0.95));
        assert!(matches!(decided(&cli, &project, false), Tier::Cli { .. }));
        // The `repository` reading "read" answers from the repository
        // whatever the route read (a prepared answer about connecting a
        // codebase, the product notes); unsure, or not asked, it doesn't.
        let mut meta = routed(RouteId::Meta, 0.4, "meta.codebase", 0.9, 0.1);
        let before = decided(&meta, &project, false);
        assert_ne!(before, repo);
        meta.repository = Some((RepositoryAsk::Read, 0.8));
        assert_eq!(decided(&meta, &project, false), repo);
        meta.repository = Some((RepositoryAsk::Read, 0.3));
        assert_eq!(decided(&meta, &project, false), before);
        let mut kb = routed(RouteId::ProductKb, 0.95, "none", 0.0, 0.9);
        kb.repository = Some((RepositoryAsk::Read, 0.9));
        assert_eq!(decided(&kb, &project, false), repo);
        kb.repository = Some((RepositoryAsk::Other, 0.9));
        assert!(matches!(
            decided(&kb, &project, false),
            Tier::Grounded { .. }
        ));
        // Off the website, or with no repository, the reading changes nothing.
        let mut away = routed(RouteId::ProductKb, 0.95, "none", 0.0, 0.9);
        away.repository = Some((RepositoryAsk::Read, 0.9));
        assert!(matches!(
            decided(&away, &web(), false),
            Tier::Grounded { .. }
        ));
    }

    /// Jev reads that the chat has a repository, by name, just before the
    /// latest message; never the repository's own text.
    #[test]
    fn jev_reads_the_project_repository_by_name_only() {
        let input = vec![crate::generate::Message {
            role: crate::generate::Role::User,
            text: "Summarize this repo.".into(),
        }];
        let judged = in_a_project().judged(&input);
        assert_eq!(judged.len(), 2);
        assert!(judged[0].text.contains("AtlantisPleb/finances"));
        assert!(!judged[0].text.contains("Personal finance scripts."));
        assert_eq!(judged[1].text, "Summarize this repo.");
        assert_eq!(web().judged(&input).len(), 1);
    }

    /// On the website (#10106) work, commands, and screens become the
    /// model told [`WEB_NOTE`]; answers, knowledge, and refusals stay,
    /// without an offer.
    #[test]
    fn the_website_answers_and_never_offers() {
        assert_eq!(web().surface(), Surface::Web);
        assert!(WEB_NOTE.contains("https://openagents.com/download"));
        assert!(WEB_NOTE.contains("https://openagents.com/cli/install.sh | bash"));
        assert!(!WEB_NOTE.contains("openagents.com/install"));
        let work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.9, 0.9);
        assert!(matches!(
            router(&work),
            Tier::CannedStem { offer: Some(_), .. }
        ));
        assert_eq!(
            decided(&work, &web(), true),
            Tier::Model {
                lead: None,
                note: Some(WEB_NOTE)
            }
        );
        let mut cli = routed(RouteId::Cli, 0.95, "cli.offer", 0.9, 0.9);
        cli.cli_group = Some(("session".to_string(), 0.95));
        assert!(matches!(router(&cli), Tier::Cli { .. }));
        assert!(matches!(
            decided(&cli, &web(), false),
            Tier::Model {
                note: Some(WEB_NOTE),
                ..
            }
        ));
        let wallet = routed(RouteId::Wallet, 0.95, "wallet.receive", 0.95, 0.1);
        assert!(matches!(
            decided(&wallet, &web(), false),
            Tier::Model {
                note: Some(WEB_NOTE),
                ..
            }
        ));
        // The website has accounts: an account question reads the product
        // notes, never "the app does that".
        let account = routed(RouteId::Account, 0.95, "account.computers", 0.9, 0.1);
        assert_eq!(
            decided(&account, &web(), false),
            Tier::Grounded {
                corpus: Corpus::Product,
                lead: None
            }
        );
        assert!(!WEB_NOTE.contains("an account"));
        let kb = routed(RouteId::ProductKb, 0.95, "none", 0.0, 0.9);
        assert!(matches!(
            decided(&kb, &web(), false),
            Tier::Grounded {
                corpus: Corpus::Product,
                ..
            }
        ));
        let map = routed(RouteId::Meta, 0.95, "meta.map", 0.95, 0.1);
        match decided(&map, &web(), false) {
            Tier::CannedFinal { answer, offer, .. } => {
                assert_eq!(answer.id, "meta.map");
                assert_eq!(offer, None);
            }
            other => panic!("{other:?}"),
        }
        let mut secret = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.9, 0.9);
        secret.risk = Risk::SecretShared;
        secret.risk_p = 0.99;
        assert!(matches!(
            decided(&secret, &web(), false),
            Tier::Refuse { .. }
        ));
    }

    /// On the website, a wish to try or test a plugin ("Which plugin should
    /// I try?", the `eval.run` route) is the prepared answer that comes
    /// with the catalog's cards, never the model naming plugins we don't
    /// have (docs/web/plugin-card.md). Elsewhere `eval.run` is unchanged.
    #[test]
    fn the_website_answers_a_plugin_question_with_the_cards() {
        let run = routed(RouteId::EvalRun, 0.9, "none", 0.0, 0.1);
        match decided(&run, &web(), false) {
            Tier::CannedFinal { answer, offer, .. } => {
                assert_eq!(answer.id, WEB_PLUGINS);
                assert!(answer.plugins);
                assert_eq!(offer, None);
            }
            other => panic!("{other:?}"),
        }
        assert!(!matches!(
            router(&run),
            Tier::CannedFinal { answer, .. } if answer.id == WEB_PLUGINS
        ));
        let mut secret = run.clone();
        secret.risk = Risk::SecretShared;
        secret.risk_p = 0.99;
        assert!(matches!(
            decided(&secret, &web(), false),
            Tier::Refuse { .. }
        ));
    }

    /// T0: a sure route and a sure answer of that route, with no
    /// specifics, is the whole reply, with its followups available.
    #[test]
    fn a_sure_meta_answer_is_canned_final() {
        let tier = router(&routed(RouteId::Meta, 0.93, "meta.model", 0.88, 0.07));
        let Tier::CannedFinal {
            answer,
            text,
            offer,
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "meta.model");
        assert!(text.starts_with("There isn't one model."));
        assert_eq!(offer, &None);
        assert_eq!((tier.word(), tier.number()), ("canned", 0));
        assert!(!tier.keeps_model());
        let chips = Bank::builtin().followups(answer, &facts());
        assert_eq!(chips[0], ("meta.jev".into(), "What is Jev?".into()));
    }

    /// Each T0 condition is necessary.
    #[test]
    fn t0_needs_route_answer_agreement_and_no_specifics() {
        for routing in [
            routed(RouteId::Meta, 0.79, "meta.model", 0.9, 0.0),
            routed(RouteId::Meta, 0.9, "meta.model", 0.79, 0.0),
            routed(RouteId::Meta, 0.9, "meta.model", 0.9, 0.3),
            routed(RouteId::Smalltalk, 0.9, "meta.model", 0.9, 0.0),
        ] {
            // A meta question no prepared answer serves is grounded
            // (#10135); never a whole answer.
            assert!(
                matches!(
                    router(&routing),
                    Tier::Model { .. }
                        | Tier::Grounded {
                            corpus: Corpus::Product,
                            ..
                        }
                ),
                "{routing:?} -> {:?}",
                router(&routing)
            );
        }
        let mut risky = routed(RouteId::Meta, 0.9, "meta.model", 0.9, 0.0);
        risky.risk = Risk::Harmful;
        risky.risk_p = 0.7;
        assert!(matches!(router(&risky), Tier::Model { lead: None, .. }));
    }

    /// Wallet and account answers carry their screen as an offer.
    #[test]
    fn how_to_answers_offer_their_screen() {
        let tier = router(&routed(
            RouteId::Account,
            0.9,
            "account.report_problem",
            0.9,
            0.1,
        ));
        let Tier::CannedFinal {
            offer: Some(Offer::OpenScreen { screen, label }),
            ..
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(*screen, Screen::AccountReportProblem);
        assert_eq!(label, "Report a problem");
        assert_eq!(tier.number(), 0);
    }

    /// Work on code gets a dispatch stem and a Run Coder offer, or, with no
    /// computer, the no-computer answer and the computers screen.
    #[test]
    fn work_is_offered_to_coder_never_attempted() {
        let routing = routed(
            RouteId::WorkDispatch,
            0.9,
            "dispatch.explore_stem",
            0.6,
            0.9,
        );
        let tier = decided(&routing, &Context::default(), true);
        let Tier::CannedStem {
            answer,
            stem,
            generic_end,
            offer,
            personalize,
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.explore_stem");
        assert_eq!(stem, "Looking through");
        assert!(generic_end.starts_with(' '));
        assert!(matches!(offer, Some(Offer::RunCoder { .. })));
        assert!(*personalize);
        assert_eq!((tier.word(), tier.number()), ("offer", 4));

        // A non-dispatch answer falls back to dispatch.stem.
        let routing = routed(RouteId::WorkDispatch, 0.9, "meta.coder", 0.6, 0.9);
        assert!(
            matches!(router(&routing), Tier::CannedStem { answer, .. } if answer.id == "dispatch.stem")
        );

        let none = Context {
            computer_ready: Some(false),
            ..Context::default()
        };
        let tier = decided(&routing, &none, true);
        let Tier::CannedFinal { answer, offer, .. } = &tier else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.no_computer");
        assert!(matches!(
            offer,
            Some(Offer::OpenScreen {
                screen: Screen::AccountComputers,
                ..
            })
        ));
        assert_eq!(tier.word(), "offer");

        // The lane alone is enough.
        let mut lane = routed(RouteId::General, 0.6, "none", 0.0, 0.9);
        lane.lane = Lane::Computer;
        lane.lane_p = 0.8;
        assert_eq!(router(&lane).word(), "offer");
    }

    /// A sure refusal risk is a bank refusal; the warn band gives the model
    /// the turn with no canned text or offer; money movement is the
    /// wallet how-to with no amount.
    #[test]
    fn risk_refuses_warns_or_redirects() {
        let mut secret = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.9, 0.9);
        secret.risk = Risk::SecretShared;
        secret.risk_p = 0.9;
        let tier = router(&secret);
        assert!(
            matches!(&tier, Tier::Refuse { answer, .. } if answer.id == "refuse.secret_shared")
        );
        assert_eq!((tier.word(), tier.number()), ("refuse", 0));

        secret.risk_p = 0.7;
        let mut agreed = secret.clone();
        agreed.route = RouteId::Refuse;
        assert!(matches!(router(&agreed), Tier::Refuse { .. }));
        let Tier::Model {
            lead: Some(lead), ..
        } = router(&secret)
        else {
            panic!("{:?}", router(&secret));
        };
        assert_eq!(lead.id, "warn.secret_shared");

        let mut harm = routed(RouteId::General, 0.9, "none", 0.0, 0.5);
        harm.risk = Risk::Harmful;
        harm.risk_p = 0.9;
        assert!(
            matches!(router(&harm), Tier::Refuse { answer, .. } if answer.id == "refuse.harmful")
        );

        let mut pay = routed(RouteId::Wallet, 0.9, "none", 0.0, 0.9);
        pay.risk = Risk::MoneyMovement;
        pay.risk_p = 0.8;
        let tier = router(&pay);
        assert!(matches!(&tier, Tier::CannedFinal { answer, .. } if answer.id == "wallet.send"));
    }

    /// Two close routes: a likely clarify wins, else the model; never a
    /// canned answer or an unsupported offer.
    #[test]
    fn a_close_call_does_less() {
        let mut close = routed(RouteId::General, 0.5, "meta.capabilities", 0.9, 0.1);
        close.runner_up = Some((RouteId::Smalltalk, 0.4));
        assert!(matches!(router(&close), Tier::Model { .. }));
        close.clarify_p = 0.4;
        assert!(matches!(router(&close), Tier::Model { note: Some(_), .. }));
        assert!(matches!(
            decided(&close, &Context::default(), true),
            Tier::Model {
                note: Some(CLARIFY_NOTE),
                ..
            }
        ));
    }

    /// A question about us is grounded in the product documentation when
    /// no surer rule serves it: a meta follow-up no prepared answer fits,
    /// a product question below the grounded bar or in a close call with
    /// a clarify, and a general reading whose close runner-up is
    /// `product.kb` (#10135, #10136, #10137). A general question stays
    /// with the model.
    #[test]
    fn questions_about_us_are_grounded_below_the_bar() {
        let grounded = |routing: &Routing| {
            matches!(
                router(routing),
                Tier::Grounded {
                    corpus: Corpus::Product,
                    ..
                }
            )
        };
        let meta = routed(RouteId::Meta, 0.97, "none", 0.0, 0.15);
        assert!(grounded(&meta), "{:?}", router(&meta));
        let unsure = routed(RouteId::Meta, 0.5, "none", 0.0, 0.28);
        assert!(grounded(&unsure), "{:?}", router(&unsure));
        let mut iphone = routed(RouteId::ProductKb, 0.42, "none", 0.0, 0.09);
        iphone.runner_up = Some((RouteId::Clarify, 0.35));
        iphone.clarify_p = 0.45;
        assert!(grounded(&iphone), "{:?}", router(&iphone));
        let mut general = routed(RouteId::General, 0.45, "none", 0.0, 0.1);
        general.runner_up = Some((RouteId::ProductKb, 0.4));
        assert!(grounded(&general), "{:?}", router(&general));
        general.runner_up = Some((RouteId::ProductKb, 0.1));
        assert!(matches!(router(&general), Tier::Model { .. }));
        let poem = routed(RouteId::General, 0.9, "none", 0.0, 0.1);
        assert!(matches!(router(&poem), Tier::Model { .. }));
    }

    /// On a later turn a clarify is the model told to read the earlier
    /// messages first, never the generic stem (#10138).
    #[test]
    fn a_clarify_on_a_later_turn_reads_the_earlier_messages() {
        let later = |routing: &Routing| {
            decide(
                routing,
                Bank::builtin(),
                &facts(),
                &Situation {
                    mode: Mode::Router,
                    context: &Context::default(),
                    personalize: true,
                    draft: false,
                    earlier: true,
                    plugin: false,
                },
            )
        };
        let reads = Tier::Model {
            lead: None,
            note: Some(LATER_CLARIFY_NOTE),
        };
        let asks = Tier::Model {
            lead: None,
            note: Some(CLARIFY_NOTE),
        };
        // "try that again, I stopped it too soon", after a reply: a close
        // call either way round, and a sure clarify.
        let mut again = routed(RouteId::General, 0.52, "none", 0.0, 0.21);
        again.runner_up = Some((RouteId::Clarify, 0.42));
        again.clarify_p = 0.42;
        assert_eq!(later(&again), reads);
        let mut first = routed(RouteId::Clarify, 0.48, "none", 0.0, 0.19);
        first.runner_up = Some((RouteId::General, 0.4));
        first.clarify_p = 0.48;
        assert_eq!(later(&first), reads);
        let sure = routed(RouteId::Clarify, 0.8, "none", 0.0, 0.5);
        assert_eq!(later(&sure), reads);
        // A first message is the model asking or answering in a few words.
        assert_eq!(decided(&sure, &Context::default(), true), asks);
    }

    /// A clarify reading loses to a sure prepared answer that answers in
    /// the chat: "what is this?" is who we are, on the website too
    /// (#10138). An unsure answer, or one that needs particulars, still
    /// asks.
    #[test]
    fn a_sure_prepared_answer_beats_a_clarify() {
        let mut what = routed(RouteId::Clarify, 0.72, "meta.who", 0.93, 0.22);
        what.clarify_p = 0.72;
        for context in [Context::default(), web()] {
            assert!(matches!(
                decided(&what, &context, true),
                Tier::CannedFinal { answer, .. } if answer.id == "meta.who"
            ));
        }
        let unsure = routed(RouteId::Clarify, 0.72, "meta.who", 0.6, 0.22);
        assert!(matches!(
            decided(&unsure, &Context::default(), true),
            Tier::Model {
                note: Some(CLARIFY_NOTE),
                ..
            }
        ));
        let particular = routed(RouteId::Clarify, 0.72, "meta.who", 0.93, 0.5);
        assert!(matches!(
            decided(&particular, &Context::default(), true),
            Tier::Model {
                note: Some(CLARIFY_NOTE),
                ..
            }
        ));
    }

    #[test]
    fn end_knowledge_cli_and_clarify_routes() {
        let end = routed(RouteId::End, 0.9, "none", 0.0, 0.0);
        let tier = router(&end);
        assert!(matches!(&tier, Tier::CannedFinal { answer, .. } if answer.id == "smalltalk.bye"));

        let kb = routed(RouteId::ProductKb, 0.7, "none", 0.0, 0.5);
        assert!(matches!(
            router(&kb),
            Tier::Grounded {
                corpus: Corpus::Product,
                ..
            }
        ));
        let code = routed(RouteId::CodebaseKb, 0.7, "none", 0.0, 0.5);
        assert!(matches!(
            router(&code),
            Tier::Grounded {
                corpus: Corpus::Codebase,
                ..
            }
        ));

        let mut cli = routed(RouteId::Cli, 0.8, "none", 0.0, 0.5);
        assert!(
            matches!(router(&cli), Tier::Model { .. }),
            "no group, no proposal"
        );
        cli.cli_group = Some(("computer".into(), 0.7));
        cli.cli_alternatives = vec![("reach".into(), 0.2)];
        assert_eq!(
            router(&cli),
            Tier::Cli {
                group: "computer".into(),
                also: vec!["reach".into()],
                lead: None
            }
        );
        // A sure route descends an unsure group beside the next likely
        // ones ("which of my computers are online": `computer` or `reach`).
        cli.route_p = 1.0;
        cli.cli_group = Some(("computer".into(), 0.38));
        cli.cli_alternatives = vec![("reach".into(), 0.33)];
        assert_eq!(
            router(&cli),
            Tier::Cli {
                group: "computer".into(),
                also: vec!["reach".into()],
                lead: None
            }
        );
        // A sure group from a less sure route proposes it alone.
        cli.route_p = 0.65;
        cli.cli_group = Some(("task".into(), 0.95));
        assert_eq!(
            router(&cli),
            Tier::Cli {
                group: "task".into(),
                also: Vec::new(),
                lead: None
            }
        );
        // Neither sure: no proposal, and the lane alone does not dispatch a
        // `cli` message.
        cli.cli_group = Some(("task".into(), 0.5));
        cli.lane = Lane::Computer;
        cli.lane_p = 0.9;
        assert!(matches!(router(&cli), Tier::Model { .. }));

        let clarify = routed(RouteId::Clarify, 0.7, "none", 0.0, 0.5);
        assert_eq!(
            router(&clarify),
            Tier::Model {
                lead: None,
                note: Some(CLARIFY_NOTE)
            }
        );
    }

    /// The legacy mode serves what coder-first-response-v2 served: a whole
    /// answer with no offer, else an opener, else nothing.
    #[test]
    fn legacy_requests_get_no_offers_or_stems() {
        let legacy = |routing: &Routing| {
            decide(
                routing,
                Bank::builtin(),
                &facts(),
                &Situation {
                    mode: Mode::Legacy,
                    context: &Context::default(),
                    personalize: true,
                    draft: false,
                    earlier: false,
                    plugin: false,
                },
            )
        };
        let who = routed(RouteId::Meta, 0.9, "meta.who", 0.9, 0.1);
        assert_eq!(legacy(&who).word(), "canned");
        let wallet = routed(RouteId::Wallet, 0.9, "wallet.what", 0.9, 0.1);
        assert_eq!(legacy(&wallet).word(), "model");
        let mut work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.9, 0.9);
        work.opener = Bank::builtin().opener("plan").map(|o| (o.clone(), 0.8));
        assert_eq!(legacy(&work).word(), "opener");
    }

    /// The Gym and eval routes: news and the eval routes read the records,
    /// the interview is its own tier, credit is the bank's, and an unsure
    /// reading is the model told it has no records.
    #[test]
    fn gym_and_eval_routes_read_records_or_say_they_have_none() {
        let mut news = routed(RouteId::GymNews, 0.65, "none", 0.0, 0.5);
        assert_eq!(
            router(&news),
            Tier::Gym {
                route: RouteId::GymNews,
                tool: None,
                lead: None
            }
        );
        assert_eq!(router(&news).number(), 2);
        news.route_p = 0.55;
        news.runner_up = Some((RouteId::General, 0.3));
        assert_eq!(
            router(&news),
            Tier::Model {
                lead: None,
                note: Some(crate::router::gym::NO_RECORDS_NOTE)
            }
        );

        let mut run = routed(RouteId::EvalRun, 0.8, "none", 0.0, 0.5);
        run.tool = Some(("openagents.tool-project-map".into(), 0.7));
        let tier = router(&run);
        assert_eq!(
            tier,
            Tier::Gym {
                route: RouteId::EvalRun,
                tool: Some("openagents.tool-project-map".into()),
                lead: None
            }
        );
        assert_eq!((tier.word(), tier.number()), ("gym", 4));
        assert!(
            tier.keeps_model(),
            "the model is the fallback until the records answer"
        );
        run.tool = Some(("openagents.tool-project-map".into(), 0.5));
        assert!(matches!(router(&run), Tier::Gym { tool: None, .. }));
        run.route_p = 0.65;
        assert!(matches!(router(&run), Tier::Model { note: Some(_), .. }));

        let author = routed(RouteId::EvalAuthor, 0.8, "none", 0.0, 0.9);
        assert_eq!(router(&author), Tier::Author);
        let credit = routed(RouteId::EvalCredit, 0.8, "none", 0.0, 0.2);
        assert!(
            matches!(router(&credit), Tier::CannedFinal { answer, .. } if answer.id == "eval.credit.mine")
        );
        // A sure credit answer at the specifics ceiling is still that answer.
        let how = routed(RouteId::EvalCredit, 0.9, "eval.credit.how", 0.95, 0.3);
        assert!(
            matches!(router(&how), Tier::CannedFinal { answer, .. } if answer.id == "eval.credit.how")
        );
        // A close call with a Gym route tells the model it has no records.
        let mut close = routed(RouteId::General, 0.5, "none", 0.0, 0.5);
        close.runner_up = Some((RouteId::EvalResult, 0.45));
        assert!(
            matches!(router(&close), Tier::Model { note: Some(note), .. }
            if note == crate::router::gym::NO_RECORDS_NOTE)
        );
    }

    /// With a draft open, the interview continues through short answers
    /// and runs, but not through a sure other route or a risk.
    #[test]
    fn an_open_draft_continues_the_interview() {
        let drafting = |routing: &Routing| {
            decide(
                routing,
                Bank::builtin(),
                &facts(),
                &Situation {
                    mode: Mode::Router,
                    context: &Context::default(),
                    personalize: false,
                    draft: true,
                    earlier: false,
                    plugin: false,
                },
            )
        };
        let looks_good = routed(RouteId::Smalltalk, 0.9, "smalltalk.thanks", 0.9, 0.1);
        assert_eq!(drafting(&looks_good), Tier::Author);
        assert_eq!(
            router(&looks_good).word(),
            "canned",
            "no draft: the bank answers"
        );
        let pilot = routed(RouteId::EvalRun, 0.9, "none", 0.0, 0.5);
        assert_eq!(drafting(&pilot), Tier::Author);
        let unsure = routed(RouteId::Wallet, 0.6, "none", 0.0, 0.5);
        assert_eq!(drafting(&unsure), Tier::Author);
        let wallet = routed(RouteId::Wallet, 0.9, "wallet.what", 0.9, 0.1);
        assert_eq!(drafting(&wallet).word(), "canned");
        let mut secret = routed(RouteId::Smalltalk, 0.9, "none", 0.0, 0.5);
        secret.risk = Risk::SecretShared;
        secret.risk_p = 0.9;
        assert_eq!(drafting(&secret).word(), "refuse");
    }

    /// A plugin being made on this computer (#10177) continues through
    /// short answers and through a reply that reads as more work for Coder
    /// or as a command, but not through a sure other route or a risk; with
    /// no plugin open, the same work reading is a dispatch.
    #[test]
    fn an_open_plugin_flow_continues_through_work_and_commands() {
        let making = |routing: &Routing, plugin: bool| {
            decide(
                routing,
                Bank::builtin(),
                &facts(),
                &Situation {
                    mode: Mode::Router,
                    context: &Context::default(),
                    personalize: false,
                    draft: false,
                    earlier: true,
                    plugin,
                },
            )
        };
        let more_work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.8, 0.5);
        assert_eq!(making(&more_work, true), Tier::Author);
        assert_ne!(making(&more_work, false), Tier::Author);
        let turn_it_on = routed(RouteId::Cli, 0.9, "none", 0.0, 0.5);
        assert_eq!(making(&turn_it_on, true), Tier::Author);
        let yes = routed(RouteId::Smalltalk, 0.9, "smalltalk.thanks", 0.9, 0.1);
        assert_eq!(making(&yes, true), Tier::Author);
        let wallet = routed(RouteId::Wallet, 0.9, "wallet.what", 0.9, 0.1);
        assert_eq!(making(&wallet, true).word(), "canned");
        let mut secret = routed(RouteId::Smalltalk, 0.9, "none", 0.0, 0.5);
        secret.risk = Risk::SecretShared;
        secret.risk_p = 0.9;
        assert_eq!(making(&secret, true).word(), "refuse");
    }

    /// A missing capability shows only when the route and the capability
    /// reading agree: the bank's line, naming the closest admitted
    /// capability when one is close; a lone reading is the model.
    #[test]
    fn a_missing_capability_needs_both_readings_to_agree() {
        use crate::router::capability::{Admitted, CODER};
        let admitted = Admitted::of(
            &[crate::router::gym::fixtures::tool(
                "project-map",
                "Project map",
            )],
            &[],
        );
        let coder = admitted.get(CODER).unwrap().clone();
        let mut flight = routed(RouteId::CapabilityMissing, 0.85, "none", 0.0, 0.9);
        flight.capability_missing_p = 0.8;
        let tier = router(&flight);
        let Tier::Capability {
            answer,
            text,
            closest,
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "capability.missing");
        assert!(
            text.starts_with("There's no plugin for that yet."),
            "{text}"
        );
        assert_eq!(closest, &None);
        assert_eq!((tier.word(), tier.number()), ("canned", 0));
        assert!(!tier.keeps_model());
        assert_eq!(
            tier.answer().map(|entry| entry.id.as_str()),
            Some("capability.missing")
        );

        // A close admitted entry is named from the set, never the message.
        flight.capability_closest = Some((coder.clone(), 0.25));
        let Tier::Capability {
            answer,
            text,
            closest,
        } = router(&flight)
        else {
            panic!("{:?}", router(&flight));
        };
        assert_eq!(answer.id, "capability.missing_near");
        assert!(
            text.contains("The closest thing we have is Coder:"),
            "{text}"
        );
        assert_eq!(closest.map(|entry| entry.id), Some(CODER.to_string()));
        flight.capability_closest = Some((coder.clone(), 0.15));
        assert!(
            matches!(router(&flight), Tier::Capability { closest: None, .. }),
            "under the closest floor, no name"
        );

        // Each reading alone is not enough.
        let mut unsure_route = flight.clone();
        unsure_route.route_p = 0.65;
        unsure_route.runner_up = Some((RouteId::General, 0.3));
        assert!(matches!(router(&unsure_route), Tier::Model { .. }));
        let mut unsure_none = flight.clone();
        unsure_none.capability_missing_p = 0.5;
        assert!(matches!(router(&unsure_none), Tier::Model { .. }));
        let mut general = flight.clone();
        general.route = RouteId::General;
        general.runner_up = Some((RouteId::Meta, 0.1));
        assert!(matches!(router(&general), Tier::Model { .. }));
        // An admitted entry named at confidence is never "missing".
        let mut named = flight.clone();
        named.capability = Some((coder.clone(), 0.9));
        named.capability_closest = None;
        assert!(matches!(router(&named), Tier::Model { .. }));
        // A refusal still comes first.
        let mut risky = flight.clone();
        risky.risk = Risk::Harmful;
        risky.risk_p = 0.9;
        assert!(matches!(router(&risky), Tier::Refuse { .. }));
    }

    /// A dispatch offer names the engine the `engine` reading found at
    /// [`ENGINE_CONFIDENCE`], and its stem says so; an unsure reading, or
    /// none, is no preference. A turn that is not a dispatch carries no
    /// engine, whatever the reading (#10076).
    #[test]
    fn a_dispatch_names_the_engine_the_person_asked_for() {
        use crate::router::CodingEngine as Engine;
        let mut work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.8, 0.9);
        work.engine = Some((Engine::ClaudeCode, 0.9));
        let tier = router(&work);
        let Tier::CannedStem {
            answer,
            stem,
            offer,
            ..
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.engine_stem");
        assert_eq!(stem, "Starting Claude Code on");
        // Closed by its generic end, never a continuation.
        assert!(matches!(
            tier,
            Tier::CannedStem {
                personalize: false,
                ..
            }
        ));
        assert_eq!(
            offer,
            &Some(Offer::RunCoder {
                label: "Run Coder".into(),
                engine: Some(Engine::ClaudeCode),
                plan: Default::default(),
            })
        );
        // An unsure reading is no preference, on the plain stem.
        work.engine = Some((Engine::ClaudeCode, ENGINE_CONFIDENCE - 0.01));
        let tier = router(&work);
        let Tier::CannedStem { answer, offer, .. } = &tier else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.stem");
        assert!(matches!(offer, Some(Offer::RunCoder { engine: None, .. })));
        work.engine = None;
        assert!(matches!(
            router(&work),
            Tier::CannedStem {
                offer: Some(Offer::RunCoder { engine: None, .. }),
                ..
            }
        ));
        // Not a dispatch: "ask Claude what a monad is" names Claude only as
        // the subject, and no offer carries an engine.
        let mut general = routed(RouteId::General, 0.9, "none", 0.0, 0.9);
        general.engine = Some((Engine::ClaudeCode, 0.9));
        general.lane = Lane::Chat;
        assert!(matches!(router(&general), Tier::Model { .. }));
    }

    /// #10183: "do 3 readonly delegations, 1 per agent, explore repo and
    /// summarize briefly" in a terminal is a plan, not one run: one run on
    /// each engine the computer has ready, read-only, with a combined
    /// summary, and the reply says plainly what starts. An unsure reading,
    /// another surface, or fewer than two engines is one run, as before.
    #[test]
    fn a_fan_out_in_a_terminal_plans_one_run_per_ready_engine() {
        use crate::router::{CodingEngine as Engine, DispatchPlan, Fanout};
        let terminal = Context::of(&serde_json::json!({
            "surface": "terminal",
            "computer": { "place": "here", "engines": [
                {"engine": "codex", "state": "ready"},
                {"engine": "claude", "state": "ready"},
                {"engine": "grok", "state": "ready"},
                {"engine": "devin", "state": "not_signed_in"},
            ]},
        }));
        let mut work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.8, 0.9);
        work.fanout = Some((Fanout::EachEngine, 0.9));
        work.read_only = 0.93;
        work.summarize = 0.88;
        let tier = decided(&work, &terminal, false);
        let Tier::CannedFinal {
            answer,
            text,
            offer,
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.fan_out");
        assert_eq!(
            text,
            "Exploring the repo with Codex, Claude Code, and Grok Build."
        );
        let plan = DispatchPlan {
            runs: vec![Engine::Codex, Engine::ClaudeCode, Engine::GrokBuild],
            read_only: true,
            summarize: true,
        };
        assert_eq!(
            offer,
            &Some(Offer::RunCoder {
                label: "Run Coder".into(),
                engine: None,
                plan: plan.clone(),
            })
        );
        // The offer reaches the wire whole.
        let wire = offer.as_ref().unwrap().feedback(2).unwrap();
        assert_eq!(
            wire["runs"],
            serde_json::json!(["codex", "claude_code", "grok_build"])
        );
        assert_eq!(wire["read_only"], true);
        // Two named engines that may change files.
        work.fanout = Some((Fanout::Pair(Engine::Codex, Engine::ClaudeCode), 0.85));
        work.read_only = 0.2;
        work.summarize = 0.1;
        let Tier::CannedFinal { text, offer, .. } = decided(&work, &terminal, false) else {
            panic!();
        };
        assert_eq!(text, "Working on this with Codex and Claude Code.");
        assert!(matches!(
            offer,
            Some(Offer::RunCoder { plan: DispatchPlan { ref runs, read_only: false, summarize: false }, .. })
                if *runs == [Engine::Codex, Engine::ClaudeCode]
        ));
        // Unsure: one run on the plain stem.
        work.fanout = Some((Fanout::EachEngine, FANOUT_CONFIDENCE - 0.01));
        assert!(matches!(
            decided(&work, &terminal, false),
            Tier::CannedStem { offer: Some(Offer::RunCoder { ref plan, .. }), .. } if plan.is_single()
        ));
        // A phone or the desktop starts one run.
        work.fanout = Some((Fanout::EachEngine, 0.9));
        assert!(matches!(router(&work), Tier::CannedStem { .. }));
        // Only one engine ready: one run.
        let lonely = Context::of(&serde_json::json!({
            "surface": "terminal",
            "computer": { "place": "here", "engines": [
                {"engine": "codex", "state": "ready"},
                {"engine": "claude", "state": "not_signed_in"},
            ]},
        }));
        assert!(matches!(
            decided(&work, &lonely, false),
            Tier::CannedStem { .. }
        ));
        // A terminal whose context names no engines: the three.
        let bare = Context::of(&serde_json::json!({ "surface": "terminal" }));
        let Tier::CannedFinal { text, .. } = decided(&work, &bare, false) else {
            panic!();
        };
        assert_eq!(
            text,
            "Working on this with Codex, Claude Code, and Grok Build."
        );
        // Not a dispatch: no plan, whatever the reading.
        let mut general = routed(RouteId::General, 0.9, "none", 0.0, 0.9);
        general.fanout = Some((Fanout::EachEngine, 0.9));
        general.lane = Lane::Chat;
        assert!(matches!(
            decided(&general, &terminal, false),
            Tier::Model { .. }
        ));
    }

    /// A dispatch offer names the Coder-run capability the reading found
    /// (a catalog tool), and a `capability.missing` route that names one
    /// with the lane at computer is that offer, not the card.
    #[test]
    fn a_dispatch_names_the_capability_it_would_use() {
        use crate::router::capability::{Admitted, CODER};
        let admitted = Admitted::of(
            &[crate::router::gym::fixtures::tool(
                "project-map",
                "Project map",
            )],
            &[],
        );
        let map = admitted.get("openagents.tool-project-map").unwrap().clone();
        let mut work = routed(RouteId::WorkDispatch, 0.9, "dispatch.stem", 0.8, 0.9);
        work.capability = Some((map.clone(), 0.8));
        let tier = decided(&work, &Context::default(), true);
        let Tier::CannedStem {
            answer,
            stem,
            offer,
            ..
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "dispatch.capability_stem");
        assert_eq!(stem, "Using Project map to");
        assert!(matches!(offer, Some(Offer::RunCoder { .. })));
        assert_eq!(tier.word(), "offer");
        // Coder itself, or an unsure reading, is the plain stem.
        work.capability = Some((admitted.get(CODER).unwrap().clone(), 0.9));
        assert!(
            matches!(router(&work), Tier::CannedStem { answer, .. } if answer.id == "dispatch.stem")
        );
        work.capability = Some((map.clone(), 0.5));
        assert!(
            matches!(router(&work), Tier::CannedStem { answer, .. } if answer.id == "dispatch.stem")
        );
        // With no computer, the no-computer answer as before.
        work.capability = Some((map.clone(), 0.8));
        let none = Context {
            computer_ready: Some(false),
            ..Context::default()
        };
        assert!(matches!(
            decided(&work, &none, true),
            Tier::CannedFinal { answer, .. } if answer.id == "dispatch.no_computer"
        ));
        // The route read missing, the reading named a Coder-run capability,
        // and the lane says computer: the admitted capability answers.
        let mut missed = routed(RouteId::CapabilityMissing, 0.8, "none", 0.0, 0.9);
        missed.capability = Some((map, 0.8));
        missed.capability_missing_p = 0.1;
        missed.lane = Lane::Computer;
        missed.lane_p = 0.9;
        assert!(
            matches!(router(&missed), Tier::CannedStem { answer, .. } if answer.id == "dispatch.capability_stem")
        );
        missed.lane = Lane::Chat;
        assert!(matches!(router(&missed), Tier::Model { .. }));
    }

    /// Every phone surface keeps money, secrets, and grants off the CLI.
    #[test]
    fn phone_gates_stay_read_only() {
        for effect in [
            crate::router::Effect::LocalWrite,
            crate::router::Effect::Publishes,
            crate::router::Effect::LongRunning,
            crate::router::Effect::Spends,
            crate::router::Effect::Secret,
            crate::router::Effect::Grants,
        ] {
            assert_ne!(
                crate::router::gate(effect, Surface::Phone),
                crate::router::CliGate::Offer
            );
        }
    }

    /// `presentation.open` on the desktop: the deck the `deck` reading
    /// named, with a typed `open_presentation` offer for its id; no deck,
    /// or an unsure one, a plain refusal listing the decks; off the
    /// desktop, the line that says where decks open, with no offer
    /// (#10058).
    #[test]
    fn a_deck_is_offered_on_the_desktop_and_refused_plainly_elsewhere() {
        let decks = crate::router::decks();
        let desktop = Context {
            surface: Some(Surface::Desktop),
            ..Context::default()
        };
        let mut routing = routed(RouteId::PresentationOpen, 0.9, "none", 0.0, 0.2);
        routing.deck = Some((decks[1].id.to_string(), 0.85));
        let tier = decided(&routing, &desktop, false);
        let Tier::CannedFinal {
            answer,
            text,
            offer: Some(Offer::OpenPresentation { deck, label }),
        } = &tier
        else {
            panic!("{tier:?}");
        };
        assert_eq!(answer.id, "presentation.open");
        assert_eq!(deck, decks[1].id);
        assert_eq!(text, &format!("Opening {}.", decks[1].title));
        assert_eq!(label, &format!("Open {}", decks[1].title));
        assert_eq!((tier.word(), tier.number()), ("canned", 0));
        assert!(!tier.keeps_model());
        let offer = tier_offer(&tier).feedback(2).expect("NIP-CJ writes it");
        assert_eq!(offer["offer"], "open_presentation");
        assert_eq!(offer["deck"], decks[1].id);

        // No deck named, or one below the floor: the plain refusal, which
        // lists every deck and offers nothing.
        for deck in [None, Some((decks[0].id.to_string(), 0.4))] {
            routing.deck = deck;
            let tier = decided(&routing, &desktop, false);
            let Tier::CannedFinal {
                answer,
                text,
                offer,
            } = &tier
            else {
                panic!("{tier:?}");
            };
            assert_eq!(answer.id, "presentation.unknown");
            assert_eq!(offer, &None);
            assert!(text.starts_with("We can't find that deck."), "{text}");
            for deck in decks {
                assert!(text.contains(&deck.title), "{text}");
            }
        }

        // The phone and the terminal have no slide viewer.
        routing.deck = Some((decks[0].id.to_string(), 0.95));
        for surface in [None, Some(Surface::Phone), Some(Surface::Terminal)] {
            let context = Context {
                surface,
                ..Context::default()
            };
            let tier = decided(&routing, &context, false);
            let Tier::CannedFinal {
                answer,
                text,
                offer,
            } = &tier
            else {
                panic!("{tier:?}");
            };
            assert_eq!(answer.id, "presentation.elsewhere");
            assert_eq!(offer, &None);
            assert_eq!(
                text,
                "Decks open in the OpenAgents desktop app, so we can't show one here."
            );
        }

        // An unsure route reading is the model's, deck or not.
        let mut unsure = routed(RouteId::PresentationOpen, 0.65, "none", 0.0, 0.2);
        unsure.runner_up = Some((RouteId::General, 0.2));
        unsure.deck = Some((decks[0].id.to_string(), 0.95));
        assert!(matches!(
            decided(&unsure, &desktop, false),
            Tier::Model { .. }
        ));
        // A deck reading on another route opens nothing.
        let mut other = routed(RouteId::General, 0.9, "none", 0.0, 0.9);
        other.deck = Some((decks[0].id.to_string(), 0.95));
        assert!(matches!(
            decided(&other, &desktop, false),
            Tier::Model { .. }
        ));
    }

    fn tier_offer(tier: &Tier) -> &Offer {
        match tier {
            Tier::CannedFinal {
                offer: Some(offer), ..
            } => offer,
            other => panic!("{other:?}"),
        }
    }
}
