//! `openagents cloud up|down|status`: a pool of GCE spot hosts started from
//! the daily `oa-coder-host` image, granted to this computer as one
//! computer, `gce` (issue #10225; audit
//! `docs/cloud/2026-10-02-cloud-parallel-execution-audit.md` §4;
//! runbook `docs/cloud/gce-pool.md`).
//!
//! - **The grant.** `cloud up` writes the pool record
//!   (`~/.openagents/cloud/pool.json`): one computer `gce`, a grant id
//!   `gce:<pool>`, and a revocation epoch. `chat work --on gce` runs only
//!   under a live grant; `cloud down` deletes every host and revokes it.
//!   Nothing else places work on the pool, and the pool never stands in
//!   for another computer.
//! - **Hosts.** Spot `c3-standard-8` VMs (300 GB pd-balanced, no external
//!   address, service account `oa-coder-host@` that reaches only the
//!   sccache bucket) from image family `oa-coder-host`. Each zone of the
//!   region is tried in turn, then on demand, as the image bake does. Two
//!   Coder runs per host, each holding a slot lock.
//! - **Self-delete.** The host agent (the VM's startup script, [`HOST_AGENT`])
//!   checks every minute whether a run is alive and deletes its own VM after
//!   10 idle minutes, through a role granted on that one instance only. A
//!   12-hour maximum run duration is the backstop.
//! - **Reach.** `ssh` as user `coder` with a pool key of this computer
//!   (`~/.openagents/cloud/pool_ed25519`), through an IAP tunnel
//!   (`gcloud compute start-iap-tunnel`), or straight to the internal
//!   address with `OA_POOL_SSH=internal` from inside the VPC.
//!
//! - **The budget** (#12058, [`coder_cloud::budget`]). A spend cap over
//!   the last 24 hours and per run, a hard host limit, and the most idle
//!   minutes; `cloud budget` sets it, `cloud status` shows spend against it,
//!   and nothing starts a host or a run past it. An agent (`--agent ID` or
//!   `OPENAGENTS_AGENT_ID`) starts hosts only under its own grant.
//!
//! Every `gcloud` call honors `CLOUDSDK_CONFIG`; on the owner's Mac that is
//! the automation account's isolated configuration. The grant remembers
//! it, so a later call without it (an agent's tool call) runs as the same
//! account, never a person's login that expires.

use crate::out::{Output, table};
use crate::{Args, EXIT_FAILURE};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_cloud::budget::{Budget, Ledger};
pub(crate) use coder_cloud::pool::*;
use serde_json::json;
use std::time::Instant;
pub(crate) fn start_hosts(pool: &Pool, count: u64, output: Output) -> Result<Vec<Started>, String> {
    coder_cloud::pool::start_hosts(pool, count, output.json())
}
pub(crate) const USAGE: &str = "usage: openagents cloud COMMAND [--agent ID]
  up [--hosts N] [--machine TYPE] [--on-demand] [--max-hosts N] [--idle-minutes N]
                 Grant this computer the GCE pool `gce` and start hosts until
                 it has N (default 1): spot VMs from the newest oa-coder-host
                 image, each zone of us-central1 in turn, then on demand
                 (--on-demand skips spot). Waits until each host is ready and
                 has built origin/main's openagents and microcoder, and prints
                 how long that took. Each host runs two Coder runs at once and
                 deletes itself after --idle-minutes (default 10) without a
                 run. --max-hosts (default 8) caps how far `chat work --on
                 gce` may grow the pool.
  down           Delete every host of the pool, list what is left, and revoke
                 the grant: `chat work --on gce` refuses until the next `up`.
  status         The grant, each host (zone, spot or on demand, uptime, live
                 runs, idle minutes), the pool's hourly cost estimate, and its
                 spend over the last 24 hours against the budget.
  budget [--daily USD] [--per-run USD] [--max-hosts N] [--idle-minutes N]
         [--agent ID --daily USD | --agent ID --revoke]
                 Show or set the compute budget (defaults $300 a day, $10 a
                 run, 8 hosts, 10 idle minutes). Hosts and runs never start
                 past it. With --agent, grant that agent definition ID its own
                 daily cap: an agent's `cloud up` and `chat work --on gce`
                 are refused without one.
