//! `/pylons`' live data: one relay subscription per server, through
//! `pylon::field` (the Pylon Field's relay source, which Verse's Everglade
//! zone also reads), turned into the snapshot the page draws.
//!
//! Every event passes `nostr::pylon`'s checks in [`Live::offer`] before it
//! counts: a beacon's signature, its envelope, and its body, and a receipt's
//! the same way. A pylon is drawn only while its newest beacon is fresh
//! under `nostr::pylon::freshness` at the server's clock and says it is
//! online or draining; past `valid_until` (or 300 s after its sample) it
//! leaves the snapshot, and the page fades it out. The snapshot names only
//! what the events carry; a figure the events don't give is left out.
//!
//! Nothing connects until `/pylons` or its event stream is first asked
//! for, and only when the server was started with a relay (the binary's
//! default is `wss://relay.openagents.com`; tests start none).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use nostr::pylon::{Family, Lane, Standing};
use pylon::field::{Live, RelayField};
use pylons_view::{
    Checks, Group as GroupView, Jobs, Profile, Pylon as PylonView, Service as ServiceView, Snapshot,
};
use tokio::sync::watch;

/// The most pylons the page draws, as NIP-PYLON's world projection allows.
pub(crate) const MAX_DRAWN: usize = 256;

fn family(family: Family) -> &'static str {
    match family {
        Family::Gpu => "gpu",
        Family::Cpu => "cpu",
        Family::UnifiedMemory => "unified-memory",
    }
}

fn kind(lane: Lane) -> &'static str {
    match lane {
        Lane::CjDecision => "Decisions",
        Lane::CjConversation => "Chat",
        Lane::CjExecution => "Tasks",
        Lane::Nip90 => "Text jobs",
    }
}

fn npub(hex: &str) -> String {
    let mut key = [0u8; 32];
    if hex.len() != 64 {
        return hex.to_owned();
    }
    for (i, byte) in key.iter_mut().enumerate() {
        match u8::from_str_radix(&hex[2 * i..2 * i + 2], 16) {
            Ok(value) => *byte = value,
            Err(_) => return hex.to_owned(),
        }
    }
    nostr::nip19::encode_npub(&key)
}

