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
//!    ([`AUTHOR_CONTINUES`]); risk is still read first.
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
//! 8. **T2 grounded.** `route` = `product.kb` or `codebase.kb` at
//!    [`GROUNDED_ROUTE`].
//! 9. **Gym and eval.** `gym.news` at [`GROUNDED_ROUTE`], or another
//!    `eval.*` route at [`EVAL_ROUTE`]: `eval.author` is a step of the
//!    interview ([`Tier::Author`]), `eval.credit` the bank's
//!    `eval.credit.mine`, and the rest read the Gym's verified records
//!    ([`Tier::Gym`]), which [`super::gym::reply`] turns into a bank line,
//!    a card, and an offer, or a grounded reply for news.
//! 10. **T4 dispatch by lane.** `lane` = computer at [`DISPATCH_LANE`],
//!     after the CLI and knowledge routes, which read such a message more
//!     precisely, and only on a route in [`LANE_ROUTES`]: an offer loses to
//!     a route with its own answer.
//! 11. **Clarify.** `route` = `clarify` at [`CLARIFY_ROUTE`]: the
//!     `clarify.generic` stem when personalization is available, else the
//!     model told to ask one question.
//! 12. **T3 model**, led by the argmax opener at [`OPENER_CONFIDENCE`];
//!     when the route or the runner-up is a Gym or eval route, the model is
//!     told it has no verified records ([`super::gym::NO_RECORDS_NOTE`]).
//!
//! A request that asks only for `opener` or `judge` (the phones before the
//! router) is decided in [`Mode::Legacy`]: rule 3 for entries with no
//! offer, then rule 12, which is what `coder-first-response-v2` showed.

use super::bank::{Bank, Entry, Facts};
use super::judge::Routing;
use super::{Context, Corpus, Offer, Risk, RouteId};
use crate::first::Lane;

/// The least `route` probability for a whole prepared answer or `end`.
pub const ROUTE_CONFIDENCE: f64 = 0.80;
/// The least `answer` probability for a whole prepared answer.
pub const ANSWER_CONFIDENCE: f64 = 0.80;
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

/// The instruction the model gets when the router wants one question.
pub const CLARIFY_NOTE: &str = "The user's message is ambiguous. Reply with one short question \
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
            Tier::CannedFinal { .. } => "canned",
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
            Tier::CannedFinal { .. } | Tier::Refuse { .. } => 0,
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
            | Tier::Refuse { answer, .. } => Some(answer),
            _ => None,
        }
    }
}

fn dispatches(entry: &Entry) -> bool {
    entry.answers(RouteId::WorkDispatch) || entry.answers(RouteId::Cli)
}

fn final_of(bank: &Bank, facts: &Facts, id: &str) -> Option<Tier> {
    let entry = bank.entry(id)?;
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
        .filter(|(_, p)| *p >= OPENER_CONFIDENCE)
        .map(|(opener, _)| Lead {
            id: opener.id.clone(),
            text: opener.text.clone(),
        })
}

/// T3: the model, led by the opener; told it has no Gym records when the
/// route or the runner-up is a Gym or eval route, so it states no result.
fn model(routing: &Routing) -> Tier {
    let gym = routing.route.is_gym()
        || routing
            .runner_up
            .is_some_and(|(route, p)| route.is_gym() && routing.route_p - p < CLOSE_MARGIN);
    Tier::Model {
        lead: opener_lead(routing),
        note: gym.then_some(super::gym::NO_RECORDS_NOTE),
    }
}

/// Rule 8b: the Gym and eval routes.
fn gym(routing: &Routing, bank: &Bank, facts: &Facts) -> Option<Tier> {
    let floor = if routing.route == RouteId::GymNews {
        GROUNDED_ROUTE
    } else {
        EVAL_ROUTE
    };
    if !routing.route.is_gym() || routing.route_p < floor {
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
                .filter(|(entry, p)| entry.answers(RouteId::EvalCredit) && *p >= STEM_CONFIDENCE)
                .map_or("eval.credit.mine", |(entry, _)| entry.id.as_str());
            final_of(bank, facts, id)
        }
        route => Some(Tier::Gym {
            route,
            tool: routing
                .tool
                .as_ref()
                .filter(|(_, p)| *p >= TOOL_CONFIDENCE)
                .map(|(tool, _)| tool.clone()),
            lead: opener_lead(routing),
        }),
    }
}

