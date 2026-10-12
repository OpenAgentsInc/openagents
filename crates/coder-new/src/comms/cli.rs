//! The `comms` command line, so an agent outside Coder's process reaches
//! the same channel the `comms` tool does.

use std::io;
use std::path::PathBuf;

/// `coder comms` (and `openagents coder comms`): talk to the other
/// subagents in a session over the agent channel, the command
/// [`super::preamble`] tells a delegated agent to run.
///
/// # Errors
/// A missing or unknown option, or a channel that can't be read or written.
pub fn command(rest: &[String]) -> io::Result<()> {
    const USAGE: &str = "Usage: coder comms SUBCOMMAND --channel DIR --as NAME [OPTIONS]\n\n\
Talk to the other subagents in a session over the agent channel.\n\n\
Subcommands:\n\
  post    --to NAME|all --text TEXT       Post a message.\n\
  read    [--since N]                     Print what is new (JSON).\n\
  intent  --file PATH [--file PATH] [--resource KIND:VALUE]   Say what you will change.\n\
  lease   acquire|renew|release --kind KIND --value VALUE     Take, keep or free a scarce thing.\n\
  agree   --with NAME --outcome first|split|merge --why TEXT  Settle an overlap.\n\
  status                                  Print the channel as JSON.\n\n\
Lease kinds: build-folder, build-host, deploy-slot, migration, port.";
    let mut args = rest.iter();
    let action = match args.next() {
        Some(action) if !matches!(action.as_str(), "--help" | "-h") => action.clone(),
        _ => {
            println!("{USAGE}");
            return Ok(());
        }
    };
    let mut channel_dir = None;
    let mut name = None;
    let mut to = "all".to_owned();
    let mut text = None;
    let mut since = 0usize;
    let mut files = Vec::new();
    let mut resources = Vec::new();
    let mut kind = None;
    let mut value = None;
    let mut with = None;
    let mut outcome = None;
    let mut why = String::new();
    let mut detail = String::new();
    let mut json = false;
    while let Some(arg) = args.next() {
        let mut next = |name: &str| {
            args.next().cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} needs a value."),
                )
            })
        };
        match arg.as_str() {
            "--channel" => channel_dir = Some(PathBuf::from(next("--channel")?)),
            "--as" => name = Some(next("--as")?),
            "--to" => to = next("--to")?,
            "--text" => text = Some(next("--text")?),
            "--since" => {
                since = next("--since")?.parse().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--since takes a number.")
                })?
            }
            "--file" => files.push(next("--file")?),
            "--resource" => resources.push(next("--resource")?),
            "--kind" => kind = Some(next("--kind")?),
            "--value" => value = Some(next("--value")?),
            "--with" => with = Some(next("--with")?),
            "--outcome" => outcome = Some(next("--outcome")?),
            "--why" => why = next("--why")?,
            "--detail" => detail = next("--detail")?,
            "--json" => json = true,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown option {other}. Run coder comms --help."),
                ));
            }
        }
    }
    let dir = channel_dir
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--channel DIR is required."))?;
    let name = name.unwrap_or_else(|| "agent".into());
    let channel = super::Channel::at(&dir).map_err(io::Error::other)?;
    let fail = |message: String| io::Error::other(message);
    match action.as_str() {
        "post" => {
            let text = text
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "post needs --text."))?;
            let record = channel.post(&name, &to, &text).map_err(fail)?;
            println!("{}", serde_json::to_string(&record).unwrap_or_default());
        }
        "read" => {
            let (records, cursor) = channel.read_for(&name, since).map_err(fail)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "since": cursor,
                        "records": records,
                    }))
                    .unwrap_or_default()
                );
            } else {
                for record in &records {
                    println!("{}", record.line());
                }
            }
        }
        "intent" => {
            let overlaps = channel
                .intent(super::Intent::new(&name, files, resources))
                .map_err(fail)?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({"overlaps": overlaps}))
                    .unwrap_or_default()
            );
        }
        "lease" => {
            let kind = kind.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "lease needs --kind.")
            })?;
            let value = value.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "lease needs --value.")
            })?;
            let op = rest.get(1).map(String::as_str).unwrap_or("acquire");
            match op {
                "acquire" => {
                    let held = channel.acquire(&name, &[(kind, value)]).map_err(fail)?;
                    println!("{}", serde_json::to_string(&held).unwrap_or_default());
                }
                "renew" => {
                    let held = channel.renew(&name, &kind, &value).map_err(fail)?;
                    println!("{}", serde_json::to_string(&held).unwrap_or_default());
                }
                "release" => {
                    channel.release(&name, &kind, &value).map_err(fail)?;
                    println!("released {kind}:{value}");
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("lease takes acquire, renew or release, not {other}."),
                    ));
                }
            }
        }
        "agree" => {
            let outcome = outcome
                .as_deref()
                .and_then(super::Outcome::parse)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "agree needs --outcome first|split|merge.",
                    )
                })?;
            let other = with.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "agree needs --with NAME.")
            })?;
            let agreement = channel
                .agree(super::Agreement {
                    members: [name.clone(), other],
                    outcome,
                    detail,
                    decided_by: name,
                    why,
                    at: 0,
                })
                .map_err(fail)?;
            println!("{}", serde_json::to_string(&agreement).unwrap_or_default());
        }
        "status" => {
            let snapshot = channel.snapshot().map_err(fail)?;
            println!("{}", serde_json::to_string(&snapshot).unwrap_or_default());
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown subcommand {other}. Run coder comms --help."),
            ));
        }
    }
    Ok(())
}
