use super::*;
use crate::task::sales::Role as AccessRole;

fn now() -> u64 {
    1_800_000_000
}

fn fixture() -> (tempfile::TempDir, Store, Access, Access) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner_file = dir.path().join("owner");
    store.initialize("operator", &owner_file).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    let reader_file = dir.path().join("reader");
    store
        .issue(&owner, "reader", AccessRole::Reader, &reader_file)
        .unwrap();
    let reader = store
        .authenticate(&Store::read_credential(&reader_file).unwrap())
        .unwrap();
    (dir, store, owner, reader)
}

fn member(name: &str, role: Role, lifecycle: Lifecycle) -> Member {
    Member {
        name: name.into(),
        pubkey: format!("{:0>64}", name.len()),
        role,
        lifecycle,
    }
}

fn roster() -> Vec<Member> {
    vec![
        member("paul", Role::Leader, Lifecycle::Active),
        member("erin", Role::Hire, Lifecycle::Active),
        member("frank", Role::Hire, Lifecycle::Paused),
        member("pat", Role::Hire, Lifecycle::Retired),
    ]
}

#[test]
fn the_agora_table_matches_the_world_tree() {
    let table = Table::agora();
    assert!(table.validate(world_tree::everglade()).is_empty());
    let mut broken = table.clone();
    broken.desks[0].source = "agora:desk:99".into();
    broken.booths.needs = Affordance::Sleep;
    let problems = broken.validate(world_tree::everglade());
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_ne!(broken.digest(), table.digest());
}

#[test]
fn placement_is_deterministic_and_skips_the_retired() {
    let tree = world_tree::everglade();
    let table = Table::agora();
    let placed = place(&roster(), &table, tree);
    assert_eq!(
        placed["paul"],
        Placement::Placed {
            station: "agora:paul".into(),
            node: tree.by_source("agora:paul").unwrap().id.clone(),
        }
    );
    assert!(
        matches!(&placed["erin"], Placement::Placed { station, .. } if station == "agora:desk:0")
    );
    assert!(
        matches!(&placed["frank"], Placement::Placed { station, .. } if station == "agora:desk:1")
    );
    assert!(!placed.contains_key("pat"));
    let mut reversed = roster();
    reversed.reverse();
    assert_eq!(place(&reversed, &table, tree), placed);
    let crowd: Vec<Member> = (0..DESKS + 2)
        .map(|i| member(&format!("hire-{i:02}"), Role::Hire, Lifecycle::Active))
        .collect();
    let placed = place(&crowd, &table, tree);
    assert_eq!(
        placed
            .values()
            .filter(|p| **p == Placement::Unplaced)
            .count(),
        2
    );
}

#[test]
fn an_empty_floor_idles_and_reads_need_the_owner() {
    let (_dir, mut store, owner, reader) = fixture();
    let bodies = store.town_bodies(&owner, &roster(), &[]).unwrap();
    assert_eq!(bodies.bodies.len(), 4);
    assert!(bodies.bodies.iter().all(|b| b.idle && b.current.is_none()));
    assert!(bodies.unattributed.is_empty());
    assert_eq!(bodies.layout_digest, world_tree::everglade().digest());
    assert!(store.town_bodies(&reader, &roster(), &[]).is_err());
    let again = store.town_bodies(&owner, &roster(), &[]).unwrap();
    assert_eq!(bodies, again);
}

#[test]
fn work_cites_its_source_and_never_busies_a_paused_or_retired_body() {
    let (_dir, mut store, owner, _) = fixture();
    let pending = [
        PendingHire {
            id: "hire-1".into(),
            proposed_at: now() - 60,
        },
        PendingHire {
            id: "hire-1".into(),
            proposed_at: now() - 60,
        },
    ];
    let mut crew = roster();
    let bodies = store.town_bodies(&owner, &crew, &pending).unwrap();
    let paul = &bodies.bodies[0];
    let current = paul.current.as_ref().unwrap();
    assert_eq!(current.source, "hire:hire-1");
    assert_eq!(current.station, "agora:owner");
    assert!(
        paul.queued.is_empty(),
        "a repeated proposal is one piece of work"
    );
    crew[0].lifecycle = Lifecycle::Paused;
    let bodies = store.town_bodies(&owner, &crew, &pending).unwrap();
    assert!(bodies.bodies[0].idle && bodies.bodies[0].current.is_none());
    assert_eq!(bodies.bodies[0].queued.len(), 1);
    crew.remove(0);
    let bodies = store.town_bodies(&owner, &crew, &pending).unwrap();
    assert_eq!(bodies.unattributed.len(), 1);
}
