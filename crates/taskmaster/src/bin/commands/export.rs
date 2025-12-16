//! Export issues to disk or stdout

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use clap::{Args, ValueEnum};
use taskmaster::{IssueFilter, IssueRepository, Result};

/// Supported export formats
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ExportFormat {
    /// Newline-delimited JSON (one issue per line)
    Jsonl,
    /// Single JSON array
    Json,
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Json => "json",
        };
        f.write_str(label)
    }
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Export format (json or jsonl)
    #[arg(long, value_enum, default_value = "jsonl")]
    format: ExportFormat,

    /// Include tombstoned issues
    #[arg(long)]
    include_tombstones: bool,
}

pub fn run(repo: &impl IssueRepository, args: ExportArgs) -> Result<()> {
    let mut filter = IssueFilter::new();
    if args.include_tombstones {
        filter.include_tombstones = true;
    }

    let issues = repo.list(filter)?;

    let mut writer: Box<dyn Write> = match &args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            Box::new(BufWriter::new(File::create(path)?))
        }
        None => Box::new(BufWriter::new(io::stdout())),
    };

    match args.format {
        ExportFormat::Jsonl => {
            for issue in &issues {
                serde_json::to_writer(&mut writer, issue)?;
                writer.write_all(b"\n")?;
            }
        }
        ExportFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &issues)?;
            writer.write_all(b"\n")?;
        }
    }

    writer.flush()?;

    if let Some(path) = args.output {
        println!(
            "Exported {} issues to {} ({})",
            issues.len(),
            path.display(),
            args.format
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskmaster::{Issue, IssueCreate, Priority, SqliteRepository};
    use tempfile::tempdir;

    #[test]
    fn exports_to_jsonl_file() {
        let repo = SqliteRepository::in_memory().unwrap();
        repo.create(IssueCreate::new("First").priority(Priority::High), "tm")
            .unwrap();
        repo.create(IssueCreate::new("Second"), "tm").unwrap();

        let dir = tempdir().unwrap();
        let path = dir.path().join("issues.jsonl");

        let args = ExportArgs {
            output: Some(path.clone()),
            format: ExportFormat::Jsonl,
            include_tombstones: false,
        };

        run(&repo, args).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = contents.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: Issue = serde_json::from_str(lines[0]).unwrap();
        let second: Issue = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(first.title, "First");
        assert_eq!(second.title, "Second");
    }

    #[test]
    fn exports_to_json_array() {
        let repo = SqliteRepository::in_memory().unwrap();
        repo.create(IssueCreate::new("Only"), "tm").unwrap();

        let dir = tempdir().unwrap();
        let path = dir.path().join("issues.json");

        let args = ExportArgs {
            output: Some(path.clone()),
            format: ExportFormat::Json,
            include_tombstones: false,
        };

        run(&repo, args).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.trim_start().starts_with('['));

        let exported: Vec<Issue> = serde_json::from_str(&contents).unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].title, "Only");
    }
}
