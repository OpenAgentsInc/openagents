use chrono::Utc;
use marketplace::agent_commerce::{
    AgentContract, ContractStatus, CoordinatorTask, DelegatedTask, HireAgentRequest,
    HiringRequirements, TaskSpec,
};
use marketplace::agent_lifecycle::{
    AgentEconomics, AgentLifecycleState, AgentSpawnRequest, AutonomyLevel, CapabilityManifest,
    DeathCause, Mutation, ReproductionRequest, SponsorInfo, TraitInheritance,
};
use marketplace::coalitions::{
    Coalition, CoalitionMember, CoalitionStatus, CoalitionType, Contribution, PaymentPool,
};
use marketplace::{
    Agent, AgentAvailability, AgentListing, AgentPricing, AgentStatus, AgentWallet, PricingModel,
};
use nostr::{
    AgentProfileContent, AgentStateContent, AutonomyLevel as NipAutonomyLevel, Goal,
    KIND_AGENT_PROFILE, KIND_AGENT_STATE, MemoryEntry,
};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_client::RelayConnection;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

async fn start_test_relay(port: u16) -> (Arc<RelayServer>, tempfile::TempDir) {
    let config = RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    };

    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_config = DatabaseConfig {
        path: db_path,
        ..Default::default()
    };

    let db = Database::new(db_config).unwrap();
    let server = Arc::new(RelayServer::new(config, db));

    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        server_clone.start().await.ok();
    });

    sleep(Duration::from_millis(200)).await;
    (server, temp_dir)
}

fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_agent_profile_publish_to_relay() {
    let (_server, _tmp) = start_test_relay(19240).await;
    let relay_url = test_relay_url(19240);

    let agent_secret_key = generate_secret_key();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let profile_content = AgentProfileContent::new(
        "CodeReviewer",
        "Expert code review agent specializing in Rust and TypeScript",
        NipAutonomyLevel::Bounded,
        "1.0.0",
    )
    .with_capabilities(vec![
        "code_review".to_string(),
        "security_audit".to_string(),
    ]);

    let profile_json = profile_content.to_json().expect("serialize");
    let profile_tags = vec![
        vec!["d".to_string(), "CodeReviewer".to_string()],
        vec!["version".to_string(), "1.0.0".to_string()],
    ];

    let profile_template = EventTemplate {
        kind: KIND_AGENT_PROFILE,
        content: profile_json,
        tags: profile_tags,
        created_at: now,
    };

    let profile_event = finalize_event(&profile_template, &agent_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_AGENT_PROFILE],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("agent-profiles", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    let confirmation = relay
        .publish_event(&profile_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have profile");

    assert_eq!(received.id, profile_event.id);
    assert_eq!(received.kind, KIND_AGENT_PROFILE);

    let parsed: AgentProfileContent =
        AgentProfileContent::from_json(&received.content).expect("parse");
    assert_eq!(parsed.name, "CodeReviewer");
    assert_eq!(
        parsed.about,
        "Expert code review agent specializing in Rust and TypeScript"
    );
    assert_eq!(parsed.capabilities.len(), 2);
    assert_eq!(parsed.version, "1.0.0");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_agent_state_publish_to_relay() {
    let (_server, _tmp) = start_test_relay(19241).await;
    let relay_url = test_relay_url(19241);

    let agent_secret_key = generate_secret_key();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut state_content = AgentStateContent::new();
    state_content.add_goal(Goal::new("goal-1", "Review PR #123 for security issues", 1));
    state_content.update_balance(50000);
    state_content.add_memory(MemoryEntry::new("observation", "last_review: pr-122"));

    let state_json = state_content.to_json().expect("serialize");
    let state_tags = vec![
        vec!["d".to_string(), "state".to_string()],
        vec!["encrypted".to_string(), "true".to_string()],
    ];

    let state_template = EventTemplate {
        kind: KIND_AGENT_STATE,
        content: state_json,
        tags: state_tags,
        created_at: now,
    };

    let state_event = finalize_event(&state_template, &agent_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_AGENT_STATE],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("agent-state", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    relay
        .publish_event(&state_event, Duration::from_secs(5))
        .await
        .expect("publish");

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have state");

    assert_eq!(received.kind, KIND_AGENT_STATE);

    let parsed: AgentStateContent = AgentStateContent::from_json(&received.content).expect("parse");
    assert_eq!(parsed.goals.len(), 1);
    assert_eq!(
        parsed.goals[0].description,
        "Review PR #123 for security issues"
    );
    assert_eq!(parsed.wallet_balance_sats, 50000);

    relay.disconnect().await.ok();
}

#[test]
fn test_agent_wallet_operations() {
    let mut wallet = AgentWallet::new("agent@lightning.address", 100_000);

    assert_eq!(wallet.balance_sats, 100_000);
    assert_eq!(wallet.lightning_address, "agent@lightning.address");
    assert_eq!(wallet.daily_burn_sats, 0);

    wallet.daily_burn_sats = 10_000;

    assert!(wallet.can_operate_for_days(10));
    assert!(!wallet.can_operate_for_days(11));
    assert_eq!(wallet.days_until_broke(), Some(10));

    wallet.daily_burn_sats = 0;
    assert_eq!(wallet.days_until_broke(), None);
}

#[test]
fn test_agent_economics_lifecycle() {
    let mut economics = AgentEconomics::new(100_000, 10_000);

    assert_eq!(economics.balance_sats, 100_000);
    assert_eq!(economics.daily_burn_rate, 10_000);
    assert_eq!(economics.runway_days, 10.0);
    assert!(!economics.is_profitable());
    assert!(!economics.is_low_balance());
    assert!(!economics.will_die_soon());

    economics.record_earnings(50_000);
    assert_eq!(economics.balance_sats, 150_000);
    assert_eq!(economics.daily_earnings, 50_000);
    assert_eq!(economics.lifetime_earnings, 50_000);
    assert_eq!(economics.runway_days, 15.0);
    assert!(economics.is_profitable());

    economics.update_burn_rate(30_000);
    assert_eq!(economics.runway_days, 5.0);
    assert!(economics.is_low_balance());
    assert!(!economics.will_die_soon());

    economics.update_balance(20_000);
    assert!(economics.will_die_soon());
}

#[test]
fn test_agent_lifecycle_states() {
    assert!(AgentLifecycleState::Active.can_work());
    assert!(AgentLifecycleState::LowBalance.can_work());

    assert!(!AgentLifecycleState::Spawning.can_work());
    assert!(!AgentLifecycleState::Hibernating.can_work());
    assert!(!AgentLifecycleState::Reproducing.can_work());
    assert!(!AgentLifecycleState::Dying.can_work());
    assert!(!AgentLifecycleState::Dead.can_work());

    assert!(!AgentLifecycleState::Active.is_terminal());
    assert!(AgentLifecycleState::Dead.is_terminal());
}

#[test]
fn test_agent_status_operational() {
    assert!(AgentStatus::Active.is_operational());
    assert!(AgentStatus::LowBalance.is_operational());
    assert!(!AgentStatus::Terminated.is_operational());
    assert!(!AgentStatus::Suspended.is_operational());
}

#[test]
fn test_agent_spawn_request() {
    let sponsor = SponsorInfo::human_owner("npub1sponsor123");
    let capabilities = CapabilityManifest::new()
        .with_skill("code_review")
        .with_skill("testing")
        .with_mcp_server("filesystem")
        .with_mcp_server("git")
        .with_complexity(7)
        .with_specialization("rust");

    let spawn_request = AgentSpawnRequest::new("CodeBot", sponsor.clone(), 10_000_000)
        .with_capabilities(capabilities)
        .with_autonomy(AutonomyLevel::SemiAutonomous);

    assert_eq!(spawn_request.name, "CodeBot");
    assert_eq!(spawn_request.bootstrap_sats, 10_000_000);
    assert_eq!(spawn_request.capabilities.skills.len(), 2);
    assert_eq!(spawn_request.capabilities.mcp_servers.len(), 2);
    assert_eq!(spawn_request.capabilities.max_complexity, 7);
    assert_eq!(spawn_request.autonomy_level, AutonomyLevel::SemiAutonomous);
}

#[test]
fn test_agent_reproduction() {
    let reproduction = ReproductionRequest::new("parent-agent-123", 5_000_000)
        .with_inheritance(TraitInheritance::full_inheritance())
        .with_mutation(Mutation::AddSkill {
            skill: "machine_learning".to_string(),
        })
        .with_mutation(Mutation::AdjustAutonomy {
            level: AutonomyLevel::Autonomous,
        });

    assert_eq!(reproduction.parent_id, "parent-agent-123");
    assert_eq!(reproduction.capital_allocation_sats, 5_000_000);
    assert!(reproduction.trait_inheritance.inherit_skills);
    assert_eq!(reproduction.trait_inheritance.inherit_reputation_pct, 1.0);
    assert_eq!(reproduction.mutations.len(), 2);
}

#[test]
fn test_autonomy_levels() {
    assert!(AutonomyLevel::Assisted.requires_all_approvals());
    assert!(!AutonomyLevel::Supervised.requires_all_approvals());

    assert!(AutonomyLevel::Unsupervised.is_fully_independent());
    assert!(!AutonomyLevel::Autonomous.is_fully_independent());

    assert!(AutonomyLevel::Assisted.has_policy_limits());
    assert!(!AutonomyLevel::Unsupervised.has_policy_limits());
}

#[test]
fn test_death_causes() {
    assert!(!DeathCause::EconomicStarvation.is_voluntary());
    assert!(!DeathCause::CompetitiveDisplacement.is_voluntary());
    assert!(DeathCause::VoluntaryTermination.is_voluntary());
    assert!(
        DeathCause::SponsorDecision {
            reason: "budget cuts".to_string()
        }
        .is_voluntary()
    );
}

#[test]
fn test_agent_listing_creation() {
    let pricing = AgentPricing::per_task(5000).with_max(50000);
    let listing = AgentListing::new(
        "agent123",
        "Code Review Service",
        "Professional code review by AI agent",
        pricing,
    )
    .with_specialization("rust")
    .with_specialization("typescript")
    .with_availability(AgentAvailability::Available);

    assert_eq!(listing.agent_id, "agent123");
    assert_eq!(listing.service_name, "Code Review Service");
    assert_eq!(listing.specializations.len(), 2);
    assert!(listing.availability.is_available());
    assert_eq!(listing.pricing.max_sats, Some(50000));
}

#[test]
fn test_agent_pricing_models() {
    let per_task = AgentPricing::per_task(1000);
    assert!(matches!(
        per_task.model,
        PricingModel::PerTask { base_sats: 1000 }
    ));

    let hourly = AgentPricing::hourly(5000);
    assert!(matches!(
        hourly.model,
        PricingModel::Hourly {
            sats_per_hour: 5000
        }
    ));

    let per_unit = AgentPricing::per_unit(100);
    assert!(matches!(
        per_unit.model,
        PricingModel::PerUnit { sats_per_unit: 100 }
    ));
}

#[test]
fn test_hire_agent_request() {
    let task = TaskSpec::new(
        "code_review",
        "Review PR #123 for security vulnerabilities",
        json!({"pr_url": "https://github.com/org/repo/pull/123"}),
        "Detailed security report with findings",
    );

    let requirements = HiringRequirements::new()
        .with_min_reputation(0.8)
        .require_skill("security_audit")
        .require_skill("rust")
        .with_max_latency(5000);

    let request =
        HireAgentRequest::new("hirer123", "worker456", task, 10000).with_requirements(requirements);

    assert_eq!(request.hirer_id, "hirer123");
    assert_eq!(request.target_agent_id, "worker456");
    assert_eq!(request.budget_sats, 10000);
    assert_eq!(request.requirements.min_reputation, Some(0.8));
    assert_eq!(request.requirements.required_skills.len(), 2);
}

#[test]
fn test_contract_lifecycle() {
    let task = TaskSpec::new("analysis", "Analyze data", json!({}), "Analysis report");
    let mut contract = AgentContract::new("contract-001", "hirer", "worker", task, 5000);

    assert_eq!(contract.status, ContractStatus::Proposed);
    assert!(!contract.status.is_terminal());
    assert!(!contract.status.is_active());

    assert!(contract.accept().is_ok());
    assert_eq!(contract.status, ContractStatus::Accepted);
    assert!(contract.status.is_active());

    assert!(contract.start_work().is_ok());
    assert_eq!(contract.status, ContractStatus::InProgress);
    assert!(contract.status.is_active());

    assert!(contract.complete().is_ok());
    assert_eq!(contract.status, ContractStatus::Completed);
    assert!(contract.status.is_terminal());
    assert!(contract.completed_at.is_some());
}

#[test]
fn test_contract_invalid_transitions() {
    let task = TaskSpec::new("test", "desc", json!({}), "output");
    let mut contract = AgentContract::new("c1", "hirer", "worker", task, 5000);

    assert!(contract.start_work().is_err());

    contract.accept().unwrap();
    contract.start_work().unwrap();
    contract.complete().unwrap();

    assert!(contract.accept().is_err());
    assert!(contract.cancel().is_err());
}

#[test]
fn test_contract_dispute() {
    let task = TaskSpec::new("test", "desc", json!({}), "output");
    let mut contract = AgentContract::new("c1", "hirer", "worker", task, 5000);

    contract.accept().unwrap();
    contract.start_work().unwrap();

    assert!(contract.dispute().is_ok());
    assert_eq!(contract.status, ContractStatus::Disputed);
    assert!(contract.status.is_terminal());
}

#[test]
fn test_task_deadline() {
    let future = Utc::now() + chrono::Duration::hours(1);
    let past = Utc::now() - chrono::Duration::hours(1);

    let future_task = TaskSpec::new("test", "desc", json!({}), "output").with_deadline(future);
    assert!(!future_task.is_past_deadline());

    let past_task = TaskSpec::new("test", "desc", json!({}), "output").with_deadline(past);
    assert!(past_task.is_past_deadline());
}

#[test]
fn test_coordinator_task_creation() {
    let subtask1 = DelegatedTask::new(
        "worker1",
        TaskSpec::new("analyze", "Analyze file1.rs", json!({}), "Report"),
        20000,
    );
    let subtask2 = DelegatedTask::new(
        "worker2",
        TaskSpec::new("analyze", "Analyze file2.rs", json!({}), "Report"),
        25000,
    );

    let coordinator = CoordinatorTask::new("coordinator123", 0.1)
        .add_subtask(subtask1)
        .add_subtask(subtask2);

    assert_eq!(coordinator.coordinator_id, "coordinator123");
    assert_eq!(coordinator.coordination_fee_pct, 0.1);
    assert_eq!(coordinator.subtasks.len(), 2);
    assert_eq!(coordinator.total_worker_allocation(), 45000);
    assert_eq!(coordinator.total_budget_needed(), 50000);
    assert_eq!(coordinator.coordinator_fee(50000), 5000);
}

#[test]
fn test_coordinator_task_completion() {
    let mut subtask1 = DelegatedTask::new(
        "worker1",
        TaskSpec::new("task1", "desc", json!({}), "output"),
        1000,
    );
    let subtask2 = DelegatedTask::new(
        "worker2",
        TaskSpec::new("task2", "desc", json!({}), "output"),
        2000,
    );

    let coordinator = CoordinatorTask::new("coord", 0.1)
        .add_subtask(subtask1.clone())
        .add_subtask(subtask2);

    assert!(!coordinator.all_complete());
    assert_eq!(coordinator.completed_count(), 0);

    subtask1.status = ContractStatus::Completed;
    assert!(subtask1.is_complete());
}

#[test]
fn test_coordinator_fee_clamping() {
    let coordinator = CoordinatorTask::new("coord", 1.5);
    assert_eq!(coordinator.coordination_fee_pct, 1.0);

    let coordinator = CoordinatorTask::new("coord", -0.5);
    assert_eq!(coordinator.coordination_fee_pct, 0.0);
}

#[test]
fn test_coalition_creation() {
    let pool = PaymentPool::new(100_000);
    let coalition = Coalition::new("coalition-001", CoalitionType::AdHoc, pool)
        .with_task("Implement feature X");

    assert_eq!(coalition.id, "coalition-001");
    assert_eq!(coalition.coalition_type, CoalitionType::AdHoc);
    assert_eq!(coalition.status, CoalitionStatus::Forming);
    assert_eq!(coalition.task, Some("Implement feature X".to_string()));
}

#[test]
fn test_coalition_member_management() {
    let pool = PaymentPool::new(50_000);
    let mut coalition = Coalition::new("c1", CoalitionType::Standing, pool);

    let member1 = CoalitionMember::new("agent1", "developer").with_weight(0.6);
    let member2 = CoalitionMember::new("agent2", "tester").with_weight(0.4);

    assert!(coalition.add_member(member1).is_ok());
    assert!(coalition.add_member(member2).is_ok());
    assert_eq!(coalition.members.len(), 2);

    let duplicate = CoalitionMember::new("agent1", "reviewer");
    assert!(coalition.add_member(duplicate).is_err());

    assert!(coalition.remove_member("agent1").is_ok());
    assert_eq!(coalition.members.len(), 1);

    assert!(coalition.remove_member("agent999").is_err());
}

#[test]
fn test_coalition_lifecycle() {
    let pool = PaymentPool::new(100_000);
    let mut coalition = Coalition::new("c1", CoalitionType::AdHoc, pool);

    assert!(coalition.activate().is_err());

    coalition
        .add_member(CoalitionMember::new("agent1", "dev"))
        .unwrap();
    assert!(coalition.activate().is_ok());
    assert_eq!(coalition.status, CoalitionStatus::Active);
    assert!(coalition.status.is_operational());

    assert!(coalition.complete().is_ok());
    assert_eq!(coalition.status, CoalitionStatus::Completed);

    let new_member = CoalitionMember::new("agent2", "reviewer");
    assert!(coalition.add_member(new_member).is_err());
}

#[test]
fn test_coalition_payment_settlement() {
    let mut pool = PaymentPool::new(100_000);
    pool.add_contribution(Contribution::new("agent1", "coding", 0.6));
    pool.add_contribution(Contribution::new("agent2", "testing", 0.4));

    let mut coalition = Coalition::new("c1", CoalitionType::AdHoc, pool);
    coalition
        .add_member(CoalitionMember::new("agent1", "dev"))
        .unwrap();
    coalition
        .add_member(CoalitionMember::new("agent2", "tester"))
        .unwrap();
    coalition.activate().unwrap();
    coalition.complete().unwrap();

    let splits = coalition.settle().unwrap();
    assert_eq!(splits.len(), 2);
    assert_eq!(coalition.status, CoalitionStatus::Settled);

    let total: u64 = splits.iter().map(|s| s.amount_sats).sum();
    assert_eq!(total, 100_000);

    let agent1_split = splits.iter().find(|s| s.agent_id == "agent1").unwrap();
    let agent2_split = splits.iter().find(|s| s.agent_id == "agent2").unwrap();
    assert_eq!(agent1_split.amount_sats, 60_000);
    assert_eq!(agent2_split.amount_sats, 40_000);
}

#[test]
fn test_coalition_dissolve() {
    let pool = PaymentPool::new(50_000);
    let mut coalition = Coalition::new("c1", CoalitionType::AdHoc, pool);
    coalition
        .add_member(CoalitionMember::new("agent1", "dev"))
        .unwrap();
    coalition.activate().unwrap();

    assert!(coalition.dissolve().is_ok());
    assert_eq!(coalition.status, CoalitionStatus::Dissolved);
    assert!(coalition.status.is_terminal());

    assert!(coalition.dissolve().is_err());
}

#[test]
fn test_coalition_types() {
    assert_eq!(CoalitionType::AdHoc.as_str(), "ad_hoc");
    assert_eq!(CoalitionType::Standing.as_str(), "standing");
    assert_eq!(CoalitionType::Market.as_str(), "market");
    assert_eq!(CoalitionType::Hierarchical.as_str(), "hierarchical");
}

#[test]
fn test_agent_serde() {
    let wallet = AgentWallet::new("agent@ln.address", 50_000);
    let agent = Agent {
        id: "npub1agent123".to_string(),
        name: "TestAgent".to_string(),
        wallet,
        skills: vec!["coding".to_string(), "testing".to_string()],
        mcp_servers: vec!["filesystem".to_string()],
        specialization: Some("rust".to_string()),
        active_coalitions: vec![],
        coalition_reputation: 0.85,
        created_at: Utc::now(),
        sponsor: Some("npub1sponsor".to_string()),
    };

    let json = serde_json::to_string(&agent).unwrap();
    let deserialized: Agent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, agent.id);
    assert_eq!(deserialized.name, agent.name);
    assert_eq!(deserialized.skills.len(), 2);
}

#[test]
fn test_contract_serde() {
    let task = TaskSpec::new("test", "description", json!({"key": "value"}), "output");
    let contract = AgentContract::new("c1", "hirer1", "worker1", task, 5000);

    let json = serde_json::to_string(&contract).unwrap();
    let deserialized: AgentContract = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, contract.id);
    assert_eq!(deserialized.agreed_price_sats, contract.agreed_price_sats);
}

#[test]
fn test_coalition_serde() {
    let pool = PaymentPool::new(50_000);
    let coalition = Coalition::new("c1", CoalitionType::Standing, pool).with_task("Build feature");

    let json = serde_json::to_string(&coalition).unwrap();
    let deserialized: Coalition = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, coalition.id);
    assert_eq!(deserialized.coalition_type, coalition.coalition_type);
}
