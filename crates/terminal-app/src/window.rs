use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use terminal_gfx::{KeyIn, Mount, Overlay, mouse::Button};
use verse_gfx::ui::{Atlas, UiBatch, UiVertex};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[derive(Default)]
pub struct Options {
    capability_flow: Option<PathBuf>,
    root: Option<PathBuf>,
    knowledge_workbench: Option<PathBuf>,
    contribution_workbench: Option<PathBuf>,
    quest_workbench: Option<PathBuf>,
    compute_workbench: Option<PathBuf>,
    knowledge_review: Option<PathBuf>,
    knowledge_operator: Option<String>,
    knowledge_evaluator: Option<String>,
    host: Option<String>,
    task: Option<String>,
    paired_store: Option<PathBuf>,
    reference: Option<String>,
    shell: Option<PathBuf>,
    socket: Option<PathBuf>,
    stress_out: Option<PathBuf>,
    busy: Option<usize>,
    seconds: Option<u32>,
    warmup: Option<u32>,
    startup_out: Option<PathBuf>,
    /// Open behind the active app and keep drawing while covered, for
    /// automated runs on a computer someone is using.
    background: bool,
    latency_out: Option<PathBuf>,
}

pub fn run() -> Result<(), String> {
    let started = Instant::now();
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--capability-flow" => {
                options.capability_flow = Some(
                    args.next()
                        .ok_or("Expected a retained capability flow directory")?
                        .into(),
                )
            }
            "--startup-out" => {
                options.startup_out =
                    Some(args.next().ok_or("Expected a startup report path")?.into())
            }
            "--stress-out" => {
                options.stress_out = Some(args.next().ok_or("Expected a report path")?.into())
            }
            "--busy" => {
                options.busy = Some(
                    args.next()
                        .ok_or("Expected a pane count")?
                        .parse()
                        .map_err(|_| "Invalid pane count")?,
                )
            }
            "--seconds" => {
                options.seconds = Some(
                    args.next()
                        .ok_or("Expected seconds")?
                        .parse()
                        .map_err(|_| "Invalid seconds")?,
                )
            }
            "--warmup" => {
                options.warmup = Some(
                    args.next()
                        .ok_or("Expected seconds")?
                        .parse()
                        .map_err(|_| "Invalid seconds")?,
                )
            }
            "--help" | "-h" => {
                println!(
                    "OpenAgents Terminal\nUsage: openagents-terminal [--root DIR] [--shell PATH] [--socket PATH] [--host KEY --paired-store DIR --terminal GENERATION/TERMINAL]\n\nStarts your login shell under one fixed sheet. --capability-flow DIR opens its retained capability workflow beside the shell. Type a command or a question; ENTER runs or asks. F1 shows the keys.\nCtrl+B % or \" splits; Ctrl+B c opens a tab. Cmd+Q exits on macOS.\n--root isolates shell and helper HOME for a scratch run.\nThe native package supports macOS arm64; Linux is a development platform.\n--stress-out FILE runs the shared workload; --busy N --seconds S --warmup S configure it.\n--startup-out FILE records process entry to first presented frame.\n--background opens behind the active app and keeps drawing while covered.\n--latency-out FILE records key-to-frame times on the sheet until quit."
                );
                return Ok(());
            }
            "--version" => {
                println!("openagents-terminal {}", env!("CARGO_PKG_VERSION"));
                if let Some(commit) = option_env!("OPENAGENTS_BUILD_COMMIT") {
                    println!("commit {commit}");
                }
                return Ok(());
            }
            "--background" => options.background = true,
            "--latency-out" => {
                options.latency_out =
                    Some(args.next().ok_or("Expected a latency report path")?.into())
            }
            "--knowledge-operator" => {
                options.knowledge_operator = Some(
                    args.next()
                        .ok_or("Expected the trusted operator public key")?,
                )
            }
            "--knowledge-evaluator" => {
                options.knowledge_evaluator = Some(
                    args.next()
                        .ok_or("Expected the trusted evaluator public key")?,
                )
            }
            "--knowledge-review" => {
                options.knowledge_review = Some(
                    args.next()
                        .ok_or("Expected retained knowledge evidence")?
                        .into(),
                )
            }
            "--quest-workbench" => {
                options.quest_workbench = Some(
                    args.next()
                        .ok_or("Expected private quest configuration")?
                        .into(),
                )
            }
            "--contribution-workbench" => {
                options.contribution_workbench = Some(
                    args.next()
                        .ok_or("Expected private contribution configuration")?
                        .into(),
                )
            }
            "--compute-workbench" => {
                options.compute_workbench = Some(
                    args.next()
                        .ok_or("Expected private compute configuration")?
                        .into(),
                )
            }
            "--knowledge-workbench" => {
                options.knowledge_workbench = Some(
                    args.next()
                        .ok_or("Expected a retained knowledge session")?
                        .into(),
                )
            }
            "--task" => {
                options.task = Some(args.next().ok_or("--task needs a studio task identity")?)
            }
            "--host" => options.host = Some(args.next().ok_or("--host needs a host key")?),
            "--paired-store" => {
                options.paired_store = Some(
                    args.next()
                        .ok_or("--paired-store needs a directory")?
                        .into(),
                )
            }
            "--terminal" => {
                options.reference = Some(args.next().ok_or("--terminal needs generation/terminal")?)
            }
            "--root" => options.root = Some(args.next().ok_or("--root needs a directory")?.into()),
            "--shell" => options.shell = Some(args.next().ok_or("--shell needs a path")?.into()),
            "--socket" => options.socket = Some(args.next().ok_or("--socket needs a path")?.into()),
            _ => return Err(format!("unknown option: {argument}")),
        }
    }
    if options.knowledge_review.is_some()
        && (options.knowledge_operator.is_none() || options.knowledge_evaluator.is_none())
    {
        return Err("--knowledge-review requires --knowledge-operator and --knowledge-evaluator public keys".into());
    }
    if options.knowledge_review.is_some() && options.knowledge_workbench.is_none() {
        return Err("--knowledge-review requires --knowledge-workbench".into());
    }
    if options.host.is_none()
        && (options.paired_store.is_some() || options.reference.is_some() || options.task.is_some())
    {
        return Err("--paired-store, --terminal, and --task require --host.".into());
    }
    if options.root.as_ref().is_some_and(|root| !root.is_dir()) {
        return Err("the scratch root must be an existing directory".into());
    }
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_activate_ignoring_other_apps(!options.background);
    }
    let event_loop = builder.build().map_err(|error| error.to_string())?;
    let mut app = App {
        options,
        state: None,
        error: None,
        point: [0.0; 2],
        hidden: false,
        started,
        startup_recorded: false,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| error.to_string())?;
    app.error.map_or(Ok(()), Err)
}

