//! The Computers screens on the terminal adapter.
//!
//! ```sh
//! cargo run -p coder-computers --example terminal            # interactive fixture
//! cargo run -p coder-computers --example terminal -- --print # each screen as text
//! cargo run -p coder-computers --example terminal -- --live ~/.openagents/coder-computers
//! ```
//!
//! Tab and the arrow keys move focus, Enter or Space activates, and `q` or
//! Esc quits. When a screen asks for input, type it and press Enter; Esc
//! cancels. By default the offline fixture contacts no host, relay, or SSH
//! server. `--live DIR` uses the live service: it keeps this client's
//! device key and grants owner-only in `DIR` and reaches real hosts.
//! `--loopback-test` admits a `ws://` loopback relay for a local test, and
//! `--same-machine` states that the hosts run on this computer, which
//! allows loopback routes.
//!
//! To offer **Connect over SSH**, name the `coder` release archives to
//! install and the host's owner and relay:
//!
//! ```sh
//! cargo run -p coder-computers --features ssh --example terminal -- --live DIR \
//!   --ssh-archive linux/x86_64=coder-linux-x86_64.tar.gz \
//!   --owner OWNER_PUBLIC_KEY --relay wss://relay.example/
//! ```
//!
//! Each archive is pinned to the SHA-256 it has when the example starts.
//! Password prompts appear as masked input.
use coder_computers::live::{FileStore, Live, Settings, SshSetup, load_or_create_key};
use coder_computers::synthetic::Synthetic;
use coder_computers::{Capabilities, Computers, ComputersService, Platform};
use coder_reach::hints::Locality;
use coder_terminal::native::{Focus, render};
use coder_terminal::{Guard, Ladder};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};
use std::io;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The service the flags select. The runtime must outlive a live service.
fn service(runtime: &tokio::runtime::Runtime) -> Result<Box<dyn ComputersService + Send>, String> {
    let args: Vec<String> = std::env::args().collect();
    let Some(directory) = args
        .iter()
        .position(|arg| arg == "--live")
        .and_then(|index| args.get(index + 1))
    else {
        return Ok(Box::new(Synthetic::fixture(Platform::Terminal, now)));
    };
    let directory = std::path::Path::new(directory);
    let mut settings = Settings::new(Platform::Terminal);
    if args.iter().any(|arg| arg == "--loopback-test") {
        settings.policy = coder_access::RelayPolicy::LoopbackTest;
    }
    if args.iter().any(|arg| arg == "--same-machine") {
        settings.locality = Locality::SameMachine;
    }
    settings.ssh = ssh_setup(
        &args,
        settings.policy == coder_access::RelayPolicy::LoopbackTest,
    )?;
    let secret = load_or_create_key(directory)?;
    let store = FileStore::open(directory)?;
    let live = Live::open(settings, secret, Box::new(store), runtime.handle().clone())
        .map_err(|error| error.to_string())?;
    Ok(Box::new(live))
}

/// Every value that follows `flag`.
fn values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
        .collect()
}

