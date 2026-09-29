//! The descent end to end, against a loopback judge that answers each
//! request from a script, and a model that answers with fixed text.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;
use crate::router::Surface;

/// A judge on loopback that answers the `n`th request with `answers[n]`
/// and records each request body.
fn judge(answers: Vec<Value>) -> (jev::Client, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for answer in answers {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let answer = spread(answer, &request);
            record.lock().unwrap().push(request);
            let reply = json!({ "model": "jev-test", "answers": answer }).to_string();
            let mut stream = reader.into_inner();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
                 connection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    (
        jev::Client::new(jev::Config::local(url, "jev-test")).unwrap(),
        seen,
    )
}

/// Give every option the request lists a probability: the scripted
/// ones as scripted, the rest an even share of what is left.
fn spread(mut answers: Value, request: &Value) -> Value {
    let Some(map) = answers.as_object_mut() else {
        return answers;
    };
    for (id, answer) in map.iter_mut() {
        let Some(criteria) = request["questions"][id.as_str()]["criteria"].as_object() else {
            continue;
        };
        let given = answer["probabilities"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let used: f64 = criteria
            .keys()
            .filter_map(|key| given.get(key).and_then(Value::as_f64))
            .sum();
        let rest = criteria
            .keys()
            .filter(|key| !given.contains_key(*key))
            .count();
        let share = if rest == 0 {
            0.0
        } else {
            (1.0 - used).max(0.0) / rest as f64
        };
        let probabilities: serde_json::Map<String, Value> = criteria
            .keys()
            .map(|key| {
                let p = given.get(key).and_then(Value::as_f64).unwrap_or(share);
                (key.clone(), json!(p))
            })
            .collect();
        answer["probabilities"] = Value::Object(probabilities);
    }
    answers
}

/// A choice answer for question `id` that is sure of `choice`.
fn sure(id: &str, choice: &str) -> Value {
    json!({ id: {
        "type": "choice", "choice": choice, "confidence": 0.95,
        "probabilities": { choice: 0.95 },
    }})
}

/// A model that answers every fill with `text`, counting calls.
struct Canned {
    text: String,
    calls: Mutex<usize>,
}

impl Fill for Canned {
    fn fill<'a>(&'a self, _: &'a str, _: &'a [Message]) -> BoxFuture<'a, Result<String, String>> {
        *self.calls.lock().unwrap() += 1;
        Box::pin(async move { Ok(self.text.clone()) })
    }

    fn recipients(&self) -> Vec<String> {
        vec!["test model".into()]
    }
}

fn canned(text: &str) -> Arc<Canned> {
    Arc::new(Canned {
        text: text.into(),
        calls: Mutex::new(0),
    })
}

fn ask(group: &str, message: &str, surface: Surface) -> CliAsk {
    CliAsk {
        also: Vec::new(),
        group: group.into(),
        message: message.into(),
        transcript: Vec::new(),
        surface,
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(' ').map(str::to_owned).collect()
}

#[tokio::test]
async fn a_phone_listing_runs_on_the_phone() {
    let (jev, seen) = judge(vec![sure(descend::LEVEL_QUESTION, "list")]);
    let route = CommandRoute::new(jev, Arc::new(NoFill));
    let outcome = route
        .outcome(&ask(
            "computer",
            "which of my computers are online",
            Surface::Phone,
        ))
        .await
        .unwrap();
    let Outcome::Proposal {
        argv,
        effect,
        runs_on,
        execution,
        ..
    } = &outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(argv, &words("computer list"));
    assert_eq!(*effect, Effect::ReadOnly);
    assert_eq!(*runs_on, RunsOn::ThisDevice);
    assert_eq!(
        execution,
        &Some(Execution::Device {
            argv: words("computer list")
        })
    );
    // The level's options are exactly the group's commands plus `none`.
    let sent = seen.lock().unwrap();
    let options = sent[0]["questions"][descend::LEVEL_QUESTION]["criteria"]
        .as_object()
        .unwrap();
    assert!(options.contains_key("list") && options.contains_key("none"));
    assert_eq!(
        options.len(),
        tree::bundled().group("computer").unwrap().children.len() + 1
    );
    assert!(matches!(outcome.answer(), CliAnswer::Proposal(_)));
}

#[tokio::test]
async fn free_text_is_written_by_the_model_and_checked() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "search")]);
    let model = canned(r#"{"TEXT": "docker cp", "--limit": null}"#);
    let route = CommandRoute::new(jev, model.clone());
    let outcome = route
        .outcome(&ask(
            "kb",
            "search the knowledge base for docker cp",
            Surface::Phone,
        ))
        .await
        .unwrap();
    let Outcome::Proposal {
        argv, execution, ..
    } = &outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(argv, &["kb", "search", "docker cp"]);
    assert_eq!(
        execution,
        &Some(Execution::Computer {
            command: vec![
                "openagents".into(),
                "--json".into(),
                "kb".into(),
                "search".into(),
                "docker cp".into()
            ]
        })
    );
    assert_eq!(*model.calls.lock().unwrap(), 1);
    // The evidence names the command, never the search text.
    assert!(!outcome.evidence().to_string().contains("docker"));
}

