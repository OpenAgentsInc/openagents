//! Tick Executor
//!
//! Executes a single tick cycle for a sovereign agent:
//! 1. Perceive (fetch observations - mentions, DMs, zaps)
//! 2. Think (request compute, pay for it)
//! 3. Act (execute actions from LLM response)
//! 4. Update state

use super::compute::ComputeClient;
use super::state::StateManager;
use agent::{LifecycleManager, LifecycleState, RunwayAnalysis};
use anyhow::{anyhow, Result};
use nostr::nip_sa::AgentStateContent;
use nostr::Event;
use nostr_client::RelayConnection;
use std::time::Duration;

/// What triggered this tick
#[derive(Debug, Clone)]
pub enum TickTrigger {
    /// Scheduled heartbeat timer
    Heartbeat,
    /// Agent was mentioned in a note
    Mention(Event),
    /// Agent received a DM
    DirectMessage(Event),
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
}

/// Action taken by the agent
#[derive(Debug, Clone)]
pub enum TickAction {
    /// Post a note
    Post { content: String },
    /// Send a DM
    DirectMessage { recipient: String, content: String },
    /// Send a zap
    Zap { target: String, amount_sats: u64 },
    /// Update a goal
    UpdateGoal { goal_id: String, progress: f64 },
    /// Add a memory
    AddMemory { memory_type: String, content: String },
    /// No action taken
    None,
}

/// Tick executor for a sovereign agent
pub struct TickExecutor {
    state_manager: StateManager,
    compute_client: ComputeClient,
    lifecycle_manager: LifecycleManager,
    relay: RelayConnection,
    pubkey: String,
    agent_name: String,
}

impl TickExecutor {
    /// Create a new tick executor
    pub fn new(
        state_manager: StateManager,
        compute_client: ComputeClient,
        relay: RelayConnection,
        pubkey: String,
        agent_name: String,
    ) -> Self {
        Self {
            state_manager,
            compute_client,
            lifecycle_manager: LifecycleManager::with_state(LifecycleState::Active),
            relay,
            pubkey,
            agent_name,
        }
    }

    /// Execute a single tick
    pub async fn execute_tick(&mut self, trigger: TickTrigger) -> Result<TickResult> {
        // 1. Fetch current state
        let mut state = self.state_manager.get_or_create_state().await?;
        let tick_number = state.tick_count + 1;

        tracing::info!("[{}] Starting tick #{}", self.agent_name, tick_number);

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
            return Ok(TickResult {
                tick_number,
                trigger,
                observations: vec![],
                reasoning: "Skipped - insufficient funds".to_string(),
                actions: vec![TickAction::None],
                lifecycle_state,
                runway,
                compute_cost_sats: 0,
            });
        }

        // 3. Gather observations
        let observations = self.gather_observations(&trigger).await?;
        tracing::info!(
            "[{}] Gathered {} observations",
            self.agent_name,
            observations.len()
        );

        // 4. Build prompt for reasoning
        let prompt = self.build_reasoning_prompt(&state, &trigger, &observations);

        // 5. Discover providers and request compute
        let providers = self.compute_client.discover_providers(3).await?;
        if providers.is_empty() {
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

        // 6. Request inference and PAY for it
        let reasoning = self
            .compute_client
            .request_inference(provider, &prompt, 500, budget_sats)
            .await?;

        let compute_cost_sats = provider.price_msats / 1000;

        // 7. Parse actions from reasoning
        let actions = self.parse_actions(&reasoning);

        // 8. Execute actions
        for action in &actions {
            if let Err(e) = self.execute_action(action).await {
                tracing::warn!("[{}] Action failed: {}", self.agent_name, e);
            }
        }

        // 9. Update state
        state.record_tick(chrono::Utc::now().timestamp() as u64);
        state.record_spend(compute_cost_sats);

        // Add memory of this tick
        state.memory.push(nostr::nip_sa::MemoryEntry::new(
            "tick",
            format!("Tick #{}: {}", tick_number, reasoning.chars().take(100).collect::<String>()),
        ));

        // 10. Publish updated state
        self.state_manager.publish_state(&state).await?;

        tracing::info!(
            "[{}] Tick #{} complete. Cost: {} sats, New balance: {} sats",
            self.agent_name,
            tick_number,
            compute_cost_sats,
            state.wallet_balance_sats
        );

        Ok(TickResult {
            tick_number,
            trigger,
            observations,
            reasoning,
            actions,
            lifecycle_state,
            runway,
            compute_cost_sats,
        })
    }

    /// Gather observations based on trigger
    async fn gather_observations(&self, trigger: &TickTrigger) -> Result<Vec<Event>> {
        let mut observations = Vec::new();

        // Add the triggering event if applicable
        match trigger {
            TickTrigger::Mention(event) => observations.push(event.clone()),
            TickTrigger::DirectMessage(event) => observations.push(event.clone()),
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
            .subscribe_with_channel("observations", &filters)
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
            TickTrigger::Zap(_) => prompt.push_str("You received a zap (Bitcoin payment).\n"),
        }
        prompt.push('\n');

        // Add observations
        if !observations.is_empty() {
            prompt.push_str("## Recent Observations:\n");
            for (i, event) in observations.iter().take(5).enumerate() {
                prompt.push_str(&format!(
                    "{}. [kind:{}] {}\n",
                    i + 1,
                    event.kind,
                    event.content.chars().take(200).collect::<String>()
                ));
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
            "Based on the above context, decide what action to take. \
            You can: POST a note, REPLY to someone, ZAP someone, or do NOTHING.\n\n\
            Respond with a brief reasoning and your chosen action.\n"
        );

        prompt
    }

    /// Parse actions from LLM reasoning
    fn parse_actions(&self, reasoning: &str) -> Vec<TickAction> {
        let mut actions = Vec::new();

        let lower = reasoning.to_lowercase();

        // Simple parsing - look for action keywords
        if lower.contains("post:") || lower.contains("post a note") {
            // Extract content after "POST:" if present
            if let Some(pos) = reasoning.to_lowercase().find("post:") {
                let content = reasoning[pos + 5..].trim();
                let content = content.lines().next().unwrap_or(content);
                actions.push(TickAction::Post {
                    content: content.to_string(),
                });
            }
        }

        if lower.contains("nothing") || lower.contains("no action") {
            actions.push(TickAction::None);
        }

        if actions.is_empty() {
            actions.push(TickAction::None);
        }

        actions
    }

    /// Execute a single action
    async fn execute_action(&self, action: &TickAction) -> Result<()> {
        match action {
            TickAction::Post { content } => {
                tracing::info!("[{}] Posting: {}", self.agent_name, content);
                // TODO: Actually post to Nostr
            }
            TickAction::DirectMessage { recipient, content } => {
                tracing::info!(
                    "[{}] DM to {}: {}",
                    self.agent_name,
                    recipient,
                    content
                );
                // TODO: Send encrypted DM
            }
            TickAction::Zap { target, amount_sats } => {
                tracing::info!(
                    "[{}] Zapping {} with {} sats",
                    self.agent_name,
                    target,
                    amount_sats
                );
                // TODO: Send zap
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
}
