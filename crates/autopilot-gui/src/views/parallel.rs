//! Parallel agents view for autopilot-gui
//!
//! This module provides the UI for managing parallel autopilot containers.

use maud::{html, Markup};

/// Parallel agents page content
pub fn parallel_agents_page(agents: Vec<AgentViewInfo>, open_issues: Vec<IssueViewInfo>, platform_info: PlatformViewInfo) -> Markup {
    html! {
        // Include HTMX
        script src="https://unpkg.com/htmx.org@1.9.10" {}

        // Log modal container
        div id="log-modal" {}

        div class="parallel-container" {
            h1 { "Parallel Agents" }

            // Control panel
            div class="control-panel" {
                form hx-post="/api/parallel/start" hx-swap="none" hx-trigger="submit" {
                    label for="agent-count" { "Agent Count:" }
                    select name="count" id="agent-count" {
                        @for i in 1..=platform_info.max_agents {
                            option value=(i) selected[i == 3] { (i) }
                        }
                    }
                    button type="submit" class="btn-start" { "Start Agents" }
                }
                form hx-post="/api/parallel/stop" hx-swap="none" hx-trigger="submit" {
                    button type="submit" class="btn-stop" { "Stop All" }
                }
            }

            // Running agents section
            div class="section" {
                h2 { "Running Agents" }
                div class="agents-list" id="agents-list" hx-get="/api/parallel/status" hx-trigger="every 5s" hx-swap="innerHTML" {
                    (agents_list(&agents))
                }
            }

            // Open issues queue
            div class="section" {
                h2 { "Open Issues Queue" }
                div class="issues-list" {
                    @if open_issues.is_empty() {
                        p class="empty" { "No open issues" }
                    } @else {
                        @for issue in &open_issues {
                            div class="issue-row" {
                                span class="issue-number" { "#" (issue.number) }
                                span class=(format!("priority priority-{}", issue.priority)) { "[" (issue.priority) "]" }
                                span class="issue-title" { (issue.title) }
                            }
                        }
                    }
                }
            }

            // Resource usage
            div class="section" {
                h2 { "Resource Usage" }
                div class="resource-info" {
                    span { "Platform: " (platform_info.platform) }
                    span { " | " }
                    span { "Max Agents: " (platform_info.max_agents) }
                    span { " | " }
                    span { "Memory/Agent: " (platform_info.memory_per_agent) }
                }
            }
        }

        // Styles
        style {
            r#"
            .parallel-container {
                padding: 2rem;
                max-width: 1200px;
                margin: 0 auto;
            }
            .control-panel {
                display: flex;
                gap: 2rem;
                padding: 1.5rem;
                background: #2a2a2a;
                border: 1px solid #3a3a3a;
                margin-bottom: 2rem;
                align-items: center;
            }
            .control-panel form {
                display: flex;
                gap: 0.5rem;
                align-items: center;
            }
            .control-panel label {
                color: #a0a0a0;
            }
            .control-panel select {
                background: #1a1a1a;
                color: #e0e0e0;
                border: 1px solid #3a3a3a;
                padding: 0.5rem;
            }
            .btn-start {
                background: #2d5016;
                color: #7dff7d;
                border: none;
                padding: 0.5rem 1rem;
                cursor: pointer;
            }
            .btn-start:hover {
                background: #3d6020;
            }
            .btn-stop {
                background: #501616;
                color: #ff7d7d;
                border: none;
                padding: 0.5rem 1rem;
                cursor: pointer;
            }
            .btn-stop:hover {
                background: #602020;
            }
            .section {
                margin-bottom: 2rem;
            }
            .section h2 {
                color: #4a9eff;
                margin-bottom: 1rem;
                font-size: 1.25rem;
            }
            .agents-list {
                background: #2a2a2a;
                border: 1px solid #3a3a3a;
            }
            .agent-row {
                display: flex;
                justify-content: space-between;
                align-items: center;
                padding: 1rem;
                border-bottom: 1px solid #3a3a3a;
            }
            .agent-row:last-child {
                border-bottom: none;
            }
            .agent-id {
                font-family: monospace;
                color: #4a9eff;
            }
            .agent-status {
                display: flex;
                align-items: center;
                gap: 0.5rem;
            }
            .status-dot {
                width: 8px;
                height: 8px;
            }
            .status-dot.running {
                background: #7dff7d;
            }
            .status-dot.stopped {
                background: #ff7d7d;
            }
            .status-dot.idle {
                background: #a0a0a0;
            }
            .agent-issue {
                color: #a0a0a0;
            }
            .agent-uptime {
                color: #a0a0a0;
                font-size: 0.875rem;
            }
            .btn-logs {
                background: #1a3a5a;
                color: #4a9eff;
                border: none;
                padding: 0.25rem 0.5rem;
                cursor: pointer;
                font-size: 0.75rem;
            }
            .issues-list {
                background: #2a2a2a;
                border: 1px solid #3a3a3a;
            }
            .issue-row {
                display: flex;
                gap: 1rem;
                padding: 0.75rem 1rem;
                border-bottom: 1px solid #3a3a3a;
            }
            .issue-row:last-child {
                border-bottom: none;
            }
            .issue-number {
                font-family: monospace;
                color: #4a9eff;
            }
            .priority {
                font-size: 0.75rem;
                padding: 0.125rem 0.5rem;
            }
            .priority-urgent {
                background: #501616;
                color: #ff7d7d;
            }
            .priority-high {
                background: #503016;
                color: #ffaa7d;
            }
            .priority-medium {
                background: #1a3a5a;
                color: #4a9eff;
            }
            .priority-low {
                background: #2a2a2a;
                color: #a0a0a0;
            }
            .issue-title {
                color: #e0e0e0;
            }
            .resource-info {
                background: #2a2a2a;
                border: 1px solid #3a3a3a;
                padding: 1rem;
                color: #a0a0a0;
                font-family: monospace;
            }
            .empty {
                color: #a0a0a0;
                padding: 1rem;
                text-align: center;
            }
            "#
        }
    }
}