/// The snapshot of `live` at `now`: every pylon whose newest verified
/// beacon is fresh and online or draining, newest sample first.
pub(crate) fn snapshot(live: &Live, now: u64) -> Snapshot {
    let activity = live.activity(now);
    let mut pylons: Vec<PylonView> = Vec::new();
    let mut more = 0;
    for pylon in live.pylons(now) {
        let status = match pylon.state.status.as_str() {
            "online" => "online",
            "draining" => "draining",
            _ => continue,
        };
        let Some(beacon) = live.beacon(&pylon.state.pylon) else {
            continue;
        };
        if pylons.len() >= MAX_DRAWN {
            more += 1;
            continue;
        }
        let jobs = live.synced.then(|| {
            let seen = activity
                .get(&pylon.state.pylon)
                .copied()
                .unwrap_or_default();
            Jobs {
                receipts: seen.receipts,
                done: seen.accepted,
                avg_ms: (seen.accepted > 0).then(|| seen.job_secs * 1000 / seen.accepted),
                last_at: seen.last_finished_at,
                last_secs: seen.last_accepted.map(|(_, secs, _)| secs),
                last_tokens: seen.last_accepted.and_then(|(_, _, tokens)| tokens),
            }
        });
        let paid = &pylon.state.paid_msat;
        let test_paid = paid
            .testnet
            .saturating_add(paid.signet)
            .saturating_add(paid.regtest);
        pylons.push(PylonView {
            id: pylon.state.pylon.clone(),
            name: beacon.label.clone(),
            slug: beacon.pylon.clone(),
            npub: npub(&beacon.provider),
            owner_npub: live.owner(&pylon.state.pylon).map(|owner| npub(&owner)),
            status: status.to_owned(),
            family: family(beacon.class.family).to_owned(),
            memory_gb: beacon.class.memory_gb,
            busy: pylon.state.busy,
            total: pylon.state.total,
            since: beacon.since,
            observed_at: beacon.observed_at,
            expires_at: beacon
                .valid_until
                .min(beacon.observed_at + nostr::pylon::MAX_VALIDITY_SECS),
            services: beacon
                .services
                .iter()
                .map(|service| ServiceView {
                    model: service.model.clone(),
                    kinds: service
                        .lanes
                        .iter()
                        .map(|lane| kind(*lane).to_owned())
                        .collect(),
                    price_hint_msat: service.price_hint_msat,
                })
                .collect(),
            free: beacon.settlement == ["free-v1"],
            pools: beacon.pools.clone(),
            jobs,
            checks: match pylon.standing {
                Standing::Unchecked => None,
                Standing::Passing => Some(Checks { passing: true }),
                Standing::Failing => Some(Checks { passing: false }),
            },
            paid_msat: (paid.bitcoin > 0).then_some(paid.bitcoin),
            test_paid_msat: (test_paid > 0).then_some(test_paid),
            group: None,
        });
    }
    for group in live.groups(now) {
        let Some(head) = &group.head else { continue };
        if !matches!(head.status.as_str(), "online" | "draining") {
            continue;
        }
        let stages: Vec<_> = group
            .stages
            .iter()
            .filter(|stage| {
                stage.current && matches!(stage.pylon.status.as_str(), "online" | "draining")
            })
            .collect();
        let parts = u32::try_from(stages.len()).unwrap_or(u32::MAX);
        let state = group.readiness.map(|readiness| {
            match readiness {
                nostr::pylon::Readiness::Ready => "ready",
                nostr::pylon::Readiness::Starting => "starting",
                nostr::pylon::Readiness::Recovering => "recovering",
            }
            .to_owned()
        });
        let mut members = vec![(
            head,
            GroupView {
                id: group.id.clone(),
                role: "head".into(),
                part: None,
                parts,
                layers: None,
                state,
                deadline_secs: group.deadline_secs,
                device: None,
                profile: live
                    .group_beacon(&head.pylon)
                    .and_then(pylon::group::answer_profile)
                    .map(|p| Profile {
                        answers: u64::try_from(p.answers).unwrap_or(u64::MAX),
                        first_token_ms: p.first_token_ms,
                        tokens_per_s: p.tokens_per_s,
                    }),
            },
        )];
        for (i, stage) in stages.iter().enumerate() {
            members.push((
                &stage.pylon,
                GroupView {
                    id: group.id.clone(),
                    role: "part".into(),
                    part: Some(u32::try_from(i + 1).unwrap_or(u32::MAX)),
                    parts,
                    layers: Some([stage.layers.from, stage.layers.to]),
                    state: None,
                    deadline_secs: None,
                    device: live
                        .group_beacon(&stage.pylon.pylon)
                        .and_then(pylon::group::stage_device)
                        .and_then(|d| d.class),
                    profile: None,
                },
            ));
        }
        for (state, view) in members {
            let Some(beacon) = live.group_beacon(&state.pylon) else {
                continue;
            };
            if pylons.len() >= MAX_DRAWN {
                more += 1;
                continue;
            }
            let jobs = live.synced.then(|| {
                let seen = activity.get(&state.pylon).copied().unwrap_or_default();
                Jobs {
                    receipts: seen.receipts,
                    done: seen.accepted,
                    avg_ms: (seen.accepted > 0).then(|| seen.job_secs * 1000 / seen.accepted),
                    last_at: seen.last_finished_at,
                    last_secs: seen.last_accepted.map(|(_, secs, _)| secs),
                    last_tokens: seen.last_accepted.and_then(|(_, _, tokens)| tokens),
                }
            });
            let status = if state.status == "draining" {
                "draining"
            } else {
                "online"
            };
            pylons.push(PylonView {
                id: state.pylon.clone(),
                name: beacon.label.clone(),
                slug: beacon.pylon.clone(),
                npub: npub(&beacon.provider),
                owner_npub: live.group_owner(&state.pylon).map(|owner| npub(&owner)),
                status: status.to_owned(),
                family: family(beacon.class.family).to_owned(),
                memory_gb: beacon.class.memory_gb,
                busy: state.busy,
                total: state.total,
                since: beacon.since,
                observed_at: beacon.observed_at,
                expires_at: beacon
                    .valid_until
                    .min(beacon.observed_at + nostr::pylon::MAX_VALIDITY_SECS),
                services: beacon
                    .services
                    .iter()
                    .map(|service| ServiceView {
                        model: service.model.clone(),
                        kinds: service
                            .lanes
                            .iter()
                            .map(|lane| kind(*lane).to_owned())
                            .collect(),
                        price_hint_msat: service.price_hint_msat,
                    })
                    .collect(),
                free: beacon.settlement == ["free-v1"],
                pools: beacon.pools.clone(),
                jobs,
                checks: None,
                paid_msat: None,
                test_paid_msat: None,
                group: Some(view),
            });
        }
    }
    Snapshot {
        now,
        synced: live.synced,
        pylons,
        more,
    }
}

