//! Class-only remeasurement; no engine starts and no task workspace changes.
//! Run with an NDJSON row set path. Results go to stdout as NDJSON.
use std::io::{BufRead, Write};

use coder_delegate::component::jev::{Ask, JevMode, ask};
use coder_delegate::{credentials, recipe, record::Recorder, usage};
use serde_json::{Value, json};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected an NDJSON row set path")?;
    let home = std::env::var("HOME")?;
    let resolved = credentials::jev(
        |name| std::env::var(name).ok(),
        &std::path::Path::new(&home).join(".openagents"),
    )?;
    let mode = JevMode::Live(resolved.client);
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    for line in file.lines() {
        let row: Value = serde_json::from_str(&line?)?;
        let id = row["id"].as_str().ok_or("row has no id")?;
        let state = recipe::class_state(
            row["state"]["request"]
                .as_str()
                .ok_or("row has no request")?,
            row["state"]["earlier"].as_str().unwrap_or(""),
        );
        let recorder = Recorder::default();
        let started = std::time::Instant::now();
        let answer = ask(
            &mode,
            &recorder,
            Ask {
                component: "recipe.class",
                name: "jev_recipe_class",
                id: id.to_owned(),
                state: state.clone(),
                questions: recipe::class_questions(),
                parent: None,
                deadline: None,
            },
        )
        .await;
        let (asks_only, hard) = (answer.noul("asks_only"), answer.noul("hard"));
        let class = recipe::class_of(asks_only, hard);
        let steps = recorder.steps();
        println!(
            "{}",
            json!({
                "id": id, "set": recipe::CLASS_SET, "hard_at": recipe::HARD_AT,
                "state": state, "asks_only": asks_only, "hard": hard,
                "class": class.map(recipe::TaskClass::word), "expected": row["expected"],
                "error": answer.error, "seconds": started.elapsed().as_secs_f64(),
                "usage": usage::usage(&steps, false), "steps": steps,
            })
        );
        std::io::stdout().flush()?;
        if class.is_none() {
            return Err("class call failed; partial results retained".into());
        }
    }
    Ok(())
}
