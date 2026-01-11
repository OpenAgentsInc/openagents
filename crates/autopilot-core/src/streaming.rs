use std::path::Path;

pub fn query_issue_summary(cwd: &Path) -> Option<String> {
    let db_path = cwd.join(".openagents/autopilot.db");
    if !db_path.exists() {
        return None;
    }

    let output = std::process::Command::new("sqlite3")
        .arg(&db_path)
        .arg("SELECT status, COUNT(*) FROM issues GROUP BY status; SELECT '---'; SELECT number, substr(title,1,50), status, priority FROM issues WHERE status != 'done' ORDER BY CASE priority WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 ELSE 4 END LIMIT 10;")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout);

    let mut done = 0;
    let mut in_progress = 0;
    let mut open = 0;
    let mut active_issues = Vec::new();
    let mut in_active = false;

    for line in raw.lines() {
        if line == "---" {
            in_active = true;
            continue;
        }
        if !in_active {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let count: i32 = parts[1].parse().unwrap_or(0);
                match parts[0] {
                    "done" => done = count,
                    "in_progress" => in_progress = count,
                    "open" => open = count,
                    _ => {}
                }
            }
        } else {
            active_issues.push(line.to_string());
        }
    }

    let total = done + in_progress + open;
    if total == 0 {
        return None;
    }

    let mut summary = format!(
        "Issue Status: {} total, {} done ({}%), {} in-progress, {} open\n\nActive issues:\n",
        total,
        done,
        (done * 100) / total,
        in_progress,
        open
    );

    for issue in active_issues {
        summary.push_str(&format!("- {}\n", issue));
    }

    summary.push_str("\nProvide brief analysis: health, top priority, risks.");
    Some(summary)
}
