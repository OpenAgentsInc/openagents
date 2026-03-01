use super::*;

pub struct CodexAccountPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub account_summary: String,
    pub requires_openai_auth: bool,
    pub auth_mode: Option<String>,
    pub pending_login_id: Option<String>,
    pub pending_login_url: Option<String>,
    pub rate_limits_summary: Option<String>,
}

impl Default for CodexAccountPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for account/read".to_string()),
            account_summary: "unknown".to_string(),
            requires_openai_auth: true,
            auth_mode: None,
            pending_login_id: None,
            pending_login_url: None,
            rate_limits_summary: None,
        }
    }
}

pub struct CodexModelCatalogEntryState {
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<String>,
}

pub struct CodexModelsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub include_hidden: bool,
    pub entries: Vec<CodexModelCatalogEntryState>,
    pub last_reroute: Option<String>,
}

impl Default for CodexModelsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for model/list".to_string()),
            include_hidden: false,
            entries: Vec::new(),
            last_reroute: None,
        }
    }
}

pub struct CodexConfigPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub config_json: String,
    pub requirements_json: String,
    pub detected_external_configs: usize,
}

impl Default for CodexConfigPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for config/read".to_string()),
            config_json: "{}".to_string(),
            requirements_json: "null".to_string(),
            detected_external_configs: 0,
        }
    }
}

pub struct CodexMcpServerEntryState {
    pub name: String,
    pub auth_status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub template_count: usize,
}

pub struct CodexMcpPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub servers: Vec<CodexMcpServerEntryState>,
    pub selected_server_index: Option<usize>,
    pub last_oauth_url: Option<String>,
    pub last_oauth_result: Option<String>,
    pub next_cursor: Option<String>,
}

impl Default for CodexMcpPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for mcpServerStatus/list".to_string()),
            servers: Vec::new(),
            selected_server_index: None,
            last_oauth_url: None,
            last_oauth_result: None,
            next_cursor: None,
        }
    }
}

pub struct CodexAppEntryState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub is_enabled: bool,
}

pub struct CodexAppsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub apps: Vec<CodexAppEntryState>,
    pub selected_app_index: Option<usize>,
    pub next_cursor: Option<String>,
    pub update_count: u64,
}

impl Default for CodexAppsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for app/list".to_string()),
            apps: Vec::new(),
            selected_app_index: None,
            next_cursor: None,
            update_count: 0,
        }
    }
}

pub struct CodexLabsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub review_last_turn_id: Option<String>,
    pub review_last_thread_id: Option<String>,
    pub command_last_exit_code: Option<i32>,
    pub command_last_stdout: String,
    pub command_last_stderr: String,
    pub collaboration_modes_json: String,
    pub experimental_features_json: String,
    pub experimental_enabled: bool,
    pub realtime_started: bool,
    pub fuzzy_session_id: String,
    pub fuzzy_last_status: String,
    pub windows_last_status: Option<String>,
}

impl Default for CodexLabsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Codex Labs ready".to_string()),
            review_last_turn_id: None,
            review_last_thread_id: None,
            command_last_exit_code: None,
            command_last_stdout: String::new(),
            command_last_stderr: String::new(),
            collaboration_modes_json: "[]".to_string(),
            experimental_features_json: "[]".to_string(),
            experimental_enabled: false,
            realtime_started: false,
            fuzzy_session_id: format!("labs-{}", std::process::id()),
            fuzzy_last_status: "idle".to_string(),
            windows_last_status: None,
        }
    }
}

pub struct CodexDiagnosticsMethodCountState {
    pub method: String,
    pub count: u64,
}

pub struct CodexDiagnosticsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub notification_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub server_request_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub raw_events: Vec<String>,
    pub last_command_failure: Option<String>,
    pub last_snapshot_error: Option<String>,
    pub wire_log_path: String,
    pub wire_log_enabled: bool,
}

impl Default for CodexDiagnosticsPaneState {
    fn default() -> Self {
        let env_wire_log_path = std::env::var("OPENAGENTS_CODEX_WIRE_LOG_PATH").ok();
        let wire_log_path = env_wire_log_path
            .clone()
            .unwrap_or_else(|| "/tmp/openagents-codex-wire.log".to_string());
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Codex diagnostics idle".to_string()),
            notification_counts: Vec::new(),
            server_request_counts: Vec::new(),
            raw_events: Vec::new(),
            last_command_failure: None,
            last_snapshot_error: None,
            wire_log_path: wire_log_path.clone(),
            wire_log_enabled: env_wire_log_path.is_some(),
        }
    }
}

pub struct AgentProfileStatePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub profile_name: String,
    pub profile_about: String,
    pub goals_summary: String,
    pub profile_event_id: Option<String>,
    pub state_event_id: Option<String>,
    pub goals_event_id: Option<String>,
}

impl Default for AgentProfileStatePaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA profile/state snapshot".to_string()),
            profile_name: "Autopilot".to_string(),
            profile_about: "Desktop sovereign agent runtime".to_string(),
            goals_summary: "Earn sats and complete queued jobs".to_string(),
            profile_event_id: None,
            state_event_id: None,
            goals_event_id: None,
        }
    }
}

