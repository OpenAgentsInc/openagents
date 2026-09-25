//! The fire loop: watch a Coder One trial step by step, judge each step
//! against how Fable 5.1 won the same task, and stop the run the moment
//! it drifts.
//!
//! A benchmark trial takes 10 to 40 minutes and says only pass or fail at
//! the end. The fire loop gives an answer in seconds instead. It follows
//! the trial's live logs, which the harness copies to the host every few
//! seconds, and prints every event in full: each component that starts and
//! the digest of its parameters, each Jev request with its state,
//! questions, and answers, each model turn with its tokens and cost, and
//! each command with its output. After every action it checks code rules
//! and asks Jev to compare the run with a strategy card, a file that
//! describes how Fable's winning runs solved the task in phases, with the
//! times they reached each one. When a rule holds, or Jev judges the run
//! off course twice in a row, the loop stops the run and writes a report
//! that says what went wrong, with the evidence.
//!
//! The card is read only by the judge. Nothing from it reaches the run
//! under test, so a fire loop run is a development tool: every task it
//! runs on is in-sample.
//!
//! `coder-one fire replay` runs the same judge over a finished trial's
//! logs, to see where the loop would have stopped it and whether that run
//! had in fact passed. `scripts/fire-loop.sh` starts trials and watches
//! them.

pub mod card;
pub mod events;
pub mod judge;
pub mod report;
pub mod show;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use card::Card;
use events::{Event, Follower};
use judge::{Rules, Run, Stop};
use report::Ending;
use show::Style;

use crate::component::jev::JevMode;

pub const USAGE: &str =
    "usage: coder-one fire watch --card FILE (--job DIR | --trial DIR) [options]
       coder-one fire replay --card FILE TRIAL_DIR [options]
       coder-one fire card FILE

watch follows a running trial's live logs; replay reads a finished trial's
logs. Both print every event in full, judge each action against the card,
and stop at the first rule that holds.

Options:
  --policy FILE        print this policy manifest before the run
  --jev live|off       judge with Jev (default live); off keeps the code rules only
  --clip N             keep N characters of each long field (default 0: all)
  --budget-x X         stop past X times the winners' times (default 4)
  --max-usd N          stop past N dollars of logged spend (default 0.50)
  --stop-p P           Jev's stop answer that votes to stop (default 0.85)
  --pitfall-p P        Jev's stop answer that votes to stop with a named pitfall (default 0.7)
  --idle-s S           stop after S seconds with no action from the model (default 300)
  --votes N            votes in a row that stop the run (default 2)
  --no-stop            judge and report, but never stop
  --on-stop CMD        run CMD with sh when the loop stops a watched trial;
                       FIRE_TRIAL and FIRE_TRIAL_DIR name the trial
  --out DIR            where the report goes (default: the trial's fire/ folder)
  --verifier-wait S    seconds to wait for the verifier after the run ends (default 900)
  --speed N            replay at N times real speed (default 0: no waiting)

Exit codes: 0 the run finished and passed, 1 it finished without a pass or
the result is unknown, 3 the fire loop stopped it, 2 bad arguments.";

struct Options {
    card: Option<PathBuf>,
    job: Option<PathBuf>,
    trial: Option<PathBuf>,
    policy: Option<PathBuf>,
    jev: String,
    clip: usize,
    rules: Rules,
    no_stop: bool,
    on_stop: Option<String>,
    out: Option<PathBuf>,
    verifier_wait: u64,
    speed: f64,
}

fn parse(args: &[String]) -> Result<(Options, Vec<String>), String> {
    let mut options = Options {
        card: None,
        job: None,
        trial: None,
        policy: None,
        jev: "live".to_string(),
        clip: 0,
        rules: Rules::default(),
        no_stop: false,
        on_stop: None,
        out: None,
        verifier_wait: 900,
        speed: 0.0,
    };
    let mut positional = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        let number = |text: String| {
            text.parse::<f64>()
                .map_err(|_| format!("{arg} wants a number, not {text}"))
        };
        match arg.as_str() {
            "--card" => options.card = Some(PathBuf::from(value()?)),
            "--job" => options.job = Some(PathBuf::from(value()?)),
            "--trial" => options.trial = Some(PathBuf::from(value()?)),
            "--policy" => options.policy = Some(PathBuf::from(value()?)),
            "--jev" => options.jev = value()?,
            "--clip" => options.clip = number(value()?)? as usize,
            "--budget-x" => options.rules.budget_x = number(value()?)?,
            "--max-usd" => options.rules.max_usd = number(value()?)?,
            "--stop-p" => options.rules.stop_p = number(value()?)?,
            "--pitfall-p" => options.rules.pitfall_p = number(value()?)?,
            "--idle-s" => options.rules.idle_s = number(value()?)?,
            "--votes" => options.rules.votes = (number(value()?)? as usize).max(1),
            "--no-stop" => options.no_stop = true,
            "--on-stop" => options.on_stop = Some(value()?),
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--verifier-wait" => options.verifier_wait = number(value()?)? as u64,
            "--speed" => options.speed = number(value()?)?,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positional.push(arg.clone()),
        }
    }
    Ok((options, positional))
}

