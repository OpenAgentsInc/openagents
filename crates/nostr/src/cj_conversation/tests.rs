//! Conversation offers, cards, and drafts: every builder round-trips
//! through its parser, cards and drafts match their schemas, and anything
//! a client doesn't know refuses.

use serde_json::{Value, json};

use super::*;
use crate::eval_ext::tests::{art, code, id, pubkey, schema_check};

fn catalog() -> DefinitionRef {
    parse_definition(&json!({
        "id": format!("{}:project-map/map", pubkey("ext-author")),
        "artifact": art(b"project map program", "application/json", None),
        "event": {"id": id("subject-release"), "pubkey": pubkey("ext-author"), "kind": 3184},
    }))
    .unwrap()
}

fn report_ref() -> ArtifactRef {
    parse_artifact(&art(
        b"{}",
        "application/json",
        Some(crate::kb::REPORT_SCHEMA),
    ))
    .unwrap()
}

fn pointer(label: &str, kind: u16) -> EventPointer {
    EventPointer {
        id: id(label),
        pubkey: pubkey(label),
        kind,
    }
}

pub(crate) fn draft() -> Draft {
    Draft {
        tool: DraftTool {
            name: "Tidy imports".into(),
            summary: "Keeps Rust imports grouped and sorted.".into(),
            catalog: None,
            skill: Some("Group std, external, then crate imports.".into()),
            uses: vec![format!("{}:project-map/map", pubkey("ext-author"))],
        },
        cases: vec![
            DraftCase {
                id: "sort-imports".into(),
                kind: CaseKind::ShouldFire,
                prompt: "+++\nv = \"openagents.eval-case.v1\"\n+++\n\nClean up main.rs.\n".into(),
                graders: vec![(
                    "criteria".into(),
                    "+++\ntype = \"decision\"\n+++\n\nSorted.\n".into(),
                )],
            },
            DraftCase {
                id: "say-hello".into(),
                kind: CaseKind::ShouldNotFire,
                prompt: "+++\nv = \"openagents.eval-case.v1\"\n+++\n\nSay hello.\n".into(),
                graders: vec![(
                    "criteria".into(),
                    "+++\ntype = \"decision\"\n+++\n\nHello.\n".into(),
                )],
            },
        ],
    }
}

fn offers() -> Vec<Offer> {
    vec![
        Offer::RunCoder {
            label: "Run on Studio Mac".into(),
        },
        Offer::OpenScreen {
            screen: Screen::GymResult,
            label: "See details".into(),
        },
        Offer::OpenScreen {
            screen: Screen::AccountComputers,
            label: "Connect a computer".into(),
        },
        Offer::Cli {
            argv: vec!["ext".into(), "list".into()],
            effect: Effect::ReadOnly,
            runs_on: RunsOn::ThisDevice,
        },
        Offer::StartEval {
            suite: SuiteSource::Published(pointer("suite", kinds::EXT_RELEASE)),
            subject: SubjectSource::Definition(Box::new(catalog())),
            size: Size {
                cases: 8,
                runs: 3,
                arms: 2,
            },
            at: Where::Hosted,
            label: "Start the test".into(),
        },
        Offer::StartEval {
            suite: SuiteSource::Draft,
            subject: SubjectSource::Draft,
            size: Size {
                cases: 2,
                runs: 1,
                arms: 2,
            },
            at: Where::ConnectedComputer,
            label: "Try it once".into(),
        },
        Offer::PublishEval {
            report: report_ref(),
            label: "Add to the Gym".into(),
        },
    ]
}

fn line() -> ResultLine {
    ResultLine {
        publication: pointer("result", kinds::EVAL_DECLARATION),
        headline: Headline {
            subject_passed: 7,
            baseline_passed: Some(5),
            total: 8,
        },
        verdict: Verdict::Pass,
    }
}

fn cards() -> Vec<Card> {
    vec![
        Card::Tool {
            name: "Project map".into(),
            summary: "Maps a repository before Coder edits it.".into(),
            definition: Some(catalog()),
            latest: Some(line()),
        },
        Card::Draft(draft()),
        Card::Run {
            request: pointer("request", kinds::CJ_EXECUTION_REQUEST),
            at: Where::Hosted,
            completed: 12,
            planned: 48,
        },
        Card::Result {
            headline: line().headline,
            verdict: Verdict::Pass,
            report: report_ref(),
            publication: None,
        },
        Card::News(vec![
            NewsItem {
                title: "Project map checked".into(),
                line: "Two trainers confirmed its result.".into(),
                source: Source::Event(pointer("result", kinds::EVAL_DECLARATION)),
            },
            NewsItem {
                title: "Build 21".into(),
                line: "Chat is the main menu's first row.".into(),
                source: Source::Path("docs/changelog.md".into()),
            },
        ]),
        Card::Check {
            tool: "Project map".into(),
            line: line(),
            confirms: 2,
            disputes: 0,
        },
        Card::Credit {
            total: 50,
            awards: vec![
                CreditItem {
                    confirmed: true,
                    role: "checker".into(),
                    xp: 50,
                    title: "Check Project map".into(),
                    award: Some(pointer("award", kinds::XP_AWARD)),
                },
                CreditItem {
                    confirmed: false,
                    role: "evaluator".into(),
                    xp: 25,
                    title: "Check Project map".into(),
                    award: None,
                },
            ],
        },
    ]
}

