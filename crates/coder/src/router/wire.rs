//! What the router puts on the wire (NIP-CJ, all additive) and in the
//! worker's log.
//!
//! - [`judgment`]: the `27000` `judgment` feedback, today's fields plus
//!   the router's typed readings.
//! - [`Served`] and [`annotate`]: the `26900` result's `tier`, `answer`,
//!   `route`, `bank`, `followups`, and citations.
//! - [`Shadow`]: one log record per routed turn, built only from ids,
//!   probabilities, tiers, and timings, so it can never carry message
//!   text.

use serde::Serialize;
use serde_json::{Value, json};

use super::SET;
use super::bank::Bank;
use super::judge::Routing;
use super::policy::{Mode, Tier};
use super::seams::Passage;
use crate::classify::Route;

/// The NIP-CJ verdict for a routed turn.
#[must_use]
pub fn verdict(routing: &Routing, tier: &Tier) -> &'static str {
    if tier
        .answer()
        .and_then(|entry| entry.verdict.as_deref())
        .is_some_and(|verdict| verdict == "end_conversation")
    {
        return "end_conversation";
    }
    match routing.action {
        Route::Respond => "respond",
        Route::Clarify => "clarify",
        Route::End => "end_conversation",
        Route::Halt(_) => "unrouted",
    }
}

/// The display line: what is shown first, else the verdict.
#[must_use]
pub fn line(routing: &Routing, tier: &Tier) -> String {
    match tier {
        Tier::CannedFinal { text, .. } | Tier::Refuse { text, .. } => text.clone(),
        Tier::CannedStem { stem, .. } => stem.clone(),
        Tier::Model {
            lead: Some(lead), ..
        } => lead.text.clone(),
        _ => verdict(routing, tier).to_string(),
    }
}

/// The `27000` judgment feedback for a routed turn at payload `version`.
///
/// `verdict` and `line` are NIP-CJ's. `set`, `lane`, `opener`,
/// `confidence`, `bank`, `answer`, `answer_p`, `needs_specifics`, and
/// `tier` keep the meaning `coder-first-response-v2` gave them, so a
/// reader from before the router still reads them; `route`, `route_p`,
/// `lane_p`, `risk`, `risk_p`, and `cli_group` are the router's. In shadow
/// mode `tier` is what was served and `shadow` names what the router
/// would have served.
#[must_use]
pub fn judgment(
    version: u64,
    routing: &Routing,
    tier: &Tier,
    bank: &Bank,
    shadow: Option<&Tier>,
) -> Value {
    let opener = match tier {
        Tier::Model {
            lead: Some(lead), ..
        } if bank.opener(&lead.id).is_some() => Some(lead.id.clone()),
        _ => None,
    };
    let mut body = json!({
        "v": version,
        "requires": [],
        "type": "judgment",
        "verdict": verdict(routing, tier),
        "line": line(routing, tier),
        "set": SET,
        "bank": bank.id(),
        "route": routing.route.word(),
        "route_p": routing.route_p,
        "answer": routing.answer.as_ref().map(|(entry, _)| entry.tag()),
        "answer_p": routing.answer.as_ref().map_or(0.0, |(_, p)| *p),
        "needs_specifics": routing.needs_specifics,
        "lane": routing.lane.word(),
        "lane_p": routing.lane_p,
        "opener": opener,
        "confidence": routing.opener.as_ref().map_or(0.0, |(_, p)| *p),
        "cli_group": routing.cli_group.as_ref().map(|(group, _)| group.clone()),
        "risk": routing.risk.word(),
        "risk_p": routing.risk_p,
        "tier": tier.word(),
    });
    if let Some(shadow) = shadow {
        body["shadow"] = json!(shadow.word());
    }
    body
}

/// One cited passage in a grounded result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Citation {
    pub id: String,
    pub title: String,
    pub source: String,
}

impl From<&Passage> for Citation {
    fn from(passage: &Passage) -> Self {
        Self {
            id: passage.id.clone(),
            title: passage.title.clone(),
            source: passage.source.clone(),
        }
    }
}

