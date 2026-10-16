//! `layertree.events.v1`: logs written from real lab histories replay to
//! the same roots and sums, the checked-in fixtures replay cleanly, and
//! malformed or value-breaking logs are refused.

mod common;

use bitcoin::Amount;
use common::{MEASUREMENT, World};
use layertree_tee_lab::attest::{LogEntry, Policy, verify};
use layertree_tee_lab::events::{Body, EventError, Network, Writer, read};
use layertree_tee_lab::light::Member;
use layertree_tee_lab::rules::Op;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn member(w: &World, pos: u64) -> Member {
    let st = w.enclave.state().unwrap();
    Member {
        pos,
        leaf: st.tree.get(pos).copied(),
        path: st.tree.path(pos, 0).unwrap(),
        root: st.tree.root().unwrap(),
        outpoint: st.outpoint,
        unroll_key: st.output.unroll_key,
    }
}

fn open(w: &World, log: &mut Writer) {
    let p = w.params();
    log.push(Some(100), Body::tree_open(&p, w.enclave.state().unwrap()));
    let entry = LogEntry {
        measurement: MEASUREMENT,
        source_commit: "abc123".into(),
    };
    let policy = Policy {
        vendor_root: w.vendor.root(&w.secp),
        releases: vec![entry.clone()],
    };
    log.push(
        None,
        Body::attestation(
            &w.report,
            Some(&entry),
            verify(&w.secp, &policy, &w.report, &p),
        ),
    );
}

/// Step `op` and log it: the transition, any payout, and the member's view.
fn step(w: &mut World, log: &mut Writer, me: &mut Member, op: Op, height: u32) {
    let p = w.params();
    let before = w.enclave.state().unwrap().clone();
    match w.step(op.clone()) {
        Ok(s) => {
            log.push(
                Some(height),
                Body::transition(&op, &before, &s.record, &s.tx.compute_txid()),
            );
            if let Op::Exit { pos, .. } = op {
                let leaf = before.tree.get(pos).unwrap();
                log.push(Some(height), Body::payout(&leaf.exit_key, leaf.amount));
            }
            let seen = me.follow(&w.secp, &p, &s.tx, &s.record);
            log.push(Some(height), Body::light_client(me.pos, &seen));
        }
        Err(r) => log.push(None, Body::rejected(&op, &r)),
    }
}

fn happy() -> String {
    let mut w = World::new(6, 128, None);
    let mut log = Writer::new(Network::Lab);
    open(&w, &mut log);
    let mut me = member(&w, 4);
    let src = w.leaf(4);
    let mut claim = w.member_leaf(30, 2_000);
    claim.evictor_key = src.evictor_key;
    claim.evict_after = src.evict_after;
    let voucher = w.voucher(&src, &claim);
    let joiner = w.member_leaf(31, 9_000);
    let ops = vec![
        Op::Exit {
            pos: 1,
            evict: false,
        },
        Op::Join {
            height: 2,
            index: 5,
            leaves: vec![(1, joiner)],
        },
        Op::Claim {
            source_pos: 4,
            claim_pos: 70,
            claim_leaf: claim,
            voucher_sig: voucher,
        },
        // Refused: position 1 is empty now.
        Op::Exit {
            pos: 1,
            evict: true,
        },
        Op::Refresh,
        Op::Exit {
            pos: 5,
            evict: false,
        },
    ];
    for (i, op) in ops.into_iter().enumerate() {
        if matches!(
            op,
            Op::Exit {
                pos: 1,
                evict: true
            }
        ) {
            let p = w.params();
            let st = w.enclave.state().unwrap();
            let r = layertree_tee_lab::rules::effects(&w.secp, &p, st, &op).unwrap_err();
            log.push(None, Body::rejected(&op, &r));
            continue;
        }
        step(&mut w, &mut log, &mut me, op, 101 + i as u32);
    }
    log.to_jsonl()
}

