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

use super::bank::Bank;
use super::judge::Routing;
use super::policy::{Mode, Tier};
use super::seams::Passage;
use super::set_id;
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
/// `lane_p`, `risk`, `risk_p`, `cli_group`, `tool`, `capability` (the
/// admitted capability the turn calls for, an id from the typed set, or
/// null), `capability_p`, and `capability_missing_p` are the router's.
/// `set` is the question set's identity, `chat-router-v5@<digest>`
/// ([`set_id`]), whichever set the request named, and `bank` the bank's,
/// so a judgment names the exact question and answers it was decided
/// with, as an eval report pins them (#9959). In shadow mode `tier` is
/// what was served and `shadow` names what the router would have served.
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
        "set": set_id(),
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
        "tool": routing.tool.as_ref().map(|(tool, _)| tool.clone()),
        "capability": routing.capability.as_ref().map(|(entry, _)| entry.id.clone()),
        "capability_p": routing.capability.as_ref().map_or(0.0, |(_, p)| *p),
        "capability_missing_p": routing.capability_missing_p,
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

impl From<&super::gym::Item> for Citation {
    fn from(item: &super::gym::Item) -> Self {
        Self {
            id: item.id(),
            title: item.kind().to_string(),
            source: item.source().cite(),
        }
    }
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
    /// The admitted capability that answered, by its id in the typed
    /// set, when the reading named one at the policy's confidence.
    pub capability: Option<String>,
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
    /// The plugin-creation step the turn served (#10177): the result's
    /// typed `plugin` field (`openagents_chat::plugin_flow::Flow::wire`).
    pub plugin: Option<Value>,
    /// The catalog plugins the answer shows as cards, by package slug
    /// (`docs/web/plugin-card.md`): set only from a bank entry's
    /// [`super::Entry::plugins`] and the compiled-in catalog, never from
    /// the message or a model.
    pub plugins: Vec<String>,
}

