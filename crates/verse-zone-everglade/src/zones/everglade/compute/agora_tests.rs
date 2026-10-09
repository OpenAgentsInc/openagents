//! The Agora's market (P4): the counter, the services wall, and the
//! settlement threads.

use std::collections::BTreeMap;

use glam::Vec3;
use world_tree::{Family, PylonStatus, State, Tier};

use super::draw::agora;
use super::sim::Sim;
use super::*;

const NOW: u64 = 1_791_400_000;

fn remote(busy: u32) -> PylonSample {
    PylonSample {
        id: "30200:ab:studio-4080".into(),
        label: "studio-4080".into(),
        family: Family::Gpu,
        tier: Tier::Medium,
        memory_gb: 16,
        status: PylonStatus::Online,
        busy,
        total: 2,
        jobs: 12,
        uptime: Some(3_600),
        observed_at: NOW,
        owner: false,
        sigil: false,
        paid_msat: BTreeMap::new(),
        coin: None,
    }
}

/// A source the test drives by hand.
struct Fixed(Sample);

impl ComputeSource for Fixed {
    fn sample(&mut self, _: u64) -> Sample {
        self.0.clone()
    }
}

fn market() -> Market {
    Market {
        services: vec![ServiceSample {
            seller: "npub1victor".into(),
            offer: "plan-review".into(),
            summary: "Victor reviews a day plan".into(),
            price_msat: Some(25_000),
            test: true,
        }],
        jobs: 12,
        sales: 4,
        paid_msat: BTreeMap::from([("testnet".to_string(), 30_000)]),
        threads: Vec::new(),
    }
}

#[test]
fn the_counter_and_the_wall_read_the_market_and_mark_test_sats() {
    let pylons = vec![project(&remote(1), NOW)];
    let counter = agora::counter_lines(&market(), &pylons);
    assert_eq!(counter[1], "1 PYLONS  1 SLOTS FREE");
    assert_eq!(counter[2], "12 JOBS TODAY  4 SOLD");
    // Test sats are marked and never summed with mainnet.
    assert_eq!(counter[3..], ["30 SATS PAID TEST".to_string()]);
    assert_eq!(agora::wall_lines(&market())[1], "plan-review 25 SATS TEST");
    assert_eq!(
        agora::wall_lines(&Market::default())[1],
        "NOTHING OFFERED YET"
    );
}

#[test]
fn a_thread_runs_only_to_a_pylon_the_field_knows() {
    let quiet = Sample {
        pylons: vec![remote(1)],
        pool: "everglade".into(),
        market: market(),
        ..Sample::default()
    };
    let mut threaded = quiet.clone();
    threaded.market.threads = vec![ThreadSample {
        pylon: remote(1).id,
        mainnet: false,
    }];
    let [x, z] = agora::counter();
    let eye = Vec3::new(x, 4.0, z - 14.0);
    let glows = |sample: Sample| {
        let mut field = Compute::with_source(Box::new(Fixed(sample)));
        field.tick(0.0, NOW);
        field.mesh(eye, false, 0.0).glow.len()
    };
    let without = glows(quiet.clone());
    let with = glows(threaded.clone());
    assert!(with >= without + 6 * 32, "{without} -> {with}");
    let mut stray = quiet;
    stray.market.threads = vec![ThreadSample {
        pylon: "30200:cd:elsewhere".into(),
        mainnet: true,
    }];
    assert_eq!(glows(stray), without);

    // Up close, the counter and the wall read their records.
    let mut field = Compute::with_source(Box::new(Fixed(threaded)));
    field.tick(0.0, NOW);
    let read = field.inspect(agora::counter()).unwrap();
    assert!(read.starts_with("Agora compute counter"), "{read}");
    assert!(read.contains("12 JOBS TODAY  4 SOLD"));
    let wall = field.inspect(agora::wall()).unwrap();
    assert!(
        wall.contains("plan-review: Victor reviews a day plan, 25 sats (test sats)"),
        "{wall}"
    );
}