/// Rule 3: a whole prepared answer, if the judgment is sure of one.
fn canned(routing: &Routing, facts: &Facts, offers: bool) -> Option<Tier> {
    let (entry, p) = routing.answer.as_ref()?;
    if routing.route_p < ROUTE_CONFIDENCE
        || *p < ANSWER_CONFIDENCE
        || routing.needs_specifics >= SPECIFICS_CEILING
        || !entry.answers(routing.route)
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
fn dispatch(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Option<Tier> {
    if situation.context.computer_ready == Some(false) {
        return final_of(bank, facts, "dispatch.no_computer");
    }
    let chosen = routing
        .answer
        .as_ref()
        .map(|(entry, _)| entry)
        .filter(|entry| entry.answers(RouteId::WorkDispatch) && entry.stem.is_some());
    let entry = chosen.or_else(|| bank.entry("dispatch.stem"))?;
    stem_of(entry, facts, situation.personalize)
}

/// The tier for `routing`. See the module documentation for the rules.
#[must_use]
pub fn decide(routing: &Routing, bank: &Bank, facts: &Facts, situation: &Situation) -> Tier {
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
    let agreed = routing.route == RouteId::Refuse && routing.route_p >= ROUTE_CONFIDENCE;
    if risky && (routing.risk_p >= RISK_REFUSE || (agreed && routing.risk_p >= RISK_WARN)) {
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
        && (AUTHOR_CONTINUES.contains(&routing.route) || routing.route_p < ROUTE_CONFIDENCE)
        && !(risky && routing.risk_p >= RISK_WARN)
    {
        return Tier::Author;
    }
    if risky && routing.risk_p >= RISK_WARN {
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
        && routing.risk_p >= RISK_WARN
        && let Some(tier) = final_of(bank, facts, "wallet.send")
    {
        return tier;
    }

    // 2. Close call.
    if let Some((second, second_p)) = routing.runner_up
        && routing.route != RouteId::Unknown
        && routing.route_p - second_p < CLOSE_MARGIN
    {
        if routing.clarify_p >= CLARIFY_WINS {
            return clarify(bank, facts, situation);
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
            && routing.lane_p >= DISPATCH_LANE
            && let Some(tier) = dispatch(routing, bank, facts, situation)
        {
            return tier;
        }
        return model(routing);
    }

    // 3. T0.
    if let Some(tier) = canned(routing, facts, true) {
        return tier;
    }

    // 4. End.
    if routing.route == RouteId::End
        && routing.route_p >= ROUTE_CONFIDENCE
        && let Some(tier) = final_of(bank, facts, "smalltalk.bye")
    {
        return tier;
    }

    // 5. T1.
    if let Some((entry, p)) = &routing.answer
        && *p >= STEM_CONFIDENCE
        && routing.needs_specifics >= SPECIFICS_CEILING
        && entry.answers(routing.route)
        && !dispatches(entry)
        && let Some(tier) = stem_of(entry, facts, situation.personalize)
    {
        return tier;
    }

    // 6. T4 dispatch, by route.
    if routing.route == RouteId::WorkDispatch
        && routing.route_p >= DISPATCH_ROUTE
        && let Some(tier) = dispatch(routing, bank, facts, situation)
    {
        return tier;
    }

    // 7. T4 CLI: the route is sure enough, and so is a group, or the
    // route is sure and the seam descends the likely groups as a beam.
    if routing.route == RouteId::Cli
        && let Some((group, group_p)) = &routing.cli_group
        && ((routing.route_p >= CLI_ROUTE && *group_p >= CLI_GROUP)
            || (routing.route_p >= CLI_ROUTE_SURE && *group_p >= CLI_GROUP_BEAM)
            || (routing.route_p >= GROUNDED_ROUTE && *group_p >= CLI_GROUP_SURE))
    {
        let also = if *group_p >= CLI_GROUP_SURE {
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
    if routing.route_p >= GROUNDED_ROUTE {
        let corpus = match routing.route {
            RouteId::ProductKb => Some(Corpus::Product),
            RouteId::CodebaseKb => Some(Corpus::Codebase),
            _ => None,
        };
        if let Some(corpus) = corpus {
            return Tier::Grounded {
                corpus,
                lead: opener_lead(routing),
            };
        }
    }

    // 9. Gym and eval.
    if let Some(tier) = gym(routing, bank, facts) {
        return tier;
    }

    // 10. T4 dispatch, by lane: work the route did not name, when the lane is sure it
    // needs a computer. It comes after the CLI and knowledge routes, which
    // read a computer question more precisely, and only when the route is
    // not one with its own answer: an offer loses to an answer.
    if LANE_ROUTES.contains(&routing.route)
        && routing.lane == Lane::Computer
        && routing.lane_p >= DISPATCH_LANE
        && let Some(tier) = dispatch(routing, bank, facts, situation)
    {
        return tier;
    }

    // 11. Clarify.
    if routing.route == RouteId::Clarify && routing.route_p >= CLARIFY_ROUTE {
        return clarify(bank, facts, situation);
    }

    // 12. T3.
    model(routing)
}

fn clarify(bank: &Bank, facts: &Facts, situation: &Situation) -> Tier {
    if situation.personalize
        && let Some(tier) = bank
            .entry("clarify.generic")
            .and_then(|entry| stem_of(entry, facts, true))
    {
        return tier;
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
            Some((6, 40)),
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
            risk: Risk::Ok,
            risk_p: 0.95,
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
            },
        )
    }

    fn router(routing: &Routing) -> Tier {
        decided(routing, &Context::default(), false)
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
        assert!(text.starts_with("Our chat runs on Google's Gemini 3.8 Flash"));
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
            assert!(
                matches!(router(&routing), Tier::Model { .. }),
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
        assert_eq!(stem, "We'll have Coder look through");
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
        let mut close = routed(RouteId::Meta, 0.5, "meta.capabilities", 0.9, 0.1);
        close.runner_up = Some((RouteId::General, 0.4));
        assert!(matches!(router(&close), Tier::Model { .. }));
        close.clarify_p = 0.4;
        assert!(matches!(router(&close), Tier::Model { note: Some(_), .. }));
        assert!(matches!(
            decided(&close, &Context::default(), true),
            Tier::CannedStem { answer, .. } if answer.id == "clarify.generic"
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
}
