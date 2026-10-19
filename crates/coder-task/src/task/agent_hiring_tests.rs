//! REV-64 fixtures: proposal replay, concurrent caps, stale digests,
//! non-owner refusal, and retirement handing leads back to Paul.
use super::super::agent_hiring::{self as hiring, Status};
use super::*;
use coder_host::access::crew::{
    Evidence, HIRE_SCHEMA, HireAction, HireDecision, HireProposal, HireVerdict, JobRole,
};

fn clock() -> u64 {
    1_791_158_400
}
fn owner() -> Principal {
    Principal {
        device: "owner".into(),
        grant: None,
        epoch: None,
    }
}
fn granted() -> Principal {
    Principal {
        device: "phone".into(),
        grant: Some("grant".into()),
        epoch: Some(1),
    }
}
fn owner_key() -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([7_u8; 32]).unwrap()
}
fn host(dir: &tempfile::TempDir) -> (Agents, std::path::PathBuf) {
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let agents = Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        BTreeMap::new(),
    )
    .with_clock(clock)
    .with_coder_state(dir.path().join("coder"));
    agents
        .create_crew("paul", &workspace, JobRole::SalesLead, Some(&owner_key()))
        .unwrap();
    (agents, workspace)
}
fn hire(id: &str, name: &str, role: JobRole, budget: u64) -> HireProposal {
    HireProposal {
        schema: HIRE_SCHEMA.into(),
        id: id.into(),
        action: HireAction::Hire {
            name: name.into(),
            role,
        },
        daily_usd_millionths: budget,
        charter_revision: 1,
        reason: "The research queue is three days deep.".into(),
        evidence: vec![Evidence {
            reference: "queue:research".into(),
            sha256: "ab".repeat(32),
        }],
        expires_at: clock() + 3600,
    }
}
fn retire(id: &str, name: &str) -> HireProposal {
    HireProposal {
        schema: HIRE_SCHEMA.into(),
        id: id.into(),
        action: HireAction::Retire { name: name.into() },
        daily_usd_millionths: 0,
        charter_revision: 0,
        reason: "The queue emptied.".into(),
        evidence: Vec::new(),
        expires_at: clock() + 3600,
    }
}
fn decision(entry: &serde_json::Value, verdict: HireVerdict) -> HireDecision {
    HireDecision {
        proposal: entry["proposal"]["id"].as_str().unwrap().into(),
        expected_sha256: entry["sha256"].as_str().unwrap().into(),
        verdict,
        reason: "Owner decision.".into(),
    }
}

#[test]
fn confirmation_creates_once_and_replays() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, workspace) = host(&dir);
    let entry = agents
        .propose_hire(
            &owner(),
            &hire("h1", "erin", JobRole::SalesResearcher, 1_000_000),
        )
        .unwrap();
    assert_eq!(entry["status"], "pending");
    // Proposing the identical document again is a no-op; a changed one is refused.
    assert_eq!(
        agents
            .propose_hire(
                &owner(),
                &hire("h1", "erin", JobRole::SalesResearcher, 1_000_000)
            )
            .unwrap(),
        entry
    );
    assert!(
        agents
            .propose_hire(
                &owner(),
                &hire("h1", "erin", JobRole::SalesResearcher, 2_000_000)
            )
            .is_err()
    );
    let d = decision(&entry, HireVerdict::Confirm);
    let first = agents
        .decide_hire(&owner(), &d, workspace.to_str(), Some(&owner_key()))
        .unwrap();
    assert_eq!(first["status"], "confirmed");
    assert_eq!(first["decision"]["outcome"]["certification"], "training");
    let (_, record) = agents.store("erin").unwrap();
    assert_eq!(record.job_role, Some(JobRole::SalesResearcher));
    let pubkey = record.pubkey.clone();
    // Replay: same entry, no second identity, the opposite verdict is refused.
    let again = agents
        .decide_hire(&owner(), &d, workspace.to_str(), Some(&owner_key()))
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(agents.store("erin").unwrap().1.pubkey, pubkey);
    assert!(
        agents
            .decide_hire(
                &owner(),
                &decision(&entry, HireVerdict::Reject),
                workspace.to_str(),
                Some(&owner_key()),
            )
            .is_err()
    );
}

