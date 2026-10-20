//! Editing in the file panel (#12272): a text file the computer lets us
//! write opens in an editor that saves with Cmd/Ctrl+S (or by itself after
//! the autosave delay), keeps up with changes made on the computer, and
//! says so when both changed. Rename, move (drag a row onto a folder), and
//! delete go through the computer's mutation methods, which refuse a stale
//! entry rather than replace anything.

use gpui::{Subscription, WeakEntity};
use openagents_proto::{
    DeleteWorkspaceEntryRequest, MoveWorkspaceEntryRequest, WorkspaceEntryKind,
    WorkspaceLineEnding, WorkspaceMutationOutcome, WorkspaceTextEncoding,
    WorkspaceWritableEncoding, WorkspaceWritableLineEnding, WriteWorkspaceFileOutcome,
    WriteWorkspaceFileRequest,
};
use openagents_web_chat_view::composer::{Edited, SaveRequested};

use super::*;

pub(super) mod write_methods {
    pub const WRITE_WORKSPACE_FILE: &str = "WriteWorkspaceFile";
    pub const MOVE_WORKSPACE_ENTRY: &str = "MoveWorkspaceEntry";
    pub const DELETE_WORKSPACE_ENTRY: &str = "DeleteWorkspaceEntry";
}

/// An open file's editor and what the computer last had.
pub(super) struct Editing {
    pub editor: Entity<TextInput>,
    /// The text as last read or saved.
    saved: String,
    hash: String,
    encoding: WorkspaceWritableEncoding,
    line_ending: WorkspaceWritableLineEnding,
    pub saving: bool,
    /// The file changed on the computer while this tab had unsaved changes.
    pub conflict: bool,
    _autosave: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// A row being dragged onto a folder.
#[derive(Clone)]
pub(super) struct Dragged {
    pub path: String,
    pub name: String,
}

/// What follows the pointer while a row is dragged.
pub(super) struct DragLabel(SharedString);

impl Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(4.))
            .rounded(px(8.))
            .bg(theme::surface_raised())
            .border_1()
            .border_color(theme::border_strong())
            .text_size(px(13.))
            .text_color(theme::text())
            .child(self.0.clone())
    }
}

pub(super) fn drag_label(
    dragged: &Dragged,
    _: Point<Pixels>,
    _: &mut Window,
    cx: &mut App,
) -> Entity<DragLabel> {
    cx.new(|_| DragLabel(dragged.name.clone().into()))
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

const SETTINGS_KEY: &str = "openagents.files.settings.v1";

pub(super) fn read_settings() -> model::Settings {
    let json = storage().and_then(|storage| storage.get_item(SETTINGS_KEY).ok().flatten());
    model::Settings::read(json.as_deref())
}

impl Editing {
    pub fn dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).text() != self.saved
    }
}

impl FilesApp {
    pub(super) fn save_settings(&mut self, cx: &mut Context<Self>) {
        if let (Some(storage), Ok(json)) = (storage(), serde_json::to_string(&self.settings)) {
            let _ = storage.set_item(SETTINGS_KEY, &json);
        }
        let wrap = self.settings.word_wrap;
        for tab in &self.tabs {
            if let Some(edit) = &tab.edit {
                edit.editor
                    .update(cx, |editor, cx| editor.set_wrap(wrap, cx));
            }
        }
        cx.notify();
    }