/// The page's list of online pylons, as HTML: what screen readers, a
/// browser without script, and one without WebGL read. The page's script
/// keeps the same markup current.
pub(crate) fn list_html(snapshot: &Snapshot) -> String {
    let mut html = String::new();
    let count = snapshot.pylons.len() + snapshot.more;
    let heading = match count {
        0 if !snapshot.synced => "Looking for pylons".to_owned(),
        0 => "No pylons online".to_owned(),
        1 => "1 pylon online".to_owned(),
        n => format!("{n} pylons online"),
    };
    html.push_str(&format!(
        "<h2 id=\"pylons-count\">{heading}</h2><ul id=\"pylons-list\">"
    ));
    for pylon in &snapshot.pylons {
        html.push_str(&format!(
            "<li data-id=\"{}\"><span class=\"n\">{}</span> <span class=\"d\">{}</span></li>",
            escape(&pylon.id),
            escape(&pylon.name),
            escape(&pylon.summary()),
        ));
    }
    html.push_str("</ul>");
    if snapshot.more > 0 {
        html.push_str(&format!("<p>And {} more.</p>", snapshot.more));
    }
    html
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The server's one subscription and the newest snapshot of it.
pub(crate) struct Field {
    snapshots: watch::Sender<Arc<Snapshot>>,
    #[allow(dead_code)]
    stop: Arc<AtomicBool>,
}

impl Field {
    /// The newest snapshot.
    pub(crate) fn current(&self) -> Arc<Snapshot> {
        self.snapshots.borrow().clone()
    }

    /// Snapshots as they change.
    pub(crate) fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.snapshots.subscribe()
    }
}

static FIELD: OnceLock<Field> = OnceLock::new();

/// The subscription to `relay`, started the first time it is asked for.
/// Checkers' verdicts count from `OPENAGENTS_PYLON_CHECKERS` and reported
/// payments from `OPENAGENTS_PYLON_PAYMENT_ISSUERS` (both lists of keys,
/// empty by default, so by default the page shows neither).
pub(crate) fn field(relay: &str) -> &'static Field {
    FIELD.get_or_init(|| start(relay))
}