Run a task with `openagents coder delegate AGENT --on gce --task TEXT`.
Run issues with `openagents chat work --on gce --issues N --parallel M`.
Needs gcloud logged in to project openagentsgemini as a service account
(CLOUDSDK_CONFIG; a person's login expires), and ssh; OA_PROJECT and OA_ZONES
override the project and zones. docs/cloud/gce-pool.md.";

/// What each command above does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    // Starting hosts bills machine time.
    Declared::computer("up", Effect::Spends),
    Declared::computer("down", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("budget", Effect::LocalWrite),
];

// The commands.

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("cloud", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &["on-demand", "revoke"]) {
        Ok(args) => args,
        Err(message) => return output.usage("cloud", &message, USAGE),
    };
    let allowed: &[&str] = match command.as_str() {
        "up" => &["hosts", "machine", "max-hosts", "idle-minutes", "agent"],
        "down" | "status" => &["agent"],
        "budget" => &[
            "daily",
            "per-run",
            "max-hosts",
            "idle-minutes",
            "agent",
            "revoke",
        ],
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        other => {
            return output.usage("cloud", &format!("unknown command `{other}`"), USAGE);
        }
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !allowed.contains(name))
    {
        return output.usage("cloud", &format!("unknown option `--{name}`"), USAGE);
    }
    if command != "budget" {
        coder_cloud::budget::act_for(args.option("agent"));
    }
    let result = match command.as_str() {
        "up" => up(*output, &args),
        "down" => down(*output),
        "budget" => budget(*output, &args),
        _ => status(*output),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail("cloud", &message),
    }
}

fn up(output: Output, args: &Args) -> Result<u8, String> {
    let limits = Budget::load();
    let hosts: u64 = args.number("hosts", 1)?;
    let max_hosts: u64 = args.number(
        "max-hosts",
        DEFAULT_MAX_HOSTS.max(hosts).min(limits.max_hosts),
    )?;
    let idle: u64 = args.number(
        "idle-minutes",
        DEFAULT_IDLE_MINUTES.min(limits.idle_minutes),
    )?;
    if !(1..=32).contains(&hosts) || hosts > max_hosts {
        return Err("--hosts is 1 to 32 and at most --max-hosts".into());
    }
    if max_hosts > limits.max_hosts {
        return Err(format!(
            "--max-hosts {max_hosts} is past the budget's {}; `openagents cloud budget --max-hosts \
             N` raises it",
            limits.max_hosts
        ));
    }
    if !(1..=limits.idle_minutes.min(240)).contains(&idle) {
        return Err(format!(
            "--idle-minutes is 1 to {} (the budget's idle limit)",
            limits.idle_minutes.min(240)
        ));
    }
    let machine = args.option("machine").unwrap_or(DEFAULT_MACHINE);
    let previous = Pool::load();
    let mut pool = Pool::grant(
        previous.as_ref(),
        machine,
        !args.switch("on-demand"),
        max_hosts,
        idle,
    );
    // Which account the pool runs as; a person's login expires daily, so
    // say so rather than fail at the next reauthentication.
    pool.account = coder_cloud::pool::active_account().or(pool.account);
    if let Some(account) = pool
        .account
        .as_deref()
        .filter(|a| coder_cloud::pool::is_person(a))
    {
        eprintln!(
            "cloud: gcloud is signed in as {account}, a person's login that expires; set \
             CLOUDSDK_CONFIG to a service account's configuration (docs/cloud/gce-pool.md, \
             \"Accounts\")"
        );
    }
    pool.save()?;
    let live: Vec<Host> = list_hosts(&pool.project, Some(&pool.pool))?
        .into_iter()
        .filter(|h| h.status == "RUNNING")
        .collect();
    let missing = hosts.saturating_sub(live.len() as u64);
    if !output.json() {
        eprintln!(
            "cloud: pool {} granted to this computer as `{COMPUTER}` (grant {}, epoch {}); {} \
             host(s) running, starting {missing}",
            pool.pool,
            pool.grant,
            pool.epoch,
            live.len()
        );
    }
    let began = Instant::now();
    let started = if missing > 0 {
        start_hosts(&pool, missing, output)?
    } else {
        Vec::new()
    };
    let failed: Vec<&Started> = started.iter().filter(|s| s.error.is_some()).collect();
    let all = list_hosts(&pool.project, Some(&pool.pool))?;
    let value = json!({
        "event": "cloud_up",
        "pool": pool,
        "hosts": all,
        "started": started,
        "seconds": began.elapsed().as_secs_f64(),
        "slots": all.iter().filter(|h| h.status == "RUNNING").count() as u64 * pool.slots_per_host,
    });
    output.emit(&value, |_| {
        let mut lines = Vec::new();
        for s in &started {
            let name = s.host.as_ref().map_or("(none)", |h| h.name.as_str());
            match &s.error {
                Some(error) => lines.push(format!("{name}: failed: {error}")),
                None => lines.push(format!(
                    "{name}: created {:.0} s, ready {:.0} s, ssh {:.0} s, built {:.0} s",
                    s.create_seconds, s.ready_seconds, s.ssh_seconds, s.prepared_seconds
                )),
            }
        }
        lines.push(status_text(&pool, &all, &[]));
        lines.join("\n")
    });
    Ok(if failed.is_empty() { 0 } else { EXIT_FAILURE })
}

