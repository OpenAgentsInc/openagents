//! Raw shell forwarding with an explicit pending-proposal approval key.
use coder_pty::{
    host::{self, Config, Host, Right, Rights},
    proposal::{Action, Request as ProposalRequest, State},
    wire::{Attach, Body, Close, Input, Launch, Mode, Open, Resize, Size, Status, Value},
};
use std::{
    collections::BTreeSet,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use terminal_core::{
    blocks::Blocks,
    bridge::Request,
    context::Context,
    integration::Hooks,
    proposals::{Binding, Book, Phase, Proposal},
};

const OWNER: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const WORKSPACE: &str = "2222222222222222222222222222222222222222222222222222222222222222";
fn id() -> String {
    terminal_core::proposals::digest(&terminal_core::smart::id())
}
struct Owner;
impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == OWNER
    }
}
struct Shell {
    _hooks: Option<Hooks>,
}
impl host::Wrap for Shell {
    fn command(&self, program: &Path, args: &[std::ffi::OsString]) -> Result<Command, String> {
        let mut command = Command::new(program);
        command.args(args);
        Ok(command)
    }
}
struct Raw(libc::termios);
impl Raw {
    fn enter() -> Result<Self, String> {
        // SAFETY: both calls use a valid termios object and this process's stdin.
        let mut saved = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(0, &mut saved) } != 0 {
            return Err("A terminal is required.".into());
        }
        let mut raw = saved;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        if unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } != 0 {
            return Err("Cannot enter terminal input mode.".into());
        }
        Ok(Self(saved))
    }
}
impl Drop for Raw {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.0);
        }
    }
}
fn size() -> Size {
    let mut size = unsafe { std::mem::zeroed::<libc::winsize>() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut size);
    }
    Size::new(
        if size.ws_row == 0 { 24 } else { size.ws_row },
        if size.ws_col == 0 { 80 } else { size.ws_col },
    )
}
fn notice(text: &str) {
    let mut output = std::io::stdout().lock();
    let _ = write!(output, "\r\n{}\r\n", text.replace('\n', "\r\n"));
    let _ = output.flush();
}
struct Helper {
    child: Child,
    lines: mpsc::Receiver<Option<serde_json::Value>>,
    eof: bool,
    start: Instant,
    result: Option<(String, String)>,
}
impl Helper {
    fn start(
        program: &Path,
        cwd: &str,
        verb: &str,
        body: &impl serde::Serialize,
        result: Option<(String, String)>,
    ) -> Result<Self, String> {
        let mut child = Command::new(program)
            .args(["--json", "chat", verb, "-"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
        let mut input = child.stdin.take().ok_or("No helper input")?;
        let output = child.stdout.take().ok_or("No helper output")?;
        let (send, lines) = mpsc::sync_channel(64);
        std::thread::spawn(move || {
            let _ = input.write_all(&bytes);
        });
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = String::new();
                if (&mut reader)
                    .take(256 * 1024 + 1)
                    .read_line(&mut line)
                    .ok()
                    .is_none_or(|n| n == 0)
                    || line.len() > 256 * 1024
                {
                    break;
                }
                if let Ok(value) = serde_json::from_str(&line) {
                    if send.send(Some(value)).is_err() {
                        break;
                    }
                }
            }
            let _ = send.send(None);
        });
        Ok(Self {
            child,
            lines,
            eof: false,
            start: Instant::now(),
            result,
        })
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// This function never classifies or rewrites ordinary input. Only a shell's
/// explicit request mark and Ctrl+G on an idle displayed proposal have effects.
pub fn run(root: &Path, shell: &Path, helper: &Path) -> Result<i32, String> {
    let home = std::env::var_os("ZDOTDIR")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| root.into());
    let mut config = Config::new().workspace(WORKSPACE, root);
    config.shell = shell.into();
    config.emulator = Some(coder_vt::Authority::factory(500));
    let hooks = supported(shell)
        .then(|| Hooks::for_shell(shell, &home))
        .flatten()
        .map(|(hooks, start)| {
            config.shell_args = start.args;
            config.base_env.extend(start.env);
            hooks
        });
    let integrated = hooks.is_some();
    config.wrap = Some(Arc::new(Shell { _hooks: hooks }));
    let host = Host::new(config, Arc::new(Owner));
    let mut dimensions = size();
    let terminal = match host.open(
        OWNER,
        &Open::new(id(), WORKSPACE, "", Launch::Shell, dimensions),
    ) {
        Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
        other => return Err(format!("Cannot open shell: {other:?}")),
    };
    let (sink, frames) = host::channel(4096);
    let attachment = match host.attach(
        OWNER,
        &Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 26).with_effects(),
        Box::new(sink),
    ) {
        Ok((_, Value::Attached { attachment, .. })) => attachment,
        other => return Err(format!("Cannot attach shell: {other:?}")),
    };
    let _raw = Raw::enter()?;
    if integrated {
        notice(
            "Hook-only shell: # requests; Ctrl+G shows the warning, then Ctrl+Y confirms the displayed proposal. Ordinary keys stay in the shell.",
        );
    } else {
        notice(
            "Shell hooks are unavailable. Ordinary shell input remains available; requests and proposals are disabled.",
        );
    }
    let (send, keys) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut bytes = [0; 8192];
        while let Ok(n) = input.read(&mut bytes) {
            if n == 0 || send.send(bytes[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut vt = coder_vt::Terminal::new(dimensions.rows as usize, dimensions.cols as usize, 5000);
    let mut blocks = Blocks::default();
    let mut book = Book::default();
    let mut pending: Option<String> = None;
    let mut workers = Vec::<Helper>::new();
    let mut requests = std::collections::BTreeMap::<String, Request>::new();
    let mut thread = terminal_core::smart::id();
    let mut new = true;
    let mut result_attempted = BTreeSet::new();
    let mut execution_before = std::collections::BTreeMap::<String, u64>::new();
    let started = Instant::now();
    let mut paste = Paste::default();
    let mut last_input_prompt = 0;
    let mut request_arm: Option<(u64, Instant)> = None;
    let exit = 'running: loop {
        while let Ok(frame) = frames.try_recv() {
            match frame.body {
                Body::Output { data, .. } => {
                    let mut output = std::io::stdout().lock();
                    if output
                        .write_all(&data)
                        .and_then(|_| output.flush())
                        .is_err()
                    {
                        break 'running 1;
                    }
                    vt.feed(&data);
                    // The owner answers queries; this projection never sends another reply.
                    let _ = vt.take_replies();
                    blocks.update(&mut vt, started.elapsed().as_millis() as u64);
                }
                Body::Exit { exit, .. } => break 'running exit.code.unwrap_or(1),
                Body::Gap { .. } | Body::Detached { .. } => {
                    notice("Terminal output is unavailable. Pending approval is disabled.");
                    pending = None;
                    for before in execution_before.values_mut() {
                        *before = u64::MAX;
                    }
                }
                _ => {}
            }
        }
        loop {
            let bytes = match keys.try_recv() {
                Ok(bytes) => bytes,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break 'running 0,
            };
            let confirmation = bytes == [25]
                && pending
                    .as_ref()
                    .is_some_and(|key| matches!(book.entries[key].phase, Phase::Warned { .. }));
            let approval = (bytes == [7] || confirmation)
                && !paste.active
                && ready(&blocks, &vt, last_input_prompt);
            let pasted = paste.feed(&bytes);
            if approval && let Some(key) = pending.clone() {
                if bytes == [7] && matches!(book.entries[&key].phase, Phase::Warned { .. }) {
                    notice(
                        "Ctrl+Y confirms this warned proposal; repeating Ctrl+G does not approve it.",
                    );
                    continue;
                }
                let proposal = book.entries[&key].proposal.clone();
                let request = ProposalRequest::new(
                    id(),
                    terminal.clone(),
                    Action::Decide {
                        thread: proposal.thread.clone(),
                        proposal: proposal.id.clone(),
                        revision: proposal.revision,
                        approve: true,
                        attachment: attachment.clone(),
                    },
                );
                execution_before.insert(
                    key.clone(),
                    blocks.records.back().map_or(0, |block| block.id),
                );
                book.entries.get_mut(&key).unwrap().phase = Phase::Uncertain {
                    approval: request.request.clone(),
                };
                match host.proposal(OWNER, &request) {
                    Ok((_, Value::Proposals { page })) => {
                        match page.entries.first().map(|entry| &entry.state) {
                            Some(State::Warned { nonce }) => {
                                book.entries.get_mut(&key).unwrap().phase = Phase::Warned {
                                    nonce: nonce.clone(),
                                };
                                notice(
                                    "This command may change files or this computer. Ctrl+Y confirms this exact proposal.",
                                );
                            }
                            Some(State::Executing) => {
                                execution_before.insert(
                                    key.clone(),
                                    blocks.records.back().map_or(0, |block| block.id),
                                );
                                book.entries.get_mut(&key).unwrap().phase = Phase::Executing {
                                    approval: request.request,
                                };
                                pending = None;
                            }
                            _ => {
                                pending = None;
                                notice(
                                    "The proposal has no fresh execution receipt. No input was replayed.",
                                );
                            }
                        }
                    }
                    Err(error) => {
                        pending = None;
                        notice(&format!("Proposal refused: {}", error.detail));
                    }
                    _ => {
                        pending = None;
                        notice("Unexpected proposal receipt. No input was replayed.");
                    }
                }
            } else {
                if !bytes.is_empty() {
                    pending = None;
                    last_input_prompt = blocks.prompt_revision;
                    if !pasted && bytes.iter().any(|byte| matches!(byte, b'\r' | b'\n')) {
                        request_arm = Some((
                            blocks.records.back().map_or(0, |block| block.id),
                            Instant::now(),
                        ));
                    }
                }
                for chunk in bytes.chunks(coder_pty::wire::INPUT_MAX) {
                    let _ = host.input(OWNER, &Input::new(id(), terminal.clone(), chunk));
                }
            }
        }
        if ready(&blocks, &vt, last_input_prompt)
            && let Some(text) = blocks.request.take()
        {
            let explicit = request_arm.take().is_some_and(|(before, at)| {
                at.elapsed() <= Duration::from_secs(5)
                    && blocks.records.back().map_or(0, |block| block.id) == before
            });
            if !explicit {
                continue;
            }
            if let Some(edit) = text.strip_prefix("/edit ") {
                if let Some((key, command)) = edit.split_once(' ')
                    && let Some(entry) = book.entries.get(key)
                {
                    let mut proposal = entry.proposal.clone();
                    proposal.revision += 1;
                    proposal.command = command.into();
                    if proposal.valid()
                        && matches!(entry.phase, Phase::Pending | Phase::Warned { .. })
                    {
                        offer(&host, &terminal, &mut book, &mut pending, proposal);
                    } else {
                        notice("Only an exact pending proposal can be edited.");
                    }
                } else {
                    notice("Use # /edit KEY COMMAND with the displayed proposal key.");
                }
            } else if let Some(key) = text.strip_prefix("/reject ") {
                if let Some(entry) = book.entries.get_mut(key) {
                    let proposal = &entry.proposal;
                    let reply = host.proposal(
                        OWNER,
                        &ProposalRequest::new(
                            id(),
                            terminal.clone(),
                            Action::Decide {
                                thread: proposal.thread.clone(),
                                proposal: proposal.id.clone(),
                                revision: proposal.revision,
                                approve: false,
                                attachment: attachment.clone(),
                            },
                        ),
                    );
                    if reply.is_ok() {
                        entry.phase = Phase::Rejected;
                        pending = None;
                        notice("Proposal rejected.");
                    }
                }
            } else if requests.len() >= 256 {
                notice(
                    "This shell reached its request limit. Open a new hook-only shell for another request.",
                );
            } else if workers.iter().any(|worker| worker.result.is_none()) {
                notice("A request is still in flight. Nothing was submitted twice.");
            } else {
                let cwd = blocks
                    .cwd
                    .as_ref()
                    .and_then(|cwd| std::fs::canonicalize(cwd).ok())
                    .unwrap_or_else(|| root.into())
                    .display()
                    .to_string();
                let mut context = Context {
                    directory: Some(cwd.clone()),
                    ..Context::default()
                };
                if let Some(block) = blocks
                    .records
                    .back()
                    .filter(|block| block.status.is_some_and(|code| code != 0))
                {
                    context.attach(block, &terminal_core::smart::scrub);
                }
                let request = Request {
                    thread: thread.clone(),
                    request: terminal_core::smart::id(),
                    new,
                    text,
                    binding: Binding {
                        terminal: terminal.terminal.clone(),
                        generation: terminal.generation.clone(),
                        cwd,
                        shell_directory: blocks.cwd.clone(),
                        context_digest: context.identity(),
                    },
                    context,
                };
                if request.message().is_ok() {
                    match Helper::start(
                        helper,
                        &request.binding.cwd,
                        "shell-request",
                        &request,
                        None,
                    ) {
                        Ok(worker) => {
                            requests.insert(request.request.clone(), request);
                            new = false;
                            workers.push(worker);
                        }
                        Err(_) => notice("The request helper is unavailable. No command was run."),
                    }
                }
            }
        }
        for worker in &mut workers {
            // Defer inline notices while a full-screen program owns the display.
            if !ready(&blocks, &vt, last_input_prompt) {
                continue;
            }
            while let Ok(value) = worker.lines.try_recv() {
                let Some(value) = value else {
                    worker.eof = true;
                    break;
                };
                match value["event"].as_str() {
                    Some("attached") => {
                        if let Some(attached) = value["thread"].as_str() {
                            if requests.values().any(|request| request.thread == attached) {
                                thread = attached.into();
                                new = false;
                            }
                        }
                    }
                    Some("answer") => {
                        if let Some(text) = value["text"].as_str() {
                            notice(&terminal_core::ascii::plain(text));
                        }
                    }
                    Some("shell-proposal") => {
                        if let Ok(proposal) =
                            serde_json::from_value::<Proposal>(value["proposal"].clone())
                            && requests.get(&proposal.id).is_some_and(|request| {
                                request.thread == proposal.thread
                                    && request.binding == proposal.binding
                                    && proposal.revision == 1
                            })
                        {
                            offer(&host, &terminal, &mut book, &mut pending, proposal);
                        }
                    }
                    _ => {}
                }
            }
        }
        workers.retain_mut(|worker| {
            if worker.start.elapsed() > Duration::from_secs(150) {
                let _ = worker.child.kill();
            }
            match worker.child.try_wait() {
                Ok(Some(status)) if worker.eof => {
                    if worker.result.is_none() && !status.success() {
                        notice("The request failed or its acknowledgment is unknown. It was not submitted again; no suggestion was executed.");
                    }
                    if let Some((key, approval)) = &worker.result {
                        if status.success() {
                            let _ = book.acknowledge(key, approval);
                        } else {
                            notice(
                                "The result acknowledgment is unknown. Execution was not replayed.",
                            );
                        }
                    }
                    false
                }
                _ => true,
            }
        });
        let completed = book
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                let (Phase::Executing { approval } | Phase::Uncertain { approval }) = &entry.phase
                else {
                    return None;
                };
                blocks
                    .records
                    .iter()
                    .rev()
                    .find(|block| {
                        block.command == entry.proposal.command
                            && block.end.is_some()
                            && block.id
                                == execution_before
                                    .get(key)
                                    .and_then(|before| before.checked_add(1))
                                    .unwrap_or(u64::MAX)
                    })
                    .map(|block| (key.clone(), approval.clone(), block.clone()))
            })
            .collect::<Vec<_>>();
        for (key, approval, block) in completed {
            if book.complete(&key, &approval, block.clone()).is_ok()
                && result_attempted.insert(key.clone())
            {
                let proposal = book.entries[&key].proposal.clone();
                let result = crate::ResultRequest {
                    proposal: proposal.clone(),
                    approval: approval.clone(),
                    block,
                };
                if let Ok(worker) = Helper::start(
                    helper,
                    &proposal.binding.cwd,
                    "shell-result",
                    &result,
                    Some((key, approval)),
                ) {
                    workers.push(worker);
                } else {
                    notice("The result could not be submitted. Execution was not replayed.");
                }
            }
        }
        let current = size();
        if current != dimensions {
            dimensions = current;
            let _ = host.resize(OWNER, &Resize::new(id(), terminal.clone(), current));
            vt.resize(current.rows as usize, current.cols as usize);
        }
        std::thread::sleep(Duration::from_millis(8));
    };
    let _ = host.close(OWNER, &Close::new(id(), terminal));
    host.shutdown();
    Ok(exit)
}

