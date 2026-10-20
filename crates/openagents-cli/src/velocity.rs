//! `openagents velocity` (#12212): the day's delivery numbers that the
//! 2026-10-19 velocity audit counted by hand, over
//! `coder_cloud::velocity`. Foreman's first check-in after 13:00 UTC each
//! day carries the same summary.

use std::path::{Path, PathBuf};

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_cloud::velocity;

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents velocity [--hours N] [--repo OWNER/NAME] [--decision-log FILE] [--fixture FILE]
Print the delivery numbers for the last N hours (default 24, up to 720):
commits on main by author; issues filed and closed, with the median and
p90 hours from filing to close; landing queue entries by state, the landed
median, and bounces grouped by cause; failed cloud runs by cause; and the
minutes the coordinator's check-ins were errored or empty. It reads
`git log origin/main` in this checkout, `gh issue list` for --repo
(default OpenAgentsInc/openagents), the landing queue, the cloud run
records in ~/.openagents/cloud/runs.jsonl, and the newest decision log of
the desktop app's agents (--decision-log FILE names another). A reading
that fails shows as not known, never as zero. --fixture FILE replays a
recorded input (the --json input shape of coder_cloud::velocity::Input)
and reads nothing else. Under --json it prints the report as one object.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("velocity", &message, USAGE),
    };
    if let Some(word) = args.positional().first() {
        return output.usage("velocity", &format!("unexpected word `{word}`"), USAGE);
    }
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !matches!(*name, "hours" | "repo" | "decision-log" | "fixture"))
    {
        return output.usage("velocity", &format!("unknown option `--{name}`"), USAGE);
    }
    let hours = match args.option("hours").map(str::parse::<u64>) {
        None => 24,
        Some(Ok(hours)) if (1..=720).contains(&hours) => hours,
        Some(_) => return output.usage("velocity", "--hours is 1 to 720", USAGE),
    };
    let input = if let Some(path) = args.option("fixture") {
        match std::fs::read_to_string(path)
            .map_err(|e| format!("can't read {path}: {e}"))
            .and_then(|text| {
                serde_json::from_str::<velocity::Input>(&text)
                    .map_err(|e| format!("{path} isn't a velocity input: {e}"))
            }) {
            Ok(input) => input,
            Err(message) => return output.fail("velocity", &message),
        }
    } else {
        let repo = args.option("repo").unwrap_or("OpenAgentsInc/openagents");
        if repo.split('/').count() != 2 || repo.starts_with('-') {
            return output.usage("velocity", "--repo is OWNER/NAME", USAGE);
        }
        let checkout = std::env::current_dir().ok().and_then(|dir| top(&dir));
        let sources = velocity::Sources::here(
            repo,
            checkout,
            args.option("decision-log").map(PathBuf::from),
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        crate::runtime().block_on(velocity::collect(&sources, hours, now))
    };
    let mut input = input;
    if args.option("hours").is_some() {
        input.hours = hours;
    }
    let report = velocity::measure(&input);
    let value = serde_json::to_value(&report).unwrap_or_default();
    output.emit(&value, |_| report.text());
    0
}

/// The checkout `dir` is in.
fn top(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|path| path.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn a_recorded_day_prints_and_bad_options_are_usage_errors() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../coder-cloud/tests/fixtures/velocity/2026-10-19.json"
        );
        let output = Output::new(true);
        assert_eq!(run(&output, &words(&["--fixture", fixture])), 0);
        assert_eq!(run(&output, &words(&["--hours", "0"])), crate::EXIT_USAGE);
        assert_eq!(run(&output, &words(&["--nope", "1"])), crate::EXIT_USAGE);
        assert_eq!(run(&output, &words(&["extra"])), crate::EXIT_USAGE);
    }
}
