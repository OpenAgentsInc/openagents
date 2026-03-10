use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use wgpui::components::sections::TerminalStream;

const TERMINAL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_TERMINAL_COLS: u16 = 120;
const DEFAULT_TERMINAL_ROWS: u16 = 32;

#[derive(Debug)]
pub struct ChatTerminalWorker {
    command_tx: Sender<ChatTerminalCommand>,
    update_rx: Receiver<ChatTerminalUpdate>,
}

impl ChatTerminalWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<ChatTerminalCommand>();
        let (update_tx, update_rx) = mpsc::channel::<ChatTerminalUpdate>();
        std::thread::spawn(move || run_terminal_loop(command_rx, update_tx));
        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(&self, command: ChatTerminalCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("Failed to queue terminal command: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<ChatTerminalUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatTerminalCommand {
    Open {
        thread_id: String,
        workspace: String,
        cols: u16,
        rows: u16,
    },
    Write {
        thread_id: String,
        text: String,
    },
    Resize {
        thread_id: String,
        cols: u16,
        rows: u16,
    },
    Restart {
        thread_id: String,
        workspace: String,
        cols: u16,
        rows: u16,
    },
    Close {
        thread_id: String,
    },
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatTerminalUpdate {
    SessionOpened {
        thread_id: String,
        workspace: String,
        shell: String,
        pid: u32,
        cols: u16,
        rows: u16,
    },
    SessionOutput {
        thread_id: String,
        stream: TerminalStream,
        text: String,
    },
    SessionResized {
        thread_id: String,
        cols: u16,
        rows: u16,
    },
    SessionClosed {
        thread_id: String,
        exit_code: Option<i32>,
        reason: Option<String>,
    },
    SessionFailed {
        thread_id: String,
        error: String,
    },
}

#[derive(Debug)]
struct ManagedTerminalSession {
    workspace: String,
    shell: String,
    child: Child,
    stdin: ChildStdin,
    cols: u16,
    rows: u16,
}

fn run_terminal_loop(
    command_rx: Receiver<ChatTerminalCommand>,
    update_tx: Sender<ChatTerminalUpdate>,
) {
    let mut sessions = HashMap::<String, ManagedTerminalSession>::new();
    loop {
        match command_rx.recv_timeout(TERMINAL_POLL_INTERVAL) {
            Ok(command) => {
                if matches!(command, ChatTerminalCommand::Shutdown) {
                    shutdown_sessions(&mut sessions);
                    break;
                }
                handle_terminal_command(command, &mut sessions, &update_tx);
                while let Ok(next) = command_rx.try_recv() {
                    if matches!(next, ChatTerminalCommand::Shutdown) {
                        shutdown_sessions(&mut sessions);
                        return;
                    }
                    handle_terminal_command(next, &mut sessions, &update_tx);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shutdown_sessions(&mut sessions);
                break;
            }
        }
        poll_terminal_sessions(&mut sessions, &update_tx);
    }
}

fn handle_terminal_command(
    command: ChatTerminalCommand,
    sessions: &mut HashMap<String, ManagedTerminalSession>,
    update_tx: &Sender<ChatTerminalUpdate>,
) {
    match command {
        ChatTerminalCommand::Open {
            thread_id,
            workspace,
            cols,
            rows,
        } => {
            if sessions.contains_key(&thread_id) {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                    thread_id,
                    error: "Terminal session is already running for this thread.".to_string(),
                });
                return;
            }
            spawn_terminal_session(thread_id, workspace, cols, rows, sessions, update_tx);
        }
        ChatTerminalCommand::Write { thread_id, text } => {
            let Some(session) = sessions.get_mut(&thread_id) else {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                    thread_id,
                    error: "No running terminal session exists for this thread.".to_string(),
                });
                return;
            };
            let mut payload = text;
            if !payload.ends_with('\n') {
                payload.push('\n');
            }
            if let Err(error) = session.stdin.write_all(payload.as_bytes()) {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                    thread_id,
                    error: format!("Failed to write to terminal session: {error}"),
                });
                return;
            }
            if let Err(error) = session.stdin.flush() {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                    thread_id,
                    error: format!("Failed to flush terminal session input: {error}"),
                });
            }
        }
        ChatTerminalCommand::Resize {
            thread_id,
            cols,
            rows,
        } => {
            let Some(session) = sessions.get_mut(&thread_id) else {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                    thread_id,
                    error: "No running terminal session exists for this thread.".to_string(),
                });
                return;
            };
            session.cols = cols;
            session.rows = rows;
            let _ = update_tx.send(ChatTerminalUpdate::SessionResized {
                thread_id,
                cols,
                rows,
            });
        }
        ChatTerminalCommand::Restart {
            thread_id,
            workspace,
            cols,
            rows,
        } => {
            if sessions.contains_key(&thread_id) {
                close_terminal_session(thread_id.as_str(), sessions, update_tx, None);
            }
            spawn_terminal_session(thread_id, workspace, cols, rows, sessions, update_tx);
        }
        ChatTerminalCommand::Close { thread_id } => {
            close_terminal_session(thread_id.as_str(), sessions, update_tx, None);
        }
        ChatTerminalCommand::Shutdown => {}
    }
}

