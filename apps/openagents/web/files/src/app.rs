//! The file panel: a tab strip (the Files tab, then each open file), the
//! tree with git badges and a search over names or contents, and each open
//! file drawn as text, Markdown, or an image, or as a card with its name,
//! size, and Download.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use base64::Engine as _;
use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, Image, ImageFormat, IntoElement,
    ParentElement, Render, SharedString, Styled, Task, Window, div, prelude::*, px, uniform_list,
};
use openagents_gpui_web::img;
use openagents_proto::{
    ListWorkspaceDirectoryRequest, ReadWorkspaceBytesRequest, ReadWorkspaceFileRequest,
    ReadWorkspaceImageRequest, SearchWorkspaceContentRequest, SearchWorkspaceFilesRequest,
    WatchWorkspaceFilesRequest, WorkspaceBytesChunk, WorkspaceContentSearchResult,
    WorkspaceDirectoryPage, WorkspaceEntry, WorkspaceFileChanges, WorkspaceFileSearchMatch,
    WorkspaceFileText, WorkspaceGitStatusFrame, WorkspaceImageChunk, WorkspaceTarget,
};
use openagents_web_chat_view::composer::{Cancelled, Submitted, TextInput};
use openagents_web_chat_view::markdown::Markdown;
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{button, footer_text, truncate};
use openagents_web_client::computer;
use openagents_web_client::web::Client;
use serde_json::Value;

use crate::model::{self, Opens, Row};

/// The computer's method names (`openagents_rpc::methods`).
mod methods {
    pub const LIST_WORKSPACE_DIRECTORY: &str = "ListWorkspaceDirectory";
    pub const READ_WORKSPACE_FILE: &str = "ReadWorkspaceFile";
    pub const READ_WORKSPACE_IMAGE: &str = "ReadWorkspaceImage";
    pub const READ_WORKSPACE_BYTES: &str = "ReadWorkspaceBytes";
    pub const SEARCH_WORKSPACE_FILES: &str = "SearchWorkspaceFiles";
    pub const SEARCH_WORKSPACE_CONTENT: &str = "SearchWorkspaceContent";
    pub const WATCH_WORKSPACE_GIT_STATUS: &str = "WatchWorkspaceGitStatus";
    pub const WATCH_WORKSPACE_FILES: &str = "WatchWorkspaceFiles";
}

/// The largest download the panel assembles in the tab's memory.
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
/// A row's height in the tree and the text view.
const ROW: f32 = 26.;
const LINE: f32 = 20.;

/// The chat whose files the panel shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub device: String,
    pub chat: String,
}

impl Target {
    fn workspace(&self) -> WorkspaceTarget {
        WorkspaceTarget {
            chat_id: Some(self.chat.clone()),
            space_id: None,
            checkout_path: None,
        }
    }
}

/// What an open file shows.
enum Body {
    Loading,
    Text {
        file: WorkspaceFileText,
        lines: Arc<Vec<SharedString>>,
        /// Markdown drawn, or (false) its source.
        rendered: bool,
        markdown: bool,
    },
    Image(Arc<Image>),
    /// No preview: the name and size card with Download.
    Card {
        size: Option<u64>,
    },
    Failed(String),
}

struct Tab {
    path: String,
    name: String,
    size: Option<u64>,
    body: Body,
    /// A download in progress: bytes so far.
    downloading: Option<u64>,
    _task: Option<Task<()>>,
}

enum Found {
    Names(Vec<WorkspaceFileSearchMatch>),
    Contents(WorkspaceContentSearchResult),
}

pub struct FilesApp {
    client: Client,
    target: Option<Target>,
    /// The checkout the last listing came from; reads and writes name it.
    checkout: Option<String>,
    listed: HashMap<String, Vec<WorkspaceEntry>>,
    loading: HashSet<String>,
    expanded: HashSet<String>,
    git: HashMap<String, char>,
    hidden: bool,
    tabs: Vec<Tab>,
    /// The open file tab, or the Files tab when none.
    active: Option<usize>,
    search: Entity<TextInput>,
    contents: bool,
    found: Option<Found>,
    notice: Option<String>,
    _search: Option<Task<()>>,
    _watch: Option<Task<()>>,
    _git: Option<Task<()>>,
}

