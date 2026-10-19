//! `openagents pylon` and the standalone `pylon` binary share this
//! dispatcher. Every command prints one JSON document with `--json`.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::{PoolPolicy, Tier};
use serde_json::{Value, json};

use crate::client::{self, Ask, Pay};
use crate::engine::Psionic;
use crate::field::RelayField;
use crate::identity::{
    Identity, check_owner, hex_pubkey, load_owner, mint_owner, npub, parse_owner, save_owner,
};
use crate::lease::{Dedicated, Leases, Machine, Paused};
use crate::paid::{Grant, Granted, Network, Price, Wallet};
use crate::pool;
use crate::provider::{Config, Provider};
use crate::{DEFAULT_RELAY, home};

/// Usage text.
pub const USAGE: &str = "\
Usage: openagents pylon <command> [options]

Share this computer's model as a NIP-PYLON pylon, or use one. Every job is
NIP-44 encrypted to the pylon. Jobs are free unless the pylon names a price;
a priced job is bought first under NIP-X402 from this computer's wallet.

Commands:
  up                        Make this computer a pylon and keep it one: read it
                            (doctor), download the decision model if it is
                            missing, then run Psionic's decision engine (and a
                            text engine when memory allows), self-check it, and
                            serve. The host service keeps it running, restarts
                            a crashed engine, and brings it back after a
                            restart; without the host service it runs here.
                            Admits any key unless --allow names some (rate
                            limits apply). Takes serve's options.
      --yes                 Download the model without asking.
      --foreground          Run it in this terminal, not the host service.
      --text off            Serve decisions only.
      --text-model FILE     The text engine's GGUF (default
                            ~/.openagents/models/Qwen3.5-0.8B-Q8_0.gguf).
      --device NAME         metal, cuda, or cpu (default: what doctor picks).
      --port N              The decision engine's loopback port (default 18096).
      --dedicated           Ignore this computer's lease table.
      --allow-slow          Serve decisions on the CPU anyway (refused by
                            default: seconds a decision).
  down                      Stop serving: tell the relay this pylon is offline,
                            let jobs it took finish (at most 90 s), stop the
                            engines, and keep it off until `up`.
  status                    This pylon: serving, paused and why, or off; jobs
                            today, the model, its engines, the last self-check,
                            and checkers' verdicts about it.
      --field               Every verified pylon on the relay instead.
      --pool SLUG           With --field: only pylons asking to join this pool.
  id                        This pylon's key, name, owner link, relays, and
                            beacon address, with this computer's buyer,
                            aggregator, and checker keys (`whoami` too).
  doctor                    Read this computer (memory, GPU, disk, the pinned
                            decision model and its digest, the Psionic server,
                            outbound reach to the relay, the last self-check)
                            and name what blocks serving decisions, each with
                            the command that fixes it. Writes nothing.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
      --rehash              Re-hash the model instead of the remembered digest.
      --allow-slow          Count a computer that would decide on the CPU (no
                            Apple Silicon from 16 GB, no NVIDIA from 8 GB) as
                            ready; it is refused by default because each
                            decision takes seconds.
  plan                      The ordered steps that get this computer serving,
                            each marked agent (may run unattended) or owner
                            (needs the owner: identity, payout, a slow
                            machine), with download size, time, cost, and the
                            command. Writes nothing.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
  verify                    A pass/fail checklist for this running pylon: the
                            engine answers /health, its beacon is visible on
                            the relay from a fresh connection, the owner link
                            verifies, the trusted checkers' latest verdicts,
                            the clock is within 30 s of the relay's, and
                            nothing answers on the engine ports beyond
                            loopback.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
  logs                      Recent log lines (the pylon home's run/ folder,
                            the engine's log, the host service's pylon lines),
                            secrets redacted, each line at most 400 characters,
                            marked untrusted.
      --lines N             Lines per log (default 100, at most 2000).
  settings                  Show or change your own pause choices. Both are
                            off by default; when on, the pylon finishes the
                            jobs it took and takes no new ones while it holds.
      --pause-on-battery on|off
                            Pause while this computer runs on battery.
      --pause-when-hot on|off
                            Pause while this computer (or its NVIDIA GPU) is
                            hot.
  setup                     Download the pinned decision model (Clef-Flash
                            Q4_K_M, 6.5 GB, sha256-checked), install this
                            release's Psionic engine for this platform in
                            ~/.openagents/bin (size and sha256 checked), and
                            self-check a decision server that is already
                            running. Without a published engine for this
                            platform, the one you built on PATH is used.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
  update                    As setup, but re-hash the model and fetch it again
                            when it is not the file this build pins. Never
                            changes the served model id on its own.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
      --accept-model-change The owner accepts a new served model id; the
                            self-check runs again before it serves. Not
                            offered to agents or timers.
  start                     Start Psionic's decision lane on loopback, self-check
                            it, and serve decisions only. Takes serve's options
                            (--allow NPUB or --allow-any, --slots, --rate, ...).
                            Nothing listens beyond loopback; the pylon dials out.
      --port N              Psionic's loopback port (default 18096).
      --device NAME         metal, cuda, or cpu (default: what doctor picks).
      --yes                 Link this pylon to your OpenAgents key and send its
                            earnings to your OpenAgents wallet without asking,
                            when neither is set. On a terminal start asks;
                            without one and without --yes it changes nothing.
      --allow-slow          Serve decisions on the CPU anyway (refused by
                            default: each decision takes seconds).
  selfcheck                 Answer the shipped reference decisions on this
                            computer and compare them with the reference; serve
                            refuses decisions after a failed self-check.
      --decide URL          The local decision server (default
                            http://127.0.0.1:18096).
      --items N             How many reference items (default 64).
      --model-file PATH     The GGUF the server loaded, when it is not the
                            pinned download or ~/.openagents/models' copy.
  calibrate run             Answer seeded decision items and write them as JSON
                            lines, for comparing machines or models.
      --decide URL          The decision server (default http://127.0.0.1:18096).
      --decide-model NAME   The model to ask for (default clef-flash).
      --seed N              The item seed (default 20261014).
      --count N             How many items (default 200).
      --out FILE            Append the answers to FILE.
  calibrate compare REF CAND
                            Distances, top-answer agreement, and simulated round
                            pass and strike shares between two runs.
  calibrate reference RUN --measured-on TEXT --out FILE
                            Make a self-check reference set from a run.
  whoami                    Print this computer's pylon, buyer, and aggregator keys.
  spec                      Print the agent contract: the --json result object,
                            its statuses, the exit codes, and the rules.
  agent-rules               Print the safety rules for an agent running a pylon.
  net-policy                Print every destination a pylon may contact.
  version                   Print pylon/1, the CLI version, and this release's
                            pins (pylon-release.v1).
  serve                     Publish beacons and answer jobs from a local Psionic server.
      --engine URL          The Psionic server (default http://127.0.0.1:18080).
      --model NAME          The served model (default qwen3.5-0.8b-q8_0).
      --decide URL          Also answer NIP-DEC decision jobs with the System
                            One server at URL (Psionic's Clef lane), free;
                            the beacon advertises pylon/decision.
      --decide-model NAME   The decision model at that server (default: the
                            first one it lists).
      --decisions-only      Answer decision jobs only, no text jobs.
      --attested            Run as a NIP-ATT attested endpoint inside a Google
                            Confidential Space workload (the release image
                            only): a key made in memory, the pinned weights
                            fetched and checked, Psionic started on loopback,
                            a launcher token bound to the key, sealed
                            decisions only. Takes --release ID --publisher
                            HEX --weights-url URL --weights-sha256 HEX
                            --weights PATH --psionic BIN [--operator HEX]
                            [--workload SLUG] [--teeserver SOCKET]
                            [--decision-device cpu|cuda] [--decision-chunk N]
                            [--require-gpu-cc] [--idle-stop SECS]
                            (OA_ATT_RELEASE and OA_ATT_PUBLISHER also work).
                            --require-gpu-cc refuses to start (and stops on
                            a refresh) unless Google's token says the GPU is
                            in confidential-computing mode (cc_mode ON, every
                            GPU an H100). --idle-stop stops this VM
                            (Compute instances.stop on itself) once no
                            decision has come for SECS.
      --pylon SLUG          The beacon's name (default: the host name).
      --label TEXT          Display label (default: the slug).
      --slots N             Concurrent jobs (default 2).
      --allow NPUB          A key that may send jobs; repeat or comma-separate.
      --allow-any           Admit any key (rate limits still apply).
      --rate N              Jobs per buyer per minute (default 10).
      --max-tokens N        Output bound per job (default 512).
      --vram-gb N           GPU memory for the beacon's class (default 16).
      --pool SLUG           Pool to ask to join (default everglade).
      --dedicated           Ignore this computer's lease table (a box that
                            runs no owner work); otherwise each job takes a
                            background `pylon` lease and the pylon drains
                            while the owner's work needs the computer.
      --price-msat N        Sell each job for N msat under NIP-X402, with
                            invoices from this computer's Lightning node.
      --network NAME        The price's network: testnet (default) or
                            bitcoin, which needs the owner's grant.json.
  link                      Show the owner's NIP-OA link on this pylon's beacons.
      --owner-secret FILE   Mint the link with the owner key in FILE (hex or
                            nsec); the key is read once, never stored.
      --credential JSON     Store a link minted elsewhere: the auth
                            tag [\"auth\", owner, \"kind=30200\", signature].
      --remove              Remove the link.
  earnings                  What this pylon has earned so far (earned, held for
                            checking, paid, forfeited), where it is paid, and
                            the wallet balance when that is this computer's
                            OpenAgents wallet. Read from the OpenAgents API as
                            the earning key: the linked owner, else the pylon.
      --owner-secret FILE   The linked owner's key, when it is not this
                            computer's OpenAgents key (read once, never stored).
      --api URL             The OpenAgents API (default https://api.openagents.com,
                            or OPENAGENTS_API_BASE).
  payout set [ADDRESS]      Where earnings go: a Lightning address or a Spark
                            address. With no ADDRESS, this computer's OpenAgents
                            wallet (`openagents wallet`). Bitcoin rails only.
                            Takes --owner-secret FILE and --api URL as earnings.
  route on|off|status       Send Alice's and the crew's low-risk text jobs (day
                            plans) to the pool, falling back to their own model.
      --pylon NPUB          Only this pylon (default: the best fresh one).
      --wait SECS           How long a job waits (default 90).
  ask PROMPT                Find a pylon, run one job, and publish a receipt.
                            Skips pylons a trusted checker failed.
      --pylon NPUB          Use this pylon instead of the best fresh one.
      --wait SECS           How long to wait for the answer (default 90).
      --no-receipt          Do not publish a receipt.
      --max-msat N          Pay a priced pylon up to N msat for the job,
                            from this computer's wallet (see `openagents
                            x402`); its policy's ceilings also apply. On
                            bitcoin it pays from your wallet (`openagents
                            wallet`, labelled \"Pylon job\" in its history),
                            on testnet from this computer's Lightning node.
      --pay-with wallet|node
                            Choose who pays (`openagents pylon` only).
      --network NAME        The wallet's network: testnet (default) or
                            bitcoin, which needs the owner's grant.json.
  check canary --pylon NPUB Send the pylon its class's pinned Gym suite of
                            known-answer jobs as the buyer key, and sign a
                            check verdict on each receipt with the checker key.
      --award               Also award NIP-XP (pylon-check) for a passing canary.
  check redundant PROMPT --pylon NPUB --pylon NPUB [--pylon NPUB]
                            Send one prompt to several pylons and sign verdicts
                            from the majority answer.
  check decide --pylon NPUB Send the pylon fresh decision items (a new seed each
                            round) as ordinary decision jobs, recompute the items
                            drawn after the answers arrive on a reference server
                            running the same pinned model, and sign a verdict:
                            pass, fail (a strike), or inconclusive (a small miss).
                            Items come from this checker's private seed
                            (checker-seed, made on first use) and each round
                            from a new buyer key, so a pylon can neither
                            rebuild the items nor recognise the checker.
      --reference URL       The reference decision server (default
                            http://127.0.0.1:18096).
      --rounds N            Rounds to run (default 1).
      --seed N              Public items from seed N (counting up per round),
                            for reproducible calibration runs.
      --own-buyer           Send from the stored buyer key (for a pylon that
                            admits only allowlisted keys).
  check decide --watch      Keep checking: read the relay's beacons and check
                            every online pylon serving the reference's model
                            when it first appears, then every --every seconds;
                            a struck pylon again after --recheck-struck. Leaves
                            offline and draining pylons alone. Runs until
                            stopped.
      --every SECS          Seconds between rounds per pylon (default 900).
      --recheck-struck SECS Seconds before a struck pylon is checked again
                            (default 3600).
      --concurrency N       Rounds at once (default 4).
      --poll SECS           Seconds between reads of the beacons (default 30).
      --pool SLUG           Only pylons asking to join this pool.
      --pylon NPUB          Only this pylon; repeat.
  canary make               Make a checker-private pool of hidden decision canaries:
                            items from the generator under a seed from a secret
                            file, each with the reference server's probabilities,
                            written as JSON lines (owner-only) for the gateway's
                            decisions.canaries.
      --seed-file FILE      The secret seed file (keep it private).
      --reference URL       The reference decision server running the pinned
                            model (default http://127.0.0.1:18096).
      --count N             How many canaries (default 200).
      --out FILE            Where to write the pool.
  league                    The pylon league: per class, pass rates on the
                            pinned suites, jobs, median time, and cost.
  pool                      Compute a pool aggregate over the last window.
      --pool SLUG           The pool (default everglade).
      --minutes N           Window length, 1 to 60 (default 60).
      --publish             Sign and publish it as the aggregator.
      --checked             Count the trusted checkers' verdicts and drop
                            pylons they failed from admission.
  pool verify --aggregator NPUB [--checked]
                            Fetch that aggregator's newest aggregate and recompute it.

Common options:
  --relay URL               The relay (default wss://relay.openagents.com).
  --checker NPUB            Trust this checker's verdicts; repeat. This
                            computer's own checker key and
                            OPENAGENTS_PYLON_CHECKERS are always trusted.
  --json                    One result object on standard output (see `spec`);
                            serve and start print it when they start serving,
                            then one JSON event per line.

Exit codes: 0 ok, 1 refused or failed, 10 blocked, 11 needs the owner,
13 network or download failure, 20 verification failed, 64 invalid usage.

Keys live in ~/.openagents/compute (OPENAGENTS_PYLON_HOME overrides). On
bitcoin, nothing sells or pays without the owner's standing grant in
grant.json there: {\"per_payment_msat\": N, \"daily_msat\": N}.";

struct Args {
    words: Vec<String>,
}

impl Args {
    fn flag(&mut self, name: &str) -> bool {
        if let Some(i) = self.words.iter().position(|w| w == name) {
            self.words.remove(i);
            true
        } else {
            false
        }
    }

    fn values(&mut self, name: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        while let Some(i) = self.words.iter().position(|w| w == name) {
            if i + 1 >= self.words.len() {
                return Err(format!("{name} needs a value"));
            }
            let value = self.words.remove(i + 1);
            self.words.remove(i);
            out.push(value);
        }
        Ok(out)
    }

    fn value(&mut self, name: &str) -> Result<Option<String>, String> {
        Ok(self.values(name)?.pop())
    }

    fn number(&mut self, name: &str, default: u64) -> Result<u64, String> {
        self.value(name)?.map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("{name} takes a number"))
        })
    }
}

fn emit(json_out: bool, value: &Value, text: &str) {
    if crate::envelope::take(value) {
        return;
    }
    if json_out {
        println!("{value}");
    } else {
        println!("{text}");
    }
}

/// Run one command. Returns the process exit code.
#[must_use]
/// Run a command with no wallet: priced serving and paid asking refuse.
pub fn run(json_out: bool, words: &[String]) -> u8 {
    run_with(json_out, words, &crate::paid::NoWallet)
}

/// Run a command with `wallet` behind priced serving and paid asking.
/// Every command runs under the agent contract (`crate::envelope`): one
/// result object with `--json`, and the pylon group's exit codes.
pub fn run_with(json_out: bool, words: &[String], wallet: &dyn Wallet) -> u8 {
    let command = words.first().map_or("", String::as_str);
    crate::envelope::run(command, json_out, || dispatch(json_out, words, wallet))
}

fn dispatch(
    json_out: bool,
    words: &[String],
    wallet: &dyn Wallet,
) -> Result<(), crate::envelope::Outcome> {
    use crate::envelope::Outcome;
    let Some((command, rest)) = words.split_first() else {
        emit(json_out, &json!({"usage": USAGE}), USAGE);
        return Err(Outcome::Usage("name a command".into()));
    };
    if command == "--help" || command == "help" {
        emit(json_out, &json!({"usage": USAGE}), USAGE);
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let mut args = Args {
        words: rest.to_vec(),
    };
    let result = runtime.block_on(async {
        let relay = args
            .value("--relay")?
            .unwrap_or_else(|| DEFAULT_RELAY.into());
        let mut checkers = crate::check::trusted(&home());
        for value in args.values("--checker")? {
            checkers.insert(hex_pubkey(&value).ok_or_else(|| format!("`{value}` is not a key"))?);
        }
        let plain: Result<(), String> = match command.as_str() {
            "id" | "whoami" => crate::online::id(json_out, &args.words),
            "up" => crate::online::up(json_out, &args.words, &relay).await,
            "down" => crate::online::down(json_out, &args.words).await,
            "link" => link(json_out, &mut args),
            "route" => crate::route::command(&args.words, &home()).map(|(value, text)| {
                emit(json_out, &value, &text);
            }),
            "serve" => serve(json_out, &mut args, &relay, wallet).await,
            "ask" => ask(json_out, &mut args, &relay, checkers, wallet).await,
            "status" => {
                if args.flag("--field") || args.words.iter().any(|w| w == "--pool") {
                    status(json_out, &mut args, &relay).await
                } else {
                    crate::online::status(json_out, &args.words, &relay).await
                }
            }
            "pool" => pool(json_out, &mut args, &relay, checkers).await,
            "check" => check(json_out, &mut args, &relay).await,
            "league" => league(json_out, &relay, checkers).await,
            "calibrate" => calibrate(json_out, &mut args).await,
            "canary" => canary(json_out, &mut args).await,
            "selfcheck" => return selfcheck(json_out, &mut args).await,
            "doctor" => return doctor(json_out, &mut args, &relay).await,
            "setup" => setup(json_out, &mut args, false).await,
            "update" => setup(json_out, &mut args, true).await,
            "start" => start(json_out, &mut args, &relay, wallet).await,
            "earnings" => earnings(json_out, &mut args, wallet).await,
            "payout" => payout(json_out, &mut args, wallet).await,
            "plan" => plan(json_out, &mut args, &relay).await,
            "verify" => verify(json_out, &mut args, &relay, &checkers).await,
            "logs" => logs(json_out, &mut args),
            "settings" => settings(json_out, &mut args),
            "spec" | "agent-rules" | "net-policy" | "version" => {
                meta(json_out, command, &args, &relay)
            }
            other => {
                return Err(Outcome::Usage(format!(
                    "unknown command `{other}`; see `openagents pylon --help`"
                )));
            }
        };
        plain.map_err(Outcome::from)
    });
    result
}

/// `spec`, `agent-rules`, `net-policy`, `version`: the agent contract,
/// its rules, the destinations, and this release's pins. Write nothing.
fn meta(json_out: bool, command: &str, args: &Args, relay: &str) -> Result<(), String> {
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let (value, text) = match command {
        "spec" => crate::envelope::spec(),
        "agent-rules" => crate::envelope::agent_rules(),
        "net-policy" => crate::envelope::net_policy(relay),
        _ => crate::release::version(),
    };
    emit(json_out, &value, &text);
    Ok(())
}

fn key(name: &str) -> Result<Identity, String> {
    Identity::load_or_create(&home().join(format!("{name}.key")))
}

/// This computer's name as a beacon slug.
#[must_use]
pub fn host_slug() -> String {
    // macOS exports no HOSTNAME and has no /etc/hostname, so ask
    // `hostname -s` before falling back to "pylon" (every Mac was "pylon").
    let raw = std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .filter(|h| !h.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .arg("-s")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })
        .unwrap_or_else(|| "pylon".into());
    let slug: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    if slug.is_empty() {
        "pylon".into()
    } else {
        slug
    }
}

/// `--network`: testnet unless named.
fn network(args: &mut Args) -> Result<Network, String> {
    match args.value("--network")?.as_deref() {
        None | Some("testnet") => Ok(Network::Testnet),
        Some("bitcoin") => Ok(Network::Bitcoin),
        Some(other) => Err(format!(
            "--network takes testnet or bitcoin (x402 names no other), not `{other}`"
        )),
    }
}

/// The owner's grant, which `bitcoin` requires before any wallet opens.
fn grant_for(network: Network) -> Result<Option<Grant>, String> {
    let grant = Grant::load(&home())?;
    if network == Network::Bitcoin && grant.is_none() {
        return Err(format!(
            "bitcoin needs the owner's standing grant in {}; nothing was opened",
            home().join(Grant::FILE).display()
        ));
    }
    Ok(grant)
}

async fn serve(
    json_out: bool,
    args: &mut Args,
    relay: &str,
    wallet: &dyn Wallet,
) -> Result<(), String> {
    let engine_url = args
        .value("--engine")?
        .unwrap_or_else(|| "http://127.0.0.1:18080".into());
    let model = args
        .value("--model")?
        .unwrap_or_else(|| "qwen3.5-0.8b-q8_0".into());
    if args.flag("--attested") {
        return serve_attested(json_out, args, relay).await;
    }
    let decide_url = args.value("--decide")?;
    let decide_model = args.value("--decide-model")?;
    let decisions_only = args.flag("--decisions-only");
    if decisions_only && decide_url.is_none() {
        return Err("--decisions-only needs --decide URL".into());
    }
    let slug = args.value("--pylon")?.unwrap_or_else(host_slug);
    let mut config = Config::new(relay, &slug, home());
    config.label = args.value("--label")?.unwrap_or_else(|| slug.clone());
    config.slots = u32::try_from(args.number("--slots", 2)?.clamp(1, 64)).unwrap_or(2);
    config.rate_per_minute = u32::try_from(args.number("--rate", 10)?.clamp(1, 600)).unwrap_or(10);
    if let Some(total) = args.value("--total-rate")? {
        let total: u32 = total
            .parse()
            .map_err(|_| "--total-rate takes jobs per minute from all callers")?;
        config.total_per_minute = Some(total.clamp(1, 6_000));
    }
    config.max_tokens =
        u32::try_from(args.number("--max-tokens", 512)?.clamp(1, 4_096)).unwrap_or(512);
    let vram = u32::try_from(args.number("--vram-gb", 16)?).unwrap_or(16);
    config.class.tier = Tier::for_gpu(vram);
    config.class.memory_gb = [512, 256, 128, 64, 32, 16, 8]
        .into_iter()
        .find(|&gb| gb <= vram)
        .unwrap_or(8);
    if let Some(pool) = args.value("--pool")? {
        config.pools = vec![pool];
    }
    if args.flag("--allow-any") {
        config.allow = None;
    } else {
        let mut allow = BTreeSet::new();
        for value in args.values("--allow")? {
            for item in value.split(',').filter(|s| !s.is_empty()) {
                allow.insert(
                    hex_pubkey(item.trim()).ok_or_else(|| format!("`{item}` is not a key"))?,
                );
            }
        }
        if allow.is_empty() {
            return Err("name who may send jobs with --allow NPUB, or pass --allow-any".into());
        }
        config.allow = Some(allow);
    }
    let price_msat = args.number("--price-msat", 0)?;
    let net = network(args)?;
    let inner: Arc<dyn Machine> = if args.flag("--dedicated") {
        Arc::new(Dedicated)
    } else {
        Arc::new(Leases::from_env()?)
    };
    // The user's own pause choices (on battery, hot) drain it too.
    let machine: Arc<dyn Machine> = Arc::new(Paused::new(inner, &home()));
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    config.owner = load_owner(&home())?;
    let engine = Arc::new(Psionic::new(&engine_url, &model)?);
    let identity = key("provider")?;
    let provider = if let Some(url) = &decide_url {
        if price_msat > 0 {
            return Err("decisions are free work; drop --price-msat with --decide".into());
        }
        let clef = crate::decide::Clef::new(url, decide_model.as_deref())?;
        match clef.refresh().await {
            Ok(found) => {
                eprintln!("pylon: decisions from {} at {url}", found.advertised());
                if let Some(id) = found
                    .artifact_digest
                    .as_deref()
                    .and_then(|digest| crate::setup::model_id_for(digest, None))
                {
                    match crate::calibrate::may_serve(&home(), &id)? {
                        Some(check) => eprintln!(
                            "pylon: self-check passed at {} ({:.1}% of simulated rounds)",
                            check.at,
                            check.round_pass * 100.0
                        ),
                        None if crate::calibrate::shipped_reference(&id).is_some() => {
                            eprintln!(
                                "pylon: no self-check for this model yet; run `openagents pylon selfcheck --decide {url}` so checkers' rounds pass"
                            );
                        }
                        None => {}
                    }
                }
            }
            Err(why) => eprintln!("pylon: {why}; the beacon says draining until it answers"),
        }
        let text: Option<Arc<dyn crate::engine::Engine>> =
            if decisions_only { None } else { Some(engine) };
        Provider::deciding(
            config.clone(),
            identity.clone(),
            text,
            Arc::new(clef),
            machine,
        )?
    } else if price_msat > 0 {
        let grant = grant_for(net)?;
        config.price = Some(Price {
            msat: price_msat,
            network: net,
        });
        let receiver = wallet.receiver(net)?;
        Provider::priced(
            config.clone(),
            identity.clone(),
            engine,
            machine,
            receiver,
            grant,
        )?
    } else {
        Provider::on(config.clone(), identity.clone(), engine, machine)?
    };
    emit(
        json_out,
        &json!({
            "serving": true,
            "pylon": format!("30200:{}:{}", identity.pubkey(), slug),
            "npub": identity.npub(),
            "relay": relay,
            "engine": (!decisions_only).then_some(&engine_url),
            "model": (!decisions_only).then_some(&model),
            "decide": decide_url,
            "allow": config.allow.as_ref().map(|a| a.iter().map(|k| npub(k)).collect::<Vec<_>>()),
            "price_msat": config.price.map(|p| p.msat),
            "network": config.price.map(|p| p.network.as_str()),
        }),
        &format!(
            "pylon {} serving {} on {relay}\nnpub {}\nstop with Ctrl-C or SIGTERM; an offline beacon goes out on the way down",
            slug,
            match (&decide_url, decisions_only) {
                (Some(url), true) => format!("decisions from {url}"),
                (Some(url), false) => format!("{model} from {engine_url} and decisions from {url}"),
                (None, _) => format!("{model} from {engine_url}"),
            },
            identity.npub()
        ),
    );
    let stop = async {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                () = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    };
    let counters_of = Arc::clone(&provider);
    provider.run(stop).await?;
    let counters = counters_of.counters().await;
    eprintln!(
        "pylon: stopped after {} served, {} refused, {} failed",
        counters.served, counters.refused, counters.failed
    );
    Ok(())
}

/// `serve --attested`: the NIP-ATT endpoint inside the release image
/// (`crate::attested`). Free, sealed decisions only, any caller under the
/// rate limit and the slot count.
async fn serve_attested(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    use crate::attested::{self, Attestation, Setup};
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let release = args
        .value("--release")?
        .or_else(|| env("OA_ATT_RELEASE"))
        .ok_or("--attested needs --release ID (or OA_ATT_RELEASE)")?;
    let publisher = args
        .value("--publisher")?
        .or_else(|| env("OA_ATT_PUBLISHER"))
        .ok_or("--attested needs --publisher HEX (or OA_ATT_PUBLISHER)")?;
    let operator = args
        .value("--operator")?
        .unwrap_or_else(|| publisher.clone());
    let workload = args
        .value("--workload")?
        .unwrap_or_else(|| "clef-decisions".into());
    let socket = args
        .value("--teeserver")?
        .unwrap_or_else(|| attested::TEESERVER.into());
    let weights_url = args
        .value("--weights-url")?
        .ok_or("--attested needs --weights-url")?;
    let weights_sha256 = args
        .value("--weights-sha256")?
        .ok_or("--attested needs --weights-sha256")?;
    let weights = PathBuf::from(
        args.value("--weights")?
            .ok_or("--attested needs --weights PATH")?,
    );
    let psionic = args
        .value("--psionic")?
        .ok_or("--attested needs --psionic BIN")?;
    let port = args.number("--psionic-port", 18_096)?;
    let device = args
        .value("--decision-device")?
        .unwrap_or_else(|| "cpu".into());
    let gpu = match device.as_str() {
        "cpu" => false,
        "cuda" => true,
        other => {
            return Err(format!(
                "--decision-device takes cpu or cuda, not `{other}`"
            ));
        }
    };
    let chunk = args.value("--decision-chunk")?;
    if let Some(n) = &chunk {
        n.parse::<u32>()
            .map_err(|_| format!("--decision-chunk takes a number, not `{n}`"))?;
    }
    let require_gpu_cc = args.flag("--require-gpu-cc");
    let idle_stop = args.number("--idle-stop", 0)?;
    let slug = args
        .value("--pylon")?
        .unwrap_or_else(|| if gpu { "att-h100" } else { "att-tdx" }.into());
    let mut config = Config::new(relay, &slug, home());
    config.label = args.value("--label")?.unwrap_or_else(|| {
        if gpu {
            "Sealed Clef (H100 in CC mode, Intel TDX, Confidential Space)"
        } else {
            "Sealed Clef (Intel TDX, Confidential Space)"
        }
        .into()
    });
    config.slots = u32::try_from(args.number("--slots", 1)?.clamp(1, 8)).unwrap_or(1);
    config.rate_per_minute = u32::try_from(args.number("--rate", 6)?.clamp(1, 60)).unwrap_or(6);
    if gpu {
        // One H100, 80 GB.
        config.class.family = nostr::pylon::Family::Gpu;
        config.class.tier = Tier::for_gpu(80);
        config.class.memory_gb = 64;
    } else {
        config.class.family = nostr::pylon::Family::Cpu;
        config.class.tier = Tier::Small;
        config.class.memory_gb = 16;
    }
    config.pools = Vec::new();
    config.allow = None;
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    for (value, field) in [
        (&release, "release"),
        (&publisher, "publisher"),
        (&operator, "operator"),
    ] {
        if hex_pubkey(value).as_deref() != Some(value.as_str())
            && !(value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(format!("--{field} is not 64 hex characters"));
        }
    }
    let setup = Setup {
        release: release.clone(),
        publisher,
        workload,
        operator,
        socket: PathBuf::from(socket),
    };
    // The endpoint key lives only in this process's memory.
    let identity = Identity::generate();
    eprintln!("pylon: attested endpoint key {}", identity.pubkey());
    attested::fetch_weights(&weights_url, &weights_sha256, &weights).await?;
    let mut psionic_command = std::process::Command::new(&psionic);
    crate::harden::scrub(&mut psionic_command);
    psionic_command.args(["-m"]).arg(&weights).args([
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "--decision-device",
        &device,
    ]);
    if let Some(n) = &chunk {
        psionic_command.args(["--decision-chunk", n]);
    }
    let mut psionic_child = psionic_command
        .spawn()
        .map_err(|e| format!("Psionic did not start: {e}"))?;
    let url = format!("http://127.0.0.1:{port}");
    let clef = crate::decide::Clef::new(&url, None)?;
    let started = std::time::Instant::now();
    let served = loop {
        match clef.refresh().await {
            Ok(found) => break found,
            Err(why) => {
                if let Ok(Some(status)) = psionic_child.try_wait() {
                    return Err(format!("Psionic exited ({status}) before it served"));
                }
                if started.elapsed() > Duration::from_secs(1_800) {
                    return Err(format!("Psionic never served: {why}"));
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    };
    let pinned = format!("sha256:{weights_sha256}");
    if served.artifact_digest.as_deref() != Some(pinned.as_str()) {
        let _ = psionic_child.kill();
        return Err(format!(
            "Psionic serves {:?}, not the pinned weights {pinned}; refusing",
            served.artifact_digest
        ));
    }
    eprintln!("pylon: Psionic serves {} on loopback", served.advertised());
    let instance = attested::instance_id();
    let (first, claims) = attested::endpoint_event(&identity, &setup, &instance).await?;
    if require_gpu_cc {
        match attested::check_gpu_cc(&claims) {
            Ok((count, driver)) => eprintln!(
                "pylon: the token attests {count} H100 in confidential-computing mode (driver {driver})"
            ),
            Err(why) => {
                let _ = psionic_child.kill();
                return Err(why);
            }
        }
    }
    let measurement = claims["submods"]["container"]["image_digest"]
        .as_str()
        .ok_or("the launcher's token names no image digest")?
        .to_string();
    let attestation = Attestation {
        address: format!(
            "{}:{}:{instance}",
            nostr::att::ENDPOINT_KIND,
            identity.pubkey()
        ),
        release,
        measurement: measurement.clone(),
        level: nostr::att::Level::TeeCloud,
        model: served.model.clone(),
        model_digest: pinned,
    };
    let provider = Provider::deciding(
        config,
        identity.clone(),
        None,
        Arc::new(clef),
        Arc::new(Dedicated),
    )?;
    provider.attest(attestation.clone())?;
    provider.queue(first).await?;
    emit(
        json_out,
        &json!({
            "serving": true,
            "attested": attestation.address,
            "release": attestation.release,
            "measurement": measurement,
            "model": served.advertised(),
            "relay": relay,
        }),
        &format!(
            "attested pylon {slug} serving sealed decisions on {relay}\nendpoint {}\nmeasurement {measurement}\nmodel {}",
            attestation.address,
            served.advertised()
        ),
    );
    let refresher = {
        let provider = Arc::clone(&provider);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(attested::REFRESH).await;
                match attested::endpoint_event(&identity, &setup, &instance).await {
                    Ok((_, claims))
                        if require_gpu_cc && attested::check_gpu_cc(&claims).is_err() =>
                    {
                        // The GPU left CC mode: stop serving rather than
                        // publish an endpoint without it.
                        eprintln!(
                            "pylon: {}; stopping",
                            attested::check_gpu_cc(&claims).unwrap_err()
                        );
                        std::process::exit(1);
                    }
                    Ok((event, _)) => {
                        if provider.queue(event).await.is_err() {
                            return;
                        }
                        eprintln!("pylon: endpoint refreshed with a fresh token");
                    }
                    Err(why) => eprintln!("pylon: endpoint refresh failed: {why}"),
                }
            }
        })
    };
    let idler = (idle_stop > 0).then(|| {
        let provider = Arc::clone(&provider);
        tokio::spawn(async move {
            let idle = Duration::from_secs(idle_stop);
            let mut seen = provider.counters().await;
            let mut since = std::time::Instant::now();
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                let now = provider.counters().await;
                if now != seen {
                    seen = now;
                    since = std::time::Instant::now();
                } else if since.elapsed() >= idle {
                    eprintln!("pylon: idle, stopping");
                    if let Err(why) = attested::stop_self().await {
                        eprintln!("pylon: the idle stop failed: {why}");
                    }
                    // Try again after another idle period if the VM is
                    // still up.
                    since = std::time::Instant::now();
                }
            }
        })
    });
    let stop = async {
        #[cfg(unix)]
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        // No SIGTERM off Unix; Ctrl-C still stops the provider.
        #[cfg(not(unix))]
        let mut term: Option<NoTerm> = None;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
        }
    };
    let result = provider.run(stop).await;
    refresher.abort();
    if let Some(idler) = idler {
        idler.abort();
    }
    let _ = psionic_child.kill();
    result
}

fn link(json_out: bool, args: &mut Args) -> Result<(), String> {
    let provider = key("provider")?;
    if args.flag("--remove") {
        save_owner(&home(), None)?;
    } else if let Some(file) = args.value("--owner-secret")? {
        let credential = mint_owner(std::path::Path::new(&file), provider.pubkey())?;
        save_owner(&home(), Some(&credential))?;
    } else if let Some(text) = args.value("--credential")? {
        let credential = parse_owner(&text)?;
        check_owner(&provider, &credential)?;
        save_owner(&home(), Some(&credential))?;
    }
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let owner = load_owner(&home())?.map(|o| npub(&o.owner_pubkey));
    emit(
        json_out,
        &json!({"pylon": provider.npub(), "owner": owner}),
        &format!(
            "pylon {}
owner {}",
            provider.npub(),
            owner.as_deref().unwrap_or("not linked")
        ),
    );
    Ok(())
}

async fn ask(
    json_out: bool,
    args: &mut Args,
    relay: &str,
    checkers: BTreeSet<String>,
    wallet: &dyn Wallet,
) -> Result<(), String> {
    let pylon = match args.value("--pylon")? {
        Some(p) => Some(hex_pubkey(&p).ok_or("--pylon is not a key")?),
        None => None,
    };
    let wait = Duration::from_secs(args.number("--wait", 90)?.clamp(1, 600));
    let publish_receipt = !args.flag("--no-receipt");
    let max_msat = args.number("--max-msat", 0)?;
    let net = network(args)?;
    let prompt = args.words.join(" ");
    if prompt.trim().is_empty() {
        return Err("ask needs a prompt".into());
    }
    let buyer = key("buyer")?;
    let pay = if max_msat > 0 {
        let grant = grant_for(net)?;
        let payer = wallet.payer(net, max_msat)?;
        Some(Pay::Wallet {
            payer: Arc::new(Granted::new(payer, grant, &home())),
            max_msat,
        })
    } else {
        None
    };
    let answer = client::ask(
        &buyer,
        &Ask {
            relay: relay.into(),
            pylon,
            prompt,
            wait,
            publish_receipt,
            home: home(),
            checkers,
            pay,
        },
    )
    .await?;
    let value = serde_json::to_value(&answer).map_err(|e| e.to_string())?;
    let mut text = format!(
        "pylon   {} ({})\nmodel   {}\n",
        answer.label, answer.pylon_npub, answer.model
    );
    match &answer.text {
        Some(t) => text.push_str(&format!("answer  {t}\n")),
        None => text.push_str(&format!(
            "error   {}\n",
            answer.error.as_deref().unwrap_or("no answer")
        )),
    }
    text.push_str(&format!(
        "latency discovery {} ms, first contact {}, answer {}\n",
        answer.discover_ms,
        answer
            .contact_ms
            .map_or("-".into(), |ms| format!("{ms} ms")),
        answer.answer_ms.map_or("-".into(), |ms| format!("{ms} ms")),
    ));
    match (&answer.receipt, &answer.receipt_error) {
        (Some(id), _) => text.push_str(&format!("receipt {id} ({})", answer.outcome)),
        (None, Some(e)) => text.push_str(&format!("receipt not published: {e}")),
        (None, None) => text.push_str("receipt not published"),
    }
    emit(json_out, &value, &text);
    if answer.text.is_some() {
        Ok(())
    } else {
        Err(answer.error.unwrap_or_default())
    }
}

async fn status(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let pool = args.value("--pool")?;
    let field = RelayField::new(relay, pool.as_deref(), key("buyer")?);
    let states = field.poll().await?;
    let mut text = String::new();
    if states.is_empty() {
        text.push_str("no pylons on this relay");
    }
    for s in &states {
        text.push_str(&format!(
            "{:<8} {:<20} {:?}/{:?} busy {}/{} jobs {} up {}s  {}\n",
            s.status, s.label, s.family, s.tier, s.busy, s.total, s.jobs, s.uptime, s.pylon
        ));
    }
    emit(
        json_out,
        &json!({"relay": relay, "pylons": states}),
        text.trim_end(),
    );
    Ok(())
}

async fn pool(
    json_out: bool,
    args: &mut Args,
    relay: &str,
    checkers: BTreeSet<String>,
) -> Result<(), String> {
    let verify = args.words.first().is_some_and(|w| w == "verify");
    if verify {
        args.words.remove(0);
    }
    let slug = args.value("--pool")?.unwrap_or_else(|| "everglade".into());
    let mut policy =
        PoolPolicy::open(&slug, pool::SLICES).trusting_payments(crate::paid::trusted_issuers());
    if args.flag("--checked") {
        if checkers.is_empty() {
            return Err("--checked needs a trusted checker (--checker NPUB)".into());
        }
        policy = policy.checked(checkers);
    }
    if verify {
        let aggregator = args
            .value("--aggregator")?
            .ok_or("pool verify needs --aggregator NPUB")?;
        let aggregator = hex_pubkey(&aggregator).ok_or("--aggregator is not a key")?;
        let aggregate = pool::verify(&key("buyer")?, relay, &aggregator, &policy).await?;
        emit(
            json_out,
            &json!({"verified": true, "aggregate": aggregate}),
            &format!(
                "verified: {} online, {} accepted jobs, {} tokens in the window",
                aggregate.totals.pylons_online,
                aggregate.totals.jobs.accepted,
                aggregate.totals.units.tokens
            ),
        );
        return Ok(());
    }
    let minutes = args.number("--minutes", 60)?;
    let publish = args.flag("--publish");
    let aggregator = key("aggregator")?;
    let policy_path: PathBuf = home().join(format!("pool-{slug}.policy.json"));
    std::fs::create_dir_all(home()).map_err(|e| e.to_string())?;
    std::fs::write(
        &policy_path,
        serde_json::to_vec_pretty(&policy).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let (aggregate, event) = pool::aggregate(&aggregator, relay, &policy, minutes, publish).await?;
    emit(
        json_out,
        &json!({"aggregate": aggregate, "event": event.as_ref().map(|e| &e.id), "policy": policy_path}),
        &format!(
            "pool {slug}: {} online, {} slots ({} free), {} accepted jobs, {} tokens, rate {:?}\npolicy {} ({})\n{}",
            aggregate.totals.pylons_online,
            aggregate.totals.slots_total,
            aggregate.totals.slots_free,
            aggregate.totals.jobs.accepted,
            aggregate.totals.units.tokens,
            aggregate.rate,
            aggregate.policy,
            policy_path.display(),
            event.map_or("not published (pass --publish)".into(), |e| format!(
                "published {} by {}",
                e.id,
                aggregator.npub()
            )),
        ),
    );
    Ok(())
}

async fn check(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let mode = if args.words.is_empty() {
        String::new()
    } else {
        args.words.remove(0)
    };
    let pylons = args
        .values("--pylon")?
        .iter()
        .map(|p| hex_pubkey(p).ok_or_else(|| format!("`{p}` is not a key")))
        .collect::<Result<Vec<_>, _>>()?;
    let award = args.flag("--award");
    let checker = crate::check::Checker {
        relay: relay.into(),
        checker: key("checker")?,
        buyer: key("buyer")?,
        home: home(),
        wait: Duration::from_secs(args.number("--wait", 90)?.clamp(1, 600)),
    };
    if mode == "decide" && args.flag("--watch") {
        return check_decide_watch(json_out, args, checker, pylons).await;
    }
    if mode == "decide" {
        let [pylon] = pylons.as_slice() else {
            return Err("check decide takes one --pylon NPUB".into());
        };
        let reference = args
            .value("--reference")?
            .unwrap_or_else(|| DEFAULT_DECIDE.into());
        let rounds = args.number("--rounds", 1)?.clamp(1, 100);
        let public = args
            .value("--seed")?
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| "--seed takes a number".to_string())
            })
            .transpose()?;
        let fresh_buyer = !args.flag("--own-buyer");
        let clef = crate::decide::Clef::new(&reference, None)?;
        clef.refresh().await?;
        let secret = match public {
            Some(_) => None,
            None => Some(crate::items::checker_seed(&home())?),
        };
        let mut results = Vec::new();
        let mut text = String::new();
        for n in 0..rounds {
            let seed = match (public, secret) {
                (Some(seed), _) => crate::check_decide::Seed::Public(seed.wrapping_add(n)),
                (None, Some(secret)) => crate::check_decide::Seed::Private {
                    secret,
                    round: crate::check_decide::next_round(&home())?,
                },
                (None, None) => crate::check_decide::Seed::Fresh,
            };
            let plan = crate::check_decide::Plan { seed, fresh_buyer };
            let c = crate::check_decide::round_with(
                &checker,
                pylon,
                &clef,
                &crate::verify::CLEF_FLASH,
                plan,
            )
            .await?;
            text.push_str(&format!(
                "{:<18} {}  mean {:.4}  worst {:.4}  verified {}/{}  label {}{}\n",
                c.verdict.label(),
                c.pylon,
                c.round.mean,
                c.round.worst,
                c.round.verified,
                c.items.len(),
                c.label.as_deref().unwrap_or("not published"),
                c.items
                    .iter()
                    .find_map(|i| i.problem.as_ref())
                    .map(|p| format!("  ({p})"))
                    .unwrap_or_default()
            ));
            results.push(c);
        }
        text.push_str(&format!("checker {}", checker.checker.npub()));
        emit(
            json_out,
            &json!({"checker": checker.checker.npub(), "rounds": results}),
            &text,
        );
        return Ok(());
    }
    let checked = match mode.as_str() {
        "canary" => {
            let [pylon] = pylons.as_slice() else {
                return Err("check canary takes one --pylon NPUB".into());
            };
            checker.canaries(pylon).await?
        }
        "redundant" => {
            let prompt = args.words.join(" ");
            if prompt.trim().is_empty() || !(2..=5).contains(&pylons.len()) {
                return Err("check redundant takes a prompt and 2 to 5 --pylon NPUB".into());
            }
            checker.redundant(&prompt, &pylons).await?
        }
        _ => return Err("check takes `canary`, `redundant`, or `decide`".into()),
    };
    let awarded = if award {
        checker.award(&checked).await?
    } else {
        None
    };
    let mut text = String::new();
    for c in &checked {
        text.push_str(&format!(
            "{:<18} {}  answer {:?}  label {}\n",
            c.verdict.label(),
            c.pylon,
            c.answer.as_deref().unwrap_or("-"),
            c.label.as_deref().unwrap_or("not published"),
        ));
    }
    text.push_str(&format!(
        "checker {}\n{}",
        checker.checker.npub(),
        awarded.as_ref().map_or_else(
            || "no XP awarded".to_string(),
            |id| format!("XP award {id}")
        )
    ));
    emit(
        json_out,
        &json!({"checker": checker.checker.npub(), "checked": checked, "award": awarded}),
        &text,
    );
    Ok(())
}

/// `check decide --watch`: the continuous decision checker. Runs until
/// interrupted, one line (or JSON document) per finished round.
async fn check_decide_watch(
    json_out: bool,
    args: &mut Args,
    checker: crate::check::Checker,
    pylons: Vec<String>,
) -> Result<(), String> {
    use crate::check_decide::{Items, WatchConfig, Watcher};
    let reference = args
        .value("--reference")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    let config = WatchConfig {
        every: args.number("--every", 900)?.clamp(10, 7 * 86_400),
        recheck_struck: args
            .number("--recheck-struck", 3_600)?
            .clamp(10, 7 * 86_400),
        concurrency: usize::try_from(args.number("--concurrency", 4)?.clamp(1, 64)).unwrap_or(4),
        pool: args.value("--pool")?,
        only: (!pylons.is_empty()).then(|| pylons.into_iter().collect()),
        fresh_buyer: !args.flag("--own-buyer"),
        ..WatchConfig::default()
    };
    let poll = Duration::from_secs(args.number("--poll", 30)?.clamp(1, 3_600));
    let items = match args.value("--seed")? {
        Some(v) => Items::Public(v.parse().map_err(|_| "--seed takes a number".to_string())?),
        None => Items::Private(crate::items::checker_seed(&home())?),
    };
    let clef = Arc::new(crate::decide::Clef::new(&reference, None)?);
    clef.refresh().await?;
    let npub = checker.checker.npub();
    let mut watcher = Watcher::new(checker, clef, crate::verify::CLEF_FLASH, config, items);
    eprintln!(
        "pylon: checking decision pylons as {npub}, every {} s, struck pylons after {} s",
        watcher.config.every, watcher.config.recheck_struck
    );
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);
    loop {
        match watcher.pass(crate::now()).await {
            Ok(watched) => {
                for w in watched {
                    let text = match &w.check {
                        Some(c) => format!(
                            "{:<18} {}  mean {:.4}  worst {:.4}  label {}  next {}",
                            c.verdict.label(),
                            c.pylon,
                            c.round.mean,
                            c.round.worst,
                            c.label.as_deref().unwrap_or("not published"),
                            w.next
                        ),
                        None => format!(
                            "not checked        {}  ({})  next {}",
                            w.pylon,
                            w.error.as_deref().unwrap_or("no round"),
                            w.next
                        ),
                    };
                    emit(
                        json_out,
                        &serde_json::to_value(&w).unwrap_or_default(),
                        &text,
                    );
                }
            }
            Err(e) => eprintln!("pylon: checker pass: {e}"),
        }
        tokio::select! {
            _ = &mut stop => return Ok(()),
            () = tokio::time::sleep(poll) => {}
        }
    }
}

async fn league(json_out: bool, relay: &str, checkers: BTreeSet<String>) -> Result<(), String> {
    let league = crate::league::fetch(&key("buyer")?, relay, &checkers).await?;
    emit(
        json_out,
        &serde_json::to_value(&league).map_err(|e| e.to_string())?,
        &crate::league::render(&league),
    );
    Ok(())
}

/// Stands in for the SIGTERM stream where there is none (Windows).
#[cfg(not(unix))]
struct NoTerm;

#[cfg(not(unix))]
impl NoTerm {
    async fn recv(&mut self) -> Option<()> {
        std::future::pending().await
    }
}

/// The default local decision server: Psionic's Clef lane on loopback.
pub const DEFAULT_DECIDE: &str = "http://127.0.0.1:18096";

/// `calibrate run|compare|reference`.
/// `canary make`: a checker-private pool of decision canaries.
async fn canary(json_out: bool, args: &mut Args) -> Result<(), String> {
    let Some(sub) = (!args.words.is_empty()).then(|| args.words.remove(0)) else {
        return Err("canary needs make".into());
    };
    if sub != "make" {
        return Err(format!("unknown canary command `{sub}`; it takes make"));
    }
    let seed_file = args
        .value("--seed-file")?
        .map(PathBuf::from)
        .ok_or("canary make needs --seed-file FILE")?;
    let reference = args
        .value("--reference")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    let count = u32::try_from(args.number("--count", 200)?.clamp(1, 100_000)).unwrap_or(200);
    let out = args
        .value("--out")?
        .map(PathBuf::from)
        .ok_or("canary make needs --out FILE")?;
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let secret = std::fs::read(&seed_file).map_err(|e| format!("{}: {e}", seed_file.display()))?;
    if secret.len() < 16 {
        return Err(format!(
            "{} holds {} bytes; a seed file needs at least 16 random bytes",
            seed_file.display(),
            secret.len()
        ));
    }
    let server = crate::decide::Clef::new(&reference, None)?;
    let pool = crate::canary::make(crate::canary::seed(&secret), &server, count).await?;
    crate::canary::write(&out, &pool)?;
    let identity = pool.first().map(|c| c.identity.clone()).unwrap_or_default();
    emit(
        json_out,
        &json!({"ok": true, "canaries": pool.len(), "identity": identity, "out": out}),
        &format!(
            "Wrote {} canaries answered by {identity} to {}. Keep it and the seed file private.",
            pool.len(),
            out.display()
        ),
    );
    Ok(())
}

async fn calibrate(json_out: bool, args: &mut Args) -> Result<(), String> {
    let Some(sub) = (!args.words.is_empty()).then(|| args.words.remove(0)) else {
        return Err("calibrate needs run, compare, or reference".into());
    };
    let limits = crate::verify::CLEF_FLASH;
    match sub.as_str() {
        "run" => {
            let url = args
                .value("--decide")?
                .unwrap_or_else(|| DEFAULT_DECIDE.into());
            let model = args
                .value("--decide-model")?
                .unwrap_or_else(|| "clef-flash".into());
            let seed = args.number("--seed", 20_261_014)?;
            let count =
                u32::try_from(args.number("--count", 200)?.clamp(1, 100_000)).unwrap_or(200);
            let out = args.value("--out")?.map(PathBuf::from);
            if let Some(extra) = args.words.first() {
                return Err(format!("unexpected argument `{extra}`"));
            }
            let mut server = crate::calibrate::Server::new(&url, &model)?;
            // Stamp the model id the served file resolves to here, so a
            // reference made from this run keys on the weights.
            server.model_id = crate::decide::Clef::new(&url, Some(&model))?
                .refresh()
                .await
                .ok()
                .and_then(|found| found.artifact_digest)
                .and_then(|digest| crate::setup::model_id_for(&digest, None));
            let mut done = 0u32;
            let run = crate::calibrate::run(&server, seed, count, out.as_deref(), |a| {
                done += 1;
                if !json_out && (done % 25 == 0 || a.problem.is_some()) {
                    eprintln!(
                        "{done}/{count}{}",
                        a.problem
                            .as_ref()
                            .map(|p| format!(" item {}: {p}", a.index))
                            .unwrap_or_default()
                    );
                }
            })
            .await?;
            let unusable = run.iter().filter(|a| a.probabilities.is_none()).count();
            let ms: u64 = run.iter().map(|a| a.ms).sum();
            emit(
                json_out,
                &json!({"items": run.len(), "unusable": unusable, "seed": seed,
                        "model_id": server.model_id,
                        "artifact_digest": run.first().and_then(|a| a.artifact_digest.clone()),
                        "backend": run.first().and_then(|a| a.backend.clone()),
                        "mean_ms": ms / run.len().max(1) as u64, "out": out}),
                &format!(
                    "{} items answered ({unusable} unusable), mean {} ms each{}",
                    run.len(),
                    ms / run.len().max(1) as u64,
                    out.map(|p| format!(", written to {}", p.display()))
                        .unwrap_or_default()
                ),
            );
            Ok(())
        }
        "compare" => {
            if args.words.len() != 2 {
                return Err("calibrate compare REFERENCE.jsonl CANDIDATE.jsonl".into());
            }
            let reference = crate::calibrate::load(std::path::Path::new(&args.words[0]))?;
            let candidate = crate::calibrate::load(std::path::Path::new(&args.words[1]))?;
            let c = crate::calibrate::compare(&reference, &candidate, &limits);
            emit(
                json_out,
                &serde_json::to_value(&c).map_err(|e| e.to_string())?,
                &format!(
                    "{} items paired, {} unusable\ndistance median {:.4}, p90 {:.4}, p99 {:.4}, max {:.4}\ntop answer agrees on {:.1}%\nsimulated rounds ({} verified, limits {}/{}/{}): {:.2}% pass, {:.2}% strike",
                    c.paired,
                    c.unusable,
                    c.distance.median,
                    c.distance.p90,
                    c.distance.p99,
                    c.distance.max,
                    c.top_agree * 100.0,
                    limits.verified,
                    limits.mean,
                    limits.strike,
                    limits.item,
                    c.round_pass * 100.0,
                    c.round_strike * 100.0
                ),
            );
            Ok(())
        }
        "reference" => {
            let measured_on = args
                .value("--measured-on")?
                .ok_or("calibrate reference RUN.jsonl --measured-on TEXT --out FILE")?;
            let out = args
                .value("--out")?
                .ok_or("calibrate reference needs --out FILE")?;
            let [run] = args.words.as_slice() else {
                return Err("calibrate reference RUN.jsonl --measured-on TEXT --out FILE".into());
            };
            let run = crate::calibrate::load(std::path::Path::new(run))?;
            let reference = crate::calibrate::Reference::from_run("clef-flash", &measured_on, &run)
                .ok_or("the run is empty")?;
            let text = serde_json::to_string(&reference).map_err(|e| e.to_string())?;
            std::fs::write(&out, text).map_err(|e| format!("cannot write {out}: {e}"))?;
            emit(
                json_out,
                &json!({"items": reference.items.len(), "out": out}),
                &format!("{} reference items written to {out}", reference.items.len()),
            );
            Ok(())
        }
        other => Err(format!("unknown calibrate command `{other}`")),
    }
}

/// `selfcheck`: answer the shipped reference set on this computer's
/// decision server and keep the result; `serve --decide` reads it.
async fn selfcheck(json_out: bool, args: &mut Args) -> Result<(), crate::envelope::Outcome> {
    let url = args
        .value("--decide")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    let limit = usize::try_from(args.number("--items", 64)?.clamp(8, 10_000)).unwrap_or(64);
    let model_file = args.value("--model-file")?.map(PathBuf::from);
    if let Some(extra) = args.words.first() {
        return Err(crate::envelope::Outcome::Usage(format!(
            "unexpected argument `{extra}`"
        )));
    }
    let check = run_selfcheck(&url, limit, json_out, model_file.as_deref()).await?;
    let text = format!(
        "{}: {} items on {} ({}), distance median {:.4}, max {:.4}; simulated rounds {:.2}% pass, {:.2}% strike; {:.2} decisions/s",
        if check.pass { "PASS" } else { "FAIL" },
        check.items,
        check.model,
        check.backend.as_deref().unwrap_or("unknown backend"),
        check.median,
        check.max,
        check.round_pass * 100.0,
        check.round_strike * 100.0,
        check.decisions_per_second
    );
    emit(
        json_out,
        &serde_json::to_value(&check).map_err(|e| e.to_string())?,
        &text,
    );
    if check.pass {
        Ok(())
    } else {
        Err(crate::envelope::Outcome::VerificationFailed(
            "this computer's answers differ from the reference; it should not serve decisions until the self-check passes".into(),
        ))
    }
}

/// Run and keep a self-check of the decision server at `url`. The
/// server names its file's sha256; the reference is looked up by the
/// model id that file resolves to here (`model_file`, when given, is the
/// GGUF the server loaded).
pub(crate) async fn run_selfcheck(
    url: &str,
    limit: usize,
    quiet: bool,
    model_file: Option<&std::path::Path>,
) -> Result<crate::calibrate::SelfCheck, String> {
    let clef = crate::decide::Clef::new(url, None)?;
    let found = clef.refresh().await?;
    let digest = found.artifact_digest.clone().ok_or(
        "the decision server names no artifact digest, so there is nothing to check it against",
    )?;
    let model_id = crate::setup::model_id_for(&digest, model_file).ok_or_else(|| {
        format!(
            "the server's model file ({digest}) is not the pinned download and no GGUF here has that hash; \
             pass the file it loaded with --model-file PATH"
        )
    })?;
    let reference = crate::calibrate::shipped_reference(&model_id).ok_or_else(|| {
        format!(
            "no reference set ships for {model_id} (file {digest}); the pinned model is Clef-Flash Q4_K_M ({})",
            crate::setup::CLEF_FLASH_MODEL_ID
        )
    })?;
    let server = crate::calibrate::Server::new(url, &found.model)?;
    let total = limit.min(reference.items.len());
    let mut done = 0usize;
    let check = crate::calibrate::selfcheck(
        &server,
        &reference,
        limit,
        &crate::verify::CLEF_FLASH,
        |_| {
            done += 1;
            if !quiet && done % 16 == 0 {
                eprintln!("self-check {done}/{total}");
            }
        },
    )
    .await;
    crate::calibrate::save(&home(), &check)?;
    Ok(check)
}

/// `doctor`: what this computer has and what blocks serving decisions.
async fn doctor(
    json_out: bool,
    args: &mut Args,
    relay: &str,
) -> Result<(), crate::envelope::Outcome> {
    let decide = args
        .value("--decide")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    let rehash = args.flag("--rehash");
    let allow_slow = args.flag("--allow-slow");
    if let Some(extra) = args.words.first() {
        return Err(crate::envelope::Outcome::Usage(format!(
            "unexpected argument `{extra}`"
        )));
    }
    let mut report = crate::setup::doctor(relay, &decide, rehash).await;
    if allow_slow {
        report = report.allowing_slow();
    }
    emit(
        json_out,
        &serde_json::to_value(&report).map_err(|e| e.to_string())?,
        &crate::setup::render(&report),
    );
    if report.blocks.is_empty() {
        Ok(())
    } else {
        Err(crate::envelope::Outcome::Blocked {
            blocks: report.blocks,
        })
    }
}

/// `setup` and `update`: fetch the pinned model when it is missing or not
/// the pinned file, install this release's Psionic engine for this
/// platform, then self-check a running decision server. Neither changes
/// the served model id without `--accept-model-change`.
async fn setup(json_out: bool, args: &mut Args, update: bool) -> Result<(), String> {
    let decide = args
        .value("--decide")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    let accept = args.flag(crate::install::ACCEPT_MODEL_CHANGE);
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let path = crate::setup::model_path();
    let mut state = crate::setup::model_state(&path, crate::setup::CLEF_FLASH_MODEL_ID, update);
    let change = crate::install::guard_model_change(
        &home(),
        state.model_id.as_deref(),
        crate::setup::CLEF_FLASH_MODEL_ID,
        accept,
    )?;
    let fetched = !state.verified;
    if fetched {
        if !json_out {
            eprintln!(
                "downloading Clef-Flash Q4_K_M ({:.1} GB) to {}",
                crate::setup::CLEF_FLASH_BYTES as f64 / 1e9,
                path.display()
            );
        }
        let mut shown = 0u64;
        crate::setup::fetch(
            crate::setup::CLEF_FLASH_URL,
            &path,
            crate::setup::CLEF_FLASH_BYTES,
            crate::setup::CLEF_FLASH_SHA256,
            |got| {
                if !json_out && got >= shown + 500_000_000 {
                    shown = got;
                    eprintln!(
                        "{:.1} of {:.1} GB",
                        got as f64 / 1e9,
                        crate::setup::CLEF_FLASH_BYTES as f64 / 1e9
                    );
                }
            },
        )
        .await
        .map_err(crate::envelope::network)?;
        state = crate::setup::model_state(&path, crate::setup::CLEF_FLASH_MODEL_ID, false);
        if !state.verified {
            return Err(format!(
                "the download's tensors are not the pinned model's ({:?}, wanted {})",
                state.model_id,
                crate::setup::CLEF_FLASH_MODEL_ID
            ));
        }
    }
    crate::install::record_served_model(&home(), crate::setup::CLEF_FLASH_MODEL_ID)?;
    let host = crate::setup::host();
    let platform = crate::install::platform(&host);
    let pin = crate::install::this_pin(&host);
    if let Some(pin) = &pin
        && !json_out
    {
        eprintln!(
            "checking the Psionic engine ({}, {:.0} MB)",
            pin.platform,
            pin.size as f64 / 1e6
        );
    }
    let engine = crate::install::ensure_engine(
        pin.as_ref(),
        platform.as_deref(),
        &crate::install::bin_dir(),
        |_| {},
    )
    .await?;
    let new_engine = matches!(engine, crate::install::EngineOutcome::Installed { .. });
    if new_engine {
        // A new engine is checked again before it serves.
        crate::install::reset_selfcheck(&home())?;
    }
    let server = engine.path().map(std::path::Path::to_path_buf);
    // A server already answering runs the old engine; `start` checks the
    // new one.
    let check = if !new_engine && crate::setup::serving(&decide).await.is_some() {
        Some(run_selfcheck(&decide, 64, json_out, None).await?)
    } else {
        None
    };
    let mut next = Vec::new();
    if server.is_none() {
        next.push(crate::setup::BUILD_SERVER.to_string());
    }
    next.push("openagents pylon start --allow-any".to_string());
    let changed = match &change {
        crate::install::ModelChange::Changed { from, to } => {
            format!(
                "served model changed from {from} to {to}; it is checked again before it serves\n"
            )
        }
        _ => String::new(),
    };
    emit(
        json_out,
        &json!({"model": state, "fetched": fetched, "model_change": change, "engine": engine,
                "server": server, "selfcheck": check, "next": next}),
        &format!(
            "{changed}model {} ({}{})\nengine {}\n{}next: {}",
            path.display(),
            if fetched {
                "downloaded and verified"
            } else {
                "already the pinned model"
            },
            state
                .sha256
                .as_deref()
                .map(|sha| format!(", file sha256 {}…", &sha[..sha.len().min(8)]))
                .unwrap_or_default(),
            engine.describe(),
            check
                .as_ref()
                .map(|c| format!(
                    "self-check {}: {:.2}% of rounds pass\n",
                    if c.pass { "passed" } else { "failed" },
                    c.round_pass * 100.0
                ))
                .unwrap_or_default(),
            next.join("\nnext: ")
        ),
    );
    Ok(())
}

/// `start`: run Psionic's decision lane on loopback (unless one already
/// answers), self-check it when this model has no passing self-check, and
/// serve decisions only. Takes `serve`'s options (`--allow`, `--slots`, …).
async fn start(
    json_out: bool,
    args: &mut Args,
    relay: &str,
    wallet: &dyn Wallet,
) -> Result<(), String> {
    let port = u16::try_from(args.number("--port", u64::from(crate::setup::DECIDE_PORT))?)
        .map_err(|_| "--port takes a port number")?;
    let device = args.value("--device")?;
    let yes = args.flag("--yes");
    let allow_slow = args.flag("--allow-slow");
    let fit = crate::setup::fit(&crate::setup::host());
    if device.as_deref() == Some("cpu") || (device.is_none() && !fit.useful) {
        let cpu = crate::setup::Fit {
            device: "cpu".into(),
            useful: false,
            note: if fit.useful {
                "--device cpu decides on the CPU".into()
            } else {
                fit.note.clone()
            },
        };
        crate::setup::may_serve_on(&cpu, allow_slow)?;
    }
    let url = format!("http://127.0.0.1:{port}");
    let mut child = None;
    if crate::setup::serving(&url).await.is_none() {
        let server = crate::setup::find_server().ok_or_else(|| {
            if crate::install::this_pin(&crate::setup::host()).is_some() {
                format!(
                    "{} is not installed; run `openagents pylon setup`",
                    crate::setup::SERVER
                )
            } else {
                format!(
                    "{} is not installed; build it with: {}",
                    crate::setup::SERVER,
                    crate::setup::BUILD_SERVER
                )
            }
        })?;
        let model = crate::setup::model_path();
        if !crate::setup::model_state(&model, crate::setup::CLEF_FLASH_MODEL_ID, false).verified {
            return Err("the decision model is missing or not the pinned model; run `openagents pylon setup`".into());
        }
        let device = device.unwrap_or_else(|| fit.device.clone());
        let log = home().join("psionic-decide.log");
        std::fs::create_dir_all(home()).map_err(|e| e.to_string())?;
        let out = std::fs::File::create(&log).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        eprintln!(
            "pylon: starting Psionic on {device} at {url} (log {})",
            log.display()
        );
        let lane = crate::online::LaneSpec {
            kind: crate::online::LaneKind::Decisions,
            port,
            model_file: model,
            device: Some(device),
            model: None,
        };
        let mut command = crate::online::psionic_command(&server, &lane);
        let spawned = crate::harden::scrub_async(&mut command)
            .stdout(out)
            .stderr(err)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Psionic did not start: {e}"))?;
        child = Some(spawned);
        let started = std::time::Instant::now();
        while crate::setup::serving(&url).await.is_none() {
            if let Some(c) = child.as_mut()
                && let Ok(Some(status)) = c.try_wait()
            {
                return Err(format!(
                    "Psionic exited ({status}) before it served; see {}",
                    log.display()
                ));
            }
            if started.elapsed() > Duration::from_secs(900) {
                return Err(format!("Psionic never served; see {}", log.display()));
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    let model_id = crate::setup::serving(&url)
        .await
        .and_then(|s| s["model_id"].as_str().map(str::to_string))
        .unwrap_or_default();
    if crate::calibrate::shipped_reference(&model_id).is_some()
        && crate::calibrate::may_serve(&home(), &model_id)?.is_none()
    {
        eprintln!("pylon: self-checking this computer before it serves");
        let check = run_selfcheck(&url, 64, json_out, None).await?;
        if !check.pass {
            return Err(format!(
                "the self-check failed ({:.1}% of rounds pass, worst item {:.3}); not serving",
                check.round_pass * 100.0,
                check.max
            ));
        }
    }
    start_offers(yes, wallet).await;
    args.words
        .extend(["--decide".to_string(), url, "--decisions-only".to_string()]);
    let served = serve(json_out, args, relay, wallet).await;
    drop(child);
    served
}

/// On `start` (and `up`): offer to link this pylon to the person's key and
/// to send its earnings to this computer's OpenAgents wallet. Neither
/// blocks serving; with no terminal and no `yes` neither changes anything.
pub async fn start_offers(yes: bool, wallet: &dyn Wallet) {
    use crate::earnings::PayoutOffer;
    use crate::identity::{LinkOffer, ask_on_terminal, offer_link};
    let home = home();
    let Ok(provider) = key("provider") else {
        return;
    };
    match offer_link(&home, &provider, yes) {
        Ok(LinkOffer::LinkedNow(owner)) => {
            eprintln!("pylon: linked to {}; its earnings go to you", npub(&owner));
        }
        Ok(_) => {}
        Err(e) => eprintln!("pylon: this pylon was not linked: {e}"),
    }
    let Some(spark) = wallet.spark() else {
        return;
    };
    let Ok(party) = crate::earnings::party(&home, &provider, None) else {
        return;
    };
    let api = crate::earnings::api_base(None);
    match crate::earnings::offer_payout(&api, &party, Some(spark), yes, &mut ask_on_terminal).await
    {
        Ok(PayoutOffer::SetNow(address)) => {
            eprintln!("pylon: earnings go to your OpenAgents wallet ({address})");
        }
        Ok(_) => {}
        Err(e) => eprintln!(
            "pylon: where earnings go is not set ({e}); `openagents pylon payout set` sets it"
        ),
    }
}

/// `earnings`: this pylon's statement.
async fn earnings(json_out: bool, args: &mut Args, wallet: &dyn Wallet) -> Result<(), String> {
    let api = crate::earnings::api_base(args.value("--api")?);
    let secret = args.value("--owner-secret")?;
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let party = crate::earnings::party(
        &home(),
        &key("provider")?,
        secret.as_deref().map(std::path::Path::new),
    )?;
    let (value, text) = crate::earnings::earnings(&api, &party, wallet.spark()).await?;
    emit(json_out, &value, &text);
    Ok(())
}

/// `payout set [ADDRESS]`: where this pylon's earnings go.
async fn payout(json_out: bool, args: &mut Args, wallet: &dyn Wallet) -> Result<(), String> {
    let api = crate::earnings::api_base(args.value("--api")?);
    let secret = args.value("--owner-secret")?;
    if args.words.first().map(String::as_str) != Some("set") {
        return Err("usage: openagents pylon payout set [ADDRESS]".into());
    }
    args.words.remove(0);
    let address = (!args.words.is_empty()).then(|| args.words.remove(0));
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let party = crate::earnings::party(
        &home(),
        &key("provider")?,
        secret.as_deref().map(std::path::Path::new),
    )?;
    let (value, text) =
        crate::earnings::payout_set(&api, &party, address.as_deref(), wallet.spark()).await?;
    emit(json_out, &value, &text);
    Ok(())
}

/// `plan`: the ordered steps that get this computer serving.
async fn plan(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let decide = args
        .value("--decide")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let report = crate::setup::doctor(relay, &decide, false).await;
    let facts = crate::plan::Facts {
        linked: load_owner(&home())?.is_some(),
        payout_set: None,
    };
    let steps = crate::plan::steps(&report, facts);
    let mut text = String::new();
    for (i, step) in steps.iter().enumerate() {
        let who = match step.who {
            crate::plan::Who::Agent => "agent",
            crate::plan::Who::Owner => "owner",
        };
        let extra: Vec<&str> = [step.size.as_deref(), step.time.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        text.push_str(&format!(
            "{}. [{}] {}{}{}{}\n",
            i + 1,
            if step.done { "done" } else { who },
            step.does,
            if extra.is_empty() {
                String::new()
            } else {
                format!(" ({})", extra.join(", "))
            },
            if step.done { "" } else { "\n   " },
            if step.done {
                String::new()
            } else {
                step.command.clone().unwrap_or_default()
            },
        ));
    }
    emit(
        json_out,
        &json!({"ready": report.ready, "device": report.fit.device, "steps": steps}),
        text.trim_end(),
    );
    Ok(())
}

/// `verify`: the running pylon's pass/fail checklist.
async fn verify(
    json_out: bool,
    args: &mut Args,
    relay: &str,
    checkers: &BTreeSet<String>,
) -> Result<(), String> {
    let decide = args
        .value("--decide")?
        .unwrap_or_else(|| DEFAULT_DECIDE.into());
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let pylon = key("provider")?;
    let owner = load_owner(&home())?;
    let port = decide
        .rsplit(':')
        .next()
        .and_then(|p| p.trim_end_matches('/').parse::<u16>().ok())
        .unwrap_or(crate::setup::DECIDE_PORT);
    let mut ports = vec![port, crate::checkup::TEXT_PORT];
    ports.dedup();
    let list = crate::checkup::run(crate::checkup::Inputs {
        relay,
        engine: &decide,
        pylon: &pylon,
        owner: owner.as_ref(),
        checkers,
        ports,
        addresses: None,
        now: crate::now(),
    })
    .await;
    let text = list
        .items
        .iter()
        .map(|item| {
            format!(
                "{:<4} {:<10} {}{}",
                match item.state {
                    crate::checkup::State::Pass => "ok",
                    crate::checkup::State::Fail => "FAIL",
                    crate::checkup::State::Skip => "-",
                },
                item.id,
                item.detail,
                item.fix
                    .as_deref()
                    .filter(|_| item.state == crate::checkup::State::Fail)
                    .map(|f| format!("\n                fix: {f}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    emit(
        json_out,
        &serde_json::to_value(&list).map_err(|e| e.to_string())?,
        &text,
    );
    if list.pass {
        Ok(())
    } else {
        Err("this pylon did not pass every check".into())
    }
}

/// `logs`: recent log lines, redacted, capped, marked untrusted.
fn logs(json_out: bool, args: &mut Args) -> Result<(), String> {
    let lines = usize::try_from(args.number("--lines", crate::logs::DEFAULT_LINES as u64)?)
        .unwrap_or(crate::logs::DEFAULT_LINES);
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let logs = crate::logs::read(&home(), crate::logs::host_log(), lines);
    let mut text = format!("({})\n", crate::logs::NOTICE);
    if logs.sources.is_empty() {
        text.push_str(&format!("no pylon logs yet under {}", home().display()));
    }
    for source in &logs.sources {
        text.push_str(&format!("\n== {}\n", source.path.display()));
        for line in &source.lines {
            text.push_str(line);
            text.push('\n');
        }
    }
    emit(
        json_out,
        &serde_json::to_value(&logs).map_err(|e| e.to_string())?,
        text.trim_end(),
    );
    Ok(())
}

/// `settings`: show or change the user's own pause choices.
fn settings(json_out: bool, args: &mut Args) -> Result<(), String> {
    let switch = |value: Option<String>, name: &str| -> Result<Option<bool>, String> {
        match value.as_deref() {
            None => Ok(None),
            Some("on") => Ok(Some(true)),
            Some("off") => Ok(Some(false)),
            Some(other) => Err(format!("{name} takes on or off, not `{other}`")),
        }
    };
    let battery = switch(args.value("--pause-on-battery")?, "--pause-on-battery")?;
    let hot = switch(args.value("--pause-when-hot")?, "--pause-when-hot")?;
    if let Some(extra) = args.words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let home = home();
    let mut settings = crate::settings::Settings::load(&home)?;
    let changed = battery.is_some() || hot.is_some();
    if let Some(on) = battery {
        settings.pause_on_battery = on;
    }
    if let Some(on) = hot {
        settings.pause_when_hot = on;
    }
    if changed {
        settings.save(&home)?;
    }
    let sensors = crate::settings::System;
    let on_battery = crate::settings::Sensors::on_battery(&sensors);
    let is_hot = crate::settings::Sensors::hot(&sensors);
    let paused = crate::settings::pause(settings, &sensors);
    let word = |on: bool| if on { "on" } else { "off" };
    let reading = |v: Option<bool>, yes: &str, no: &str| match v {
        Some(true) => yes.to_string(),
        Some(false) => no.to_string(),
        None => "unknown here".to_string(),
    };
    emit(
        json_out,
        &json!({
            "pause_on_battery": settings.pause_on_battery,
            "pause_when_hot": settings.pause_when_hot,
            "changed": changed,
            "path": crate::settings::Settings::path(&home),
            "now": {"on_battery": on_battery, "hot": is_hot, "paused": paused},
        }),
        &format!(
            "pause on battery  {} (now {})\npause when hot    {} (now {})\n{}",
            word(settings.pause_on_battery),
            reading(on_battery, "on battery", "plugged in"),
            word(settings.pause_when_hot),
            reading(is_hot, "hot", "not hot"),
            paused.map_or_else(
                || "the pylon is not paused by these settings".to_string(),
                |p| p.words().to_string()
            ),
        ),
    );
    Ok(())
}
