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
use crate::lease::{Dedicated, Leases, Machine};
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
  whoami                    Print this computer's pylon, buyer, and aggregator keys.
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
                            (OA_ATT_RELEASE and OA_ATT_PUBLISHER also work).
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
                            x402`); its policy's ceilings also apply.
      --network NAME        The wallet's network: testnet (default) or
                            bitcoin, which needs the owner's grant.json.
  check canary --pylon NPUB Send the pylon its class's pinned Gym suite of
                            known-answer jobs as the buyer key, and sign a
                            check verdict on each receipt with the checker key.
      --award               Also award NIP-XP (pylon-check) for a passing canary.
  check redundant PROMPT --pylon NPUB --pylon NPUB [--pylon NPUB]
                            Send one prompt to several pylons and sign verdicts
                            from the majority answer.
  league                    The pylon league: per class, pass rates on the
                            pinned suites, jobs, median time, and cost.
  status                    The Pylon Field: every verified pylon's state.
      --pool SLUG           Only pylons asking to join this pool.
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
  --json                    One JSON document on standard output.

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
pub fn run_with(json_out: bool, words: &[String], wallet: &dyn Wallet) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        println!("{USAGE}");
        return 2;
    };
    if command == "--help" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("pylon: {e}");
            return 1;
        }
    };
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
        match command.as_str() {
            "whoami" => whoami(json_out),
            "link" => link(json_out, &mut args),
            "route" => crate::route::command(&args.words, &home()).map(|(value, text)| {
                emit(json_out, &value, &text);
            }),
            "serve" => serve(json_out, &mut args, &relay, wallet).await,
            "ask" => ask(json_out, &mut args, &relay, checkers, wallet).await,
            "status" => status(json_out, &mut args, &relay).await,
            "pool" => pool(json_out, &mut args, &relay, checkers).await,
            "check" => check(json_out, &mut args, &relay).await,
            "league" => league(json_out, &relay, checkers).await,
            other => Err(format!(
                "unknown command `{other}`; see `openagents pylon --help`"
            )),
        }
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            if json_out {
                println!("{}", json!({"ok": false, "error": e}));
            } else {
                eprintln!("pylon: {e}");
            }
            1
        }
    }
}

fn key(name: &str) -> Result<Identity, String> {
    Identity::load_or_create(&home().join(format!("{name}.key")))
}

fn whoami(json_out: bool) -> Result<(), String> {
    let (provider, buyer, aggregator) = (key("provider")?, key("buyer")?, key("aggregator")?);
    let checker = key("checker")?;
    emit(
        json_out,
        &json!({
            "home": home(),
            "provider": provider.npub(),
            "buyer": buyer.npub(),
            "aggregator": aggregator.npub(),
            "checker": checker.npub(),
        }),
        &format!(
            "pylon (provider): {}\nbuyer:            {}\naggregator:       {}\nchecker:          {}\nkeys in {}",
            provider.npub(),
            buyer.npub(),
            aggregator.npub(),
            checker.npub(),
            home().display()
        ),
    );
    Ok(())
}

/// This computer's name as a beacon slug.
#[must_use]
pub fn host_slug() -> String {
    let raw = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
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
    let machine: Arc<dyn Machine> = if args.flag("--dedicated") {
        Arc::new(Dedicated)
    } else {
        Arc::new(Leases::from_env()?)
    };
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
            Ok(found) => eprintln!("pylon: decisions from {} at {url}", found.advertised()),
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
    let slug = args.value("--pylon")?.unwrap_or_else(|| "att-tdx".into());
    let mut config = Config::new(relay, &slug, home());
    config.label = args
        .value("--label")?
        .unwrap_or_else(|| "Sealed Clef (Intel TDX, Confidential Space)".into());
    config.slots = u32::try_from(args.number("--slots", 1)?.clamp(1, 8)).unwrap_or(1);
    config.rate_per_minute = u32::try_from(args.number("--rate", 6)?.clamp(1, 60)).unwrap_or(6);
    config.class.family = nostr::pylon::Family::Cpu;
    config.class.tier = Tier::Small;
    config.class.memory_gb = 16;
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
    let mut psionic_child = std::process::Command::new(&psionic)
        .args(["-m"])
        .arg(&weights)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--decision-device",
            "cpu",
        ])
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
    let stop = async {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
        }
    };
    let result = provider.run(stop).await;
    refresher.abort();
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
    let mut policy = PoolPolicy::open(&slug, pool::SLICES);
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
        _ => return Err("check takes `canary` or `redundant`".into()),
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

async fn league(json_out: bool, relay: &str, checkers: BTreeSet<String>) -> Result<(), String> {
    let league = crate::league::fetch(&key("buyer")?, relay, &checkers).await?;
    emit(
        json_out,
        &serde_json::to_value(&league).map_err(|e| e.to_string())?,
        &crate::league::render(&league),
    );
    Ok(())
}
