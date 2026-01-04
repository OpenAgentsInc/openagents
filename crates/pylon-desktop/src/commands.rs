//! Command registry for pylon-desktop command palette

use wgpui::components::hud::Command;

/// Command IDs for matching in handler
pub mod ids {
    pub const JOIN_CHANNEL: &str = "nostr.join_channel";
    pub const LIST_CHANNELS: &str = "nostr.list_channels";
    pub const CREATE_JOB: &str = "nostr.create_job";
    pub const VIEW_JOBS: &str = "nostr.view_jobs";
    pub const RECONNECT: &str = "nostr.reconnect";
    pub const COPY_PUBKEY: &str = "nostr.copy_pubkey";
    pub const FOCUS_CHAT: &str = "nav.focus_chat";
    pub const FOCUS_PROMPT: &str = "nav.focus_prompt";
    pub const CLEAR_OUTPUT: &str = "action.clear_output";
}

/// Build the list of available commands
pub fn build_commands() -> Vec<Command> {
    vec![
        // Nostr channel commands
        Command::new(ids::JOIN_CHANNEL, "Join Channel")
            .description("Subscribe to a Nostr chat channel")
            .category("Nostr"),
        Command::new(ids::LIST_CHANNELS, "List Channels")
            .description("Show available Nostr channels")
            .category("Nostr"),
        // Job commands
        Command::new(ids::CREATE_JOB, "Create Job Request")
            .description("Submit a NIP-90 job request to the network")
            .category("Jobs"),
        Command::new(ids::VIEW_JOBS, "View Jobs")
            .description("Focus the jobs panel")
            .keybinding("Tab")
            .category("Jobs"),
        // Connection commands
        Command::new(ids::RECONNECT, "Reconnect to Relay")
            .description("Reconnect to the Nostr relay")
            .category("Connection"),
        // Identity commands
        Command::new(ids::COPY_PUBKEY, "Copy Public Key")
            .description("Copy your Nostr public key to clipboard")
            .category("Identity"),
        // Navigation commands
        Command::new(ids::FOCUS_CHAT, "Focus Chat")
            .description("Switch focus to chat panel")
            .category("Navigation"),
        Command::new(ids::FOCUS_PROMPT, "Focus Prompt")
            .description("Switch focus to prompt input")
            .category("Navigation"),
        // Actions
        Command::new(ids::CLEAR_OUTPUT, "Clear Output")
            .description("Clear the token stream output")
            .category("Action"),
    ]
}