pub struct AgentScheduleTickPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub heartbeat_seconds: u64,
    pub next_tick_reason: String,
    pub last_tick_outcome: String,
    pub schedule_event_id: Option<String>,
    pub tick_request_event_id: Option<String>,
    pub tick_result_event_id: Option<String>,
}

impl Default for AgentScheduleTickPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA schedule/tick snapshot".to_string()),
            heartbeat_seconds: 30,
            next_tick_reason: "manual.operator".to_string(),
            last_tick_outcome: "n/a".to_string(),
            schedule_event_id: None,
            tick_request_event_id: None,
            tick_result_event_id: None,
        }
    }
}

pub struct TrajectoryAuditPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_session_id: Option<String>,
    pub verified_hash: Option<String>,
    pub step_filter: String,
}

impl Default for TrajectoryAuditPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for trajectory session stream".to_string()),
            active_session_id: None,
            verified_hash: None,
            step_filter: "all".to_string(),
        }
    }
}

pub struct SkillRegistryPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub search_query: String,
    pub manifest_slug: String,
    pub manifest_version: String,
    pub manifest_a: Option<String>,
    pub manifest_event_id: Option<String>,
    pub version_event_id: Option<String>,
    pub search_result_event_id: Option<String>,
    pub source: String,
    pub repo_skills_root: Option<String>,
    pub discovered_skills: Vec<SkillRegistryDiscoveredSkill>,
    pub discovery_errors: Vec<String>,
    pub selected_skill_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillRegistryDiscoveredSkill {
    pub name: String,
    pub path: String,
    pub scope: String,
    pub enabled: bool,
    pub interface_display_name: Option<String>,
    pub dependency_count: usize,
}

impl Default for SkillRegistryPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL registry snapshot".to_string()),
            search_query: "summarize".to_string(),
            manifest_slug: "summarize-text".to_string(),
            manifest_version: "0.1.0".to_string(),
            manifest_a: None,
            manifest_event_id: None,
            version_event_id: None,
            search_result_event_id: None,
            source: "codex".to_string(),
            repo_skills_root: None,
            discovered_skills: Vec::new(),
            discovery_errors: Vec::new(),
            selected_skill_index: None,
        }
    }
}

pub struct SkillTrustRevocationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub trust_tier: String,
    pub manifest_a: Option<String>,
    pub attestation_count: u32,
    pub kill_switch_active: bool,
    pub revocation_event_id: Option<String>,
}

impl Default for SkillTrustRevocationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL trust gate snapshot".to_string()),
            trust_tier: "unknown".to_string(),
            manifest_a: None,
            attestation_count: 0,
            kill_switch_active: false,
            revocation_event_id: None,
        }
    }
}

pub struct CreditDeskPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub scope: String,
    pub requested_sats: u64,
    pub offered_sats: u64,
    pub envelope_cap_sats: u64,
    pub spend_sats: u64,
    pub spend_job_id: String,
    pub intent_event_id: Option<String>,
    pub offer_event_id: Option<String>,
    pub envelope_event_id: Option<String>,
    pub spend_event_id: Option<String>,
}

impl Default for CreditDeskPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC credit desk snapshot".to_string()),
            scope: "skill:33400:npub1agent:summarize-text:0.1.0:constraints".to_string(),
            requested_sats: 1500,
            offered_sats: 1400,
            envelope_cap_sats: 1200,
            spend_sats: 600,
            spend_job_id: "job-credit-001".to_string(),
            intent_event_id: None,
            offer_event_id: None,
            envelope_event_id: None,
            spend_event_id: None,
        }
    }
}

pub struct CreditSettlementLedgerPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub result_event_id: String,
    pub payment_pointer: String,
    pub default_reason: String,
    pub settlement_event_id: Option<String>,
    pub default_event_id: Option<String>,
}

impl Default for CreditSettlementLedgerPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC settlement ledger snapshot".to_string()),
            result_event_id: "nip90:result:pending".to_string(),
            payment_pointer: "pay:pending".to_string(),
            default_reason: "settlement timeout".to_string(),
            settlement_event_id: None,
            default_event_id: None,
        }
    }
}

