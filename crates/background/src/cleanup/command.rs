//! The standalone cleanup command used by the plugin and shell entry point.

use std::path::{Path, PathBuf};

use super::{Options, Plan, apply_selected_recorded, create_record, save, scan};
use crate::Layout;

pub const HELP: &str = "usage: openagents-disk-cleanup scan [--age-hours 24] [--builds-only | --worktrees-only] [--plan FILE] [--json] [--home DIR]
       openagents-disk-cleanup apply --plan FILE --receipt FILE [--only 1,3] [--home DIR]
       openagents-disk-cleanup migrate-plugin [--home DIR]

Scan old Cargo outputs and clean worktrees merged into origin/main.
Scan deletes nothing. Apply rechecks the saved plan before each removal.
The minimum age is 24 hours, regardless of free disk space. Source, models,
results, package caches, and unknown folders stay. Shared target folders,
Cargo locks, and compiler state markers stay. Receipts include worktree undo.
";

pub fn run(words: &[String]) -> Result<(), String> {
    if words.is_empty()
        || words
            .iter()
            .any(|word| matches!(word.as_str(), "--help" | "-h"))
    {
        print!("{HELP}");
        return Ok(());
    }
    let mut home = user_home::home_os()
        .map(PathBuf::from)
        .ok_or("Cannot find your home folder.")?;
    let mut options = Options::default();
    let mut plan_file = None;
    let mut receipt_file = None;
    let mut json = false;
    let mut selection = Vec::new();
    let mut scan_options = false;
    let mut args = words[1..].iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--home" => home = PathBuf::from(args.next().ok_or("--home needs a folder.")?),
            "--plan" => plan_file = Some(PathBuf::from(args.next().ok_or("--plan needs a file.")?)),
            "--receipt" => {
                receipt_file = Some(PathBuf::from(args.next().ok_or("--receipt needs a file.")?))
            }
            "--age-hours" => {
                scan_options = true;
                options.minimum_age_secs = args
                    .next()
                    .ok_or("--age-hours needs a number.")?
                    .parse::<u64>()
                    .map_err(|_| "Invalid age.")?
                    .checked_mul(3600)
                    .ok_or("Invalid age.")?;
            }
            "--builds-only" if options.builds => {
                scan_options = true;
                options.worktrees = false;
            }
            "--worktrees-only" if options.worktrees => {
                scan_options = true;
                options.builds = false;
            }
            "--json" => json = true,
            "--only" => {
                selection = args
                    .next()
                    .ok_or("--only needs entry numbers.")?
                    .split(',')
                    .map(|value| value.parse::<usize>().map_err(|_| "Invalid entry number."))
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => return Err(format!("Unknown or conflicting option: {arg}")),
        }
    }
    let layout = Layout::new(&home, None).map_err(|e| e.to_string())?;
    let host = layout.openagents.join("bin/openagents");
    let facts = || task_facts(&host, &layout.store);
    let provider = host.is_file().then_some(&facts as &dyn crate::plan::Facts);
    match words[0].as_str() {
        "migrate-plugin" => {
            let installed = crate::plugin_list::find(&layout, "disk-cleanup")?;
            for id in &installed.background {
                let rule = crate::plugins::load_rule(&installed, id)?;
                if !rule.actions.is_empty()
                    || !rule.triggers.is_empty()
                    || !rule.needs.delete.is_empty()
                {
                    return Err(
                        "Install the scan-only Disk cleanup package before migrating it.".into(),
                    );
                }
            }
            let plugin = crate::plugins::set_enabled(&layout, "disk-cleanup", true)?;
            println!(
                "{} {} is installed. Its saved rule now uses the package's scan-only settings.",
                plugin.name, plugin.version
            );
        }
        "scan" => {
            if !selection.is_empty() || receipt_file.is_some() {
                return Err("--only and --receipt belong to apply.".into());
            }
            let plan = scan(&layout, options, provider)?;
            if let Some(file) = plan_file {
                save(&file, &plan)?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
                );
            } else {
                println!(
                    "{} entries, {} bytes of old outputs and worktrees.",
                    plan.entries.len(),
                    plan.bytes()
                );
                for (index, entry) in plan.entries.iter().enumerate() {
                    println!(
                        "{}: {} bytes  {}",
                        index + 1,
                        entry.bytes,
                        entry.path.display()
                    );
                }
                println!(
                    "Kept {} folders; {} need a closer look. Nothing deleted.",
                    plan.kept.len(),
                    plan.uncovered.len()
                );
                for entry in &plan.kept {
                    println!("keep {}: {}", entry.path.display(), entry.reason);
                }
                for path in &plan.uncovered {
                    println!("not checked: {}", path.display());
                }
            }
        }
        "apply" => {
            if scan_options {
                return Err(
                    "Apply uses the saved scope and age. Create another plan to change them."
                        .into(),
                );
            }
            let file = plan_file.ok_or("Apply needs --plan FILE.")?;
            let receipt_file = receipt_file.ok_or("Apply needs --receipt FILE.")?;
            if receipt_file.exists() {
                return Err("The receipt file already exists.".into());
            }
            let plan: Plan =
                serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            use std::io::{Seek, Write};
            let mut file = create_record(&receipt_file)?;
            let receipt =
                apply_selected_recorded(&layout, &plan, provider, &selection, |receipt| {
                    let bytes = serde_json::to_vec_pretty(receipt).map_err(|e| e.to_string())?;
                    file.rewind().map_err(|e| e.to_string())?;
                    file.write_all(&bytes).map_err(|e| e.to_string())?;
                    file.set_len(bytes.len() as u64)
                        .map_err(|e| e.to_string())?;
                    file.sync_all().map_err(|e| e.to_string())
                })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?
            );
        }
        _ => return Err(HELP.into()),
    }
    Ok(())
}

// The host owns task semantics. Without a task-aware host, managed task
// worktrees stay. Any task record is conservatively treated as still live.
fn task_facts(host: &Path, store: &Path) -> Result<Vec<crate::records::TaskFact>, String> {
    let output = std::process::Command::new(host)
        .args(["--json", "worktree", "ls", "--no-size", "--tasks"])
        .arg(store)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Cannot check the host's task worktrees.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let rows = value
        .get("worktrees")
        .and_then(serde_json::Value::as_array)
        .ok_or("The host did not return its task worktrees.")?;
    rows.iter()
        .filter(|row| !row.get("task").is_some_and(serde_json::Value::is_null))
        .map(|row| {
            let path = row
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("A task has no worktree path.")?;
            Ok(crate::records::TaskFact {
                worktree: path.into(),
                ended: false,
                ..Default::default()
            })
        })
        .collect()
}
