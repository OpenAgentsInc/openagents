//! `coder comms`: the channel from a shell, for engines that run as their
//! own process (Codex, OpenCode, Cursor, Devin, Claude in print mode,
//! Microcoder). Their task's preamble gives the exact command
//! ([`super::Member::preamble`]).

use serde_json::{Value, json};

use super::{Channel, LeaseRequest, Outcome, lease};

/// `coder comms --help`.
pub const USAGE: &str = "usage: coder comms [--channel DIR] [--as NAME] [--json] COMMAND
  read [--all]                        What others posted that concerns you (--all: everything).
  post [--to NAME] TEXT               Post to everyone, or to one member.
  intent [--file PATH]... [--crate NAME]... [--resource NAME]... [--note TEXT]
                                      Say what you will change; prints whom you overlap with.
  agree --with NAME --outcome first|split|merge [--first NAME] [--on PATH]... [--note TEXT]
                                      Record how an overlap is settled.
  lease take KIND[:NAME]... [--ttl SECONDS] [--wait]
                                      Take leases (build-folder, build-host, deploy-slot,
                                      migration, port), all or none, in a fixed order.
  lease renew RESOURCE [--ttl SECONDS]
  lease release RESOURCE
  lease list
  members                             Who is in the session and what each will change.
The channel and name default to $CODER_COMMS_CHANNEL and $CODER_COMMS_MEMBER.";