pub struct CadDemoPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub session_id: String,
    pub document_id: String,
    pub document_revision: u64,
    pub active_variant_id: String,
    pub variant_ids: Vec<String>,
    pub last_rebuild_receipt: Option<CadRebuildReceiptState>,
    pub rebuild_receipts: Vec<CadRebuildReceiptState>,
    pub eval_cache: openagents_cad::eval::EvalCacheStore,
    pub rebuild_worker: Option<crate::cad_rebuild_worker::CadBackgroundRebuildWorker>,
    pub next_rebuild_request_id: u64,
    pub pending_rebuild_request_id: Option<u64>,
    pub last_good_mesh_id: Option<String>,
    pub warnings: Vec<CadDemoWarningState>,
    pub warning_filter_severity: String,
    pub warning_filter_code: String,
    pub warning_hover_index: Option<usize>,
    pub focused_warning_index: Option<usize>,
    pub focused_geometry_ref: Option<String>,
    pub history_stack: openagents_cad::history::CadHistoryStack,
    pub timeline_rows: Vec<CadTimelineRowState>,
    pub timeline_selected_index: Option<usize>,
    pub timeline_scroll_offset: usize,
    pub selected_feature_params: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadRebuildReceiptState {
    pub event_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub rebuild_hash: String,
    pub duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_evictions: u64,
    pub feature_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadDemoWarningState {
    pub warning_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
    pub remediation_hint: String,
    pub semantic_refs: Vec<String>,
    pub deep_link: Option<String>,
    pub feature_id: String,
    pub entity_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadTimelineRowState {
    pub feature_id: String,
    pub feature_name: String,
    pub op_type: String,
    pub status_badge: String,
    pub provenance: String,
    pub params: Vec<(String, String)>,
}

impl Default for CadDemoPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("CAD demo initialized; waiting for feature graph state".to_string()),
            session_id: "cad.session.local".to_string(),
            document_id: "cad.doc.demo-rack".to_string(),
            document_revision: 0,
            active_variant_id: "variant.baseline".to_string(),
            variant_ids: vec![
                "variant.baseline".to_string(),
                "variant.lightweight".to_string(),
                "variant.low-cost".to_string(),
                "variant.stiffness".to_string(),
            ],
            last_rebuild_receipt: None,
            rebuild_receipts: Vec::new(),
            eval_cache: openagents_cad::eval::EvalCacheStore::new(128)
                .expect("cad eval cache capacity should be valid"),
            rebuild_worker: None,
            next_rebuild_request_id: 1,
            pending_rebuild_request_id: None,
            last_good_mesh_id: None,
            warnings: Vec::new(),
            warning_filter_severity: "all".to_string(),
            warning_filter_code: "all".to_string(),
            warning_hover_index: None,
            focused_warning_index: None,
            focused_geometry_ref: None,
            history_stack: openagents_cad::history::CadHistoryStack::new("cad.session.local", 128)
                .expect("cad history max_steps should be valid"),
            timeline_rows: Vec::new(),
            timeline_selected_index: None,
            timeline_scroll_offset: 0,
            selected_feature_params: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentNetworkSimulationEvent {
    pub seq: u64,
    pub protocol: String,
    pub event_ref: String,
    pub summary: String,
}

pub struct AgentNetworkSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub channel_name: String,
    pub channel_event_id: Option<String>,
    pub rounds_run: u32,
    pub total_transferred_sats: u64,
    pub learned_skills: Vec<String>,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for AgentNetworkSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Run simulation to create NIP-28 coordination channel".to_string()),
            channel_name: "sovereign-agents-lab".to_string(),
            channel_event_id: None,
            rounds_run: 0,
            total_transferred_sats: 0,
            learned_skills: Vec::new(),
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl AgentNetworkSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let channel_event_id = match self.channel_event_id.clone() {
            Some(existing) => existing,
            None => {
                let metadata = nostr::ChannelMetadata::new(
                    self.channel_name.clone(),
                    "Public SA/SKL/AC negotiation channel",
                    "https://openagents.com/channel.png",
                )
                .with_relays(vec!["wss://relay.openagents.dev".to_string()]);
                let creation = nostr::ChannelCreateEvent::new(metadata, now_epoch_seconds);
                if let Err(error) = creation.content() {
                    self.load_state = PaneLoadState::Error;
                    self.last_error = Some(error.to_string());
                    return Err(error.to_string());
                }
                let id = format!(
                    "sim:{}:{:08x}",
                    nostr::KIND_CHANNEL_CREATION,
                    self.rounds_run + 1
                );
                self.channel_event_id = Some(id.clone());
                self.push_event(
                    "NIP-28",
                    &id,
                    format!(
                        "created channel #{} ({})",
                        self.channel_name, creation.created_at
                    ),
                );
                id
            }
        };

        let round = self.rounds_run.saturating_add(1);
        let relay = "wss://relay.openagents.dev";
        let skill_version = format!("0.{}.0", round + 1);
        let skill_ref = format!("33400:npub1beta:summarize-text:{skill_version}");
        let base = u64::from(round) * 10;

        let announce = nostr::ChannelMessageEvent::new(
            channel_event_id.clone(),
            relay,
            format!(
                "agent-alpha requests summarize-text@{skill_version} for client brief #{round}"
            ),
            now_epoch_seconds.saturating_add(base + 1),
        );
        let announce_id = format!("sim:{}:{:08x}:a", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &announce_id,
            format!(
                "alpha broadcast in channel ({} tags)",
                announce.to_tags().len()
            ),
        );

        let negotiation = nostr::ChannelMessageEvent::reply(
            channel_event_id,
            announce_id.clone(),
            relay,
            format!("agent-beta offers {skill_ref} with AC escrow"),
            now_epoch_seconds.saturating_add(base + 2),
        )
        .mention_pubkey("npub1alpha", Some(relay.to_string()));
        let negotiation_id = format!("sim:{}:{:08x}:b", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &negotiation_id,
            format!(
                "beta replied with terms ({} tags)",
                negotiation.to_tags().len()
            ),
        );

        self.push_event(
            "NIP-SKL",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_MANIFEST, round),
            format!("beta published manifest {skill_ref}"),
        );
        self.push_event(
            "NIP-SA",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_LICENSE, round),
            format!("alpha learned skill summarize-text@{skill_version}"),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_INTENT, round),
            "opened escrow intent for skill execution".to_string(),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_SETTLEMENT, round),
            "settled escrow after successful delivery".to_string(),
        );

        let transfer_sats = 250_u64.saturating_add(u64::from(round) * 35);
        self.total_transferred_sats = self.total_transferred_sats.saturating_add(transfer_sats);
        self.rounds_run = round;

        if !self.learned_skills.iter().any(|skill| skill == &skill_ref) {
            self.learned_skills.push(skill_ref);
        }

        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: agents exchanged NIP-28 messages and settled {transfer_sats} sats"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Simulation reset".to_string());
        self.channel_event_id = None;
        self.rounds_run = 0;
        self.total_transferred_sats = 0;
        self.learned_skills.clear();
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct TreasuryExchangeSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rounds_run: u32,
    pub order_event_id: Option<String>,
    pub mint_reference: Option<String>,
    pub wallet_connect_url: Option<String>,
    pub total_liquidity_sats: u64,
    pub trade_volume_sats: u64,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for TreasuryExchangeSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model treasury + exchange NIP interactions".to_string(),
            ),
            rounds_run: 0,
            order_event_id: None,
            mint_reference: None,
            wallet_connect_url: None,
            total_liquidity_sats: 0,
            trade_volume_sats: 0,
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl TreasuryExchangeSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);
        let swap_sats = 40_000_u64.saturating_add(u64::from(round) * 5_000);

        let handler = nostr::nip89::HandlerInfo::new(
            "npub1treasury",
            nostr::nip89::HandlerType::Agent,
            nostr::nip89::HandlerMetadata::new(
                "Treasury Agent",
                "Provides FX and liquidity routing for agent payments",
            ),
        )
        .add_capability("fx.quote.btcusd")
        .add_capability("swap.cashu.lightning")
        .with_pricing(
            nostr::nip89::PricingInfo::new(25)
                .with_model("per-swap")
                .with_currency("sat"),
        );
        let handler_tags = handler.to_tags();
        self.push_event(
            "NIP-89",
            &format!("sim:{}:{:08x}", nostr::nip89::KIND_HANDLER_INFO, round),
            format!("announced treasury handler ({} tags)", handler_tags.len()),
        );

        let mint_tags = nostr::nip87::create_cashu_mint_tags(
            "mint-pubkey-alpha",
            "https://mint.openagents.dev",
            &[1, 2, 3, 4, 5, 11, 12],
            nostr::nip87::MintNetwork::Mainnet,
        );
        let mint = nostr::nip87::parse_cashu_mint(
            nostr::nip87::KIND_CASHU_MINT,
            &mint_tags,
            "{\"name\":\"OpenAgents Mint\"}",
        )
        .map_err(|error| error.to_string())?;
        self.mint_reference = Some(format!("{} ({})", mint.url, mint.network.as_str()));
        self.push_event(
            "NIP-87",
            &format!("sim:{}:{:08x}", nostr::nip87::KIND_CASHU_MINT, round),
            format!("discovered mint {} with {} nuts", mint.url, mint.nuts.len()),
        );

        let order_id = format!("order-{:04}", round);
        let expires_at = now_epoch_seconds.saturating_add(900);
        let order_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip69::P2P_ORDER_KIND, round),
            pubkey: "npub1treasury".to_string(),
            created_at: now_epoch_seconds,
            kind: nostr::nip69::P2P_ORDER_KIND,
            tags: vec![
                vec!["d".to_string(), order_id.clone()],
                vec!["k".to_string(), "sell".to_string()],
                vec!["f".to_string(), "USD".to_string()],
                vec!["s".to_string(), "pending".to_string()],
                vec!["amt".to_string(), swap_sats.to_string()],
                vec!["fa".to_string(), "1250".to_string()],
                vec!["pm".to_string(), "wire".to_string(), "cashapp".to_string()],
                vec!["premium".to_string(), "1.5".to_string()],
                vec!["network".to_string(), "bitcoin".to_string()],
                vec!["layer".to_string(), "lightning".to_string()],
                vec!["expires_at".to_string(), expires_at.to_string()],
                vec!["expiration".to_string(), expires_at.to_string()],
                vec!["y".to_string(), "openagents-exchange".to_string()],
            ],
            content: String::new(),
            sig: format!("sim-signature-{round}"),
        };
        let order = nostr::nip69::P2POrder::from_event(order_event.clone())
            .map_err(|error| error.to_string())?;
        self.order_event_id = Some(order_event.id.clone());
        self.push_event(
            "NIP-69",
            &order_event.id,
            format!(
                "published {} order {} for {} sats",
                order.order_type.as_str(),
                order.order_id,
                order.amount_sats
            ),
        );

        let token_content = nostr::nip60::TokenContent::new(
            mint.url.clone(),
            vec![nostr::nip60::CashuProof::new(
                format!("proof-{round:04}"),
                swap_sats,
                format!("secret-{round:04}"),
                format!("C-{round:04}"),
            )],
        )
        .with_unit("sat".to_string());
        let locked_sats = token_content.total_amount();
        self.total_liquidity_sats = self.total_liquidity_sats.saturating_add(locked_sats);
        self.push_event(
            "NIP-60",
            &format!("sim:{}:{:08x}", nostr::nip60::TOKEN_KIND, round),
            format!("locked {} sats in wallet token batch", locked_sats),
        );

        let nutzap_proof = nostr::nip61::NutzapProof::new(
            swap_sats,
            format!("C-{round:04}"),
            format!("proof-{round:04}"),
            format!("secret-{round:04}"),
        );
        let proof_tag =
            nostr::nip61::create_proof_tag(&nutzap_proof).map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-61",
            &format!("sim:{}:{:08x}", nostr::nip61::NUTZAP_KIND, round),
            format!(
                "created nutzap settlement proof ({} fields)",
                proof_tag.len()
            ),
        );

        let wallet_connect_url = nostr::nip47::NostrWalletConnectUrl::new(
            "walletpubkey123",
            vec!["wss://relay.openagents.dev".to_string()],
            format!("secret-{round:04}"),
        )
        .with_lud16("treasury@openagents.dev")
        .to_string();
        self.wallet_connect_url = Some(wallet_connect_url.clone());
        self.push_event(
            "NIP-47",
            &format!("sim:{}:{:08x}", nostr::nip47::REQUEST_KIND, round),
            format!(
                "prepared wallet connect session ({} chars)",
                wallet_connect_url.len()
            ),
        );

        let reputation_label = nostr::nip32::LabelEvent::new(
            vec![nostr::nip32::Label::new("success", "exchange/trade")],
            vec![nostr::nip32::LabelTarget::event(
                order_event.id.clone(),
                Some("wss://relay.openagents.dev"),
            )],
        )
        .with_content("atomic swap completed within policy bounds");
        self.push_event(
            "NIP-32",
            &format!("sim:{}:{:08x}", nostr::nip32::KIND_LABEL, round),
            format!(
                "emitted trade attestation ({} tags)",
                reputation_label.to_tags().len()
            ),
        );

        self.rounds_run = round;
        self.trade_volume_sats = self.trade_volume_sats.saturating_add(swap_sats);
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: routed {} sats through discovery, orderbook, wallet and settlement rails",
            swap_sats
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Treasury exchange simulation reset".to_string());
        self.rounds_run = 0;
        self.order_event_id = None;
        self.mint_reference = None;
        self.wallet_connect_url = None;
        self.total_liquidity_sats = 0;
        self.trade_volume_sats = 0;
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto treasury simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto treasury simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct RelaySecuritySimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub relay_url: String,
    pub challenge: String,
    pub auth_event_id: Option<String>,
    pub rounds_run: u32,
    pub dm_relay_count: u32,
    pub sync_ranges: u32,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for RelaySecuritySimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model secure relay, auth, and sync lifecycle".to_string(),
            ),
            relay_url: "wss://relay.openagents.dev".to_string(),
            challenge: "auth-bootstrap".to_string(),
            auth_event_id: None,
            rounds_run: 0,
            dm_relay_count: 0,
            sync_ranges: 0,
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl RelaySecuritySimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);

        let relay_doc = nostr::nip11::RelayInformationDocument {
            name: Some("OpenAgents Auth Relay".to_string()),
            description: Some("Relay exposing auth, DM, and audit sync capabilities".to_string()),
            supported_nips: Some(vec![11, 17, 42, 46, 65, 77, 98]),
            limitation: Some(nostr::nip11::RelayLimitation {
                auth_required: Some(true),
                restricted_writes: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let relay_doc_json = relay_doc.to_json().map_err(|error| error.to_string())?;
        let relay_doc_roundtrip =
            nostr::nip11::RelayInformationDocument::from_json(&relay_doc_json)
                .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-11",
            &format!("sim:30111:{:08x}", round),
            format!(
                "loaded relay document ({} advertised nips)",
                relay_doc_roundtrip
                    .supported_nips
                    .map_or(0, |nips| nips.len())
            ),
        );

        let relay_list = nostr::nip65::RelayListMetadata::new(vec![
            nostr::nip65::RelayEntry::write(self.relay_url.clone()),
            nostr::nip65::RelayEntry::read("wss://relay.backup.openagents.dev".to_string()),
        ]);
        self.dm_relay_count = relay_list.all_relays().len() as u32;
        self.push_event(
            "NIP-65",
            &format!(
                "sim:{}:{:08x}",
                nostr::nip65::RELAY_LIST_METADATA_KIND,
                round
            ),
            format!(
                "published relay list (read={} write={})",
                relay_list.read_relays().len(),
                relay_list.write_relays().len()
            ),
        );

        let challenge = format!("auth-{round:04x}");
        self.challenge.clone_from(&challenge);
        let auth_template = nostr::nip42::create_auth_event_template(&self.relay_url, &challenge);
        let auth_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip42::AUTH_KIND, round),
            pubkey: "npub1agentalpha".to_string(),
            created_at: auth_template.created_at,
            kind: auth_template.kind,
            tags: auth_template.tags,
            content: auth_template.content,
            sig: format!("sim-auth-signature-{round}"),
        };
        nostr::nip42::validate_auth_event(
            &auth_event,
            &self.relay_url,
            &challenge,
            Some(auth_event.created_at),
        )
        .map_err(|error| error.to_string())?;
        self.auth_event_id = Some(auth_event.id.clone());
        self.push_event(
            "NIP-42",
            &auth_event.id,
            "validated relay authentication event".to_string(),
        );

        let signer_request = nostr::nip46::NostrConnectRequest::get_public_key();
        let signer_request_json = signer_request
            .to_json()
            .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-46",
            &format!("sim:{}:{:08x}", nostr::nip46::KIND_NOSTR_CONNECT, round),
            format!(
                "queued remote signing request {} ({} bytes)",
                signer_request.id,
                signer_request_json.len()
            ),
        );

        let sender_sk = nostr::generate_secret_key();
        let recipient_sk = nostr::generate_secret_key();
        let recipient_pk = nostr::get_public_key_hex(&recipient_sk).map_err(|e| e.to_string())?;
        let message =
            nostr::nip17::ChatMessage::new(format!("relay-auth heartbeat {} confirmed", round))
                .add_recipient(recipient_pk.clone(), Some(self.relay_url.clone()))
                .subject("secure-ops");
        let wrapped = nostr::nip17::send_chat_message(
            &message,
            &sender_sk,
            &recipient_pk,
            now_epoch_seconds.saturating_add(3),
        )
        .map_err(|error| error.to_string())?;
        let received = nostr::nip17::receive_chat_message(&wrapped, &recipient_sk)
            .map_err(|e| e.to_string())?;
        self.push_event(
            "NIP-17",
            &format!("sim:{}:{:08x}", nostr::nip17::KIND_CHAT_MESSAGE, round),
            format!(
                "sent private chat message to {} recipient(s)",
                received.recipients.len()
            ),
        );
        self.push_event(
            "NIP-59",
            &format!("sim:{}:{:08x}", nostr::nip59::KIND_GIFT_WRAP, round),
            format!("wrapped private message event (kind {})", wrapped.kind),
        );

        let endpoint = format!("https://api.openagents.dev/v1/relay/check/{round}");
        let payload = format!(
            "{{\"challenge\":\"{}\",\"relay\":\"{}\"}}",
            challenge, self.relay_url
        );
        let payload_hash = nostr::nip98::hash_payload(payload.as_bytes());
        let http_auth =
            nostr::nip98::HttpAuth::new(endpoint.clone(), nostr::nip98::HttpMethod::Post)
                .with_payload_hash(payload_hash.clone());
        let http_auth_tags = http_auth.to_tags();
        let validation = nostr::nip98::ValidationParams::new(
            endpoint,
            nostr::nip98::HttpMethod::Post,
            now_epoch_seconds.saturating_add(4),
        )
        .with_payload_hash(payload_hash)
        .with_timestamp_window(120);
        nostr::nip98::validate_http_auth_event(
            nostr::nip98::KIND_HTTP_AUTH,
            now_epoch_seconds.saturating_add(4),
            &http_auth_tags,
            &validation,
        )
        .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-98",
            &format!("sim:{}:{:08x}", nostr::nip98::KIND_HTTP_AUTH, round),
            "validated HTTP auth request tags".to_string(),
        );

        let round_byte = (round % u32::from(u8::MAX.saturating_sub(2))).saturating_add(1) as u8;
        let mut records = vec![
            nostr::nip77::Record::new(now_epoch_seconds.saturating_add(1), [round_byte; 32]),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(2),
                [round_byte.saturating_add(1); 32],
            ),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(3),
                [round_byte.saturating_add(2); 32],
            ),
        ];
        nostr::nip77::sort_records(&mut records);
        let ids: Vec<nostr::nip77::EventId> = records.iter().map(|record| record.id).collect();
        let fingerprint = nostr::nip77::calculate_fingerprint(&ids);
        let negentropy =
            nostr::nip77::NegentropyMessage::new(vec![nostr::nip77::Range::fingerprint(
                nostr::nip77::Bound::infinity(),
                fingerprint,
            )]);
        let encoded = negentropy.encode_hex().map_err(|error| error.to_string())?;
        let decoded =
            nostr::nip77::NegentropyMessage::decode_hex(&encoded).map_err(|e| e.to_string())?;
        self.sync_ranges = self.sync_ranges.saturating_add(decoded.ranges.len() as u32);
        self.push_event(
            "NIP-77",
            &format!("sim:30077:{:08x}", round),
            format!(
                "reconciled negentropy message ({} hex chars)",
                encoded.len()
            ),
        );

        self.rounds_run = round;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: relay auth, private messaging, HTTP auth, and negentropy sync succeeded"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Relay security simulation reset".to_string());
        self.challenge = "auth-bootstrap".to_string();
        self.auth_event_id = None;
        self.rounds_run = 0;
        self.dm_relay_count = 0;
        self.sync_ranges = 0;
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto relay security simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto relay security simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsWalletMode {
    Btc,
    Usd,
}

