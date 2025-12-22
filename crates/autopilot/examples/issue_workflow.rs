//! Issue workflow example
//!
//! This example demonstrates programmatic issue management:
//! creating issues, claiming them, processing tasks, and marking
//! them complete. This is the core workflow used by autopilot's
//! full-auto mode.
//!
//! Run with:
//! ```bash
//! cargo run --example issue_workflow
//! ```

use issues::{db, issue, Priority, IssueType, Status};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Autopilot Issue Workflow Example");
    println!("====================================\n");

    // Initialize database (in-memory for this example)
    let conn = db::init_memory_db()?;
    println!("✓ Initialized in-memory database\n");

    // Create multiple issues with different priorities
    println!("Creating issues...\n");

    let issue1 = issue::create_issue(
        &conn,
        "Fix authentication bug",
        Some("Users report being logged out unexpectedly"),
        Priority::Urgent,
        IssueType::Bug,
        Some("claude"),
        None,
    )?;
    println!("  #{}: {} [{:?}]", issue1.number, issue1.title, issue1.priority);

    let issue2 = issue::create_issue(
        &conn,
        "Add unit tests for parser module",
        Some("Parser needs comprehensive test coverage"),
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )?;
    println!("  #{}: {} [{:?}]", issue2.number, issue2.title, issue2.priority);

    let issue3 = issue::create_issue(
        &conn,
        "Update documentation",
        Some("README needs examples section"),
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
    )?;
    println!("  #{}: {} [{:?}]", issue3.number, issue3.title, issue3.priority);

    let issue4 = issue::create_issue(
        &conn,
        "Refactor error handling",
        Some("Consolidate error types across crates"),
        Priority::Low,
        IssueType::Feature,
        Some("claude"),
        None,
    )?;
    println!("  #{}: {} [{:?}]", issue4.number, issue4.title, issue4.priority);

    println!("\n✓ Created {} issues\n", 4);

    // Simulate autopilot processing loop
    println!("Processing issues by priority...\n");

    let run_id = "example_run_001";
    let mut processed_count = 0;

    loop {
        // Get next highest priority issue for this agent
        let next = issue::get_next_ready_issue(&conn, Some("claude"))?;

        match next {
            Some(iss) => {
                println!("→ Processing issue #{}: {}", iss.number, iss.title);
                println!("  Priority: {:?}, Type: {:?}", iss.priority, iss.issue_type);

                // Claim the issue
                issue::claim_issue(&conn, &iss.id, run_id)?;
                println!("  ✓ Claimed");

                // Simulate work being done
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Complete the issue
                issue::complete_issue(&conn, &iss.id)?;
                println!("  ✓ Completed\n");

                processed_count += 1;
            }
            None => {
                println!("✓ No more issues to process");
                break;
            }
        }
    }

    println!("\nProcessed {} issues total", processed_count);

    // Show issue statistics
    println!("\nFinal issue status:");

    let all_issues = issue::list_issues(&conn, Some(Status::Done))?;
    println!("  Done: {}", all_issues.len());

    let pending = issue::list_issues(&conn, Some(Status::Open))?;
    println!("  Pending: {}", pending.len());

    // Demonstrate blocking an issue
    println!("\n--- Blocking Workflow ---\n");

    let blocked = issue::create_issue(
        &conn,
        "Implement payment integration",
        Some("Need API credentials from customer"),
        Priority::High,
        IssueType::Feature,
        Some("claude"),
        None,
    )?;
    println!("Created issue #{}: {}", blocked.number, blocked.title);

    // Block the issue with a reason
    issue::block_issue(
        &conn,
        &blocked.id,
        "Waiting for API credentials from customer",
    )?;
    println!("✓ Blocked issue #{}", blocked.number);

    // Verify it won't appear in ready queue
    let next = issue::get_next_ready_issue(&conn, Some("claude"))?;
    assert!(next.is_none(), "Blocked issue should not appear in queue");
    println!("✓ Verified blocked issue excluded from queue");

    println!("\n✓ Example complete!");

    Ok(())
}
