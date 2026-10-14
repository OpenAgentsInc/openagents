//! `coder solver N --cloud` (#11562): Solver on a Google Cloud box of its
//! own instead of this computer. The run is asked of openagents.com as the
//! signed-in account ([`coder_sdk::cloud_solver`]); the box dispatcher takes
//! it, and this prints its progress lines until it ends.

use std::path::PathBuf;

use coder_sdk::cloud_solver::{self, Account};
use serde_json::Value;

/// What `--cloud` takes from the solver's arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub repo: String,
    pub issue: u64,
    /// One JSON line per progress line instead of text.
    pub json: bool,
    /// Ask and print the run's page without following it.
    pub detach: bool,
}

/// Reads `N`, `#N` or `OWNER/NAME#N`, `--github OWNER/R`, `--repo PATH`
/// (whose GitHub remote names the repository), `--json` and `--detach`.
///
/// # Errors
/// The issue or its repository is not named.
pub fn parse(args: &[String]) -> Result<Ask, String> {
    let mut issue = None;
    let mut github = None;
    let mut repo_dir = None;
    let mut json = false;
    let mut detach = false;
    let mut words = args.iter();
    while let Some(arg) = words.next() {
        match arg.as_str() {
            "--cloud" | "--plain" => {}
            "--json" => json = true,
            "--detach" => detach = true,
            "--github" => github = words.next().cloned(),
            "--repo" => repo_dir = words.next().map(PathBuf::from),
            other if !other.starts_with("--") && issue.is_none() => {
                issue =
                    Some(super::issue_reference(other).ok_or_else(|| {
                        format!("{other} is not an issue: give N or OWNER/NAME#N.")
                    })?);
            }
            other => {
                return Err(format!(
                    "{other} does not work with --cloud: the run's settings are the cloud's."
                ));
            }
        }
    }
    let (named, number) = issue.ok_or("Name the issue: coder solver N --cloud.")?;
    let repo = named
        .or(github)
        .or_else(|| {
            let dir = repo_dir.clone().or_else(|| std::env::current_dir().ok())?;
            crate::issue_run::github_repo(&dir)
        })
        .ok_or_else(|| crate::issue_run::no_github_remote(number))?;
    Ok(Ask {
        repo,
        issue: number,
        json,
        detach,
    })
}

/// One progress line as text: `kind  text`.
#[must_use]
pub fn line_text(line: &Value) -> String {
    let phase = line["phase"].as_str().unwrap_or_default();
    let text = line["text"].as_str().unwrap_or_default();
    if phase.is_empty() {
        text.to_owned()
    } else {
        format!("{phase:<9} {text}")
    }
}

/// The finished run in a few lines: the pull request, the cost, or why it
/// stopped.
#[must_use]
pub fn end_text(view: &Value) -> String {
    let result = &view["result"];
    let mut out = format!(
        "Run {} ended: {}",
        view["id"].as_str().unwrap_or("?"),
        view["state"].as_str().unwrap_or("finished")
    );
    if let Some(pr) = result["pr"].as_str() {
        out.push_str(&format!("\nPull request: {pr}"));
    }
    if let Some(usd) = result["cost_usd"].as_f64() {
        out.push_str(&format!("\nCost: ${usd:.2}"));
    }
    if let Some(why) = view["why"].as_str().filter(|why| !why.is_empty()) {
        out.push_str(&format!("\n{why}"));
    }
    out
}

/// Runs `coder solver N --cloud`.
///
/// # Errors
/// Not signed in, or the site refused or could not be reached.
pub async fn command(args: &[String]) -> Result<(), String> {
    let ask = parse(args)?;
    let saved = crate::home::data_dir()
        .as_deref()
        .and_then(crate::connection_tools::signed_in)
        .ok_or("Solver in the cloud runs as your OpenAgents account: run coder login first.")?;
    let account = Account {
        origin: saved.origin.clone(),
        token: saved.token().to_owned(),
    };
    let queued = cloud_solver::submit(&account, &ask.repo, ask.issue).await?;
    let page = format!("{}{}", account.origin.trim_end_matches('/'), queued.url);
    if ask.json {
        println!(
            "{}",
            serde_json::json!({"id": queued.id, "state": queued.state, "url": page})
        );
    } else {
        println!("Solving {}#{} in the cloud: {page}", ask.repo, ask.issue);
    }
    if ask.detach {
        return Ok(());
    }
    let view = cloud_solver::follow(&account, &queued.id, |line| {
        if ask.json {
            println!("{line}");
        } else {
            println!("{}", line_text(line));
        }
    })
    .await?;
    if ask.json {
        println!("{}", serde_json::json!({"run": view}));
    } else {
        println!("{}", end_text(&view));
    }
    if view["result"]["pr"].is_string() {
        Ok(())
    } else {
        Err("The run ended without a pull request.".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn cloud_takes_the_issue_and_its_repository() {
        assert_eq!(
            parse(&words("AtlantisPleb/solver-box-smoke#3 --cloud")).unwrap(),
            Ask {
                repo: "AtlantisPleb/solver-box-smoke".into(),
                issue: 3,
                json: false,
                detach: false
            }
        );
        assert_eq!(
            parse(&words("7 --cloud --github o/r --json --detach")).unwrap(),
            Ask {
                repo: "o/r".into(),
                issue: 7,
                json: true,
                detach: true
            }
        );
        assert!(parse(&words("7 --cloud --engine claude --github o/r")).is_err());
        assert!(parse(&words("--cloud --github o/r")).is_err());
    }

    #[test]
    fn lines_and_the_end_read_plainly() {
        assert_eq!(
            line_text(&json!({"secs": 3.0, "phase": "box", "text": "Asking for a computer"})),
            "box       Asking for a computer"
        );
        let end = end_text(&json!({
            "id": "w1", "state": "passed", "why": null,
            "result": {"pr": "https://github.com/o/r/pull/4", "cost_usd": 0.4213}
        }));
        assert_eq!(
            end,
            "Run w1 ended: passed\nPull request: https://github.com/o/r/pull/4\nCost: $0.42"
        );
    }
}
