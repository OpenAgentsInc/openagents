//! The agent market on testnet for captures (P4): an in-process relay,
//! three free pylons that serve OpenAgents' broker key, three crew
//! members' NIP-MKT offerings on the relay, and a loop that alternates an
//! agent hiring another (a NIP-LAB order whose job the broker buys from
//! the pool, paid after acceptance) with a customer's brokered x402 job.
//! Every sat is a worthless testnet sat in memory (`TestLightning`), and
//! each settlement lands in an in-memory split ledger, printed as it
//! settles. Nothing touches a wallet or a live relay.
//!
//! ```sh
//! cargo run -p pylon --features fixture --example agent_market -- 600
//! # prints RELAY ws://127.0.0.1:PORT and BROKER <hex>; then, in another shell:
//! OPENAGENTS_PYLON_BROKERS=<hex> VERSE_PYLON_RELAY=ws://127.0.0.1:PORT \
//!   VERSE_CAPTURE_COMPUTE=live VERSE_CAPTURE_COMPUTE_WAIT=market \
//!   target/release/examples/everglade_capture agora-compute-counter.png agora
//! ```

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nostr::pylon::{Class, Family, Tier, parse_receipt};
use openagents_x402::{FileReplayStore, PaymentPayload};
use pay_ledger::Ledger;
use pylon::broker::Broker;
use pylon::client::{self, Ask, Pay};
use pylon::fixture::{Oracle, relay};
use pylon::identity::Identity;
use pylon::market::{self, Desk, Hire, Service};
use pylon::paid::{Network, TestLightning, pay};
use pylon::provider::{Config, Provider};

/// The broker's price for one job, msat.
const COMPUTE_MSAT: u64 = 10_000;
/// Where OpenAgents sells brokered jobs, in fixtures.
const SALE_URL: &str = "https://openagents.test/v1/pylon/jobs";

#[tokio::main]
async fn main() -> Result<(), String> {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let (url, _hub) = relay().await?;
    let home = std::env::temp_dir().join(format!("pylon-agent-market-{}", std::process::id()));
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let broker = Identity::generate();
    let mut pylons = Vec::new();
    for (slug, family, tier, memory_gb) in [
        ("studio-mac", Family::UnifiedMemory, Tier::Large, 64),
        ("tower-4080", Family::Gpu, Tier::Medium, 16),
        ("garage-box", Family::Cpu, Tier::Small, 8),
    ] {
        let key = Identity::generate();
        let mut config = Config::new(&url, slug, home.clone());
        config.label = slug.into();
        config.allow = Some(BTreeSet::from([broker.pubkey().to_string()]));
        config.rate_per_minute = 600;
        config.class = Class {
            family,
            tier,
            memory_gb,
        };
        let provider = Provider::new(config, key.clone(), Arc::new(Oracle))?;
        tokio::spawn(Arc::clone(&provider).run(std::future::pending()));
        pylons.push(key);
    }
    println!("RELAY {url}");
    println!("BROKER {}", broker.pubkey());

    // The crew's services, on the relay for the Agora's wall.
    let now = pylon::now();
    let mut sellers = Vec::new();
    let mut conn = pylon::relay::connect(&url, &broker, pylon::relay::LIFETIME).await?;
    for (offer, summary, sats) in [
        ("plan-review", "Victor reviews a day plan", 25),
        ("lead-brief", "Erin briefs a sales lead", 40),
        ("call-notes", "Paul summarizes a call", 15),
    ] {
        let seller = Identity::generate();
        let service = Service {
            offer: offer.into(),
            summary: summary.into(),
            price_msat: sats * 1_000,
            network: Network::Testnet,
        };
        let event = market::offering(&seller, &service, now, now + seconds + 3_600)?;
        pylon::relay::publish(&mut conn, &event).await?;
        sellers.push((seller, event, service));
    }
    let _ = conn.close().await;

    let net = Arc::new(TestLightning::new(Network::Testnet)?);
    let replay: pylon::broker::Replay =
        Arc::new(FileReplayStore::open(&home.join("replay")).map_err(|e| format!("{e:?}"))?);
    let book = Arc::new(Mutex::new(Broker::open(
        Ledger::in_memory().map_err(|e| e.to_string())?,
        Network::Testnet,
        broker.pubkey(),
        replay,
    )?));
    let alice = Identity::generate();
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut n = 0_usize;
    while std::time::Instant::now() < until {
        n += 1;
        let pylon = pylons[n % pylons.len()].pubkey().to_string();
        if n % 2 == 1 {
            let (seller, offering, service) = &sellers[(n / 2) % sellers.len()];
            hire(
                &broker, &alice, seller, offering, service, &url, &home, pylon, &net, &book,
            )
            .await?;
        } else {
            brokered(&broker, &url, &home, pylon, &net, &book, n).await?;
        }
        tokio::time::sleep(Duration::from_millis(1_500)).await;
    }
    Ok(())
}