impl FilesApp {
    pub fn new(target: Option<Target>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search files", cx));
        cx.subscribe(&search, |this, _, event: &Submitted, cx| {
            this.run_search(event.0.clone(), cx)
        })
        .detach();
        cx.subscribe(&search, |this, search, _: &Cancelled, cx| {
            search.update(cx, |search, cx| {
                search.take(cx);
            });
            this.found = None;
            cx.notify();
        })
        .detach();
        let _ = window;
        let mut this = Self {
            client: Client::from_page(),
            target: None,
            checkout: None,
            listed: HashMap::new(),
            loading: HashSet::new(),
            expanded: HashSet::new(),
            git: HashMap::new(),
            hidden: false,
            tabs: Vec::new(),
            active: None,
            search,
            contents: false,
            found: None,
            notice: None,
            _search: None,
            _watch: None,
            _git: None,
        };
        this.show(target, cx);
        this
    }

    /// Show `target`'s files (the open chat changed).
    pub fn show(&mut self, target: Option<Target>, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        self.target = target;
        self.checkout = None;
        self.listed.clear();
        self.loading.clear();
        self.expanded.clear();
        self.git.clear();
        self.tabs.clear();
        self.active = None;
        self.found = None;
        self.notice = None;
        self._watch = None;
        self._git = None;
        if let Some(target) = self.target.clone() {
            self.list("", cx);
            self._git = Some(self.watch_git(target.clone(), cx));
            self._watch = Some(self.watch_files(target, cx));
        }
        cx.notify();
    }

    async fn call(
        client: &Client,
        target: &Target,
        method: &str,
        params: Value,
    ) -> anyhow::Result<Value> {
        let computer = computer::connect(client, &target.device, || {}).await?;
        computer.call(method, params).await
    }

