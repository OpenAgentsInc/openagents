//! Scores Jev, Kev, and Lev on the same items, through the same client.
//!
//! All three speak `POST /v1/systemone`, so one `crates/jev` client pointed
//! at three base URLs is the whole harness. That is the payoff of building a
//! third implementation of one contract rather than a third API.
//!
//! ```text
//! # hosted Jev reads TYPESAFE_API_KEY from the environment; never pass a key
//! # on the command line and never print one.
//! set -a; . ~/work/.secrets/typesafe.env; set +a
//! cargo run -p lev --bin lev-compare -- \
//!     --jev \
//!     --door kev=http://127.0.0.1:8009 \
//!     --door lev=http://127.0.0.1:11436
//! ```

use std::time::Instant;

use jev::{Choice, Client, Config, Questions, SystemOneRequest};

struct Item {
    label: &'static str,
    state: &'static str,
    truth: &'static str,
}

const DEPARTMENTS: [(&str, &str); 4] = [
    ("billing", "Charges, invoices, refunds, and payment problems"),
    ("technical", "Bugs, crashes, outages, and performance"),
    ("sales", "Quotes, plans, upgrades, and renewals"),
    ("other", "Anything the three above do not cover"),
];

const ITEMS: [Item; 8] = [
    Item { label: "easy/charge", state: "I was charged twice for the same order and want one refunded.", truth: "billing" },
    Item { label: "easy/crash", state: "The app crashes every time I open the settings screen.", truth: "technical" },
    Item { label: "easy/quote", state: "Can you send me a quote for 50 seats on the enterprise plan?", truth: "sales" },
    Item { label: "easy/offtopic", state: "Does anyone here know a good restaurant near your office?", truth: "other" },
    Item { label: "hard/upgrade", state: "My card was declined when the plan tried to upgrade itself, and now I cannot log in.", truth: "billing" },
    Item { label: "hard/seats", state: "We added seats last month and the invoice does not match what the sales rep quoted.", truth: "billing" },
    Item { label: "hard/slow", state: "Everything has felt slower since the update, but I am not sure if it is my laptop.", truth: "technical" },
    Item { label: "hard/renewal", state: "Our renewal is coming up and I want to talk through whether the higher tier is worth it.", truth: "sales" },
];

fn questions() -> Questions {
    let mut choice = Choice::default();
    for (name, description) in DEPARTMENTS {
        choice = choice.option(name, description);
    }
    Questions::new().with(
        "department",
        choice,
    )
}

struct Row {
    door: String,
    correct: usize,
    total: usize,
    mean_confidence: f64,
    mean_ms: f64,
    confident_errors: usize,
    answers: Vec<String>,
}

async fn run(door: &str, client: &Client) -> Row {
    let mut correct = 0;
    let mut confidences = Vec::new();
    let mut millis = Vec::new();
    let mut confident_errors = 0;
    let mut answers = Vec::new();

    for item in &ITEMS {
        let start = Instant::now();
        let outcome = client
            .system_one(SystemOneRequest::new(item.state, questions()))
            .await;
        let elapsed = start.elapsed().as_millis() as f64;
        match outcome.as_ref().map(|response| response.choice("department")) {
            Ok(Ok(answer)) => {
                let right = answer.choice == item.truth;
                if right {
                    correct += 1;
                } else if answer.confidence >= 0.9 {
                    confident_errors += 1;
                }
                confidences.push(answer.confidence);
                millis.push(elapsed);
                answers.push(format!("{}{}", answer.choice, if right { "" } else { "*" }));
            }
            Ok(Err(error)) => answers.push(format!("<{error}>")),
            Err(error) => answers.push(format!("<{error}>")),
        }
    }

    let mean = |values: &[f64]| {
        if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 }
    };
    Row {
        door: door.to_string(),
        correct,
        total: ITEMS.len(),
        mean_confidence: mean(&confidences),
        mean_ms: mean(&millis),
        confident_errors,
        answers,
    }
}

#[tokio::main]
async fn main() {
    let mut doors: Vec<(String, Client)> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--jev" => match Client::from_env() {
                Ok(client) => doors.push(("jev (hosted)".to_string(), client)),
                Err(error) => eprintln!("skipping hosted Jev: {error}"),
            },
            "--door" => {
                let Some(spec) = args.next() else { continue };
                let Some((name, url)) = spec.split_once('=') else {
                    eprintln!("--door takes name=url, got {spec}");
                    continue;
                };
                let config = Config::new()
                    .api_key("unused-by-a-local-door")
                    .base_url(url.to_string())
                    .default_model(name.to_string());
                match Client::new(config) {
                    Ok(client) => doors.push((name.to_string(), client)),
                    Err(error) => eprintln!("skipping {name}: {error}"),
                }
            }
            other => eprintln!("unknown flag {other}"),
        }
    }

    if doors.is_empty() {
        eprintln!("no doors; pass --jev or --door name=url");
        std::process::exit(2);
    }

    let mut rows = Vec::new();
    for (name, client) in &doors {
        rows.push(run(name, client).await);
    }

    println!("# One contract, three implementations\n");
    println!(
        "The same eight items and the same four-option Choice, sent by the same \
         `crates/jev` client to every door. A `*` marks a wrong answer.\n"
    );

    print!("| Item | Truth |");
    for row in &rows {
        print!(" {} |", row.door);
    }
    println!();
    print!("| --- | --- |");
    for _ in &rows {
        print!(" --- |");
    }
    println!();
    for (index, item) in ITEMS.iter().enumerate() {
        print!("| `{}` | `{}` |", item.label, item.truth);
        for row in &rows {
            print!(" `{}` |", row.answers.get(index).map_or("<none>", String::as_str));
        }
        println!();
    }

    println!("\n| Door | Correct | Mean confidence | Confident errors | Mean latency |");
    println!("| --- | --- | --- | --- | --- |");
    for row in &rows {
        println!(
            "| {} | {} of {} | {:.2} | {} | {:.0} ms |",
            row.door, row.correct, row.total, row.mean_confidence, row.confident_errors, row.mean_ms
        );
    }
    println!(
        "\nConfident errors count answers that were wrong at a confidence of 0.9 or above. \
         Read Lev's confidence column knowing what it is: a statistic over seeded samples, \
         not a calibrated probability. See `docs/lev/measurements/`.\n"
    );
}
