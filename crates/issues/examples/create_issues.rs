use issues::{
    db::init_db,
    issue::{claim_issue, get_next_ready_issue},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = init_db(Path::new(".openagents/autopilot.db"))?;

    // Get next ready issue for codex agent
    if let Some(issue) = get_next_ready_issue(&conn, Some("codex"))? {
        // Claim the issue
        let run_id = format!("autopilot-{}", chrono::Utc::now().timestamp());
        if claim_issue(&conn, &issue.id, &run_id)? {
            println!("Claimed issue #{}: {}", issue.number, issue.title);
            println!("ID: {}", issue.id);
            println!("Priority: {:?}", issue.priority);
            if let Some(directive_id) = &issue.directive_id {
                println!("Directive: {}", directive_id);
            }
            if let Some(description) = &issue.description {
                println!("\nDescription:\n{}", description);
            }
        } else {
            println!("Failed to claim issue - it may have been claimed by another process");
        }
    } else {
        println!("No ready issues available for codex agent");
    }

    Ok(())
}