fn start(relay: &str) -> Field {
    let checkers: BTreeSet<String> = std::env::var("OPENAGENTS_PYLON_CHECKERS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|key| pylon::identity::hex_pubkey(key.trim()))
        .collect();
    let live = Arc::new(Mutex::new(
        Live::new(None)
            .trusting(checkers.clone())
            .trusting_payments(pylon::paid::trusted_issuers()),
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let source =
        RelayField::new(relay, None, pylon::identity::Identity::generate()).trusting(checkers);
    let (snapshots, _) = watch::channel(Arc::new(Snapshot::default()));
    {
        let (live, stop) = (live.clone(), stop.clone());
        let _ = std::thread::Builder::new()
            .name("pylons-field".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(source.watch(&live, &stop));
            });
    }
    {
        let (live, stop, snapshots) = (live, stop.clone(), snapshots.clone());
        let _ = std::thread::Builder::new()
            .name("pylons-snapshot".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let now = pylon::now();
                    let next = {
                        let mut live = live.lock().unwrap_or_else(PoisonError::into_inner);
                        live.settle(now);
                        snapshot(&live, now)
                    };
                    snapshots.send_if_modified(|held| {
                        if same_but_time(held, &next) {
                            false
                        } else {
                            *held = Arc::new(next);
                            true
                        }
                    });
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
    }
    Field { snapshots, stop }
}

/// Whether two snapshots differ only in their clocks.
fn same_but_time(a: &Snapshot, b: &Snapshot) -> bool {
    a.synced == b.synced && a.pylons == b.pylons && a.more == b.more
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use nostr::domain::RelaySigner;
    use nostr::pylon::{Beacon, Class, Service, Slots, Status, Tier, beacon_event};

    use super::*;

    const NOW: u64 = 1_791_400_000;

    fn signer(byte: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
    }

    fn beacon(provider: &RelaySigner, observed_at: u64, status: Status) -> Beacon {
        Beacon {
            v: nostr::pylon::BEACON_V.into(),
            requires: Vec::new(),
            meta: None,
            provider: provider.pubkey().into(),
            pylon: "studio-4080".into(),
            label: "Studio 4080".into(),
            status,
            generation: 1,
            since: observed_at - 600,
            observed_at,
            valid_until: observed_at + 240,
            class: Class {
                family: Family::Gpu,
                tier: Tier::Medium,
                memory_gb: 16,
            },
            slots: Slots { total: 2, free: 1 },
            services: vec![Service {
                capability: format!("{}:pylon/decision", provider.pubkey()),
                model: "clef-flash".into(),
                lanes: vec![Lane::CjDecision],
                offering: None,
                price_hint_msat: None,
            }],
            settlement: vec!["free-v1".into()],
            pools: vec!["everglade".into()],
            group: None,
        }
    }

    #[test]
    fn a_fresh_beacon_appears_and_leaves_when_it_expires() {
        let provider = signer(1);
        let mut live = Live::new(None);
        assert!(live.offer(
            beacon_event(&provider, &beacon(&provider, NOW, Status::Online)).unwrap(),
            NOW
        ));
        let shown = snapshot(&live, NOW + 5);
        assert_eq!(shown.pylons.len(), 1);
        let pylon = &shown.pylons[0];
        assert_eq!(pylon.name, "Studio 4080");
        assert_eq!(pylon.status, "online");
        assert_eq!((pylon.busy, pylon.total), (1, 2));
        assert_eq!(pylon.expires_at, NOW + 240);
        assert!(pylon.npub.starts_with("npub1"));
        assert!(pylon.free);
        assert_eq!(pylon.services[0].kinds, ["Decisions"]);
        // Still there at the last second it counts, gone the second after.
        assert_eq!(snapshot(&live, NOW + 240).pylons.len(), 1);
        assert!(snapshot(&live, NOW + 241).pylons.is_empty());
        // A newer beacon brings it back.
        assert!(live.offer(
            beacon_event(&provider, &beacon(&provider, NOW + 300, Status::Online)).unwrap(),
            NOW + 300
        ));
        assert_eq!(snapshot(&live, NOW + 301).pylons.len(), 1);
    }

    #[test]
    fn an_offline_beacon_takes_the_pylon_away_and_draining_stays() {
        let provider = signer(1);
        let mut live = Live::new(None);
        live.offer(
            beacon_event(&provider, &beacon(&provider, NOW, Status::Draining)).unwrap(),
            NOW,
        );
        assert_eq!(snapshot(&live, NOW).pylons[0].status, "draining");
        live.offer(
            beacon_event(&provider, &beacon(&provider, NOW + 10, Status::Offline)).unwrap(),
            NOW + 10,
        );
        assert!(snapshot(&live, NOW + 10).pylons.is_empty());
    }

    #[test]
    fn a_forged_beacon_is_never_drawn() {
        let provider = signer(1);
        let mut live = Live::new(None);
        // The content changed after signing.
        let mut tampered =
            beacon_event(&provider, &beacon(&provider, NOW, Status::Online)).unwrap();
        tampered.content = tampered.content.replace("Studio 4080", "Studio 9090");
        assert!(!live.offer(tampered, NOW));
        // Signed by one key, claiming another's pylon.
        let forger = signer(2);
        let claimed = beacon(&provider, NOW, Status::Online);
        assert!(beacon_event(&forger, &claimed).is_err());
        let mut stolen = beacon_event(&provider, &claimed).unwrap();
        stolen.sig = beacon_event(&forger, &beacon(&forger, NOW, Status::Online))
            .unwrap()
            .sig;
        assert!(!live.offer(stolen, NOW));
        assert!(snapshot(&live, NOW).pylons.is_empty());
    }

    #[test]
    fn the_snapshot_leaves_out_what_the_events_dont_give() {
        let provider = signer(1);
        let mut live = Live::new(None);
        live.offer(
            beacon_event(&provider, &beacon(&provider, NOW, Status::Online)).unwrap(),
            NOW,
        );
        let json = serde_json::to_value(snapshot(&live, NOW)).unwrap();
        let pylon = &json["pylons"][0];
        // Not caught up: no job counts. No verdicts, payments, or owner.
        for absent in [
            "jobs",
            "checks",
            "paid_msat",
            "test_paid_msat",
            "owner_npub",
        ] {
            assert!(pylon.get(absent).is_none(), "{absent}");
        }
        assert!(pylon["services"][0].get("price_hint_msat").is_none());
        assert!(json.get("more").is_none());
        live.synced = true;
        let json = serde_json::to_value(snapshot(&live, NOW)).unwrap();
        let jobs = &json["pylons"][0]["jobs"];
        assert_eq!(jobs["receipts"], 0);
        // No job, no average and no last job.
        assert!(jobs.get("avg_ms").is_none());
        assert!(jobs.get("last_at").is_none());
    }

    #[test]
    fn the_list_says_each_pylon_in_plain_words() {
        let provider = signer(1);
        let mut live = Live::new(None);
        assert!(list_html(&snapshot(&live, NOW)).contains("Looking for pylons"));
        live.synced = true;
        assert!(list_html(&snapshot(&live, NOW)).contains("No pylons online"));
        let mut named = beacon(&provider, NOW, Status::Online);
        named.label = "<b>Mine</b>".into();
        live.offer(beacon_event(&provider, &named).unwrap(), NOW);
        let html = list_html(&snapshot(&live, NOW));
        assert!(html.contains("1 pylon online"));
        assert!(html.contains("&lt;b&gt;Mine&lt;/b&gt;"));
        assert!(html.contains("Online · GPU · 1 of 2 busy"));
        assert!(oa_copy::violations(&html, &[]).is_empty());
    }

    /// A model group's head and its parts (NIP-PYLON v2) show together,
    /// head first, each saying its place; a part of an old generation and
    /// a group without a fresh head are left out (W16, #12403).
    #[test]
    fn a_model_group_shows_its_head_with_its_parts() {
        use nostr::pylon::{GROUP_BEACON_MARKER, Group, GroupRole, Layers, Readiness};
        let provider = signer(3);
        let artifact = format!("sha256:{}", "ab".repeat(32));
        let member = |slug: &str, role: GroupRole, generation: u64, layers: Option<Layers>| {
            let mut b = beacon(&provider, NOW, Status::Online);
            b.v = nostr::pylon::BEACON_V2.into();
            b.pylon = slug.into();
            b.label = format!("V4.1 {slug}");
            let head = role == GroupRole::Head;
            b.slots = if head {
                Slots { total: 1, free: 1 }
            } else {
                Slots { total: 0, free: 0 }
            };
            if head {
                b.services[0].model = format!("deepseek-v4.1-flash@{artifact}");
                b.services[0].lanes = vec![Lane::CjConversation];
            } else {
                b.services.clear();
            }
            b.group = Some(Group {
                id: "v41".into(),
                role,
                generation,
                artifact: artifact.clone(),
                readiness: head.then_some(Readiness::Ready),
                deadline_secs: head.then_some(600),
                max_output_tokens: head.then_some(4096),
                layers,
            });
            beacon_event(&provider, &b).unwrap()
        };
        let mut live = Live::new(None);
        assert!(live.offer(
            beacon_event(&provider, &beacon(&provider, NOW, Status::Online)).unwrap(),
            NOW
        ));
        let mut events = vec![member("v41-head", GroupRole::Head, 2, None)];
        for (i, from) in [0u32, 10, 20, 30].iter().enumerate() {
            events.push(member(
                &format!("v41-s{i}"),
                GroupRole::Stage,
                2,
                Some(Layers {
                    from: *from,
                    to: from + 10,
                }),
            ));
        }
        events.push(member(
            "v41-old",
            GroupRole::Stage,
            1,
            Some(Layers { from: 0, to: 10 }),
        ));
        for event in events {
            assert!(
                event
                    .tags
                    .iter()
                    .any(|t| t.0.get(1).map(String::as_str) == Some(GROUP_BEACON_MARKER))
            );
            assert!(live.offer(event, NOW));
        }
        // A stage never counts as a complete pylon.
        assert_eq!(live.pylons(NOW).len(), 1);
        let shown = snapshot(&live, NOW + 5);
        assert_eq!(shown.pylons.len(), 6);
        let head = &shown.pylons[1];
        let group = head.group.as_ref().unwrap();
        assert_eq!((group.role.as_str(), group.parts), ("head", 4));
        assert_eq!(group.state.as_deref(), Some("ready"));
        assert_eq!(
            head.summary(),
            "Online · Model group · 4 parts · 0 of 1 busy"
        );
        let parts: Vec<_> = shown.pylons[2..]
            .iter()
            .map(|p| {
                let g = p.group.as_ref().unwrap();
                assert_eq!(g.id, group.id);
                (g.part.unwrap(), g.layers.unwrap())
            })
            .collect();
        assert_eq!(
            parts,
            [(1, [0, 10]), (2, [10, 20]), (3, [20, 30]), (4, [30, 40])]
        );
        assert!(shown.pylons[0].group.is_none());
        // Without a fresh head, nothing of the group shows.
        assert_eq!(snapshot(&live, NOW + 241).pylons.len(), 0);
    }
}
