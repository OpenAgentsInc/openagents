//! Request preview and a typed subprocess adapter to the shared chat client.

use super::layout::PaneId;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use terminal_core::{
    bridge::Request,
    context::Context,
    proposals::{Book, Proposal},
};

pub struct Draft {
    pub pane: PaneId,
    pub text: String,
    pub scroll: usize,
    pub context: Context,
}

pub enum Message {
    Attached(String),
    Proposal(Proposal, terminal_core::proposals::Effect),
}

pub struct Worker {
    pub pane: PaneId,
    pub request: Request,
    pub events: Receiver<Message>,
    child: Child,
    pub eof: bool,
    started: std::time::Instant,
}

impl Worker {
    pub fn start(pane: PaneId, request: Request) -> Result<Self, String> {
        let program = super::pty::candidates("openagents")
            .into_iter()
            .next()
            .ok_or("openagents helper not found")?;
        let mut child = Command::new(program)
            .args(["--json", "chat", "shell-request", "-"])
            .current_dir(&request.binding.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "terminal request helper did not start")?;
        let body =
            serde_json::to_vec(&request).map_err(|_| "terminal request could not be encoded")?;
        let mut input = child
            .stdin
            .take()
            .ok_or("terminal request helper has no input")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("terminal request helper has no output")?;
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            if input.write_all(&body).is_err() {
                return;
            }
            drop(input);
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                // The shared helper emits bounded NDJSON; a malformed line fails closed.
                if (&mut reader)
                    .take(256 * 1024 + 1)
                    .read_line(&mut line)
                    .ok()
                    .is_none_or(|size| size == 0)
                {
                    break;
                }
                if line.len() > 256 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let message = match value["event"].as_str() {
                    Some("attached") => value["thread"].as_str().map(|thread| Message::Attached(thread.into())),
                    Some("shell-proposal") => serde_json::from_value(value["proposal"].clone()).ok().map(|proposal| Message::Proposal(proposal, if value["effect"] == "read_only" { terminal_core::proposals::Effect::Ordinary } else { terminal_core::proposals::Effect::Destructive("This command may change files or this computer, or publish data. Press Enter again to approve it.".into()) })),
                    _ => None,
                };
                if let Some(message) = message {
                    let _ = sender.send(message);
                }
            }
        });
        Ok(Self {
            pane,
            request,
            events,
            child,
            eof: false,
            started: std::time::Instant::now(),
        })
    }

    pub fn ended(&mut self) -> Option<bool> {
        if self.started.elapsed() > std::time::Duration::from_secs(150) {
            let _ = self.child.kill();
        }
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.success())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct Smart {
    pub draft: Option<Draft>,
    pub selected: Option<(PaneId, u64)>,
    pub git: Option<Receiver<(PaneId, String, String)>>,
    pub book: Book,
    pub policy: Policies,
    pub pending: Option<(PaneId, String)>,
    pub workers: Vec<Worker>,
    pub threads: std::collections::BTreeMap<PaneId, String>,
    pub results: std::collections::BTreeMap<String, (String, String)>,
    pub execution: Option<(PaneId, String, String, u64)>,
    pub proposal_scroll: usize,
    pub enter_down: bool,
}

pub fn id() -> String {
    super::pty::request()[32..].to_owned()
}

/// Scrubs common credential-bearing lines before their preview and submission.
pub fn scrub(text: &str) -> String {
    let mut private_key = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("-----begin") && lower.contains("private key") {
            private_key = true;
        }
        let sensitive = private_key
            || [
                "authorization:",
                "api_key",
                "api-key",
                "access_token",
                "password=",
                "secret=",
            ]
            .iter()
            .any(|needle| lower.contains(needle));
        if sensitive {
            out.push("[redacted]".into());
        } else {
            let words = line
                .split_inclusive(char::is_whitespace)
                .map(|word| {
                    let core = word
                        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
                    if [
                        "sk-",
                        "oak_",
                        "sess_",
                        "nsec1",
                        "ghp_",
                        "github_pat_",
                        "gho_",
                        "ghs_",
                    ]
                    .iter()
                    .any(|prefix| core.starts_with(prefix))
                    {
                        "[redacted] ".to_owned()
                    } else {
                        word.to_owned()
                    }
                })
                .collect::<String>();
            out.push(words);
        }
        if lower.contains("-----end") && lower.contains("private key") {
            private_key = false;
        }
    }
    out.join("\n")
}

