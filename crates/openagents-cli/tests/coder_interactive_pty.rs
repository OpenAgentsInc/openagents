//! What the Coder TUI does when a person is actually typing at it.
//!
//! Every other test in this crate calls a function. This one starts the real
//! `Coder` binary as a child process on a real pseudo-terminal, sends it
//! bytes the way a keyboard does, parses what comes back with a terminal
//! emulator, and asserts on the cells.
//!
//! It exists because of a specific failure, recorded in the openagents.com
//! repository as `docs/2026-08-26-rust-cli-port-parity-failure-postmortem.md`:
//! a TUI with **no input widget at all** was reported as parity and closed
//! seven issues, because every verification ran in a non-TTY subshell and took
//! the `Non-interactive terminal detected` branch in `interactive.rs` without
//! ever rendering a frame. A harness that cannot see the composer cannot
//! notice its absence. This one looks at the composer.
//!
//! Each assertion below is chosen so that it would have gone red against that
//! build: a bare invocation opening a session at all, the composer box, the
//! echo of typed characters, the caret column, backspace, the submitted turn
//! appearing in the transcript, a `/` line reaching the session's own dispatch
//! instead of the model, the frame surviving a resize, the status bar, and the
//! terminal being handed back unraw on the way out.
//!
//! ## No arguments means the session
//!
//! The shipped binary answers two surfaces: a bare invocation opens the
//! interactive TUI, and a subcommand (`issue list` and peers) dispatches to
//! the CLI command set. Every session below is started with **no arguments**,
//! and [`the_bare_binary_opens_an_interactive_session`] asserts that contract
//! by name — a change that makes a bare invocation print help instead of
//! opening a session is exactly the regression this file exists to catch.
//!
//! ## No provider, no network
//!
//! `run_tui` opens a session only when a stored credential validates against
//! `GET {origin}/api/v1/models`. The harness therefore points
//! `OPENAGENTS_API_URL` at a stub HTTP server it starts on loopback, which
//! answers that route and `GET {origin}/api/v1/credit` — the one the status
//! bar reads its balance from — with `200`, and refuses everything else. So
//! the session is real, the frame is real, no provider credential is spent,
//! and nothing leaves the machine. A turn is never completed on purpose: what
//! is asserted is that Enter *starts* one, which is the part the composer owns.

#[cfg(not(unix))]
#[test]
fn skipped_without_a_unix_pseudo_terminal() {
    // A named skip, not a silent pass. Windows has no `openpty`, and the
    // crate itself only recently compiles there (3b16e0679b).
    eprintln!(
        "SKIP interactive_pty: the Coder PTY harness needs a unix \
         pseudo-terminal. The TUI is unobserved on this platform."
    );
}

#[cfg(unix)]
mod unix_pty {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::os::unix::io::RawFd;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

    /// The frame the harness starts with. Wide enough that nothing under test
    /// wraps by accident, tall enough that the transcript has somewhere to go.
    const COLS: u16 = 100;
    const ROWS: u16 = 30;

    /// How long the first frame may take. It covers process start, the git
    /// probe, the ACP scan, and the credential check against the stub.
    const FIRST_FRAME: Duration = Duration::from_secs(30);
    /// How long any later frame may take. The session's poll interval is 50ms.
    const REDRAW: Duration = Duration::from_secs(15);

    // ─────────────────────────────────────────────────── the stub deployment

    /// A loopback HTTP server that answers exactly two routes.
    ///
    /// `GET …/api/v1/models` is what `validate_token` calls to decide whether
    /// a session may open. `GET …/api/v1/credit` is what the status bar reads
    /// its balance from, and it answers the exact body
    /// `OpenAgentsWeb.CreditController` writes — so the figure on the bottom
    /// row is one this binary parsed out of an HTTP response rather than one a
    /// test handed it. Everything else is refused with `503` and a body that
    /// says who refused it, so a request this harness did not intend is
    /// legible rather than silently satisfied.
    struct Stub {
        origin: String,
    }

