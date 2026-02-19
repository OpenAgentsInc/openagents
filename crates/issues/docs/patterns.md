# Usage Patterns

Common patterns for using the issues crate with autonomous agents.

## Basic Agent Loop

The simplest pattern: pick up work, do it, mark complete.

```rust
use issues::{init_db, issue};
use std::path::Path;

fn agent_loop(db_path: &Path, run_id: &str) -> anyhow::Result<()> {
    let conn = init_db(db_path)?;

    loop {
        // Get next available issue
        let Some(task) = issue::get_next_ready_issue(&conn)? else {
            println!("No work available");
            break;
        };

        // Claim it
        if !issue::claim_issue(&conn, &task.id, run_id)? {
            // Someone else grabbed it, try again
            continue;
        }

        println!("Working on #{}: {}", task.number, task.title);

        // Do the work
        match do_work(&task) {
            Ok(()) => {
                issue::complete_issue(&conn, &task.id)?;
                println!("Completed #{}", task.number);
            }
            Err(e) => {
                // Release claim so others can try
                issue::unclaim_issue(&conn, &task.id)?;
                println!("Failed #{}: {}", task.number, e);
            }
        }
    }

    Ok(())
}
```

## Parallel Agents

Multiple agents can work concurrently. The claim system prevents conflicts.

```rust
// Agent 1 and Agent 2 both try to get work
let issue = get_next_ready_issue(&conn)?;

// Both see the same issue, but only one claim succeeds
if claim_issue(&conn, &issue.id, "agent-1")? {
    // Agent 1 got it
} else {
    // Agent 2's claim failed - issue was already claimed
    // Try again with next issue
}
```

## Long-Running Tasks with Heartbeat

For tasks that take longer than 15 minutes, refresh the claim periodically.

```rust
use std::time::{Duration, Instant};

fn work_with_heartbeat(conn: &Connection, issue_id: &str) -> anyhow::Result<()> {
    let heartbeat_interval = Duration::from_secs(300); // 5 minutes
    let mut last_heartbeat = Instant::now();

    loop {
        // Check if we should send heartbeat
        if last_heartbeat.elapsed() > heartbeat_interval {
            refresh_claim(conn, issue_id)?;
            last_heartbeat = Instant::now();
        }

        // Do a chunk of work
        if work_is_done() {
            break;
        }
    }

    Ok(())
}

fn refresh_claim(conn: &Connection, issue_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE issues SET claimed_at = datetime('now') WHERE id = ?",
        [issue_id],
    )?;
    Ok(())
}
```

## Creating Sub-Issues

When an agent discovers additional work, create sub-issues.

```rust
fn process_feature(conn: &Connection, parent: &Issue) -> anyhow::Result<()> {
    // Analyze the feature request
    let subtasks = analyze_feature(&parent.description);

    // Create sub-issues for each part
    for (i, subtask) in subtasks.iter().enumerate() {
        let sub = issue::create_issue(
            conn,
            &format!("{} - Part {}", parent.title, i + 1),
            Some(subtask),
            parent.priority, // Inherit priority
            IssueType::Task,
        )?;
        println!("Created sub-issue #{}: {}", sub.number, sub.title);
    }

    // Complete the parent (it's now broken down)
    issue::complete_issue(conn, &parent.id)?;

    Ok(())
}
```

## Handling Blockers

When work is blocked, record why and move on.

```rust
fn try_work(conn: &Connection, issue: &Issue) -> anyhow::Result<()> {
    match check_prerequisites(issue) {
        Ok(()) => {
            // Prerequisites met, do the work
            do_work(issue)?;
            issue::complete_issue(conn, &issue.id)?;
        }
        Err(blocker) => {
            // Can't proceed, block the issue
            issue::block_issue(conn, &issue.id, &blocker.to_string())?;
            println!("Blocked #{}: {}", issue.number, blocker);
        }
    }
    Ok(())
}

// Later, when blockers are resolved:
fn unblock_resolved(conn: &Connection) -> anyhow::Result<()> {
    let issues = issue::list_issues(conn, Some(Status::Open))?;

    for issue in issues.iter().filter(|i| i.is_blocked) {
        if blocker_resolved(&issue.blocked_reason) {
            issue::unblock_issue(conn, &issue.id)?;
            println!("Unblocked #{}", issue.number);
        }
    }
    Ok(())
}
```