fn spawn_terminal_session(
    thread_id: String,
    workspace: String,
    cols: u16,
    rows: u16,
    sessions: &mut HashMap<String, ManagedTerminalSession>,
    update_tx: &Sender<ChatTerminalUpdate>,
) {
    let workspace_path = PathBuf::from(workspace.trim());
    let resolved_workspace = std::fs::canonicalize(&workspace_path).unwrap_or(workspace_path);
    let Some(shell_program) = default_shell_program() else {
        let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
            thread_id,
            error: "Could not resolve a default shell for the terminal lane.".to_string(),
        });
        return;
    };

    let (mut child, shell_label) =
        match spawn_shell_process(resolved_workspace.as_path(), shell_program.as_path()) {
            Ok(value) => value,
            Err(error) => {
                let _ = update_tx.send(ChatTerminalUpdate::SessionFailed { thread_id, error });
                return;
            }
        };

    let Some(stdin) = child.stdin.take() else {
        let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
            thread_id,
            error: "Shell process did not expose stdin.".to_string(),
        });
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
            thread_id,
            error: "Shell process did not expose stdout.".to_string(),
        });
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
            thread_id,
            error: "Shell process did not expose stderr.".to_string(),
        });
        let _ = child.kill();
        let _ = child.wait();
        return;
    };

    spawn_output_reader(
        thread_id.clone(),
        TerminalStream::Stdout,
        stdout,
        update_tx.clone(),
    );
    spawn_output_reader(
        thread_id.clone(),
        TerminalStream::Stderr,
        stderr,
        update_tx.clone(),
    );

    let pid = child.id();
    let workspace_label = resolved_workspace.display().to_string();
    sessions.insert(
        thread_id.clone(),
        ManagedTerminalSession {
            workspace: workspace_label.clone(),
            shell: shell_label.clone(),
            child,
            stdin,
            cols: normalize_terminal_cols(cols),
            rows: normalize_terminal_rows(rows),
        },
    );

    let _ = update_tx.send(ChatTerminalUpdate::SessionOpened {
        thread_id,
        workspace: workspace_label,
        shell: shell_label,
        pid,
        cols: normalize_terminal_cols(cols),
        rows: normalize_terminal_rows(rows),
    });
}

fn spawn_shell_process(workspace: &Path, shell_program: &Path) -> Result<(Child, String), String> {
    let shell_label = shell_program.display().to_string();
    let mut command = Command::new(shell_program);
    if cfg!(target_os = "windows") {
        command.arg("/Q");
    } else {
        command.arg("-i");
    }
    command
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().map_err(|error| {
        format!(
            "Failed to spawn shell `{}` in {}: {error}",
            shell_label,
            workspace.display()
        )
    })?;
    Ok((child, shell_label))
}

fn spawn_output_reader<R: std::io::Read + Send + 'static>(
    thread_id: String,
    stream: TerminalStream,
    reader: R,
    update_tx: Sender<ChatTerminalUpdate>,
) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if update_tx
                        .send(ChatTerminalUpdate::SessionOutput {
                            thread_id: thread_id.clone(),
                            stream: stream.clone(),
                            text: trimmed,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
                        thread_id,
                        error: format!("Failed to read terminal output: {error}"),
                    });
                    break;
                }
            }
        }
    });
}

fn close_terminal_session(
    thread_id: &str,
    sessions: &mut HashMap<String, ManagedTerminalSession>,
    update_tx: &Sender<ChatTerminalUpdate>,
    reason: Option<String>,
) {
    let Some(mut session) = sessions.remove(thread_id) else {
        let _ = update_tx.send(ChatTerminalUpdate::SessionFailed {
            thread_id: thread_id.to_string(),
            error: "No terminal session exists for this thread.".to_string(),
        });
        return;
    };
    let _ = session.child.kill();
    match session.child.wait() {
        Ok(status) => {
            let _ = update_tx.send(ChatTerminalUpdate::SessionClosed {
                thread_id: thread_id.to_string(),
                exit_code: status.code(),
                reason,
            });
        }
        Err(error) => {
            let _ = update_tx.send(ChatTerminalUpdate::SessionClosed {
                thread_id: thread_id.to_string(),
                exit_code: None,
                reason: Some(format!("Failed to wait for shell shutdown: {error}")),
            });
        }
    }
}

