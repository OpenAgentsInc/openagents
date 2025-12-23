use issues::{Priority, IssueType, issue::create_issue, db::init_db};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = init_db(&"autopilot.db".into())?;

    // Issue 1: Emit metrics at end of run
    let issue1 = create_issue(
        &conn,
        "Emit metrics at end of each autopilot run",
        Some("Implement automatic metrics emission after each autopilot run completes. This should extract metrics from the trajectory and store them in metrics.db.

Context: Part of Phase 1 of d-004 (Continual Constant Improvement). This enables the feedback loop for autopilot improvement.

Success criteria:
- Metrics are automatically collected when autopilot run ends
- Data is stored in metrics.db
- Includes all key dimensions: duration, token counts, errors, completion status"),
        Priority::Urgent,
        IssueType::Task,
        Some("claude"),
        Some("d-004"),
        None,
    )?;
    println!("Created issue #{}: {}", issue1.number, issue1.title);

    // Issue 2: Regression detection
    let issue2 = create_issue(
        &conn,
        "Implement regression detection in metrics analysis",
        Some("Detect when metrics regress compared to baseline. This is critical for catching performance degradation early.

Context: Part of Phase 3 of d-004. Once we have baseline metrics, we need to detect when new runs perform worse.

Implementation:
- Compare each metric against baseline (mean, stddev)
- Flag metrics that are >2 std dev worse than baseline
- Store regressions in anomalies table
- CLI command shows regressions: cargo autopilot analyze --regressions

Success criteria:
- Regression detection runs automatically on new metrics
- Regressions are flagged with severity
- Can query for all regressions in time period"),
        Priority::High,
        IssueType::Task,
        Some("claude"),
        Some("d-004"),
        None,
    )?;
    println!("Created issue #{}: {}", issue2.number, issue2.title);

    // Issue 3: Weekly trend reports
    let issue3 = create_issue(
        &conn,
        "Generate automated weekly trend reports",
        Some("Automatically generate weekly summary reports showing metric trends and improvements.

Context: Part of Phase 3 of d-004. Human-readable reports make it easy to see progress.

Implementation:
- Scheduled weekly report generation (cron/systemd timer)
- Compare current week to previous week for all metrics
- Highlight improvements and regressions
- Save report to docs/autopilot/reports/YYYY-WW.md
- Optional: Post to Nostr as NIP-90 result

Success criteria:
- Report auto-generates every Monday
- Shows trends for all key metrics
- Includes specific examples (sessions with best/worst metrics)
- Markdown format, human-readable"),
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        Some("d-004"),
        None,
    )?;
    println!("Created issue #{}: {}", issue3.number, issue3.title);

    Ok(())
}