fn theft() -> String {
    let mut w = World::new(5, 128, None);
    let p = w.params();
    let mut log = Writer::new(Network::Lab);
    open(&w, &mut log);
    let mut me = member(&w, 0);
    w.enclave.break_for_test();
    let op = Op::Exit {
        pos: 3,
        evict: false,
    };
    let before = w.enclave.state().unwrap().clone();
    let (mut tx, prevouts, sigs) = w.build(&op);
    tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() - 50_000);
    let s = w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap();
    // The record is honest; the chain output is not. The log keeps the
    // record (what the enclave claimed) and the light client's alarm.
    log.push(
        Some(101),
        Body::transition(&op, &before, &s.record, &s.tx.compute_txid()),
    );
    let leaf = before.tree.get(3).unwrap();
    log.push(Some(101), Body::payout(&leaf.exit_key, leaf.amount));
    let seen = me.follow(&w.secp, &p, &s.tx, &s.record);
    assert!(seen.is_err());
    log.push(Some(101), Body::light_client(0, &seen));
    log.to_jsonl()
}

fn unroll() -> String {
    let mut w = World::new(12, 4, None);
    let mut log = Writer::new(Network::Lab);
    open(&w, &mut log);
    let mut me = member(&w, 2);
    step(
        &mut w,
        &mut log,
        &mut me,
        Op::Exit {
            pos: 7,
            evict: false,
        },
        101,
    );
    let pkg = w.last_unroll.clone().unwrap();
    let st = w.enclave.state().unwrap().clone();
    // The enclave dies; the pre-signed unroll runs after Δ blocks.
    drop(w);
    log.push(Some(102), Body::EnclaveDown {});
    let mut level: BTreeMap<bitcoin::Txid, u32> = BTreeMap::new();
    let start = 102 + u32::from(common::DELAY);
    for tx in &pkg.txs {
        let parent = tx.input[0].previous_output.txid;
        let l = level.get(&parent).map_or(0, |p| p + 1);
        level.insert(tx.compute_txid(), l);
        let txid = tx.compute_txid().to_string();
        log.push(
            Some(start + l),
            Body::UnrollBroadcast {
                level: l,
                txid: txid.clone(),
                cpfp_fee: 1_000,
            },
        );
        log.push(
            Some(start + l + 1),
            Body::UnrollConfirmed { level: l, txid },
        );
    }
    let last = start + level.values().max().copied().unwrap_or(0) + 1;
    for leaf in st.tree.leaves().values() {
        log.push(Some(last), Body::payout(&leaf.exit_key, leaf.amount));
    }
    log.to_jsonl()
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/events")
}

/// `LAYERTREE_WRITE_EVENTS=1` regenerates the checked-in fixtures.
#[test]
fn fresh_logs_replay_and_fixtures_replay_cleanly() {
    let fresh = [
        ("happy.jsonl", happy()),
        ("theft-caught.jsonl", theft()),
        ("unroll.jsonl", unroll()),
    ];
    for (name, text) in &fresh {
        let replay = read(text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(!replay.roots.is_empty());
        if std::env::var_os("LAYERTREE_WRITE_EVENTS").is_some() {
            std::fs::create_dir_all(fixtures()).unwrap();
            std::fs::write(fixtures().join(name), text).unwrap();
        }
    }
    for (name, _) in &fresh {
        let text = std::fs::read_to_string(fixtures().join(name))
            .unwrap_or_else(|_| panic!("missing fixture {name}"));
        let replay = read(&text).unwrap_or_else(|e| panic!("fixture {name}: {e:?}"));
        match *name {
            "happy.jsonl" => {
                assert_eq!(replay.roots.len(), 6, "open plus five transitions");
                assert!(text.contains("\"kind\":\"rejected\""));
            }
            "theft-caught.jsonl" => assert!(text.contains("\"result\":\"OutputMismatch\"")),
            _ => {
                assert!(replay.enclave_down);
                assert_eq!(
                    replay.paid, replay.roots[0].sum,
                    "every sat paid out: the exit, then the unroll"
                );
            }
        }
    }
}

#[test]
fn every_kind_has_an_example_in_the_readme() {
    let readme = include_str!("../README.md");
    for kind in [
        "tree_open",
        "transition",
        "rejected",
        "attestation",
        "enclave_down",
        "block",
        "unroll_broadcast",
        "unroll_confirmed",
        "payout",
        "light_client",
    ] {
        assert!(
            readme.contains(&format!("\"kind\":\"{kind}\"")),
            "README lacks an example of {kind}"
        );
    }
    // And each README example line parses as an event of its kind.
    for line in readme.lines().filter(|l| l.starts_with("{\"seq\"")) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "{line}"
        );
    }
}

