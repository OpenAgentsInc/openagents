//! Tick Executor
//!
//! Executes a single tick cycle for a sovereign agent:
//! 1. Perceive (fetch observations - mentions, DMs, zaps)
//! 2. Think (request compute, pay for it)
//! 3. Act (execute actions from LLM response)
//! 4. Update state
//! 5. Publish trajectory for transparency

use super::compute::ComputeClient;
use super::state::StateManager;
use super::trajectory::TrajectoryPublisher;
use crate::agents::SharedRelay;
use agent::{LifecycleManager, LifecycleState, RunwayAnalysis};
use anyhow::{anyhow, Result};
use bech32::{Bech32, Hrp};
use compute::domain::UnifiedIdentity;
use nostr::nip_sa::{
    AgentStateContent, TickAction as NipSaTickAction, TickRequest, TickResult as NipSaTickResult,
    TickResultContent, TickStatus, TickTrigger as NipSaTickTrigger, KIND_TICK_REQUEST,
    KIND_TICK_RESULT,
};
use nostr::{
    decode, decrypt, decrypt_v2, encrypt, finalize_event, ChannelMessageEvent, Event,
    EventTemplate, Nip19Entity, KIND_CHANNEL_MESSAGE, ZAP_REQUEST_KIND,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// What triggered this tick
#[derive(Debug, Clone)]
pub enum TickTrigger {
    /// Scheduled heartbeat timer
    Heartbeat,
    /// Agent was mentioned in a note
    Mention(Event),
    /// Agent received a DM
    DirectMessage(Event),
    /// Agent received a NIP-28 channel message
    ChannelMessage(Event),
    /// Agent received a zap
    Zap(Event),
}

/// Result of a tick execution
#[derive(Debug)]
pub struct TickResult {
    /// Tick number
    pub tick_number: u64,
    /// What triggered this tick
    pub trigger: TickTrigger,
    /// Observations gathered
    pub observations: Vec<Event>,
    /// LLM reasoning output
    pub reasoning: String,
    /// Actions taken
    pub actions: Vec<TickAction>,
    /// Updated lifecycle state
    pub lifecycle_state: LifecycleState,
    /// Runway analysis
    pub runway: RunwayAnalysis,
    /// Sats spent on compute
    pub compute_cost_sats: u64,
    /// Trajectory hash for verification (SHA-256)
    pub trajectory_hash: Option<String>,
}

/// Action taken by the agent
#[derive(Debug, Clone)]
pub enum TickAction {
    /// Post a note
    Post { content: String },
    /// Send a DM
    DirectMessage { recipient: String, content: String },
    /// Send a message to a channel (optional channel ID overrides default)
    ChannelMessage {
        channel_id: Option<String>,
        content: String,
    },
    /// Send a zap
    Zap { target: String, amount_sats: u64 },
    /// Pay a Lightning invoice
    PayInvoice { bolt11: String },
    /// Request a payment by sending an invoice
    RequestPayment {
        recipient: String,
        amount_sats: u64,
        memo: Option<String>,
    },
    /// Update a goal
    UpdateGoal { goal_id: String, progress: f64 },
    /// Add a memory
    AddMemory { memory_type: String, content: String },
    /// No action taken
    None,
}

impl TickAction {
    /// Get the action type as a string
    pub fn action_type_str(&self) -> &'static str {
        match self {
            TickAction::Post { .. } => "post",
            TickAction::DirectMessage { .. } => "dm",
            TickAction::ChannelMessage { .. } => "channel_message",
            TickAction::Zap { .. } => "zap",
            TickAction::PayInvoice { .. } => "pay_invoice",
            TickAction::RequestPayment { .. } => "request_payment",
            TickAction::UpdateGoal { .. } => "update_goal",
            TickAction::AddMemory { .. } => "add_memory",
            TickAction::None => "none",
        }
    }

    /// Convert action to JSON value for trajectory
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            TickAction::Post { content } => {
                serde_json::json!({
                    "content_preview": content.chars().take(100).collect::<String>()
                })
            }
            TickAction::DirectMessage { recipient, content } => {
                serde_json::json!({
                    "recipient": recipient,
                    "content_preview": content.chars().take(100).collect::<String>()
                })
            }
            TickAction::ChannelMessage { channel_id, content } => {
                serde_json::json!({
                    "channel_id": channel_id,
                    "content_preview": content.chars().take(100).collect::<String>()
                })
            }
            TickAction::Zap { target, amount_sats } => {
                serde_json::json!({
                    "target": target,
                    "amount_sats": amount_sats
                })
            }
            TickAction::PayInvoice { bolt11 } => {
                serde_json::json!({
                    "bolt11_preview": bolt11.chars().take(16).collect::<String>()
                })
            }
            TickAction::RequestPayment {
                recipient,
                amount_sats,
                memo,
            } => {
                serde_json::json!({
                    "recipient": recipient,
                    "amount_sats": amount_sats,
                    "memo": memo
                })
            }
            TickAction::UpdateGoal { goal_id, progress } => {
                serde_json::json!({
                    "goal_id": goal_id,
                    "progress": progress
                })
            }
            TickAction::AddMemory { memory_type, content } => {
                serde_json::json!({
                    "memory_type": memory_type,
                    "content_preview": content.chars().take(100).collect::<String>()
                })
            }
            TickAction::None => {
                serde_json::json!({})
            }
        }
    }
}

/// Tick executor for a sovereign agent
pub struct TickExecutor {
    identity: UnifiedIdentity,
    state_manager: StateManager,
    compute_client: ComputeClient,
    lifecycle_manager: LifecycleManager,
    trajectory_publisher: TrajectoryPublisher,
    relay: SharedRelay,
    pubkey: String,
    agent_name: String,
    channel_id: Option<String>,
}