#[test]
fn stale_digest_grant_and_paul_cannot_decide() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, workspace) = host(&dir);
    let entry = agents
        .propose_hire(
            &owner(),
            &hire("h1", "erin", JobRole::SalesResearcher, 1_000_000),
        )
        .unwrap();
    let mut stale = decision(&entry, HireVerdict::Confirm);
    stale.expected_sha256 = "cd".repeat(32);
    assert!(
        agents
            .decide_hire(&owner(), &stale, workspace.to_str(), Some(&owner_key()))
            .is_err()
    );
    let d = decision(&entry, HireVerdict::Confirm);
    // A granted device, and a call without the owner key, are refused.
    assert!(
        agents
            .decide_hire(&granted(), &d, workspace.to_str(), Some(&owner_key()))
            .is_err()
    );
    assert!(
        agents
            .decide_hire(&owner(), &d, workspace.to_str(), None)
            .is_err()
    );
    assert!(
        agents
            .propose_hire(&granted(), &retire("r1", "paul"))
            .is_err()
    );
    // Paul's own binding cannot be proposed for retirement here.
    assert!(
        agents
            .propose_hire(&owner(), &retire("r1", "paul"))
            .is_err()
    );
    assert!(agents.store("erin").is_err());
}

#[test]
fn two_confirmations_cannot_take_the_last_slot_or_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, workspace) = host(&dir);
    let mut entries = Vec::new();
    for (id, name) in [("h1", "erin"), ("h2", "frank")] {
        let e = agents
            .propose_hire(
                &owner(),
                &hire(id, name, JobRole::SalesResearcher, 1_000_000),
            )
            .unwrap();
        agents
            .decide_hire(
                &owner(),
                &decision(&e, HireVerdict::Confirm),
                workspace.to_str(),
                Some(&owner_key()),
            )
            .unwrap();
    }
    // Two pending proposals for the one remaining slot.
    for (id, name) in [("h3", "pat"), ("h4", "arthur")] {
        entries.push(
            agents
                .propose_hire(&owner(), &hire(id, name, JobRole::SalesDemo, 1_000_000))
                .unwrap(),
        );
    }
    let ok = agents.decide_hire(
        &owner(),
        &decision(&entries[0], HireVerdict::Confirm),
        workspace.to_str(),
        Some(&owner_key()),
    );
    assert!(ok.is_ok());
    let over = agents.decide_hire(
        &owner(),
        &decision(&entries[1], HireVerdict::Confirm),
        workspace.to_str(),
        Some(&owner_key()),
    );
    assert!(over.is_err(), "the fourth hire must not pass the cap");
    assert!(agents.store("arthur").is_err());
    // Budget: a proposal that would pass the USD 5 floor is refused at proposal time.
    assert!(
        agents
            .propose_hire(
                &owner(),
                &hire("h5", "vanna", JobRole::SalesAffiliate, 2_500_000)
            )
            .is_err()
    );
    let book = agents.list_hires(&owner()).unwrap();
    assert_eq!(book["entries"]["h4"]["status"], "pending");
    let snapshot = CrewGuard::open(agents.root()).unwrap();
    let caps = hiring::active(agents.root(), &hiring::Book::load(&snapshot).unwrap()).unwrap();
    assert_eq!(caps.active_hires, 3);
    assert_eq!(caps.committed_usd_millionths, 3_000_000);
    let _ = Status::Pending;
}

#[test]
fn retirement_uses_the_shared_lifecycle_and_returns_leads() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, workspace) = host(&dir);
    let e = agents
        .propose_hire(
            &owner(),
            &hire("h1", "erin", JobRole::SalesResearcher, 1_000_000),
        )
        .unwrap();
    agents
        .decide_hire(
            &owner(),
            &decision(&e, HireVerdict::Confirm),
            workspace.to_str(),
            Some(&owner_key()),
        )
        .unwrap();
    let r = agents
        .propose_hire(&owner(), &retire("r1", "erin"))
        .unwrap();
    let done = agents
        .decide_hire(
            &owner(),
            &decision(&r, HireVerdict::Confirm),
            None,
            Some(&owner_key()),
        )
        .unwrap_or_else(|e| {
            panic!(
                "{e:?} {}",
                coder_host::tasks::take_reason(e).unwrap_or_default()
            )
        });
    assert_eq!(done["status"], "confirmed");
    assert!(done["decision"]["outcome"]["retired"].is_object());
    assert_eq!(
        done["decision"]["outcome"]["leads_released_to_paul"],
        serde_json::json!([])
    );
    let store = Store::new(agents.root(), "erin").unwrap();
    let record = store.load().unwrap().unwrap();
    assert_eq!(record.state, agent::State::Retired);
    assert!(store.key().unwrap().is_none());
    // The slot is free again and the retired name's history stays readable.
    assert!(
        agents
            .propose_hire(
                &owner(),
                &hire("h2", "frank", JobRole::SalesProspector, 1_000_000)
            )
            .is_ok()
    );
    let book = agents.list_hires(&owner()).unwrap();
    assert_eq!(book["entries"]["r1"]["status"], "confirmed");
    assert_eq!(book["entries"]["h1"]["status"], "confirmed");
}
