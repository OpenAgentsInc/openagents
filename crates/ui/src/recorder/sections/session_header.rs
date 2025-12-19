use maud::{Markup, html};

use super::super::molecules::SessionMode;

pub struct SessionHeader {
    pub id: String,
    pub model: String,
    pub mode: SessionMode,
    pub repo: String,
    pub repo_sha: String,
    pub branch: String,
    pub runner: Option<String>,
    pub sandbox_id: Option<String>,
    pub budget: Option<f64>,
    pub duration: Option<String>,
    pub skills: Vec<String>,
    pub mcp: Vec<String>,
    pub expanded: bool,
}

impl SessionHeader {
    pub fn new(id: &str, model: &str, repo: &str) -> Self {
        Self {
            id: id.to_string(),
            model: model.to_string(),
            mode: SessionMode::Auto,
            repo: repo.to_string(),
            repo_sha: String::new(),
            branch: "main".to_string(),
            runner: None,
            sandbox_id: None,
            budget: None,
            duration: None,
            skills: vec![],
            mcp: vec![],
            expanded: true,
        }
    }

    pub fn mode(mut self, mode: SessionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn sha(mut self, sha: &str) -> Self {
        self.repo_sha = sha.to_string();
        self
    }

    pub fn branch(mut self, branch: &str) -> Self {
        self.branch = branch.to_string();
        self
    }

    pub fn runner(mut self, runner: &str, sandbox_id: &str) -> Self {
        self.runner = Some(runner.to_string());
        self.sandbox_id = Some(sandbox_id.to_string());
        self
    }

    pub fn budget(mut self, budget: f64, duration: &str) -> Self {
        self.budget = Some(budget);
        self.duration = Some(duration.to_string());
        self
    }

    pub fn skills(mut self, skills: Vec<&str>) -> Self {
        self.skills = skills.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn mcp(mut self, mcp: Vec<&str>) -> Self {
        self.mcp = mcp.iter().map(|s| s.to_string()).collect();
        self
    }

    #[allow(dead_code)]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn build(self) -> Markup {
        html! {
            details class="bg-card border border-border" open[self.expanded] {
                summary class="px-3 py-2 cursor-pointer list-none flex items-center gap-2" {
                    span class="text-xs text-muted-foreground tracking-widest" { "SESSION" }
                    span class="text-sm text-foreground font-medium" { (self.id) }
                    span class="flex-1" {}
                    span class="text-xs text-muted-foreground" { "[\u{25BE}]" }
                }
                div class="px-3 py-3 border-t border-border" {
                    div class="grid grid-cols-[auto,1fr] gap-x-4 gap-y-1 text-xs" {
                        span class="text-muted-foreground" { "Model:" }
                        span class="text-muted-foreground" { (self.model) }

                        span class="text-muted-foreground" { "Mode:" }
                        span class="text-muted-foreground" {
                            @match self.mode {
                                SessionMode::Auto => span class="text-green" { "auto" },
                                SessionMode::Plan => span class="text-blue" { "plan" },
                                SessionMode::Chat => span class="text-muted-foreground" { "chat" },
                            }
                        }

                        span class="text-muted-foreground" { "Repo:" }
                        span class="text-muted-foreground" {
                            (self.repo)
                            @if !self.repo_sha.is_empty() {
                                span class="text-muted-foreground" { " @ " }
                                span class="text-cyan" { (self.repo_sha) }
                            }
                        }

                        span class="text-muted-foreground" { "Branch:" }
                        span class="text-muted-foreground" { (self.branch) }

                        @if let Some(ref runner) = self.runner {
                            span class="text-muted-foreground" { "Runner:" }
                            span class="text-muted-foreground" {
                                (runner)
                                @if let Some(ref sid) = self.sandbox_id {
                                    span class="text-muted-foreground" { " (" (sid) ")" }
                                }
                            }
                        }
                    }

                    @if self.budget.is_some() || !self.skills.is_empty() || !self.mcp.is_empty() {
                        div class="border-t border-border mt-3 pt-3" {
                            div class="grid grid-cols-[auto,1fr] gap-x-4 gap-y-1 text-xs" {
                                @if let (Some(budget), Some(duration)) = (self.budget, &self.duration) {
                                    span class="text-muted-foreground" { "Budget:" }
                                    span class="text-muted-foreground" { "$" (format!("{:.0}", budget)) " for " (duration) }
                                }

                                span class="text-muted-foreground" { "Skills:" }
                                span class="text-muted-foreground" {
                                    @if self.skills.is_empty() {
                                        span class="text-muted-foreground" { "(none)" }
                                    } @else {
                                        (self.skills.join(", "))
                                    }
                                }

                                span class="text-muted-foreground" { "MCP:" }
                                span class="text-muted-foreground" {
                                    @if self.mcp.is_empty() {
                                        span class="text-muted-foreground" { "(none)" }
                                    } @else {
                                        (self.mcp.join(", "))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