impl StableSatsWalletMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Btc => "BTC",
            Self::Usd => "USD",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableSatsAgentWalletState {
    pub agent_name: String,
    pub btc_balance_sats: u64,
    pub usd_balance_cents: u64,
    pub active_wallet: StableSatsWalletMode,
    pub switch_count: u32,
    pub last_switch_summary: String,
}

pub struct StableSatsSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rounds_run: u32,
    pub price_usd_cents_per_btc: u64,
    pub total_converted_sats: u64,
    pub total_converted_usd_cents: u64,
    pub last_settlement_ref: Option<String>,
    pub agents: Vec<StableSatsAgentWalletState>,
    pub price_history_usd_cents_per_btc: Vec<u64>,
    pub converted_sats_history: Vec<u64>,
    pub auto_run_enabled: bool,
    pub auto_run_interval: Duration,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
    auto_run_last_tick: Option<Instant>,
}

impl Default for StableSatsSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model StableSats wallet switches (BTC <-> USD)".to_string(),
            ),
            rounds_run: 0,
            price_usd_cents_per_btc: Self::BASE_PRICE_USD_CENTS_PER_BTC,
            total_converted_sats: 0,
            total_converted_usd_cents: 0,
            last_settlement_ref: None,
            agents: Self::default_agents(),
            price_history_usd_cents_per_btc: Vec::new(),
            converted_sats_history: Vec::new(),
            auto_run_enabled: false,
            auto_run_interval: Duration::from_millis(120),
            events: Vec::new(),
            next_seq: 1,
            auto_run_last_tick: None,
        }
    }
}