/// Alice hires `seller` for one order: the negotiation, the job on the
/// pool, her payment after acceptance, and the settlement.
#[allow(clippy::too_many_arguments)]
async fn hire(
    broker: &Identity,
    alice: &Identity,
    seller: &Identity,
    offering: &nostr::domain::Event,
    service: &Service,
    url: &str,
    home: &std::path::Path,
    pylon: String,
    net: &Arc<TestLightning>,
    book: &Mutex<Broker>,
) -> Result<(), String> {
    let now = pylon::now();
    let hire = Hire::new(
        offering,
        alice.pubkey(),
        broker.pubkey(),
        "Say hi.",
        service.price_msat,
        Network::Testnet,
        now,
    )?;
    let mut buyer = Desk::new(alice.clone(), hire.clone())?;
    let mut desk = Desk::new(seller.clone(), hire.clone())?;
    let rfq = hire.rfq(alice, now)?;
    let (quote, terms) = hire.quote(seller, &rfq, now)?;
    let order = hire.order(alice, &rfq, &quote, now)?;
    let ack = hire.ack(seller, &quote, &order, now)?;
    for side in [&mut buyer, &mut desk] {
        side.ingest(&rfq, None, now)?;
        side.ingest(&quote, Some(&terms), now)?;
        side.ingest(&order, None, now)?;
        side.ingest(&ack, None, now)?;
    }
    let confirmed = desk.confirmed().ok_or("no order")?.clone();
    let answer = market::run(broker, &desk, url, Some(pylon), home, BTreeSet::new()).await?;
    market::accept(&answer)?;
    let receipt = answer.receipt_event.clone().ok_or("no receipt")?;
    let terms = hire.terms()?;
    let invoice = market::instruction(net.as_ref(), &confirmed, &terms)?;
    let payment = pay(net.as_ref(), &invoice, Network::Testnet, terms.price_msat)?;
    let recorded = book.lock().map_err(|e| e.to_string())?.settle_order(
        net.as_ref(),
        &receipt,
        None,
        &confirmed,
        &terms,
        &invoice.bolt11,
        &payment.preimage,
        COMPUTE_MSAT,
        pylon::now() as i64,
    )?;
    let shares: Vec<String> = recorded
        .shares
        .iter()
        .filter(|s| s.amount_msat > 0)
        .map(|s| format!("{} {} msat", s.role, s.amount_msat))
        .collect();
    println!(
        "order {}: {} hired for {} msat (TEST, testnet), receipt {}: {}",
        &confirmed.order_id[..12],
        service.offer,
        terms.price_msat,
        receipt.id,
        shares.join(", ")
    );
    Ok(())
}

/// A customer's brokered job, paid by x402 through the broker's
/// facilitator before the broker buys it.
async fn brokered(
    broker: &Identity,
    url: &str,
    home: &std::path::Path,
    pylon: String,
    net: &Arc<TestLightning>,
    book: &Mutex<Broker>,
    n: usize,
) -> Result<(), String> {
    let sale = format!("sale-{n}");
    let body = format!("{{\"sale\":\"{sale}\"}}");
    let requirements = book.lock().map_err(|e| e.to_string())?.quote(
        net.as_ref(),
        SALE_URL,
        body.as_bytes(),
        COMPUTE_MSAT,
        300,
    )?;
    let bolt11 = requirements.extra["invoice"].as_str().ok_or("no invoice")?;
    let invoice = net.lookup(bolt11).ok_or("unknown invoice")?;
    let payment = pay(net.as_ref(), &invoice, Network::Testnet, COMPUTE_MSAT)?;
    let mut proof = serde_json::Map::new();
    proof.insert("preimage".into(), payment.preimage.clone().into());
    let payload = PaymentPayload {
        x402_version: 2,
        resource: None,
        accepted: requirements.clone(),
        payload: proof,
        extensions: None,
    };
    book.lock()
        .map_err(|e| e.to_string())?
        .admit(&requirements, &payload, &sale, pylon::now())?;
    let answer = client::ask(
        broker,
        &Ask {
            relay: url.into(),
            pylon: Some(pylon),
            prompt: "Say hi.".into(),
            wait: Duration::from_secs(20),
            publish_receipt: true,
            home: home.to_path_buf(),
            checkers: BTreeSet::new(),
            pay: Some(Pay::Brokered(payment)),
        },
    )
    .await?;
    let receipt = answer.receipt_event.ok_or("no receipt")?;
    let parsed = parse_receipt(&receipt, None)?;
    book.lock().map_err(|e| e.to_string())?.settle(
        &receipt,
        None,
        COMPUTE_MSAT,
        pylon::now() as i64,
        None,
    )?;
    println!(
        "sale {n}: {} msat brokered job on {} (TEST, testnet), receipt {}",
        COMPUTE_MSAT,
        &parsed.provider[..12],
        receipt.id
    );
    Ok(())
}