/// Runs a fire command and returns the exit code.
///
/// # Errors
///
/// A message for bad arguments or a run that can't start.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let (options, positional) = parse(rest)?;
    let style = Style::detect(options.clip);
    match verb.as_str() {
        "card" => {
            let path = positional.first().ok_or(USAGE)?;
            let card = Card::load(Path::new(path))?;
            println!("{}", show::card(style, &card));
            Ok(0)
        }
        "watch" | "replay" => {
            let card = Card::load(options.card.as_deref().ok_or("--card is required")?)?;
            let mode = match options.jev.as_str() {
                "live" => JevMode::Live(crate::component::cli::live_client()?),
                "off" => JevMode::Off,
                other => return Err(format!("--jev takes live or off, not {other}")),
            };
            if let Some(policy) = &options.policy {
                let text = std::fs::read_to_string(policy)
                    .map_err(|error| format!("can't read {}: {error}", policy.display()))?;
                println!("Policy manifest {}:\n{text}", policy.display());
            }
            println!("{}\n", show::card(style, &card));
            if verb == "replay" {
                let trial = PathBuf::from(positional.first().ok_or(USAGE)?);
                replay(&options, &card, &mode, style, &trial).await
            } else {
                watch(&options, &card, &mode, style).await
            }
        }
        _ => Err(USAGE.to_string()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The folder a trial's logs are in: the live copy while it runs, the
/// episode bundle once it has ended.
fn logs_dir(trial: &Path) -> PathBuf {
    let live = trial.join("agent/live");
    if live.join("episode.atif.jsonl").is_file() {
        live
    } else {
        trial.join("agent/episode")
    }
}

/// The verifier's reward, when the trial has one.
fn reward(trial: &Path) -> Option<f64> {
    std::fs::read_to_string(trial.join("verifier/reward.txt"))
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// What the loop does after it takes in some events.
struct Loop<'a> {
    options: &'a Options,
    card: &'a Card,
    mode: &'a JevMode,
    style: Style,
    run: Run,
    judged: usize,
}

impl Loop<'_> {
    /// Prints and takes in `events`, then checks the rules and judges the
    /// newest action. Returns the stop, when there is one.
    async fn take(&mut self, events: &[Event], now: u64) -> Option<Stop> {
        for event in events {
            self.run.see(event);
            println!(
                "{}",
                show::event(self.style, event, self.run.seconds(event.at))
            );
        }
        let stop = judge::rules(&self.run, self.card, &self.options.rules, now);
        if stop.is_some() {
            return stop.filter(|_| !self.options.no_stop);
        }
        if self.run.actions.len() > self.judged && !matches!(self.mode, JevMode::Off) {
            self.judged = self.run.actions.len();
            let judgment =
                judge::judge(self.mode, &self.run, self.card, &self.options.rules, now).await;
            println!("{}", show::judgment(self.style, &judgment));
            self.run.judge_usd += judgment.usd;
            self.run.judgments.push(judgment);
            if let Some(stop) = judge::jev_stop(&self.run, &self.options.rules) {
                return (!self.options.no_stop).then_some(stop);
            }
        }
        None
    }

    fn finish(
        &self,
        ending: &Ending,
        trial: &Path,
        out: &Path,
        reward: Option<f64>,
    ) -> Result<(), String> {
        let name = trial.file_name().map_or("trial".to_string(), |name| {
            name.to_string_lossy().to_string()
        });
        std::fs::create_dir_all(out)
            .map_err(|error| format!("can't make {}: {error}", out.display()))?;
        let markdown = report::markdown(&self.run, self.card, ending, &name, reward);
        let json = report::json(&self.run, self.card, ending, &name, reward);
        std::fs::write(out.join("report.md"), &markdown)
            .and_then(|()| {
                std::fs::write(
                    out.join("report.json"),
                    serde_json::to_string_pretty(&json).unwrap_or_default(),
                )
            })
            .map_err(|error| format!("can't write the report: {error}"))?;
        println!("\n{markdown}");
        println!("Report: {}", out.join("report.md").display());
        Ok(())
    }
}

async fn replay(
    options: &Options,
    card: &Card,
    mode: &JevMode,
    style: Style,
    trial: &Path,
) -> Result<i32, String> {
    let events = events::load(&logs_dir(trial));
    if events.is_empty() {
        return Err(format!("no logs under {}", trial.display()));
    }
    let actual = reward(trial);
    let mut state = Loop {
        options,
        card,
        mode,
        style,
        run: Run::default(),
        judged: 0,
    };
    let mut previous = events[0].at;
    let mut ending = Ending::Unknown;
    for event in &events {
        if options.speed > 0.0 && event.at > previous {
            let wait = (event.at - previous) as f64 / options.speed;
            tokio::time::sleep(Duration::from_millis(wait as u64)).await;
        }
        previous = event.at;
        if let Some(stop) = state.take(std::slice::from_ref(event), event.at).await {
            println!("\n{}", show::stop(style, &stop));
            ending = Ending::Stopped(stop);
            break;
        }
        if state.run.ended {
            ending = Ending::Finished(actual);
        }
    }
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| trial.join("fire-replay"));
    state.finish(&ending, trial, &out, actual)?;
    Ok(match ending {
        Ending::Stopped(_) => 3,
        Ending::Finished(Some(r)) if r >= 1.0 => 0,
        _ => 1,
    })
}

