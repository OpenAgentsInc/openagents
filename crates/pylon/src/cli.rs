//! `openagents pylon` and the standalone `pylon` binary share this
//! dispatcher. Every command prints one JSON document with `--json`.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::{PoolPolicy, Tier};
use serde_json::{Value, json};

use crate::client::{self, Ask};
use crate::engine::Psionic;
use crate::field::RelayField;
use crate::identity::{
    Identity, check_owner, hex_pubkey, load_owner, mint_owner, npub, parse_owner, save_owner,
};
use crate::lease::{Dedicated, Leases, Machine};
use crate::pool;
use crate::provider::{Config, Provider};
use crate::{DEFAULT_RELAY, home};

/// Usage text.
pub const USAGE: &str = "\
Usage: openagents pylon <command> [options]

Share this computer's model as a NIP-PYLON pylon, or use one. Free jobs
only; every job is NIP-44 encrypted to the pylon.

Commands:
  whoami                    Print this computer's pylon, buyer, and aggregator keys.
  serve                     Publish beacons and answer jobs from a local Psionic server.
      --engine URL          The Psionic server (default http://127.0.0.1:18080).
      --model NAME          The served model (default qwen3.5-0.8b-q8_0).
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
  link                      Show the owner's NIP-OA link on this pylon's beacons.
      --owner-secret FILE   Mint the link with the owner key in FILE (hex or
                            nsec); the key is read once, never stored.
      --credential JSON     Store a link minted elsewhere: the auth tag
                            [\"auth\", owner, \"kind=30200\", signature].
      --remove              Remove the link.
  route on|off|status       Send Alice's and the crew's low-risk text jobs (day
                            plans) to the pool, falling back to their own model.
      --pylon NPUB          Only this pylon (default: the best fresh one).
      --wait SECS           How long a job waits (default 90).
  ask PROMPT                Find a pylon, run one job, and publish a receipt.
      --pylon NPUB          Use this pylon instead of the best fresh one.
      --wait SECS           How long to wait for the answer (default 90).
      --no-receipt          Do not publish a receipt.
  status                    The Pylon Field: every verified pylon's state.
      --pool SLUG           Only pylons asking to join this pool.
  pool                      Compute a pool aggregate over the last window.
      --pool SLUG           The pool (default everglade).
      --minutes N           Window length, 1 to 60 (default 60).
      --publish             Sign and publish it as the aggregator.
  pool verify --aggregator NPUB
                            Fetch that aggregator's newest aggregate and recompute it.

Common options:
  --relay URL               The relay (default wss://relay.openagents.com).
  --json                    One JSON document on standard output.

Keys live in ~/.openagents/compute (OPENAGENTS_PYLON_HOME overrides).";

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
pub fn run(json_out: bool, words: &[String]) -> u8 {
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
        match command.as_str() {
            "whoami" => whoami(json_out),
            "link" => link(json_out, &mut args),
            "route" => crate::route::command(&args.words, &home()).map(|(value, text)| {
                emit(json_out, &value, &text);
            }),
            "serve" => serve(json_out, &mut args, &relay).await,
            "ask" => ask(json_out, &mut args, &relay).await,
            "status" => status(json_out, &mut args, &relay).await,
            "pool" => pool(json_out, &mut args, &relay).await,
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
    emit(
        json_out,
        &json!({
            "home": home(),
            "provider": provider.npub(),
            "buyer": buyer.npub(),
            "aggregator": aggregator.npub(),
        }),
        &format!(
            "pylon (provider): {}\nbuyer:            {}\naggregator:       {}\nkeys in {}",
            provider.npub(),
            buyer.npub(),
            aggregator.npub(),
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

async fn serve(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let engine_url = args
        .value("--engine")?
        .unwrap_or_else(|| "http://127.0.0.1:18080".into());
    let model = args
        .value("--model")?
        .unwrap_or_else(|| "qwen3.5-0.8b-q8_0".into());
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
    let provider = Provider::on(config.clone(), identity.clone(), engine, machine)?;
    emit(
        json_out,
        &json!({
            "serving": true,
            "pylon": format!("30200:{}:{}", identity.pubkey(), slug),
            "npub": identity.npub(),
            "relay": relay,
            "engine": engine_url,
            "model": model,
            "allow": config.allow.as_ref().map(|a| a.iter().map(|k| npub(k)).collect::<Vec<_>>()),
        }),
        &format!(
            "pylon {} serving {model} from {engine_url} on {relay}\nnpub {}\nstop with Ctrl-C or SIGTERM; an offline beacon goes out on the way down",
            slug,
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

async fn ask(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let pylon = match args.value("--pylon")? {
        Some(p) => Some(hex_pubkey(&p).ok_or("--pylon is not a key")?),
        None => None,
    };
    let wait = Duration::from_secs(args.number("--wait", 90)?.clamp(1, 600));
    let publish_receipt = !args.flag("--no-receipt");
    let prompt = args.words.join(" ");
    if prompt.trim().is_empty() {
        return Err("ask needs a prompt".into());
    }
    let buyer = key("buyer")?;
    let answer = client::ask(
        &buyer,
        &Ask {
            relay: relay.into(),
            pylon,
            prompt,
            wait,
            publish_receipt,
            home: home(),
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

async fn pool(json_out: bool, args: &mut Args, relay: &str) -> Result<(), String> {
    let verify = args.words.first().is_some_and(|w| w == "verify");
    if verify {
        args.words.remove(0);
    }
    let slug = args.value("--pool")?.unwrap_or_else(|| "everglade".into());
    let policy = PoolPolicy::open(&slug, pool::SLICES);
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
