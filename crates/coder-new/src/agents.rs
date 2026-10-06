//! Local demo tasks for the background-agent footer and task panes.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentView {
    #[default]
    Composer,
    Footer,
    List,
    Detail,
}

pub struct DemoAgent {
    pub name: &'static str,
    pub description: &'static str,
    pub elapsed: &'static str,
    pub tokens: &'static str,
    pub tools: u16,
    pub progress: &'static [&'static str],
    pub prompt: &'static str,
    pub running: bool,
}

pub struct Agents {
    pub view: AgentView,
    pub selected: usize,
    pub demos: Vec<DemoAgent>,
    pub detail_scroll: u16,
    stop_all_armed: bool,
}

impl Default for Agents {
    fn default() -> Self {
        Self {
            view: AgentView::Composer,
            selected: 0,
            detail_scroll: 0,
            stop_all_armed: false,
            demos: vec![
                DemoAgent {
                    name: "claude-code",
                    description: "Review the composer",
                    elapsed: "42s",
                    tokens: "8.2k",
                    tools: 6,
                    progress: &[
                        "Read the composer layout",
                        "Checked draft and paste behavior",
                        "Reviewing keyboard navigation",
                    ],
                    prompt: "Review the composer layout and keyboard controls. Identify changes that keep the draft visible while background tasks are open.",
                    running: true,
                },
                DemoAgent {
                    name: "codex",
                    description: "Check the terminal layout",
                    elapsed: "1m 23s",
                    tokens: "12.4k",
                    tools: 9,
                    progress: &[
                        "Read the terminal rendering code",
                        "Rendered a narrow viewport",
                        "Checking the footer placement",
                    ],
                    prompt: "Check the terminal layout at large and small sizes. Keep the composer and its cursor visible, and check that the background-agent footer stays below the input.",
                    running: true,
                },
                DemoAgent {
                    name: "devin-cli",
                    description: "Inspect the plugin list",
                    elapsed: "36s",
                    tokens: "4.7k",
                    tools: 4,
                    progress: &[
                        "Read the sample plugin list",
                        "Checked labels and spacing",
                        "Inspecting the compact status row",
                    ],
                    prompt: "Inspect the sample plugin list and the status row. Check that the labels fit without crowding the message input.",
                    running: true,
                },
                DemoAgent {
                    name: "grok-build",
                    description: "Verify the color palette",
                    elapsed: "28s",
                    tokens: "3.1k",
                    tools: 3,
                    progress: &[
                        "Read the Grok Night theme",
                        "Checked the composer border color",
                        "Verifying the exported preview colors",
                    ],
                    prompt: "Verify that the terminal preview uses the exact Grok Night RGB colors, including the composer border and background-agent selection.",
                    running: true,
                },
            ],
        }
    }
}

impl Agents {
    pub fn open(&mut self) {
        self.detail_scroll = 0;
        self.selected = self.selected.min(self.demos.len().saturating_sub(1));
        self.view = match self.demos.len() {
            0 => AgentView::Composer,
            1 => AgentView::Detail,
            _ => AgentView::List,
        };
    }

    /// Consumes navigation keys without editing the message draft.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let in_pane = matches!(self.view, AgentView::List | AgentView::Detail);
        let stop_all = self.stop_all_armed && ctrl && key.code == KeyCode::Char('k');
        self.stop_all_armed = false;
        if in_pane && stop_all {
            self.demos.clear();
            self.selected = 0;
            self.view = AgentView::Composer;
            return true;
        }
        if in_pane && ctrl && key.code == KeyCode::Char('x') {
            self.stop_all_armed = true;
            return true;
        }
        match self.view {
            AgentView::Composer if key.code == KeyCode::Down && !self.demos.is_empty() => {
                self.view = AgentView::Footer;
                true
            }
            AgentView::Footer => match key.code {
                KeyCode::Down | KeyCode::Enter => {
                    self.open();
                    true
                }
                KeyCode::Up | KeyCode::Esc => {
                    self.view = AgentView::Composer;
                    true
                }
                _ => {
                    self.view = AgentView::Composer;
                    false
                }
            },
            AgentView::List | AgentView::Detail => {
                match key.code {
                    KeyCode::Char('x') if key.modifiers.is_empty() => {
                        if self.selected < self.demos.len() {
                            self.demos.remove(self.selected);
                        }
                        self.selected = self.selected.min(self.demos.len().saturating_sub(1));
                        self.view = if self.demos.is_empty() {
                            AgentView::Composer
                        } else {
                            AgentView::List
                        };
                    }
                    KeyCode::Up if self.view == AgentView::List => {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    KeyCode::Down if self.view == AgentView::List => {
                        self.selected = (self.selected + 1).min(self.demos.len().saturating_sub(1));
                    }
                    KeyCode::Enter if self.view == AgentView::List => {
                        self.detail_scroll = 0;
                        self.view = AgentView::Detail;
                    }
                    KeyCode::PageUp if self.view == AgentView::Detail => {
                        self.detail_scroll = self.detail_scroll.saturating_sub(5);
                    }
                    KeyCode::PageDown if self.view == AgentView::Detail => {
                        self.detail_scroll = self.detail_scroll.saturating_add(5);
                    }
                    KeyCode::Left => {
                        self.view = if self.view == AgentView::Detail && self.demos.len() > 1 {
                            AgentView::List
                        } else {
                            AgentView::Composer
                        };
                    }
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')
                        if self.view == AgentView::Detail || key.code == KeyCode::Esc =>
                    {
                        self.view = AgentView::Composer;
                    }
                    _ => {}
                }
                true
            }
            AgentView::Composer => false,
        }
    }
}