    fn list(&mut self, dir: &str, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        if !self.loading.insert(dir.to_owned()) {
            return;
        }
        let client = self.client.clone();
        let dir = dir.to_owned();
        cx.spawn(async move |this, cx| {
            let mut entries = Vec::new();
            let mut cursor = None;
            let mut checkout = None;
            let result = loop {
                let request = ListWorkspaceDirectoryRequest {
                    target: target.workspace(),
                    directory: dir.clone(),
                    include_ignored: true,
                    cursor: cursor.clone(),
                };
                let page = Self::call(
                    &client,
                    &target,
                    methods::LIST_WORKSPACE_DIRECTORY,
                    serde_json::to_value(&request).unwrap_or_default(),
                )
                .await
                .and_then(|v| Ok(serde_json::from_value::<WorkspaceDirectoryPage>(v)?));
                match page {
                    Ok(page) => {
                        checkout = checkout.or(page.checkout_id);
                        entries.extend(page.entries);
                        match page.next_cursor {
                            Some(next) if entries.len() < 5_000 => cursor = Some(next),
                            _ => break Ok(()),
                        }
                    }
                    Err(error) => break Err(error),
                }
            };
            this.update(cx, |this, cx| {
                this.loading.remove(&dir);
                if this.target.as_ref() != Some(&target) {
                    return;
                }
                match result {
                    Ok(()) => {
                        model::sort(&mut entries);
                        if checkout.is_some() {
                            this.checkout = checkout;
                        }
                        this.listed.insert(dir, entries);
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn watch_git(&mut self, target: Target, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let Ok(computer) = computer::connect(&client, &target.device, || {}).await else {
                return;
            };
            let request = WatchWorkspaceFilesRequest {
                target: target.workspace(),
            };
            let Ok(mut frames) = computer.watch(
                methods::WATCH_WORKSPACE_GIT_STATUS,
                serde_json::to_value(&request).unwrap_or_default(),
            ) else {
                return;
            };
            while let Some(Ok(value)) = frames.next().await {
                let Ok(frame) = serde_json::from_value::<WorkspaceGitStatusFrame>(value) else {
                    continue;
                };
                let badges = frame.status.as_ref().map(model::badges).unwrap_or_default();
                if this
                    .update(cx, |this, cx| {
                        this.git = badges;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    /// Follow changes on the computer: the tree reads changed folders again
    /// and an open file that changed is read again (#12272).
    fn watch_files(&mut self, target: Target, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let Ok(computer) = computer::connect(&client, &target.device, || {}).await else {
                return;
            };
            let request = WatchWorkspaceFilesRequest {
                target: target.workspace(),
            };
            let Ok(mut frames) = computer.watch(
                methods::WATCH_WORKSPACE_FILES,
                serde_json::to_value(&request).unwrap_or_default(),
            ) else {
                return;
            };
            while let Some(Ok(value)) = frames.next().await {
                let Ok(changes) = serde_json::from_value::<WorkspaceFileChanges>(value) else {
                    continue;
                };
                if this
                    .update(cx, |this, cx| this.changed(changes, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn changed(&mut self, changes: WorkspaceFileChanges, cx: &mut Context<Self>) {
        let mut dirs: HashSet<String> = HashSet::new();
        let mut files: HashSet<String> = HashSet::new();
        if changes.resync_required {
            dirs.extend(self.listed.keys().cloned());
            files.extend(self.tabs.iter().map(|tab| tab.path.clone()));
        }
        for change in &changes.changes {
            dirs.insert(model::parent(&change.path).to_owned());
            files.insert(change.path.clone());
            if let Some(old) = &change.old_path {
                dirs.insert(model::parent(old).to_owned());
                files.insert(old.clone());
            }
        }
        for dir in dirs {
            if self.listed.contains_key(&dir) {
                self.list(&dir, cx);
            }
        }
        for at in 0..self.tabs.len() {
            if files.contains(&self.tabs[at].path) && self.reload_wanted(at) {
                self.load(at, cx);
            }
        }
    }

    /// Whether an open file should be read again after a change on the
    /// computer. Editing overrides this (#12272).
    fn reload_wanted(&self, at: usize) -> bool {
        self.tabs
            .get(at)
            .is_some_and(|tab| tab.downloading.is_none())
    }

    fn toggle(&mut self, row: &Row, cx: &mut Context<Self>) {
        if row.dir {
            if !self.expanded.remove(&row.path) {
                self.expanded.insert(row.path.clone());
                if !self.listed.contains_key(&row.path) {
                    self.list(&row.path, cx);
                }
            }
            cx.notify();
        } else {
            self.open(row.path.clone(), row.size, cx);
        }
    }

    fn open(&mut self, path: String, size: Option<u64>, cx: &mut Context<Self>) {
        if let Some(at) = self.tabs.iter().position(|tab| tab.path == path) {
            self.active = Some(at);
            cx.notify();
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
        self.tabs.push(Tab {
            path,
            name,
            size,
            body: Body::Loading,
            downloading: None,
            _task: None,
        });
        let at = self.tabs.len() - 1;
        self.active = Some(at);
        self.load(at, cx);
        cx.notify();
    }

    fn load(&mut self, at: usize, cx: &mut Context<Self>) {
        let (Some(target), Some(tab)) = (self.target.clone(), self.tabs.get_mut(at)) else {
            return;
        };
        let client = self.client.clone();
        let path = tab.path.clone();
        let checkout = self.checkout.clone().unwrap_or_default();
        let opens = model::opens(&path);
        let task = cx.spawn(async move |this, cx| {
            let body = match opens {
                Opens::Image => read_image(&client, &target, &path, &checkout).await,
                Opens::Markdown | Opens::Text => {
                    read_text(&client, &target, &path, opens == Opens::Markdown).await
                }
            };
            let body = body.unwrap_or_else(|error| Body::Failed(error.to_string()));
            this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.path == path) {
                    // A file read again keeps the view the person chose.
                    let keep = match &tab.body {
                        Body::Text { rendered, .. } => Some(*rendered),
                        _ => None,
                    };
                    tab.body = body;
                    if let (Some(keep), Body::Text { rendered, .. }) = (keep, &mut tab.body) {
                        *rendered = keep;
                    }
                    if let Body::Text { file, .. } = &tab.body {
                        tab.size = Some(file.size);
                    }
                    cx.notify();
                }
            })
            .ok();
        });
        if let Some(tab) = self.tabs.get_mut(at) {
            tab._task = Some(task);
        }
    }

    fn close(&mut self, at: usize, cx: &mut Context<Self>) {
        if at >= self.tabs.len() {
            return;
        }
        self.tabs.remove(at);
        self.active = match self.active {
            Some(active) if active == at => {
                if self.tabs.is_empty() {
                    None
                } else {
                    Some(at.min(self.tabs.len() - 1))
                }
            }
            Some(active) if active > at => Some(active - 1),
            other => other,
        };
        cx.notify();
    }

    fn download(&mut self, at: usize, cx: &mut Context<Self>) {
        let (Some(target), Some(tab)) = (self.target.clone(), self.tabs.get_mut(at)) else {
            return;
        };
        if tab.downloading.is_some() {
            return;
        }
        tab.downloading = Some(0);
        let client = self.client.clone();
        let path = tab.path.clone();
        let name = tab.name.clone();
        let checkout = self.checkout.clone().unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let result = async {
                let computer = computer::connect(&client, &target.device, || {}).await?;
                let mut bytes = Vec::new();
                let mut offset = 0u64;
                let mut revision = None;
                loop {
                    let request = ReadWorkspaceBytesRequest {
                        target: target.workspace(),
                        path: path.clone(),
                        expected_checkout_id: checkout.clone(),
                        offset,
                        expected_revision: revision.clone(),
                    };
                    let chunk: WorkspaceBytesChunk = serde_json::from_value(
                        computer
                            .call(
                                methods::READ_WORKSPACE_BYTES,
                                serde_json::to_value(&request)?,
                            )
                            .await?,
                    )?;
                    if chunk.size > MAX_DOWNLOAD {
                        anyhow::bail!(
                            "This file is {}; the browser can download up to {}.",
                            model::size(chunk.size),
                            model::size(MAX_DOWNLOAD)
                        );
                    }
                    bytes.extend(base64::engine::general_purpose::STANDARD.decode(&chunk.data)?);
                    revision.get_or_insert(chunk.revision);
                    let got = bytes.len() as u64;
                    let path = path.clone();
                    this.update(cx, |this, cx| {
                        if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.path == path) {
                            tab.downloading = Some(got);
                            cx.notify();
                        }
                    })
                    .ok();
                    if chunk.done || chunk.next_offset <= offset {
                        break;
                    }
                    offset = chunk.next_offset;
                }
                save(&name, &bytes)
            }
            .await;
            this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.path == path) {
                    tab.downloading = None;
                }
                if let Err(error) = result {
                    this.notice = Some(error.to_string());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn run_search(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim().to_owned();
        // Submitting takes the text out of the field; keep it there.
        self.search
            .update(cx, |search, cx| search.set_text(query.clone(), cx));
        let Some(target) = self.target.clone() else {
            return;
        };
        if query.is_empty() {
            self.found = None;
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let contents = self.contents;
        self._search = Some(cx.spawn(async move |this, cx| {
            let found = async {
                if contents {
                    let request = SearchWorkspaceContentRequest {
                        target: target.workspace(),
                        query,
                        limit: Some(200),
                        per_file_limit: Some(5),
                    };
                    let value = Self::call(
                        &client,
                        &target,
                        methods::SEARCH_WORKSPACE_CONTENT,
                        serde_json::to_value(&request)?,
                    )
                    .await?;
                    anyhow::Ok(Found::Contents(serde_json::from_value(value)?))
                } else {
                    let request = SearchWorkspaceFilesRequest {
                        target: target.workspace(),
                        query,
                        include_ignored: false,
                        limit: Some(200),
                    };
                    let value = Self::call(
                        &client,
                        &target,
                        methods::SEARCH_WORKSPACE_FILES,
                        serde_json::to_value(&request)?,
                    )
                    .await?;
                    anyhow::Ok(Found::Names(serde_json::from_value(value)?))
                }
            }
            .await;
            this.update(cx, |this, cx| {
                match found {
                    Ok(found) => this.found = Some(found),
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    // -----------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------

    fn render_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = |id: SharedString, label: String, active: bool| {
            div()
                .id(id)
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .px(px(10.))
                .rounded(px(8.))
                .text_size(px(13.))
                .cursor_pointer()
                .when(active, |tab| {
                    tab.bg(theme::element_active()).text_color(theme::text())
                })
                .when(!active, |tab| {
                    tab.text_color(theme::text_muted())
                        .hover(|tab| tab.bg(theme::element_hover()))
                })
                .child(label)
        };
        let mut strip = div()
            .id("file-tabs")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .px(px(8.))
            .py(px(6.))
            .border_b_1()
            .border_color(theme::border())
            .overflow_x_scroll()
            .child(
                tab("tab-files".into(), "Files".into(), self.active.is_none()).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.active = None;
                        cx.notify();
                    }),
                ),
            );
        for (at, open) in self.tabs.iter().enumerate() {
            strip = strip.child(
                tab(
                    SharedString::from(format!("tab-{at}")),
                    truncate(&open.name, 28),
                    self.active == Some(at),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.active = Some(at);
                    cx.notify();
                }))
                .child(
                    div()
                        .id(SharedString::from(format!("tab-close-{at}")))
                        .px(px(4.))
                        .rounded(px(4.))
                        .text_color(theme::text_faint())
                        .hover(|x| x.text_color(theme::text()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.close(at, cx);
                        }))
                        .child("×"),
                ),
            );
        }
        strip
            .child(div().flex_1())
            .child(
                div()
                    .id("panel-close")
                    .flex_none()
                    .px(px(8.))
                    .text_size(px(13.))
                    .text_color(theme::text_muted())
                    .cursor_pointer()
                    .hover(|x| x.text_color(theme::text()))
                    .on_click(|_: &ClickEvent, _, _| signal("oa-files-close"))
                    .child("Close"),
            )
            .into_any_element()
    }

    fn render_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.target.is_none() {
            return div()
                .p(px(16.))
                .child(footer_text(
                    "Open a chat on one of your computers to see its files.",
                ))
                .into_any_element();
        }
        let search = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme::border())
                    .child(self.search.clone()),
            )
            .child(
                button(
                    "search-contents",
                    if self.contents { "Contents" } else { "Names" },
                    false,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.contents = !this.contents;
                    let query = this.search.read(cx).text();
                    if !query.trim().is_empty() {
                        this.run_search(query, cx);
                    }
                    cx.notify();
                })),
            )
            .child(
                button(
                    "show-hidden",
                    if self.hidden {
                        "Hide dot files"
                    } else {
                        "Dot files"
                    },
                    false,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.hidden = !this.hidden;
                    cx.notify();
                })),
            );
        let body: AnyElement = match &self.found {
            Some(found) => self.render_found(found, cx),
            None => {
                let rows = model::rows(&self.listed, &self.expanded, self.hidden);
                if rows.is_empty() {
                    let line = if self.loading.contains("") {
                        "Reading the files…"
                    } else {
                        "No files here."
                    };
                    div().p(px(16.)).child(footer_text(line)).into_any_element()
                } else {
                    let entity = cx.entity();
                    uniform_list("tree", rows.len(), move |range, _window, app| {
                        entity.update(app, |this, cx| {
                            range
                                .filter_map(|ix| {
                                    rows.get(ix).map(|row| this.render_row(ix, row, cx))
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .flex_1()
                    .into_any_element()
                }
            }
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(search)
            .child(body)
            .into_any_element()
    }

    fn render_row(&self, ix: usize, row: &Row, cx: &mut Context<Self>) -> AnyElement {
        let badge = self.git.get(&row.path).copied();
        let clicked = row.clone();
        div()
            .id(("row", ix))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .h(px(ROW))
            .pl(px(10. + row.depth as f32 * 14.))
            .pr(px(10.))
            .text_size(px(13.5))
            .cursor_pointer()
            .hover(|x| x.bg(theme::element_hover()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(&clicked, cx)))
            .child(
                div()
                    .flex_none()
                    .w(px(12.))
                    .text_color(theme::text_faint())
                    .child(if !row.dir {
                        ""
                    } else if row.open {
                        "▾"
                    } else {
                        "▸"
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_color(if badge.is_some() {
                        theme::warning()
                    } else {
                        theme::text()
                    })
                    .child(row.name.clone()),
            )
            .children(badge.map(|badge| {
                div()
                    .flex_none()
                    .text_size(px(11.5))
                    .text_color(theme::warning())
                    .child(badge.to_string())
            }))
            .into_any_element()
    }

    fn render_found(&self, found: &Found, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div()
            .id("found")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll();
        let row = |id: (&'static str, usize), path: String, line: String| {
            div()
                .id(id)
                .flex()
                .flex_col()
                .px(px(12.))
                .py(px(5.))
                .cursor_pointer()
                .hover(|x| x.bg(theme::element_hover()))
                .child(div().text_size(px(13.5)).child(path))
                .when(!line.is_empty(), |row| {
                    row.child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme::text_faint())
                            .child(line),
                    )
                })
        };
        match found {
            Found::Names(matches) if matches.is_empty() => {
                return div()
                    .p(px(16.))
                    .child(footer_text("No matching files."))
                    .into_any_element();
            }
            Found::Contents(result) if result.matches.is_empty() => {
                let line = if result.indexing {
                    "No matches yet; the computer is still reading its files."
                } else {
                    "No matches."
                };
                return div().p(px(16.)).child(footer_text(line)).into_any_element();
            }
            Found::Names(matches) => {
                for (ix, found) in matches.iter().enumerate() {
                    let path = found.path.clone();
                    let dir = found.kind == openagents_proto::WorkspaceEntryKind::Directory;
                    list = list.child(
                        row(("found-name", ix), found.path.clone(), String::new()).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if dir {
                                    this.reveal(&path, cx);
                                } else {
                                    this.open(path.clone(), None, cx);
                                }
                            }),
                        ),
                    );
                }
            }
            Found::Contents(result) => {
                for (ix, found) in result.matches.iter().enumerate() {
                    let path = found.path.clone();
                    list = list.child(
                        row(
                            ("found-line", ix),
                            format!("{}:{}", found.path, found.line),
                            truncate(&found.preview, 120),
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| this.open(path.clone(), None, cx),
                        )),
                    );
                }
                if result.truncated {
                    list = list.child(div().p(px(12.)).child(footer_text(
                        "More matches on the computer; narrow the search.",
                    )));
                }
            }
        }
        list.into_any_element()
    }

    /// Show a folder in the tree, opening each folder above it.
    fn reveal(&mut self, path: &str, cx: &mut Context<Self>) {
        let mut dir = String::new();
        for part in path.trim_end_matches('/').split('/') {
            if !dir.is_empty() {
                dir.push('/');
            }
            dir.push_str(part);
            self.expanded.insert(dir.clone());
            if !self.listed.contains_key(&dir) {
                self.list(&dir, cx);
            }
        }
        self.found = None;
        self.search.update(cx, |search, cx| {
            search.take(cx);
        });
        cx.notify();
    }

    fn render_file(&self, at: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.tabs.get(at) else {
            return div().into_any_element();
        };
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(px(12.5))
                    .text_color(theme::text_muted())
                    .child(tab.path.clone()),
            )
            .when(
                matches!(&tab.body, Body::Text { markdown: true, .. }),
                |header| {
                    let rendered = matches!(&tab.body, Body::Text { rendered: true, .. });
                    header.child(
                        button(
                            "markdown-source",
                            if rendered { "Source" } else { "Preview" },
                            false,
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                if let Some(Tab {
                                    body: Body::Text { rendered, .. },
                                    ..
                                }) = this.tabs.get_mut(at)
                                {
                                    *rendered = !*rendered;
                                    cx.notify();
                                }
                            },
                        )),
                    )
                },
            )
            .child(self.download_button(at, tab, cx));
        let body: AnyElement = match &tab.body {
            Body::Loading => div()
                .p(px(16.))
                .child(footer_text("Reading…"))
                .into_any_element(),
            Body::Failed(error) => div()
                .p(px(16.))
                .child(footer_text(error.clone()))
                .into_any_element(),
            Body::Image(image) => div()
                .id("image")
                .flex_1()
                .min_h_0()
                .p(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .child(img(image.clone()).max_w_full().max_h_full())
                .into_any_element(),
            Body::Card { size } => self.render_card(at, tab, *size, cx),
            Body::Text {
                rendered: true,
                markdown: true,
                file,
                ..
            } => div()
                .id("markdown")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(16.))
                .child(
                    Markdown {
                        id: SharedString::from(format!("file-{at}")),
                    }
                    .render(
                        file.text.as_deref().unwrap_or_default(),
                        false,
                        None,
                    ),
                )
                .into_any_element(),
            Body::Text { lines, file, .. } => {
                let lines = lines.clone();
                let truncated = file.truncated;
                let digits = lines.len().to_string().len();
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list("lines", lines.len(), move |range, _, _| {
                            range
                                .map(|ix| {
                                    div()
                                        .flex()
                                        .flex_row()
                                        .h(px(LINE))
                                        .px(px(12.))
                                        .gap(px(12.))
                                        .font_family(theme::MONO)
                                        .text_size(px(12.5))
                                        .child(
                                            div()
                                                .flex_none()
                                                .w(px(8. * digits as f32 + 4.))
                                                .text_color(theme::text_faint())
                                                .child(format!("{}", ix + 1)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .whitespace_nowrap()
                                                .child(lines[ix].clone()),
                                        )
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        })
                        .flex_1(),
                    )
                    .when(truncated, |view| {
                        view.child(div().px(px(12.)).py(px(6.)).child(footer_text(
                            "This file is long; only its start shows here. Download it for the rest.",
                        )))
                    })
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn download_button(&self, at: usize, tab: &Tab, cx: &mut Context<Self>) -> AnyElement {
        match tab.downloading {
            Some(got) => {
                footer_text(format!("Downloading… {}", model::size(got))).into_any_element()
            }
            None => button(
                SharedString::from(format!("download-{at}")),
                "Download",
                false,
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.download(at, cx)))
            .into_any_element(),
        }
    }

    fn render_card(
        &self,
        at: usize,
        tab: &Tab,
        size: Option<u64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .p(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .p(px(20.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::surface_card())
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(tab.name.clone()),
                    )
                    .child(footer_text(match size {
                        Some(size) => format!("{} · no preview", model::size(size)),
                        None => "No preview".to_owned(),
                    }))
                    .child(self.download_button(at, tab, cx)),
            )
            .into_any_element()
    }
}

impl Render for FilesApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = self.notice.clone().map(|line| {
            div()
                .id("notice")
                .mx(px(10.))
                .mt(px(8.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(8.))
                .bg(theme::danger().opacity(0.08))
                .text_size(px(12.5))
                .text_color(theme::danger())
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.notice = None;
                    cx.notify();
                }))
                .child(line)
        });
        let body = match self.active {
            Some(at) => self.render_file(at, cx),
            None => self.render_tree(cx),
        };
        let mut root = div()
            .id("openagents-web-files")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .border_l_1()
            .border_color(theme::border())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY))
            .child(self.render_tabs(cx))
            .children(notice)
            .child(body);
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root
    }
}

async fn read_text(
    client: &Client,
    target: &Target,
    path: &str,
    markdown: bool,
) -> anyhow::Result<Body> {
    let request = ReadWorkspaceFileRequest {
        target: target.workspace(),
        path: path.to_owned(),
    };
    let value = FilesApp::call(
        client,
        target,
        methods::READ_WORKSPACE_FILE,
        serde_json::to_value(&request)?,
    )
    .await?;
    let file: WorkspaceFileText = serde_json::from_value(value)?;
    if !model::has_text(&file) {
        return Ok(Body::Card {
            size: Some(file.size),
        });
    }
    let lines = Arc::new(
        file.text
            .as_deref()
            .unwrap_or_default()
            .split('\n')
            .map(|line| SharedString::from(line.trim_end_matches('\r').replace('\t', "    ")))
            .collect(),
    );
    Ok(Body::Text {
        file,
        lines,
        rendered: markdown,
        markdown,
    })
}

async fn read_image(
    client: &Client,
    target: &Target,
    path: &str,
    checkout: &str,
) -> anyhow::Result<Body> {
    let computer = computer::connect(client, &target.device, || {}).await?;
    let mut bytes = Vec::new();
    let mut offset = 0usize;
    let mut hash = None;
    let mut mime: String;
    loop {
        let request = ReadWorkspaceImageRequest {
            target: target.workspace(),
            path: path.to_owned(),
            expected_checkout_id: checkout.to_owned(),
            offset,
            expected_content_hash: hash.clone(),
        };
        let chunk: WorkspaceImageChunk = match computer
            .call(
                methods::READ_WORKSPACE_IMAGE,
                serde_json::to_value(&request)?,
            )
            .await
            .and_then(|v| Ok(serde_json::from_value(v)?))
        {
            Ok(chunk) => chunk,
            // Too large or not an image the computer reads: the card.
            Err(_) if offset == 0 => return Ok(Body::Card { size: None }),
            Err(error) => return Err(error),
        };
        bytes.extend(base64::engine::general_purpose::STANDARD.decode(&chunk.data)?);
        hash.get_or_insert(chunk.content_hash);
        mime = chunk.mime_type;
        if chunk.done || chunk.next_offset <= offset {
            break;
        }
        offset = chunk.next_offset;
    }
    let format = match mime.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/bmp" => ImageFormat::Bmp,
        "image/svg+xml" => ImageFormat::Svg,
        _ => ImageFormat::Png,
    };
    Ok(Body::Image(Arc::new(Image::from_bytes(format, bytes))))
}

/// Hand `bytes` to the browser as a download named `name`.
fn save(name: &str, bytes: &[u8]) -> anyhow::Result<()> {
    use wasm_bindgen::JsCast;
    let fail =
        |error: wasm_bindgen::JsValue| anyhow::anyhow!("The download didn't start: {error:?}");
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(fail)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| anyhow::anyhow!("no document"))?;
    let link: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(fail)?
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("no link"))?;
    link.set_href(&url);
    link.set_download(name);
    link.click();
    // Let the browser start the download before the address goes.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    if let Some(window) = web_sys::window() {
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 60_000);
    }
    Ok(())
}

/// Tell the page something (`oa-files-close`).
pub fn signal(name: &str) {
    if let Some(window) = web_sys::window()
        && let Ok(event) = web_sys::Event::new(name)
    {
        let _ = window.dispatch_event(&event);
    }
}