/// Render just the agents list (for HTMX updates)
pub fn agents_list(agents: &[AgentViewInfo]) -> Markup {
    html! {
        @if agents.is_empty() {
            p class="empty" { "No agents running" }
        } @else {
            @for agent in agents {
                div class="agent-row" {
                    span class="agent-id" { "agent-" (agent.id) }
                    div class="agent-status" {
                        span class=(format!("status-dot {}", agent.status_class())) {}
                        @if let Some(issue) = agent.current_issue {
                            span class="agent-issue" { "Working on #" (issue) }
                        } @else {
                            span class="agent-issue" { "Idle" }
                        }
                    }
                    @if let Some(uptime) = &agent.uptime {
                        span class="agent-uptime" { (uptime) }
                    }
                    button class="btn-logs"
                        hx-get=(format!("/api/parallel/logs/{}", agent.id))
                        hx-target="#log-modal"
                        hx-swap="innerHTML" {
                        "View Logs"
                    }
                }
            }
        }
    }
}

/// Agent info for view rendering
#[derive(Debug, Clone)]
pub struct AgentViewInfo {
    pub id: String,
    pub status: AgentViewStatus,
    pub current_issue: Option<i32>,
    pub uptime: Option<String>,
}

impl AgentViewInfo {
    pub fn status_class(&self) -> &'static str {
        match self.status {
            AgentViewStatus::Running => "running",
            AgentViewStatus::Stopped => "stopped",
            AgentViewStatus::Idle => "idle",
        }
    }
}

/// Agent status for view
#[derive(Debug, Clone, Copy)]
pub enum AgentViewStatus {
    Running,
    Stopped,
    Idle,
}

/// Issue info for view rendering
#[derive(Debug, Clone)]
pub struct IssueViewInfo {
    pub number: i32,
    pub title: String,
    pub priority: String,
}

/// Platform info for view
#[derive(Debug, Clone)]
pub struct PlatformViewInfo {
    pub platform: String,
    pub max_agents: usize,
    pub memory_per_agent: String,
}

impl Default for PlatformViewInfo {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        return Self {
            platform: "macOS".to_string(),
            max_agents: 5,
            memory_per_agent: "3G".to_string(),
        };

        #[cfg(not(target_os = "macos"))]
        return Self {
            platform: "Linux".to_string(),
            max_agents: 10,
            memory_per_agent: "12G".to_string(),
        };
    }
}
