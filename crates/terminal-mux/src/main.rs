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
#[derive(Default)]
struct NoLocal {
    resource: Option<workbench::ResourceRef>,
    reason: Option<String>,
}
struct ResourceOnly {
    reference: workbench::ResourceRef,
    pending: bool,
    reason: String,
}
impl Attachment for ResourceOnly {
    fn input_available(&self) -> bool {
        false
    }
    fn host_answers(&self) -> bool {
        true
    }
    fn input(&self, _: &[u8]) {}
    fn resize(&self, _: u16, _: u16) {}
    fn close(&self) {}
    fn poll(&mut self) -> Option<terminal_core::pty::Event> {
        if !self.pending {
            return None;
        }
        self.pending = false;
        Some(terminal_core::pty::Event::Status(format!(
            "{:?} {} on {:?}: {}; retained reference only. No source content was read.",
            self.reference.kind, self.reference.id, self.reference.host, self.reason
        )))
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        None
    }
    fn directory(&self) -> Option<String> {
        None
    }
}

impl Transport for NoLocal {
    fn shell(&self) -> &Path {
        Path::new("/bin/sh")
    }
    fn open(&self, _: &Program, _: u16, _: u16) -> Result<Box<dyn Attachment>, String> {
        match &self.resource {
            Some(resource) => Ok(Box::new(ResourceOnly {
                reference: resource.clone(),
                pending: true,
                reason: self
                    .reason
                    .clone()
                    .unwrap_or_else(|| "No adapter for this resource on this client".into()),
            })),
            None => Err("This client attaches only named host terminals.".into()),
        }
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
    let mut session_path = None;
    let mut layout_path = None;
    let mut device = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--store" => {
                store = Some(PathBuf::from(
                    args.next().ok_or("--store needs a directory")?,
                ))
            }
            "--host" => host = Some(args.next().ok_or("--host needs a key")?),
            "--session" => {
                session_path = Some(PathBuf::from(args.next().ok_or("--session needs a file")?))
            }
            "--layout" => {
                layout_path = Some(PathBuf::from(args.next().ok_or("--layout needs a file")?))
            }
            "--device" => device = Some(args.next().ok_or("--device needs an identity digest")?),
            "--reference" => {
                let value = args.next().ok_or("--reference needs GENERATION:TERMINAL")?;
                let (g, t) = value
                    .split_once(':')
                    .ok_or("--reference needs GENERATION:TERMINAL")?;
                references.push(terminal_remote::reference(g, t)?);
            }
            "--help" => {
                println!(
                    "openagents-mux --store PAIRED_STORE --host HOST_KEY --reference GENERATION:TERMINAL [--reference ...]\nopenagents-mux --store PAIRED_STORE --session SAVED_JSON [--layout DEVICE_LAYOUT_JSON --device PUBLIC_ID]\nCtrl+B then n/p: tabs; % or double quote: split; o: focus; z: zoom; k/j: blocks; r: live; d: detach. Ctrl+B twice forwards the prefix."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    let store = store.ok_or("--store is required; no default owner home is opened")?;
    if std::env::var("TERM").is_ok_and(|t| t == "dumb" || t.is_empty()) {
        return Err("This outer terminal has no cursor-addressed redraw capability. Use the hook-only terminal instead.".into());
    }
    fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 {
            return Err("The saved view exceeds the client limit.".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
    let mut mux = if let Some(path) = session_path {
        if host.is_some() || !references.is_empty() {
            return Err(
                "A saved session already names each owner; do not supply --host or --reference."
                    .into(),
            );
        }
        let saved: workbench_session::Saved = load(&path)?;
        let members = saved.members()?;
        if members.is_empty() || members.len() > terminal_mux::MAX_PANES {
            return Err(
                "This client supports one to eight session members; none were mounted.".into(),
            );
        }
        let local: Option<workbench_session::Override> =
            layout_path.as_ref().map(|p| load(p)).transpose()?;
        let actual_device = terminal_remote::device_id(&store)?;
        if device
            .as_ref()
            .is_some_and(|claimed| claimed != &actual_device)
        {
            return Err("The layout device differs from this paired store identity.".into());
        }
        let device = actual_device;
        // Validate all references and the local layout before contacting an owner.
        saved.layout(&device, local.as_ref())?;
        let mut transports = Vec::new();
        for member in members {
            let resource = member.resource;
            if resource.kind == workbench::Kind::Terminal
                && let workbench::Host::Paired { key } = &resource.host
            {
                let reference = terminal_remote::reference(
                    resource
                        .generation
                        .as_deref()
                        .ok_or("A terminal needs its original generation")?,
                    &resource.id,
                )?;
                let transport = terminal_remote::Remote::paired(
                    &store,
                    key.clone(),
                    Some(reference),
                    Arc::new(NoLocal::default()),
                );
                // A missing store/grant for one owner is a retained unavailable pane,
                // never a reason to abort other owners or open a local shell.
                match transport {
                    Ok(mut remote) => {
                        remote.pin_admission();
                        transports.push(Sessions(Arc::new(remote)));
                    }
                    Err(error) => transports.push(Sessions(Arc::new(NoLocal {
                        resource: Some(resource),
                        reason: Some(format!("Owner transport unavailable: {error}")),
                    }))),
                }
            } else {
                transports.push(Sessions(Arc::new(NoLocal {
                    resource: Some(resource),
                    reason: None,
                })));
            }
        }
        terminal_mux::Mux::attach_saved(transports, &saved, &device, local.as_ref())?
    } else {
        if layout_path.is_some() || device.is_some() {
            return Err("--layout and --device require --session.".into());
        }
        let host = host.ok_or("--host is required")?;
        if references.is_empty() || references.len() > terminal_mux::MAX_PANES {
            return Err("Name between one and eight existing terminal references.".into());
        }
        let mut transports = Vec::new();
        for reference in references {
            let mut remote = terminal_remote::Remote::paired(
                &store,
                host.clone(),
                Some(reference),
                Arc::new(NoLocal::default()),
            )?;
            remote.pin_admission();
            transports.push(Sessions(Arc::new(remote)));
        }
        terminal_mux::Mux::attach(transports)?
    };
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