/// The trial folder inside a job, once the harness has made it.
fn trial_in(job: &Path) -> Option<PathBuf> {
    std::fs::read_dir(job)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("__"))
        })
}

async fn watch(
    options: &Options,
    card: &Card,
    mode: &JevMode,
    style: Style,
) -> Result<i32, String> {
    let begun = now_ms();
    let trial = if let Some(trial) = &options.trial {
        trial.clone()
    } else {
        let job = options.job.as_ref().ok_or("watch needs --job or --trial")?;
        println!(
            "Waiting for the harness to create the trial in {}",
            job.display()
        );
        loop {
            if let Some(trial) = trial_in(job) {
                break trial;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    };
    println!("Trial: {}", trial.display());
    let live = trial.join("agent/live");
    let mut follower = Follower::default();
    let mut harbor = 0usize;
    let mut state = Loop {
        options,
        card,
        mode,
        style,
        run: Run::default(),
        judged: 0,
    };
    let mut quiet = 0u64;
    let ending = loop {
        // The harness's own log, while the environment starts.
        if state.run.started.is_none()
            && let Ok(text) = std::fs::read_to_string(trial.join("trial.log"))
        {
            for line in text.lines().skip(harbor) {
                println!("{} {line}", style_harbor(style, now_ms() - begun));
            }
            harbor = text.lines().count();
        }
        let events = follower.poll(&live);
        let now = if state.run.started.is_some() {
            now_ms().max(state.run.last)
        } else {
            0
        };
        if events.is_empty() {
            quiet += 1;
        } else {
            quiet = 0;
        }
        if let Some(stop) = state.take(&events, now).await {
            println!("\n{}", show::stop(style, &stop));
            if let Some(command) = &options.on_stop {
                println!("Stopping the trial: {command}");
                let name = trial
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .env("FIRE_TRIAL", name)
                    .env("FIRE_TRIAL_DIR", &trial)
                    .status();
                if let Err(error) = status {
                    println!("The stop command failed to start: {error}");
                }
            }
            break Ending::Stopped(stop);
        }
        if state.run.ended {
            println!(
                "The episode ended. Waiting up to {} s for the verifier.",
                options.verifier_wait
            );
            let deadline = now_ms() + options.verifier_wait * 1000;
            while reward(&trial).is_none()
                && !trial.join("result.json").is_file()
                && now_ms() < deadline
            {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            break Ending::Finished(reward(&trial));
        }
        if state.run.started.is_none() && trial.join("result.json").is_file() {
            println!(
                "The trial ended before its episode started; see {}",
                trial.join("trial.log").display()
            );
            break Ending::Unknown;
        }
        if quiet > 0 && quiet.is_multiple_of(60) && state.run.started.is_some() {
            println!(
                "{}",
                style_harbor(style, now_ms() - begun)
                    + &format!(" no new events for {} s", quiet / 2)
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    let out = options.out.clone().unwrap_or_else(|| trial.join("fire"));
    let final_reward = reward(&trial);
    state.finish(&ending, &trial, &out, final_reward)?;
    Ok(match ending {
        Ending::Stopped(_) => 3,
        Ending::Finished(Some(r)) if r >= 1.0 => 0,
        _ => 1,
    })
}

fn style_harbor(style: Style, elapsed_ms: u64) -> String {
    let text = format!(
        "[{}] {:<14}",
        show::clock(elapsed_ms as f64 / 1000.0),
        "harness"
    );
    if style.color {
        format!("\x1b[2m{text}\x1b[0m")
    } else {
        text
    }
}

#[cfg(test)]
mod tests;
