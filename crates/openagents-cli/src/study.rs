//! Microcoder study runs on a linked host, driven and read from here.
//!
//! A study directory on the host holds a runner script, `logs/TASK.ARM.N.log`
//! for each run, `runs/TASK.ARM.N/summary.json` once a run ends, and
//! `outcomes.txt`, one line per finished run. These commands launch a run
//! detached, read progress from the logs, and read outcomes from the
//! summaries, each as one journaled `computer exec` whose result is printed
//! whole, so a reader can follow the study without a shell on the host.

use serde_json::{Value, json};

use crate::Args;
use crate::out;
use crate::{Output, computer};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents study COMMAND HOST DIR [OPTIONS]
  run HOST DIR TASK ARM N [--runner SCRIPT]
                            Start `SCRIPT TASK ARM N` in DIR on the host, detached;
                            prints its pid. SCRIPT defaults to ./one.sh.
  status HOST DIR [--until-done SECONDS]
                            Each run's last logged step, the pid and name of each
                            live microcoder process, and the finished lines from
                            outcomes.txt. With --until-done, the host waits up to
                            SECONDS (at most 1500, inside the terminal's idle
                            life) for every run to end before it reports.
  outcomes HOST DIR         One row per runs/*/summary.json: task, arm, steps,
                            reward, dollars, model, and how it ended.
  faults HOST DIR           Steps whose model call failed, from each run's events.jsonl.
HOST is an alias, key, or key prefix (see `openagents computer alias`).
Options: --store DIR, --wait SECONDS, --timeout SECONDS.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("run", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("outcomes", Effect::ReadOnly),
    Declared::computer("faults", Effect::ReadOnly),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("study", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["same-machine"]).and_then(|args| covering(args, rest)) {
        Ok(args) => args,
        Err(message) => return output.usage("study", &message, USAGE),
    };
    let command = command.as_str();
    let Some(script) = script(command, &args) else {
        return output.usage(
            "study",
            &format!("unknown or incomplete `{command}`"),
            USAGE,
        );
    };
    let words = ["bash", "-c", &script].map(String::from);
    let result = match computer::exec_json(&args, &args.positional()[0], &words) {
        Ok(result) => result,
        Err(message) => return output.fail("study", &message),
    };
    let text = result["output"].as_str().unwrap_or("").to_owned();
    if result["exit"].as_i64() != Some(0) {
        return output.fail(
            "study",
            &format!(
                "the host returned exit {}: {}",
                result["exit"],
                text.trim().lines().last().unwrap_or("no output")
            ),
        );
    }
    let value = match command {
        "run" => json!({ "started": text.trim(), "pid": text.trim().parse::<u64>().ok() }),
        "status" => status(&text),
        "outcomes" => outcomes(&text),
        _ => json!({ "faults": sections(&text) }),
    };
    let value = json!({
        "host": result["host"], "dir": args.positional()[1], "exit": result["exit"],
        "route": result["route"], "seconds": result["seconds"], "study": value,
    });
    output.emit(&value, |v| render(command, v));
    u8::try_from(result["exit"].as_i64().unwrap_or(1)).unwrap_or(255)
}

/// `args`, with the exec `--timeout` raised to cover `--until-done` plus a
/// minute for the report, so a long host-side wait is not cut off here.
fn covering(args: Args, words: &[String]) -> Result<Args, String> {
    let until: u64 = args.number::<u64>("until-done", 0)?.min(1500);
    let timeout: u64 = args.number("timeout", 600)?;
    if until + 60 <= timeout {
        return Ok(args);
    }
    let mut words = words.to_vec();
    words.extend(["--timeout".to_owned(), (until + 60).to_string()]);
    Args::parse(&words, &["same-machine"])
}

/// The shell script one command sends, or `None` when its arguments are short.
fn script(command: &str, args: &Args) -> Option<String> {
    let positional = args.positional();
    let dir = home_relative(positional.get(1)?);
    let each = "for d in runs/*/; do n=${d#runs/}; n=${n%/}; echo \"== $n\"; ";
    Some(match command {
        "run" => {
            let [_, _, task, arm, n, ..] = positional else {
                return None;
            };
            let runner = args.option("runner").unwrap_or("./one.sh");
            let words = [runner, task, arm, n]
                .iter()
                .map(|word| crate::terminal::quote(word))
                .collect::<Vec<_>>()
                .join(" ");
            // The terminal's process group ends with the shell, so the runner
            // starts in its own session, and the shell stays until it is up.
            let pattern =
                crate::terminal::quote(&format!(" {}$", [runner, task, arm, n].join(" ")));
            format!(
                "cd {dir} && mkdir -p logs runs && setsid -f nohup {words} >/dev/null 2>&1 </dev/null; \
                 sleep 1; pgrep -f {pattern} | head -1"
            )
        }
        "status" => format!(
            "cd {dir} && {wait}for f in logs/*.log; do n=${{f#logs/}}; n=${{n%.log}}; \
             echo \"== $n\"; grep -o '^\\[[0-9:]*\\] [a-z ]*step [0-9]*' \"$f\" | tail -1; \
             test -f \"runs/$n/summary.json\" && echo done; done; \
             echo '== outcomes'; cat outcomes.txt 2>/dev/null; \
             echo '== running'; pgrep -l 'microcoder'; true",
            wait = until_done(args.option("until-done"))?,
        ),
        "outcomes" => format!("cd {dir} && {each}cat \"$d/summary.json\" 2>/dev/null; echo; done"),
        "faults" => format!(
            "cd {dir} && {each}grep -oE '\"step\":[0-9]+,.{{0,40}}\"action\":[{{]\"Err\":\"[^\"]{{0,200}}' \
             \"$d/events.jsonl\" 2>/dev/null | tail -5; done; true"
        ),
        _ => return None,
    })
}