/// Adds a routed turn's fields to a `26900` result body.
pub fn annotate(result: &mut Value, served: &Served, bank: &Bank) {
    result["tier"] = json!(served.tier);
    result["route"] = json!(served.route);
    result["bank"] = json!(bank.id());
    if let Some(answer) = &served.answer {
        result["answer"] = json!(answer);
    }
    if let Some(capability) = &served.capability {
        result["capability"] = json!(capability);
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
    if let Some(plugin) = &served.plugin {
        result["plugin"] = plugin.clone();
    }
    if !served.plugins.is_empty() {
        result["plugins"] = json!(served.plugins);
    }
}

/// One routed turn in the worker's log. Every field is an id from the
/// bank, the route catalog, the CLI group list, the tool catalog, or the
/// admitted-capability set, a probability, a tier word, or a duration:
/// there is no field that could hold message text.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shadow {
    /// The question set, `chat-router-v2@<digest>`.
    pub set: String,
    pub bank: String,
    /// The calibration map the probabilities went through
    /// (`calibration-v2@<digest>`), or `None` for raw.
    pub calibration: Option<String>,
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
    /// The `tool` reading's argmax, a catalog id.
    pub tool: Option<String>,
    /// The `capability` reading's argmax when it is an admitted entry, by
    /// its id in the typed set.
    pub capability: Option<String>,
    pub capability_p: f64,
    pub capability_missing_p: f64,
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
            set: set_id(),
            bank: bank.id(),
            calibration: None,
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
            tool: routing.tool.as_ref().map(|(tool, _)| tool.clone()),
            capability: routing
                .capability
                .as_ref()
                .map(|(entry, _)| entry.id.clone()),
            capability_p: routing.capability.as_ref().map_or(0.0, |(_, p)| *p),
            capability_missing_p: routing.capability_missing_p,
            risk: routing.risk.word(),
            risk_p: routing.risk_p,
            decided: decided.word(),
            served: served.word(),
            judge_ms,
        }
    }

    /// The record with the calibration map the reading went through
    /// named, when one did.
    #[must_use]
    pub fn calibrated(mut self, map: Option<&super::calibration::Calibration>) -> Self {
        self.calibration = map.map(super::calibration::Calibration::id);
        self
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

    /// The bank's digest moves with every reviewed text change and the
    /// set's with every change to the route question, so the fixtures name
    /// them as `chat-answers-v1@DIGEST` and `chat-router-v5@DIGEST`.
    fn undigested(mut body: Value) -> Value {
        if body["bank"].is_string() {
            body["bank"] = json!("chat-answers-v1@DIGEST");
        }
        if body["set"].is_string() {
            assert_eq!(body["set"], set_id());
            body["set"] = json!("chat-router-v5@DIGEST");
        }
        body
    }

    /// The wire names the question set by its digest, the digest is the
    /// Gym's for the committed question file, and it moves when the route
    /// question does.
    #[test]
    fn the_set_is_named_with_its_digest() {
        let id = set_id();
        assert!(id.starts_with("chat-router-v5@"), "{id}");
        assert_eq!(id.len(), "chat-router-v5@".len() + 12);
        assert!(crate::router::set_digest().starts_with(&id["chat-router-v5@".len()..]));
        let committed = ::gym::questions::load(crate::router_eval::SUITE_QUESTIONS)
            .expect("the committed question set reads");
        assert_eq!(committed.digest(), crate::router::set_digest());
    }

    fn facts() -> Facts {
        crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
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
            tool: None,
            capability: None,
            capability_missing_p: 0.02,
            capability_closest: None,
            deck: None,
            engine: None,
            fanout: None,
            read_only: 0.0,
            summarize: 0.0,
            risk: Risk::Ok,
            risk_p: 0.99,
            answer_calibrated: false,
        }
    }

    /// The fixtures in `crates/coder/fixtures/nip-cj` are exactly what the
    /// router writes: the request a phone sends, the judgment, the three
    /// offers, and a canned result.
    #[test]
    fn the_wire_matches_the_nip_cj_fixtures() {
        let request = fixture("router-request.json");
        assert_eq!(
            request["router"],
            crate::router::SET_V1,
            "build 20's request"
        );
        assert!(crate::router::asks_router(&request["router"]));
        let v2 = fixture("router-request-v2.json");
        assert_eq!(v2["router"], crate::router::SET_V2, "build 21's request");
        assert!(crate::router::asks_router(&v2["router"]));
        assert!(crate::router::asks_router(&json!(crate::router::SET)));
        assert!(crate::router::card::draft(&v2["draft"]).is_ok());
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
                draft: false,
                earlier: false,
                plugin: false,
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
                label: "Run Coder".into(),
                engine: None,
                plan: Default::default(),
            }
            .feedback(2)
            .unwrap(),
            fixture("router-offer-run-coder.json")
        );
        assert_eq!(
            Offer::OpenScreen {
                screen: Screen::AccountComputers,
                label: "Connect a computer".into()
            }
            .feedback(2)
            .unwrap(),
            fixture("router-offer-open-screen.json")
        );
        assert_eq!(
            Offer::Cli {
                argv: vec!["computer".into(), "list".into()],
                effect: crate::router::Effect::ReadOnly,
                runs_on: crate::router::RunsOn::ThisDevice,
            }
            .feedback(2)
            .unwrap(),
            fixture("router-offer-cli.json")
        );
        assert_eq!(
            Offer::OpenPresentation {
                deck: "three-devdays-later".into(),
                label: "Open Three DevDays Later".into()
            }
            .feedback(2)
            .unwrap(),
            fixture("router-offer-open-presentation.json")
        );
    }

    /// The Gym and eval wire: each card and offer the router writes, built
    /// from the test records, is its fixture, and NIP-CJ's parser reads it
    /// back. `ROUTER_FIXTURES_WRITE=1` rewrites them.
    #[test]
    fn the_eval_wire_matches_its_fixtures() {
        use crate::router::card::{Award, Card};
        use crate::router::gym::Item;
        use crate::router::gym::fixtures::{event, records, result, suite, tool};
        use nostr::cj_conversation::{Size, SubjectSource, SuiteSource, Where};
        let records = records();
        let mut check = result(12, "project-map", 1);
        check.checks = Some(event(10, 3189).id);
        let draft: Value = serde_json::from_str(include_str!(
            "../../../nostr/fixtures/eval-ext/eval-draft/valid/chat-made-tool.json"
        ))
        .unwrap();
        let cards = [
            (
                "router-card-tool.json",
                Card::Tool {
                    tool: tool("project-map", "Project map"),
                    latest: Some(result(10, "project-map", 1)),
                    subject: Some(suite(20, "project-map", 8).subject),
                },
            ),
            (
                "router-card-result.json",
                Card::Result {
                    result: result(10, "project-map", 1),
                },
            ),
            (
                "router-card-check.json",
                Card::Check {
                    result: result(10, "project-map", 1),
                },
            ),
            (
                "router-card-news.json",
                Card::News {
                    items: vec![
                        Item::Result(check),
                        Item::TestSet(suite(20, "project-map", 8)),
                        Item::Build(records.releases[0].clone()),
                    ],
                },
            ),
            ("router-card-draft.json", Card::Draft { draft }),
            (
                "router-card-capability.json",
                Card::Capability {
                    closest: Some(crate::router::capability::of_tool(&tool(
                        "project-map",
                        "Project map",
                    ))),
                    add: nostr::cj_conversation::Add::Author,
                },
            ),
            (
                "router-card-credit.json",
                Card::Credit {
                    awards: vec![Award {
                        role: "author".into(),
                        xp: 25,
                        title: "Your Project map test set was checked".into(),
                        award: Some(event(30, 3193)),
                    }],
                },
            ),
        ];
        let offers = [
            (
                "router-offer-start-eval.json",
                Offer::StartEval {
                    suite: SuiteSource::Published(event(20, 3184)),
                    subject: SubjectSource::Definition(Box::new(
                        suite(20, "project-map", 8).subject,
                    )),
                    size: Size {
                        cases: 8,
                        runs: 3,
                        arms: 2,
                    },
                    at: Where::Hosted,
                    label: "Start the test".into(),
                },
            ),
            (
                "router-offer-publish-eval.json",
                Offer::PublishEval {
                    report: result(10, "project-map", 1).report,
                    label: "Add to the Gym".into(),
                },
            ),
            (
                "router-offer-open-gym-result.json",
                Offer::OpenScreen {
                    screen: Screen::GymResult,
                    label: "See your result".into(),
                },
            ),
        ];
        let bodies = cards
            .iter()
            .map(|(name, card)| (*name, card.feedback(2).unwrap()))
            .chain(
                offers
                    .iter()
                    .map(|(name, offer)| (*name, offer.feedback(2).unwrap())),
            );
        for (name, body) in bodies {
            if std::env::var_os("ROUTER_FIXTURES_WRITE").is_some() {
                std::fs::write(
                    format!("{FIXTURES}/{name}"),
                    serde_json::to_string_pretty(&body).unwrap() + "\n",
                )
                .unwrap();
            }
            assert_eq!(body, fixture(name), "{name}");
            if body["type"] == "card" {
                nostr::cj_conversation::parse_card(&body).expect(name);
            } else {
                nostr::cj_conversation::parse_offer(&body).expect(name);
            }
        }
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
                draft: false,
                earlier: false,
                plugin: false,
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
                "calibration",
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
                "tool",
                "capability",
                "capability_p",
                "capability_missing_p",
                "risk",
                "risk_p",
                "decided",
                "served",
                "judge_ms"
            ]
        );
        assert_eq!(value["answer"], "meta.model@3");
        assert_eq!(value["decided"], "canned");
        assert_eq!(value["set"], set_id());
        assert_eq!(value["calibration"], Value::Null);
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