/// Runs `coder comms ARGS` and returns what to print.
///
/// # Errors
/// A sentence for the person or agent: bad arguments, a refused lease, or
/// a channel that can't be used.
pub fn run(args: &[String]) -> Result<String, String> {
    let mut args: Vec<String> = args.to_vec();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(USAGE.into());
    }
    let channel = take(&mut args, "--channel")?
        .or_else(|| {
            std::env::var(super::CHANNEL_ENV)
                .ok()
                .filter(|dir| !dir.is_empty())
        })
        .ok_or("Name the channel with --channel DIR.")?;
    let name = take(&mut args, "--as")?.or_else(|| std::env::var(super::MEMBER_ENV).ok());
    let json_out = flag(&mut args, "--json");
    let channel = Channel::open(channel)?;
    let speaker = || channel.member(name.as_deref().unwrap_or_default());
    let Some((command, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    let mut rest = rest.to_vec();
    let (text, value): (String, Value) = match command.as_str() {
        "read" => {
            let everything = flag(&mut rest, "--all");
            let (records, _) = channel.records(0);
            let me = name.clone().unwrap_or_default();
            let shown: Vec<&super::Record> = records
                .iter()
                .filter(|record| record.kind != super::Kind::Join)
                .filter(|record| {
                    everything || me.is_empty() || record.concerns(&me) || record.from == me
                })
                .collect();
            let lines: Vec<String> = shown
                .iter()
                .map(|record| super::view::plain(record))
                .collect();
            (
                if lines.is_empty() {
                    "Nothing yet.".into()
                } else {
                    lines.join("\n")
                },
                json!({"records": shown}),
            )
        }
        "post" => {
            let to = take(&mut rest, "--to")?;
            let text = rest.join(" ");
            let record = speaker()?.post(to.as_deref(), &text)?;
            (
                format!(
                    "Posted to {}.",
                    if record.to == super::ALL {
                        "everyone"
                    } else {
                        &record.to
                    }
                ),
                json!(record),
            )
        }
        "intent" => {
            let files = take_all(&mut rest, "--file")?;
            let crates = take_all(&mut rest, "--crate")?;
            let resources = take_all(&mut rest, "--resource")?;
            let note = take(&mut rest, "--note")?.unwrap_or_default();
            // Bare words are files too.
            let files: Vec<String> = files.into_iter().chain(rest.drain(..)).collect();
            let overlaps = speaker()?.intent(&files, &crates, &resources, &note)?;
            let text = if overlaps.is_empty() {
                "Posted. No one else is changing those.".into()
            } else {
                let mut lines = vec!["Posted. You overlap:".to_owned()];
                lines.extend(
                    overlaps
                        .iter()
                        .map(|overlap| format!("- {}", overlap.sentence())),
                );
                lines.join("\n")
            };
            (text, json!({"overlaps": overlaps}))
        }
        "agree" => {
            let with =
                take(&mut rest, "--with")?.ok_or("Name the other member with --with NAME.")?;
            let outcome = Outcome::parse(
                &take(&mut rest, "--outcome")?.ok_or("Give --outcome first, split or merge.")?,
            )?;
            let first = take(&mut rest, "--first")?;
            let on = take_all(&mut rest, "--on")?;
            let note = take(&mut rest, "--note")?.unwrap_or_default();
            let agreement = speaker()?.agree(&with, outcome, first.as_deref(), &on, &note)?;
            (
                format!("Recorded: {}", agreement.sentence()),
                json!(agreement),
            )
        }
        "lease" => lease_command(&channel, speaker, &mut rest)?,
        "members" => {
            let state = channel.read_state()?;
            let lines: Vec<String> = state
                .members
                .iter()
                .map(|(member, info)| {
                    let mut line = format!("{member} ({})", info.kind);
                    if let Some(how) = &info.finished {
                        line.push_str(&format!(", finished: {how}"));
                    }
                    if let Some(intent) = state.intents.get(member) {
                        line.push_str(&format!(". {}", intent.sentence()));
                    }
                    line
                })
                .collect();
            (lines.join("\n"), json!(state.members))
        }
        other => return Err(format!("Unknown command {other}.\n{USAGE}")),
    };
    Ok(if json_out { value.to_string() } else { text })
}

fn lease_command(
    channel: &Channel,
    speaker: impl Fn() -> Result<super::Member, String>,
    rest: &mut Vec<String>,
) -> Result<(String, Value), String> {
    let ttl = take(rest, "--ttl")?
        .map(|seconds| {
            seconds
                .parse::<u64>()
                .map(|seconds| seconds * 1000)
                .map_err(|_| "--ttl takes seconds.".to_owned())
        })
        .transpose()?
        .unwrap_or(lease::DEFAULT_TTL_MS);
    let wait = flag(rest, "--wait");
    let Some((op, words)) = rest.split_first() else {
        return Err("lease takes take, renew, release or list.".into());
    };
    match op.as_str() {
        "take" => {
            // `migration 12` and `port 41010` name one; `deploy-slot:qa` too.
            let mut requests = Vec::new();
            let mut words = words.iter().peekable();
            while let Some(word) = words.next() {
                let mut request = LeaseRequest::parse(word)?;
                if request.name.is_none() {
                    if let Some(next) = words.next_if(|next| lease::LeaseKind::parse(next).is_err())
                    {
                        request.name = Some(next.clone());
                    }
                }
                requests.push(request);
            }
            let member = speaker()?;
            let taken = if wait {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .map_err(|error| error.to_string())?;
                runtime.block_on(member.acquire_waiting(
                    &requests,
                    ttl,
                    &std::sync::atomic::AtomicBool::new(false),
                ))
            } else {
                member.acquire(&requests, ttl)
            }
            .map_err(|refusal| refusal.to_string())?;
            let lines: Vec<String> = taken
                .iter()
                .map(|lease| {
                    format!(
                        "Holding {} for {} seconds. Renew it while you work; release it when done.",
                        lease.resource,
                        ttl / 1000
                    )
                })
                .collect();
            Ok((lines.join("\n"), json!(taken)))
        }
        "renew" => {
            let resource = words.first().ok_or("Name the lease to renew.")?;
            let lease = speaker()?.renew(resource, ttl)?;
            Ok((
                format!(
                    "Holding {} for {} more seconds.",
                    lease.resource,
                    ttl / 1000
                ),
                json!(lease),
            ))
        }
        "release" => {
            let resource = words.first().ok_or("Name the lease to release.")?;
            let released = speaker()?.release(resource)?;
            Ok((
                if released {
                    format!("Released {resource}.")
                } else {
                    format!("You held no lease on {resource}.")
                },
                json!({"released": released}),
            ))
        }
        "list" => {
            let state = channel.read_state()?;
            let now = super::now_ms();
            let held: Vec<&super::Lease> = state
                .leases
                .values()
                .filter(|lease| lease.expires_ms > now)
                .collect();
            let lines: Vec<String> = held
                .iter()
                .map(|lease| {
                    format!(
                        "{} held by {} for {} more seconds",
                        lease.resource,
                        lease.holder,
                        (lease.expires_ms - now) / 1000
                    )
                })
                .collect();
            Ok((
                if lines.is_empty() {
                    "No leases are held.".into()
                } else {
                    lines.join("\n")
                },
                json!(held),
            ))
        }
        other => Err(format!("Unknown lease command {other}.")),
    }
}

/// Removes `--name VALUE` and returns the value.
fn take(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(at) = args.iter().position(|arg| arg == name) else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        return Err(format!("{name} needs a value."));
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

/// Removes every `--name VALUE`.
fn take_all(args: &mut Vec<String>, name: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    while let Some(value) = take(args, name)? {
        values.push(value);
    }
    Ok(values)
}

/// Removes `name` and returns whether it was there.
fn flag(args: &mut Vec<String>, name: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg != name);
    args.len() != before
}