/// A host-side loop that polls once a second until no `microcoder` process
/// remains or `seconds` pass, so one command can cover a long run without
/// blind sleeps here. `None` when the option is not a whole number.
fn until_done(seconds: Option<&str>) -> Option<String> {
    match seconds {
        None => Some(String::new()),
        Some(text) => {
            let seconds: u64 = text.parse::<u64>().ok()?.min(1500);
            Some(format!(
                "i=0; while pgrep microcoder >/dev/null && [ $i -lt {seconds} ]; do sleep 1; i=$((i+1)); done; "
            ))
        }
    }
}

/// `path` quoted for the host's shell, with a leading `~/` left to the
/// host's `$HOME` rather than quoted away.
fn home_relative(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("\"$HOME\"/{}", crate::terminal::quote(rest)),
        None if path == "~" => "\"$HOME\"".to_owned(),
        None => crate::terminal::quote(path),
    }
}

/// `== NAME` headers and the text under each, in order.
fn sections(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("== ") {
            out.push((name.to_owned(), String::new()));
        } else if let Some((_, body)) = out.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out
}

fn status(text: &str) -> Value {
    let mut runs = Vec::new();
    let mut outcomes = Vec::new();
    let mut running = Vec::new();
    for (name, body) in sections(text) {
        match name.as_str() {
            "outcomes" => outcomes = body.lines().map(str::to_owned).collect(),
            "running" => running = body.lines().map(str::to_owned).collect(),
            _ => {
                let step = body.lines().find_map(|l| {
                    l.rsplit_once("step ")
                        .and_then(|(_, n)| n.parse::<u64>().ok())
                });
                let clock = body
                    .lines()
                    .find_map(|l| l.strip_prefix('[').and_then(|l| l.split(']').next()))
                    .map(str::to_owned);
                runs.push(json!({
                    "run": name, "step": step, "elapsed": clock,
                    "done": body.lines().any(|l| l == "done"),
                }));
            }
        }
    }
    json!({ "runs": runs, "outcomes": outcomes, "running": running })
}