/// The SSH setup from `--ssh-archive OS/ARCH=PATH`, `--owner`, and
/// `--relay`, or `None` when no archive is named.
fn ssh_setup(args: &[String], loopback_test: bool) -> Result<Option<SshSetup>, String> {
    let archives = values(args, "--ssh-archive");
    if archives.is_empty() {
        return Ok(None);
    }
    let mut artifacts = Vec::new();
    for archive in archives {
        let (platform, path) = archive
            .split_once('=')
            .ok_or("--ssh-archive takes OS/ARCH=PATH")?;
        let os = match platform.split('/').next() {
            Some("linux") => coder_ssh::Os::Linux,
            Some("macos") => coder_ssh::Os::Macos,
            _ => return Err("--ssh-archive OS is linux or macos".into()),
        };
        let arch = match platform.split('/').nth(1) {
            Some("x86_64") => coder_ssh::Arch::X86_64,
            Some("aarch64") => coder_ssh::Arch::Aarch64,
            _ => return Err("--ssh-archive ARCH is x86_64 or aarch64".into()),
        };
        let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        artifacts.push(coder_ssh::Artifact {
            os,
            arch,
            sha256: sha256_hex(&bytes),
            archive: path.into(),
        });
    }
    let owner = values(args, "--owner")
        .first()
        .copied()
        .ok_or("--ssh-archive needs --owner")?;
    let relay = values(args, "--relay")
        .first()
        .copied()
        .ok_or("--ssh-archive needs --relay")?;
    let release = coder_ssh::Release::new(artifacts).map_err(|error| error.to_string())?;
    SshSetup::coder(release, owner, relay, loopback_test)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn open(runtime: &tokio::runtime::Runtime) -> Result<Computers, String> {
    Computers::new(
        service(runtime)?,
        Capabilities {
            platform: Platform::Terminal,
            camera: false,
        },
        "computers:terminal",
    )
    .map_err(|error| error.to_string())
}

fn print(runtime: &tokio::runtime::Runtime) -> Result<(), String> {
    let mut computers = open(runtime)?;
    let ladder = Ladder::new(coder_terminal::Colors::None);
    for press in [
        None,
        Some("first-run-continue"),
        Some("tab-add"),
        Some("tab-activity"),
        Some("tab-computers"),
        Some("host-0-access"),
    ] {
        if let Some(node) = press {
            let view = computers.view().ok_or("no view")?.view();
            let activation = rust_native::Activation {
                instance: view.instance.clone(),
                revision: view.revision,
                node: node.into(),
            };
            computers
                .activate(&activation)
                .map_err(|refusal| refusal.reason())?;
        }
        let view = computers.view().ok_or("no view")?.view();
        println!(
            "===== {:?} (revision {})",
            computers.screen(),
            view.revision
        );
        for line in render(view, ladder, None).lines {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            println!("{text}");
        }
    }
    Ok(())
}

fn interactive(runtime: &tokio::runtime::Runtime) -> Result<(), String> {
    let mut computers = open(runtime)?;
    let guard = Guard::full_screen().map_err(|error| error.to_string())?;
    guard.arm_panic_hook();
    let mut terminal =
        Terminal::new(CrosstermBackend::new(io::stdout())).map_err(|error| error.to_string())?;
    let ladder = Ladder::from_environment();
    let mut focus: Option<String> = None;
    let mut draft = String::new();
    let mut token: Option<String> = None;
    loop {
        let view = computers.view().ok_or("no view")?.view().clone();
        let mut keys = Focus::new(&view, focus.as_deref());
        let input = computers.input().cloned();
        // A new request, such as the next ssh prompt, starts with an empty field.
        let asking = input.as_ref().map(|input| input.token.clone());
        if asking != token {
            draft.clear();
            token = asking;
        }
        terminal
            .draw(|frame| {
                let drawn = render(&view, ladder, keys.current());
                let mut lines = drawn.lines;
                if let Some(input) = &input {
                    lines.push(Line::raw(""));
                    lines.push(Line::raw(input.prompt.clone()));
                    // A secret is masked while it's typed.
                    let shown = if input.secret {
                        "*".repeat(draft.chars().count())
                    } else {
                        draft.clone()
                    };
                    lines.push(Line::raw(format!("{}: {shown}_", input.label)));
                }
                let height = frame.area().height as usize;
                let scroll = drawn.focus_line.unwrap_or(0).saturating_sub(height / 2);
                frame.render_widget(
                    Paragraph::new(lines)
                        .wrap(Wrap { trim: false })
                        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
                    frame.area(),
                );
            })
            .map_err(|error| error.to_string())?;
        // Redraw every second so a host's status moves without a key press.
        if !event::poll(std::time::Duration::from_secs(1)).map_err(|error| error.to_string())? {
            let _ = computers.refresh();
            continue;
        }
        let Event::Key(key) = event::read().map_err(|error| error.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if let Some(input) = &input {
            match key.code {
                KeyCode::Esc => {
                    let _ = computers.cancel_input(&input.token);
                    draft.clear();
                }
                KeyCode::Enter => {
                    let _ = computers.submit(&input.token, &draft);
                    draft.clear();
                }
                KeyCode::Backspace => {
                    draft.pop();
                }
                KeyCode::Char(c) if draft.len() < input.max_bytes => draft.push(c),
                _ => {}
            }
            continue;
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            break;
        }
        if let Some(activation) = keys.handle(key) {
            // A refusal shows on the screen as its notice.
            let _ = computers.activate(&activation);
        }
        focus = keys.current().map(str::to_owned);
    }
    drop(terminal);
    guard.restore().map_err(|error| error.to_string())
}

fn main() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("terminal example: {error}");
            std::process::exit(1);
        }
    };
    let result = if std::env::args().any(|arg| arg == "--print") {
        print(&runtime)
    } else {
        interactive(&runtime)
    };
    if let Err(error) = result {
        eprintln!("terminal example: {error}");
        std::process::exit(1);
    }
}