fn down(output: Output) -> Result<u8, String> {
    let Some(mut pool) = Pool::load() else {
        return Err("this computer has no GCE pool".into());
    };
    let hosts = list_hosts(&pool.project, Some(&pool.pool))?;
    let deleted = delete_hosts(&pool.project, &hosts);
    let left = list_hosts(&pool.project, Some(&pool.pool))?;
    if pool.revoked_at.is_none() {
        pool.revoked_at = Some(now());
    }
    pool.save()?;
    let value = json!({"event": "cloud_down", "pool": pool.pool, "deleted": hosts.iter()
        .map(|h| &h.name).collect::<Vec<_>>(), "left": left, "revoked": true,
        "error": deleted.as_ref().err()});
    output.emit(&value, |_| {
        let mut text = format!(
            "cloud: deleted {} host(s) of pool {}; grant {} revoked.",
            hosts.len(),
            pool.pool,
            pool.grant
        );
        if left.is_empty() {
            text.push_str(" Leak check: no pool host is left.");
        } else {
            text.push_str(&format!(
                " Still listed: {}",
                left.iter()
                    .map(|h| format!("{} ({})", h.name, h.status))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        text
    });
    Ok(if deleted.is_ok() && left.is_empty() {
        0
    } else {
        EXIT_FAILURE
    })
}

/// `cloud budget`: show the budget, or set it.
fn budget(output: Output, args: &Args) -> Result<u8, String> {
    let mut budget = Budget::load();
    let dollars = |name: &str| -> Result<Option<f64>, String> {
        args.option(name)
            .map(|text| {
                text.trim_start_matches('$')
                    .parse::<f64>()
                    .map_err(|_| format!("--{name} is a dollar amount, like 300"))
            })
            .transpose()
    };
    let changed = if let Some(agent) = args.option("agent") {
        let agent = agent.trim();
        if agent.is_empty() {
            return Err("--agent needs an agent definition ID".into());
        }
        if args.switch("revoke") {
            budget.agents.remove(agent);
        } else {
            let daily = dollars("daily")?.ok_or("--agent needs --daily USD, or --revoke")?;
            budget.agents.insert(
                agent.to_owned(),
                coder_cloud::budget::AgentGrant {
                    daily_usd: daily,
                    granted_at: now(),
                },
            );
        }
        true
    } else {
        let mut changed = false;
        if let Some(daily) = dollars("daily")? {
            budget.daily_usd = daily;
            changed = true;
        }
        if let Some(per_run) = dollars("per-run")? {
            budget.per_run_usd = per_run;
            changed = true;
        }
        if args.option("max-hosts").is_some() {
            budget.max_hosts = args.number("max-hosts", budget.max_hosts)?;
            changed = true;
        }
        if args.option("idle-minutes").is_some() {
            budget.idle_minutes = args.number("idle-minutes", budget.idle_minutes)?;
            changed = true;
        }
        changed
    };
    if changed {
        budget.check()?;
        budget.set_at = Some(now());
        budget.save()?;
    }
    let spent = Ledger::load().spent(now(), None);
    let value = json!({"event": "cloud_budget", "budget": budget, "spent_usd": spent,
        "changed": changed});
    output.emit(&value, |_| budget_text(&budget, spent));
    Ok(0)
}

fn budget_text(budget: &Budget, spent: f64) -> String {
    let mut text = format!(
        "Budget: about ${spent:.2} of ${:.2} spent over the last 24 hours (list price); at most \
         ${:.2} a run, {} host(s) at once, {} idle minute(s) before a host deletes itself.",
        budget.daily_usd, budget.per_run_usd, budget.max_hosts, budget.idle_minutes
    );
    if budget.agents.is_empty() {
        text.push_str(" No agent may start hosts.");
    } else {
        let ledger = Ledger::load();
        for (id, grant) in &budget.agents {
            text.push_str(&format!(
                "\nAgent {id}: about ${:.2} of ${:.2} over the last 24 hours.",
                ledger.spent(now(), Some(id)),
                grant.daily_usd
            ));
        }
    }
    text
}

fn status_text(pool: &Pool, hosts: &[Host], activity: &[Activity]) -> String {
    let grant = match pool.revoked_at {
        None => format!("granted (epoch {})", pool.epoch),
        Some(_) => "revoked".into(),
    };
    let mut rows = vec![vec![
        "HOST".to_owned(),
        "ZONE".to_owned(),
        "STATUS".to_owned(),
        "MODEL".to_owned(),
        "RUNS".to_owned(),
        "IDLE".to_owned(),
    ]];
    let mut hourly = 0.0;
    for host in hosts {
        if host.status == "RUNNING" {
            hourly += hourly_usd(&host.machine, host.spot);
        }
        let seen = activity.iter().find(|a| a.host == host.name);
        rows.push(vec![
            host.name.clone(),
            host.zone.clone(),
            host.status.clone(),
            if host.spot { "spot" } else { "on demand" }.to_owned(),
            seen.map_or_else(|| "-".into(), |a| a.runs.to_string()),
            seen.and_then(|a| a.idle_seconds)
                .map_or_else(|| "-".into(), |s| format!("{} min", s / 60)),
        ]);
    }
    let mut text = format!(
        "Pool {} (`{COMPUTER}`, grant {}): {grant}. {} host(s), {} slot(s), about ${hourly:.2} an \
         hour at list price; hosts delete themselves after {} idle minutes.",
        pool.pool,
        pool.grant,
        hosts.len(),
        hosts.iter().filter(|h| h.status == "RUNNING").count() as u64 * pool.slots_per_host,
        pool.idle_minutes
    );
    if let Some(account) = &pool.account {
        text.push_str(&format!(" gcloud account: {account}."));
    }
    text.push('\n');
    text.push_str(&budget_text(
        &Budget::load(),
        Ledger::load().spent(now(), None),
    ));
    if !hosts.is_empty() {
        text.push('\n');
        text.push_str(&table(&rows));
    }
    text
}

fn status(output: Output) -> Result<u8, String> {
    let Some(pool) = Pool::load() else {
        let value = json!({"event": "cloud_status", "pool": null});
        output.emit(&value, |_| {
            "No GCE pool on this computer; `openagents cloud up` grants one.".into()
        });
        return Ok(0);
    };
    let hosts = list_hosts(&pool.project, Some(&pool.pool))?;
    let activity = activity(&pool.project, &hosts);
    let value = json!({"event": "cloud_status", "pool": pool, "hosts": hosts,
        "activity": activity, "budget": Budget::load(),
        "spent_usd": Ledger::load().spent(now(), None)});
    output.emit(&value, |_| status_text(&pool, &hosts, &activity));
    Ok(0)
}
