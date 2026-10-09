//! Background agents in the terminal app (#11163): their rows in the agent
//! rail and the `/agents` panel, their notices in the chat, `/agent ENGINE
//! TASK`, and messages typed into an agent's chat.

use crossterm::event::{KeyCode, KeyEvent};
use serde_json::json;

use crate::{App, Draft, Mode, fleet, live};

/// The `/agents` panel: which row is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Panel {
    pub selected: usize,
}

/// How to start one by hand.
pub const USAGE: &str = "Start a background agent with /agent ENGINE TASK, for example /agent codex fix the flaky login test.";

impl App {
    fn agent_cwd(&self) -> std::path::PathBuf {
        self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
    }

    /// This chat's host for background agents.
    pub(crate) fn fleet_host(&self) -> fleet::Host {
        fleet::Host::local(self.fleet.clone(), self.session_id().map(str::to_owned))
    }

    /// The plugin settings an agent started from this chat runs with.
    pub(crate) fn agent_execution(&self) -> crate::plugin_tools::ExecutionSettings {
        let mut execution = self.plugins.execution_settings(self.agent_cwd());
        execution.fleet = Some(self.fleet_host());
        execution
    }

    fn agent_provider(&self) -> Option<crate::plugin_tools::GenerationProvider> {
        let key = self
            .plugins
            .key_for_request()
            .filter(|_| self.plugins.enabled)?;
        let client = openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new(
            key.expose(),
        )))
        .ok()?;
        Some(crate::plugin_tools::GenerationProvider {
            client,
            model: self.plugins.model.clone(),
            effort: self.plugins.options.reasoning.clone(),
        })
    }

    fn agent_delegation(&mut self, row: &agent_fleet::AgentRow) -> usize {
        if let Some(index) = self.delegations.iter().position(|child| child.id == row.id) {
            return index;
        }
        self.live.entries.push(live::Entry::Delegation {
            id: row.id.clone(),
            name: row.name.clone(),
            task: row.task.clone(),
            running: row.status == agent_fleet::Status::Running,
            output: serde_json::Value::Null,
            progress: None,
        });
        self.delegations.push(live::Delegation {
            id: row.id.clone(),
            name: row.name.clone(),
            task: row.task.clone(),
            chat: live::Chat {
                entries: vec![live::Entry::User(row.task.clone())],
                busy: true,
                reply_started_at: Some(std::time::Instant::now()),
                ..live::Chat::default()
            },
            started_at: self.elapsed_seconds,
            elapsed_seconds: 0,
            running: true,
            draft: Draft::default(),
            composer: Default::default(),
            scroll: u16::MAX,
            background: true,
        });
        self.history.dirty = true;
        self.delegations.len() - 1
    }

    /// Shows what background agents did since the last frame, and hands
    /// each finished agent's notice to the chat.
    pub(crate) fn poll_fleet(&mut self) {
        for (id, event) in self.fleet.drain_events() {
            let Some(row) = self.fleet.get(&id) else {
                continue;
            };
            self.agent_delegation(&row);
            self.apply_delegation(id, row.name, row.task, event);
        }
        for notice in self.fleet.drain_notices() {
            if let Some(row) = self.fleet.get(&notice.id) {
                let index = self.agent_delegation(&row);
                let child = &mut self.delegations[index];
                child.running = false;
                child.chat.busy = false;
                child.chat.finish_partial();
                child
                    .chat
                    .stop_tools("The agent ended before this tool returned.");
                child.chat.tokens = notice.tokens;
                child.elapsed_seconds = notice.elapsed_seconds;
                child.chat.notice = notice.error.clone();
            }
            let output = json!({
                "status": notice.status.word(),
                "report": notice.report,
                "error": notice.error,
                "branch": notice.branch,
                "tokens": notice.tokens,
                "cost_usd": notice.cost_usd,
            });
            if let Some(live::Entry::Delegation {
                running, output: o, ..
            }) = self.live.entries.iter_mut().find(
                |entry| matches!(entry, live::Entry::Delegation { id, .. } if id == &notice.id),
            ) {
                *running = false;
                *o = output;
            }
            let text = notice.text();
            if self.mode == Mode::Live && self.plugins.enabled && self.plugins.key_configured {
                // The model reads it as its next input, even mid-turn.
                self.queue_notice(text);
            } else {
                self.live.entries.push(live::Entry::User(text));
                self.scroll_main_to_end();
            }
            self.history.dirty = true;
        }
    }

    /// `/agent ENGINE TASK`.
    pub(crate) fn agent_command(&mut self, argument: &str) {
        if self.mode != Mode::Live {
            self.notice = Some("Background agents run in live mode only.".into());
            return;
        }
        let execution = self.agent_execution();
        let engines = fleet::engines(&execution);
        let Some((engine, task)) = argument
            .trim()
            .split_once(char::is_whitespace)
            .map(|(engine, task)| (engine.trim(), task.trim()))
            .filter(|(_, task)| !task.is_empty())
        else {
            self.notice = Some(format!(
                "{USAGE} Engines here: {}.",
                if engines.is_empty() {
                    "none; turn on ACP Subagents or the Coder loop in /plugins".into()
                } else {
                    engines.join(", ")
                }
            ));
            return;
        };
        let Some(host) = execution.fleet.clone() else {
            return;
        };
        self.record_prompt();
        let result = fleet::start(
            &host,
            &execution,
            fleet::StartArguments {
                engine: engine.to_owned(),
                task: task.to_owned(),
                name: None,
                background: None,
                worktree: None,
            },
            self.agent_provider(),
        );
        match result {
            Ok(row) => {
                self.draft = Draft::default();
                self.composer = Default::default();
                self.composer_history.reset();
                self.agent_delegation(&row);
                self.notice = Some(format!(
                    "Started {} on {}. Its report arrives here when it ends; /agents lists it.",
                    row.name, row.engine
                ));
            }
            Err(error) => self.notice = Some(error),
        }
    }

    /// Sends the composer's text to the background agent at `index`: a
    /// running one reads it when its current step ends; an ended one
    /// resumes with it.
    pub(crate) fn message_agent(&mut self, index: usize) {
        let Some(child) = self.delegations.get(index) else {
            return;
        };
        let id = child.id.clone();
        let text = self.draft.text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        if self.deliver_agent_message(&id, &text) {
            self.record_prompt();
            self.draft = Draft::default();
            self.composer = Default::default();
            self.composer_history.reset();
            self.scroll = u16::MAX;
        }
    }

    /// Sends `text` to background agent `id` (from this terminal or the
    /// phone) and shows it in the agent's chat. False when it could not be
    /// sent; the reason is on the agent's chat.
    pub(crate) fn deliver_agent_message(&mut self, id: &str, text: &str) -> bool {
        let result = match self.fleet.message(id, text) {
            Ok(agent_fleet::Delivery::Queued(row)) => Ok(format!(
                "{} reads this when its current step ends.",
                row.name
            )),
            Ok(agent_fleet::Delivery::Resume(row)) => {
                let execution = self.agent_execution();
                let host = self.fleet_host();
                fleet::resume(&host, &execution, id, text, self.agent_provider())
                    .map(|()| format!("{} is working again.", row.name))
            }
            Err(error) => Err(error),
        };
        let Some(row) = self.fleet.get(id) else {
            return false;
        };
        let index = self.agent_delegation(&row);
        let child = &mut self.delegations[index];
        match result {
            Ok(notice) => {
                child.chat.entries.push(live::Entry::User(text.to_owned()));
                child.chat.notice = Some(notice);
                if !child.running {
                    child.running = true;
                    child.chat.busy = true;
                    child.chat.reply_started_at = Some(std::time::Instant::now());
                    child.started_at = self.elapsed_seconds;
                }
                self.history.dirty = true;
                true
            }
            Err(error) => {
                child.chat.notice = Some(error);
                false
            }
        }
    }

    /// Opens the `/agents` panel.
    pub(crate) fn open_agents_panel(&mut self) {
        self.resume_picker = None;
        self.model_picker = None;
        self.agents_panel = Some(Panel::default());
    }

    /// One key in the `/agents` panel.
    pub(crate) fn agents_panel_key(&mut self, key: KeyEvent) {
        let rows = self.fleet.list();
        let Some(panel) = &mut self.agents_panel else {
            return;
        };
        let last = rows.len().saturating_sub(1);
        panel.selected = panel.selected.min(last);
        let chosen = rows.get(panel.selected).cloned();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.agents_panel = None,
            KeyCode::Up | KeyCode::Char('k') => panel.selected = panel.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.selected = (panel.selected + 1).min(last),
            KeyCode::Char('s') => {
                if let Some(row) = chosen {
                    self.notice = Some(match self.fleet.stop(&row.id) {
                        Ok(row) => format!("Stopping {}.", row.name),
                        Err(error) => error,
                    });
                }
            }
            KeyCode::Enter | KeyCode::Char('o' | 'm' | 'r') => {
                let Some(row) = chosen else { return };
                let index = self.agent_delegation(&row);
                self.agents_panel = None;
                self.select_agent(Some(index));
                if matches!(key.code, KeyCode::Char('m' | 'r')) {
                    self.notice = Some(if row.status == agent_fleet::Status::Running {
                        format!(
                            "Type a message for {}; it reads it when its current step ends.",
                            row.name
                        )
                    } else {
                        format!(
                            "Type a message for {}; it starts working again with it.",
                            row.name
                        )
                    });
                }
            }
            _ => {}
        }
    }

    /// Stops every background agent and waits up to `wait` for them to end,
    /// returning how many were running.
    pub fn stop_agents(&mut self, wait: std::time::Duration) -> usize {
        let running = self.fleet.running();
        if running == 0 {
            return 0;
        }
        self.fleet.stop_all();
        let started = std::time::Instant::now();
        while self.fleet.running() > 0 && started.elapsed() < wait {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        running
    }
}