    /// A $20 account that has spent $1.60 on priced lanes, every call priced.
    const STUB_CREDIT: &str = concat!(
        r#"{"credit":{"allowance_microusd":20000000,"spent_microusd":1600000,"#,
        r#""remaining_microusd":18400000,"unpriced_calls":0,"complete":true}}"#
    );

    /// The same account after three turns on a lane this deployment has no
    /// rates for: nothing was drawn down, and the server says its own spend
    /// figure is incomplete.
    const STUB_CREDIT_UNPRICED: &str = concat!(
        r#"{"credit":{"allowance_microusd":20000000,"spent_microusd":0,"#,
        r#""remaining_microusd":20000000,"unpriced_calls":3,"complete":false}}"#
    );

    impl Stub {
        fn start() -> Self {
            Self::start_with_credit(STUB_CREDIT)
        }

        fn start_with_credit(credit: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback stub");
            let port = listener.local_addr().expect("stub address").port();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") {
                        match stream.read(&mut byte) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => request.push(byte[0]),
                        }
                    }
                    let head = String::from_utf8_lossy(&request).to_string();
                    let served = if head.starts_with("GET ") && head.contains("/api/v1/models") {
                        // The shape `served_models` reads, with one available
                        // model so the Flash lane resolves and the turn gets
                        // as far as the thread call this stub refuses. An
                        // empty list would refuse at the lane instead, and the
                        // test below would stop proving the turn reaches the
                        // deployment at all.
                        Some(
                            r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true}]}"#,
                        )
                    } else if head.starts_with("GET ") && head.contains("/api/v1/credit") {
                        Some(credit)
                    } else {
                        None
                    };
                    let response = if let Some(body) = served {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    } else {
                        let body =
                            "the Coder PTY harness stub serves /api/v1/models and /credit";
                        format!(
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
            });
            Self {
                origin: format!("http://127.0.0.1:{port}"),
            }
        }
    }

    // ──────────────────────────────────────────────────────────── the screen

    /// One rendered frame, read out of the emulator.
    struct Frame {
        rows: Vec<String>,
        cursor: (u16, u16),
        cols: u16,
    }

    impl Frame {
        /// The composer: the bordered box the session types into.
        ///
        /// Found by its borders rather than by a hard-coded row, so a layout
        /// change does not fake a pass — but a build with no box at all finds
        /// nothing, which is the case this whole file exists for.
        fn composer(&self) -> Option<Composer> {
            let width = self.cols.saturating_sub(2) as usize;
            let top_border = format!("┌{}┐", "─".repeat(width));
            let bottom_border = format!("└{}┘", "─".repeat(width));
            let top = self
                .rows
                .iter()
                .position(|row| row.trim_end() == top_border)?;
            let bottom = self
                .rows
                .iter()
                .skip(top + 1)
                .position(|row| row.trim_end() == bottom_border)?
                + top
                + 1;
            Some(Composer {
                top,
                bottom,
                lines: self.rows[top + 1..bottom]
                    .iter()
                    .map(|row| inside(row))
                    .collect(),
            })
        }

        /// The rows above the composer, joined and whitespace-collapsed, so a
        /// `contains` check does not care where the renderer wrapped or padded.
        fn transcript(&self) -> String {
            let end = self.composer().map(|c| c.top).unwrap_or(self.rows.len());
            collapse(&self.rows[..end])
        }

        /// The bottom line, which since 7ad52a0c8d carries the usage.
        fn status_bar(&self) -> String {
            self.rows.last().cloned().unwrap_or_default()
        }

        /// Every row of the frame, for a failure message. A red run in this
        /// file has to show what the terminal actually held, or the next
        /// reader has to reproduce it by hand.
        fn dump(&self) -> String {
            let mut out = String::new();
            for (index, row) in self.rows.iter().enumerate() {
                out.push_str(&format!("{index:>3} |{}|\n", row.trim_end()));
            }
            out.push_str(&format!(
                "cursor at row {} col {}\n",
                self.cursor.0, self.cursor.1
            ));
            out
        }
    }

    /// One row of the composer box without its `│` side borders, trailing
    /// space removed. What is left is the `" > "` gutter and what was typed.
    fn inside(row: &str) -> String {
        let cells: Vec<char> = row.chars().collect();
        let body: String = match cells.len() {
            0 | 1 => String::new(),
            len => cells[1..len - 1].iter().collect(),
        };
        body.trim_end().to_string()
    }

    fn collapse(rows: &[String]) -> String {
        rows.join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    struct Composer {
        /// Row index of the top border.
        top: usize,
        /// Row index of the bottom border.
        bottom: usize,
        /// The rows between the borders, trailing space removed.
        lines: Vec<String>,
    }

    impl Composer {
        /// The first line inside the box, which carries the `" > "` gutter.
        fn first(&self) -> &str {
            self.lines.first().map(String::as_str).unwrap_or("")
        }

        /// Whether the first line opens with the `" > "` prompt gutter.
        ///
        /// An empty composer's line is `" >"` once its trailing space is
        /// gone, so both shapes count — and neither is produced by a box with
        /// no prompt in it.
        fn has_gutter(&self) -> bool {
            self.first() == " >" || self.first().starts_with(" > ")
        }
    }

    struct Emulator {
        parser: vt100::Parser,
        raw: Vec<u8>,
    }

    // ─────────────────────────────────────────────────────────── the session

    /// A live `Coder` on a pseudo-terminal.
    struct Tui {
        master: Box<dyn MasterPty + Send>,
        child: Box<dyn Child + Send + Sync>,
        writer: Box<dyn Write + Send>,
        emulator: Arc<Mutex<Emulator>>,
        /// The throwaway `HOME` the child wrote its history into; removed on
        /// drop.
        home: PathBuf,
        _stub: Stub,
    }

    impl Tui {
        /// Start the binary **with no arguments**, which is the invocation
        /// that opens a session. A subcommand would dispatch to the CLI
        /// command set instead, and this harness has nothing to say about
        /// that surface.
        fn start() -> Self {
            Self::start_full(ROWS, COLS, STUB_CREDIT, false)
        }

        /// Start the bare binary with an exhausted pipe as standard input,
        /// which is how `curl … | sh` hands control to Coder after install.
        fn start_with_piped_stdin() -> Self {
            Self::start_full(ROWS, COLS, STUB_CREDIT, true)
        }

        /// A session whose deployment answers `GET /api/v1/credit` with one
        /// named body, so a test can drive the status bar's states from the
        /// wire rather than from the renderer.
        fn start_with_credit(credit: &'static str) -> Self {
            Self::start_full(ROWS, COLS, credit, false)
        }

        fn start_full(
            rows: u16,
            cols: u16,
            credit: &'static str,
            piped_stdin: bool,
        ) -> Self {
            let stub = Stub::start_with_credit(credit);
            let home = scratch_dir();
            let workdir = home.join("workdir");
            std::fs::create_dir_all(&workdir).expect("create the scratch working directory");

            let pty = native_pty_system();
            let pair = pty
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .expect("open a pseudo-terminal");

            let executable = env!("CARGO_BIN_EXE_openagents");
            let mut command = if piped_stdin {
                let escaped = executable.replace('\'', "'\\''");
                let mut command = CommandBuilder::new("/bin/sh");
                command.arg("-c");
                command.arg(format!("printf '' | exec '{escaped}'"));
                command
            } else {
                // No arguments: a bare invocation must remain the Coder front
                // door rather than dispatching to another CLI surface.
                CommandBuilder::new(executable)
            };
            command.cwd(&workdir);
            command.env("HOME", &home);
            command.env("TERM", "xterm-256color");
            // A stub deployment, not a provider. The token is spent against
            // the loopback server above and nowhere else.
            command.env("OPENAGENTS_API_URL", &stub.origin);
            command.env("OPENAGENTS_BASE_URL", format!("{}/api/v1", stub.origin));
            command.env("OPENAGENTS_API_KEY", "pty-harness-not-a-real-credential");
            // A directory that does not exist, so the ACP scan returns an
            // empty list at once instead of probing this machine's agents.
            command.env("ACP_REGISTRY", home.join("no-acp-registry"));
            // Anything on the host that would redirect the session elsewhere.
            command.env_remove("OPENAGENTS_PROFILE");
            command.env_remove("OPENAGENTS_API_BASE");
            command.env_remove("NO_COLOR");

            let child = pair.slave.spawn_command(command).expect("spawn Coder");
            // The slave must close here or the reader below never sees EOF.
            drop(pair.slave);

            let emulator = Arc::new(Mutex::new(Emulator {
                parser: vt100::Parser::new(rows, cols, 0),
                raw: Vec::new(),
            }));
            let mut reader = pair
                .master
                .try_clone_reader()
                .expect("clone the pty reader");
            let sink = Arc::clone(&emulator);
            std::thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => {
                            let mut emulator = sink.lock().expect("emulator lock");
                            emulator.parser.process(&buffer[..read]);
                            emulator.raw.extend_from_slice(&buffer[..read]);
                        }
                    }
                }
            });

            let writer = pair.master.take_writer().expect("take the pty writer");
            Self {
                master: pair.master,
                child,
                writer,
                emulator,
                home,
                _stub: stub,
            }
        }

        fn send(&mut self, bytes: &[u8]) {
            self.writer.write_all(bytes).expect("write to the pty");
            self.writer.flush().expect("flush the pty");
        }

        fn type_text(&mut self, text: &str) {
            // One byte at a time, the way a keyboard delivers them, so the
            // session's key handling is exercised per character rather than
            // as one paste.
            for byte in text.as_bytes() {
                self.send(&[*byte]);
                std::thread::sleep(Duration::from_millis(4));
            }
        }

        /// Send one bracketed paste transaction, as iTerm2 and other modern
        /// terminals do for clipboard input. The embedded newlines must stay
        /// in the composer until the user presses Enter afterward.
        fn paste_text(&mut self, text: &str) {
            let mut bytes = Vec::with_capacity(text.len() + 12);
            bytes.extend_from_slice(b"\x1b[200~");
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            self.send(&bytes);
        }

        fn frame(&self) -> Frame {
            let emulator = self.emulator.lock().expect("emulator lock");
            let screen = emulator.parser.screen();
            let (_, cols) = screen.size();
            Frame {
                rows: screen.rows(0, cols).collect(),
                cursor: screen.cursor_position(),
                cols,
            }
        }

        fn raw_output(&self) -> Vec<u8> {
            self.emulator.lock().expect("emulator lock").raw.clone()
        }

        /// Poll until `ready` accepts a frame, or fail with the last one.
        fn wait_for(&self, what: &str, within: Duration, ready: impl Fn(&Frame) -> bool) -> Frame {
            let deadline = Instant::now() + within;
            let mut last = self.frame();
            loop {
                if ready(&last) {
                    return last;
                }
                if Instant::now() >= deadline {
                    panic!(
                        "waited {within:?} for {what} and it never happened.\n\
                         The terminal held:\n{}",
                        last.dump()
                    );
                }
                std::thread::sleep(Duration::from_millis(25));
                last = self.frame();
            }
        }

        /// The frame with a composer on it, or a failure that says the TUI
        /// rendered no input area — the postmortem's exact defect.
        fn wait_for_composer(&self) -> Frame {
            self.wait_for(
                "the composer to render an input box",
                FIRST_FRAME,
                |frame| frame.composer().is_some(),
            )
        }

        fn resize(&mut self, rows: u16, cols: u16) {
            self.master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .expect("resize the pty");
            self.emulator
                .lock()
                .expect("emulator lock")
                .parser
                .set_size(rows, cols);
        }

        fn master_fd(&self) -> RawFd {
            self.master
                .as_raw_fd()
                .expect("the pty master exposes a file descriptor")
        }

        /// Ctrl+C, then wait for the process to go.
        fn quit(&mut self) -> portable_pty::ExitStatus {
            self.send(&[0x03]);
            let deadline = Instant::now() + REDRAW;
            loop {
                if let Ok(Some(status)) = self.child.try_wait() {
                    return status;
                }
                if Instant::now() >= deadline {
                    panic!(
                        "Coder did not exit within {REDRAW:?} of Ctrl+C.\n\
                         The terminal held:\n{}",
                        self.frame().dump()
                    );
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }

    impl Drop for Tui {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }

    fn scratch_dir() -> PathBuf {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let unique = format!(
            "Coder-pty-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("create the scratch home");
        path
    }

    /// The column the caret sits at for `typed` characters on the first line.
    ///
    /// `CoderUi::render` puts the caret at `input_area.x + 1 + 3 + caret_col`:
    /// the box's left border and the `" > "` gutter. The box starts at column
    /// zero, so an empty composer's caret is at column four.
    fn caret_column(typed: usize) -> u16 {
        4 + typed as u16
    }

    // ────────────────────────────────────────────────────────────── the tests

    /// The bare binary opens a session rather than printing help.
    ///
    /// One binary now serves both the TUI and the CLI command set, so "no
    /// arguments means the session" is a contract that can be broken by a
    /// change to argument dispatch. The evidence that a session opened is a
    /// rendered frame with an input box on it, which is also the evidence
    /// that the TUI is not the postmortem's mock.
    #[test]
    fn the_bare_binary_opens_an_interactive_session() {
        let tui = Tui::start();
        let frame = tui.wait_for_composer();
        assert!(
            frame.composer().is_some(),
            "a bare `Coder` should open a session with an input box.\n{}",
            frame.dump()
        );
        assert!(
            !frame
                .transcript()
                .contains("Non-interactive terminal detected"),
            "the session took the headless branch on a real pty.\n{}",
            frame.dump()
        );
        assert!(
            !frame.transcript().contains("Usage: Coder"),
            "a bare invocation printed help instead of opening a session.\n{}",
            frame.dump()
        );
        assert!(
            frame
                .transcript()
                .contains(&format!("Coder v{}", openagents_cli::VERSION)),
            "the session heading should label the current Coder version.\n{}",
            frame.dump()
        );
        assert!(
            frame
                .transcript()
                .contains("Working directory"),
            "the startup summary should name the working directory.\n{}",
            frame.dump()
        );
        assert!(
            !frame.transcript().contains("Coder Flash · https://"),
            "the startup summary should not repeat the lane from the footer.\n{}",
            frame.dump()
        );
    }

    /// The installer itself runs from a pipe, then starts Coder from the same
    /// process. The binary must read keys from the controlling terminal rather
    /// than trying to register that exhausted pipe as its input source.
    #[test]
    fn an_installer_pipe_still_accepts_terminal_input() {
        let mut tui = Tui::start_with_piped_stdin();
        let _ = tui.wait_for_composer();

        tui.type_text("pipe-ok");
        let frame = tui.wait_for("piped Coder to accept terminal input", REDRAW, |frame| {
            frame
                .composer()
                .is_some_and(|composer| composer.first().contains("pipe-ok"))
        });
        assert!(
            !frame
                .transcript()
                .contains("Failed to initialize input reader"),
            "Coder lost its terminal input after an installer pipe.\n{}",
            frame.dump()
        );

        let status = tui.quit();
        assert!(status.success(), "piped Coder did not exit cleanly: {status:?}");
    }

    /// The one the postmortem is about: is there anywhere to type.
    #[test]
    fn the_session_renders_a_composer_with_a_caret_in_it() {
        let tui = Tui::start();
        let frame = tui.wait_for_composer();
        let composer = frame.composer().expect("just waited for it");

        assert!(
            composer.has_gutter(),
            "the composer's first line should carry the ` > ` gutter, and held {:?}.\n{}",
            composer.first(),
            frame.dump()
        );
        assert_eq!(
            frame.cursor,
            (composer.top as u16 + 1, caret_column(0)),
            "the caret should sit on the composer's first line, just after the gutter.\n{}",
            frame.dump()
        );
    }

    /// Typing reaches the composer, the caret follows it, and backspace
    /// removes what was typed.
    #[test]
    fn typed_characters_echo_into_the_composer_and_backspace_removes_them() {
        let mut tui = Tui::start();
        let frame = tui.wait_for_composer();
        let composer_top = frame.composer().expect("composer").top as u16;

        tui.type_text("hello");
        let frame = tui.wait_for("`hello` to echo into the composer", REDRAW, |frame| {
            frame
                .composer()
                .is_some_and(|composer| composer.first() == " > hello")
        });
        assert_eq!(
            frame.cursor,
            (composer_top + 1, caret_column(5)),
            "the caret should have advanced five columns with the five characters.\n{}",
            frame.dump()
        );

        tui.send(&[0x7f]);
        tui.send(&[0x7f]);
        let frame = tui.wait_for("backspace to leave `hel`", REDRAW, |frame| {
            frame
                .composer()
                .is_some_and(|composer| composer.first() == " > hel")
        });
        assert_eq!(
            frame.cursor,
            (composer_top + 1, caret_column(3)),
            "the caret should have come back two columns with the two deletions.\n{}",
            frame.dump()
        );
    }

    /// A clipboard paste is one editing operation, not a run of Enter keys.
    /// This catches the regression where a bulleted list submitted its first
    /// line and left later lines in the composer.
    #[test]
    fn multiline_paste_stays_in_the_composer_until_enter() {
        let mut tui = Tui::start();
        tui.wait_for_composer();

        tui.paste_text("shopping list\r\n- coffee\r\n- filters");
        let frame = tui.wait_for(
            "the complete pasted list in the composer",
            REDRAW,
            |frame| {
                frame.composer().is_some_and(|composer| {
                    composer.lines == [" > shopping list", "   - coffee", "   - filters"]
                })
            },
        );
        assert!(
            !frame.transcript().contains("shopping list"),
            "paste should not submit before Enter.\n{}",
            frame.dump()
        );

        tui.send(b"\r");
        tui.wait_for(
            "the complete pasted list to submit after Enter",
            REDRAW,
            |frame| {
                frame
                    .transcript()
                    .contains("> shopping list - coffee - filters")
            },
        );
    }

    /// Enter hands the line to the session: it leaves the composer and the
    /// transcript shows the turn starting.
    #[test]
    fn enter_submits_the_line_and_the_transcript_shows_the_turn_beginning() {
        let mut tui = Tui::start();
        tui.wait_for_composer();

        tui.type_text("ping from the pty harness");
        tui.wait_for("the prompt to reach the composer", REDRAW, |frame| {
            frame
                .composer()
                .is_some_and(|composer| composer.first().contains("ping from the pty harness"))
        });

        tui.send(b"\r");
        let frame = tui.wait_for(
            "the submitted line to appear in the transcript",
            REDRAW,
            |frame| frame.transcript().contains("> ping from the pty harness"),
        );

        let composer = frame.composer().expect("the composer survives a submit");
        assert_eq!(
            composer.first().trim_end(),
            " >",
            "the composer should be empty after a submit, and held {:?}.\n{}",
            composer.first(),
            frame.dump()
        );
        assert_eq!(
            frame.cursor,
            (composer.top as u16 + 1, caret_column(0)),
            "the caret should be back at the start of an empty composer.\n{}",
            frame.dump()
        );

        // The echo alone would be satisfied by a composer that only paints.
        // This is the turn actually reaching the runtime: the session opened a
        // thread against the stub deployment, and the stub's own refusal — a
        // string only this harness produces — came back into the transcript.
        tui.wait_for(
            "the turn to reach the deployment and its answer to land in the transcript",
            REDRAW,
            |frame| {
                frame
                    .transcript()
                    .contains("the Coder PTY harness stub serves /api/v1/models and /credit")
            },
        );
    }

    /// A `/` line reaches the session's own dispatch rather than being sent to
    /// a model or dropped. Both halves are asserted: a name that exists runs,
    /// and a name that does not is refused by name.
    #[test]
    fn a_slash_command_is_recognised_and_an_unknown_one_is_refused_by_name() {
        let mut tui = Tui::start();
        tui.wait_for_composer();

        tui.type_text("/help");
        tui.send(b"\r");
        // `/help` prints the table in `commands::COMMANDS` and the key list
        // beside it. Only that dispatch produces these strings.
        let frame = tui.wait_for("`/help` to print the command table", REDRAW, |frame| {
            let transcript = frame.transcript();
            transcript.contains("clear the transcript") && transcript.contains("Alt+Enter")
        });
        assert!(
            frame.transcript().contains("Commands"),
            "the `/help` output should be headed `Commands`.\n{}",
            frame.dump()
        );

        tui.type_text("/nosuchcommand");
        tui.send(b"\r");
        tui.wait_for("an unknown `/` name to be refused", REDRAW, |frame| {
            let transcript = frame.transcript();
            transcript.contains("There is no") && transcript.contains("nosuchcommand")
        });
    }

    /// A resize redraws the frame at the new size instead of leaving the old
    /// one behind it.
    #[test]
    fn a_resize_redraws_the_frame_at_the_new_width() {
        let mut tui = Tui::start();
        tui.wait_for_composer();

        tui.type_text("resize marker alpha");
        tui.send(b"\r");
        tui.wait_for("the marker to reach the transcript", REDRAW, |frame| {
            frame.transcript().contains("resize marker alpha")
        });

        for (rows, cols) in [(24u16, 70u16), (40, 120), (30, 100)] {
            tui.resize(rows, cols);
            let frame = tui.wait_for(
                &format!("the frame to redraw at {cols}x{rows}"),
                REDRAW,
                |frame| {
                    frame.cols == cols
                        && frame
                            .composer()
                            .is_some_and(|composer| composer.has_gutter())
                },
            );

            assert!(
                frame
                    .rows
                    .iter()
                    .all(|row| row.chars().count() <= cols as usize),
                "no row may run past the new width of {cols}.\n{}",
                frame.dump()
            );
            let borders = frame
                .rows
                .iter()
                .filter(|row| row.trim_end().starts_with('┌'))
                .count();
            assert_eq!(
                borders,
                1,
                "exactly one composer top border should survive a resize to {cols}x{rows}.\n{}",
                frame.dump()
            );
            assert!(
                frame.transcript().contains("resize marker alpha"),
                "the transcript should still hold the marker after a resize.\n{}",
                frame.dump()
            );
            assert!(
                frame.status_bar().contains("$18.40 left")
                    && frame.status_bar().trim_end().ends_with("Coder Flash"),
                "the status bar should still be on the bottom row after a resize.\n{}",
                frame.dump()
            );
        }
    }

    /// The bottom row carries remaining credit on the left and the active lane
    /// on the right. Account and endpoint details are available through
    /// `/info`.
    #[test]
    fn the_bottom_row_carries_credit_and_the_active_lane() {
        let tui = Tui::start();
        let frame = tui.wait_for(
            "the status bar to report the balance and lane",
            FIRST_FRAME,
            |frame| {
                let status = frame.status_bar();
                status.contains("$18.40") && status.contains("Coder Flash")
            },
        );

        let status = frame.status_bar();
        assert!(
            !status.contains("unverified") && !status.contains("127.0.0.1"),
            "account and endpoint details belong to /info, and the footer held {:?}.\n{}",
            status,
            frame.dump()
        );
        assert!(
            !status.contains("tokens"),
            "the token count is still on the row it was moved off, and held {:?}.\n{}",
            status,
            frame.dump()
        );

        let balance = status.find("$18.40 left").unwrap_or_else(|| {
            panic!(
                "the balance should appear at the left of the bottom row, \
                 and the row held {status:?}.\n{}",
                frame.dump()
            )
        });
        assert_eq!(balance, 0, "the balance should start at the left edge");
        assert!(
            status.trim_end().ends_with("Coder Flash"),
            "the lane should end at the right edge, and the row held {status:?}.\n{}",
            frame.dump()
        );

        let composer = frame.composer().expect("a composer above the status bar");
        assert!(
            composer.bottom < frame.rows.len() - 1,
            "the status bar should sit below the composer, not inside it.\n{}",
            frame.dump()
        );
    }

    /// The balance on the bottom row is the deployment's, not this session's.
    ///
    /// The stub answers `GET /api/v1/credit` with the body the server writes
    /// for a $20 account that has spent $1.60, so what this asserts is a figure
    /// that travelled over HTTP, through `serde`, into the frame — not one a
    /// test handed the renderer. Nothing in the session has spent a token, so a
    /// build that derived the balance from its own usage counter would show the
    /// whole $20.00 here and go red.
    #[test]
    fn the_status_bar_carries_the_balance_the_server_reported() {
        let tui = Tui::start();
        let frame = tui.wait_for("the status bar to carry a balance", FIRST_FRAME, |frame| {
            frame.status_bar().contains("$18.40")
        });

        let status = frame.status_bar();
        assert!(
            status.contains("$18.40 left"),
            "the bottom row should carry the remaining balance, and held {:?}.\n{}",
            status,
            frame.dump()
        );
        assert!(
            !status.contains("unverified") && !status.contains("127.0.0.1"),
            "the footer should not show account or endpoint details: {:?}.\n{}",
            status,
            frame.dump()
        );
        assert!(
            !status.contains("tokens"),
            "the token counts moved to `/info` and must not be back on the \
             bottom row: {:?}.\n{}",
            status,
            frame.dump()
        );
        // $20.00 is the allowance, and printing it would be reporting a grant
        // as a balance.
        assert!(
            !status.contains("$20.00"),
            "the bottom row printed the allowance rather than the remainder: {:?}.\n{}",
            status,
            frame.dump()
        );
    }

    /// An unpriced lane still shows only the remaining credit.
    #[test]
    fn an_unpriced_lane_shows_only_the_credit_figure() {
        let tui = Tui::start_with_credit(STUB_CREDIT_UNPRICED);
        let frame = tui.wait_for(
            "the status bar to report the remaining credit",
            FIRST_FRAME,
            |frame| frame.status_bar().contains("$20.00 left"),
        );

        let status = frame.status_bar();
        assert!(
            status.contains("$20.00 left") && !status.contains("unpriced"),
            "the bottom row should carry only the remaining credit, and held {:?}.\n{}",
            status,
            frame.dump()
        );
    }

    /// Leaving hands the terminal back: raw mode off, alternate screen left.
    ///
    /// The termios check is read from the pty master after the child is gone,
    /// so it observes the state a person's shell would inherit. A build that
    /// exits without `disable_raw_mode` leaves `ECHO` and `ICANON` clear here,
    /// which is the shape of a terminal a user has to `reset`.
    #[test]
    fn leaving_restores_the_terminal() {
        let mut tui = Tui::start();
        tui.wait_for_composer();

        let while_running = termios_flags(tui.master_fd());
        assert_eq!(
            while_running & (libc::ECHO | libc::ICANON),
            0,
            "the running TUI should hold the terminal in raw mode; \
             if it does not, this test cannot prove the restore"
        );

        let status = tui.quit();
        assert!(
            status.success(),
            "Ctrl+C should leave cleanly, and exited with {status:?}"
        );

        let restored = termios_flags(tui.master_fd());
        assert_ne!(
            restored & libc::ECHO,
            0,
            "ECHO should be back on after the session exits (raw mode leaked)"
        );
        assert_ne!(
            restored & libc::ICANON,
            0,
            "ICANON should be back on after the session exits (raw mode leaked)"
        );

        let output = tui.raw_output();
        assert!(
            contains(&output, b"\x1b[?1049h"),
            "the session should have entered the alternate screen"
        );
        assert!(
            contains(&output, b"\x1b[?1049l"),
            "the session should have left the alternate screen on the way out"
        );
        assert!(
            contains(&output, b"\x1b[?2004h") && contains(&output, b"\x1b[?2004l"),
            "the session should enable bracketed paste while it owns the terminal and disable it on exit"
        );
        assert!(
            contains(&output, b"\x1b[?1000h") && contains(&output, b"\x1b[?1000l"),
            "the session should capture ordinary mouse gestures for trackpad scrolling and release them on exit"
        );
    }

    fn termios_flags(fd: RawFd) -> libc::tcflag_t {
        // SAFETY: `fd` is the live pty master owned by the `Tui` it was read
        // from, and `settings` is written only by `tcgetattr`.
        unsafe {
            let mut settings: libc::termios = std::mem::zeroed();
            assert_eq!(
                libc::tcgetattr(fd, &mut settings),
                0,
                "could not read the pty's terminal settings"
            );
            settings.c_lflag
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}