#[derive(Default)]
pub struct Policies(pub std::collections::BTreeMap<String, terminal_core::proposals::Effect>);
impl terminal_core::proposals::Policy for Policies {
    fn effect(&self, command: &str) -> terminal_core::proposals::Effect {
        self.0.get(command).cloned().unwrap_or_else(|| {
            terminal_core::proposals::Effect::Denied("no host policy admitted this command".into())
        })
    }
}

impl super::Overlay {
    pub(super) fn ask(&mut self, text: String) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            return;
        };
        let mut context = Context::default();
        if let Some(binding) = pane.session.binding(String::new()) {
            context.directory = Some(binding.cwd);
        }
        if let Some(block) = self
            .smart
            .selected
            .filter(|(selected_pane, _)| *selected_pane == pane_id)
            .and_then(|(_, id)| pane.session.blocks.get(id))
            .or_else(|| {
                pane.session
                    .blocks
                    .records
                    .back()
                    .filter(|block| block.status.is_some_and(|code| code != 0))
            })
        {
            context.attach(block, &scrub);
        }
        if let Some(directory) = &context.directory {
            let directory = directory.clone();
            let (sender, receiver) = mpsc::channel();
            self.smart.git = Some(receiver);
            std::thread::spawn(move || {
                let Ok(mut child) = Command::new("git")
                    .args(["status", "--short", "--branch"])
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .current_dir(&directory)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                else {
                    return;
                };
                let Some(stdout) = child.stdout.take() else {
                    return;
                };
                let (output_sender, output_receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    let mut bytes = Vec::new();
                    let result = stdout.take(8193).read_to_end(&mut bytes);
                    let _ = output_sender.send((result, bytes));
                });
                if let Ok((Ok(_), bytes)) =
                    output_receiver.recv_timeout(std::time::Duration::from_secs(2))
                    && bytes.len() <= 8192
                    && {
                        let deadline =
                            std::time::Instant::now() + std::time::Duration::from_millis(100);
                        loop {
                            if let Ok(Some(status)) = child.try_wait() {
                                break status.success();
                            }
                            if std::time::Instant::now() >= deadline {
                                break false;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                    }
                {
                    let summary = scrub(&String::from_utf8_lossy(&bytes));
                    let _ = sender.send((pane_id, directory, summary));
                }
                let _ = child.kill();
                let _ = child.wait();
            });
        }
        self.smart.draft = Some(Draft {
            pane: pane_id,
            text,
            scroll: 0,
            context,
        });
    }

    pub(super) fn smart_key(&mut self, key: &super::KeyIn) -> bool {
        use winit::keyboard::KeyCode;
        if let Some(draft) = &mut self.smart.draft {
            match key.code {
                KeyCode::Escape => self.smart.draft = None,
                KeyCode::Backspace => {
                    draft.text.pop();
                }
                KeyCode::KeyD if self.mods.control_key() => {
                    draft.context.clear();
                    self.smart.git = None;
                }
                KeyCode::PageDown => draft.scroll = draft.scroll.saturating_add(10),
                KeyCode::PageUp => draft.scroll = draft.scroll.saturating_sub(10),
                KeyCode::Enter => self.submit_draft(),
                _ => {
                    if let Some(text) = &key.text {
                        draft.text.extend(text.chars().filter(|c| !c.is_control()));
                        draft.text.truncate(draft.text.floor_char_boundary(8192));
                    }
                }
            }
            return true;
        }
        if let Some((pane_id, proposal_key)) = self.smart.pending.clone() {
            if self.focus_id() != Some(pane_id) {
                return false;
            }
            if key.code == KeyCode::Escape {
                self.smart.pending = None;
                return true;
            }
            if key.code == KeyCode::PageDown {
                self.smart.proposal_scroll = self.smart.proposal_scroll.saturating_add(10);
                return true;
            }
            if key.code == KeyCode::PageUp {
                self.smart.proposal_scroll = self.smart.proposal_scroll.saturating_sub(10);
                return true;
            }
            if key.code == KeyCode::Enter {
                let entry = &self.smart.book.entries[&proposal_key];
                let Some(pane) = self.panes.get(&pane_id) else {
                    return true;
                };
                let Some(binding) = pane
                    .session
                    .binding(entry.proposal.binding.context_digest.clone())
                else {
                    return true;
                };
                if !pane.session.blocks.at_prompt
                    || pane
                        .session
                        .blocks
                        .buffer
                        .as_deref()
                        .is_some_and(|buffer| !buffer.is_empty())
                {
                    self.notice =
                        Some("Return to an empty shell prompt before approving a proposal.".into());
                    return true;
                }
                match self.smart.book.enter(
                    &proposal_key,
                    &binding,
                    "local-user",
                    &id(),
                    &self.smart.policy,
                ) {
                    Ok(terminal_core::proposals::Approval::Warning(warning)) => {
                        self.notice = Some(warning)
                    }
                    Ok(terminal_core::proposals::Approval::Input { identity, bytes }) => {
                        let after = pane
                            .session
                            .blocks
                            .records
                            .back()
                            .map_or(0, |block| block.id);
                        self.smart.execution = Some((pane_id, proposal_key, identity, after));
                        self.smart.pending = None;
                        self.send_to(pane_id, &bytes);
                    }
                    Err(why) => self.notice = Some(why.into()),
                }
                return true;
            }
            // A pending proposal is a modal preview, so unrelated keys cannot edit the shell silently.
            return true;
        }
        false
    }

    fn submit_draft(&mut self) {
        if self
            .smart
            .workers
            .iter()
            .any(|worker| Some(worker.pane) == self.focus_id())
            || self.smart.pending.is_some()
            || self.smart.execution.is_some()
        {
            self.notice =
                Some("Finish or dismiss the current request before sending another.".into());
            return;
        }
        let Some(draft) = self.smart.draft.take() else {
            return;
        };
        let Some(pane) = self.panes.get(&draft.pane) else {
            return;
        };
        let Some(binding) = pane.session.binding(draft.context.identity()) else {
            self.notice =
                Some("The shell directory is unavailable; the request remains unsent.".into());
            self.smart.draft = Some(draft);
            return;
        };
        let thread = self.smart.threads.get(&draft.pane).cloned();
        let request = Request {
            thread: thread.clone().unwrap_or_else(id),
            request: id(),
            new: thread.is_none(),
            text: draft.text.clone(),
            context: draft.context.clone(),
            binding,
        };
        if let Err(why) = request.message() {
            self.notice = Some(why.into());
            self.smart.draft = Some(draft);
            return;
        }
        match Worker::start(draft.pane, request) {
            Ok(worker) => {
                self.smart.workers.push(worker);
                self.notice = Some("Request submitted once; waiting for the shared client.".into());
            }
            Err(why) => {
                self.notice = Some(why);
                self.smart.draft = Some(draft);
            }
        }
    }

    pub(super) fn smart_tick(&mut self) {
        if let Some((pane_id, directory, summary)) = self
            .smart
            .git
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
            && let Some(draft) = &mut self.smart.draft
            && draft.pane == pane_id
            && draft.context.directory.as_deref() == Some(&directory)
        {
            draft.context.git = Some(summary);
        }
        let mut messages = Vec::new();
        for worker in &mut self.smart.workers {
            for _ in 0..32 {
                match worker.events.try_recv() {
                    Ok(message) => messages.push((worker.pane, worker.request.clone(), message)),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        worker.eof = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        self.smart.workers.retain_mut(|worker| {
            let ended = worker.ended().is_some();
            !(worker.eof && ended)
        });
        for (pane_id, request, message) in messages {
            match message {
                Message::Attached(thread) if thread == request.thread => {
                    if let Some((key, approval)) = self.smart.results.remove(&request.request) {
                        let _ = self.smart.book.acknowledge(&key, &approval);
                        continue;
                    }
                    if self.smart.threads.insert(pane_id, thread.clone()).is_none()
                        && let Some(super::pty::Program::Command {
                            program,
                            mut args,
                            label,
                        }) = super::pty::Program::openagents_terminal()
                    {
                        args.extend(["--thread".into(), thread, "--observe".into()]);
                        let focus = self.focus_id();
                        self.split(
                            super::layout::Axis::Columns,
                            &super::pty::Program::Command {
                                program,
                                args,
                                label,
                            },
                        );
                        if let Some(focus) = focus
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.layout.set_focus(focus);
                        }
                    }
                }
                Message::Proposal(proposal, effect)
                    if proposal.thread == request.thread
                        && proposal.id == request.request
                        && proposal.binding == request.binding =>
                {
                    self.smart.policy.0.insert(proposal.command.clone(), effect);
                    if let Ok(key) = self.smart.book.offer(proposal) {
                        self.smart.proposal_scroll = 0;
                        self.smart.pending = Some((pane_id, key));
                        self.notice =
                            Some("Pending command: Enter approves; Esc dismisses.".into());
                    }
                }
                _ => {}
            }
        }
        self.shell_result();
        let request = self.focus_id().and_then(|pane| {
            self.panes
                .get_mut(&pane)
                .and_then(|pane| pane.session.blocks.request.take())
        });
        if let Some(request) = request {
            self.ask(request);
        }
    }

    pub(super) fn block_move(&mut self, previous: bool) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get_mut(&pane_id) else {
            return;
        };
        let blocks = &pane.session.blocks.records;
        if blocks.is_empty() {
            return;
        }
        let current = self
            .smart
            .selected
            .filter(|(selected_pane, _)| *selected_pane == pane_id)
            .and_then(|(_, selected)| blocks.iter().position(|block| block.id == selected))
            .unwrap_or(blocks.len() - 1);
        let index = if previous {
            current.saturating_sub(1)
        } else {
            (current + 1).min(blocks.len() - 1)
        };
        let block = &blocks[index];
        self.smart.selected = Some((pane_id, block.id));
        let top = pane.session.vt.history_dropped() + pane.session.vt.scrollback_len() as u64;
        pane.scroll = top.saturating_sub(block.start.line) as usize;
        self.notice = Some(format!(
            "Block {} · exit {:?} · {} ms",
            block.id,
            block.status,
            block.elapsed_ms.unwrap_or_default()
        ));
    }

    pub(super) fn copy_block(&mut self) {
        let block = self
            .focus_id()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| {
                self.smart
                    .selected
                    .filter(|(selected_pane, _)| Some(*selected_pane) == self.focus_id())
                    .and_then(|(_, id)| pane.session.blocks.get(id))
                    .or_else(|| pane.session.blocks.records.back())
            })
            .cloned();
        if let Some(block) = block {
            self.set_clipboard(format!("$ {}\n{}", block.command, block.output));
        }
    }

    pub(super) fn collapse_block(&mut self) {
        let selected = self
            .smart
            .selected
            .filter(|(pane, _)| Some(*pane) == self.focus_id())
            .map(|(_, id)| id);
        if let Some(pane) = self.focused_pane()
            && let Some(id) =
                selected.or_else(|| pane.session.blocks.records.back().map(|block| block.id))
        {
            pane.session.blocks.collapse(id);
            pane.cache = None;
        }
    }

    pub(super) fn rerun_block(&mut self) {
        let block = self
            .focus_id()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| {
                self.smart
                    .selected
                    .filter(|(selected_pane, _)| Some(*selected_pane) == self.focus_id())
                    .and_then(|(_, id)| pane.session.blocks.get(id))
                    .or_else(|| pane.session.blocks.records.back())
            })
            .cloned();
        if let Some(block) = block {
            self.paste(&block.command);
            self.notice = Some("Command copied to the shell. Enter runs a new block.".into());
        }
    }

    pub(super) fn draw_smart(&self, batch: &mut crate::ui::UiBatch, atlas: &crate::ui::Atlas) {
        use coder_ui::theme::Intensity;
        let lines = if let Some(draft) = &self.smart.draft {
            format!(
                "Request: {}\n{}\nEnter sends · Ctrl+D removes context · PgUp/PgDn scroll · Esc cancels",
                draft.text,
                draft.context.preview()
            )
        } else if let Some((_, key)) = &self.smart.pending {
            let entry = &self.smart.book.entries[key];
            let proposal = &entry.proposal;
            let warning = if matches!(entry.phase, terminal_core::proposals::Phase::Warned { .. }) {
                match self.smart.policy.0.get(&proposal.command) {
                    Some(terminal_core::proposals::Effect::Destructive(warning)) => {
                        warning.as_str()
                    }
                    _ => "Press Enter again to approve this command.",
                }
            } else {
                "Enter approves · PgUp/PgDn scroll · Esc dismisses"
            };
            format!(
                "Pending command · {}\nThread {} · proposal {} revision {}\n$ {}\n{}",
                proposal.binding.cwd,
                proposal.thread,
                proposal.id,
                proposal.revision,
                proposal.command,
                warning
            )
        } else {
            return;
        };
        let [cw, ch] = self.cell;
        let rect = self.area;
        batch.rect(
            atlas,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            super::draw::field(0.98),
        );
        let rows = (rect.h / ch).max(1.0) as usize;
        let columns = (rect.w / cw).max(1.0) as usize;
        let scroll = self
            .smart
            .draft
            .as_ref()
            .map_or(self.smart.proposal_scroll, |draft| draft.scroll);
        let wrapped = lines
            .lines()
            .flat_map(|line| {
                let chars = line.chars().collect::<Vec<_>>();
                if chars.is_empty() {
                    vec![String::new()]
                } else {
                    chars
                        .chunks(columns.saturating_sub(2).max(1))
                        .map(|chunk| chunk.iter().collect::<String>())
                        .collect()
                }
            })
            .collect::<Vec<_>>();
        let scroll = scroll.min(wrapped.len().saturating_sub(rows));
        for (index, line) in wrapped.iter().skip(scroll).take(rows).enumerate() {
            batch.text(
                atlas,
                rect.x + cw,
                rect.y + index as f32 * ch,
                line,
                super::draw::white(Intensity::ThreeQuarters, 1.0),
            );
        }
    }
}

