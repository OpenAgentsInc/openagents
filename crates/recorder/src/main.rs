//! Recorder CLI - Validate and analyze .rlog files

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};

use recorder::{
    LineType, Severity, ValidationResult,
    convert::{ConvertOptions, convert_file},
    parse_content, parse_file, validate,
};

#[derive(Parser)]
#[command(name = "recorder")]
#[command(about = "Recorder format validator and analyzer")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a .rlog file
    Validate {
        /// Path to the .rlog file
        file: PathBuf,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,

        /// Show only errors (no warnings/info)
        #[arg(short, long)]
        errors_only: bool,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show statistics about a .rlog file
    Stats {
        /// Path to the .rlog file
        file: PathBuf,
    },

    /// Parse and dump structure of a .rlog file
    Parse {
        /// Path to the .rlog file
        file: PathBuf,

        /// Show line-by-line breakdown
        #[arg(short, long)]
        lines: bool,

        /// Maximum lines to show
        #[arg(short = 'n', long, default_value = "50")]
        max_lines: usize,
    },

    /// Fix issues in a .rlog file
    Fix {
        /// Path to the .rlog file
        file: PathBuf,

        /// Renumber steps sequentially (fixes out-of-order steps)
        #[arg(long)]
        renumber_steps: bool,

        /// Write changes to file (default: dry-run)
        #[arg(short, long)]
        write: bool,

        /// Output file (default: overwrite input)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Export session(s) from database to .rlog files (requires --features export)
    #[cfg(feature = "export")]
    Export {
        /// Session ID to export (sess_YYYYMMDD_NNN format) or UUID
        #[arg(short, long)]
        session: Option<String>,

        /// Export all sessions for a user
        #[arg(short, long)]
        user_id: Option<String>,

        /// Output file path (for single session) or directory (for multiple)
        #[arg(short, long, default_value = ".")]
        output: PathBuf,

        /// Database URL (defaults to DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// List sessions instead of exporting
        #[arg(short, long)]
        list: bool,

        /// Maximum sessions to list/export
        #[arg(long, default_value = "50")]
        limit: i64,
    },

    /// Convert Claude Code JSONL session to .rlog format
    Convert {
        /// Path to the Claude Code .jsonl file
        file: PathBuf,

        /// Git repository SHA (auto-detect from cwd if not specified)
        #[arg(long)]
        repo_sha: Option<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include thinking blocks
        #[arg(long, default_value = "true")]
        include_thinking: bool,

        /// Include signature on thinking blocks
        #[arg(long, default_value = "true")]
        include_signature: bool,

        /// Include file-history-snapshot events as comments
        #[arg(long, default_value = "true")]
        include_snapshots: bool,

        /// Include queue-operation events as comments
        #[arg(long, default_value = "true")]
        include_queue_ops: bool,

        /// Include raw Claude Code JSONL events as comments (may contain sensitive data)
        #[arg(long, default_value = "false")]
        include_raw_events: bool,

        /// Validate the converted output
        #[arg(long)]
        validate: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate {
            file,
            verbose,
            errors_only,
            format,
        } => cmd_validate(&file, verbose, errors_only, &format),

        Commands::Stats { file } => cmd_stats(&file),

        Commands::Parse {
            file,
            lines,
            max_lines,
        } => cmd_parse(&file, lines, max_lines),

        Commands::Fix {
            file,
            renumber_steps,
            write,
            output,
        } => cmd_fix(&file, renumber_steps, write, output.as_ref()),

        #[cfg(feature = "export")]
        Commands::Export {
            session,
            user_id,
            output,
            database_url,
            list,
            limit,
        } => {
            // Run async export command
            tokio::runtime::Runtime::new()?.block_on(cmd_export(
                session,
                user_id,
                output,
                database_url,
                list,
                limit,
            ))
        }

        Commands::Convert {
            file,
            repo_sha,
            output,
            include_thinking,
            include_signature,
            include_snapshots,
            include_queue_ops,
            include_raw_events,
            validate: do_validate,
        } => cmd_convert(
            &file,
            repo_sha.as_deref(),
            output.as_ref(),
            include_thinking,
            include_signature,
            include_snapshots,
            include_queue_ops,
            include_raw_events,
            do_validate,
        ),
    }
}

fn cmd_validate(path: &Path, verbose: bool, errors_only: bool, format: &str) -> Result<()> {
    let session = parse_file(path).context("Failed to parse file")?;
    let result = validate(&session);

    if format == "json" {
        print_validation_json(&result);
    } else {
        print_validation_text(path, &result, verbose, errors_only);
    }

    // Exit with error code if validation failed
    if result.is_valid() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn print_validation_text(path: &Path, result: &ValidationResult, verbose: bool, errors_only: bool) {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    // Header
    println!();
    if result.is_valid() {
        println!("{} {}", "✓".green().bold(), filename.green().bold());
    } else {
        println!("{} {}", "✗".red().bold(), filename.red().bold());
    }
    println!();

    // Issues
    let mut shown_issues = 0;
    for issue in &result.issues {
        let skip = errors_only && issue.severity != Severity::Error;
        if skip {
            continue;
        }

        let (prefix, color) = match issue.severity {
            Severity::Error => ("ERROR", "red"),
            Severity::Warning => ("WARN ", "yellow"),
            Severity::Info => ("INFO ", "blue"),
        };

        let line_info = issue
            .line
            .map(|l| format!("line {}:", l))
            .unwrap_or_default();

        let msg = match color {
            "red" => format!(
                "{} [{}] {} {}",
                prefix.red().bold(),
                issue.code,
                line_info,
                issue.message
            ),
            "yellow" => format!(
                "{} [{}] {} {}",
                prefix.yellow().bold(),
                issue.code,
                line_info,
                issue.message
            ),
            _ => format!(
                "{} [{}] {} {}",
                prefix.blue().bold(),
                issue.code,
                line_info,
                issue.message
            ),
        };

        println!("  {}", msg);
        shown_issues += 1;
    }

    if shown_issues > 0 {
        println!();
    }

    // Summary
    let error_count = result.error_count();
    let warning_count = result.warning_count();
    let info_count = result
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();

    if verbose || error_count > 0 || warning_count > 0 {
        print!("  ");
        if error_count > 0 {
            print!("{} errors  ", format!("{}", error_count).red().bold());
        }
        if warning_count > 0 {
            print!(
                "{} warnings  ",
                format!("{}", warning_count).yellow().bold()
            );
        }
        if info_count > 0 && !errors_only {
            print!("{} info", format!("{}", info_count).blue());
        }
        println!();
    }

    // Stats summary
    if verbose {
        println!();
        println!("  {}", "Statistics:".dimmed());
        println!("    Lines:        {}", result.stats.total_lines);
        println!("    User msgs:    {}", result.stats.user_messages);
        println!("    Agent msgs:   {}", result.stats.agent_messages);
        println!("    Tool calls:   {}", result.stats.tool_calls);
        println!("    Observations: {}", result.stats.observations);
        println!("    Subagents:    {}", result.stats.subagents);
        println!("    MCP calls:    {}", result.stats.mcp_calls);
        println!("    Questions:    {}", result.stats.questions);
        println!("    Phases:       {}", result.stats.phases);
        println!("    Call IDs:     {}", result.stats.unique_call_ids);
        if let Some(max_step) = result.stats.max_step {
            println!("    Max step:     {}", max_step);
        }
        println!(
            "    Timestamps:   {}",
            if result.stats.has_timestamps {
                "yes"
            } else {
                "no"
            }
        );
        println!("    Blobs:        {}", result.stats.blob_references);
        println!("    Redacted:     {}", result.stats.redacted_values);
    }

    println!();

    // Final verdict
    if result.is_valid() {
        println!("  {} Valid Recorder file", "✓".green().bold());
    } else {
        println!("  {} Invalid Recorder file", "✗".red().bold());
    }
    println!();
}

fn print_validation_json(result: &ValidationResult) {
    let issues: Vec<_> = result
        .issues
        .iter()
        .map(|i| {
            serde_json::json!({
                "line": i.line,
                "severity": format!("{:?}", i.severity).to_lowercase(),
                "code": i.code,
                "message": i.message,
            })
        })
        .collect();

    let output = serde_json::json!({
        "valid": result.is_valid(),
        "errors": result.error_count(),
        "warnings": result.warning_count(),
        "issues": issues,
        "stats": {
            "total_lines": result.stats.total_lines,
            "user_messages": result.stats.user_messages,
            "agent_messages": result.stats.agent_messages,
            "tool_calls": result.stats.tool_calls,
            "observations": result.stats.observations,
            "subagents": result.stats.subagents,
            "mcp_calls": result.stats.mcp_calls,
            "questions": result.stats.questions,
            "phases": result.stats.phases,
            "unique_call_ids": result.stats.unique_call_ids,
            "max_step": result.stats.max_step,
            "has_timestamps": result.stats.has_timestamps,
            "blob_references": result.stats.blob_references,
            "redacted_values": result.stats.redacted_values,
        }
    });

    let json = serde_json::to_string_pretty(&output)
        .expect("Failed to serialize validation output to JSON");
    println!("{}", json);
}

fn cmd_stats(path: &Path) -> Result<()> {
    let session = parse_file(path).context("Failed to parse file")?;
    let result = validate(&session);

    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    println!();
    println!("{}", filename.bold());
    println!("{}", "=".repeat(filename.len()));
    println!();

    // Header info
    println!("{}", "Header".cyan().bold());
    println!("  Format:       {}", session.header.format);
    println!("  Session ID:   {}", session.header.id);
    if let Some(ref mode) = session.header.mode {
        println!("  Mode:         {}", mode);
    }
    if let Some(ref model) = session.header.model {
        println!("  Model:        {}", model);
    }
    if let Some(ref repo) = session.header.repo {
        println!("  Repo:         {}", repo);
    }
    println!("  Repo SHA:     {}", session.header.repo_sha);
    if let Some(ref branch) = session.header.branch {
        println!("  Branch:       {}", branch);
    }
    if let Some(ref runner) = session.header.runner {
        println!("  Runner:       {}", runner);
    }
    if let Some(ref sandbox) = session.header.sandbox_id {
        println!("  Sandbox:      {}", sandbox);
    }
    if let Some(ref budget) = session.header.budget {
        println!("  Budget:       {}", budget);
    }
    if let Some(ref duration) = session.header.duration {
        println!("  Duration:     {}", duration);
    }
    println!();

    // Content stats
    println!("{}", "Content".cyan().bold());
    println!("  Total lines:       {}", result.stats.total_lines);
    println!("  User messages:     {}", result.stats.user_messages);
    println!("  Agent messages:    {}", result.stats.agent_messages);
    println!("  Tool calls:        {}", result.stats.tool_calls);
    println!("  Observations:      {}", result.stats.observations);
    println!("  Subagent spawns:   {}", result.stats.subagents);
    println!("  MCP calls:         {}", result.stats.mcp_calls);
    println!("  Questions:         {}", result.stats.questions);
    println!("  Phases:            {}", result.stats.phases);
    println!("  Lifecycle events:  {}", result.stats.lifecycle_events);
    println!("  Comments:          {}", result.stats.comments);
    println!();

    // Correlation stats
    println!("{}", "Correlation".cyan().bold());
    println!("  Unique call IDs:   {}", result.stats.unique_call_ids);
    if let Some(max_step) = result.stats.max_step {
        println!("  Max step ID:       {}", max_step);
    }
    println!(
        "  Has timestamps:    {}",
        if result.stats.has_timestamps {
            "yes"
        } else {
            "no"
        }
    );
    println!();

    // Data stats
    println!("{}", "Data".cyan().bold());
    println!("  Blob references:   {}", result.stats.blob_references);
    println!("  Redacted values:   {}", result.stats.redacted_values);
    println!();

    // Validation
    println!("{}", "Validation".cyan().bold());
    println!("  Errors:            {}", result.error_count());
    println!("  Warnings:          {}", result.warning_count());
    println!(
        "  Status:            {}",
        if result.is_valid() {
            "VALID".green()
        } else {
            "INVALID".red()
        }
    );
    println!();

    Ok(())
}

fn cmd_parse(path: &Path, show_lines: bool, max_lines: usize) -> Result<()> {
    let session = parse_file(path).context("Failed to parse file")?;

    println!();
    println!("{}", "Header".cyan().bold());
    println!("  format: {}", session.header.format);
    println!("  id: {}", session.header.id);
    println!("  repo_sha: {}", session.header.repo_sha);
    println!();

    if show_lines {
        println!("{}", "Lines".cyan().bold());

        let lines_to_show = session.lines.iter().take(max_lines);
        for line in lines_to_show {
            let type_str = match line.line_type {
                LineType::User => "USER".magenta(),
                LineType::Agent => "AGNT".green(),
                LineType::Tool => "TOOL".yellow(),
                LineType::ToolStart => "T_ST".yellow(),
                LineType::ToolProgress => "T_PG".yellow().dimmed(),
                LineType::Observation => "OBSV".cyan(),
                LineType::Skill => "SKIL".blue(),
                LineType::Plan => "PLAN".blue(),
                LineType::Mode => "MODE".white(),
                LineType::Recall => "RECL".magenta().dimmed(),
                LineType::Subagent => "SUBX".red(),
                LineType::Mcp => "MCP ".cyan(),
                LineType::Question => "QSTN".magenta().bold(),
                LineType::Comment => "  # ".dimmed(),
                LineType::Lifecycle => "@ LC".white().bold(),
                LineType::Phase => "@PHS".blue().bold(),
                LineType::Thinking => "THNK".cyan().dimmed(),
                LineType::Todos => "TODO".blue(),
                LineType::Empty => "    ".normal(),
                LineType::Continuation => " ...".dimmed(),
                LineType::Unknown => "????".red(),
            };

            let mut meta = Vec::new();
            if let Some(ref id) = line.call_id {
                meta.push(format!("id={}", id));
            }
            if let Some(step) = line.step {
                meta.push(format!("step={}", step));
            }
            if let Some(ref ts) = line.timestamp {
                meta.push(format!("ts={}", &ts[11..19])); // Just time part
            }

            let meta_str = if meta.is_empty() {
                String::new()
            } else {
                format!(" [{}]", meta.join(" ").dimmed())
            };

            let content = truncate_str(&line.content, 60);

            println!(
                "  {:>4} {} {}{}",
                line.line_number.to_string().dimmed(),
                type_str,
                content,
                meta_str
            );
        }

        if session.lines.len() > max_lines {
            println!();
            println!(
                "  {} (showing {}/{})",
                "...".dimmed(),
                max_lines,
                session.lines.len()
            );
        }
    } else {
        // Just show line type distribution
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

        for line in &session.lines {
            let key = match line.line_type {
                LineType::User => "user",
                LineType::Agent => "agent",
                LineType::Tool | LineType::ToolStart | LineType::ToolProgress => "tool",
                LineType::Observation => "observation",
                LineType::Skill => "skill",
                LineType::Plan => "plan",
                LineType::Mode => "mode",
                LineType::Recall => "recall",
                LineType::Subagent => "subagent",
                LineType::Mcp => "mcp",
                LineType::Question => "question",
                LineType::Comment => "comment",
                LineType::Lifecycle => "lifecycle",
                LineType::Phase => "phase",
                LineType::Thinking => "thinking",
                LineType::Todos => "todos",
                LineType::Empty => "empty",
                LineType::Continuation => "continuation",
                LineType::Unknown => "unknown",
            };
            *counts.entry(key).or_insert(0) += 1;
        }

        println!("{}", "Line distribution".cyan().bold());
        let mut sorted: Vec<_> = counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (key, count) in sorted {
            if *count > 0 {
                println!("  {:12} {}", key, count);
            }
        }
    }

    println!();
    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

fn cmd_fix(
    path: &PathBuf,
    renumber_steps: bool,
    write: bool,
    output: Option<&PathBuf>,
) -> Result<()> {
    let content = std::fs::read_to_string(path).context("Failed to read file")?;
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    println!();
    println!("{} {}", "Fixing".cyan().bold(), filename);
    println!();

    let mut modified_content = content.clone();
    let mut changes = 0;

    if renumber_steps {
        let (new_content, step_changes) = renumber_steps_sequential(&content)?;
        if step_changes > 0 {
            modified_content = new_content;
            changes += step_changes;
            println!(
                "  {} Renumbered {} step references",
                "✓".green(),
                step_changes
            );
        } else {
            println!("  {} Steps already in order", "·".dimmed());
        }
    }

    if changes == 0 {
        println!();
        println!("  {} No changes needed", "✓".green());
        println!();
        return Ok(());
    }

    println!();
    println!("  Total changes: {}", changes);

    if write {
        let output_path = output.unwrap_or(path);
        std::fs::write(output_path, &modified_content).context("Failed to write file")?;
        println!("  {} Written to {}", "✓".green(), output_path.display());
    } else {
        println!();
        println!(
            "  {} Dry run - use {} to apply changes",
            "!".yellow(),
            "--write".bold()
        );

        // Show a preview of changes
        let original_lines: Vec<&str> = content.lines().collect();
        let modified_lines: Vec<&str> = modified_content.lines().collect();

        let mut diff_count = 0;
        let max_diffs = 10;

        for (i, (orig, modified)) in original_lines.iter().zip(modified_lines.iter()).enumerate() {
            if orig != modified && diff_count < max_diffs {
                println!();
                println!("  Line {}:", i + 1);
                println!("    {} {}", "-".red(), truncate_str(orig, 70).red());
                println!("    {} {}", "+".green(), truncate_str(modified, 70).green());
                diff_count += 1;
            }
        }

        if diff_count >= max_diffs {
            println!();
            println!("  ... and more changes");
        }
    }

    println!();
    Ok(())
}

/// Renumber all step= references sequentially starting from 1
fn renumber_steps_sequential(content: &str) -> Result<(String, usize)> {
    use regex::Regex;

    let step_re = Regex::new(r"step=(\d+)").expect("Invalid regex pattern for step renumbering");
    let lines: Vec<&str> = content.lines().collect();

    // First pass: find all lines with step= and their current numbers
    let mut step_lines: Vec<(usize, u32)> = Vec::new(); // (line_index, current_step)

    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = step_re.captures(line)
            && let Ok(step_num) = caps[1].parse::<u32>()
        {
            step_lines.push((i, step_num));
        }
    }

    if step_lines.is_empty() {
        return Ok((content.to_string(), 0));
    }

    // Check if already sequential
    let mut is_sequential = true;
    for (expected, (_, actual)) in step_lines.iter().enumerate() {
        if *actual != (expected + 1) as u32 {
            is_sequential = false;
            break;
        }
    }

    if is_sequential {
        return Ok((content.to_string(), 0));
    }

    // Second pass: create mapping from old step numbers to new sequential numbers
    // We process in order of appearance (line number), assigning sequential steps
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut changes = 0;

    for (new_step, (line_idx, _old_step)) in step_lines.iter().enumerate() {
        let new_step_num = (new_step + 1) as u32;
        let line = &lines[*line_idx];

        // Replace all step=N occurrences in this line with the new step number
        let new_line = step_re
            .replace_all(line, |_caps: &regex::Captures| {
                format!("step={}", new_step_num)
            })
            .to_string();

        if new_line != *line {
            new_lines[*line_idx] = new_line;
            changes += 1;
        }
    }

    Ok((new_lines.join("\n"), changes))
}

// ============================================================================
// Export command (requires --features export)
// ============================================================================

#[cfg(feature = "export")]
async fn cmd_export(
    session: Option<String>,
    user_id: Option<String>,
    output: PathBuf,
    database_url: String,
    list: bool,
    limit: i64,
) -> Result<()> {
    use recorder::export;
    use sqlx::postgres::PgPoolOptions;

    println!();
    println!("{}", "Recorder Export".cyan().bold());
    println!();

    // Connect to database
    println!("  {} Connecting to database...", "→".dimmed());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;
    println!("  {} Connected", "✓".green());

    // List mode
    if list {
        let user_uuid = user_id
            .as_ref()
            .map(|id| uuid::Uuid::parse_str(id))
            .transpose()
            .context("Invalid user UUID")?;

        let sessions = export::list_sessions(&pool, user_uuid, limit).await?;

        println!();
        println!("  {}", "Sessions:".bold());
        println!("  {}", "-".repeat(80));

        for s in &sessions {
            let _status_color = match s.status.as_str() {
                "completed" => "green",
                "running" => "yellow",
                "failed" => "red",
                _ => "white",
            };

            let duration = s
                .completed_at
                .map(|end| format!("{}s", (end - s.started_at).num_seconds()))
                .unwrap_or_else(|| "running".to_string());

            let status = match s.status.as_str() {
                "completed" => s.status.green(),
                "running" => s.status.yellow(),
                "failed" => s.status.red(),
                _ => s.status.normal(),
            };

            println!(
                "  {} {} {:>8} {:>6} tools {:>8} tokens  {}",
                s.session_id.bold(),
                status,
                duration,
                s.total_tool_calls,
                s.total_input_tokens + s.total_output_tokens,
                s.started_at.format("%Y-%m-%d %H:%M")
            );
        }

        println!();
        println!("  Total: {} sessions", sessions.len());
        println!();
        return Ok(());
    }

    // Export mode
    if let Some(session_id) = session {
        // Single session export
        println!("  {} Exporting session: {}", "→".dimmed(), session_id);

        // Try parsing as UUID first, then as session_id
        let session = if let Ok(uuid) = uuid::Uuid::parse_str(&session_id) {
            export::load_session(&pool, uuid).await?
        } else {
            export::load_session_by_id(&pool, &session_id).await?
        };

        let output_file = if output.is_dir() {
            output.join(format!("{}.rlog", session.session_id))
        } else {
            output
        };

        export::export_session(&pool, session.id, &output_file).await?;
        println!("  {} Exported to: {}", "✓".green(), output_file.display());
    } else if let Some(uid) = user_id {
        // All sessions for user
        let user_uuid = uuid::Uuid::parse_str(&uid).context("Invalid user UUID")?;

        println!(
            "  {} Exporting all sessions for user: {}",
            "→".dimmed(),
            uid
        );

        let output_dir = if output.is_file() {
            output.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            output
        };

        let exported =
            export::export_sessions_for_user(&pool, user_uuid, None, None, &output_dir).await?;

        println!(
            "  {} Exported {} sessions to: {}",
            "✓".green(),
            exported.len(),
            output_dir.display()
        );
    } else {
        println!("  {} No session or user specified", "!".yellow());
        println!();
        println!("  Usage:");
        println!("    recorder export --session sess_20251219_001 --output ./sessions/");
        println!("    recorder export --user-id <uuid> --output ./sessions/");
        println!("    recorder export --list");
    }

    println!();
    Ok(())
}

// ============================================================================
// Convert command
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn cmd_convert(
    path: &Path,
    repo_sha: Option<&str>,
    output: Option<&PathBuf>,
    include_thinking: bool,
    include_signature: bool,
    include_snapshots: bool,
    include_queue_ops: bool,
    include_raw_events: bool,
    do_validate: bool,
) -> Result<()> {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    println!();
    println!("{} {}", "Converting".cyan().bold(), filename.bold());
    println!();

    // Get repo SHA - either from arg or auto-detect
    let sha = if let Some(sha) = repo_sha {
        sha.to_string()
    } else {
        // Try to auto-detect from git
        match std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
        {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => {
                println!(
                    "  {} Could not auto-detect git SHA, using placeholder",
                    "!".yellow()
                );
                "unknown".to_string()
            }
        }
    };

    println!("  {} repo_sha: {}", "·".dimmed(), sha);

    let options = ConvertOptions {
        include_thinking,
        include_signature,
        include_snapshots,
        include_queue_ops,
        include_raw_events,
    };

    // Convert the file
    let bbox_content = convert_file(path, &sha, &options).context("Failed to convert file")?;

    // Validate if requested
    if do_validate {
        println!("  {} Validating output...", "→".dimmed());
        let session = parse_content(&bbox_content).context("Failed to parse converted output")?;
        let result = validate(&session);

        if result.is_valid() {
            println!("  {} Validation passed", "✓".green());
        } else {
            println!("  {} Validation issues:", "!".yellow());
            for issue in &result.issues {
                let prefix = match issue.severity {
                    Severity::Error => "ERROR".red(),
                    Severity::Warning => "WARN".yellow(),
                    Severity::Info => "INFO".blue(),
                };
                println!("    {} {}", prefix, issue.message);
            }
        }

        // Print stats
        println!();
        println!("  {}", "Stats:".bold());
        println!("    User messages:    {}", result.stats.user_messages);
        println!("    Agent messages:   {}", result.stats.agent_messages);
        println!("    Tool calls:       {}", result.stats.tool_calls);
        println!("    Thinking blocks:  {}", result.stats.thinking_blocks);
        println!("    Todos updates:    {}", result.stats.todos_updates);
        println!("    Total tokens in:  {}", result.stats.total_tokens_in);
        println!("    Total tokens out: {}", result.stats.total_tokens_out);
    }

    // Output
    if let Some(output_path) = output {
        std::fs::write(output_path, &bbox_content).context("Failed to write output file")?;
        println!();
        println!("  {} Written to: {}", "✓".green(), output_path.display());
    } else {
        println!();
        println!("{}", "--- Output ---".dimmed());
        println!("{}", bbox_content);
    }

    println!();
    Ok(())
}
