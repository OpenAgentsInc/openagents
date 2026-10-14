//! `code-graph`: build, publish and query the code graph (#11622).
//!
//! ```text
//! code-graph build [--root DIR] [--rev REV | --worktree] [--out FILE]
//! code-graph publish [--root DIR] [--rev origin/main] [--remote origin] [--force]
//! code-graph query TOOL JSON [--root DIR]
//! ```
//!
//! `build` prints the counts and the time to stderr; with `--out` it
//! writes the graph (`.gz` compresses it). `publish` writes the
//! `code-graph` branch (see `store`). `query` runs one `graph_*` tool, as
//! Coder does.

use std::path::PathBuf;
use std::process::ExitCode;

use code_graph::{build, store, tools};

fn take(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.remove(at);
    (at < args.len()).then(|| args.remove(at))
}

fn flag(args: &mut Vec<String>, flag: &str) -> bool {
    match args.iter().position(|arg| arg == flag) {
        Some(at) => {
            args.remove(at);
            true
        }
        None => false,
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: code-graph build [--root DIR] [--rev REV | --worktree] [--out FILE]\n       code-graph publish [--root DIR] [--rev REV] [--remote NAME] [--force]\n       code-graph query TOOL JSON [--root DIR]";
    if args.is_empty() {
        return Err(usage.into());
    }
    let command = args.remove(0);
    let root = PathBuf::from(take(&mut args, "--root").unwrap_or_else(|| ".".into()));
    match command.as_str() {
        "build" => {
            let out = take(&mut args, "--out");
            let source = if flag(&mut args, "--worktree") {
                build::Source::Worktree { root }
            } else {
                build::Source::Commit {
                    root,
                    rev: take(&mut args, "--rev").unwrap_or_else(|| "HEAD".into()),
                }
            };
            let built = build::build(&source)?;
            let bytes = built.graph.to_bytes();
            let header = &built.graph.header;
            eprintln!(
                "commit {}{}: {} files ({} MB of Rust, {} with parse errors), {} nodes, {} edges, built in {} ms, {} bytes",
                header.commit,
                if header.dirty { " (dirty)" } else { "" },
                header.files,
                built.source_bytes / 1_000_000,
                built.files_with_parse_errors,
                header.nodes,
                header.edges,
                built.millis,
                bytes.len()
            );
            if let Some(out) = out {
                let data = if out.ends_with(".gz") {
                    store::compress(&bytes)
                } else {
                    bytes
                };
                std::fs::write(&out, data).map_err(|error| format!("{out}: {error}"))?;
            }
            Ok(())
        }
        "publish" => {
            let rev = take(&mut args, "--rev").unwrap_or_else(|| "origin/main".into());
            let remote = take(&mut args, "--remote");
            let force = flag(&mut args, "--force");
            let published = store::publish(&root, &rev, remote.as_deref(), force)?;
            match published.branch_commit {
                Some(branch) => eprintln!(
                    "published the graph of {} as {branch}: {} nodes, {} edges, built in {} ms, {} bytes compressed",
                    published.commit,
                    published.nodes,
                    published.edges,
                    published.millis,
                    published.bytes
                ),
                None => eprintln!(
                    "{}: no code changed since the published graph",
                    published.commit
                ),
            }
            Ok(())
        }
        "query" => {
            if args.len() != 2 {
                return Err(usage.into());
            }
            let arguments: serde_json::Value = serde_json::from_str(&args[1])
                .map_err(|error| format!("The arguments are not JSON: {error}"))?;
            let result = tools::execute(&args[0], &arguments, &root)?;
            println!("graph {}", result["graph"].as_str().unwrap_or_default());
            print!("{}", result["answer"].as_str().unwrap_or_default());
            Ok(())
        }
        _ => Err(usage.into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("code-graph: {error}");
            ExitCode::FAILURE
        }
    }
}