struct State {
    window: Arc<Window>,
    gpu: Gpu,
    atlas: Atlas,
    terminal: Overlay,
    stress: Option<terminal_gfx::stress::Driver>,
}
struct App {
    options: Options,
    state: Option<State>,
    error: Option<String>,
    point: [f32; 2],
    hidden: bool,
    started: Instant,
    startup_recorded: bool,
}
impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.error = Some(error);
        if let Some(state) = &mut self.state {
            state.terminal.shutdown();
        }
        event_loop.exit();
    }
    fn draw(&mut self) -> Result<(), String> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let start = Instant::now();
        let size = state.window.inner_size();
        let mut batch = UiBatch::default();
        state.terminal.draw(
            &mut batch,
            &mut state.atlas,
            [size.width as f32, size.height as f32],
        );
        let presented = state.gpu.draw(&batch, &state.atlas)?;
        state.terminal.frame_done(start);
        if presented {
            state.terminal.core.paper.presented();
        }
        if presented && !self.startup_recorded {
            self.startup_recorded = true;
            if let Some(path) = &self.options.startup_out {
                let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0;
                let report = serde_json::json!({"schema": "openagents.native-terminal.startup.v1", "elapsed_ms": elapsed_ms,
                    "start": "process entry before argument parsing", "end": "first GPU frame submitted and presented",
                    "target_ms": 2000, "target_met": elapsed_ms <= 2000.0, "version": env!("CARGO_PKG_VERSION"),
                    "compiled_commit": option_env!("OPENAGENTS_BUILD_COMMIT")});
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}
impl App {
    /// Writes the key-to-frame latencies the sheet recorded, when asked.
    fn write_latency(&self) {
        let (Some(path), Some(state)) = (&self.options.latency_out, &self.state) else {
            return;
        };
        let mut samples = state.terminal.core.paper.latencies.clone();
        samples.sort_by(f64::total_cmp);
        let at = |q: f64| {
            samples
                .get(((samples.len() as f64 - 1.0) * q).round().max(0.0) as usize)
                .copied()
        };
        let report = serde_json::json!({"schema": "openagents.native-terminal.latency.v1",
            "start": "key handled (input line edited)", "end": "next GPU frame submitted and presented",
            "samples": samples.len(), "p50_ms": at(0.5), "p95_ms": at(0.95), "max_ms": samples.last(),
            "version": env!("CARGO_PKG_VERSION"), "compiled_commit": option_env!("OPENAGENTS_BUILD_COMMIT")});
        if let Ok(json) = serde_json::to_vec_pretty(&report) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// The window: the sheet's fixed 3:2 size in points, which nobody resizes.
#[must_use]
pub fn attributes() -> winit::window::WindowAttributes {
    let [w, h] = terminal_gfx::SHEET_POINTS;
    Window::default_attributes()
        .with_title("OpenAgents Terminal")
        .with_inner_size(winit::dpi::LogicalSize::new(f64::from(w), f64::from(h)))
        .with_resizable(false)
        .with_enabled_buttons(
            winit::window::WindowButtons::CLOSE | winit::window::WindowButtons::MINIMIZE,
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_is_a_fixed_three_by_two_sheet() {
        let attributes = super::attributes();
        assert!(!attributes.resizable);
        assert_eq!(
            attributes.inner_size,
            Some(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
                1200.0, 800.0
            )))
        );
        assert!(
            !attributes
                .enabled_buttons
                .contains(winit::window::WindowButtons::MAXIMIZE)
        );
        let [w, h] = terminal_gfx::SHEET_POINTS;
        assert!((w / h - 1.5).abs() < f32::EPSILON);
    }
    struct NoEffects;
    impl terminal_core::pty::Transport for NoEffects {
        fn shell(&self) -> &std::path::Path {
            std::path::Path::new("/unused")
        }
        fn open(
            &self,
            _: &terminal_core::pty::Program,
            _: u16,
            _: u16,
        ) -> Result<Box<dyn terminal_core::pty::Attachment>, String> {
            panic!("Viewing a candidate must not open a terminal")
        }
        fn git_summary(
            &self,
            _: u64,
            _: String,
        ) -> std::sync::mpsc::Receiver<(u64, String, String)> {
            panic!("Candidate inspection must not read git")
        }
        fn open_link(&self, _: &str) -> Result<(), String> {
            panic!("Candidate inspection must not open links")
        }
        fn clipboard(&self) -> Option<String> {
            None
        }
        fn copy(&self, _: &str) -> Result<(), String> {
            panic!("Candidate inspection must not write the clipboard")
        }
        fn shutdown(&self) {}
        fn thread_program(&self) -> Option<terminal_core::pty::Program> {
            None
        }
        fn resolve(&self, _: &str) -> Option<std::path::PathBuf> {
            None
        }
        fn request(
            &self,
            _: &terminal_core::bridge::Request,
        ) -> Result<terminal_core::bridge::Connection, String> {
            panic!("Viewing a candidate must not dispatch")
        }
    }
    #[test]
    fn cited_candidate_body_and_unknown_costs_reach_the_native_sheet() {
        use knowledge::workbench::{Adapter, Selection, Session, Source};
        let selection = Selection {
            sources: vec![Source {
                task: "source-task".into(),
                run: "source-run".into(),
                group: "source-group".into(),
                artifact: knowledge::digest(b"source"),
                citation: "Shell manual".into(),
                disclosed: "Quoted arguments".into(),
            }],
            forbidden: Vec::new(),
            costs: std::collections::BTreeMap::from([
                ("acquisition_usd".into(), None),
                ("setup_usd".into(), None),
                ("checks_usd".into(), None),
            ]),
        };
        let mut session = Session::new(selection).unwrap();
        let document = "---\nid: shell.quoting\nversion: 1\nkind: method\ntitle: Shell quoting\nsummary: Quote arguments.\ntags: [shell]\napplies_when: Passing arguments.\nstatus: candidate\nauthor: scratch\nprovenance:\n  written_from: [reference]\n  cites: [\"Shell manual\"]\nevidence: []\n---\n\n## Details\n\nCited lesson text.\n";
        session
            .edit(document, &knowledge::lint::Corpus::default())
            .unwrap();
        let host = workbench::Host::Local {
            instance: "11".repeat(32),
        };
        let subject = workbench::pane::Subject::Record {
            host: host.clone(),
            id: "lesson".into(),
            revision: None,
        };
        let mut app = terminal_core::Application::new(terminal_core::pty::Sessions(
            std::sync::Arc::new(NoEffects),
        ));
        app.open = true;
        app.focused = true;
        app.paper.on = true;
        app.products.panes = std::mem::take(&mut app.products.panes).adapter(Box::new(Adapter {
            id: "lesson".into(),
            host,
            session,
        }));
        app.products
            .open(workbench::pane::PaneKind::Knowledge, &subject)
            .unwrap();
        let sheet = app.paper_sheet(120, 40, "00:00", "idle");
        let shown = sheet
            .rows
            .iter()
            .flatten()
            .map(|span| span.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(shown.contains("Cited lesson text."));
        assert!(shown.contains("source-task"));
        assert!(shown.contains("setup_usd: unknown"));
        assert!(sheet.caret.is_none());
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(attributes())
                    .map_err(|error| error.to_string())?,
            );
            let scale = window.scale_factor() as f32;
            let mut atlas = Atlas::new(16.0 * scale);
            atlas.reserve_glyphs(terminal_gfx::GLYPH_ROWS)?;
            let stress = self.options.stress_out.as_ref().map(|out| {
                terminal_gfx::stress::Driver::new(terminal_gfx::stress::Plan {
                    busy: self.options.busy.unwrap_or(8),
                    seconds: self.options.seconds.unwrap_or(15),
                    warmup: self.options.warmup.unwrap_or(4),
                    out: out.clone(),
                    spells: false,
                    label: "standalone; blocks enabled".into(),
                })
            });
            if let Some(driver) = &stress {
                std::fs::create_dir_all(driver.root()).map_err(|error| error.to_string())?;
            }
            let mut terminal = if let Some(host) = &self.options.host {
                let store = self
                    .options
                    .paired_store
                    .as_ref()
                    .ok_or("--host requires --paired-store")?;
                let reference = self
                    .options
                    .reference
                    .as_ref()
                    .map(|value| {
                        let (generation, terminal) = value
                            .split_once('/')
                            .ok_or("--terminal needs generation/terminal")?;
                        Ok::<_, String>(terminal_gfx::remote::reference(generation, terminal)?)
                    })
                    .transpose()?;
                let remote = terminal_gfx::remote::Remote::paired(
                    store,
                    host.clone(),
                    reference,
                    terminal_gfx::pty::for_user().0,
                )?;
                let remote = match &self.options.task {
                    Some(task) => remote.for_task(task.clone())?,
                    None => remote,
                };
                Overlay::on_host(remote)
            } else if let Some(driver) = &stress {
                Overlay::with(
                    driver.root(),
                    terminal_gfx::pty::user_shell(),
                    terminal_gfx::pty::Program::Shell,
                )
            } else if let Some(root) = &self.options.root {
                let shell = self
                    .options
                    .shell
                    .clone()
                    .unwrap_or_else(terminal_gfx::pty::user_shell);
                Overlay::with(root, shell, terminal_gfx::pty::Program::Shell)
            } else if let Some(shell) = &self.options.shell {
                let root = terminal_gfx::pty::user_home();
                Overlay::with(&root, shell.clone(), terminal_gfx::pty::Program::Shell)
            } else {
                Overlay::new()
            };
            terminal.studio_transport = std::sync::Arc::new(terminal_studio::Native::new(
                stress
                    .as_ref()
                    .map(|driver| driver.root().to_path_buf())
                    .or_else(|| self.options.root.clone()),
            ));
            terminal.mount = Mount::Window;
            terminal.open = true;
            if let Some(path) = &self.options.knowledge_workbench {
                let session = knowledge::workbench::Session::read(path)?;
                let host = workbench::Host::Local {
                    instance: knowledge::digest(path.as_os_str().as_encoded_bytes())[7..].into(),
                };
                let subject = workbench::pane::Subject::Record {
                    host: host.clone(),
                    id: "selected-knowledge".into(),
                    revision: None,
                };
                let candidate = knowledge::workbench::Adapter {
                    id: "selected-knowledge".into(),
                    host,
                    session,
                };
                let adapter: Box<dyn workbench::pane::PaneAdapter> =
                    if let Some(evidence) = &self.options.knowledge_review {
                        Box::new(knowledge::prospective::Adapter {
                            candidate,
                            evidence: knowledge::prospective::Bundle::read(evidence)?,
                            trust: knowledge::prospective::Trust {
                                operator: self
                                    .options
                                    .knowledge_operator
                                    .clone()
                                    .ok_or("Expected a trusted operator")?,
                                evaluator: self
                                    .options
                                    .knowledge_evaluator
                                    .clone()
                                    .ok_or("Expected a trusted evaluator")?,
                            },
                        })
                    } else {
                        Box::new(candidate)
                    };
                terminal.core.products.panes =
                    std::mem::take(&mut terminal.core.products.panes).adapter(adapter);
                terminal
                    .core
                    .products
                    .open(workbench::pane::PaneKind::Knowledge, &subject)?;
            }
            if let Some(path) = &self.options.quest_workbench {
                contribution_workbench::quests::host::mount(
                    &mut terminal.core,
                    contribution_workbench::quests::host::Config::load(path)?,
                )?;
            }
            if let Some(path) = &self.options.contribution_workbench {
                contribution_workbench::host::mount(
                    &mut terminal.core,
                    contribution_workbench::host::Config::load(path)?,
                )?;
            }
            if let Some(path) = &self.options.compute_workbench {
                compute_workbench::host::mount(
                    &mut terminal.core,
                    compute_workbench::host::Config::load(path)?,
                )?;
            }
            if let Some(root) = &self.options.capability_flow {
                use workbench::pane::{PaneKind, Subject};
                let owner = openagents_chat::plugin_workbench::Owner::open(
                    root.clone(),
                    openagents_chat::client::NoCoder,
                );
                let record = owner.read()?;
                let instance = openagents_chat::plugin_workbench::local_instance(root);
                let subject = Subject::Resource {
                    resource: workbench::ResourceRef::new(
                        workbench::Kind::Evidence,
                        workbench::Host::Local { instance },
                        record.source.flow,
                    ),
                };
                let draft = openagents_chat::plugin_workbench::DraftPane::open(root.clone());
                let subjects = draft.subjects()?;
                terminal.core.products.panes =
                    std::mem::take(&mut terminal.core.products.panes).adapter(Box::new(draft));
                for file in subjects {
                    terminal
                        .core
                        .products
                        .open(workbench::pane::PaneKind::Artifact, &file)?;
                }
                terminal.core.products.panes =
                    std::mem::take(&mut terminal.core.products.panes).adapter(Box::new(owner));
                terminal
                    .core
                    .products
                    .open(PaneKind::Evaluation, &subject)?;
                terminal.core.paper.on = true;
            }
            terminal.focused = true;
            terminal.fit(
                &atlas,
                [
                    window.inner_size().width as f32,
                    window.inner_size().height as f32,
                ],
            );
            if let Some(path) = &self.options.socket {
                terminal.listen(path)?;
            }
            let gpu = Gpu::new(window.clone(), &atlas)?;
            // Probe the matching helper away from the frame thread.
            std::thread::spawn(terminal_gfx::pty::openagents_terminal);
            window.set_ime_allowed(true);
            Ok::<_, String>(State {
                window,
                gpu,
                atlas,
                terminal,
                stress,
            })
        })();
        match result {
            Ok(state) => self.state = Some(state),
            Err(error) => self.fail(event_loop, error),
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };
        if state.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                state.terminal.shutdown();
                self.write_latency();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.gpu.resize(size.width, size.height);
                state
                    .terminal
                    .fit(&state.atlas, [size.width as f32, size.height as f32]);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let mut atlas = Atlas::new(16.0 * scale_factor as f32);
                if let Err(error) = atlas.reserve_glyphs(terminal_gfx::GLYPH_ROWS) {
                    self.fail(event_loop, error);
                    return;
                }
                state.atlas = atlas;
                state.gpu.rebuild_atlas(&state.atlas);
                // The cell size changed with the backing scale; refit the panes.
                let size = state.window.inner_size();
                state
                    .terminal
                    .fit(&state.atlas, [size.width as f32, size.height as f32]);
            }
            WindowEvent::Focused(focused) => state.terminal.focused = focused,
            WindowEvent::Occluded(hidden) => self.hidden = hidden && !self.options.background,
            WindowEvent::ModifiersChanged(mods) => state.terminal.modifiers(mods.state()),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed
                        && code == KeyCode::KeyQ
                        && state.terminal.mods.super_key()
                    {
                        state.terminal.shutdown();
                        event_loop.exit();
                        return;
                    }
                    let plain = event.key_without_modifiers().to_text().map(str::to_owned);
                    state.terminal.key(&KeyIn {
                        code,
                        logical: event.logical_key,
                        text: event.text.map(|text| text.to_string()),
                        plain,
                        pressed: event.state == ElementState::Pressed,
                        repeat: event.repeat,
                        synthetic: is_synthetic,
                    });
                    if state.terminal.core.paper.quit {
                        state.terminal.shutdown();
                        self.write_latency();
                        event_loop.exit();
                        return;
                    }
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => state.terminal.paste(&text),
            WindowEvent::CursorMoved { position, .. } => {
                self.point = [position.x as f32, position.y as f32];
                state.terminal.pointer(self.point);
            }
            WindowEvent::MouseInput {
                state: pressed,
                button,
                ..
            } => {
                let button = match button {
                    MouseButton::Left => Some(Button::Left),
                    MouseButton::Middle => Some(Button::Middle),
                    MouseButton::Right => Some(Button::Right),
                    _ => None,
                };
                if let Some(button) = button {
                    state
                        .terminal
                        .button(button, pressed == ElementState::Pressed, self.point);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines,
                    MouseScrollDelta::PixelDelta(position) => {
                        position.y as f32 / state.terminal.cell[1]
                    }
                };
                state.terminal.wheel(self.point, lines);
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    self.fail(event_loop, error);
                }
            }
            _ => {}
        }
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            if let Some(driver) = &mut state.stress {
                let actions = driver.step(true);
                if driver.recording() {
                    state.terminal.stats.record = true;
                }
                for action in actions {
                    match action {
                        terminal_gfx::stress::Action::Open => {
                            let programs = match driver.programs() {
                                Ok(programs) => programs,
                                Err(error) => {
                                    self.fail(event_loop, error);
                                    return;
                                }
                            };
                            state.terminal.shutdown();
                            state.terminal = Overlay::with(
                                driver.root(),
                                terminal_gfx::pty::user_shell(),
                                programs[0].clone(),
                            );
                            state.terminal.studio_transport = std::sync::Arc::new(
                                terminal_studio::Native::new(Some(driver.root().to_path_buf())),
                            );
                            state.terminal.mount = Mount::Window;
                            let size = state.window.inner_size();
                            state
                                .terminal
                                .fit(&state.atlas, [size.width as f32, size.height as f32]);
                            state.terminal.open_grid(&programs);
                        }
                        terminal_gfx::stress::Action::Key(character) => {
                            let mut key = KeyIn {
                                code: KeyCode::KeyA,
                                logical: winit::keyboard::Key::Character(
                                    character.to_string().into(),
                                ),
                                text: Some(character.to_string()),
                                plain: None,
                                pressed: true,
                                repeat: false,
                                synthetic: false,
                            };
                            if character == '\r' {
                                key.code = KeyCode::Enter;
                                key.logical =
                                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter);
                                key.text = None;
                            }
                            state.terminal.key(&key);
                            key.pressed = false;
                            state.terminal.key(&key);
                        }
                        terminal_gfx::stress::Action::Finish => {
                            let report = driver.report(
                                &state.terminal.stats.frames,
                                &state.terminal.stats.latencies,
                            );
                            let result = serde_json::to_vec_pretty(&report)
                                .map_err(|error| error.to_string())
                                .and_then(|json| {
                                    std::fs::write(&driver.plan.out, json)
                                        .map_err(|error| error.to_string())
                                });
                            state.terminal.shutdown();
                            driver.clean();
                            if let Err(error) = result {
                                self.error = Some(error);
                            }
                            event_loop.exit();
                            return;
                        }
                        _ => {}
                    }
                }
            }
            state.terminal.tick();
            if !self.hidden {
                state.window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(8),
        ));
    }
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    screen: wgpu::Buffer,
    atlas: wgpu::Texture,
    atlas_revision: u64,
    vertices: wgpu::Buffer,
    capacity: u64,
    depth: wgpu::Texture,
}
impl Gpu {
    fn new(window: Arc<Window>, atlas: &Atlas) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let size = window.inner_size();
        let surface = instance
            .create_surface(window)
            .map_err(|error| error.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|error| error.to_string())?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| error.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .ok_or("no sRGB window format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let (pipeline, _, group, screen, texture) =
            verse_gfx::ui_pipeline::ui_pipeline_with_texture(&device, &queue, format, 1, atlas);
        let capacity = 1024 * 1024;
        let vertices = Self::buffer(&device, capacity);
        let depth = Self::depth(&device, config.width, config.height);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            group,
            screen,
            atlas: texture,
            atlas_revision: atlas.revision(),
            vertices,
            capacity,
            depth,
        })
    }
    fn buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal vertices"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    fn depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth = Self::depth(&self.device, width, height);
    }
    fn rebuild_atlas(&mut self, atlas: &Atlas) {
        let (pipeline, _, group, screen, texture) =
            verse_gfx::ui_pipeline::ui_pipeline_with_texture(
                &self.device,
                &self.queue,
                self.config.format,
                1,
                atlas,
            );
        self.pipeline = pipeline;
        self.group = group;
        self.screen = screen;
        self.atlas = texture;
        self.atlas_revision = atlas.revision();
    }
    fn draw(&mut self, batch: &UiBatch, atlas: &Atlas) -> Result<bool, String> {
        if self.atlas_revision != atlas.revision() {
            if !verse_gfx::ui_pipeline::write_atlas(&self.queue, &self.atlas, atlas) {
                self.rebuild_atlas(atlas);
            }
            self.atlas_revision = atlas.revision();
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            _ => return Err("the window surface was lost".into()),
        };
        let bytes = bytemuck::cast_slice::<UiVertex, u8>(&batch.vertices);
        if bytes.len() as u64 > 64 * 1024 * 1024 {
            return Err("the terminal drawing exceeds its vertex budget".into());
        }
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.vertices = Self::buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(
            &self.screen,
            0,
            bytemuck::cast_slice(&[
                self.config.width as f32,
                self.config.height as f32,
                0.0,
                0.0,
            ]),
        );
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.vertices, 0, bytes);
        }
        let view = frame.texture.create_view(&Default::default());
        let depth = self.depth.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.005,
                            g: 0.005,
                            b: 0.005,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !batch.vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.group, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..bytes.len() as u64));
                pass.draw(0..batch.vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(true)
    }
}
