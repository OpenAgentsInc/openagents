use issues::{db, issue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the autopilot database
    let conn = db::init_db(std::path::Path::new(".openagents/autopilot.db"))?;

    // Get next ready issue for codex
    match issue::get_next_ready_issue(&conn, Some("codex"))? {
        Some(iss) => {
            println!("{}", serde_json::to_string_pretty(&iss)?);
        }
        None => {
            eprintln!("No ready issues available");
            std::process::exit(1);
        }
    }

    Ok(())
}
