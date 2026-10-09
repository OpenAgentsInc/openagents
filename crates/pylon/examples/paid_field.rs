//! A paid Pylon Field on testnet for captures: an in-process relay, three
//! priced pylons of different classes on fake engines, and a buyer that
//! buys each job under NIP-X402 from an in-memory test-sat wallet
//! (`TestLightning`), one job every second, round-robin. Every receipt carries a preimage that
//! hashes to its payment hash, so Verse lights each pylon's TEST coin.
//! Nothing touches a wallet or a live relay.
//!
//! ```sh
//! cargo run -p pylon --features fixture --example paid_field -- 300
//! # prints RELAY ws://127.0.0.1:PORT; then, in another shell:
//! VERSE_PYLON_RELAY=ws://127.0.0.1:PORT VERSE_CAPTURE_COMPUTE=live \
//!   VERSE_CAPTURE_COMPUTE_WAIT=coin \
//!   target/release/examples/everglade_capture pylon-field-paid-test.png pylons
//! ```

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::{Class, Family, Tier};
use pylon::client::{self, Ask, Pay};
use pylon::fixture::{Oracle, relay};
use pylon::identity::Identity;
use pylon::paid::{Network, Payer, Price, Receiver, TestLightning};
use pylon::provider::{Config, Provider};

#[tokio::main]
async fn main() -> Result<(), String> {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let (url, _hub) = relay().await?;
    let home = std::env::temp_dir().join(format!("pylon-paid-field-{}", std::process::id()));
    let net = Arc::new(TestLightning::new(Network::Testnet)?);
    let mut keys = Vec::new();
    for (slug, family, tier, memory_gb) in [
        ("studio-mac", Family::UnifiedMemory, Tier::Large, 64),
        ("tower-4080", Family::Gpu, Tier::Medium, 16),
        ("garage-box", Family::Cpu, Tier::Small, 8),
    ] {
        let key = Identity::generate();
        let mut config = Config::new(&url, slug, home.clone());
        config.label = slug.into();
        config.allow = None;
        config.rate_per_minute = 600;
        config.class = Class {
            family,
            tier,
            memory_gb,
        };
        config.price = Some(Price {
            msat: 3_000,
            network: Network::Testnet,
        });
        let provider = Provider::priced(
            config,
            key.clone(),
            Arc::new(Oracle),
            Arc::new(pylon::lease::Dedicated),
            Arc::clone(&net) as Arc<dyn Receiver>,
            None,
        )?;
        tokio::spawn(Arc::clone(&provider).run(std::future::pending()));
        keys.push(key);
    }
    println!("RELAY {url}");
    let buyer = Identity::generate();
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    let mut n = 0_usize;
    tokio::time::sleep(Duration::from_secs(2)).await;
    while std::time::Instant::now() < until {
        let key = &keys[n % keys.len()];
        n += 1;
        let answer = client::ask(
            &buyer,
            &Ask {
                relay: url.clone(),
                pylon: Some(key.pubkey().into()),
                prompt: "Say hi.".into(),
                wait: Duration::from_secs(20),
                publish_receipt: true,
                home: home.clone(),
                checkers: BTreeSet::new(),
                pay: Some(Pay::Wallet {
                    payer: Arc::clone(&net) as Arc<dyn Payer>,
                    max_msat: 5_000,
                }),
            },
        )
        .await?;
        println!(
            "job {n}: {} paid {:?} msat (TEST, testnet), receipt {:?}",
            answer.label, answer.paid_msat, answer.receipt
        );
        tokio::time::sleep(Duration::from_millis(700)).await;
    }
    Ok(())
}