/// What a routed turn served, for its result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Served {
    /// The tier word as served.
    pub tier: &'static str,
    /// The route word.
    pub route: &'static str,
    /// `id@version` of the bank entry that supplied text, when one did.
    pub answer: Option<String>,
    /// The model that wrote any of the text: `bank:<name>` when none did,
    /// the continuation's model for a personalized stem, `None` to keep
    /// the door's.
    pub model: Option<String>,
    /// Suggestion chips as `(entry id, chip)`.
    pub followups: Vec<(String, String)>,
    /// Passages a grounded reply was told to answer from.
    pub citations: Vec<Citation>,
    /// The commit the corpus was read at.
    pub commit: Option<String>,
}

/// Adds a routed turn's fields to a `26900` result body.
pub fn annotate(result: &mut Value, served: &Served, bank: &Bank) {
    result["tier"] = json!(served.tier);
    result["route"] = json!(served.route);
    result["bank"] = json!(bank.id());
    if let Some(answer) = &served.answer {
        result["answer"] = json!(answer);
    }
    if let Some(model) = &served.model {
        result["model"] = json!(model);
    }
    if !served.followups.is_empty() {
        result["followups"] = served
            .followups
            .iter()
            .map(|(id, chip)| json!({ "id": id, "label": chip }))
            .collect();
    }
    if !served.citations.is_empty() {
        result["citations"] = json!(served.citations);
    }
    if let Some(commit) = &served.commit {
        result["commit"] = json!(commit);
    }
}

/// One routed turn in the worker's log. Every field is an id from the
/// bank, the route catalog, or the CLI group list, a probability, a tier
/// word, or a duration: there is no field that could hold message text.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shadow {
    pub set: &'static str,
    pub bank: String,
    pub mode: &'static str,
    pub route: &'static str,
    pub route_p: f64,
    pub answer: Option<String>,
    pub answer_p: f64,
    pub needs_specifics: f64,
    pub lane: &'static str,
    pub lane_p: f64,
    pub opener: Option<String>,
    pub opener_p: f64,
    pub cli_group: Option<String>,
    pub risk: &'static str,
    pub risk_p: f64,
    /// What the router decided.
    pub decided: &'static str,
    /// What the turn served (differs in shadow and legacy modes).
    pub served: &'static str,
    pub judge_ms: u64,
}

impl Shadow {
    /// The record for one judged turn.
    #[must_use]
    pub fn of(
        routing: &Routing,
        bank: &Bank,
        mode: Mode,
        shadow: bool,
        decided: &Tier,
        served: &Tier,
        judge_ms: u64,
    ) -> Self {
        Self {
            set: SET,
            bank: bank.id(),
            mode: match (mode, shadow) {
                (_, true) => "shadow",
                (Mode::Router, false) => "router",
                (Mode::Legacy, false) => "legacy",
            },
            route: routing.route.word(),
            route_p: routing.route_p,
            answer: routing.answer.as_ref().map(|(entry, _)| entry.tag()),
            answer_p: routing.answer.as_ref().map_or(0.0, |(_, p)| *p),
            needs_specifics: routing.needs_specifics,
            lane: routing.lane.word(),
            lane_p: routing.lane_p,
            opener: routing.opener.as_ref().map(|(opener, _)| opener.id.clone()),
            opener_p: routing.opener.as_ref().map_or(0.0, |(_, p)| *p),
            cli_group: routing.cli_group.as_ref().map(|(group, _)| group.clone()),
            risk: routing.risk.word(),
            risk_p: routing.risk_p,
            decided: decided.word(),
            served: served.word(),
            judge_ms,
        }
    }

    /// The log line: `router ` and the record as one JSON object.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "router {}",
            serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::bank::Facts;
    use crate::router::{Context, Offer, Risk, RouteId, Screen, Situation, decide};
    use serde_json::Value;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/nip-cj");