impl StableSatsSimulationPaneState {
    const SATS_PER_BTC: u128 = 100_000_000;
    const BASE_PRICE_USD_CENTS_PER_BTC: u64 = 8_400_000;
    const PRICE_STEP_USD_CENTS_PER_BTC: u64 = 12_500;

    fn default_agents() -> Vec<StableSatsAgentWalletState> {
        vec![
            StableSatsAgentWalletState {
                agent_name: "agent-alpha".to_string(),
                btc_balance_sats: 260_000,
                usd_balance_cents: 42_000,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "agent-beta".to_string(),
                btc_balance_sats: 180_000,
                usd_balance_cents: 64_000,
                active_wallet: StableSatsWalletMode::Usd,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
            StableSatsAgentWalletState {
                agent_name: "agent-gamma".to_string(),
                btc_balance_sats: 120_000,
                usd_balance_cents: 36_000,
                active_wallet: StableSatsWalletMode::Btc,
                switch_count: 0,
                last_switch_summary: "none".to_string(),
            },
        ]
    }

    pub fn total_btc_balance_sats(&self) -> u64 {
        self.agents.iter().map(|agent| agent.btc_balance_sats).sum()
    }

    pub fn total_usd_balance_cents(&self) -> u64 {
        self.agents
            .iter()
            .map(|agent| agent.usd_balance_cents)
            .sum()
    }

    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        if self.agents.is_empty() {
            self.load_state = PaneLoadState::Error;
            let error = "No agents configured for StableSats simulation".to_string();
            self.last_error = Some(error.clone());
            return Err(error);
        }

        let round = self.rounds_run.saturating_add(1);
        let price = Self::BASE_PRICE_USD_CENTS_PER_BTC
            .saturating_add(u64::from(round) * Self::PRICE_STEP_USD_CENTS_PER_BTC);
        self.price_usd_cents_per_btc = price;

        let quote_ref = format!("sim:blink:price:{round:04}:{now_epoch_seconds}");
        self.push_event(
            "BLINK-PRICE",
            &quote_ref,
            format!("quoted BTC/USD at {}", Self::format_usd_cents(price)),
        );

        let mut converted_sats_round = 0_u64;
        let mut converted_usd_round = 0_u64;
        let mut agents_switched = 0_u32;

        for index in 0..self.agents.len() {
            let index_u64 = index as u64;
            let event_ref = format!("sim:blink:swap:{round:04}:{index:02}");
            let event_summary = {
                let agent = &mut self.agents[index];
                match agent.active_wallet {
                    StableSatsWalletMode::Btc => {
                        let target_sats = 6_000_u64
                            .saturating_add(u64::from(round) * 500)
                            .saturating_add(index_u64 * 400);
                        let switch_sats = target_sats.min(agent.btc_balance_sats);
                        if switch_sats == 0 {
                            None
                        } else {
                            let credited_usd = Self::sats_to_usd_cents(switch_sats, price);
                            agent.btc_balance_sats =
                                agent.btc_balance_sats.saturating_sub(switch_sats);
                            agent.usd_balance_cents =
                                agent.usd_balance_cents.saturating_add(credited_usd);
                            agent.active_wallet = StableSatsWalletMode::Usd;
                            agent.switch_count = agent.switch_count.saturating_add(1);
                            agent.last_switch_summary = format!(
                                "BTC->USD {} sats -> {}",
                                switch_sats,
                                Self::format_usd_cents(credited_usd)
                            );

                            converted_sats_round = converted_sats_round.saturating_add(switch_sats);
                            converted_usd_round = converted_usd_round.saturating_add(credited_usd);
                            agents_switched = agents_switched.saturating_add(1);
                            Some(format!(
                                "{} switched {} sats to {}",
                                agent.agent_name,
                                switch_sats,
                                Self::format_usd_cents(credited_usd)
                            ))
                        }
                    }
                    StableSatsWalletMode::Usd => {
                        let target_usd = 280_u64
                            .saturating_add(u64::from(round) * 25)
                            .saturating_add(index_u64 * 20);
                        let switch_usd = target_usd.min(agent.usd_balance_cents);
                        if switch_usd == 0 {
                            None
                        } else {
                            let credited_sats = Self::usd_cents_to_sats(switch_usd, price);
                            agent.usd_balance_cents =
                                agent.usd_balance_cents.saturating_sub(switch_usd);
                            agent.btc_balance_sats =
                                agent.btc_balance_sats.saturating_add(credited_sats);
                            agent.active_wallet = StableSatsWalletMode::Btc;
                            agent.switch_count = agent.switch_count.saturating_add(1);
                            agent.last_switch_summary = format!(
                                "USD->BTC {} -> {} sats",
                                Self::format_usd_cents(switch_usd),
                                credited_sats
                            );

                            converted_sats_round =
                                converted_sats_round.saturating_add(credited_sats);
                            converted_usd_round = converted_usd_round.saturating_add(switch_usd);
                            agents_switched = agents_switched.saturating_add(1);
                            Some(format!(
                                "{} switched {} to {} sats",
                                agent.agent_name,
                                Self::format_usd_cents(switch_usd),
                                credited_sats
                            ))
                        }
                    }
                }
            };

            if let Some(summary) = event_summary {
                self.push_event("BLINK-SWAP", &event_ref, summary);
            }
        }

        self.total_converted_sats = self
            .total_converted_sats
            .saturating_add(converted_sats_round);
        self.total_converted_usd_cents = self
            .total_converted_usd_cents
            .saturating_add(converted_usd_round);
        self.rounds_run = round;
        self.last_settlement_ref = Some(format!("sim:blink:settlement:{round:04}"));
        self.price_history_usd_cents_per_btc.push(price);
        if self.price_history_usd_cents_per_btc.len() > 18 {
            let overflow = self
                .price_history_usd_cents_per_btc
                .len()
                .saturating_sub(18);
            self.price_history_usd_cents_per_btc.drain(0..overflow);
        }
        self.converted_sats_history.push(converted_sats_round);
        if self.converted_sats_history.len() > 18 {
            let overflow = self.converted_sats_history.len().saturating_sub(18);
            self.converted_sats_history.drain(0..overflow);
        }
        if let Some(settlement_ref) = self.last_settlement_ref.clone() {
            self.push_event(
                "BLINK-LEDGER",
                &settlement_ref,
                format!(
                    "settled {} wallet switches ({} total, {} total)",
                    agents_switched,
                    self.total_converted_sats,
                    Self::format_usd_cents(self.total_converted_usd_cents)
                ),
            );
        }

        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: {} agents switched wallets at {} per BTC",
            agents_switched,
            Self::format_usd_cents(price)
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("StableSats simulation reset".to_string());
        self.rounds_run = 0;
        self.price_usd_cents_per_btc = Self::BASE_PRICE_USD_CENTS_PER_BTC;
        self.total_converted_sats = 0;
        self.total_converted_usd_cents = 0;
        self.last_settlement_ref = None;
        self.agents = Self::default_agents();
        self.price_history_usd_cents_per_btc.clear();
        self.converted_sats_history.clear();
        self.auto_run_enabled = false;
        self.events.clear();
        self.next_seq = 1;
        self.auto_run_last_tick = None;
    }