impl TickExecutor {
    /// Create a new tick executor
    pub fn new(
        identity: UnifiedIdentity,
        state_manager: StateManager,
        compute_client: ComputeClient,
        relay: SharedRelay,
        trajectory_relay: SharedRelay,
        pubkey: String,
        agent_name: String,
        channel_id: Option<String>,
    ) -> Self {
        let trajectory_publisher = TrajectoryPublisher::new(identity.clone(), trajectory_relay);
        Self {
            identity,
            state_manager,
            compute_client,
            lifecycle_manager: LifecycleManager::with_state(LifecycleState::Active),
            trajectory_publisher,
            relay,
            pubkey,
            agent_name,
            channel_id,
        }
    }

    /// Execute a single tick
    pub async fn execute_tick(&mut self, trigger: TickTrigger) -> Result<TickResult> {
        let tick_start = Instant::now();

        // 1. Fetch current state
        let mut state = self.state_manager.get_or_create_state().await?;
        let tick_number = state.tick_count + 1;
        let tick_id = format!("tick-{}-{}", self.pubkey, tick_number);

        tracing::info!("[{}] Starting tick #{}", self.agent_name, tick_number);

        if let Err(e) = self.compute_client.refresh_wallet_balance(&mut state).await {
            tracing::warn!("[{}] Failed to refresh wallet balance: {}", self.agent_name, e);
        }

        let tick_request_id = match self.publish_tick_request(&trigger).await {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::warn!("[{}] Failed to publish tick request: {}", self.agent_name, e);
                None
            }
        };

        // 2. Check wallet balance and update lifecycle
        let balance_sats = state.wallet_balance_sats;
        let runway = self.lifecycle_manager.analyze_runway(balance_sats);

        // Update lifecycle state based on balance
        if let Err(e) = self
            .lifecycle_manager
            .update_from_balance(balance_sats)
        {
            tracing::warn!("[{}] Lifecycle transition error: {}", self.agent_name, e);
        }

        let lifecycle_state = self.lifecycle_manager.current_state().clone();

        // Check if we should tick
        if !self.lifecycle_manager.should_tick(balance_sats) {
            tracing::info!(
                "[{}] Skipping tick - insufficient funds or hibernating",
                self.agent_name
            );
            let reasoning = "Skipped - insufficient funds".to_string();
            let actions = vec![TickAction::None];

            if let Some(request_id) = tick_request_id.as_ref() {
                let duration_ms = tick_start.elapsed().as_millis() as u64;
                if let Err(e) = self
                    .publish_tick_result_event(
                        request_id,
                        TickStatus::Success,
                        &actions,
                        "",
                        &reasoning,
                        duration_ms.max(1),
                        None,
                    )
                    .await
                {
                    tracing::warn!(
                        "[{}] Failed to publish tick result: {}",
                        self.agent_name,
                        e
                    );
                }
            }

            return Ok(TickResult {
                tick_number,
                trigger,
                observations: vec![],
                reasoning,
                actions,
                lifecycle_state,
                runway,
                compute_cost_sats: 0,
                trajectory_hash: None,
            });
        }

        // 3. Start trajectory session for transparency
        if let Err(e) = self
            .trajectory_publisher
            .start_session(&tick_id, "claude")
            .await
        {
            tracing::warn!("[{}] Failed to start trajectory session: {}", self.agent_name, e);
        }

        // 4. Gather observations
        let observations = self.gather_observations(&trigger).await?;
        tracing::info!(
            "[{}] Gathered {} observations",
            self.agent_name,
            observations.len()
        );

        // Record observations in trajectory
        if let Err(e) = self.trajectory_publisher.record_observations(&observations).await {
            tracing::warn!("[{}] Failed to record observations: {}", self.agent_name, e);
        }

        // 5. Build prompt for reasoning
        let prompt = self.build_reasoning_prompt(&state, &trigger, &observations);

        // 6. Discover providers and request compute
        let providers = self.compute_client.discover_providers(3).await?;
        if providers.is_empty() {
            // End trajectory session even on error
            let _trajectory_hash = self.trajectory_publisher.end_session().await.ok();
            let failure_reason = "Failed - no compute providers available".to_string();

            if let Some(request_id) = tick_request_id.as_ref() {
                let duration_ms = tick_start.elapsed().as_millis() as u64;
                let actions = vec![TickAction::None];
                if let Err(e) = self
                    .publish_tick_result_event(
                        request_id,
                        TickStatus::Failure,
                        &actions,
                        &prompt,
                        &failure_reason,
                        duration_ms.max(1),
                        None,
                    )
                    .await
                {
                    tracing::warn!(
                        "[{}] Failed to publish tick result: {}",
                        self.agent_name,
                        e
                    );
                }
            }

            return Err(anyhow!("No compute providers available"));
        }

        // Select cheapest provider within budget
        let budget_sats = state
            .budget
            .as_ref()
            .map(|b| b.limits.per_tick_limit_sats)
            .unwrap_or(1000);

        let provider = ComputeClient::select_cheapest_provider(&providers, budget_sats)
            .ok_or_else(|| anyhow!("No provider within budget {} sats", budget_sats))?;

        tracing::info!(
            "[{}] Selected provider: {} ({} msats)",
            self.agent_name,
            provider.name,
            provider.price_msats
        );

        // Record tool use (compute request) in trajectory
        if let Err(e) = self
            .trajectory_publisher
            .record_tool_use(
                "compute_request",
                serde_json::json!({
                    "provider": provider.name,
                    "price_msats": provider.price_msats,
                    "budget_sats": budget_sats
                }),
            )
            .await
        {
            tracing::warn!("[{}] Failed to record tool use: {}", self.agent_name, e);
        }