    fn tab_at(&self, path: &str) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.path == path)
    }

    /// The open file at `path` was read: give it an editor when the
    /// computer lets us write it, or bring its editor up to date.
    pub(super) fn attach_editor(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(at) = self.tab_at(path) else {
            return;
        };
        let wrap = self.settings.word_wrap;
        let tab = &mut self.tabs[at];
        let Body::Text { file, .. } = &tab.body else {
            return;
        };
        let writable = file.read_only_reason.is_none() && !file.truncated && model::has_text(file);
        let (Some(text), Some(hash), true) =
            (file.text.clone(), file.content_hash.clone(), writable)
        else {
            tab.edit = None;
            return;
        };
        let encoding = match file.encoding {
            WorkspaceTextEncoding::Utf8Bom => WorkspaceWritableEncoding::Utf8Bom,
            _ => WorkspaceWritableEncoding::Utf8,
        };
        let line_ending = match file.line_ending {
            Some(WorkspaceLineEnding::Crlf) => WorkspaceWritableLineEnding::Crlf,
            _ => WorkspaceWritableLineEnding::Lf,
        };
        if let Some(edit) = &mut tab.edit {
            let current = edit.editor.read(cx).text();
            if current == text {
                edit.conflict = false;
            } else if current != edit.saved {
                // Both changed: say so, and keep the person's text.
                edit.conflict = true;
                cx.notify();
                return;
            } else {
                let cursor = edit.editor.read(cx).cursor();
                edit.editor
                    .update(cx, |editor, cx| editor.reset(text.clone(), cursor, cx));
                edit.conflict = false;
            }
            edit.saved = text;
            edit.hash = hash;
            edit.encoding = encoding;
            edit.line_ending = line_ending;
            cx.notify();
            return;
        }
        let editor = cx.new(|cx| {
            let mut editor = TextInput::editor(cx);
            editor.set_wrap(wrap, cx);
            editor.reset(text.clone(), 0, cx);
            editor
        });
        let edited_path = path.to_owned();
        let saved_path = path.to_owned();
        let subscriptions = vec![
            cx.subscribe(&editor, move |this, _, _: &Edited, cx| {
                this.edited(&edited_path, cx)
            }),
            cx.subscribe(&editor, move |this, _, _: &SaveRequested, cx| {
                this.save_file(&saved_path, None, cx)
            }),
        ];
        tab.edit = Some(Editing {
            editor,
            saved: text,
            hash,
            encoding,
            line_ending,
            saving: false,
            conflict: false,
            _autosave: None,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    fn edited(&mut self, path: &str, cx: &mut Context<Self>) {
        let autosave = self.settings.autosave;
        let delay = self.settings.autosave_delay_ms;
        let Some(at) = self.tab_at(path) else {
            return;
        };
        let Some(edit) = &mut self.tabs[at].edit else {
            return;
        };
        cx.notify();
        if !autosave || edit.conflict {
            edit._autosave = None;
            return;
        }
        let path = path.to_owned();
        edit._autosave = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(delay))
                .await;
            this.update(cx, |this, cx| this.save_file(&path, None, cx))
                .ok();
        }));
    }

    /// Write the editor's text. `over` is the computer's current content
    /// hash when the person chose to save over a change made there.
    pub(super) fn save_file(&mut self, path: &str, over: Option<String>, cx: &mut Context<Self>) {
        let (Some(target), Some(at)) = (self.target.clone(), self.tab_at(path)) else {
            return;
        };
        let checkout = self.checkout.clone().unwrap_or_default();
        let Some(edit) = &mut self.tabs[at].edit else {
            return;
        };
        let text = edit.editor.read(cx).text();
        if edit.saving
            || (text == edit.saved && over.is_none())
            || (edit.conflict && over.is_none())
        {
            return;
        }
        edit.saving = true;
        edit._autosave = None;
        let request = WriteWorkspaceFileRequest {
            expected_checkout_id: checkout,
            target: target.workspace(),
            path: path.to_owned(),
            text: text.clone(),
            expected_content_hash: over.unwrap_or_else(|| edit.hash.clone()),
            encoding: edit.encoding,
            line_ending: edit.line_ending,
        };
        let client = self.client.clone();
        let path = path.to_owned();
        cx.spawn(async move |this, cx| {
            let outcome = async {
                let value = Self::call(
                    &client,
                    &target,
                    write_methods::WRITE_WORKSPACE_FILE,
                    serde_json::to_value(&request)?,
                )
                .await?;
                anyhow::Ok(serde_json::from_value::<WriteWorkspaceFileOutcome>(value)?)
            }
            .await;
            this.update(cx, |this, cx| {
                let Some(at) = this.tab_at(&path) else {
                    return;
                };
                let tab = &mut this.tabs[at];
                let Some(edit) = &mut tab.edit else {
                    return;
                };
                edit.saving = false;
                match outcome {
                    Ok(WriteWorkspaceFileOutcome::Written { file }) => {
                        edit.saved = text;
                        edit.hash = file.content_hash;
                        edit.conflict = false;
                        tab.size = Some(file.size);
                    }
                    Ok(WriteWorkspaceFileOutcome::Conflict { .. }) => {
                        edit.conflict = true;
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Save the editor's text over what the computer has now.
    pub(super) fn save_over(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let client = self.client.clone();
        let path = path.to_owned();
        cx.spawn(async move |this, cx| {
            let request = ReadWorkspaceFileRequest {
                target: target.workspace(),
                path: path.clone(),
            };
            let current = async {
                let value = Self::call(
                    &client,
                    &target,
                    methods::READ_WORKSPACE_FILE,
                    serde_json::to_value(&request)?,
                )
                .await?;
                anyhow::Ok(serde_json::from_value::<WorkspaceFileText>(value)?)
            }
            .await;
            this.update(cx, |this, cx| match current {
                Ok(file) => {
                    let hash = file.content_hash.unwrap_or_default();
                    this.save_file(&path, Some(hash), cx);
                }
                Err(error) => {
                    this.notice = Some(error.to_string());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Drop the unsaved text and read the file again.
    pub(super) fn reload_file(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(at) = self.tab_at(path) {
            self.tabs[at].edit = None;
            self.load(at, cx);
        }
    }

    /// Whether a change on the computer should read `at` again: always,
    /// except while a download runs.
    pub(super) fn reload_wanted(&self, at: usize) -> bool {
        self.tabs
            .get(at)
            .is_some_and(|tab| tab.downloading.is_none())
    }

    pub(super) fn render_save_state(
        &self,
        at: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tab = self.tabs.get(at)?;
        let edit = tab.edit.as_ref()?;
        let path = tab.path.clone();
        if edit.conflict {
            let reload = path.clone();
            return Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(footer_text("Changed on the computer"))
                    .child(
                        button("conflict-reload", "Reload", false).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| this.reload_file(&reload, cx),
                        )),
                    )
                    .child(button("conflict-keep", "Save mine", false).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.save_over(&path, cx)),
                    ))
                    .into_any_element(),
            );
        }
        if edit.saving {
            return Some(footer_text("Saving…").into_any_element());
        }
        if edit.dirty(cx) {
            return Some(
                button("save", "Save", true)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.save_file(&path, None, cx)
                    }))
                    .into_any_element(),
            );
        }
        Some(footer_text("Saved").into_any_element())
    }

    // -----------------------------------------------------------------
    // Rename, move, delete
    // -----------------------------------------------------------------

    fn entry(&self, path: &str) -> Option<&WorkspaceEntry> {
        self.listed
            .get(model::parent(path))?
            .iter()
            .find(|entry| entry.path == path)
    }

    pub(super) fn start_rename(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let field = cx.new(|cx| {
            let mut field = TextInput::new("New name", cx);
            field.set_text(name, cx);
            field
        });
        let source = path.to_owned();
        cx.subscribe(&field, move |this, _, event: &Submitted, cx| {
            this.renaming = None;
            let name = event.0.trim().to_owned();
            if name.is_empty() || model::sibling(&source, &name) == source {
                return cx.notify();
            }
            if !model::valid_name(&name) {
                this.notice = Some("A name can't contain a slash or be . or ..".into());
                return cx.notify();
            }
            this.move_entry(&source, &model::sibling(&source, &name), cx);
        })
        .detach();
        cx.subscribe(&field, |this, _, _: &Cancelled, cx| {
            this.renaming = None;
            cx.notify();
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        self.renaming = Some((path.to_owned(), field));
        self.deleting = None;
        cx.notify();
    }

    /// Move or rename `source` to `destination` on the computer.
    pub(super) fn move_entry(&mut self, source: &str, destination: &str, cx: &mut Context<Self>) {
        if source == destination
            || destination.starts_with(&format!("{source}/"))
            || model::parent(source) == destination && self.entry(source).is_none()
        {
            return;
        }
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(entry) = self.entry(source).cloned() else {
            return;
        };
        let Some(revision) = entry.mutation_revision.clone() else {
            self.notice = Some(
                "Update the OpenAgents app on this computer to change files from here.".into(),
            );
            return cx.notify();
        };
        let request = MoveWorkspaceEntryRequest {
            target: target.workspace(),
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_checkout_id: self.checkout.clone().unwrap_or_default(),
            source_path: source.to_owned(),
            destination_path: destination.to_owned(),
            expected_source_revision: revision,
            expected_kind: entry.kind,
        };
        let (source, destination) = (source.to_owned(), destination.to_owned());
        self.mutate(
            write_methods::MOVE_WORKSPACE_ENTRY,
            serde_json::to_value(&request).unwrap_or_default(),
            move |this, cx| {
                this.list(model::parent(&source), cx);
                this.list(model::parent(&destination), cx);
                // Open files and open folders follow the move.
                let moved = |path: &str| -> Option<String> {
                    if path == source {
                        Some(destination.clone())
                    } else {
                        path.strip_prefix(&format!("{source}/"))
                            .map(|rest| format!("{destination}/{rest}"))
                    }
                };
                for tab in &mut this.tabs {
                    if let Some(path) = moved(&tab.path) {
                        tab.name = path.rsplit('/').next().unwrap_or(&path).to_owned();
                        tab.path = path;
                    }
                }
                this.expanded = this
                    .expanded
                    .drain()
                    .map(|path| moved(&path).unwrap_or(path))
                    .collect();
            },
            cx,
        );
    }

    pub(super) fn delete_entry(&mut self, path: &str, cx: &mut Context<Self>) {
        self.deleting = None;
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(entry) = self.entry(path).cloned() else {
            return;
        };
        let Some(revision) = entry.mutation_revision.clone() else {
            self.notice = Some(
                "Update the OpenAgents app on this computer to change files from here.".into(),
            );
            return cx.notify();
        };
        let request = DeleteWorkspaceEntryRequest {
            target: target.workspace(),
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_checkout_id: self.checkout.clone().unwrap_or_default(),
            path: path.to_owned(),
            expected_source_revision: revision,
            expected_kind: entry.kind,
            recursive: entry.kind == WorkspaceEntryKind::Directory,
        };
        let path = path.to_owned();
        self.mutate(
            write_methods::DELETE_WORKSPACE_ENTRY,
            serde_json::to_value(&request).unwrap_or_default(),
            move |this, cx| {
                this.list(model::parent(&path), cx);
                let inside = format!("{path}/");
                while let Some(at) = this
                    .tabs
                    .iter()
                    .position(|tab| tab.path == path || tab.path.starts_with(&inside))
                {
                    this.close(at, cx);
                }
            },
            cx,
        );
    }

    fn mutate(
        &mut self,
        method: &'static str,
        params: Value,
        applied: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let client = self.client.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let outcome = async {
                let value = Self::call(&client, &target, method, params).await?;
                anyhow::Ok(serde_json::from_value::<WorkspaceMutationOutcome>(value)?)
            }
            .await;
            this.update(cx, |this, cx| {
                match outcome {
                    Ok(WorkspaceMutationOutcome::Applied { .. }) => applied(this, cx),
                    Ok(WorkspaceMutationOutcome::Rejected { message, .. }) => {
                        this.notice = Some(message)
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // -----------------------------------------------------------------
    // Settings
    // -----------------------------------------------------------------

    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let row = |title: &'static str, value: String, id: &'static str| {
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .px(px(16.))
                .py(px(10.))
                .border_b_1()
                .border_color(theme::border())
                .child(div().flex_1().text_size(px(14.)).child(title))
                .child(
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme::border_strong())
                        .text_size(px(13.))
                        .cursor_pointer()
                        .hover(|x| x.bg(theme::element_hover()))
                        .child(value),
                )
        };
        let on_off = |on: bool| if on { "On" } else { "Off" }.to_owned();
        let settings = &self.settings;
        div()
            .flex()
            .flex_col()
            .child(
                row("Autosave", on_off(settings.autosave), "set-autosave").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.settings.autosave = !this.settings.autosave;
                        this.save_settings(cx);
                    },
                )),
            )
            .when(settings.autosave, |list| {
                list.child(
                    row(
                        "Autosave delay",
                        model::delay(settings.autosave_delay_ms),
                        "set-delay",
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.settings.next_delay();
                        this.save_settings(cx);
                    })),
                )
            })
            .child(
                row("Word wrap", on_off(settings.word_wrap), "set-wrap").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.settings.word_wrap = !this.settings.word_wrap;
                        this.save_settings(cx);
                    },
                )),
            )
            .child(
                row("Show dot files", on_off(settings.show_hidden), "set-hidden").on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.settings.show_hidden = !this.settings.show_hidden;
                        this.save_settings(cx);
                    }),
                ),
            )
            .into_any_element()
    }
}
