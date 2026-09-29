//! `chat-load-bench`: time each phase of loading chats on the phone.
//!
//! ```text
//! chat-load-bench [--source coder-fixture|coder|fixture|fixture-large|real] [--relay fixture|URL]
//!                 [--runs N] [--chats K] [--basic-coder N] [--basic-coder-local N] [--no-relay] [--no-direct] [--no-cold]
//! ```
//!
//! The defaults use 80 synthetic Coder task transcripts and the loopback
//! relay, so they need no network and read nothing private. `--source coder`
//! reads this machine's Coder task transcripts (`--source real` adds
//! `~/.claude` and `~/.codex`); `--relay wss://relay.openagents.com` carries
//! relay reads over the real relay with a fresh, temporary host key; and
//! `--basic-coder N` sends N messages to the OpenAgents chat worker. Titles,
//! paths, and message text are never printed. Read
//! `docs/coder/runtime/chat-load-benchmark.md`.

use chat_load_bench::bench::{self, Options, Relay, Source};
use chat_load_bench::dataset::Scale;
use std::path::PathBuf;

const USAGE: &str = "chat-load-bench [--source coder-fixture|coder|fixture|fixture-large|real] [--relay fixture|wss://URL] [--runs N] [--chats K] [--basic-coder N] [--basic-coder-local N] [--no-relay] [--no-direct] [--no-cold]";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("internal-cold") {
        std::process::exit(internal_cold(&arguments[1..]));
    }
    let options = match parse(&arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\nusage: {USAGE}");
            std::process::exit(2);
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    match runtime.block_on(bench::run(options.clone())) {
        Ok(report) => {
            println!("# Chat load benchmark\n");
            println!(
                "Source: {:?}; relay: {:?}; runs: {}; chats opened per run: {}\n",
                options.source, options.relay, options.runs, options.chats
            );
            println!("Dataset: {}\n", report.dataset);
            for fact in &report.facts {
                println!("- {fact}");
            }
            println!("{}", chat_load_bench::stats::markdown(&report.phases));
        }
        Err(message) => {
            eprintln!("chat-load-bench: {message}");
            std::process::exit(1);
        }
    }
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options {
        source: Source::CoderFixture(80),
        relay: Relay::Fixture,
        runs: 5,
        chats: 5,
        use_relay: true,
        use_direct: true,
        basic_coder: 0,
        basic_coder_local: 0,
        exe: std::env::current_exe().ok(),
    };
    let mut rest = arguments.iter();
    while let Some(flag) = rest.next() {
        let mut value = || rest.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--source" => {
                options.source = match value()?.as_str() {
                    "coder-fixture" => Source::CoderFixture(80),
                    "coder" => Source::Coder,
                    "fixture" => Source::Fixture(Scale::CI),
                    "fixture-large" => Source::Fixture(Scale::LARGE),
                    "real" => Source::Real,
                    other => return Err(format!("unknown source {other}")),
                }
            }
            "--relay" => {
                options.relay = match value()?.as_str() {
                    "fixture" => Relay::Fixture,
                    url if url.starts_with("wss://") => Relay::Url(url.into()),
                    other => return Err(format!("unknown relay {other}")),
                }
            }
            "--runs" => options.runs = value()?.parse().map_err(|_| "--runs needs a number")?,
            "--chats" => options.chats = value()?.parse().map_err(|_| "--chats needs a number")?,
            "--basic-coder" => {
                options.basic_coder = value()?
                    .parse()
                    .map_err(|_| "--basic-coder needs a number")?;
            }
            "--basic-coder-local" => {
                options.basic_coder_local = value()?
                    .parse()
                    .map_err(|_| "--basic-coder-local needs a number")?;
            }
            "--no-relay" => options.use_relay = false,
            "--no-direct" => options.use_direct = false,
            "--no-cold" => options.exe = None,
            "--help" | "-h" => return Err("chat load benchmark".into()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if options.runs == 0 {
        return Err("--runs must be at least 1".into());
    }
    Ok(options)
}

fn internal_cold(arguments: &[String]) -> i32 {
    let mut config = coder_history::Config::default();
    let mut rest = arguments.iter();
    while let (Some(flag), Some(path)) = (rest.next(), rest.next()) {
        let path = Some(PathBuf::from(path));
        match flag.as_str() {
            "--codex" => config.codex = path,
            "--claude" => config.claude = path,
            "--coder" => config.coder = path,
            "--opencode" => config.opencode = path,
            "--devin" => config.devin = path,
            _ => return 2,
        }
    }
    match bench::cold(config) {
        Ok(sample) => {
            println!("{}", serde_json::to_string(&sample).unwrap_or_default());
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}