#[tokio::test]
async fn money_is_never_proposed_and_costs_no_model_call() {
    for surface in [Surface::Phone, Surface::Desktop, Surface::Terminal] {
        let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "pay")]);
        let model = canned("{}");
        let route = CommandRoute::new(jev, model.clone());
        let outcome = route
            .outcome(&ask("wallet", "pay this invoice lnbc1...", surface))
            .await
            .unwrap();
        assert!(
            matches!(
                &outcome,
                Outcome::NotOffered {
                    effect: Effect::Spends,
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(outcome.answer(), CliAnswer::NoCommand);
        assert_eq!(*model.calls.lock().unwrap(), 0);
    }
}

#[tokio::test]
async fn the_phone_is_not_offered_what_is_off_its_list() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "say")]);
    let route = CommandRoute::new(jev, canned(r#"{"TEXT": "hello"}"#));
    let outcome = route
        .outcome(&ask("verse", "say hello in the plaza", Surface::Phone))
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        Outcome::NotOffered {
            effect: Effect::Publishes,
            ..
        }
    ));
    // In the terminal it is offered; `--to` is an enum, so it is selected.
    let (jev, _) = judge(vec![
        sure(descend::LEVEL_QUESTION, "say"),
        sure("--to", "none"),
    ]);
    let route = CommandRoute::new(jev, canned(r#"{"TEXT": "hello"}"#));
    let outcome = route
        .outcome(&ask("verse", "say hello in the plaza", Surface::Terminal))
        .await
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Proposal { argv, .. } if argv == &["verse", "say", "hello"]),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn none_or_an_unsure_level_ends_the_descent() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "none")]);
    let route = CommandRoute::new(jev, Arc::new(NoFill));
    let outcome = route
        .outcome(&ask("computer", "tell me a joke", Surface::Phone))
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::NoCommand { .. }));
    let unsure = json!({ descend::LEVEL_QUESTION: {
        "type": "choice", "choice": "list", "confidence": 0.2,
        "probabilities": { "list": 0.4, "show": 0.35, "none": 0.25 },
    }});
    let (jev, _) = judge(vec![unsure]);
    let route = CommandRoute::new(jev, Arc::new(NoFill));
    let outcome = route
        .outcome(&ask("computer", "computers?", Surface::Phone))
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::NoCommand { .. }));
    let outcome = route
        .outcome(&ask("fridge", "open the fridge", Surface::Phone))
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::NoCommand { .. }));
}

#[tokio::test]
async fn a_host_is_selected_from_the_devices_computers_or_asked_for() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "show")]);
    let route = CommandRoute::new(jev, Arc::new(NoFill));
    let outcome = route
        .outcome(&ask("computer", "show me my studio", Surface::Phone))
        .await
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Missing { what, .. } if what == "which computer"),
        "{outcome:?}"
    );
    assert_eq!(
        outcome.answer(),
        CliAnswer::Missing("which computer".to_string())
    );

    let (jev, seen) = judge(vec![
        sure(descend::LEVEL_QUESTION, "show"),
        sure("HOST", "c1"),
    ]);
    let hosts = vec![
        Host {
            id: "laptop".into(),
            label: "MacBook".into(),
            workspaces: vec![],
        },
        Host {
            id: "studio".into(),
            label: "Mac Studio".into(),
            workspaces: vec![],
        },
    ];
    let route =
        CommandRoute::new(jev, Arc::new(NoFill)).with_hosts(Arc::new(move || hosts.clone()));
    let outcome = route
        .outcome(&ask("computer", "show me my studio", Surface::Desktop))
        .await
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Proposal { argv, runs_on: RunsOn::ThisDevice, .. } if argv == &words("computer show studio")),
        "{outcome:?}"
    );
    let sent = seen.lock().unwrap();
    assert!(
        sent[1]["questions"]["HOST"]["criteria"]["c1"]
            .to_string()
            .contains("Mac Studio")
    );
}

#[tokio::test]
async fn a_node_that_is_also_a_command_can_be_chosen_itself() {
    let (jev, _) = judge(vec![
        sure(descend::LEVEL_QUESTION, "xp"),
        sure(descend::LEVEL_QUESTION, descend::SELF),
    ]);
    let route = CommandRoute::new(jev, Arc::new(NoFill));
    let outcome = route
        .outcome(&ask("verse", "show my XP", Surface::Phone))
        .await
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Proposal { argv, runs_on: RunsOn::ConnectedComputer, .. } if argv == &words("verse xp")),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_fill_that_does_not_parse_asks_rather_than_guesses() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "search")]);
    let route = CommandRoute::new(jev, canned("I think you want docker"));
    let outcome = route
        .outcome(&ask("kb", "search the kb", Surface::Phone))
        .await
        .unwrap();
    assert!(
        matches!(&outcome, Outcome::Missing { what, .. } if what == "what to search for"),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn the_seam_lists_every_group_and_its_recipients() {
    let (jev, _) = judge(vec![sure(descend::LEVEL_QUESTION, "who")]);
    let route: Arc<dyn CliRoute> = Arc::new(CommandRoute::new(jev, canned("{}")));
    let groups = route.groups();
    assert_eq!(groups.len(), tree::bundled().groups.len());
    assert!(groups.iter().any(|g| g.id == "x402"));
    assert_eq!(route.recipients(), ["test model"]);
    let answer = route
        .propose(&ask("verse", "who is around", Surface::Phone))
        .await
        .unwrap();
    assert_eq!(
        answer,
        CliAnswer::Proposal(CliProposal {
            argv: words("verse who"),
            effect: Effect::ReadOnly,
            runs_on: RunsOn::ConnectedComputer,
        })
    );
}
