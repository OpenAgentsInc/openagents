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
//! - **Hosts.** Spot `c3-standard-8` VMs (200 GB pd-balanced, no external
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
//! Every `gcloud` call honors `CLOUDSDK_CONFIG`; on the owner's Mac that is
//! the automation account's isolated configuration.

use crate::out::{Output, table};
use crate::{Args, EXIT_FAILURE};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
pub(crate) use coder_cloud::pool::*;
use serde_json::json;
use std::time::Instant;
pub(crate) fn start_hosts(pool: &Pool, count: u64, output: Output) -> Result<Vec<Started>, String> {
    coder_cloud::pool::start_hosts(pool, count, output.json())
}
pub(crate) const USAGE: &str = "usage: openagents cloud COMMAND
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
                 runs, idle minutes), and the pool's hourly cost estimate.
Run a task with `openagents coder delegate AGENT --on gce --task TEXT`.
Run issues with `openagents chat work --on gce --issues N --parallel M`.
Needs gcloud signed in to project openagentsgemini (CLOUDSDK_CONFIG), and ssh;
OA_PROJECT and OA_ZONES override the project and zones. docs/cloud/gce-pool.md.";

/// What each command above does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    // Starting hosts bills machine time.
    Declared::computer("up", Effect::Spends),
    Declared::computer("down", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
];

// The commands.

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("cloud", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &["on-demand"]) {
        Ok(args) => args,
        Err(message) => return output.usage("cloud", &message, USAGE),
    };
    let allowed: &[&str] = match command.as_str() {
        "up" => &["hosts", "machine", "max-hosts", "idle-minutes"],
        "down" | "status" => &[],
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
    let result = match command.as_str() {
        "up" => up(*output, &args),
        "down" => down(*output),
        _ => status(*output),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail("cloud", &message),
    }
}

fn up(output: Output, args: &Args) -> Result<u8, String> {
    let hosts: u64 = args.number("hosts", 1)?;
    let max_hosts: u64 = args.number("max-hosts", DEFAULT_MAX_HOSTS.max(hosts))?;
    let idle: u64 = args.number("idle-minutes", DEFAULT_IDLE_MINUTES)?;
    if !(1..=32).contains(&hosts) || hosts > max_hosts {
        return Err("--hosts is 1 to 32 and at most --max-hosts".into());
    }
    if !(1..=240).contains(&idle) {
        return Err("--idle-minutes is 1 to 240".into());
    }
    let machine = args.option("machine").unwrap_or(DEFAULT_MACHINE);
    let previous = Pool::load();
    let pool = Pool::grant(
        previous.as_ref(),
        machine,
        !args.switch("on-demand"),
        max_hosts,
        idle,
    );
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
        "activity": activity});
    output.emit(&value, |_| status_text(&pool, &hosts, &activity));
    Ok(0)
}