    pub fn start_auto_run(&mut self, now: Instant) {
        self.auto_run_enabled = true;
        self.auto_run_last_tick = Some(now);
        self.last_error = None;
        self.last_action = Some("Auto StableSats simulation running".to_string());
    }

    pub fn stop_auto_run(&mut self) {
        self.auto_run_enabled = false;
        self.auto_run_last_tick = None;
        self.last_action = Some("Auto StableSats simulation paused".to_string());
    }

    pub fn should_run_auto_round(&self, now: Instant) -> bool {
        if !self.auto_run_enabled {
            return false;
        }
        self.auto_run_last_tick
            .is_none_or(|last| now.duration_since(last) >= self.auto_run_interval)
    }

    pub fn mark_auto_round(&mut self, now: Instant) {
        self.auto_run_last_tick = Some(now);
    }

    fn sats_to_usd_cents(sats: u64, price_usd_cents_per_btc: u64) -> u64 {
        let numerator = u128::from(sats).saturating_mul(u128::from(price_usd_cents_per_btc));
        ((numerator + (Self::SATS_PER_BTC / 2)) / Self::SATS_PER_BTC) as u64
    }

    fn usd_cents_to_sats(usd_cents: u64, price_usd_cents_per_btc: u64) -> u64 {
        if price_usd_cents_per_btc == 0 {
            return 0;
        }
        let numerator = u128::from(usd_cents).saturating_mul(Self::SATS_PER_BTC);
        ((numerator + (u128::from(price_usd_cents_per_btc) / 2))
            / u128::from(price_usd_cents_per_btc)) as u64
    }

    fn format_usd_cents(usd_cents: u64) -> String {
        format!("${}.{:02}", usd_cents / 100, usd_cents % 100)
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 24 {
            let overflow = self.events.len().saturating_sub(24);
            self.events.drain(0..overflow);
        }
    }
}