fn supported(shell: &Path) -> bool {
    let minimum = match shell.file_name().and_then(|name| name.to_str()) {
        Some("zsh") => return true,
        Some("bash") => (4, 4),
        Some("fish") => (3, 3),
        _ => return false,
    };
    let Ok(output) = Command::new(shell).arg("--version").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .next()
        .unwrap_or_default()
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|part| {
            let mut parts = part.split('.');
            Some((
                parts.next()?.parse::<u32>().ok()?,
                parts.next()?.parse::<u32>().ok()?,
            ))
        })
        .next()
        .is_some_and(|version| version >= minimum)
}

fn ready(blocks: &Blocks, vt: &coder_vt::Terminal, last_input_prompt: u64) -> bool {
    blocks.at_prompt
        && !vt.alternate_screen()
        && blocks.prompt_revision > last_input_prompt
        && blocks.buffer.as_ref().is_none_or(String::is_empty)
}

fn offer(
    host: &Host,
    terminal: &coder_pty::wire::TerminalRef,
    book: &mut Book,
    pending: &mut Option<String>,
    proposal: Proposal,
) {
    match host.proposal(
        OWNER,
        &ProposalRequest::new(
            id(),
            terminal.clone(),
            Action::Offer {
                proposal: proposal.clone(),
            },
        ),
    ) {
        Ok(_) => {
            if let Ok(key) = book.offer(proposal.clone()) {
                notice(&format!(
                    "Pending {} revision {} in {}:\n{}\nCtrl+G confirms; # /edit {} COMMAND changes the revision; # /reject {} rejects. Thread: {}",
                    key,
                    proposal.revision,
                    proposal.binding.cwd,
                    proposal.command,
                    key,
                    key,
                    proposal.thread
                ));
                *pending = Some(key);
            }
        }
        Err(error) => notice(&format!(
            "Proposal unavailable: {}. No command was run.",
            error.detail
        )),
    }
}

#[derive(Default)]
struct Paste {
    active: bool,
    tail: Vec<u8>,
}
impl Paste {
    fn feed(&mut self, bytes: &[u8]) -> bool {
        let mut pasted = self.active;
        for byte in bytes {
            self.tail.push(*byte);
            if self.tail.len() > 6 {
                self.tail.remove(0);
            }
            if self.tail == b"\x1b[200~" {
                self.active = true;
                pasted = true;
            }
            if self.tail == b"\x1b[201~" {
                self.active = false;
            }
        }
        pasted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_bracketed_paste_never_becomes_a_confirmation_key() {
        let mut paste = Paste::default();
        paste.feed(b"\x1b[20");
        paste.feed(b"0~\x07");
        assert!(paste.active);
        paste.feed(b"\x1b[201");
        assert!(paste.active);
        paste.feed(b"~");
        assert!(!paste.active);
    }
}
