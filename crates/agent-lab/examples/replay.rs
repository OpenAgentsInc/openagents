//! Print a 2026-10-17 replay task, or score an agent's reply to one (#12034).
//!
//! cargo run -p agent-lab --example replay -- list
//! cargo run -p agent-lab --example replay -- task stall-ladder
//! cargo run -p agent-lab --example replay -- score stall-ladder reply.json [AGENT_ID]
//!
//! Scoring reads only the reply file; nothing else is touched.
use agent_lab::replay;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let day = replay::day();
    let scenario = |id: &str| {
        day.scenario(id)
            .ok_or(format!("No scenario {id}. Try list."))
    };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["list"] => {
            for s in &day.scenarios {
                println!("{}\t{}", s.id, s.title);
            }
        }
        ["task", id] => println!("{}", day.task(scenario(id)?)),
        ["score", id, file, rest @ ..] => {
            scenario(id)?;
            let agent = rest.first().copied().unwrap_or(&day.subject);
            let score = day.score(id, agent, &std::fs::read_to_string(file)?);
            println!("{}", serde_json::to_string_pretty(&score)?);
            if !score.passed {
                std::process::exit(1);
            }
        }
        _ => return Err("Use: list | task SCENARIO | score SCENARIO REPLY_FILE [AGENT_ID]".into()),
    }
    Ok(())
}