impl super::Overlay {
    fn shell_result(&mut self) {
        let Some((pane_id, key, approval, after)) = self.smart.execution.clone() else {
            return;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            self.smart.book.recover();
            self.smart.execution = None;
            return;
        };
        let Some(block) = pane
            .session
            .blocks
            .records
            .iter()
            .find(|block| block.id > after && block.end.is_some())
            .cloned()
        else {
            return;
        };
        if let Err(why) = self.smart.book.complete(&key, &approval, block.clone()) {
            self.notice = Some(format!(
                "Command result is uncertain: {why}. It will not be replayed."
            ));
            self.smart.book.recover();
            self.smart.execution = None;
            return;
        }
        let mut context = Context::default();
        context.attach(&block, &scrub);
        let proposal = &self.smart.book.entries[&key].proposal;
        let mut binding = proposal.binding.clone();
        binding.context_digest = context.identity();
        let request = Request {
            thread: proposal.thread.clone(),
            request: approval[..32].to_owned(),
            new: false,
            text: format!(
                "Result of approved shell proposal {} revision {} (approval {}). Continue this same thread from its attached output; do not repeat the command.",
                proposal.id, proposal.revision, approval
            ),
            context,
            binding,
        };
        match Worker::start(pane_id, request.clone()) {
            Ok(worker) => {
                self.smart.results.insert(request.request, (key, approval));
                self.smart.workers.push(worker);
                self.smart.execution = None;
                self.notice = Some(
                    "Approved command completed; its block is returning to the same thread.".into(),
                );
            }
            Err(why) => {
                self.notice = Some(format!(
                    "Result delivery is pending: {why}. The command will not be repeated."
                ))
            }
        }
    }
}
