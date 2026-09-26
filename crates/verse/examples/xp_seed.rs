//! Seeds a local relay with a throwaway NIP-XP completion so the quest
//! board and XP have something to show:
//!
//! ```sh
//! cargo run -p verse --example xp_seed -- ws://127.0.0.1:7490
//! cargo run -p verse --example xp_seed -- ws://127.0.0.1:7490 --all
//! ```
//!
//! By default it publishes a knowledge entry and a runner's passing
//! evidence for it, signed with throwaway keys, and prints the evidence ID
//! for `microcoder xp award`. `--all` also publishes the quest, the award,
//! and a `beat-reference` achievement label, signed with a throwaway
//! referee key, and prints that key's npub for `xp-trust.json`.
//!
//! It refuses any relay that isn't on this machine: these keys are public,
//! and fixture events don't belong on a shared relay.

use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use knowledge::remote::npub;
use nostr::domain::Event;
use verse::net::{In, Link, Out};
use verse::xp::fixture::Completion;

fn publish(link: &Link, events: &[&Event]) -> Result<(), String> {
    let start = Instant::now();
    let mut connected = false;
    while !connected {
        if start.elapsed() > Duration::from_secs(5) {
            return Err(format!("{} didn't answer", link.url));
        }
        connected = link.drain().iter().any(|m| matches!(m, In::Connected));
        std::thread::sleep(Duration::from_millis(20));
    }
    for event in events {
        link.send(Out::Publish((*event).clone()));
    }
    let mut waiting: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
    while !waiting.is_empty() {
        if start.elapsed() > Duration::from_secs(10) {
            return Err(format!("no answer for {} events", waiting.len()));
        }
        for message in link.drain() {
            if let In::Ok {
                id,
                accepted,
                message,
            } = message
            {
                if !accepted {
                    return Err(format!("the relay refused {id}: {message}"));
                }
                waiting.retain(|w| *w != id);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(relay) = args.first() else {
        eprintln!("usage: xp_seed <ws://127.0.0.1:PORT> [--all]");
        return ExitCode::FAILURE;
    };
    let local = ["ws://127.0.0.1:", "ws://localhost:", "ws://[::1]:"];
    if !local.iter().any(|p| relay.starts_with(p)) {
        eprintln!("xp_seed: {relay} isn't a relay on this machine; fixtures stay local");
        return ExitCode::FAILURE;
    }
    let all = args.iter().any(|a| a == "--all");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let c = Completion::new(9_001, 9_002, 9_003, now - 10);
    let mut events = vec![&c.entry, &c.evidence];
    let award = c.award(now);
    let label = Completion::label(&c.referee, &award, "beat-reference", now);
    if all {
        events.extend([&c.quest, &award, &label]);
    }
    let link = Link::start(relay);
    if let Err(e) = publish(&link, &events) {
        eprintln!("xp_seed: {e}");
        return ExitCode::FAILURE;
    }
    println!("entry    {}", c.entry.id);
    println!("evidence {}", c.evidence.id);
    println!("author   {}", npub(c.author.pubkey()));
    println!("runner   {}", npub(c.runner.pubkey()));
    if all {
        println!("referee  {}", npub(c.referee.pubkey()));
        println!("award    {}", award.id);
    }
    ExitCode::SUCCESS
}
