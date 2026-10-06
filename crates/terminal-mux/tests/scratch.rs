//! An outer PTY mounts two admitted host PTYs, then reattaches the same processes.
#![cfg(unix)]
use coder_pty::{
    host::{self, Config, Delivery, Host, Right, Rights},
    wire::{Attach, Body, Detach, Input, Launch, Mode, Open, Size, Status, TerminalRef, Value},
};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use terminal_core::{
    bridge,
    pty::{Attachment, Event, Program, Sessions, Transport},
};
const PRINCIPAL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "{:064x}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    )
}
struct Grant;
impl Rights for Grant {
    fn holds(&self, p: &str, _: Right) -> bool {
        p == PRINCIPAL
    }
}
struct Mount {
    host: Arc<Host>,
    reference: TerminalRef,
}
struct Attached {
    host: Arc<Host>,
    reference: TerminalRef,
    attachment: String,
    frames: mpsc::Receiver<Delivery>,
    streams: coder_vt::Streams,
    pending: VecDeque<Event>,
    available: bool,
}
impl Attachment for Attached {
    fn host_grid(&self) -> bool {
        true
    }
    fn host_answers(&self) -> bool {
        true
    }
    fn input_available(&self) -> bool {
        self.available
    }
    fn reference(&self) -> Option<TerminalRef> {
        Some(self.reference.clone())
    }
    fn input(&self, b: &[u8]) {
        self.host
            .input(
                PRINCIPAL,
                &Input::new(id(), self.reference.clone(), b)
                    .from_attachment(self.attachment.clone()),
            )
            .unwrap();
    }
    fn resize(&self, r: u16, c: u16) {
        self.host
            .resize(
                PRINCIPAL,
                &coder_pty::wire::Resize::new(id(), self.reference.clone(), Size::new(r, c))
                    .from_attachment(self.attachment.clone()),
            )
            .unwrap();
    }
    fn close(&self) {
        panic!("a mount must never close a host terminal")
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        None
    }
    fn directory(&self) -> Option<String> {
        None
    }
    fn poll(&mut self) -> Option<Event> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        match self.frames.try_recv().ok()? {
            Delivery::Records(part) => {
                for e in self.streams.push(&part).unwrap() {
                    if let coder_vt::StreamEvent::Ready { terminal, .. } = e {
                        self.pending.push_back(Event::Snapshot(terminal));
                    }
                }
                self.pending.pop_front()
            }
            Delivery::Frame(frame) => match frame.body {
                Body::Output { data, .. } => Some(Event::Output(data)),
                Body::Exit { .. } => {
                    self.available = false;
                    Some(Event::End("Process ended".into()))
                }
                Body::Typist { typist, size } => {
                    self.available =
                        typist.is_none() || typist.as_deref() == Some(&self.attachment);
                    Some(Event::Size(size.rows, size.cols))
                }
                Body::Gap { .. } => Some(Event::Gap),
                Body::Detached { .. } => {
                    self.available = false;
                    Some(Event::End("Detached".into()))
                }
                _ => Some(Event::Status("Admitted scratch host".into())),
            },
        }
    }
}
impl Drop for Attached {
    fn drop(&mut self) {
        let _ = self.host.detach(
            PRINCIPAL,
            &Detach::new(id(), self.reference.clone(), self.attachment.clone()),
        );
    }
}
impl Transport for Mount {
    fn shell(&self) -> &Path {
        Path::new("/bin/sh")
    }
    fn open(&self, _: &Program, _: u16, _: u16) -> Result<Box<dyn Attachment>, String> {
        let (sink, frames) = host::deliveries(1024);
        let request = Attach::new(id(), self.reference.clone(), Mode::Interact, 0, 1 << 20)
            .joining(coder_pty::ext::Join::Snapshot)
            .with_effects()
            .with_typist();
        let (_, Value::Attached { attachment, .. }) = self
            .host
            .attach(PRINCIPAL, &request, Box::new(sink))
            .map_err(|e| format!("{e:?}"))?
        else {
            return Err("attachment refused".into());
        };
        Ok(Box::new(Attached {
            host: self.host.clone(),
            reference: self.reference.clone(),
            attachment,
            frames,
            streams: coder_vt::Streams::new(self.reference.clone(), 5000),
            pending: VecDeque::new(),
            available: false,
        }))
    }
    fn shutdown(&self) {}
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn request(&self, _: &bridge::Request) -> Result<bridge::Connection, String> {
        Err("Use retained thread view".into())
    }
    fn git_summary(&self, _: u64, _: String) -> mpsc::Receiver<(u64, String, String)> {
        mpsc::channel().1
    }
    fn open_link(&self, _: &str) -> Result<(), String> {
        Err("Text only".into())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, _: &str) -> Result<(), String> {
        Err("Text only".into())
    }
}
#[test]
fn outer_child() {
    let Some(root) = std::env::var_os("OPENAGENTS_MUX_SCRATCH") else {
        return;
    };
    let workspace = "b".repeat(64);
    let mut config = Config::new().workspace(&workspace, PathBuf::from(&root));
    config.base_env.retain(|(k, _)| k != "HOME");
    config
        .base_env
        .push(("HOME".into(), PathBuf::from(&root).display().to_string()));
    config.emulator = Some(coder_vt::Authority::factory(5000));
    let hosts = [
        Arc::new(Host::new(config.clone(), Arc::new(Grant))),
        Arc::new(Host::new(config, Arc::new(Grant))),
    ];
    let mut refs = Vec::new();
    for host in &hosts {
        let launch=Launch::Command{program:"/bin/sh".into(),args:vec!["-c".into(),"stty -echo; printf x >> counter; printf 'HOST-%s\\n' $$; while IFS= read -r line; do eval \"$line\"; done".into()]};
        let (Status::Accepted, Value::Opened { terminal, .. }) = host
            .open(
                PRINCIPAL,
                &Open::new(id(), &workspace, "", launch, Size::new(24, 80)),
            )
            .unwrap()
        else {
            panic!("open")
        };
        refs.push(terminal);
    }
    let groups = refs
        .iter()
        .enumerate()
        .map(|(i, r)| hosts[i].process_group(r).unwrap())
        .collect::<Vec<_>>();
    let saved = workbench_session::Saved {
        v: workbench_session::SCHEMA.into(),
        owner: workbench::Host::Paired {
            key: "f".repeat(64),
        },
        record: coder_pty::ext::SessionRecord {
            session: Some("e".repeat(64)),
            revision: 1,
            name: "Two independently admitted hosts".into(),
            members: refs
                .iter()
                .enumerate()
                .map(|(i, r)| coder_pty::ext::Member::Resource {
                    member: i as u16 + 1,
                    resource: serde_json::to_value(workbench::ResourceRef::terminal(
                        workbench::Host::Paired {
                            key: if i == 0 { "a" } else { "b" }.repeat(64),
                        },
                        r.generation.clone(),
                        r.terminal.clone(),
                    ))
                    .unwrap(),
                })
                .collect(),
            layout: coder_pty::ext::Layout {
                tabs: vec![coder_pty::ext::Tab {
                    name: "shared".into(),
                    root: coder_pty::ext::Node::Split {
                        axis: coder_pty::ext::Axis::Columns,
                        ratio: 500,
                        first: Box::new(coder_pty::ext::Node::Pane { member: 1 }),
                        second: Box::new(coder_pty::ext::Node::Pane { member: 2 }),
                    },
                }],
                active: 0,
            },
        },
    };
    let retained = serde_json::to_vec(&saved).unwrap();
    for round in 0..2 {
        let transports = refs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                Sessions(Arc::new(Mount {
                    host: hosts[i].clone(),
                    reference: r.clone(),
                }))
            })
            .collect();
        let restored: workbench_session::Saved = serde_json::from_slice(&retained).unwrap();
        let device = if round == 0 { "c" } else { "d" }.repeat(64);
        let mut layout = restored.record.layout.clone();
        if round == 1
            && let coder_pty::ext::Node::Split { ratio, .. } = &mut layout.tabs[0].root
        {
            *ratio = 400;
        }
        let local = workbench_session::Override {
            device: device.clone(),
            session: restored.record.session.clone(),
            revision: 1,
            layout,
        };
        let mut mux =
            terminal_mux::Mux::attach_saved(transports, &restored, &device, Some(&local)).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), retained);
        let guard = coder_terminal::Guard::full_screen_with_mouse().unwrap();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !mux.detached {
            assert!(Instant::now() < deadline, "outer fixture timed out");
            mux.pump();
            terminal.draw(|f| mux.draw(f)).unwrap();
            if crossterm::event::poll(Duration::from_millis(20)).unwrap() {
                mux.event(
                    crossterm::event::read().unwrap(),
                    ratatui::layout::Rect::new(0, 0, 80, 24),
                );
            }
        }
        drop(terminal);
        guard.restore().unwrap();
        drop(mux);
        assert_eq!(
            groups,
            refs.iter()
                .enumerate()
                .map(|(i, r)| hosts[i].process_group(r).unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            std::fs::read(PathBuf::from(&root).join("counter")).unwrap(),
            b"xx"
        );
        println!("ROUND-{round}-SAME-PROCESSES");
        std::io::stdout().flush().unwrap();
    }
    for host in hosts {
        host.shutdown();
    }
}
fn read_until(master: &mut std::fs::File, text: &str, log: &mut Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(25);
    while !String::from_utf8_lossy(log).contains(text) {
        assert!(
            Instant::now() < deadline,
            "waiting for {text}: {}",
            String::from_utf8_lossy(log)
        );
        let mut poll = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe { libc::poll(&mut poll, 1, 100) };
        if poll.revents & libc::POLLIN != 0 {
            let mut bytes = [0; 8192];
            let n = master.read(&mut bytes).unwrap();
            assert!(n > 0);
            log.extend_from_slice(&bytes[..n]);
        }
    }
}
#[test]
fn real_outer_pty_tabs_nested_fullscreen_detach_and_reattach() {
    let root = tempfile::tempdir().unwrap();
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        assert_eq!(
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size
            ),
            0
        )
    };
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "outer_child", "--nocapture"])
        .env("OPENAGENTS_MUX_SCRATCH", root.path())
        .env("TERM", "xterm-256color")
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(command.spawn().unwrap());
    let mut log = Vec::new();
    read_until(&mut master, "HOST-", &mut log);
    read_until(&mut master, "typist", &mut log);
    master
        .write_all(
            b"printf '\033[?1049hFULLSCREEN'; read answer; printf '\033[?1049lRESTORED\\n'\r",
        )
        .unwrap();
    read_until(&mut master, "FULLSCREEN", &mut log);
    master.write_all(b"\r").unwrap();
    read_until(&mut master, "RESTORED", &mut log);
    master.write_all(b"\x02%\x02o\x02n\x02d").unwrap();
    read_until(&mut master, "ROUND-0-SAME-PROCESSES", &mut log);
    // The second attachment replays the original processes' grid and output.
    std::thread::sleep(Duration::from_millis(100));
    master.write_all(b"\x02d").unwrap();
    read_until(&mut master, "ROUND-1-SAME-PROCESSES", &mut log);
    assert!(child.0.wait().unwrap().success());
    let mut termios = std::mem::MaybeUninit::uninit();
    unsafe {
        assert_eq!(libc::tcgetattr(slave.as_raw_fd(), termios.as_mut_ptr()), 0);
        assert_ne!(termios.assume_init().c_lflag & libc::ICANON, 0)
    };
    assert!(log.windows(8).any(|w| w == b"\x1b[?1049l"));
}