## Priority-Based Scheduling

Use priorities to control work order.

```rust
// Create urgent issue that jumps the queue
issue::create_issue(
    &conn,
    "Production down - fix auth",
    Some("500 errors on /api/login"),
    Priority::Urgent,
    IssueType::Bug,
)?;

// Low priority for nice-to-haves
issue::create_issue(
    &conn,
    "Add loading spinners",
    None,
    Priority::Low,
    IssueType::Feature,
)?;

// get_next_ready_issue will return urgent bugs before low-priority features
```

## Dashboard / Reporting

Query the database for status summaries.

```rust
fn print_status(conn: &Connection) -> anyhow::Result<()> {
    let all = issue::list_issues(conn, None)?;

    let open = all.iter().filter(|i| i.status == Status::Open).count();
    let in_progress = all.iter().filter(|i| i.status == Status::InProgress).count();
    let done = all.iter().filter(|i| i.status == Status::Done).count();
    let blocked = all.iter().filter(|i| i.is_blocked).count();

    println!("Issues: {} open, {} in progress, {} done", open, in_progress, done);
    println!("Blocked: {}", blocked);

    // Show what's in progress
    for issue in all.iter().filter(|i| i.status == Status::InProgress) {
        let claimed = issue.claimed_by.as_deref().unwrap_or("unknown");
        println!("  #{} {} (claimed by {})", issue.number, issue.title, claimed);
    }

    Ok(())
}
```

## Integration with Autopilot

The autopilot crate uses issues for task management:

```rust
// In autopilot/src/main.rs

use issues::{init_db, issue, Priority, IssueType};

async fn run_with_issue_tracking(prompt: &str) -> anyhow::Result<()> {
    let conn = init_db(Path::new("autopilot.db"))?;
    let run_id = uuid::Uuid::new_v4().to_string();

    // Check if working on a specific issue
    if let Some(issue_num) = args.issue {
        let issue = issue::get_issue_by_number(&conn, issue_num)?
            .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", issue_num))?;

        issue::claim_issue(&conn, &issue.id, &run_id)?;

        // Run the agent with the issue context
        let result = run_agent(&issue.title, issue.description.as_deref()).await;

        match result {
            Ok(()) => issue::complete_issue(&conn, &issue.id)?,
            Err(e) => issue::block_issue(&conn, &issue.id, &e.to_string())?,
        }
    } else if args.next {
        // Work on next available issue
        if let Some(issue) = issue::get_next_ready_issue(&conn)? {
            issue::claim_issue(&conn, &issue.id, &run_id)?;
            // ... same pattern
        }
    }

    Ok(())
}
```

## Error Recovery

Handle failures gracefully to avoid stuck issues.

```rust
fn safe_work(conn: &Connection, issue: &Issue, run_id: &str) -> anyhow::Result<()> {
    // Claim with automatic unclaim on failure
    if !issue::claim_issue(conn, &issue.id, run_id)? {
        anyhow::bail!("Could not claim issue");
    }

    // Use a guard pattern for cleanup
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        do_risky_work(issue)
    }));

    match result {
        Ok(Ok(())) => {
            issue::complete_issue(conn, &issue.id)?;
        }
        Ok(Err(e)) => {
            // Known error - unclaim so it can be retried
            issue::unclaim_issue(conn, &issue.id)?;
            return Err(e);
        }
        Err(_panic) => {
            // Panic - unclaim and report
            issue::unclaim_issue(conn, &issue.id)?;
            anyhow::bail!("Work panicked");
        }
    }

    Ok(())
}
```