fn poll_terminal_sessions(
    sessions: &mut HashMap<String, ManagedTerminalSession>,
    update_tx: &Sender<ChatTerminalUpdate>,
) {
    let mut closed = Vec::<(String, Option<i32>, Option<String>)>::new();
    for (thread_id, session) in sessions.iter_mut() {
        match session.child.try_wait() {
            Ok(Some(status)) => {
                let reason = (!status.success()).then(|| {
                    format!(
                        "Shell exited from {} with status {}.",
                        session.workspace, status
                    )
                });
                closed.push((thread_id.clone(), status.code(), reason));
            }
            Ok(None) => {}
            Err(error) => {
                closed.push((
                    thread_id.clone(),
                    None,
                    Some(format!("Failed to poll shell status: {error}")),
                ));
            }
        }
    }
    for (thread_id, exit_code, reason) in closed {
        sessions.remove(&thread_id);
        let _ = update_tx.send(ChatTerminalUpdate::SessionClosed {
            thread_id,
            exit_code,
            reason,
        });
    }
}

fn shutdown_sessions(sessions: &mut HashMap<String, ManagedTerminalSession>) {
    for (_, mut session) in sessions.drain() {
        let _ = session.child.kill();
        let _ = session.child.wait();
    }
}

fn default_shell_program() -> Option<PathBuf> {
    let env_shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|value| value.is_file());
    if env_shell.is_some() {
        return env_shell;
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("COMSPEC")
            .map(PathBuf::from)
            .filter(|value| value.is_file());
    }
    for candidate in ["/bin/zsh", "/bin/bash", "/bin/sh"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn default_terminal_size() -> (u16, u16) {
    (DEFAULT_TERMINAL_COLS, DEFAULT_TERMINAL_ROWS)
}

pub fn normalize_terminal_cols(cols: u16) -> u16 {
    cols.max(40)
}

pub fn normalize_terminal_rows(rows: u16) -> u16 {
    rows.max(12)
}

pub fn pump_runtime(state: &mut crate::app_state::RenderState) -> bool {
    let updates = state.chat_terminal_worker.drain_updates();
    if updates.is_empty() {
        return false;
    }
    let mut changed = false;
    for update in updates {
        match update {
            ChatTerminalUpdate::SessionOpened {
                thread_id,
                workspace,
                shell,
                pid,
                cols,
                rows,
            } => {
                state.autopilot_chat.record_terminal_session_opened(
                    thread_id.as_str(),
                    workspace,
                    shell,
                    pid,
                    cols,
                    rows,
                );
                changed = true;
            }
            ChatTerminalUpdate::SessionOutput {
                thread_id,
                stream,
                text,
            } => {
                state.autopilot_chat.append_terminal_session_output(
                    thread_id.as_str(),
                    stream,
                    text,
                );
                changed = true;
            }
            ChatTerminalUpdate::SessionResized {
                thread_id,
                cols,
                rows,
            } => {
                state
                    .autopilot_chat
                    .resize_terminal_session(thread_id.as_str(), cols, rows);
                changed = true;
            }
            ChatTerminalUpdate::SessionClosed {
                thread_id,
                exit_code,
                reason,
            } => {
                state.autopilot_chat.record_terminal_session_closed(
                    thread_id.as_str(),
                    exit_code,
                    reason,
                );
                changed = true;
            }
            ChatTerminalUpdate::SessionFailed { thread_id, error } => {
                state
                    .autopilot_chat
                    .record_terminal_session_failure(thread_id.as_str(), error.clone());
                state.autopilot_chat.last_error = Some(error);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_size_normalization_enforces_minimums() {
        assert_eq!(normalize_terminal_cols(0), 40);
        assert_eq!(normalize_terminal_rows(0), 12);
        assert_eq!(normalize_terminal_cols(120), 120);
        assert_eq!(normalize_terminal_rows(32), 32);
    }

    #[test]
    fn default_terminal_size_is_reasonable() {
        let (cols, rows) = default_terminal_size();
        assert!(cols >= 80);
        assert!(rows >= 24);
    }
}
