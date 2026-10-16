//! Desktop geometry, selection, and text rendering for conversation files.
use super::*;
use openagents_chat_app::workspace_tabs::{self as files, Control, Target, Workspaces};
use route_contract::Digest;

pub(super) const DOCUMENT: &str = "workspace-document";
const LINE_HEIGHT: f32 = 20.0;

pub(super) struct DocumentView {
    key: Option<(String, Digest, Option<Digest>)>,
    transcript: Transcript,
    size: (f32, f32),
    row_top: f32,
}
impl Default for DocumentView {
    fn default() -> Self {
        let mut transcript = chat_transcript();
        transcript
            .set_metrics(rust_native::layout::Metrics {
                reading_width: 16384,
                row_gap: 0,
                markdown: Some(rust_native::layout::MarkdownMetrics {
                    headings: [[16, 24]; 4],
                    code_size_half_points: 26,
                    code_line_height: 20,
                    code_header_height: 24,
                    code_label_size: 11,
                    code_padding_y: 8,
                    copy_icon: true,
                    inline_code: None,
                    strong_weight: None,
                }),
                ..Default::default()
            })
            .expect("file metrics");
        Self {
            key: None,
            transcript,
            size: (0.0, 0.0),
            row_top: 0.0,
        }
    }
}
impl DocumentView {
    pub(super) fn release(&mut self) {
        *self = Self::default();
    }
    pub(super) fn version(&self) -> u64 {
        self.transcript.version()
    }
    pub(super) fn access(&self) -> rust_native_desktop::access::Content {
        let mut content = self.transcript.access();
        content.conversation = false;
        content
    }
    pub(super) fn remember(&self, files: &mut Workspaces) {
        if let Some((chat, id, _)) = &self.key
            && let Some(tab) = files.selected_mut(chat)
            && &tab.id == id
        {
            // Native anchors are relative to the code row's top margin.
            tab.scroll = self
                .transcript
                .reading_anchor()
                .map_or(0.0, |(_, y)| y + self.row_top);
        }
    }
    fn prepare(&mut self, files: &mut Workspaces, chat: &str, width: f32, height: f32) {
        let Some(tab) = files.selected_mut(chat) else {
            self.release();
            return;
        };
        let key = (chat.to_owned(), tab.id.clone(), tab.content_id.clone());
        if self.key.as_ref() != Some(&key) {
            self.release();
            self.key = Some(key);
            let Some(content) = &tab.content else {
                return;
            };
            let language = tab.key.path.rsplit_once('.').map(|(_, ext)| ext.to_owned());
            let rows = vec![Node {
                key: "workspace-code".into(),
                style: Style::default(),
                element: Element::Markdown {
                    blocks: vec![rust_native::markdown::Block::Code {
                        language,
                        text: content.as_ref().clone(),
                    }],
                },
            }];
            self.transcript
                .set_syntax_palette(openagents_chat_app::visual::current().syntax);
            self.transcript
                .set_font_family(rust_native::layout::display::FontFamily::PaperMono);
            let _ = self
                .transcript
                .update(rows, width.max(1.0), height.max(1.0));
            self.transcript.scroll(f32::MAX);
            self.row_top = self
                .transcript
                .access()
                .bounds
                .get("workspace-code")
                .map_or(0.0, |bounds| bounds.y);
            let y = tab.jump.take().map_or(tab.scroll, |line| {
                if line <= 1 {
                    0.0
                } else {
                    32.0 + line.saturating_sub(1) as f32 * LINE_HEIGHT
                }
            });
            self.transcript.scroll(-y);
            self.size = (width, height);
        } else if self.size != (width, height) {
            // Reuse the text and selection while the pane changes size.
            if let Some(content) = &tab.content {
                let language = tab.key.path.rsplit_once('.').map(|(_, ext)| ext.to_owned());
                let _ = self.transcript.update(
                    vec![Node {
                        key: "workspace-code".into(),
                        style: Style::default(),
                        element: Element::Markdown {
                            blocks: vec![rust_native::markdown::Block::Code {
                                language,
                                text: content.as_ref().clone(),
                            }],
                        },
                    }],
                    width.max(1.0),
                    height.max(1.0),
                );
                self.size = (width, height);
            }
        }
        if let Some(line) = tab.jump.take() {
            self.transcript.scroll(f32::MAX);
            self.transcript.scroll(
                -(if line <= 1 {
                    0.0
                } else {
                    32.0 + line.saturating_sub(1) as f32 * LINE_HEIGHT
                }),
            );
        }
    }
    fn scroll(&mut self, dy: f32, files: &mut Workspaces) {
        self.transcript.scroll(dy);
        self.remember(files);
    }
    fn paint(
        &mut self,
        files: &mut Workspaces,
        chat: &str,
        frame: &mut Frame,
        rect: PxRect,
        scale: f32,
        fonts: &mut Fonts,
    ) {
        self.prepare(files, chat, rect.w / scale, rect.h / scale);
        self.transcript.paint(frame, rect, scale, fonts);
        self.remember(files);
    }
}
impl Panel {
    pub fn files_open(&self) -> bool {
        !self.saved_visible
            && self
                .session
                .selected
                .as_deref()
                .is_some_and(|chat| self.files.visible(chat))
    }
    pub(super) fn files_beside(&self) -> bool {
        let width = if self.workspace_width > 0.0 {
            self.workspace_width
        } else {
            self.viewport.0 - self.sidebar_width
        };
        width >= 840.0
    }
    pub fn file_outcome(
        &mut self,
        read: files::Read,
        result: Result<route_contract::workspace::Output, String>,
    ) {
        if self.files.received(&read, result) {
            self.rows_dirty = true;
        }
    }
    pub(super) fn file_action(&mut self, target: Target) {
        if self.session.selected.as_ref() != Some(&target.conversation) {
            return;
        }
        self.file_view.remember(&mut self.files);
        let previous = self
            .files
            .selected(&target.conversation)
            .map(|t| t.id.clone());
        if self.files.apply(&target) {
            if !self.files_open()
                || previous
                    != self
                        .files
                        .selected(&target.conversation)
                        .map(|t| t.id.clone())
            {
                self.file_view.release();
            }
            self.file_focus = self.files_open()
                && self
                    .session
                    .selected
                    .as_deref()
                    .and_then(|chat| self.files.selected(chat))
                    .is_some()
                && !self.files.browser_visible(&target.conversation);
            if !self.file_focus {
                self.focus_composer();
            } else if let Some(field) = self.field() {
                field.focused = false;
            }
            if let Some(chat) = self.session.selected.as_deref() {
                self.files.focus(chat, self.file_focus);
            }
            self.rows_dirty = true;
        }
    }
    pub(super) fn file_pane(&self) -> Node<Intent> {
        let chat = self.session.selected.as_deref().unwrap_or("");
        let mut children = vec![text("workspace-pane-title", "Files", TextRole::Heading)];
        if self.files.browser_visible(chat)
            && self.task().is_none()
            && let Some(run) = self.run()
        {
            let search = Node {
                key: "workspace-query".into(),
                style: Style::default(),
                element: Element::Composer {
                    token: format!("workspace:{chat}"),
                    placeholder: "Search this folder".into(),
                    max_bytes: 256,
                    enabled: true,
                    busy: false,
                    stop: None,
                    choices: vec![],
                    draft: Some(run.workspace().query.clone()),
                    focus: self.workspace_query.focused,
                    attach: None,
                },
            };
            if let Some(browser) = run.workspace_view(Some(search), |target| Intent::Chat {
                action: Action::Workspace { target },
            }) {
                children.push(browser);
            }
        }
        if let Some(tabs) = self.files.view(chat, |target| Intent::Chat {
            action: Action::FileTab { target },
        }) {
            children.push(tabs);
        }
        if !self.files.browser_visible(chat)
            && self
                .files
                .selected(chat)
                .and_then(|tab| tab.content.as_ref())
                .is_some()
        {
            children.push(Node {
                key: "workspace-file-surface".into(),
                style: Style {
                    fill_height: Some(true),
                    ..Default::default()
                },
                element: Element::Surface {
                    label: "File contents, read only".into(),
                    resource: DOCUMENT.into(),
                },
            });
        }
        let mut pane = stack("workspace-pane", Axis::Vertical, children);
        pane.style.fill_height = Some(true);
        pane.style.background = Some(openagents_chat_app::visual::current().canvas);
        pane.style.padding_start = Some(Space::Sm);
        pane.style.padding_end = Some(Space::Sm);
        pane
    }
    pub(super) fn file_shortcut(&mut self, event: &TextInput<'_>) -> bool {
        if !self.file_focus || !self.files_open() || self.modal() {
            return false;
        }
        let Some(chat) = self.session.selected.clone() else {
            return false;
        };
        let TextInput::Key {
            key,
            command,
            control,
            shift,
            ..
        } = event
        else {
            return false;
        };
        let modifier = *command || *control;
        let action = match *key {
            "w" | "W" if modifier => self
                .files
                .selected(&chat)
                .map(|tab| Control::Close(tab.id.clone())),
            "Tab" if *control => Some(if *shift {
                Control::Previous
            } else {
                Control::Next
            }),
            "Escape" => {
                self.file_focus = false;
                self.focus_composer();
                return true;
            }
            "c" | "C" if modifier => {
                let value = self.file_view.transcript.selected_text();
                if !value.is_empty() {
                    let _ = std::thread::Builder::new()
                        .name("file-copy".into())
                        .spawn(move || rust_native_desktop::input::copy(&value));
                }
                return true;
            }
            "ArrowUp" | "Up" => {
                self.file_view.scroll(LINE_HEIGHT, &mut self.files);
                return true;
            }
            "ArrowDown" | "Down" => {
                self.file_view.scroll(-LINE_HEIGHT, &mut self.files);
                return true;
            }
            "PageUp" => {
                self.file_view
                    .scroll(self.file_view.size.1.max(40.0) * 0.8, &mut self.files);
                return true;
            }
            "PageDown" => {
                self.file_view
                    .scroll(-self.file_view.size.1.max(40.0) * 0.8, &mut self.files);
                return true;
            }
            "Home" => {
                self.file_view.scroll(f32::MAX, &mut self.files);
                return true;
            }
            "End" => {
                self.file_view.scroll(-f32::MAX, &mut self.files);
                return true;
            }
            _ => None,
        };
        if let Some(action) = action {
            self.file_action(self.files.target(&chat, action));
            return true;
        }
        false
    }
    pub(super) fn file_surface(&mut self, event: SurfaceInput, now: Instant) -> bool {
        if !self.files_open() {
            return false;
        }
        self.file_focus = true;
        if let Some(field) = self.field() {
            field.input(TextInput::FocusLost, 0);
        }
        self.workspace_query.input(TextInput::FocusLost, 0);
        self.search.input(TextInput::FocusLost, 0);
        if let Some(action) = self.file_view.transcript.pointer(event, &mut self.fonts) {
            if let rust_native_desktop::transcript::Action::Copy(value) = action {
                let _ = std::thread::Builder::new()
                    .name("file-copy".into())
                    .spawn(move || rust_native_desktop::input::copy(&value));
            }
        }
        self.file_view.remember(&mut self.files);
        if let Some(chat) = self.session.selected.as_deref() {
            self.files.focus(chat, true);
        }
        let _ = now;
        !matches!(event, SurfaceInput::Move { .. }) || self.file_view.transcript.dragging()
    }
    pub(super) fn paint_file(&mut self, frame: &mut Frame, rect: PxRect) {
        let Some(chat) = self.session.selected.clone() else {
            return;
        };
        self.file_view.paint(
            &mut self.files,
            &chat,
            frame,
            rect,
            self.viewport.2,
            &mut self.fonts,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::{digest_of, workspace as wire};
    fn reference() -> wire::Reference {
        let mut reference = wire::Reference {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            authority: route_contract::pending::Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/fixture".into(),
                target: "inode".into(),
            },
        };
        reference.seal();
        reference
    }
    fn mounted(panel: &mut Panel, revision: u64) -> ValidatedView<Intent> {
        let body = panel.body();
        let footer = panel.footer();
        let view = rust_native::View::new(
            "files-test",
            revision,
            stack("files-root", Axis::Vertical, vec![body, footer]),
        )
        .validate()
        .unwrap();
        panel.mounted(&view);
        view
    }
    fn panel() -> Panel {
        let now = Instant::now();
        let mut panel = Panel::new(now);
        panel.viewport = (1280.0, 1000.0, 1.0);
        panel.workspace_width = 1000.0;
        panel.session.select("chat");
        panel.selected_changed(None);
        panel
            .runs
            .insert("chat".into(), Run::follow("chat", "task", None, now));
        mounted(&mut panel, 1);
        panel
    }
    fn load(panel: &mut Panel, path: &str, text: &str) -> Digest {
        let id = panel
            .files
            .open(
                "chat",
                files::Open {
                    reference: reference(),
                    path: path.into(),
                    line: None,
                },
            )
            .unwrap();
        let read = panel.files.next_read("chat").unwrap();
        panel.file_outcome(
            read.clone(),
            Ok(wire::Output {
                reference: read.request.reference.id.clone(),
                operation: read.request.operation.id(),
                rows: vec![],
                text: Some(text.into()),
                content: Some(Digest::of_bytes(text.as_bytes())),
                truncated: false,
            }),
        );
        id
    }
    #[test]
    fn browser_file_selection_opens_a_tab_and_leaves_the_conversation_and_browser_intact() {
        let mut panel = panel();
        let now = Instant::now();
        panel.input(TextInput::Commit("Keep my draft"), now);
        let target = panel.runs["chat"]
            .workspace()
            .target(coder_run::workspace::Control::Open);
        let view = mounted(&mut panel, 2);
        panel.action(Action::Workspace { target }, &view, now);
        let (ticket, _) = panel.runs.get_mut("chat").unwrap().tick(now).unwrap();
        panel.run_outcome(
            "chat".into(),
            ticket,
            Ok(coder_run::Answer::WorkspaceReference(reference())),
        );
        let (ticket, request) = panel.runs.get_mut("chat").unwrap().tick(now).unwrap();
        let coder_run::Request::WorkspaceRead { request, .. } = request else {
            panic!("list")
        };
        panel.run_outcome(
            "chat".into(),
            ticket,
            Ok(coder_run::Answer::WorkspaceRead(wire::Output {
                reference: request.reference.id,
                operation: request.operation.id(),
                rows: vec![wire::Row::new(
                    "src/main.rs".into(),
                    wire::Kind::File,
                    None,
                    String::new(),
                )],
                text: None,
                content: None,
                truncated: false,
            })),
        );
        let target = panel.runs["chat"]
            .workspace()
            .target(coder_run::workspace::Control::Row(0));
        let view = mounted(&mut panel, 3);
        assert!(
            panel
                .action(Action::Workspace { target }, &view, now)
                .is_none()
        );
        assert_eq!(
            panel.files.selected("chat").unwrap().key.path,
            "src/main.rs"
        );
        assert_eq!(panel.draft(), "Keep my draft");
        assert!(panel.take_requests().is_empty());
        let read = panel.files.next_read("chat").unwrap();
        assert!(read.valid());
        assert!(
            matches!(&read.request.operation,wire::Operation::Read {path} if path=="src/main.rs")
        );
        let browser = panel.runs["chat"].workspace_view(None, |_| ()).unwrap();
        assert!(
            crate::screens::words(&browser)
                .iter()
                .any(|text| text == "src/main.rs")
        );
        assert_eq!(panel.runs["chat"].state(), coder_run::State::Running);
        let fixture = include_str!(
            "../../../openagents-chat/fixtures/coder-events/question-then-result.ndjson"
        );
        let mut started: serde_json::Value =
            serde_json::from_str(fixture.lines().next().unwrap()).unwrap();
        started["task"] = "task".into();
        started["turn"] = 2.into();
        let (ticket, request) = panel
            .runs
            .get_mut("chat")
            .unwrap()
            .tick(now + std::time::Duration::from_secs(2))
            .unwrap();
        assert!(
            matches!(request, coder_run::Request::Poll { .. }),
            "{request:?}"
        );
        panel.run_outcome(
            "chat".into(),
            ticket,
            Ok(coder_run::Answer::Lines {
                children: None,
                configuration: None,
                native: None,
                lines: vec![serde_json::from_value(started).unwrap()],
                state: coder_run::State::Running,
            }),
        );
        assert!(panel.files.selected("chat").unwrap().stale);
        assert!(!panel.files.received(
            &read,
            Ok(wire::Output {
                reference: read.request.reference.id.clone(),
                operation: read.request.operation.id(),
                rows: vec![],
                text: Some("old turn".into()),
                content: Some(Digest::of_bytes(b"old turn")),
                truncated: false,
            })
        ));
        assert_eq!(panel.draft(), "Keep my draft");
    }
    #[test]
    fn tab_and_conversation_switches_restore_scroll_and_never_edit_the_composer() {
        let mut panel = panel();
        let now = Instant::now();
        panel.input(TextInput::Commit("First draft"), now);
        let content = (0..200)
            .map(|n| format!("line {n:03} alpha beta gamma\n"))
            .collect::<String>();
        let first = load(&mut panel, "first.rs", &content);
        panel
            .file_view
            .prepare(&mut panel.files, "chat", 300.0, 120.0);
        panel.file_view.scroll(-400.0, &mut panel.files);
        let scroll = panel.files.selected("chat").unwrap().scroll;
        assert!(scroll > 100.0);
        let second = load(&mut panel, "second.rs", "other text\n");
        panel
            .file_view
            .prepare(&mut panel.files, "chat", 300.0, 120.0);
        panel.file_action(panel.files.target("chat", Control::Select(first.clone())));
        panel
            .file_view
            .prepare(&mut panel.files, "chat", 300.0, 120.0);
        assert!((panel.files.selected("chat").unwrap().scroll - scroll).abs() < 1.0);
        assert!(
            (panel.file_view.transcript.reading_anchor().unwrap().1 + panel.file_view.row_top
                - scroll)
                .abs()
                < 1.0,
            "native scroll {:?}, stored {scroll}",
            panel.file_view.transcript.reading_anchor()
        );
        panel.input(TextInput::Commit("This must not type into chat"), now);
        assert_eq!(panel.draft(), "First draft");
        panel.session.select("other");
        panel.selected_changed(Some("chat".into()));
        assert!(
            (panel.files.selected("chat").unwrap().scroll - scroll).abs() < 1.0,
            "scroll after leaving: {}",
            panel.files.selected("chat").unwrap().scroll
        );
        mounted(&mut panel, 4);
        panel.input(TextInput::Commit("Second draft"), now);
        let stale = panel.files.target("chat", Control::Close(first.clone()));
        let view = mounted(&mut panel, 5);
        panel.action(Action::FileTab { target: stale }, &view, now);
        assert_eq!(panel.files.tabs("chat").len(), 2);
        panel.session.select("chat");
        panel.selected_changed(Some("other".into()));
        mounted(&mut panel, 6);
        assert_eq!(panel.files.selected("chat").unwrap().id, first);
        assert_eq!(panel.draft(), "First draft");
        assert!(panel.file_focus);
        panel
            .file_view
            .prepare(&mut panel.files, "chat", 300.0, 120.0);
        assert!((panel.files.selected("chat").unwrap().scroll - scroll).abs() < 1.0);
        panel.file_action(panel.files.target("chat", Control::Select(second)));
        assert_eq!(panel.draft(), "First draft");
        panel.input(
            TextInput::Key {
                key: "Escape",
                text: None,
                shift: false,
                command: false,
                control: false,
                alt: false,
            },
            now,
        );
        assert!(!panel.file_focus);
        assert_eq!(panel.draft(), "First draft");
    }
    #[test]
    fn a_closed_tabs_late_reply_does_not_reopen_it_or_clear_a_new_draft() {
        let mut panel = panel();
        let now = Instant::now();
        panel.input(TextInput::Commit("Still here"), now);
        let id = panel
            .files
            .open(
                "chat",
                files::Open {
                    reference: reference(),
                    path: "file.txt".into(),
                    line: None,
                },
            )
            .unwrap();
        let read = panel.files.next_read("chat").unwrap();
        panel.file_action(panel.files.target("chat", Control::Close(id)));
        panel.file_outcome(
            read.clone(),
            Ok(wire::Output {
                reference: read.request.reference.id,
                operation: read.request.operation.id(),
                rows: vec![],
                text: Some("late".into()),
                content: Some(Digest::of_bytes(b"late")),
                truncated: false,
            }),
        );
        assert!(panel.files.selected("chat").is_none());
        assert_eq!(panel.draft(), "Still here");
        assert!(panel.take_requests().is_empty());
    }
    #[test]
    fn native_text_selection_and_read_only_keyboard_navigation_work_in_the_file_surface() {
        let mut panel = panel();
        let now = Instant::now();
        let content = (0..100)
            .map(|n| format!("line {n:03} alpha beta gamma\n"))
            .collect::<String>();
        load(&mut panel, "file.rs", &content);
        panel
            .file_view
            .prepare(&mut panel.files, "chat", 320.0, 160.0);
        panel.file_surface(
            SurfaceInput::Down {
                x: 48.0,
                y: 54.0,
                shift: false,
            },
            now,
        );
        panel.file_surface(SurfaceInput::Move { x: 180.0, y: 54.0 }, now);
        panel.file_surface(SurfaceInput::Up { x: 180.0, y: 54.0 }, now);
        assert!(!panel.file_view.transcript.selected_text().is_empty());
        let accessible = panel.access_content(DOCUMENT).unwrap();
        assert!(!accessible.conversation);
        assert_eq!(accessible.rows.len(), 1);
        assert!(panel.file_shortcut(&TextInput::Key {
            key: "PageDown",
            text: None,
            shift: false,
            command: false,
            control: false,
            alt: false
        }));
        assert!(panel.files.selected("chat").unwrap().scroll > 0.0);
        assert_eq!(
            panel
                .files
                .selected("chat")
                .unwrap()
                .content
                .as_deref()
                .map(String::as_str),
            Some(content.as_str())
        );
        assert!(panel.file_shortcut(&TextInput::Key {
            key: "w",
            text: None,
            shift: false,
            command: true,
            control: false,
            alt: false
        }));
        assert!(panel.files.tabs("chat").is_empty());
        assert!(!panel.file_focus);
    }
    #[test]
    fn file_panes_fit_beside_chat_or_take_the_available_column_without_changing_tabs() {
        let mut panel = panel();
        let id = load(&mut panel, "file.rs", "fn main() {}\n");
        assert_eq!(panel.body().key, "workspace-split");
        panel.workspace_width = 600.0;
        assert_eq!(panel.body().key, "workspace-pane");
        assert_eq!(panel.files.selected("chat").unwrap().id, id);
        let view = mounted(&mut panel, 2);
        assert!(view.view().root.key == "files-root");
        assert!(panel.files_open());
        panel.file_action(panel.files.target("chat", Control::Hide));
        assert!(!panel.files_open());
        assert_eq!(panel.files.tabs("chat").len(), 1);
    }
    #[test]
    fn maximum_text_reads_paint_and_scroll_in_a_bounded_surface() {
        let mut panel = panel();
        let text = "let value = 1;\n".repeat(4096);
        assert!(text.len() <= 64 * 1024);
        load(&mut panel, "large.rs", &text);
        let mut frame = Frame::new(320, 160, Color::rgb(6, 6, 6));
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 320.0,
            h: 160.0,
        };
        assert!(panel.paint(DOCUMENT, &mut frame, rect));
        panel.file_view.scroll(-f32::MAX, &mut panel.files);
        assert!(panel.files.selected("chat").unwrap().scroll > 40_000.0);
        assert!(panel.paint(DOCUMENT, &mut frame, rect));
        assert!(
            frame
                .pixels
                .chunks(4)
                .any(|pixel| pixel[0] != 6 || pixel[1] != 6 || pixel[2] != 6)
        );
    }
    #[test]
    fn native_layout_keeps_the_document_inside_the_window_at_both_pane_widths() {
        use rust_native_desktop::{layout, theme::Theme};
        let mut panel = panel();
        load(&mut panel, "file.rs", "fn main() {}\n");
        for width in [1280.0, 800.0] {
            panel.viewport = (width, 840.0, 1.0);
            panel.workspace_width = width - 296.0;
            let leading = stack(
                "leading",
                Axis::Vertical,
                vec![
                    text("leading-header", "OpenAgents", TextRole::Heading),
                    text("leading-body", "Conversations", TextRole::Body),
                    text("leading-footer", "Account", TextRole::Body),
                ],
            );
            let content = stack(
                "content",
                Axis::Vertical,
                vec![
                    text("content-header", "Conversation", TextRole::Heading),
                    panel.body(),
                    panel.footer(),
                ],
            );
            let view = rust_native::View::new(
                "file-layout",
                1,
                stack("window", Axis::Horizontal, vec![leading, content]),
            );
            let scene = layout::lay_out_with_layout(
                &view,
                &Theme::default(),
                &mut Fonts::new(),
                &|resource, available| panel.size(resource, available),
                &layout::Interaction::default(),
                width,
                840.0,
                layout::WindowLayout::Split(layout::SplitLayout {
                    leading_width: 280.0,
                    min_leading_width: 224.0,
                    max_leading_width: 400.0,
                    min_content_width: 360.0,
                    collapsed: false,
                    center_content: false,
                    center_footer: false,
                }),
            );
            let document = scene.surface_rect(DOCUMENT).expect("file surface");
            assert!(document.w >= 200.0 && document.h >= 80.0, "{document:?}");
            assert!(
                document.x >= 280.0 && document.x + document.w <= width,
                "{document:?}"
            );
            assert!(
                document.y >= 0.0 && document.y + document.h <= 840.0,
                "{document:?}"
            );
        }
    }
}
