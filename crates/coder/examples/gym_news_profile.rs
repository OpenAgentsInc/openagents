//! Profiles a `gym.news` turn's stages on a host with the chat worker's
//! environment: the message embedding, the relevance judgment, and the
//! grounded model's first words and end at several reasoning settings.
//! Prints stage times only, never message or reply text.

use std::sync::Arc;
use std::time::Instant;

use coder::generate::{Generate, Message, ResponsesDoor, Role};
use coder::router::RouteId;
use coder::router::seams::GymLookup;

const BASE: &str = "We are OpenAgents, chatting with the user in the OpenAgents app on their \
phone. Always speak as \"we\" and \"us\", never \"I\" or \"me\". Answer directly and helpfully \
in our own words; use Markdown when it helps, and keep answers short on a small screen.";

#[tokio::main]
async fn main() -> Result<(), String> {
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(5);
    let judge: Arc<dyn coder::product_kb::Judge> =
        Arc::new(coder::decision::from_env()?.ok_or("no judge")?);
    let gym = coder::gym_kb::GymKnowledge::from_env(judge)?;
    let secret = std::env::var("CODER_WORKER_SECRET").map_err(|_| "no secret")?;
    let identity = coder::relay::Identity::from_text(&secret, "CODER_WORKER_SECRET")?;
    let t = Instant::now();
    let admitted = gym.refresh("wss://relay.openagents.com", &identity).await?;
    eprintln!(
        "refresh {} ms: {} results, {} suites",
        t.elapsed().as_millis(),
        admitted.results.len(),
        admitted.suites.len()
    );
    let t = Instant::now();
    gym.warm().await.map_err(|e| format!("{e:?}"))?;
    eprintln!(
        "warm {} ms, {} items",
        t.elapsed().as_millis(),
        gym.records().items().len()
    );
    let door = ResponsesDoor::from_env()
        .ok_or("no door")?
        .serving("gemini");
    let messages = [
        "What's new in the Gym?",
        "Anything new in the gym lately?",
        "What's happening in the Gym this week?",
        "Any new results for Project map?",
        "what changed in the gym",
    ];
    let spec = std::env::var("VARIANTS").unwrap_or_else(|_| "gemini:default,gemini:low".into());
    let efforts: Vec<(String, Option<String>)> = spec
        .split(',')
        .map(|v| {
            let (m, e) = v.rsplit_once(':').unwrap();
            (m.to_string(), (e != "default").then(|| e.to_string()))
        })
        .collect();
    for n in 0..runs {
        let message = messages[n % messages.len()];
        let lookup = GymLookup {
            route: RouteId::GymNews,
            message: message.to_string(),
            transcript: vec![Message {
                role: Role::User,
                text: message.to_string(),
            }],
        };
        let t = Instant::now();
        let items = gym.records().items();
        let _ = gym
            .candidates(message, &items)
            .await
            .map_err(|e| format!("{e:?}"))?;
        let embed = t.elapsed().as_millis();
        let t = Instant::now();
        let news = gym.news(&lookup).await.map_err(|e| format!("{e:?}"))?;
        let news_ms = t.elapsed().as_millis();
        let items: Vec<_> = news.into_iter().map(|(i, _)| i).collect();
        let instructions = format!(
            "{BASE}\n\n{}",
            coder::router::gym::instructions(&items, Some("Here's what's new in the Gym."))
        );
        let mut line = format!(
            "run {n}: embed {embed} ms, news(embed+judge) {news_ms} ms, {} items, prompt {} chars",
            items.len(),
            instructions.len()
        );
        for (model, effort) in &efforts {
            let door = door.clone().serving(model);
            let door = match effort {
                None => door,
                Some(e) => {
                    let mut options = serde_json::Map::new();
                    options.insert("reasoning".into(), serde_json::json!({ "effort": e }));
                    door.with_options(options)
                }
            };
            let t = Instant::now();
            let mut first: Option<u128> = None;
            let mut sink = |_: &str| {
                if first.is_none() {
                    first = Some(t.elapsed().as_millis());
                }
            };
            let answered = door
                .generate(&instructions, &lookup.transcript, &mut sink, &mut |_| {})
                .await;
            let done = t.elapsed().as_millis();
            let chars = answered.as_ref().map(|(text, _)| text.len()).unwrap_or(0);
            let quality = answered
                .as_ref()
                .map(|(text, _)| {
                    let cited = coder::router::gym::check_reply(text, &items);
                    let shown = coder::router::gym::tidy(text);
                    let check = coder::router::gym::post_check(&shown);
                    if std::env::var("SHOW").is_ok() {
                        eprintln!("---- {model}:{effort:?}\n{shown}\n----");
                    }
                    format!(
                        " cited {} invented {} banned {:?} raw {}",
                        cited.known.len(),
                        cited.invented.len(),
                        check.banned,
                        check.raw.len()
                    )
                })
                .unwrap_or_default();
            let err = answered
                .err()
                .map(|e| format!(" ERR {e}"))
                .unwrap_or_default();
            line.push_str(&format!(
                " | {model}:{}: first {:?} done {done} ({chars} chars){quality}{err}",
                effort.as_deref().unwrap_or("default"),
                first
            ));
        }
        println!("{line}");
    }
    Ok(())
}