#[test]
fn malformed_and_value_breaking_logs_are_refused() {
    let text = happy();
    let lines: Vec<&str> = text.lines().collect();
    let join = |v: &[String]| v.join("\n");
    let owned: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    // An unknown kind, an unknown field, a seq gap, another network.
    let mut v = owned.clone();
    v[1] = v[1].replace("\"kind\":\"attestation\"", "\"kind\":\"mystery\"");
    assert!(matches!(read(&join(&v)), Err(EventError::Malformed(1, _))));
    let mut v = owned.clone();
    v[1] = v[1].replacen('{', "{\"extra\":1,", 1);
    assert!(matches!(read(&join(&v)), Err(EventError::Malformed(1, _))));
    let mut v = owned.clone();
    v.remove(1);
    assert!(matches!(read(&join(&v)), Err(EventError::Sequence(1))));
    let mut v = owned.clone();
    v[1] = v[1].replace("\"network\":\"lab\"", "\"network\":\"signet\"");
    assert!(matches!(read(&join(&v)), Err(EventError::Network(1))));
    // A transition whose new root lies.
    let edit = |v: &mut Vec<String>, i: usize, f: &dyn Fn(&mut serde_json::Value)| {
        let mut value: serde_json::Value = serde_json::from_str(&v[i]).unwrap();
        f(&mut value);
        v[i] = value.to_string();
    };
    let t = owned
        .iter()
        .position(|l| l.contains("\"kind\":\"transition\""))
        .unwrap();
    let mut v = owned.clone();
    edit(&mut v, t, &|x| {
        x["new_root"]["sum"] = (x["new_root"]["sum"].as_u64().unwrap() + 1).into()
    });
    assert!(matches!(read(&join(&v)), Err(EventError::Replay(_, _))));
    // A path that does not lead to the root.
    let mut v = owned.clone();
    edit(&mut v, t, &|x| {
        x["changes"][0]["path"][0]["right"] =
            (!x["changes"][0]["path"][0]["right"].as_bool().unwrap()).into()
    });
    assert!(matches!(read(&join(&v)), Err(EventError::Replay(_, _))));
    // A payout nobody is owed.
    let mut v = owned.clone();
    let p = v
        .iter()
        .position(|l| l.contains("\"kind\":\"payout\""))
        .unwrap();
    edit(&mut v, p, &|x| {
        x["amount"] = (x["amount"].as_u64().unwrap() + 1).into()
    });
    assert!(matches!(
        read(&join(&v)),
        Err(EventError::Conservation(_, _))
    ));
    // A transition before the tree is open.
    assert!(matches!(
        read(&join(&owned[2..])),
        Err(EventError::Sequence(0)) | Err(EventError::Open(0))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]
    /// Random histories of joins, exits, refreshes and claims → log →
    /// reader gives back the same roots and sums.
    #[test]
    fn random_histories_round_trip(steps in prop::collection::vec((0u8..4, 0u64..32, 1u64..50), 1..10)) {
        let mut w = World::new(6, 128, None);
        let mut log = Writer::new(Network::Regtest);
        open(&w, &mut log);
        let mut roots = vec![w.enclave.state().unwrap().tree.root().unwrap()];
        let mut fresh = 40u32;
        for (i, (kind, pos, amount)) in steps.into_iter().enumerate() {
            let st = w.enclave.state().unwrap().clone();
            let occupied: Vec<u64> = st.tree.leaves().keys().copied().collect();
            let op = match kind {
                0 if !occupied.is_empty() && occupied.len() > 1 => Op::Exit { pos: occupied[(pos as usize) % occupied.len()], evict: false },
                1 => {
                    fresh += 1;
                    let free = (0..st.tree.capacity()).find(|p| st.tree.get(*p).is_none()).unwrap();
                    let leaf = w.member_leaf(fresh, 1_000 * amount);
                    Op::Join { height: 0, index: free, leaves: vec![(0, leaf)] }
                }
                _ => Op::Refresh,
            };
            let before = st;
            if let Ok(s) = w.step(op.clone()) {
                log.push(Some(100 + i as u32), Body::transition(&op, &before, &s.record, &s.tx.compute_txid()));
                if let Op::Exit { pos, .. } = op {
                    let leaf = before.tree.get(pos).unwrap();
                    log.push(Some(100 + i as u32), Body::payout(&leaf.exit_key, leaf.amount));
                }
                roots.push(w.enclave.state().map_or(layertree_tee_lab::tree::Node::EMPTY, |s| s.tree.root().unwrap()));
            }
        }
        let replay = read(&log.to_jsonl()).unwrap();
        prop_assert_eq!(replay.roots, roots);
    }
}

/// The logs `tests/regtest.rs` wrote on a real bitcoind (#11781): they
/// replay, every height is a block height, the enclave went down, and
/// every sat that entered the tree was paid out (an exit, then the unroll).
#[test]
fn regtest_fixtures_replay_cleanly() {
    for (name, levels) in [("regtest.jsonl", 1), ("regtest-two-level.jsonl", 2)] {
        let text = std::fs::read_to_string(fixtures().join(name))
            .unwrap_or_else(|_| panic!("missing fixture {name}"));
        let replay = read(&text).unwrap_or_else(|e| panic!("fixture {name}: {e:?}"));
        assert!(replay.events.iter().all(|e| e.network == Network::Regtest));
        assert!(replay.enclave_down);
        let entered: u64 = replay.roots[0].sum
            + replay
                .events
                .iter()
                .filter_map(|e| match &e.body {
                    Body::Transition {
                        op: layertree_tee_lab::events::OpKind::Join,
                        old_root,
                        new_root,
                        ..
                    } => Some(new_root.sum - old_root.sum),
                    _ => None,
                })
                .sum::<u64>();
        assert_eq!(replay.paid, entered, "{name}: every sat paid");
        let confirmed: Vec<u32> = replay
            .events
            .iter()
            .filter_map(|e| match &e.body {
                Body::UnrollConfirmed { level, .. } => Some(*level),
                _ => None,
            })
            .collect();
        assert_eq!(confirmed.iter().max().map(|l| l + 1), Some(levels));
        // The unroll's first transaction confirmed Δ blocks after the last
        // tree output.
        let down = replay
            .events
            .iter()
            .find(|e| matches!(e.body, Body::EnclaveDown {}))
            .and_then(|e| e.height)
            .unwrap();
        let first = replay
            .events
            .iter()
            .find(|e| matches!(e.body, Body::UnrollConfirmed { level: 0, .. }))
            .and_then(|e| e.height)
            .unwrap();
        let Body::TreeOpen { unroll_delay, .. } = &replay.events[0].body else {
            panic!("{name}: first line opens the tree");
        };
        assert_eq!(first - down, u32::from(*unroll_delay));
    }
}

/// `measured` lines (#11785) replay as no-ops, and only known units pass.
#[test]
fn measured_lines_need_a_known_unit() {
    let text = std::fs::read_to_string(fixtures().join("happy.jsonl")).unwrap();
    let n = text.lines().filter(|l| !l.trim().is_empty()).count();
    let line = |unit: &str| {
        format!(
            "{text}{{\"seq\":{n},\"height\":101,\"network\":\"lab\",\"kind\":\"measured\",\"what\":\"refresh_vsize\",\"value\":255,\"unit\":\"{unit}\",\"level\":null}}\n"
        )
    };
    assert!(read(&line("vB")).is_ok());
    assert_eq!(
        read(&line("furlongs")).unwrap_err(),
        EventError::Field("unit")
    );
}