        // 7. Request inference and PAY for it
        let reasoning = match self
            .compute_client
            .request_inference(&provider, &prompt, 500, budget_sats)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                if let Some(request_id) = tick_request_id.as_ref() {
                    let duration_ms = tick_start.elapsed().as_millis() as u64;
                    let actions = vec![TickAction::None];
                    let failure_reason = format!("Failed - compute request error: {}", e);
                    if let Err(err) = self
                        .publish_tick_result_event(
                            request_id,
                            TickStatus::Failure,
                            &actions,
                            &prompt,
                            &failure_reason,
                            duration_ms.max(1),
                            None,
                        )
                        .await
                    {
                        tracing::warn!(
                            "[{}] Failed to publish tick result: {}",
                            self.agent_name,
                            err
                        );
                    }
                }

                return Err(e);
            }
        };

        let compute_cost_sats = provider.price_msats / 1000;

        // Record tool result in trajectory
        if let Err(e) = self
            .trajectory_publisher
            .record_tool_result(
                "compute_request",
                serde_json::json!({
                    "tokens_estimated": reasoning.len() / 4,
                    "cost_sats": compute_cost_sats
                }),
                true,
            )
            .await
        {
            tracing::warn!("[{}] Failed to record tool result: {}", self.agent_name, e);
        }

        // Record thinking (redacted) in trajectory
        if let Err(e) = self.trajectory_publisher.record_thinking(&reasoning).await {
            tracing::warn!("[{}] Failed to record thinking: {}", self.agent_name, e);
        }

        // 8. Parse actions from reasoning
        let actions = self.parse_actions(&reasoning);

        // 9. Execute actions and record in trajectory
        for action in &actions {
            // Record action in trajectory
            if let Err(e) = self
                .trajectory_publisher
                .record_action(&action.action_type_str(), action.to_json_value())
                .await
            {
                tracing::warn!("[{}] Failed to record action: {}", self.agent_name, e);
            }

            if let Err(e) = self.execute_action(action).await {
                tracing::warn!("[{}] Action failed: {}", self.agent_name, e);
            }
        }

        // 10. Update state
        state.record_tick(chrono::Utc::now().timestamp() as u64);
        state.record_spend(compute_cost_sats);

        // Add memory of this tick
        state.memory.push(nostr::nip_sa::MemoryEntry::new(
            "tick",
            format!("Tick #{}: {}", tick_number, reasoning.chars().take(100).collect::<String>()),
        ));

        // 11. Publish updated state
        self.state_manager.publish_state(&state).await?;

        // 12. End trajectory session and get hash
        let trajectory_hash = match self.trajectory_publisher.end_session().await {
            Ok(hash) => {
                tracing::debug!(
                    "[{}] Trajectory hash: {}",
                    self.agent_name,
                    &hash[..16]
                );
                Some(hash)
            }
            Err(e) => {
                tracing::warn!("[{}] Failed to end trajectory session: {}", self.agent_name, e);
                None
            }
        };

        tracing::info!(
            "[{}] Tick #{} complete. Cost: {} sats, New balance: {} sats",
            self.agent_name,
            tick_number,
            compute_cost_sats,
            state.wallet_balance_sats
        );

        if let Some(request_id) = tick_request_id.as_ref() {
            let duration_ms = tick_start.elapsed().as_millis() as u64;
            if let Err(e) = self
                .publish_tick_result_event(
                    request_id,
                    TickStatus::Success,
                    &actions,
                    &prompt,
                    &reasoning,
                    duration_ms.max(1),
                    trajectory_hash.as_deref(),
                )
                .await
            {
                tracing::warn!(
                    "[{}] Failed to publish tick result: {}",
                    self.agent_name,
                    e
                );
            }
        }

        Ok(TickResult {
            tick_number,
            trigger,
            observations,
            reasoning,
            actions,
            lifecycle_state,
            runway,
            compute_cost_sats,
            trajectory_hash,
        })
    }

    /// Gather observations based on trigger
    async fn gather_observations(&self, trigger: &TickTrigger) -> Result<Vec<Event>> {
        let mut observations = Vec::new();

        // Add the triggering event if applicable
        match trigger {
            TickTrigger::Mention(event) => observations.push(event.clone()),
            TickTrigger::DirectMessage(event) => observations.push(event.clone()),
            TickTrigger::ChannelMessage(event) => observations.push(event.clone()),
            TickTrigger::Zap(event) => observations.push(event.clone()),
            TickTrigger::Heartbeat => {}
        }

        // Fetch recent mentions
        let now = chrono::Utc::now().timestamp() as u64;
        let since = now.saturating_sub(900); // Last 15 minutes

        let filters = vec![serde_json::json!({
            "kinds": [1],
            "#p": [self.pubkey],
            "since": since,
            "limit": 10
        })];

        // Subscribe and collect events
        if let Ok(mut rx) = self
            .relay
            .subscribe_with_channel(&format!("observations-{}", Uuid::new_v4()), &filters)
            .await
        {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);

            while std::time::Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                match tokio::time::timeout(remaining.max(Duration::from_millis(100)), rx.recv()).await
                {
                    Ok(Some(event)) => {
                        if !observations.iter().any(|e| e.id == event.id) {
                            observations.push(event);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }

        Ok(observations)
    }

    /// Build the reasoning prompt for the LLM
    fn build_reasoning_prompt(
        &self,
        state: &AgentStateContent,
        trigger: &TickTrigger,
        observations: &[Event],
    ) -> String {
        let mut prompt = format!(
            "You are {}, a sovereign AI agent operating on Nostr.\n\n",
            self.agent_name
        );

        // Add goals
        if !state.goals.is_empty() {
            prompt.push_str("## Your Current Goals:\n");
            for goal in &state.goals {
                prompt.push_str(&format!(
                    "- {} (priority: {}, progress: {:.0}%)\n",
                    goal.description,
                    goal.priority,
                    goal.progress * 100.0
                ));
            }
            prompt.push('\n');
        }

        // Add trigger context
        prompt.push_str("## Trigger:\n");
        match trigger {
            TickTrigger::Heartbeat => prompt.push_str("Scheduled heartbeat tick.\n"),
            TickTrigger::Mention(_) => prompt.push_str("Someone mentioned you.\n"),
            TickTrigger::DirectMessage(_) => prompt.push_str("You received a direct message.\n"),
            TickTrigger::ChannelMessage(_) => {
                prompt.push_str("You received a channel message.\n")
            }
            TickTrigger::Zap(_) => prompt.push_str("You received a zap (Bitcoin payment).\n"),
        }
        prompt.push('\n');

        // Add observations
        if !observations.is_empty() {
            prompt.push_str("## Recent Observations:\n");
            for (i, event) in observations.iter().take(5).enumerate() {
                let summary = self.format_observation(event);
                prompt.push_str(&format!("{}. {}\n", i + 1, summary));
            }
            prompt.push('\n');
        }

        // Add recent memories
        if !state.memory.is_empty() {
            prompt.push_str("## Recent Memories:\n");
            for entry in state.memory.iter().rev().take(5) {
                prompt.push_str(&format!(
                    "- [{}] {}\n",
                    entry.memory_type,
                    entry.content.chars().take(100).collect::<String>()
                ));
            }
            prompt.push('\n');
        }

        // Add wallet status
        prompt.push_str(&format!(
            "## Wallet: {} sats\n\n",
            state.wallet_balance_sats
        ));

        // Instructions
        prompt.push_str(
            "Respond with action lines using the following formats:\n\
            POST: <text>\n\
            DM: <npub_or_hex> | <message>\n\
            CHANNEL: [channel_id |] <message>\n\
            PAY_INVOICE: <bolt11>\n\
            REQUEST_PAYMENT: <npub_or_hex|channel[:id]> | <amount_sats> | <memo optional>\n\
            ZAP: <npub_or_note/nevent> | <amount_sats>\n\
            NOTHING\n\n\
            Include a brief reasoning above the action lines.\n",
        );

        prompt
    }

    /// Parse actions from LLM reasoning
    fn parse_actions(&self, reasoning: &str) -> Vec<TickAction> {
        let mut actions = Vec::new();

        for line in reasoning.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let lower = trimmed.to_lowercase();

            if lower.starts_with("post:") {
                let content = trimmed[5..].trim();
                if !content.is_empty() {
                    actions.push(TickAction::Post {
                        content: content.to_string(),
                    });
                }
                continue;
            }

            if lower.starts_with("dm:") {
                let payload = trimmed[3..].trim();
                if let Some((recipient, message)) = payload.split_once('|') {
                    let recipient = recipient.trim();
                    let message = message.trim();
                    if !recipient.is_empty() && !message.is_empty() {
                        actions.push(TickAction::DirectMessage {
                            recipient: recipient.to_string(),
                            content: message.to_string(),
                        });
                    }
                }
                continue;
            }

            if lower.starts_with("channel:") {
                let payload = trimmed[8..].trim();
                let mut parts = payload.splitn(2, '|');
                let first = parts.next().unwrap_or("").trim();
                if let Some(second) = parts.next() {
                    let content = second.trim();
                    if !content.is_empty() {
                        let channel_id = if first.is_empty() {
                            None
                        } else {
                            Some(first.to_string())
                        };
                        actions.push(TickAction::ChannelMessage {
                            channel_id,
                            content: content.to_string(),
                        });
                    }
                } else if !first.is_empty() {
                    actions.push(TickAction::ChannelMessage {
                        channel_id: None,
                        content: first.to_string(),
                    });
                }
                continue;
            }

            if lower.starts_with("pay_invoice:") {
                let bolt11 = trimmed["pay_invoice:".len()..].trim();
                if !bolt11.is_empty() {
                    actions.push(TickAction::PayInvoice {
                        bolt11: bolt11.to_string(),
                    });
                }
                continue;
            }

            if lower.starts_with("request_payment:") {
                let payload = trimmed["request_payment:".len()..].trim();
                let mut parts = payload.split('|').map(str::trim);
                let recipient = parts.next().unwrap_or("");
                let amount = parts.next().unwrap_or("");
                let memo = parts.next().map(|value| value.to_string()).filter(|s| !s.is_empty());

                if !recipient.is_empty() {
                    if let Ok(amount_sats) = amount.parse::<u64>() {
                        actions.push(TickAction::RequestPayment {
                            recipient: recipient.to_string(),
                            amount_sats,
                            memo,
                        });
                    }
                }
                continue;
            }

            if lower.starts_with("zap:") {
                let payload = trimmed[4..].trim();
                if let Some((target, amount)) = payload.split_once('|') {
                    let target = target.trim();
                    let amount = amount.trim();
                    if !target.is_empty() {
                        if let Ok(amount_sats) = amount.parse::<u64>() {
                            actions.push(TickAction::Zap {
                                target: target.to_string(),
                                amount_sats,
                            });
                        }
                    }
                }
                continue;
            }

            if lower == "nothing" || lower.contains("no action") {
                continue;
            }
        }

        if actions.is_empty() {
            actions.push(TickAction::None);
        }

        actions
    }

    fn format_observation(&self, event: &Event) -> String {
        match event.kind {
            4 => {
                let peer = self
                    .dm_peer_pubkey(event)
                    .unwrap_or_else(|| event.pubkey.clone());
                let content = self
                    .decrypt_direct_message(event)
                    .unwrap_or_else(|| "<encrypted>".to_string());
                format!(
                    "[kind:4] DM from {}: {}",
                    self.short_label(&peer),
                    content.chars().take(200).collect::<String>()
                )
            }
            KIND_CHANNEL_MESSAGE => {
                let channel_id = event
                    .tags
                    .iter()
                    .find(|tag| tag.len() >= 2 && tag[0] == "e")
                    .map(|tag| tag[1].clone());
                let content = event.content.chars().take(200).collect::<String>();
                if let Some(id) = channel_id {
                    format!(
                        "[kind:{}] Channel {}: {}",
                        event.kind,
                        self.short_label(&id),
                        content
                    )
                } else {
                    format!("[kind:{}] Channel: {}", event.kind, content)
                }
            }
            _ => format!(
                "[kind:{}] {}",
                event.kind,
                event.content.chars().take(200).collect::<String>()
            ),
        }
    }

    fn short_label(&self, value: &str) -> String {
        if value.len() <= 12 {
            value.to_string()
        } else {
            format!("{}...{}", &value[..8], &value[value.len() - 4..])
        }
    }

    fn dm_peer_pubkey(&self, event: &Event) -> Option<String> {
        if event.pubkey == self.pubkey {
            event
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "p")
                .map(|tag| tag[1].clone())
        } else {
            Some(event.pubkey.clone())
        }
    }

    fn decrypt_direct_message(&self, event: &Event) -> Option<String> {
        let peer_pubkey = self.dm_peer_pubkey(event)?;
        let compressed = self.compress_pubkey_hex(&peer_pubkey).ok()?;

        if let Ok(plaintext) = decrypt(self.identity.private_key_bytes(), &compressed, &event.content)
        {
            return Some(plaintext);
        }

        if let Ok(plaintext) =
            decrypt_v2(self.identity.private_key_bytes(), &compressed, &event.content)
        {
            return Some(plaintext);
        }

        None
    }

    fn parse_pubkey_input(&self, value: &str) -> Result<String> {
        let trimmed = value.trim();
        if trimmed.to_lowercase().starts_with("npub") {
            match decode(trimmed) {
                Ok(Nip19Entity::Pubkey(bytes)) => Ok(hex::encode(bytes)),
                Ok(_) => Err(anyhow!("Expected npub, got different entity type")),
                Err(e) => Err(anyhow!("Invalid npub: {}", e)),
            }
        } else {
            let bytes =
                hex::decode(trimmed).map_err(|e| anyhow!("Invalid pubkey hex: {}", e))?;
            if bytes.len() != 32 {
                return Err(anyhow!("Invalid pubkey length: {}", bytes.len()));
            }
            Ok(trimmed.to_lowercase())
        }
    }

    fn compress_pubkey_hex(&self, pubkey_hex: &str) -> Result<Vec<u8>> {
        let bytes = hex::decode(pubkey_hex).map_err(|e| anyhow!("Invalid pubkey hex: {}", e))?;
        if bytes.len() != 32 {
            return Err(anyhow!("Invalid pubkey length: {}", bytes.len()));
        }
        let mut compressed = Vec::with_capacity(33);
        compressed.push(0x02);
        compressed.extend_from_slice(&bytes);
        Ok(compressed)
    }

    fn estimate_tokens(&self, text: &str) -> u64 {
        (text.len() as u64) / 4
    }

    fn to_nip_sa_trigger(&self, trigger: &TickTrigger) -> NipSaTickTrigger {
        match trigger {
            TickTrigger::Heartbeat => NipSaTickTrigger::Heartbeat,
            TickTrigger::Mention(_) => NipSaTickTrigger::Mention,
            TickTrigger::DirectMessage(_) => NipSaTickTrigger::Dm,
            TickTrigger::ChannelMessage(_) => NipSaTickTrigger::Manual,
            TickTrigger::Zap(_) => NipSaTickTrigger::Zap,
        }
    }

    fn to_nip_sa_action(&self, action: &TickAction) -> NipSaTickAction {
        match action {
            TickAction::Post { content } => NipSaTickAction::new("post").with_metadata(
                "content_preview",
                serde_json::json!(content.chars().take(100).collect::<String>()),
            ),
            TickAction::DirectMessage { recipient, content } => NipSaTickAction::new("dm")
                .with_metadata("recipient", serde_json::json!(recipient))
                .with_metadata(
                    "content_preview",
                    serde_json::json!(content.chars().take(100).collect::<String>()),
                ),
            TickAction::ChannelMessage { channel_id, content } => {
                let mut action = NipSaTickAction::new("channel_message").with_metadata(
                    "content_preview",
                    serde_json::json!(content.chars().take(100).collect::<String>()),
                );
                if let Some(id) = channel_id {
                    action = action.with_metadata("channel_id", serde_json::json!(id));
                }
                action
            }
            TickAction::Zap {
                target,
                amount_sats,
            } => NipSaTickAction::new("zap")
                .with_metadata("target", serde_json::json!(target))
                .with_metadata("amount_sats", serde_json::json!(amount_sats)),
            TickAction::PayInvoice { bolt11 } => NipSaTickAction::new("pay_invoice")
                .with_metadata("bolt11_preview", serde_json::json!(&bolt11[..16.min(bolt11.len())])),
            TickAction::RequestPayment {
                recipient,
                amount_sats,
                memo,
            } => {
                let mut action = NipSaTickAction::new("request_payment")
                    .with_metadata("recipient", serde_json::json!(recipient))
                    .with_metadata("amount_sats", serde_json::json!(amount_sats));
                if let Some(memo) = memo {
                    action = action.with_metadata("memo", serde_json::json!(memo));
                }
                action
            }
            TickAction::UpdateGoal { goal_id, progress } => NipSaTickAction::new("update_goal")
                .with_metadata("goal_id", serde_json::json!(goal_id))
                .with_metadata("progress", serde_json::json!(progress)),
            TickAction::AddMemory {
                memory_type,
                content,
            } => NipSaTickAction::new("add_memory")
                .with_metadata("memory_type", serde_json::json!(memory_type))
                .with_metadata(
                    "content_preview",
                    serde_json::json!(content.chars().take(100).collect::<String>()),
                ),
            TickAction::None => NipSaTickAction::new("none"),
        }
    }

    async fn publish_tick_request(&self, trigger: &TickTrigger) -> Result<String> {
        let request = TickRequest::new(self.pubkey.clone(), self.to_nip_sa_trigger(trigger));
        let now = chrono::Utc::now().timestamp() as u64;

        let template = EventTemplate {
            created_at: now,
            kind: KIND_TICK_REQUEST,
            tags: request.build_tags(),
            content: String::new(),
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign tick request: {}", e))?;

        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await
            .map_err(|e| anyhow!("Failed to publish tick request: {}", e))?;

        Ok(event.id.clone())
    }

    async fn publish_tick_result_event(
        &self,
        request_id: &str,
        status: TickStatus,
        actions: &[TickAction],
        prompt: &str,
        reasoning: &str,
        duration_ms: u64,
        trajectory_hash: Option<&str>,
    ) -> Result<String> {
        let tokens_in = self.estimate_tokens(prompt);
        let tokens_out = self.estimate_tokens(reasoning);
        let goals_updated = actions
            .iter()
            .filter(|action| matches!(action, TickAction::UpdateGoal { .. }))
            .count() as u32;

        let nip_actions = actions
            .iter()
            .map(|action| self.to_nip_sa_action(action))
            .collect::<Vec<_>>();

        let content =
            TickResultContent::new(tokens_in, tokens_out, 0.0, goals_updated).with_actions(
                nip_actions,
            );

        let mut result =
            NipSaTickResult::new(request_id, self.pubkey.clone(), status, duration_ms, content);

        if let Some(hash) = trajectory_hash {
            result = result.with_trajectory_hash(hash);
        }

        let now = chrono::Utc::now().timestamp() as u64;
        let template = EventTemplate {
            created_at: now,
            kind: KIND_TICK_RESULT,
            tags: result.build_tags(),
            content: result
                .content
                .to_json()
                .map_err(|e| anyhow!("Failed to serialize tick result: {}", e))?,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign tick result: {}", e))?;

        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await
            .map_err(|e| anyhow!("Failed to publish tick result: {}", e))?;

        Ok(event.id.clone())
    }

    async fn fetch_latest_event(
        &self,
        subscription_prefix: &str,
        filters: Vec<serde_json::Value>,
        timeout: Duration,
    ) -> Result<Option<Event>> {
        let subscription_id = format!("{}-{}", subscription_prefix, Uuid::new_v4());
        let mut rx = self
            .relay
            .subscribe_with_channel(&subscription_id, &filters)
            .await?;

        let mut events = Vec::new();
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(remaining.max(Duration::from_millis(100)), rx.recv()).await
            {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        Ok(events.into_iter().max_by_key(|event| event.created_at))
    }

    async fn fetch_profile_event(&self, pubkey: &str) -> Result<Event> {
        let filters = vec![serde_json::json!({
            "kinds": [0],
            "authors": [pubkey],
            "limit": 1
        })];

        self.fetch_latest_event("profile-fetch", filters, Duration::from_secs(5))
            .await?
            .ok_or_else(|| anyhow!("No profile found for {}", pubkey))
    }

    async fn fetch_event_by_id(&self, event_id: &str) -> Result<Event> {
        let filters = vec![serde_json::json!({
            "ids": [event_id],
            "limit": 1
        })];

        self.fetch_latest_event("event-fetch", filters, Duration::from_secs(5))
            .await?
            .ok_or_else(|| anyhow!("Event {} not found on relays", event_id))
    }

    fn build_zap_request_event(
        &self,
        recipient_pubkey: &str,
        amount_msats: u64,
        lnurl: &str,
        zapped_event: Option<&Event>,
    ) -> Result<Event> {
        let relays = self.relay.relay_urls();
        if relays.is_empty() {
            return Err(anyhow!("No relays configured for zap receipts"));
        }

        let mut tags = Vec::new();
        let mut relay_tag = Vec::with_capacity(relays.len() + 1);
        relay_tag.push("relays".to_string());
        relay_tag.extend(relays);
        tags.push(relay_tag);
        tags.push(vec!["amount".to_string(), amount_msats.to_string()]);
        tags.push(vec!["lnurl".to_string(), lnurl.to_string()]);
        tags.push(vec!["p".to_string(), recipient_pubkey.to_string()]);

        if let Some(event) = zapped_event {
            tags.push(vec!["e".to_string(), event.id.clone()]);
            tags.push(vec!["k".to_string(), event.kind.to_string()]);
        }

        let template = EventTemplate {
            created_at: chrono::Utc::now().timestamp() as u64,
            kind: ZAP_REQUEST_KIND,
            tags,
            content: String::new(),
        };

        finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign zap request: {}", e))
    }

    async fn execute_channel_message(&self, channel_id: Option<&str>, content: &str) -> Result<()> {
        let channel_id = channel_id
            .or_else(|| self.channel_id.as_deref())
            .ok_or_else(|| anyhow!("No channel ID configured for channel message"))?;

        let relay_url = self.relay.relay_url();
        let now = chrono::Utc::now().timestamp() as u64;
        let channel_msg = ChannelMessageEvent::new(channel_id, &relay_url, content, now);

        let template = EventTemplate {
            created_at: now,
            kind: KIND_CHANNEL_MESSAGE,
            tags: channel_msg.to_tags(),
            content: content.to_string(),
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign channel message: {}", e))?;

        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await
            .map_err(|e| anyhow!("Failed to publish channel message: {}", e))?;

        Ok(())
    }

    async fn execute_pay_invoice(&self, bolt11: &str) -> Result<()> {
        self.compute_client.pay_invoice(bolt11, None).await?;
        Ok(())
    }

    async fn execute_request_payment(
        &self,
        recipient: &str,
        amount_sats: u64,
        memo: Option<&str>,
    ) -> Result<()> {
        let invoice = self
            .compute_client
            .create_invoice(amount_sats, memo.map(|value| value.to_string()), None)
            .await?;

        let message = if let Some(memo) = memo {
            format!("{} Invoice: {}", memo, invoice)
        } else {
            format!("Invoice for {} sats: {}", amount_sats, invoice)
        };

        if let Some(channel_id) = recipient.strip_prefix("channel:") {
            self.execute_channel_message(Some(channel_id.trim()), &message)
                .await?;
        } else if recipient == "channel" {
            self.execute_channel_message(None, &message).await?;
        } else {
            self.execute_dm(recipient, &message).await?;
        }

        Ok(())
    }

    /// Execute a single action
    async fn execute_action(&self, action: &TickAction) -> Result<()> {
        match action {
            TickAction::Post { content } => {
                tracing::info!("[{}] Posting: {}", self.agent_name, content);
                self.execute_post(content).await?;
            }
            TickAction::DirectMessage { recipient, content } => {
                tracing::info!(
                    "[{}] DM to {}: {}",
                    self.agent_name,
                    recipient,
                    content
                );
                self.execute_dm(recipient, content).await?;
            }
            TickAction::ChannelMessage { channel_id, content } => {
                tracing::info!(
                    "[{}] Channel message: {}",
                    self.agent_name,
                    content
                );
                self.execute_channel_message(channel_id.as_deref(), content).await?;
            }
            TickAction::Zap { target, amount_sats } => {
                tracing::info!(
                    "[{}] Zapping {} with {} sats",
                    self.agent_name,
                    target,
                    amount_sats
                );
                self.execute_zap(target, *amount_sats).await?;
            }
            TickAction::PayInvoice { bolt11 } => {
                let preview = if bolt11.len() > 16 {
                    &bolt11[..16]
                } else {
                    bolt11
                };
                tracing::info!("[{}] Paying invoice {}", self.agent_name, preview);
                self.execute_pay_invoice(bolt11).await?;
            }
            TickAction::RequestPayment {
                recipient,
                amount_sats,
                memo,
            } => {
                tracing::info!(
                    "[{}] Requesting {} sats from {}",
                    self.agent_name,
                    amount_sats,
                    recipient
                );
                self.execute_request_payment(recipient, *amount_sats, memo.as_deref())
                    .await?;
            }
            TickAction::UpdateGoal { goal_id, progress } => {
                tracing::info!(
                    "[{}] Updating goal {} to {}%",
                    self.agent_name,
                    goal_id,
                    progress * 100.0
                );
            }
            TickAction::AddMemory {
                memory_type,
                content,
            } => {
                tracing::info!(
                    "[{}] Adding memory [{}]: {}",
                    self.agent_name,
                    memory_type,
                    content
                );
            }
            TickAction::None => {
                tracing::debug!("[{}] No action taken", self.agent_name);
            }
        }

        Ok(())
    }

    /// Post a note to Nostr (kind:1)
    async fn execute_post(&self, content: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template = EventTemplate {
            created_at: now,
            kind: 1, // Short text note
            tags: vec![],
            content: content.to_string(),
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign event: {}", e))?;

        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await
            .map_err(|e| anyhow!("Failed to publish event: {}", e))?;

        tracing::info!("[{}] Posted event: {}", self.agent_name, &event.id[..16]);
        Ok(())
    }

    /// Send an encrypted direct message (NIP-04, kind:4)
    async fn execute_dm(&self, recipient: &str, content: &str) -> Result<()> {
        // Parse recipient pubkey
        let recipient_pubkey = self.parse_pubkey_input(recipient)?;

        // Get recipient pubkey bytes (compressed)
        let compressed_pubkey = self.compress_pubkey_hex(&recipient_pubkey)?;

        // Encrypt the message using NIP-04
        let encrypted = encrypt(
            self.identity.private_key_bytes(),
            &compressed_pubkey,
            content,
        )
        .map_err(|e| anyhow!("Failed to encrypt DM: {}", e))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template = EventTemplate {
            created_at: now,
            kind: 4, // Encrypted direct message
            tags: vec![vec!["p".to_string(), recipient_pubkey.clone()]],
            content: encrypted,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())
            .map_err(|e| anyhow!("Failed to sign DM event: {}", e))?;

        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await
            .map_err(|e| anyhow!("Failed to publish DM: {}", e))?;

        tracing::info!(
            "[{}] Sent DM to {}: {}",
            self.agent_name,
            &recipient_pubkey[..16],
            &event.id[..16]
        );
        Ok(())
    }

    /// Send a zap (NIP-57)
    async fn execute_zap(&self, target: &str, amount_sats: u64) -> Result<()> {
        if amount_sats == 0 {
            return Err(anyhow!("Zap amount must be greater than zero"));
        }

        let (recipient_pubkey, zapped_event) = if target.starts_with("note")
            || target.starts_with("nevent")
        {
            let event_id = parse_note_reference(target)?;
            let event = self.fetch_event_by_id(&event_id).await?;
            (event.pubkey.clone(), Some(event))
        } else {
            let pubkey = self.parse_pubkey_input(target)?;
            (pubkey, None)
        };

        let profile = self.fetch_profile_event(&recipient_pubkey).await?;
        let lnurl_source = lnurl_from_profile(&profile)?;

        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        let lnurl_info = fetch_lnurl_pay_info(&http, &lnurl_source).await?;

        let amount_msats = amount_sats
            .checked_mul(1000)
            .ok_or_else(|| anyhow!("Zap amount too large"))?;

        if amount_msats < lnurl_info.min_sendable || amount_msats > lnurl_info.max_sendable {
            return Err(anyhow!(
                "Zap amount {} msats outside LNURL limits ({}-{} msats)",
                amount_msats,
                lnurl_info.min_sendable,
                lnurl_info.max_sendable
            ));
        }

        let zap_request = self.build_zap_request_event(
            &recipient_pubkey,
            amount_msats,
            &lnurl_info.lnurl,
            zapped_event.as_ref(),
        )?;

        let invoice = request_zap_invoice(
            &http,
            &lnurl_info.callback,
            amount_msats,
            &zap_request,
            &lnurl_info.lnurl,
        )
        .await?;

        self.compute_client.pay_invoice(&invoice, None).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct LnurlSource {
    lnurl: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LnurlPayResponse {
    callback: String,
    min_sendable: u64,
    max_sendable: u64,
    allows_nostr: Option<bool>,
    nostr_pubkey: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Clone)]
struct LnurlPayInfo {
    lnurl: String,
    callback: String,
    min_sendable: u64,
    max_sendable: u64,
}

fn parse_note_reference(note_id: &str) -> Result<String> {
    if note_id.starts_with("note") || note_id.starts_with("nevent") {
        let entity = decode(note_id)
            .map_err(|e| anyhow!("Failed to decode note reference '{}': {}", note_id, e))?;
        match entity {
            Nip19Entity::Note(id) => Ok(hex::encode(id)),
            Nip19Entity::Event(pointer) => Ok(hex::encode(pointer.id)),
            _ => Err(anyhow!("Unsupported note reference: {}", note_id)),
        }
    } else {
        Err(anyhow!("Unsupported note reference: {}", note_id))
    }
}

fn encode_lnurl(url: &str) -> Result<String> {
    let hrp = Hrp::parse("lnurl").map_err(|e| anyhow!("Failed to build LNURL hrp: {}", e))?;
    bech32::encode::<Bech32>(hrp, url.as_bytes())
        .map_err(|e| anyhow!("Failed to encode LNURL: {}", e))
}

fn is_hex_32_bytes(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn decode_lnurl(lnurl: &str) -> Result<String> {
    let (hrp, data) = bech32::decode(&lnurl.to_lowercase())
        .map_err(|e| anyhow!("Failed to decode LNURL: {}", e))?;
    if hrp.to_string() != "lnurl" {
        return Err(anyhow!("Invalid LNURL prefix: {}", hrp));
    }
    String::from_utf8(data).map_err(|e| anyhow!("LNURL payload is not valid UTF-8: {}", e))
}

fn lnurl_from_lud16(address: &str) -> Result<LnurlSource> {
    let mut parts = address.split('@');
    let name = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if name.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err(anyhow!("Invalid lightning address '{}'", address));
    }

    let url = format!("https://{}/.well-known/lnurlp/{}", domain, name);
    let lnurl = encode_lnurl(&url)?;
    Ok(LnurlSource { lnurl, url })
}

fn lnurl_from_lud06(lnurl: &str) -> Result<LnurlSource> {
    let url = decode_lnurl(lnurl)?;
    Ok(LnurlSource {
        lnurl: lnurl.to_lowercase(),
        url,
    })
}

fn lnurl_from_profile(profile: &Event) -> Result<LnurlSource> {
    let payload: serde_json::Value = serde_json::from_str(&profile.content)
        .map_err(|e| anyhow!("Failed to parse profile metadata: {}", e))?;

    if let Some(lud16) = payload.get("lud16").and_then(|value| value.as_str()) {
        return lnurl_from_lud16(lud16);
    }

    if let Some(lud06) = payload.get("lud06").and_then(|value| value.as_str()) {
        return lnurl_from_lud06(lud06);
    }

    Err(anyhow!(
        "Profile does not include a lightning address (lud16/lud06)"
    ))
}

async fn fetch_lnurl_pay_info(http: &Client, source: &LnurlSource) -> Result<LnurlPayInfo> {
    let response = http
        .get(&source.url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch LNURL pay info: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "LNURL pay request failed with status {}",
            response.status()
        ));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse LNURL pay response: {}", e))?;

    if let Some(status) = payload.get("status").and_then(|value| value.as_str()) {
        if status.eq_ignore_ascii_case("ERROR") {
            let reason = payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or("Unknown LNURL error");
            return Err(anyhow!("LNURL error: {}", reason));
        }
    }

    let info: LnurlPayResponse = serde_json::from_value(payload)
        .map_err(|e| anyhow!("Failed to parse LNURL pay info: {}", e))?;
    if info.tag.as_deref() != Some("payRequest") {
        return Err(anyhow!("LNURL response missing payRequest tag"));
    }

    if !info.allows_nostr.unwrap_or(false) {
        return Err(anyhow!(
            "Recipient LNURL endpoint does not support Nostr zaps"
        ));
    }

    let nostr_pubkey = info
        .nostr_pubkey
        .ok_or_else(|| anyhow!("LNURL response missing nostrPubkey"))?;
    if !is_hex_32_bytes(&nostr_pubkey) {
        return Err(anyhow!("LNURL nostrPubkey is not valid hex"));
    }

    Ok(LnurlPayInfo {
        lnurl: source.lnurl.clone(),
        callback: info.callback,
        min_sendable: info.min_sendable,
        max_sendable: info.max_sendable,
    })
}

async fn request_zap_invoice(
    http: &Client,
    callback: &str,
    amount_msats: u64,
    zap_request: &Event,
    lnurl: &str,
) -> Result<String> {
    let mut url =
        Url::parse(callback).map_err(|e| anyhow!("Invalid LNURL callback '{}': {}", callback, e))?;
    let zap_json = serde_json::to_string(zap_request)
        .map_err(|e| anyhow!("Failed to encode zap request: {}", e))?;

    url.query_pairs_mut()
        .append_pair("amount", &amount_msats.to_string())
        .append_pair("nostr", &zap_json)
        .append_pair("lnurl", lnurl);

    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch zap invoice: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Zap invoice request failed with status {}",
            response.status()
        ));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse zap invoice response: {}", e))?;

    if let Some(status) = payload.get("status").and_then(|value| value.as_str()) {
        if status.eq_ignore_ascii_case("ERROR") {
            let reason = payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or("Unknown LNURL error");
            return Err(anyhow!("LNURL error: {}", reason));
        }
    }

    let invoice = payload
        .get("pr")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("LNURL response missing invoice (pr)"))?;

    Ok(invoice.to_string())
}
