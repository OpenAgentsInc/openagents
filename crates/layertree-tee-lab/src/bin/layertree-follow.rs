//! Research-only signet/regtest follower. Credentials come from environment variables.
use layertree_tee_lab::follow_live::{self, Config};
use std::{path::PathBuf, process::ExitCode};
fn run() -> Result<u8, String> {
    let mut input = std::env::args().skip(1).peekable();
    if input.peek().is_some_and(|arg| arg == "publish") {
        input.next();
        let mut relay = None;
        let mut directory = None;
        let mut receipts = None;
        let mut key_env = "LAYERTREE_MIRROR_SECRET".to_string();
        while let Some(arg) = input.next() {
            let value = input
                .next()
                .ok_or_else(|| format!("{arg} needs a value."))?;
            match arg.as_str() {
                "--relay" => relay = Some(value),
                "--directory" => directory = Some(PathBuf::from(value)),
                "--receipts" => receipts = Some(PathBuf::from(value)),
                "--secret-env" => key_env = value,
                _ => return Err(format!("Unknown publication option: {arg}.")),
            }
        }
        follow_live::publish::directory(
            &relay.ok_or("Choose --relay URL.")?,
            &directory.ok_or("Choose --directory PATH.")?,
            &receipts.ok_or("Choose --receipts FILE.")?,
            &key_env,
        )?;
        return Ok(0);
    }
    let mut path = None;
    let mut once = false;
    let mut until = None;
    let mut args = input;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => path = Some(PathBuf::from(args.next().ok_or("--config needs a file.")?)),
            "--once" => once = true,
            "--until" => {
                until = Some(
                    args.next()
                        .ok_or("--until needs Unix seconds.")?
                        .parse::<u64>()
                        .map_err(|_| "Choose an end time in Unix seconds.")?,
                )
            }
            "--help" | "-h" => {
                println!(
                    "layertree-follow --config FILE (--once | --until UNIX_SECONDS)\nlayertree-follow publish --relay URL --directory PATH --receipts FILE [--secret-env NAME]"
                );
                return Ok(0);
            }
            _ => return Err(format!("Unknown option: {arg}.")),
        }
    }
    follow_live::run(
        Config::load(&path.ok_or("Choose --config FILE.")?)?,
        once,
        until,
    )
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
