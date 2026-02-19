use issues::{db, issue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <issue_id> <run_id>", args[0]);
        std::process::exit(1);
    }

    let issue_id = &args[1];
    let run_id = &args[2];

    let conn = db::init_db(std::path::Path::new(".openagents/autopilot.db"))?;
    issue::claim_issue(&conn, issue_id, run_id)?;

    println!("✓ Claimed issue {}", issue_id);
    Ok(())
}