#[test]
fn the_clock_towers_dial_lights_the_pools_load() {
    use super::draw::dial;
    let (center, out, _) = dial::face().expect("the town has its clock tower");
    let well = |busy, total, verified| State::Wellspring {
        pool: "everglade".into(),
        online: 1,
        busy,
        total,
        rate: 0,
        verified,
    };
    assert_eq!(dial::load(None), None);
    assert_eq!(dial::load(Some(&well(0, 0, false))), None);
    assert_eq!(dial::load(Some(&well(1, 4, true))), Some((0.25, true)));
    // From in front of the tower, a busier pool lights more of the ring.
    let eye = center + out * 20.0;
    let lit = |busy| {
        let mut p = remote(busy);
        p.total = 2;
        let mut field = Compute::with_source(Box::new(Fixed(Sample {
            pylons: vec![p],
            pool: "everglade".into(),
            ..Sample::default()
        })));
        field.tick(0.0, NOW);
        field
            .mesh(eye, false, 0.0)
            .glow
            .iter()
            .map(|v| v.radiance[0] + v.radiance[1] + v.radiance[2])
            .sum::<f32>()
    };
    assert!(
        lit(2) > lit(1) && lit(1) > lit(0),
        "{} {} {}",
        lit(0),
        lit(1),
        lit(2)
    );
    // A dormant field leaves the dial dark.
    let mut dormant = Compute::default();
    dormant.tick(0.0, NOW);
    assert!(dormant.mesh(eye, false, 0.0).glow.is_empty());
}

#[test]
fn villagers_pass_on_the_pools_news_from_the_newest_sample() {
    assert!(Compute::default().rumor(3).is_none());
    let mut field = Compute::with_source(Box::new(Fixed(Sample {
        pylons: vec![remote(1)],
        pool: "everglade".into(),
        rate: 1,
        market: market(),
        ..Sample::default()
    })));
    field.tick(0.0, NOW);
    let news = field.rumor(3).unwrap();
    assert_eq!(news.id, "pylon-pool-news-3");
    assert_eq!(
        news.fact,
        "The Wellspring runs on 1 pylon with 1 of 2 slots busy, and 1 job finished in the last \
         minute. The Agora sold 4 jobs today."
    );
    assert!(news.quest_step().is_none());
    let mut demo = Compute::with_source(Box::new(Sim));
    demo.tick(0.0, NOW);
    assert!(demo.rumor(3).unwrap().fact.starts_with("In the demo pool"));
}

#[test]
fn the_demo_market_is_marked_and_pays_only_test_sats() {
    let s = Sim.sample(NOW);
    assert!(s.demo);
    assert!(!s.market.services.is_empty());
    assert!(s.market.services.iter().all(|x| x.test));
    assert!(!s.market.paid_msat.contains_key("bitcoin"));
    assert!(s.market.threads.iter().all(|t| !t.mainnet));
}

#[cfg(feature = "pylon-relay")]
#[test]
fn a_relay_market_maps_listings_sales_and_threads() {
    let market = pylon::field::Market {
        listings: vec![pylon::market::Listing {
            id: "ab".repeat(32),
            seller: pylon::identity::Identity::generate().pubkey().to_string(),
            offer: "plan-review".into(),
            summary: "Plan review".into(),
            price_msat: Some(25_000),
            networks: vec!["testnet".into()],
            created_at: NOW,
            valid_until: NOW + 60,
        }],
        jobs: 3,
        sales: 2,
        paid_msat: nostr::pylon::PaidMsat {
            testnet: 9_000,
            ..Default::default()
        },
        threads: vec![pylon::field::Thread {
            pylon: "30200:ab:studio-4080".into(),
            finished_at: NOW,
            mainnet: false,
        }],
    };
    let out = super::relay::market_sample(market);
    assert_eq!(out.services[0].offer, "plan-review");
    assert!(out.services[0].test);
    assert!(out.services[0].seller.starts_with("npub1"));
    assert_eq!((out.jobs, out.sales), (3, 2));
    assert_eq!(out.paid_msat.get("testnet"), Some(&9_000));
    assert!(!out.paid_msat.contains_key("bitcoin"));
    assert_eq!(out.threads[0].pylon, "30200:ab:studio-4080");
}
