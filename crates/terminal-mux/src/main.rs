//! Attach explicitly named, already admitted durable host terminals.
use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};
use terminal_core::{
    bridge,
    pty::{Attachment, Program, Sessions, Transport},
};
struct NoLocal;
impl Transport for NoLocal {
    fn shell(&self) -> &Path {
        Path::new("/bin/sh")
    }
    fn open(&self, _: &Program, _: u16, _: u16) -> Result<Box<dyn Attachment>, String> {
        Err("This client attaches only named host terminals.".into())
    }
    fn shutdown(&self) {}
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn request(&self, _: &bridge::Request) -> Result<bridge::Connection, String> {
        Err("Use the retained thread client to ask or review proposals.".into())
    }
    fn git_summary(&self, pane: u64, directory: String) -> mpsc::Receiver<(u64, String, String)> {
        let (s, r) = mpsc::channel();
        let _ = s.send((pane, directory, "Host terminal".into()));
        r
    }
    fn open_link(&self, _: &str) -> Result<(), String> {
        Err("TTY links are text only.".into())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, _: &str) -> Result<(), String> {
        Err("Use outer-terminal text selection.".into())
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut store = None;
    let mut host = None;
    let mut references = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--store" => {
                store = Some(PathBuf::from(
                    args.next().ok_or("--store needs a directory")?,
                ))
            }
            "--host" => host = Some(args.next().ok_or("--host needs a key")?),
            "--reference" => {
                let value = args.next().ok_or("--reference needs GENERATION:TERMINAL")?;
                let (g, t) = value
                    .split_once(':')
                    .ok_or("--reference needs GENERATION:TERMINAL")?;
                references.push(terminal_remote::reference(g, t)?);
            }
            "--help" => {
                println!(
                    "openagents-mux --store PAIRED_STORE --host HOST_KEY --reference GENERATION:TERMINAL [--reference ...]\nCtrl+B then n/p: tabs; % or double quote: split; o: focus; z: zoom; k/j: blocks; r: live; d: detach. Ctrl+B twice forwards the prefix."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    let store = store.ok_or("--store is required; no default owner home is opened")?;
    let host = host.ok_or("--host is required")?;
    if references.is_empty() || references.len() > terminal_mux::MAX_PANES {
        return Err("Name between one and eight existing terminal references.".into());
    }
    if std::env::var("TERM").is_ok_and(|t| t == "dumb" || t.is_empty()) {
        return Err("This outer terminal has no cursor-addressed redraw capability. Use the hook-only terminal instead.".into());
    }
    let mut transports = Vec::new();
    for reference in references {
        transports.push(Sessions(Arc::new(terminal_remote::Remote::paired(
            &store,
            host.clone(),
            Some(reference),
            Arc::new(NoLocal),
        )?)));
    }
    let mut mux = terminal_mux::Mux::attach(transports)?;
    let guard = coder_terminal::Guard::full_screen_with_mouse().map_err(|e| e.to_string())?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))
            .map_err(|e| e.to_string())?;
    while !mux.detached {
        mux.pump();
        terminal
            .draw(|frame| mux.draw(frame))
            .map_err(|e| e.to_string())?;
        if crossterm::event::poll(std::time::Duration::from_millis(33))
            .map_err(|e| e.to_string())?
        {
            let area = terminal.size().map_err(|e| e.to_string())?;
            mux.event(
                crossterm::event::read().map_err(|e| e.to_string())?,
                ratatui::layout::Rect::new(0, 0, area.width, area.height),
            );
        }
    }
    drop(terminal);
    guard.restore().map_err(|e| e.to_string())?;
    // Dropping the remote attachments sends detach, never close or signal.
    drop(mux);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("openagents-mux: {e}");
        std::process::exit(1);
    }
}
