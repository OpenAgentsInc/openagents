//! The `livehost` command: run a host, deploy, roll back, and benchmark.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use livehost::deploy::{self, Target};
use livehost::host::{self, HostConfig};
use livehost::store::Store;
use livehost::{bench, keys, load};

#[derive(Parser)]
#[command(
    name = "livehost",
    about = "Web deploys in seconds: a prototype host and deploy tool."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a deploy key and print its public key.
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Print the public key of a deploy key.
    Pubkey {
        #[arg(long)]
        key: PathBuf,
    },
    /// Serve the tenants in a host config, following their pointers.
    Serve {
        /// JSON: {"tenants": [{"name", "pubkey", "channel", "hosts"}]}.
        #[arg(long)]
        config: PathBuf,
        /// A directory, or gs://BUCKET/PREFIX.
        #[arg(long)]
        store: String,
        #[arg(long)]
        relay: String,
        #[arg(long, default_value = "127.0.0.1:8090")]
        listen: SocketAddr,
    },
    /// Upload what changed in a tenant's directory, build changed guests,
    /// and move the channel's pointer.
    Deploy {
        /// Every file under it becomes an object by path; an optional
        /// site.json adds routes and guest builds.
        dir: PathBuf,
        /// Required unless site.json names it.
        #[arg(long)]
        tenant: Option<String>,
        /// A JSON file for the manifest's `meta`.
        #[arg(long)]
        meta: Option<PathBuf>,
        #[arg(long)]
        store: String,
        #[arg(long)]
        relay: String,
        #[arg(long)]
        key: PathBuf,
        #[arg(long, default_value = "staging")]
        channel: String,
        /// Rebuild every guest and publish even when nothing changed.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Point a channel back at the previous manifest, or at --to.
    Rollback {
        #[arg(long)]
        tenant: String,
        #[arg(long)]
        store: String,
        #[arg(long)]
        relay: String,
        #[arg(long)]
        key: PathBuf,
        #[arg(long, default_value = "staging")]
        channel: String,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print the manifest a tenant's channel points at.
    Status {
        #[arg(long)]
        tenant: String,
        #[arg(long)]
        store: String,
        #[arg(long)]
        relay: String,
        #[arg(long)]
        key: PathBuf,
        #[arg(long, default_value = "staging")]
        channel: String,
    },
    /// Send load to a host and report requests per second and latency.
    Load {
        #[arg(long)]
        url: Vec<String>,
        #[arg(long, default_value_t = 32)]
        concurrency: usize,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
    /// Time edits from file to host to browser. Run under
    /// `openagents browser run` to include the browser.
    Bench {
        #[arg(long, default_value = "127.0.0.1:8090")]
        host: String,
        #[arg(long)]
        store: String,
        #[arg(long)]
        relay: String,
        #[arg(long, default_value = "staging")]
        channel: String,
        #[arg(long)]
        site: PathBuf,
        #[arg(long)]
        site_key: PathBuf,
        #[arg(long)]
        game: Option<PathBuf>,
        #[arg(long)]
        game_key: Option<PathBuf>,
        #[arg(long, default_value = "game.localhost")]
        game_host: String,
        #[arg(long, default_value_t = 5)]
        runs: usize,
        #[arg(
            long,
            value_delimiter = ',',
            default_value = "css,copy,template,module,handler,game-asset,rollback"
        )]
        cases: Vec<String>,
        /// Concurrency and seconds for a load test during swaps, as C,S.
        #[arg(long, value_delimiter = ',')]
        load: Vec<u64>,
        /// Where to write every run as JSON.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Keygen { out } => println!("{}", keys::generate(&out)?),
        Command::Pubkey { key } => println!("{}", keys::load(&key)?.pubkey()),
        Command::Serve {
            config,
            store,
            relay,
            listen,
        } => {
            let config: HostConfig = serde_json::from_slice(
                &std::fs::read(&config).with_context(|| format!("reading {}", config.display()))?,
            )?;
            host::serve(config, Store::parse(&store)?, relay, listen).await?;
        }
        Command::Deploy {
            dir,
            tenant,
            meta,
            store,
            relay,
            key,
            channel,
            force,
            json,
        } => {
            let (store, signer) = (Store::parse(&store)?, keys::load(&key)?);
            let target = Target {
                store: &store,
                relay: &relay,
                signer: &signer,
                channel: &channel,
            };
            let meta = match meta {
                Some(path) => Some(serde_json::from_slice(&std::fs::read(&path)?)?),
                None => None,
            };
            let options = deploy::Options {
                tenant,
                force,
                meta,
                ..Default::default()
            };
            let report = deploy::deploy(&dir, &target, &options).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print();
            }
        }
        Command::Rollback {
            tenant,
            store,
            relay,
            key,
            channel,
            to,
            json,
        } => {
            let (store, signer) = (Store::parse(&store)?, keys::load(&key)?);
            let target = Target {
                store: &store,
                relay: &relay,
                signer: &signer,
                channel: &channel,
            };
            let report = deploy::rollback(&tenant, &target, to.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print();
            }
        }
        Command::Status {
            tenant,
            store,
            relay,
            key,
            channel,
        } => {
            let (store, signer) = (Store::parse(&store)?, keys::load(&key)?);
            let mut relay = livehost::relay::Relay::connect(&relay).await?;
            match deploy::current(&mut relay, &store, &signer, &tenant, &channel).await? {
                Some((pointer, manifest)) => println!(
                    "{tenant}/{channel}: {} ({} objects, parent {}), pointer {} at {}",
                    pointer.manifest,
                    manifest.objects.len(),
                    manifest.parent.as_deref().unwrap_or("none"),
                    pointer.id,
                    pointer.created_at
                ),
                None => println!("{tenant}/{channel}: no deploy"),
            }
        }
        Command::Load {
            url,
            concurrency,
            seconds,
        } => {
            let targets = url
                .into_iter()
                .map(|url| load::LoadTarget { url, host: None })
                .collect();
            let report = load::run(targets, concurrency, Duration::from_secs(seconds)).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Bench {
            host,
            store,
            relay,
            channel,
            site,
            site_key,
            game,
            game_key,
            game_host,
            runs,
            cases,
            load,
            out,
        } => {
            let game = match (game, game_key) {
                (Some(game), Some(key)) => Some((game, keys::load(&key)?)),
                (None, None) => None,
                _ => anyhow::bail!("--game and --game-key go together"),
            };
            let cdp_port = std::env::var("OPENAGENTS_CHROME_PORT")
                .ok()
                .and_then(|port| port.parse().ok());
            let bench = bench::Bench {
                host,
                store: Store::parse(&store)?,
                relay,
                channel,
                site,
                site_key: keys::load(&site_key)?,
                game,
                game_host,
                runs,
                cases,
                cdp_port,
                load: match load.as_slice() {
                    [concurrency, seconds] => Some((*concurrency as usize, *seconds)),
                    [] => None,
                    _ => anyhow::bail!("--load takes CONCURRENCY,SECONDS"),
                },
            };
            let results = bench.run().await?;
            if let Some(out) = out {
                std::fs::write(&out, serde_json::to_vec_pretty(&results)?)?;
            }
            println!("{}", bench::summary(&results));
        }
    }
    Ok(())
}