fn outcomes(text: &str) -> Value {
    let rows: Vec<Value> = sections(text)
        .into_iter()
        .map(
            |(name, body)| match serde_json::from_str::<Value>(body.trim()) {
                Ok(summary) => {
                    let outcome = &summary["outcome"];
                    let mut parts = name.rsplitn(3, '.');
                    let n = parts.next().unwrap_or("");
                    let arm = parts.next().unwrap_or("");
                    json!({
                        "run": name, "task": summary["task"], "arm": arm, "n": n,
                        "steps": outcome["steps"], "seconds": outcome["seconds"],
                        "reward": summary["reward"], "usd": outcome["usd"],
                        "model_usd": outcome["model_usd"],
                        "provider": summary["provider"], "model": summary["model"],
                        "ending": outcome["ending"], "cost_basis": summary["cost_basis"],
                        "reward_unknown_because": summary["reward_unknown_because"],
                    })
                }
                Err(_) => json!({ "run": name, "pending": true }),
            },
        )
        .collect();
    json!({ "runs": rows })
}

fn render(command: &str, value: &Value) -> String {
    let study = &value["study"];
    match command {
        "run" => format!("started, pid {}", study["started"].as_str().unwrap_or("?")),
        "status" => {
            let mut rows = vec![vec![
                "run".to_owned(),
                "step".to_owned(),
                "elapsed".to_owned(),
                "state".to_owned(),
            ]];
            for run in study["runs"].as_array().into_iter().flatten() {
                rows.push(vec![
                    run["run"].as_str().unwrap_or("").to_owned(),
                    run["step"]
                        .as_u64()
                        .map_or("-".to_owned(), |s| s.to_string()),
                    run["elapsed"].as_str().unwrap_or("-").to_owned(),
                    if run["done"].as_bool().unwrap_or(false) {
                        "done"
                    } else {
                        "running"
                    }
                    .to_owned(),
                ]);
            }
            let outcomes: Vec<&str> = study["outcomes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let running = study["running"].as_array().map_or(0, Vec::len);
            format!(
                "{}\n{} finished, {} microcoder processes alive{}",
                out::table(&rows),
                outcomes.len(),
                running,
                if outcomes.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", outcomes.join("\n"))
                }
            )
        }
        "outcomes" => {
            let mut rows = vec![vec![
                "run".to_owned(),
                "reward".to_owned(),
                "steps".to_owned(),
                "time".to_owned(),
                "usd".to_owned(),
                "model".to_owned(),
                "ending".to_owned(),
            ]];
            for run in study["runs"].as_array().into_iter().flatten() {
                if run["pending"].as_bool().unwrap_or(false) {
                    rows.push(vec![
                        run["run"].as_str().unwrap_or("").to_owned(),
                        "-".to_owned(),
                        "-".to_owned(),
                        "-".to_owned(),
                        "-".to_owned(),
                        "-".to_owned(),
                        "pending".to_owned(),
                    ]);
                    continue;
                }
                let seconds = run["seconds"].as_f64().unwrap_or(0.0) as u64;
                rows.push(vec![
                    run["run"].as_str().unwrap_or("").to_owned(),
                    run["reward"]
                        .as_f64()
                        .map_or("unknown".to_owned(), |r| format!("{r}")),
                    run["steps"]
                        .as_u64()
                        .map_or("-".to_owned(), |s| s.to_string()),
                    format!("{}:{:02}", seconds / 60, seconds % 60),
                    run["usd"]
                        .as_f64()
                        .map_or("unknown".to_owned(), |u| format!("{u:.2}")),
                    run["model"].as_str().unwrap_or("").to_owned(),
                    ending(&run["ending"]),
                ]);
            }
            out::table(&rows)
        }
        _ => study["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|pair| {
                let name = pair[0].as_str().unwrap_or("");
                let body = pair[1].as_str().unwrap_or("").trim();
                if body.is_empty() {
                    format!("{name}: none")
                } else {
                    format!("{name}:\n{body}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn ending(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| match v {
                Value::String(s) => format!("{k}: {s}"),
                Value::Null => k.clone(),
                other => format!("{k}: {other}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Value::Null => "-".to_owned(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_waits_on_the_host_and_covers_the_wait_here() {
        let words: Vec<String> = ["c", "~/s", "--until-done", "900"]
            .map(String::from)
            .to_vec();
        let args = covering(Args::parse(&words, &[]).unwrap(), &words).unwrap();
        assert_eq!(args.option("timeout"), Some("960"));
        let text = script("status", &args).unwrap();
        assert!(text.contains("[ $i -lt 900 ]"));
        assert!(text.contains("pgrep -l 'microcoder'"));
        assert!(!text.contains("pgrep -af"));
        assert_eq!(
            until_done(Some("9000")).unwrap(),
            until_done(Some("1500")).unwrap()
        );
        assert!(until_done(Some("soon")).is_none());
        let plain = Args::parse(&words[..2], &[]).unwrap();
        assert!(!script("status", &plain).unwrap().contains("while"));
    }

    #[test]
    fn status_reads_steps_and_finished_runs() {
        let text = "== t.base.1\n[22:23] step 23\ndone\n== t.full.1\n[28:59] oracle step 13\n== outcomes\nline one\n== running\n123 microcoder-study t\n";
        let value = status(text);
        assert_eq!(value["runs"][0]["step"], 23);
        assert_eq!(value["runs"][0]["done"], true);
        assert_eq!(value["runs"][1]["step"], 13);
        assert_eq!(value["runs"][1]["elapsed"], "28:59");
        assert_eq!(value["runs"][1]["done"], false);
        assert_eq!(value["outcomes"].as_array().unwrap().len(), 1);
        assert_eq!(value["running"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn outcomes_reads_summaries_and_marks_pending_runs() {
        let text = "== sound.base.1\n{\"task\":\"sound\",\"reward\":1.0,\"model\":\"opus\",\"provider\":\"claude\",\"outcome\":{\"steps\":23,\"seconds\":1342.7,\"usd\":5.84,\"ending\":{\"reason\":\"finished\"}}}\n== sound.full.1\n\n";
        let value = outcomes(text);
        let rows = value["runs"].as_array().unwrap();
        assert_eq!(rows[0]["arm"], "base");
        assert_eq!(rows[0]["n"], "1");
        assert_eq!(rows[0]["steps"], 23);
        assert_eq!(rows[1]["pending"], true);
        let rendered = render("outcomes", &json!({ "study": value }));
        assert!(rendered.contains("22:22"));
        assert!(rendered.contains("reason: finished"));
        assert!(rendered.contains("pending"));
    }

    #[test]
    fn home_relative_paths_reach_the_host_home() {
        assert_eq!(home_relative("~/runs"), "\"$HOME\"/runs");
        assert_eq!(home_relative("~/my runs"), "\"$HOME\"/'my runs'");
        assert_eq!(home_relative("/tmp/x"), "/tmp/x");
    }

    #[test]
    fn run_script_quotes_its_arguments() {
        let args = Args::parse(
            &[
                "h",
                "~/d",
                "task",
                "full",
                "1",
                "--runner",
                "./one claude.sh",
            ]
            .map(String::from),
            &[],
        )
        .unwrap();
        let text = script("run", &args).unwrap();
        assert!(text.contains("setsid -f nohup './one claude.sh' task full 1"));
        assert!(text.ends_with("pgrep -f ' ./one claude.sh task full 1$' | head -1"));
        assert!(script("run", &Args::parse(&["h".to_owned()], &[]).unwrap()).is_none());
    }
}
