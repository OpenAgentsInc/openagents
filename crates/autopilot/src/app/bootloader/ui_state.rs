//! Bootloader UI state for GPU-rendered boot sequence.

use std::sync::Arc;
use tokio::sync::mpsc;
use winit::window::Window;

use super::card::{BootCard, CardState};
use super::events::{BootEvent, BootStage};
use super::module::BootloaderModule;

/// UI state for the bootloader modal.
pub struct BootloaderUIState {
    /// Cards for each boot stage.
    pub cards: Vec<BootCard>,
    /// Event receiver from bootloader module.
    pub event_rx: Option<mpsc::UnboundedReceiver<BootEvent>>,
    /// Whether boot has completed.
    pub completed: bool,
    /// Scroll offset for card list.
    pub scroll_offset: f32,
    /// Total content height.
    pub content_height: f32,
    /// Viewport height.
    pub viewport_height: f32,
    /// Summary text after boot completes.
    pub summary: Option<String>,
    /// Whether boot failed.
    pub failed: bool,
    /// Error message if boot failed.
    pub error_message: Option<String>,
}

impl BootloaderUIState {
    /// Create new bootloader UI state with all stages pending.
    pub fn new() -> Self {
        let stages = [
            BootStage::Hardware,
            BootStage::Compute,
            BootStage::Network,
            BootStage::Identity,
            BootStage::Workspace,
            BootStage::Summary,
            BootStage::Issues,
        ];

        let cards = stages.iter().map(|stage| BootCard::new(*stage)).collect();

        Self {
            cards,
            event_rx: None,
            completed: false,
            scroll_offset: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
            summary: None,
            failed: false,
            error_message: None,
        }
    }

    /// Update the Issues stage state directly (for issue evaluation).
    pub fn update_issues_state(&mut self, state: CardState, message: Option<String>) {
        if let Some(card) = self.find_card_mut(BootStage::Issues) {
            card.state = state;
            card.progress_message = message;
        }
    }

    /// Set Issues stage as complete with details.
    pub fn complete_issues(&mut self, details: super::events::StageDetails, duration_ms: u64) {
        if let Some(card) = self.find_card_mut(BootStage::Issues) {
            card.state = CardState::Complete;
            card.duration_ms = Some(duration_ms);
            card.details = Some(details);
            card.progress_message = None;
        }
    }

    /// Start the bootloader and connect event receiver.
    pub fn start(&mut self, runtime_handle: &tokio::runtime::Handle, window: Arc<Window>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let bootloader = BootloaderModule::new().with_events(tx);
        self.event_rx = Some(rx);

        let window_clone = window.clone();
        runtime_handle.spawn(async move {
            let _ = bootloader.run().await;
            window_clone.request_redraw();
        });
    }

    /// Drain events from the bootloader and update card states.
    /// Returns true if any events were processed.
    pub fn drain_events(&mut self) -> bool {
        // Collect events first to avoid borrow issues
        let events: Vec<_> = if let Some(rx) = &mut self.event_rx {
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            events
        } else {
            return false;
        };

        let updated = !events.is_empty();

        for event in events {
            self.handle_event(event);
        }

        updated
    }

    /// Handle a single boot event (public wrapper for external callers).
    pub fn handle_event_external(&mut self, event: BootEvent) {
        self.handle_event(event);
    }

    /// Handle a single boot event.
    fn handle_event(&mut self, event: BootEvent) {
        match event {
            BootEvent::BootStarted { .. } => {
                // Reset all cards to pending
                for card in &mut self.cards {
                    card.state = CardState::Pending;
                }
            }

            BootEvent::StageStarted { stage, .. } => {
                if let Some(card) = self.find_card_mut(stage) {
                    card.state = CardState::Running;
                    card.progress_message = Some(stage.description().to_string());
                }
            }

            BootEvent::StageProgress { stage, message } => {
                if let Some(card) = self.find_card_mut(stage) {
                    card.progress_message = Some(message);
                }
            }

            BootEvent::StageCompleted {
                stage,
                duration,
                details,
            } => {
                if let Some(card) = self.find_card_mut(stage) {
                    card.state = CardState::Complete;
                    card.duration_ms = Some(duration.as_millis() as u64);
                    card.details = Some(details);
                    card.progress_message = None;
                }
            }

            BootEvent::StageFailed {
                stage,
                duration,
                error,
            } => {
                if let Some(card) = self.find_card_mut(stage) {
                    card.state = CardState::Failed;
                    card.duration_ms = Some(duration.as_millis() as u64);
                    card.progress_message = Some(error);
                }
            }

            BootEvent::StageSkipped { stage, reason } => {
                if let Some(card) = self.find_card_mut(stage) {
                    card.state = CardState::Skipped;
                    card.progress_message = Some(reason.to_string());
                }
            }

            BootEvent::BootCompleted { summary, .. } => {
                self.completed = true;
                self.summary = summary;
            }

            BootEvent::BootFailed { error } => {
                self.completed = true;
                self.failed = true;
                self.error_message = Some(error);
            }
        }
    }

    /// Find a card by stage.
    fn find_card_mut(&mut self, stage: BootStage) -> Option<&mut BootCard> {
        self.cards.iter_mut().find(|c| c.stage == stage)
    }

    /// Check if boot is complete and ready to transition.
    pub fn is_ready_to_transition(&self) -> bool {
        self.completed
    }
}

impl Default for BootloaderUIState {
    fn default() -> Self {
        Self::new()
    }
}