    fn fixture(name: &str) -> Value {
        let text = std::fs::read_to_string(format!("{FIXTURES}/{name}")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The bank's digest moves with every reviewed text change, so the
    /// fixtures name it as `chat-answers-v1@DIGEST`.
    fn undigested(mut body: Value) -> Value {
        if body["bank"].is_string() {
            body["bank"] = json!("chat-answers-v1@DIGEST");
        }
        body
    }

    fn facts() -> Facts {
        crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
            Some((6, 40)),
            &crate::router::Seams::default(),
        )
    }

    fn model_question() -> Routing {
        let bank = Bank::builtin();
        Routing {
            action: Route::Respond,
            route: RouteId::Meta,
            route_p: 0.93,
            runner_up: Some((RouteId::General, 0.05)),
            clarify_p: 0.0,
            answer: bank.entry("meta.model").map(|entry| (entry.clone(), 0.88)),
            needs_specifics: 0.07,
            lane: crate::first::Lane::Chat,
            lane_p: 0.97,
            opener: None,
            cli_group: None,
            cli_alternatives: Vec::new(),
            risk: Risk::Ok,
            risk_p: 0.99,
        }
    }

    /// The fixtures in `crates/coder/fixtures/nip-cj` are exactly what the
    /// router writes: the request a phone sends, the judgment, the three
    /// offers, and a canned result.
    #[test]
    fn the_wire_matches_the_nip_cj_fixtures() {
        let request = fixture("router-request.json");
        assert_eq!(request["router"], crate::router::SET);
        let context = Context::of(&request["context"]);
        assert_eq!(context.computer_ready, Some(false));

        let bank = Bank::builtin();
        let routing = model_question();
        let tier = decide(
            &routing,
            bank,
            &facts(),
            &Situation {
                mode: Mode::Router,
                context: &context,
                personalize: false,
            },
        );
        assert_eq!(
            undigested(judgment(2, &routing, &tier, bank, None)),
            fixture("router-judgment.json")
        );

        let mut result = json!({
            "v": 2, "type": "result", "text": line(&routing, &tier), "usage": null,
            "model": crate::generate::Lane::Gemini.model(),
        });
        let entry = tier.answer().unwrap();
        annotate(
            &mut result,
            &Served {
                tier: tier.word(),
                route: routing.route.word(),
                answer: Some(entry.tag()),
                model: Some("bank:chat-answers-v1".into()),
                followups: bank.followups(entry, &facts()),
                ..Served::default()
            },
            bank,
        );
        assert_eq!(undigested(result), fixture("router-result-canned.json"));

        assert_eq!(
            Offer::RunCoder {
                label: "Run Coder".into()
            }
            .feedback(2),
            fixture("router-offer-run-coder.json")
        );
        assert_eq!(
            Offer::OpenScreen {
                screen: Screen::AccountComputers,
                label: "Connect a computer".into()
            }
            .feedback(2),
            fixture("router-offer-open-screen.json")
        );
        assert_eq!(
            Offer::Cli {
                argv: vec!["computer".into(), "list".into()],
                effect: crate::router::Effect::ReadOnly,
                runs_on: crate::router::RunsOn::ThisDevice,
            }
            .feedback(2),
            fixture("router-offer-cli.json")
        );
    }

    /// The shadow record holds ids, probabilities, tiers, and a duration:
    /// its keys are fixed, and no value can be message text.
    #[test]
    fn the_shadow_record_never_carries_message_text() {
        let bank = Bank::builtin();
        let routing = model_question();
        let tier = decide(
            &routing,
            bank,
            &facts(),
            &Situation {
                mode: Mode::Router,
                context: &Context::default(),
                personalize: false,
            },
        );
        let record = Shadow::of(&routing, bank, Mode::Router, false, &tier, &tier, 180);
        let value = serde_json::to_value(&record).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "set",
                "bank",
                "mode",
                "route",
                "route_p",
                "answer",
                "answer_p",
                "needs_specifics",
                "lane",
                "lane_p",
                "opener",
                "opener_p",
                "cli_group",
                "risk",
                "risk_p",
                "decided",
                "served",
                "judge_ms"
            ]
        );
        assert_eq!(value["answer"], "meta.model@1");
        assert_eq!(value["decided"], "canned");
        assert!(record.line().starts_with("router {"));
        // Every string value is a code-listed id: the bank's, a route, a
        // lane, a risk, a tier, or the set.
        for (key, value) in value.as_object().unwrap() {
            if let Some(text) = value.as_str() {
                assert!(
                    text.len() < 64 && !text.contains(' '),
                    "{key} = {text} looks like prose"
                );
            }
        }
    }
}