#[test]
fn every_offer_round_trips() {
    for offer in offers() {
        for version in [1, 2] {
            let body = offer_feedback(&offer, version).unwrap();
            assert_eq!(body["offer"], offer.word());
            assert_eq!(parse_offer(&body).unwrap(), (version, offer.clone()));
        }
    }
}

#[test]
fn every_card_round_trips_and_matches_its_schema() {
    for card in cards() {
        let body = card_feedback(&card, 2).unwrap();
        assert_eq!(parse_card(&body).unwrap(), (2, card.clone()));
        schema_check("cj-card.v1.json", &body);
    }
    assert_eq!(CARDS.len(), cards().len());
}

#[test]
fn a_draft_round_trips_and_matches_its_schema() {
    let value = draft_value(&draft()).unwrap();
    assert_eq!(parse_draft(&value).unwrap(), draft());
    schema_check("eval-draft.v1.json", &value);
    let mut catalog_tool = draft();
    catalog_tool.tool.catalog = Some(catalog());
    catalog_tool.tool.skill = None;
    catalog_tool.tool.uses.clear();
    let value = draft_value(&catalog_tool).unwrap();
    schema_check("eval-draft.v1.json", &value);
}

#[test]
fn an_offer_the_client_doesnt_know_refuses() {
    let mut body = offer_feedback(&offers()[0], 2).unwrap();
    body["offer"] = json!("start_training");
    assert_eq!(code(parse_offer(&body)), RefusalCode::UnsupportedFeature);
    let mut screen = offer_feedback(&offers()[1], 2).unwrap();
    screen["screen"] = json!("gym.leaderboard");
    assert_eq!(code(parse_offer(&screen)), RefusalCode::UnsupportedFeature);
    let mut extra = offer_feedback(&offers()[0], 2).unwrap();
    extra["autostart"] = json!(true);
    assert_eq!(code(parse_offer(&extra)), RefusalCode::UnsupportedFeature);
    let mut version = offer_feedback(&offers()[0], 2).unwrap();
    version["v"] = json!(3);
    assert_eq!(code(parse_offer(&version)), RefusalCode::UnsupportedVersion);
    let mut unconfirmed = offer_feedback(&offers()[3], 2).unwrap();
    unconfirmed["confirm"] = json!(false);
    assert_eq!(code(parse_offer(&unconfirmed)), RefusalCode::Malformed);
}

#[test]
fn a_hosted_start_offer_stays_inside_the_runners_bounds() {
    let mut body = offer_feedback(&offers()[4], 2).unwrap();
    body["size"]["cases"] = json!(9);
    assert_eq!(code(parse_offer(&body)), RefusalCode::LimitExceeded);
    body["size"] = json!({"cases": 8, "runs": 4, "arms": 2});
    assert_eq!(code(parse_offer(&body)), RefusalCode::LimitExceeded);
    body["where"] = json!("connected_computer");
    parse_offer(&body).unwrap();
}

#[test]
fn an_unknown_card_type_refuses() {
    let mut body = card_feedback(&cards()[0], 2).unwrap();
    body["card"] = json!("leaderboard");
    assert_eq!(code(parse_card(&body)), RefusalCode::UnsupportedFeature);
    let mut news = card_feedback(&cards()[4], 2).unwrap();
    news["items"][0]["path"] = json!("docs/changelog.md");
    assert_eq!(code(parse_card(&news)), RefusalCode::Malformed);
    let mut credit = card_feedback(&cards()[6], 2).unwrap();
    credit["total"] = json!(75);
    assert_eq!(code(parse_card(&credit)), RefusalCode::IdentityMismatch);
    let mut run = card_feedback(&cards()[2], 2).unwrap();
    run["completed"] = json!(49);
    assert_eq!(code(parse_card(&run)), RefusalCode::Malformed);
}

#[test]
fn an_oversize_draft_refuses() {
    let mut big = draft();
    big.cases[0].prompt = "x".repeat(MAX_DRAFT_BYTES);
    assert_eq!(code(draft_value(&big)), RefusalCode::LimitExceeded);
    let mut value = draft_value(&draft()).unwrap();
    value["cases"][0]["prompt"] = json!("y".repeat(MAX_DRAFT_BYTES));
    assert_eq!(code(parse_draft(&value)), RefusalCode::LimitExceeded);
    let mut both = draft();
    both.tool.catalog = Some(catalog());
    assert_eq!(code(draft_value(&both)), RefusalCode::Malformed);
    let mut twice = draft();
    twice.cases[1].id = "sort-imports".into();
    assert_eq!(code(draft_value(&twice)), RefusalCode::Conflict);
    let _: Value = value;
}
