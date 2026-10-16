//! Home state, settings, and source reads shared by desktop and phone shells.
use crate::Lease;
use openagents::dashboard::{self as api, *};
use openagents::{CancellationToken, Error};
use openagents_dashboard::view::{self as ui, Action};
use rust_native::{Activation, FieldChange, Node, View};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::watch;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    #[default]
    None,
    Session {
        id: String,
    },
    Url {
        url: String,
    },
    Export {
        name: String,
        contents: String,
    },
    Import,
}
#[derive(Clone, Serialize, Deserialize)]
enum Pending {
    Publish {
        id: String,
        request: api::Request<api::Publish>,
    },
    Remove {
        id: String,
        request: api::Request<api::Remove>,
    },
    Report {
        id: String,
        request: api::Request<api::Report>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Home,
    Edit,
    Configure,
    Directory,
    Preview,
    ConfirmDelete,
    Issues,
    Overview,
}
struct State {
    library: Library,
    selected: String,
    draft: Option<Dashboard>,
    configured: Option<String>,
    fields: BTreeMap<String, String>,
    screen: Screen,
    projects: Vec<Project>,
    local_projects: Vec<Project>,
    work: Vec<Work>,
    local_work: Vec<Work>,
    computers: Vec<Computer>,
    issues: BTreeMap<String, Result<IssueSummary, String>>,
    listings: Vec<Listing>,
    preview: Option<Listing>,
    update_target: Option<(Dashboard, Package)>,
    publishing: Option<(String, u64)>,
    issue_window: usize,
    work_offset: usize,
    remote_next: Option<String>,
    next: Option<String>,
    notice: Option<String>,
    revision: u64,
    navigation: u64,
    clock: u64,
    initialized: bool,
    last_issues: Option<Instant>,
    issue_resume: Option<Instant>,
    issue_failures: u32,
    work_error: Option<String>,
    undo: Option<(usize, Widget)>,
    library_pending: Option<api::Request<api::SaveLibrary>>,
}
pub struct Home {
    lease: Option<Lease>,
    path: PathBuf,
    state: Mutex<State>,
    updates: watch::Sender<u64>,
    busy: AtomicBool,
    visible: AtomicBool,
    foreground: AtomicBool,
    cleared: AtomicBool,
    remote_checked: AtomicBool,
    cancel: CancellationToken,
    instance: String,
    io: tokio::sync::Mutex<()>,
}
fn fresh_id() -> String {
    openagents::agents::request_id().expect("OS randomness")
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn storage_error() -> String {
    "Couldn't save this dashboard. Try again.".into()
}
impl Home {
    pub fn new(lease: Option<Lease>, path: PathBuf) -> Arc<Self> {
        let library = std::fs::read(&path)
            .ok()
            .filter(|v| v.len() <= 4 * 1024 * 1024)
            .and_then(|v| serde_json::from_slice::<Library>(&v).ok())
            .filter(|v| v.validate().is_ok())
            .unwrap_or_default();
        let mut library = library;
        if library.dashboards.is_empty() {
            let d = Dashboard::starter(fresh_id());
            library.default = Some(d.id.clone());
            library.dashboards.push(d);
        }
        let selected = library
            .default
            .clone()
            .unwrap_or_else(|| library.dashboards[0].id.clone());
        let library_pending = std::fs::read(path.with_extension("pending.json"))
            .ok()
            .filter(|v| v.len() <= 4 * 1024 * 1024)
            .and_then(|v| serde_json::from_slice(&v).ok());
        let (updates, _) = watch::channel(1);
        Arc::new(Self {
            lease,
            path,
            state: Mutex::new(State {
                library,
                selected,
                draft: None,
                configured: None,
                fields: BTreeMap::new(),
                screen: Screen::Home,
                projects: vec![],
                local_projects: vec![],
                work: vec![],
                local_work: vec![],
                computers: vec![],
                issues: BTreeMap::new(),
                listings: vec![],
                preview: None,
                update_target: None,
                publishing: None,
                issue_window: 0,
                work_offset: 0,
                remote_next: None,
                next: None,
                notice: None,
                revision: 1,
                navigation: 0,
                clock: now(),
                initialized: false,
                last_issues: None,
                issue_resume: None,
                issue_failures: 0,
                work_error: None,
                undo: None,
                library_pending,
            }),
            updates,
            busy: AtomicBool::new(false),
            visible: AtomicBool::new(false),
            foreground: AtomicBool::new(true),
            cleared: AtomicBool::new(false),
            remote_checked: AtomicBool::new(false),
            cancel: CancellationToken::new(),
            instance: fresh_id(),
            io: tokio::sync::Mutex::new(()),
        })
    }
    pub fn watch(&self) -> watch::Receiver<u64> {
        self.updates.subscribe()
    }
    pub fn start(this: &Arc<Self>) {
        let weak = Arc::downgrade(this);
        let cancel = this.cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {_ = cancel.cancelled()=>break,_=tokio::time::sleep(Duration::from_secs(15))=>{}}
                let Some(this) = weak.upgrade() else { break };
                if !this.current() {
                    this.clear();
                    break;
                }
                if this.visible.load(Ordering::Relaxed) && this.foreground.load(Ordering::Relaxed) {
                    this.refresh(false).await;
                }
            }
        });
    }
    fn current(&self) -> bool {
        !self.cancel.is_cancelled() && self.lease.as_ref().is_none_or(Lease::is_current)
    }
    fn changed(&self, s: &mut State) {
        s.revision += 1;
        s.clock = now();
        self.updates.send_replace(s.revision);
    }
    fn clear(&self) {
        if self.cleared.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut s = self.state.lock().unwrap();
        s.library = Library {
            dashboards: vec![Dashboard::starter("signed-out")],
            default: Some("signed-out".into()),
            revision: 0,
        };
        s.selected = "signed-out".into();
        s.draft = None;
        s.fields.clear();
        s.local_projects.clear();
        s.screen = Screen::Home;
        s.work.clear();
        s.local_work.clear();
        s.computers.clear();
        s.projects.clear();
        s.issues.clear();
        s.listings.clear();
        s.preview = None;
        s.update_target = None;
        s.publishing = None;
        s.notice = Some("Sign in again to see your work.".into());
        self.changed(&mut s);
    }
    pub fn shutdown(&self) {
        self.cancel.cancel();
        self.clear();
    }
    pub fn set_visible(&self, visible: bool, foreground: bool) {
        self.visible.store(visible, Ordering::Relaxed);
        self.foreground.store(foreground, Ordering::Relaxed);
    }
    pub fn set_live(&self, mut work: Vec<Work>, computers: Vec<Computer>) {
        if !self.current() {
            return;
        }
        let mut s = self.state.lock().unwrap();
        for row in &mut work {
            row.title = row
                .title
                .chars()
                .filter(|c| !c.is_control())
                .take(240)
                .collect();
            row.agent = row
                .agent
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect();
            row.host = row
                .host
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect();
            if !row.online {
                if let Some(old) = s
                    .local_work
                    .iter()
                    .find(|w| w.id == row.id && w.observed_at >= row.observed_at)
                {
                    row.state = old.state.clone();
                    row.observed_at = old.observed_at;
                }
            }
        }
        if s.local_work != work || s.computers != computers {
            s.local_work = work;
            s.computers = computers;
            if matches!(s.screen, Screen::Home | Screen::Overview) {
                self.changed(&mut s);
            }
        }
    }
    pub fn set_local_projects(&self, projects: Vec<Project>) {
        if !self.current() {
            return;
        }
        let mut s = self.state.lock().unwrap();
        if s.local_projects != projects {
            s.local_projects = projects;
            if matches!(s.screen, Screen::Home | Screen::Overview) {
                self.changed(&mut s);
            }
        }
    }
    fn save_local(&self, s: &State) -> Result<(), String> {
        s.library.validate().map_err(str::to_owned)?;
        let bytes = serde_json::to_vec(&s.library).map_err(|_| storage_error())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("Remove a dashboard before adding another.".into());
        }
        let parent = self.path.parent().ok_or_else(storage_error)?;
        std::fs::create_dir_all(parent).map_err(|_| storage_error())?;
        let temp = self.path.with_extension(format!("{}.new", self.instance));
        std::fs::write(&temp, bytes).map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        std::fs::rename(temp, &self.path).map_err(|_| storage_error())
    }
    fn selected<'a>(s: &'a State) -> &'a Dashboard {
        s.library
            .dashboards
            .iter()
            .find(|d| d.id == s.selected)
            .unwrap_or(&s.library.dashboards[0])
    }
    pub fn view(&self) -> View<Action> {
        if !self.current() {
            self.clear();
        }
        let s = self.state.lock().unwrap();
        self.build(&s)
    }
    fn build(&self, s: &State) -> View<Action> {
        let mut nodes = vec![];
        if let Some(notice) = &s.notice {
            nodes.push(ui::text("notice", notice, false));
        }
        match s.screen {
            Screen::Home => {
                let d = Self::selected(s);
                nodes.push(ui::text("dashboard-name", &d.name, true));
                nodes.push(ui::stack(
                    "home-actions",
                    vec![
                        ui::button("edit", "Edit Home", "edit"),
                        ui::button("refresh", "Refresh", "refresh"),
                        ui::button("layouts", "Layouts", "directory"),
                        ui::button("import", "Import layout", "import"),
                    ],
                    true,
                ));
                let choices = s
                    .library
                    .dashboards
                    .iter()
                    .map(|d| {
                        ui::button(
                            format!("switch-{}", d.id),
                            &d.name,
                            format!("switch:{}", d.id),
                        )
                    })
                    .collect();
                if s.library.dashboards.len() > 1 {
                    nodes.push(ui::stack("dashboards", choices, true));
                }
                nodes.push(ui::button("scope", "Change project", "scope"));
                if s.library.default.as_ref() != Some(&d.id) {
                    nodes.push(ui::button("default", "Make default", "default"));
                }
                if d.source.is_some() {
                    nodes.push(ui::button(
                        "check-template-update",
                        "Check for layout updates",
                        "check-update",
                    ));
                }
                let mut compact: Option<Node<Action>> = None;
                for w in &d.widgets {
                    let card = self.widget(s, d, w);
                    if w.size == Size::Compact {
                        if let Some(first) = compact.take() {
                            let mut row =
                                ui::stack(format!("row-{}", w.id), vec![first, card], true);
                            if let rust_native::Element::Stack { axis, .. } = &mut row.element {
                                *axis = rust_native::Axis::Horizontal;
                            }
                            nodes.push(row);
                        } else {
                            compact = Some(card);
                        }
                    } else {
                        if let Some(first) = compact.take() {
                            nodes.push(first);
                        }
                        nodes.push(card);
                    }
                }
                if let Some(first) = compact {
                    nodes.push(first);
                }
                nodes.push(ui::button("new-dashboard", "New dashboard", "new"));
                if self.path.with_extension("remote.json").exists() {
                    nodes.push(ui::button(
                        "retry-remote",
                        "Finish saved change",
                        "retry-remote",
                    ));
                }
                if self.path.with_extension("before-reload.json").exists() {
                    nodes.push(ui::button(
                        "restore-layout-copies",
                        "Restore local layouts as copies",
                        "restore-layout-copies",
                    ));
                }
                if self.lease.is_some() {
                    nodes.push(ui::stack(
                        "sync-actions",
                        vec![
                            ui::button("sync-layouts", "Save layouts to account", "sync"),
                            ui::button("reload-layouts", "Reload saved layouts", "reload-layouts"),
                        ],
                        true,
                    ));
                }
            }
            Screen::Edit => {
                let d = s.draft.as_ref().unwrap();
                nodes.push(ui::field("name", "Dashboard name", &d.name, false));
                nodes.push(ui::stack(
                    "edit-actions",
                    vec![
                        ui::button("done", "Done", "done"),
                        ui::button("cancel", "Cancel", "cancel"),
                        ui::button("duplicate", "Duplicate", "duplicate"),
                        ui::button("delete-dashboard", "Delete dashboard", "delete-dashboard"),
                        ui::button("export", "Export layout", "export"),
                        ui::button("publish", "List this layout", "publish-preview"),
                    ],
                    true,
                ));
                for (i, w) in d.widgets.iter().enumerate() {
                    nodes.push(ui::card(
                        format!("edit-{}", w.id),
                        vec![
                            ui::text(format!("title-{}", w.id), label(&w.kind), true),
                            ui::stack(
                                format!("controls-{}", w.id),
                                vec![
                                    ui::button(
                                        format!("up-{}", w.id),
                                        "Move up",
                                        format!("move:{}:-1", w.id),
                                    ),
                                    ui::button(
                                        format!("down-{}", w.id),
                                        "Move down",
                                        format!("move:{}:1", w.id),
                                    ),
                                    ui::button(
                                        format!("size-{}", w.id),
                                        if w.size == Size::Full {
                                            "Make compact"
                                        } else {
                                            "Make full width"
                                        },
                                        format!("size:{}", w.id),
                                    ),
                                    ui::button(
                                        format!("config-{}", w.id),
                                        "Settings",
                                        format!("configure:{}", w.id),
                                    ),
                                    ui::button(
                                        format!("remove-{}", w.id),
                                        "Remove",
                                        format!("remove:{}", w.id),
                                    ),
                                ],
                                true,
                            ),
                            ui::text(
                                format!("position-{}", w.id),
                                format!("{} of {}", i + 1, d.widgets.len()),
                                false,
                            ),
                        ],
                    ));
                }
                if s.undo.is_some() {
                    nodes.push(ui::button("undo", "Undo removal", "undo"));
                }
                nodes.push(ui::text("add-title", "Add widget", true));
                for (kind, name) in TYPES {
                    nodes.push(ui::button(
                        format!("add-{kind}"),
                        *name,
                        format!("add:{kind}"),
                    ));
                }
            }
            Screen::Configure => {
                nodes.push(ui::text("settings-title", "Project and filters", true));
                let kind = s
                    .configured
                    .as_ref()
                    .and_then(|id| {
                        s.draft
                            .as_ref()
                            .and_then(|d| d.widgets.iter().find(|w| &w.id == id))
                    })
                    .map_or("dashboard", |w| w.kind.as_str());
                let work_widget = matches!(kind, "agents" | "attention" | "recent");
                if kind != "computers" {
                    nodes.push(ui::button("all-projects", "All projects", "project:"));
                    let projects = if work_widget {
                        &s.local_projects
                    } else {
                        &s.projects
                    };
                    for p in projects.iter().take(200) {
                        nodes.push(ui::button(
                            format!("project-{}", p.id),
                            &p.name,
                            format!("project:{}", p.id),
                        ));
                    }
                    let chosen = s
                        .fields
                        .get("project")
                        .and_then(|id| projects.iter().find(|p| &p.id == id))
                        .map_or("All projects", |p| p.name.as_str());
                    nodes.push(ui::text("chosen-project", chosen, false));
                }
                let mut fields = vec![];
                if matches!(kind, "dashboard" | "issues" | "my_issues") {
                    fields.extend([
                        (
                            "repositories",
                            "Repositories (owner/name, separated by commas)",
                        ),
                        ("board-owner", "GitHub board owner"),
                        ("board-number", "GitHub board number"),
                    ]);
                }
                if matches!(kind, "issues" | "my_issues") {
                    fields.push(("labels", "Issue labels, separated by commas"));
                    if kind == "issues" {
                        fields.push(("assignee", "Issue assignee (blank for all)"));
                    }
                }
                if work_widget {
                    fields.push(("agent", "Agent name (blank for all)"));
                }
                if work_widget || kind == "computers" {
                    fields.push(("host", "Computer name (blank for all)"));
                }
                if kind == "recent" {
                    fields.push(("window-hours", "Past hours (1 to 720)"));
                }
                for (key, label) in fields {
                    nodes.push(ui::field(
                        key,
                        label,
                        s.fields.get(key).cloned().unwrap_or_default(),
                        false,
                    ));
                }
                nodes.push(ui::stack(
                    "settings-actions",
                    vec![
                        ui::button("apply-settings", "Apply", "apply-settings"),
                        ui::button("cancel-settings", "Cancel", "back-settings"),
                    ],
                    true,
                ));
            }
            Screen::Directory => {
                nodes.push(ui::text("layouts-title", "Layouts", true));
                nodes.push(ui::field(
                    "layout-link",
                    "Shared layout link",
                    s.fields.get("layout-link").cloned().unwrap_or_default(),
                    false,
                ));
                nodes.push(ui::button(
                    "open-layout-link",
                    "Open shared layout",
                    "open-layout-link",
                ));
                nodes.push(ui::field(
                    "search",
                    "Search layouts",
                    s.fields.get("search").cloned().unwrap_or_default(),
                    false,
                ));
                nodes.push(ui::stack(
                    "directory-actions",
                    vec![
                        ui::button("search-layouts", "Search", "search"),
                        ui::button("my-layouts", "My layouts", "mine"),
                        ui::button("close-layouts", "Back to Home", "cancel"),
                    ],
                    true,
                ));
                if s.listings.is_empty() {
                    nodes.push(ui::text("no-layouts", "No layouts found.", false));
                }
                for listing in &s.listings {
                    nodes.push(ui::button(
                        format!("template-{}", listing.id),
                        format!("{} · {}", listing.package.title, listing.author),
                        format!("preview:{}", listing.id),
                    ));
                }
                if s.next.is_some() {
                    nodes.push(ui::button("more-layouts", "More layouts", "more-layouts"));
                }
            }
            Screen::Preview => {
                if let Some(l) = &s.preview {
                    nodes.push(ui::text("preview-title", &l.package.title, true));
                    nodes.push(ui::text(
                        "preview-description",
                        &l.package.description,
                        false,
                    ));
                    nodes.push(ui::text(
                        "preview-author",
                        format!("{} · Version {}", l.author, l.version),
                        false,
                    ));
                    for (i, w) in l.package.widgets.iter().enumerate() {
                        nodes.push(ui::text(
                            format!("preview-widget-{i}"),
                            format!(
                                "{} · {}",
                                label(&w.kind),
                                if w.supported() {
                                    "Supported"
                                } else {
                                    "Unavailable on this version"
                                }
                            ),
                            false,
                        ));
                    }
                    nodes.push(ui::text(
                        "preview-connections",
                        "Issue widgets use your GitHub connection and the project you choose.",
                        false,
                    ));
                    nodes.push(ui::button("install", "Use this layout", "install"));
                    nodes.push(ui::button(
                        "author-layouts",
                        "More by this author",
                        "author-layouts",
                    ));
                    if s.update_target.is_some() {
                        nodes.push(ui::text(
                            "update-note",
                            "Updates keep your settings and widget order.",
                            false,
                        ));
                        nodes.push(ui::button(
                            "apply-template-update",
                            "Apply update",
                            "apply-update",
                        ));
                    }
                    if l.owned {
                        nodes.push(ui::button(
                            "publish-template-update",
                            "Update listing from current dashboard",
                            "publish-update",
                        ));
                        nodes.push(ui::button("unpublish", "Remove listing", "unpublish"));
                    } else {
                        nodes.push(ui::button("report-layout", "Report layout", "report"));
                    }
                } else {
                    nodes.push(ui::text("publish-title", "List this layout", true));
                    nodes.push(ui::text("publish-details","The layout includes widget types and sizes. Other people choose their own projects and connections.",false));
                    nodes.push(ui::button(
                        "publish-private",
                        "Save privately",
                        "publish:private",
                    ));
                    nodes.push(ui::button(
                        "publish-unlisted",
                        "Create unlisted link",
                        "publish:unlisted",
                    ));
                    nodes.push(ui::button(
                        "publish-public",
                        "Publish publicly",
                        "publish:public",
                    ));
                }
                nodes.push(ui::button("close-preview", "Back", "directory"));
            }
            Screen::ConfirmDelete => {
                nodes.push(ui::text("delete-title", "Delete this dashboard?", true));
                nodes.push(ui::stack(
                    "delete-actions",
                    vec![
                        ui::button("confirm-delete", "Delete", "confirm-delete"),
                        ui::button("keep-dashboard", "Keep dashboard", "edit"),
                    ],
                    true,
                ));
            }
            Screen::Issues => {
                nodes.push(ui::button("close-issues", "Back to Home", "cancel"));
                let d = Self::selected(s);
                if let Some(w) = s
                    .configured
                    .as_ref()
                    .and_then(|id| d.widgets.iter().find(|w| &w.id == id))
                {
                    if let Some(Ok(summary)) = s.issues.get(&Self::issue_key(d, w)) {
                        nodes.push(ui::text(
                            "issue-total",
                            format!(
                                "{}{} matching open issues",
                                if summary.complete { "" } else { "At least " },
                                summary.total
                            ),
                            true,
                        ));
                        for (i, issue) in summary
                            .items
                            .iter()
                            .enumerate()
                            .skip(s.issue_window)
                            .take(100)
                        {
                            nodes.push(ui::button(
                                format!("issue-row-{i}"),
                                format!("{} #{} · {}", issue.repository, issue.number, issue.title),
                                format!("issue-link:{}:{}", issue.repository, issue.number),
                            ));
                        }
                        if s.issue_window > 0 {
                            nodes.push(ui::button(
                                "previous-issues",
                                "Previous issues",
                                "previous-issues",
                            ));
                        }
                        if s.issue_window + 100 < summary.items.len() {
                            nodes.push(ui::button(
                                "next-loaded-issues",
                                "Next issues",
                                "next-loaded-issues",
                            ));
                        } else if summary.next_page.is_some() {
                            nodes.push(ui::button("more-issues", "More issues", "more-issues"));
                        }
                        if summary.board_url.is_some() {
                            nodes.push(ui::button(
                                "open-board",
                                "Open board on GitHub",
                                "open-board",
                            ));
                        }
                        if !summary.complete {
                            nodes.push(ui::text(
                                "issue-coverage",
                                "Some issues could not be loaded.",
                                false,
                            ));
                        }
                    } else {
                        nodes.push(ui::text(
                            "issue-detail-error",
                            "Couldn't load these issues. Go back and try again.",
                            false,
                        ));
                    }
                }
            }
            Screen::Overview => {
                nodes.push(ui::button("close-overview", "Back to Home", "cancel"));
                let d = Self::selected(s);
                if let Some(w) = s
                    .configured
                    .as_ref()
                    .and_then(|id| d.widgets.iter().find(|w| &w.id == id))
                {
                    nodes.push(self.widget(s, d, w));
                }
            }
        }
        let mut view = ui::document(self.instance.clone(), s.revision, nodes);
        view.root.key = format!(
            "home-{:?}-{}",
            s.screen,
            s.configured.as_deref().unwrap_or("all")
        );
        view
    }
    fn widget(&self, s: &State, d: &Dashboard, w: &Widget) -> Node<Action> {
        let key = &w.id;
        let mut children = vec![ui::text(format!("heading-{key}"), label(&w.kind), true)];
        if !w.supported() {
            children.push(ui::text(
                format!("unknown-{key}"),
                "This widget needs a newer app version.",
                false,
            ));
            return ui::card(format!("card-{key}"), children);
        }
        if let Some(scope) = &w.scope {
            children.push(ui::text(
                format!("override-{key}"),
                if *scope == Scope::default() {
                    "All projects"
                } else {
                    "Uses its own project settings"
                },
                false,
            ));
        }
        match w.kind.as_str() {
            "issues" | "my_issues" => {
                let scope = w.scope.as_ref().unwrap_or(&d.scope);
                if scope.project.is_none() && scope.repositories.is_empty() {
                    children.push(ui::button(
                        format!("choose-{key}"),
                        "Choose a project",
                        format!("configure:{key}"),
                    ));
                } else {
                    match s.issues.get(&Self::issue_key(d, w)) {
                        Some(Ok(summary)) => {
                            children.push(ui::text(
                                format!("count-{key}"),
                                if summary.complete {
                                    summary.total.to_string()
                                } else {
                                    format!("At least {}", summary.total)
                                },
                                true,
                            ));
                            children.push(ui::text(
                                format!("age-{key}"),
                                format!(
                                    "Updated {}m ago",
                                    s.clock.saturating_sub(summary.observed_at) / 60
                                ),
                                false,
                            ));
                            children.push(ui::button(
                                format!("see-issues-{key}"),
                                "See issues",
                                format!("issues:{key}"),
                            ));
                        }
                        Some(Err(message)) => {
                            children.push(ui::text(format!("error-{key}"), message, false));
                            children.push(ui::button(
                                format!("retry-{key}"),
                                "Try again",
                                "refresh",
                            ));
                            children.push(ui::button(
                                format!("connect-{key}"),
                                "Connect GitHub",
                                "github",
                            ));
                        }
                        None => children.push(ui::text(
                            format!("load-{key}"),
                            if s.issue_resume.is_some() {
                                "Issue counts unavailable. Try refreshing later."
                            } else {
                                "Loading issues…"
                            },
                            false,
                        )),
                    }
                }
            }
            "agents" | "attention" | "recent" => {
                let project = w.scope.as_ref().unwrap_or(&d.scope).project.as_deref();
                let rows = work_rows(
                    s.local_work.iter().chain(&s.work).cloned().collect(),
                    project,
                );
                let rows: Vec<_> = rows
                    .into_iter()
                    .filter(|r| {
                        w.agent
                            .as_ref()
                            .is_none_or(|a| r.agent.eq_ignore_ascii_case(a))
                            && w.host
                                .as_ref()
                                .is_none_or(|h| r.host.eq_ignore_ascii_case(h))
                    })
                    .filter(|row| match w.kind.as_str() {
                        "attention" => row.priority() == 0,
                        "recent" => {
                            matches!(row.state_label(), "Finished" | "Failed" | "Stopped")
                                && s.clock.saturating_sub(row.observed_at)
                                    <= u64::from(w.window_hours.unwrap_or(24)) * 3600
                        }
                        _ => true,
                    })
                    .collect();
                let working = rows
                    .iter()
                    .filter(|r| r.available(s.clock) && r.state_label() == "Working")
                    .count();
                let attention = rows
                    .iter()
                    .filter(|r| r.available(s.clock) && r.state_label() == "Needs you")
                    .count();
                let offline = rows.iter().filter(|r| !r.available(s.clock)).count();
                children.push(ui::text(
                    format!("summary-{key}"),
                    if rows.is_empty() {
                        if let Some(error) = &s.work_error {
                            error.clone()
                        } else if !s.initialized && self.lease.is_some() {
                            "Loading work…".into()
                        } else {
                            "No active work".into()
                        }
                    } else {
                        format!("{working} working · {attention} needs you · {offline} unavailable")
                    },
                    false,
                ));
                let limit = if s.screen == Screen::Overview { 50 } else { 12 };
                let offset = if s.screen == Screen::Overview {
                    s.work_offset
                } else {
                    0
                };
                for (i, row) in rows.iter().enumerate().skip(offset).take(limit) {
                    let status = if row.available(s.clock) {
                        row.state_label().to_owned()
                    } else {
                        format!(
                            "Last seen {} · {}m ago",
                            row.state_label().to_lowercase(),
                            s.clock.saturating_sub(row.observed_at) / 60
                        )
                    };
                    let title = format!("{} · {}\n{} · {}", row.agent, row.title, status, row.host);
                    if let Some(target) = row
                        .destination
                        .as_ref()
                        .filter(|t| !matches!(t, WorkTarget::Unknown))
                    {
                        children.push(ui::button(
                            format!("work-{key}-{i}"),
                            title,
                            format!(
                                "work-target:{}",
                                serde_json::to_string(target).unwrap_or_default()
                            ),
                        ));
                    } else if let Some(session) =
                        row.session.as_ref().filter(|_| row.destination.is_none())
                    {
                        children.push(ui::button(
                            format!("work-{key}-{i}"),
                            title,
                            format!("session:{session}"),
                        ));
                    } else {
                        children.push(ui::text(format!("work-{key}-{i}"), title, false));
                    }
                }
                if s.screen == Screen::Overview && offset > 0 {
                    children.push(ui::button(
                        format!("previous-work-{key}"),
                        "Previous work",
                        "previous-work",
                    ));
                }
                if rows.len() > offset + limit {
                    children.push(ui::button(
                        format!("all-work-{key}"),
                        if s.screen == Screen::Overview {
                            "More work"
                        } else {
                            "See all"
                        },
                        format!("all-work:{key}"),
                    ));
                }
                if s.screen == Screen::Overview && s.remote_next.is_some() {
                    children.push(ui::button(
                        format!("more-account-work-{key}"),
                        "Load more account work",
                        "more-account-work",
                    ));
                }
                if s.remote_next.is_some() {
                    children.push(ui::text(
                        format!("work-coverage-{key}"),
                        "More account work is available.",
                        false,
                    ));
                }
            }
            "computers" => {
                for (i, c) in s
                    .computers
                    .iter()
                    .filter(|c| {
                        w.host
                            .as_ref()
                            .is_none_or(|h| c.name.eq_ignore_ascii_case(h))
                    })
                    .take(12)
                    .enumerate()
                {
                    children.push(ui::text(
                        format!("computer-{key}-{i}"),
                        format!(
                            "{} · {}",
                            c.name,
                            if c.online { "Online" } else { "Unavailable" }
                        ),
                        false,
                    ));
                }
                if s.computers.is_empty() {
                    children.push(ui::text(
                        format!("no-computers-{key}"),
                        "No connected computers",
                        false,
                    ));
                }
            }
            _ => {}
        }
        ui::card(format!("card-{key}"), children)
    }
    fn issue_key(d: &Dashboard, w: &Widget) -> String {
        serde_json::to_string(&(
            w.scope.as_ref().unwrap_or(&d.scope),
            w.kind == "my_issues",
            &w.labels,
            &w.assignee,
        ))
        .unwrap_or_default()
    }
    fn fields(s: &mut State, scope: &Scope, labels: &[String], assignee: Option<&str>) {
        for (key, value) in [
            ("project", scope.project.clone().unwrap_or_default()),
            ("repositories", scope.repositories.join(", ")),
            (
                "board-owner",
                scope
                    .board
                    .as_ref()
                    .map(|b| b.owner.clone())
                    .unwrap_or_default(),
            ),
            (
                "board-number",
                scope
                    .board
                    .as_ref()
                    .map(|b| b.number.to_string())
                    .unwrap_or_default(),
            ),
            ("labels", labels.join(", ")),
            ("assignee", assignee.unwrap_or_default().into()),
        ] {
            s.fields.insert(key.into(), value);
        }
    }
    fn scope(s: &State) -> Result<Scope, String> {
        let get = |key: &str| {
            s.fields
                .get(key)
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        let project = get("project");
        let owner = get("board-owner");
        let number = get("board-number");
        let scope = Scope {
            project: (!project.is_empty()).then_some(project),
            repositories: get("repositories")
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned)
                .collect(),
            board: if owner.is_empty() && number.is_empty() {
                None
            } else {
                Some(Board {
                    owner,
                    number: number
                        .parse()
                        .map_err(|_| "Enter the board number.".to_owned())?,
                })
            },
        };
        scope.validate().map_err(str::to_owned)?;
        Ok(scope)
    }
    pub async fn activate(
        &self,
        activation: Activation,
        value: Option<String>,
    ) -> Result<Effect, String> {
        if !self.current() {
            self.clear();
            return Err("Sign in again to see your work.".into());
        }
        let action = {
            let s = self.state.lock().unwrap();
            let view = self
                .build(&s)
                .validate()
                .map_err(|_| "Couldn't open Home.".to_owned())?;
            if let Some(value) = &value {
                view.change_field(&FieldChange {
                    activation,
                    value: value.clone(),
                })
                .map_err(|_| "This screen changed. Try again.".to_owned())?
                .0
                .clone()
            } else {
                view.activate(&activation)
                    .map_err(|_| "This screen changed. Try again.".to_owned())?
                    .0
                    .clone()
            }
        };
        if let Some(value) = value {
            let mut s = self.state.lock().unwrap();
            if action == "name" {
                if let Some(d) = &mut s.draft {
                    d.name = value;
                }
            } else {
                s.fields.insert(action, value);
            }
            self.changed(&mut s);
            return Ok(Effect::None);
        }
        let result = self.action(&action).await;
        if result.is_ok() && ["done", "default", "confirm-delete"].contains(&action.as_str()) {
            self.sync(false).await;
        }
        result
    }
    /// Reorder an edit draft using two actions from the same displayed view.
    pub fn reorder(&self, source: Activation, target: Activation) -> Result<(), String> {
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        let mut s = self.state.lock().unwrap();
        if s.screen != Screen::Edit {
            return Err("Open Edit to rearrange widgets.".into());
        }
        let view = self
            .build(&s)
            .validate()
            .map_err(|_| "Couldn't open Home.")?;
        let resolve = |activation: &Activation| -> Result<String, String> {
            let action = &view
                .activate(activation)
                .map_err(|_| "This screen changed. Try again.")?
                .0;
            action
                .strip_prefix("move:")
                .and_then(|v| v.strip_suffix(":-1"))
                .map(str::to_owned)
                .ok_or_else(|| "Choose a widget to move.".into())
        };
        let from = resolve(&source)?;
        let to = resolve(&target)?;
        let d = s.draft.as_mut().ok_or("Open Edit to rearrange widgets.")?;
        let a = d
            .widgets
            .iter()
            .position(|w| w.id == from)
            .ok_or("Widget unavailable.")?;
        let b = d
            .widgets
            .iter()
            .position(|w| w.id == to)
            .ok_or("Widget unavailable.")?;
        let widget = d.widgets.remove(a);
        d.widgets.insert(b, widget);
        self.changed(&mut s);
        Ok(())
    }
    /// Commit native field drafts and their submit action against one displayed view.
    pub async fn activate_form(
        &self,
        activation: Activation,
        fields: BTreeMap<String, String>,
    ) -> Result<Effect, String> {
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        let action = {
            let mut s = self.state.lock().unwrap();
            let view = self
                .build(&s)
                .validate()
                .map_err(|_| "Couldn't open Home.".to_owned())?;
            let action = view
                .activate(&activation)
                .map_err(|_| "This screen changed. Try again.".to_owned())?
                .0
                .clone();
            for (key, value) in &fields {
                view.change_field(&FieldChange {
                    activation: Activation {
                        node: key.clone(),
                        ..activation.clone()
                    },
                    value: value.clone(),
                })
                .map_err(|_| "Check the fields and try again.".to_owned())?;
            }
            for (key, value) in fields {
                if key == "name" {
                    if let Some(d) = &mut s.draft {
                        d.name = value;
                    }
                } else {
                    s.fields.insert(key, value);
                }
            }
            self.changed(&mut s);
            action
        };
        let result = self.action(&action).await;
        if result.is_ok() && ["done", "default", "confirm-delete"].contains(&action.as_str()) {
            self.sync(false).await;
        }
        result
    }
    pub async fn import(&self, bytes: &[u8]) -> Result<(), String> {
        let package = Package::decode(bytes).map_err(str::to_owned)?;
        let d = package
            .install(fresh_id(), Scope::default(), None)
            .map_err(str::to_owned)?;
        let mut s = self.state.lock().unwrap();
        s.draft = Some(d);
        s.screen = Screen::Configure;
        s.configured = None;
        Self::fields(&mut s, &Scope::default(), &[], None);
        self.changed(&mut s);
        Ok(())
    }
    pub async fn open_link(&self, link: &str) -> Result<(), String> {
        let id = link
            .trim()
            .strip_prefix("openagents://layout/")
            .filter(|id| openagents::agents::valid_id(id))
            .ok_or("This layout link isn't supported.")?;
        self.preview(id).await
    }
    async fn action(&self, action: &str) -> Result<Effect, String> {
        if action == "more-account-work" {
            self.more_work().await?;
            return Ok(Effect::None);
        }
        if action == "more-issues" {
            self.more_issues().await?;
            return Ok(Effect::None);
        }
        if action == "check-update" {
            self.check_update().await?;
            return Ok(Effect::None);
        }
        if action == "author-layouts" {
            let author = {
                let mut s = self.state.lock().unwrap();
                let author = s
                    .preview
                    .as_ref()
                    .ok_or("Choose a layout first.")?
                    .author_id
                    .clone();
                s.fields.insert("author".into(), author.clone());
                author
            };
            let _ = author;
            self.directory("author-layouts").await?;
            return Ok(Effect::None);
        }
        if action == "open-layout-link" {
            let link = self
                .state
                .lock()
                .unwrap()
                .fields
                .get("layout-link")
                .cloned()
                .unwrap_or_default();
            self.open_link(&link).await?;
            return Ok(Effect::None);
        }
        if action == "retry-remote" {
            return self.perform(None).await;
        }
        if action == "refresh" {
            self.refresh(true).await;
            return Ok(Effect::None);
        }
        if action == "sync" || action == "reload-layouts" {
            self.sync(action == "reload-layouts").await;
            return Ok(Effect::None);
        }
        if ["directory", "search", "mine", "more-layouts"].contains(&action) {
            self.directory(action).await?;
            return Ok(Effect::None);
        }
        if let Some(id) = action.strip_prefix("preview:") {
            self.preview(id).await?;
            return Ok(Effect::None);
        }
        if let Some(kind) = action.strip_prefix("publish:") {
            return self.publish(kind).await;
        }
        if action == "unpublish" || action == "report" {
            self.listing_action(action).await?;
            return Ok(Effect::None);
        }
        let mut s = self.state.lock().unwrap();
        s.navigation += 1;
        s.notice = None;
        if let Some(encoded) = action.strip_prefix("work-target:") {
            let target: WorkTarget =
                serde_json::from_str(encoded).map_err(|_| "This work is unavailable.")?;
            let path = match target {
                WorkTarget::Session { id } => return Ok(Effect::Session { id }),
                WorkTarget::Chat { id } if openagents_dashboard::id(&id) => format!("chat/{id}"),
                WorkTarget::Work { id } if openagents_dashboard::id(&id) => format!("work/{id}"),
                WorkTarget::MacJob { id } if openagents_dashboard::id(&id) => {
                    format!("settings/mac-jobs/{id}")
                }
                WorkTarget::AgentBoard => "settings/agents".into(),
                _ => return Err("This work is unavailable.".into()),
            };
            let website = self
                .lease
                .as_ref()
                .map(|l| l.client().deployment().website().clone())
                .unwrap_or_else(|| openagents::Deployment::production().website().clone());
            return Ok(Effect::Url {
                url: website
                    .join(&path)
                    .map_err(|_| "This work is unavailable.")?
                    .to_string(),
            });
        }
        if let Some(target) = action.strip_prefix("issue-link:") {
            let (repo, number) = target
                .rsplit_once(':')
                .ok_or("This issue is unavailable.")?;
            if !repository(repo) || number.parse::<u64>().ok().is_none_or(|n| n == 0) {
                return Err("This issue is unavailable.".into());
            }
            return Ok(Effect::Url {
                url: format!("https://github.com/{repo}/issues/{number}"),
            });
        }
        if action == "open-board" {
            let d = Self::selected(&s);
            let w = s
                .configured
                .as_ref()
                .and_then(|id| d.widgets.iter().find(|w| &w.id == id))
                .ok_or("Choose an issue widget first.")?;
            let summary = s
                .issues
                .get(&Self::issue_key(d, w))
                .and_then(|v| v.as_ref().ok())
                .ok_or("Refresh issues first.")?;
            let url = summary
                .board_url
                .as_ref()
                .ok_or("This board is unavailable.")?;
            let parsed = openagents::Deployment::new("https://github.com/v1", "https://github.com")
                .unwrap()
                .website()
                .join(url)
                .map_err(|_| "This board is unavailable.")?;
            if parsed.scheme() != "https"
                || parsed.host_str() != Some("github.com")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("This board link isn't supported.".into());
            }
            return Ok(Effect::Url {
                url: parsed.to_string(),
            });
        }
        if action == "import" {
            return Ok(Effect::Import);
        }
        if action == "github" {
            return Ok(Effect::Url {
                url: self
                    .lease
                    .as_ref()
                    .map(|l| {
                        l.client()
                            .deployment()
                            .website()
                            .join("settings")
                            .unwrap()
                            .to_string()
                    })
                    .unwrap_or_else(|| "https://openagents.com/settings".into()),
            });
        }
        if let Some(id) = action.strip_prefix("session:") {
            return Ok(Effect::Session { id: id.into() });
        }
        if let Some(key) = action.strip_prefix("issues:") {
            s.configured = Some(key.into());
            s.issue_window = 0;
            s.screen = Screen::Issues;
            self.changed(&mut s);
            return Ok(Effect::None);
        }
        match action {
            "restore-layout-copies" => {
                let path = self.path.with_extension("before-reload.json");
                let bytes = std::fs::read(&path).map_err(|_| storage_error())?;
                if bytes.len() > 4 * 1024 * 1024 {
                    return Err(storage_error());
                }
                let previous: Library =
                    serde_json::from_slice(&bytes).map_err(|_| storage_error())?;
                previous.validate().map_err(str::to_owned)?;
                let mut library = s.library.clone();
                for mut d in previous.dashboards {
                    d.id = fresh_id();
                    d.name = format!("{} copy", d.name.chars().take(60).collect::<String>());
                    library.dashboards.push(d);
                }
                library.validate().map_err(str::to_owned)?;
                s.library = library;
                self.save_local(&s)?;
                std::fs::remove_file(path).map_err(|_| storage_error())?;
            }
            "previous-issues" => s.issue_window = s.issue_window.saturating_sub(100),
            "previous-work" => s.work_offset = s.work_offset.saturating_sub(50),
            "next-loaded-issues" => s.issue_window += 100,
            "publish-update" => {
                let l = s.preview.clone().ok_or("Choose a layout first.")?;
                if !l.owned {
                    return Err("Choose one of your own layouts.".into());
                }
                s.publishing = Some((l.id, l.version));
                s.preview = None;
                s.screen = Screen::Preview;
            }
            "apply-update" => {
                let (local, previous) = s
                    .update_target
                    .clone()
                    .ok_or("Check for an update first.")?;
                let latest = s.preview.clone().ok_or("Check for an update first.")?;
                if !s.library.dashboards.iter().any(|d| d == &local) {
                    return Err("This dashboard changed. Check for an update again.".into());
                }
                let mut d = latest
                    .package
                    .upgrade(&previous, &local)
                    .map_err(str::to_owned)?;
                d.source = Some(TemplateRef {
                    id: latest.id,
                    version: latest.version,
                });
                s.draft = Some(d);
                s.update_target = None;
                s.screen = Screen::Edit;
            }
            "edit" => {
                s.draft = Some(Self::selected(&s).clone());
                s.screen = Screen::Edit;
            }
            "cancel" => {
                s.draft = None;
                s.preview = None;
                s.screen = Screen::Home;
            }
            "new" => {
                s.draft = Some(Dashboard::starter(fresh_id()));
                s.screen = Screen::Edit;
            }
            "duplicate" => {
                let mut d = s.draft.clone().unwrap();
                d.id = fresh_id();
                d.name = format!("{} copy", d.name.chars().take(60).collect::<String>());
                d.revision = 1;
                s.draft = Some(d);
            }
            "default" => {
                s.library.default = Some(s.selected.clone());
                self.save_local(&s)?;
            }
            "done" => {
                let d = s.draft.clone().ok_or("Open an editor first.")?;
                d.validate().map_err(str::to_owned)?;
                if let Some(existing) = s.library.dashboards.iter_mut().find(|v| v.id == d.id) {
                    *existing = d.clone();
                    existing.revision += 1;
                } else {
                    if s.library.dashboards.len() >= 32 {
                        return Err("Remove a dashboard before adding another.".into());
                    }
                    s.library.dashboards.push(d.clone());
                }
                s.selected = d.id;
                s.draft = None;
                s.screen = Screen::Home;
                s.issues.clear();
                s.last_issues = None;
                self.save_local(&s)?;
            }
            "delete-dashboard" => s.screen = Screen::ConfirmDelete,
            "confirm-delete" => {
                let chosen = s
                    .draft
                    .as_ref()
                    .map(|d| d.id.clone())
                    .unwrap_or_else(|| s.selected.clone());
                s.library.dashboards.retain(|d| d.id != chosen);
                if s.library.dashboards.is_empty() {
                    s.library.dashboards.push(Dashboard::starter(fresh_id()));
                }
                s.selected = s.library.dashboards[0].id.clone();
                if s.library.default.as_ref() == Some(&chosen) {
                    s.library.default = Some(s.selected.clone());
                }
                s.draft = None;
                s.screen = Screen::Home;
                self.save_local(&s)?;
            }
            "undo" => {
                if let Some((i, w)) = s.undo.take() {
                    let d = s.draft.as_mut().unwrap();
                    d.widgets.insert(i.min(d.widgets.len()), w);
                }
            }
            "scope" => {
                let d = Self::selected(&s).clone();
                s.draft = Some(d.clone());
                s.configured = None;
                Self::fields(&mut s, &d.scope, &[], None);
                s.screen = Screen::Configure;
            }
            "back-settings" => s.screen = Screen::Edit,
            "apply-settings" => {
                let scope = Self::scope(&s)?;
                let configured = s.configured.clone();
                let labels = s
                    .fields
                    .get("labels")
                    .cloned()
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_owned)
                    .collect();
                let assignee = s
                    .fields
                    .get("assignee")
                    .cloned()
                    .filter(|v| !v.trim().is_empty());
                let agent = s
                    .fields
                    .get("agent")
                    .cloned()
                    .filter(|v| !v.trim().is_empty());
                let host = s
                    .fields
                    .get("host")
                    .cloned()
                    .filter(|v| !v.trim().is_empty());
                let window = s
                    .fields
                    .get("window-hours")
                    .map(String::as_str)
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        v.parse::<u32>()
                            .map_err(|_| "Enter a number of hours from 1 to 720.".to_owned())
                    })
                    .transpose()?;
                let mut d = s.draft.clone().unwrap();
                if let Some(id) = configured {
                    let w = d
                        .widgets
                        .iter_mut()
                        .find(|w| w.id == id)
                        .ok_or("Widget unavailable.")?;
                    w.scope = Some(scope);
                    w.labels = labels;
                    w.assignee = assignee;
                    w.agent = agent;
                    w.host = host;
                    w.window_hours = window;
                } else {
                    d.scope = scope;
                }
                d.validate().map_err(str::to_owned)?;
                s.draft = Some(d);
                s.screen = Screen::Edit;
            }
            "export" => {
                let d = s.draft.as_ref().unwrap();
                let package = d.export(d.name.clone()).map_err(str::to_owned)?;
                return Ok(Effect::Export {
                    name: "OpenAgents-layout.json".into(),
                    contents: serde_json::to_string_pretty(&package)
                        .map_err(|_| storage_error())?,
                });
            }
            "publish-preview" => {
                s.publishing = None;
                s.preview = None;
                s.screen = Screen::Preview;
            }
            "install" => {
                let l = s.preview.clone().ok_or("Choose a layout first.")?;
                let d = l
                    .package
                    .install(
                        fresh_id(),
                        Scope::default(),
                        Some(TemplateRef {
                            id: l.id,
                            version: l.version,
                        }),
                    )
                    .map_err(str::to_owned)?;
                s.draft = Some(d);
                s.configured = None;
                Self::fields(&mut s, &Scope::default(), &[], None);
                s.screen = Screen::Configure;
            }
            _ => {
                if let Some(id) = action.strip_prefix("all-work:") {
                    s.configured = Some(id.into());
                    s.work_offset = if s.screen == Screen::Overview {
                        s.work_offset + 50
                    } else {
                        0
                    };
                    s.screen = Screen::Overview;
                } else if let Some(id) = action.strip_prefix("switch:") {
                    if s.library.dashboards.iter().any(|d| d.id == id) {
                        s.selected = id.into();
                        s.issues.clear();
                        s.last_issues = None;
                    }
                } else if let Some(kind) = action.strip_prefix("add:") {
                    let d = s.draft.as_mut().unwrap();
                    if d.widgets.len() >= MAX_WIDGETS {
                        return Err("Remove a widget before adding another.".into());
                    }
                    d.widgets.push(Widget::new(fresh_id(), kind));
                } else if let Some(id) = action.strip_prefix("remove:") {
                    let d = s.draft.as_mut().unwrap();
                    if let Some(i) = d.widgets.iter().position(|w| w.id == id) {
                        let w = d.widgets.remove(i);
                        s.undo = Some((i, w));
                    }
                } else if let Some(id) = action.strip_prefix("size:") {
                    if let Some(w) = s
                        .draft
                        .as_mut()
                        .unwrap()
                        .widgets
                        .iter_mut()
                        .find(|w| w.id == id)
                    {
                        w.size = if w.size == Size::Full {
                            Size::Compact
                        } else {
                            Size::Full
                        };
                    }
                } else if let Some(args) = action.strip_prefix("move:") {
                    if let Some((id, delta)) = args.rsplit_once(':') {
                        let d = s.draft.as_mut().unwrap();
                        if let Some(i) = d.widgets.iter().position(|w| w.id == id) {
                            let n = (i as i32 + delta.parse::<i32>().unwrap_or(0))
                                .clamp(0, d.widgets.len() as i32 - 1)
                                as usize;
                            d.widgets.swap(i, n);
                        }
                    }
                } else if let Some(id) = action.strip_prefix("configure:") {
                    if s.draft.is_none() {
                        s.draft = Some(Self::selected(&s).clone());
                    }
                    let d = s.draft.as_ref().unwrap();
                    let w = d
                        .widgets
                        .iter()
                        .find(|w| w.id == id)
                        .ok_or("Widget unavailable.")?
                        .clone();
                    let scope = w.scope.clone().unwrap_or_else(|| d.scope.clone());
                    s.configured = Some(id.into());
                    Self::fields(&mut s, &scope, &w.labels, w.assignee.as_deref());
                    s.fields.insert("agent".into(), w.agent.unwrap_or_default());
                    s.fields.insert("host".into(), w.host.unwrap_or_default());
                    s.fields.insert(
                        "window-hours".into(),
                        w.window_hours.map(|h| h.to_string()).unwrap_or_default(),
                    );
                    s.screen = Screen::Configure;
                } else if let Some(id) = action.strip_prefix("project:") {
                    s.fields.insert("project".into(), id.into());
                    s.fields.insert("repositories".into(), String::new());
                }
            }
        }
        self.changed(&mut s);
        Ok(Effect::None)
    }
    pub async fn refresh(&self, force: bool) {
        if !self.current()
            || self.state.lock().unwrap().screen != Screen::Home
            || self.busy.swap(true, Ordering::AcqRel)
        {
            return;
        }
        self.refresh_inner(force).await;
        self.busy.store(false, Ordering::Release);
    }
    async fn refresh_inner(&self, force: bool) {
        let Some(lease) = &self.lease else { return };
        let Ok(home) = lease.client().home(lease.session()) else {
            return;
        };
        let cancel = lease.cancel.child_token();
        if !self.path.exists() && !self.remote_checked.swap(true, Ordering::AcqRel) {
            self.sync(true).await;
        }
        let (first, d, fetch_issues) = {
            let s = self.state.lock().unwrap();
            (
                !s.initialized,
                Self::selected(&s).clone(),
                force
                    || s.last_issues
                        .is_none_or(|at| at.elapsed() >= Duration::from_secs(60)),
            )
        };
        if first || force {
            match home.projects(&cancel).await {
                Ok(projects) if self.current() => {
                    let mut s = self.state.lock().unwrap();
                    s.projects = projects;
                    self.changed(&mut s);
                }
                Err(e) => self.error(e),
                _ => {}
            }
        }
        match home.work(None, &cancel).await {
            Ok(page) if self.current() => {
                let mut s = self.state.lock().unwrap();
                s.remote_next = page.next;
                s.work_error = None;
                s.work = page.data;
                if s.computers.is_empty() {
                    s.computers = page.computers;
                }
                s.initialized = true;
                self.changed(&mut s);
            }
            Err(e) => {
                if self.current() {
                    let mut s = self.state.lock().unwrap();
                    s.work_error = Some(e.to_string());
                    self.changed(&mut s);
                }
            }
            _ => {}
        }
        if fetch_issues
            && self
                .state
                .lock()
                .unwrap()
                .issue_resume
                .is_none_or(|at| at <= Instant::now())
        {
            let mut seen = std::collections::BTreeSet::new();
            for w in &d.widgets {
                if !matches!(w.kind.as_str(), "issues" | "my_issues") {
                    continue;
                }
                let key = Self::issue_key(&d, w);
                if !seen.insert(key.clone()) {
                    continue;
                }
                let scope = w.scope.clone().unwrap_or_else(|| d.scope.clone());
                if scope.project.is_none() && scope.repositories.is_empty() {
                    continue;
                }
                let filter = IssueFilter {
                    scope,
                    mine: w.kind == "my_issues",
                    assignee: w.assignee.clone(),
                    labels: w.labels.clone(),
                    page: 1,
                };
                let result = home.issues(&filter, &cancel).await;
                if !self.current() {
                    return;
                }
                let mut s = self.state.lock().unwrap();
                if Self::selected(&s).id != d.id || Self::selected(&s).revision != d.revision {
                    continue;
                }
                let mut stop = false;
                match result {
                    Ok(value) => {
                        s.issues.insert(key, Ok(value));
                        s.issue_failures = 0;
                        s.issue_resume = None;
                    }
                    Err(error) => {
                        let retry = match &error {
                            Error::RateLimited { retry_after }
                            | Error::Unavailable { retry_after } => *retry_after,
                            _ => None,
                        };
                        s.issue_failures = s.issue_failures.saturating_add(1).min(5);
                        let delay = retry
                            .unwrap_or(Duration::from_secs((30_u64 << s.issue_failures).min(900)));
                        s.issue_resume = Some(Instant::now() + delay);
                        let text = error.to_string();
                        if matches!(
                            error,
                            Error::Unauthorized
                                | Error::Forbidden
                                | Error::NotFound
                                | Error::Refused { status: 409, .. }
                        ) || !matches!(s.issues.get(&key), Some(Ok(_)))
                        {
                            s.issues.insert(key, Err(text.clone()));
                        }
                        s.notice = Some(text);
                        stop = true;
                    }
                }
                s.last_issues = Some(Instant::now());
                self.changed(&mut s);
                if stop {
                    break;
                }
            }
        }
    }
    fn error(&self, error: Error) {
        if !self.current() {
            return;
        }
        let mut s = self.state.lock().unwrap();
        s.notice = Some(error.to_string());
        self.changed(&mut s);
    }
    async fn more_issues(&self) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to see issues.")?;
        let (d, w, key, page) = {
            let s = self.state.lock().unwrap();
            let d = Self::selected(&s).clone();
            let w = s
                .configured
                .as_ref()
                .and_then(|id| d.widgets.iter().find(|w| &w.id == id))
                .cloned()
                .ok_or("Choose an issue widget first.")?;
            let key = Self::issue_key(&d, &w);
            let page = s
                .issues
                .get(&key)
                .and_then(|v| v.as_ref().ok())
                .and_then(|v| v.next_page)
                .ok_or("All available issues are shown.")?;
            (d, w, key, page)
        };
        let filter = IssueFilter {
            scope: w.scope.clone().unwrap_or_else(|| d.scope.clone()),
            mine: w.kind == "my_issues",
            assignee: w.assignee,
            labels: w.labels,
            page,
        };
        let result = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?
            .issues(&filter, &lease.cancel)
            .await;
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        let mut s = self.state.lock().unwrap();
        if Self::selected(&s).id != d.id || Self::selected(&s).revision != d.revision {
            return Err("This dashboard changed. Open the issue list again.".into());
        }
        match result {
            Ok(mut next) => {
                if let Some(Ok(old)) = s.issues.get(&key) {
                    let old_len = old.items.len();
                    let mut seen = std::collections::BTreeSet::new();
                    let mut items = old.items.clone();
                    items.extend(next.items);
                    items.retain(|i| seen.insert((i.repository.clone(), i.number)));
                    items.truncate(10_000);
                    next.items = items;
                    s.issue_window = old_len.min(next.items.len().saturating_sub(1)) / 100 * 100;
                    if next.items.len() >= 10_000 {
                        next.next_page = None;
                    }
                }
                s.issues.insert(key, Ok(next));
            }
            Err(e) => {
                if matches!(e, Error::Unauthorized | Error::Forbidden | Error::NotFound) {
                    s.issues.insert(key, Err(e.to_string()));
                }
                s.notice = Some(e.to_string());
            }
        }
        self.changed(&mut s);
        Ok(())
    }
    async fn more_work(&self) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to see work.")?;
        let after = self
            .state
            .lock()
            .unwrap()
            .remote_next
            .clone()
            .ok_or("All available work is shown.")?;
        let page = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?
            .work(Some(&after), &lease.cancel)
            .await
            .map_err(|e| e.to_string())?;
        if page.next.as_deref() == Some(&after) {
            return Err("Refresh work and try again.".into());
        }
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        let mut s = self.state.lock().unwrap();
        s.remote_next = page.next;
        s.work.extend(page.data);
        s.work = work_rows(std::mem::take(&mut s.work), None);
        self.changed(&mut s);
        Ok(())
    }
    async fn sync(&self, reload: bool) {
        let _guard = self.io.lock().await;
        let Some(lease) = &self.lease else { return };
        let Ok(home) = lease.client().home(lease.session()) else {
            return;
        };
        if reload {
            match home.library(&lease.cancel).await {
                Ok(library) if self.current() && !library.dashboards.is_empty() => {
                    let mut s = self.state.lock().unwrap();
                    if s.draft.is_some() {
                        s.notice = Some("Finish editing before reloading layouts.".into());
                    } else {
                        if s.library.dashboards != library.dashboards {
                            if let Err(message) = self.write_private(
                                &self.path.with_extension("before-reload.json"),
                                &s.library,
                            ) {
                                s.notice = Some(message);
                                self.changed(&mut s);
                                return;
                            }
                        }
                        s.library = library;
                        s.selected = s
                            .library
                            .default
                            .clone()
                            .unwrap_or_else(|| s.library.dashboards[0].id.clone());
                        s.library_pending = None;
                        if let Err(e) = self.save_local(&s) {
                            s.notice = Some(e);
                        }
                    }
                    self.changed(&mut s);
                }
                Err(e) => self.error(e),
                _ => {}
            }
            return;
        }
        let request = {
            let mut s = self.state.lock().unwrap();
            if let Some(pending) = &s.library_pending {
                pending.clone()
            } else {
                let Ok(request) = home.save_request(s.library.clone()) else {
                    return;
                };
                s.library_pending = Some(request.clone());
                request
            }
        };
        // Persist the account-bound request before sending. A lost response reuses it.
        let pending_path = self.path.with_extension("pending.json");
        if self.write_private(&pending_path, &request).is_err() {
            self.error(Error::InvalidResponse);
            return;
        }
        let result = home.save(&request, &lease.cancel).await;
        if !self.current() {
            return;
        }
        let mut s = self.state.lock().unwrap();
        match result {
            Ok(library) => {
                s.library.revision = library.revision;
                s.library_pending = None;
                s.notice = Some(
                    if s.library.dashboards == library.dashboards {
                        "Layout saved to your account."
                    } else {
                        "Earlier layout saved. Save again to include your latest edits."
                    }
                    .into(),
                );
                let _ = std::fs::remove_file(pending_path);
                if let Err(e) = self.save_local(&s) {
                    s.notice = Some(e);
                }
            }
            Err(Error::Refused { status: 409, .. }) => {
                s.library_pending = None;
                let _ = std::fs::remove_file(pending_path);
                s.notice = Some(
                    "Another device changed your layouts. Reload them or keep editing this copy."
                        .into(),
                );
            }
            Err(error) => s.notice = Some(error.to_string()),
        }
        self.changed(&mut s);
    }
    async fn directory(&self, action: &str) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to browse layouts.")?;
        let home = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?;
        let (search, after, mine, author, ticket) = {
            let mut s = self.state.lock().unwrap();
            if action == "mine" {
                s.fields.insert("directory-mode".into(), "mine".into());
            }
            if matches!(action, "directory" | "search" | "author-layouts") {
                s.fields.remove("directory-mode");
            }
            if matches!(action, "directory" | "search" | "mine") {
                s.fields.remove("author");
            }
            s.navigation += 1;
            (
                s.fields.get("search").cloned().unwrap_or_default(),
                if action == "more-layouts" {
                    s.next.clone()
                } else {
                    None
                },
                s.fields.get("directory-mode").is_some_and(|v| v == "mine"),
                s.fields.get("author").cloned(),
                s.navigation,
            )
        };
        let page = if let Some(author) = author {
            home.directory_by_author(&author, after.as_deref(), &lease.cancel)
                .await
        } else {
            home.directory(&search, after.as_deref(), mine, &lease.cancel)
                .await
        }
        .map_err(|e| e.to_string())?;
        if page.next.is_some() && page.next == after {
            return Err("Search layouts again to continue.".into());
        }
        if !self.current() {
            return Err("Sign in again to see layouts.".into());
        }
        let mut s = self.state.lock().unwrap();
        if s.navigation != ticket {
            return Ok(());
        }
        if after.is_none() {
            s.listings.clear();
        }
        let mut seen: std::collections::BTreeSet<_> =
            s.listings.iter().map(|l| l.id.clone()).collect();
        s.listings
            .extend(page.data.into_iter().filter(|l| seen.insert(l.id.clone())));
        s.next = page.next;
        s.screen = Screen::Directory;
        s.notice = None;
        self.changed(&mut s);
        Ok(())
    }
    async fn preview(&self, id: &str) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to browse layouts.")?;
        let ticket = {
            let mut s = self.state.lock().unwrap();
            s.navigation += 1;
            s.navigation
        };
        let l = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?
            .template(id, None, &lease.cancel)
            .await
            .map_err(|e| e.to_string())?;
        if !self.current() {
            return Err("Sign in again to see layouts.".into());
        }
        let mut s = self.state.lock().unwrap();
        if s.navigation != ticket {
            return Ok(());
        }
        s.preview = Some(l);
        s.update_target = None;
        s.screen = Screen::Preview;
        self.changed(&mut s);
        Ok(())
    }
    async fn check_update(&self) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to check updates.")?;
        let home = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?;
        let local = {
            let s = self.state.lock().unwrap();
            Self::selected(&s).clone()
        };
        let source = local.source.clone().ok_or("This is your own layout.")?;
        let latest = home
            .template(&source.id, None, &lease.cancel)
            .await
            .map_err(|e| e.to_string())?;
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        if latest.version == source.version {
            let mut s = self.state.lock().unwrap();
            s.notice = Some("Your layout is up to date.".into());
            self.changed(&mut s);
            return Ok(());
        }
        let old = home
            .template(&source.id, Some(source.version), &lease.cancel)
            .await
            .map_err(|e| e.to_string())?;
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        let mut s = self.state.lock().unwrap();
        s.update_target = Some((local, old.package));
        s.preview = Some(latest);
        s.screen = Screen::Preview;
        self.changed(&mut s);
        Ok(())
    }
    fn write_private<T: Serialize>(&self, path: &std::path::Path, value: &T) -> Result<(), String> {
        let parent = path.parent().ok_or_else(storage_error)?;
        std::fs::create_dir_all(parent).map_err(|_| storage_error())?;
        let temp = path.with_extension(format!("{}.new", self.instance));
        let bytes = serde_json::to_vec(value).map_err(|_| storage_error())?;
        std::fs::write(&temp, bytes).map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| storage_error())?;
        }
        std::fs::rename(temp, path).map_err(|_| storage_error())
    }
    async fn publish(&self, visibility: &str) -> Result<Effect, String> {
        let lease = self.lease.as_ref().ok_or("Sign in to share layouts.")?;
        let home = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?;
        let package = {
            let s = self.state.lock().unwrap();
            let d = s.draft.as_ref().unwrap_or_else(|| Self::selected(&s));
            d.export(d.name.clone()).map_err(str::to_owned)?
        };
        let visibility = match visibility {
            "public" => Visibility::Public,
            "unlisted" => Visibility::Unlisted,
            _ => Visibility::Private,
        };
        let target = self.state.lock().unwrap().publishing.clone();
        let request = home
            .publish_request(package, visibility, target.as_ref().map_or(0, |(_, v)| *v))
            .map_err(|e| e.to_string())?;
        let id = target
            .map(|(id, _)| id)
            .unwrap_or_else(|| request.body.request_id.clone());
        self.perform(Some(Pending::Publish { id, request })).await
    }
    async fn perform(&self, new: Option<Pending>) -> Result<Effect, String> {
        let _guard = self.io.lock().await;
        let lease = self.lease.as_ref().ok_or("Sign in to continue.")?;
        let home = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?;
        let path = self.path.with_extension("remote.json");
        let saved = std::fs::read(&path)
            .ok()
            .filter(|v| v.len() <= 1024 * 1024)
            .and_then(|v| serde_json::from_slice::<Pending>(&v).ok());
        let pending = match (saved, new) {
            (Some(_), Some(_)) => {
                return Err("Finish the saved change before starting another.".into());
            }
            (Some(p), None) | (None, Some(p)) => p,
            (None, None) => return Err("There is no saved change to finish.".into()),
        };
        self.write_private(&path, &pending)?;
        let outcome = async {
            match &pending {
                Pending::Publish { id, request } => {
                    let l = home.publish(id, request, &lease.cancel).await?;
                    Ok((Some(l), Some(id.clone()), "Layout saved."))
                }
                Pending::Remove { id, request } => {
                    home.remove(id, request, &lease.cancel).await?;
                    Ok((None, None, "Listing removed."))
                }
                Pending::Report { id, request } => {
                    home.report(id, request, &lease.cancel).await?;
                    Ok((None, None, "Report sent."))
                }
            }
        }
        .await;
        let (listing, link, message) = match outcome {
            Ok(value) => value,
            Err(openagents::Error::Changed | openagents::Error::Refused { status: 409, .. }) => {
                std::fs::remove_file(path).map_err(|_| storage_error())?;
                return Err("This listing changed. Open its latest version and try again.".into());
            }
            Err(error) => return Err(error.to_string()),
        };
        if !self.current() {
            return Err("Sign in again to continue.".into());
        }
        std::fs::remove_file(path).map_err(|_| storage_error())?;
        let mut s = self.state.lock().unwrap();
        s.preview = listing;
        s.screen = if s.preview.is_some() {
            Screen::Preview
        } else {
            Screen::Home
        };
        s.notice = Some(message.into());
        self.changed(&mut s);
        Ok(if let Some(id) = link {
            Effect::Export {
                name: "OpenAgents-layout-link.txt".into(),
                contents: format!("openagents://layout/{id}"),
            }
        } else {
            Effect::None
        })
    }
    async fn listing_action(&self, action: &str) -> Result<(), String> {
        let lease = self.lease.as_ref().ok_or("Sign in to continue.")?;
        let home = lease
            .client()
            .home(lease.session())
            .map_err(|e| e.to_string())?;
        let l = self
            .state
            .lock()
            .unwrap()
            .preview
            .clone()
            .ok_or("Choose a layout first.")?;
        let pending = if action == "unpublish" {
            Pending::Remove {
                id: l.id,
                request: home.remove_request(l.version).map_err(|e| e.to_string())?,
            }
        } else {
            Pending::Report {
                id: l.id,
                request: home.bind(Report {
                    request_id: fresh_id(),
                    reason: "Review requested by a community member.".into(),
                }),
            }
        };
        self.perform(Some(pending)).await?;
        Ok(())
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dashboard_copy_is_plain() {
        let violations = oa_copy::source_violations(include_str!("dashboard.rs"), &[]);
        assert!(violations.is_empty(), "{violations:?}");
    }
    #[test]
    fn shutdown_clears_private_layouts_fields_and_work() {
        let dir = tempfile::tempdir().unwrap();
        let h = Home::new(None, dir.path().join("home.json"));
        {
            let mut s = h.state.lock().unwrap();
            s.library.dashboards[0].name = "Secret project".into();
            s.fields.insert("project".into(), "Secret project".into());
            s.draft = Some(s.library.dashboards[0].clone());
            s.screen = Screen::Edit;
        }
        h.shutdown();
        let view = serde_json::to_string(&h.view()).unwrap();
        assert!(!view.contains("Secret project"));
        h.view().validate().unwrap();
    }
    #[tokio::test]
    async fn editing_is_cancelable_persistent_and_callbacks_reject_old_views() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home.json");
        let h = Home::new(None, path.clone());
        let original = h.state.lock().unwrap().library.clone();
        let before = h.view();
        h.action("edit").await.unwrap();
        assert!(
            h.activate(
                Activation {
                    instance: before.instance,
                    revision: before.revision,
                    node: "edit".into()
                },
                None
            )
            .await
            .is_err()
        );
        h.action("add:computers").await.unwrap();
        h.view().validate().unwrap();
        h.action("cancel").await.unwrap();
        assert_eq!(h.state.lock().unwrap().library, original);
        h.action("edit").await.unwrap();
        h.action("size:issues").await.unwrap();
        h.action("move:issues:1").await.unwrap();
        h.action("remove:mine").await.unwrap();
        h.action("undo").await.unwrap();
        h.action("done").await.unwrap();
        let reopened = Home::new(None, path);
        assert_eq!(
            h.state.lock().unwrap().library,
            reopened.state.lock().unwrap().library
        );
        for action in ["edit", "scope", "back-settings", "cancel"] {
            h.action(action).await.unwrap();
            h.view().validate().unwrap();
        }
    }
    #[tokio::test]
    async fn drag_reorders_only_the_current_edit_draft() {
        let dir = tempfile::tempdir().unwrap();
        let h = Home::new(None, dir.path().join("home.json"));
        let original = h.state.lock().unwrap().library.clone();
        h.action("edit").await.unwrap();
        let view = h.view();
        let activation = |id: &str| Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: format!("up-{id}"),
        };
        h.reorder(activation("issues"), activation("agents"))
            .unwrap();
        assert_eq!(
            h.state.lock().unwrap().draft.as_ref().unwrap().widgets[2].id,
            "issues"
        );
        assert!(
            h.reorder(activation("agents"), activation("issues"))
                .is_err()
        );
        h.action("cancel").await.unwrap();
        assert_eq!(h.state.lock().unwrap().library, original);
    }
    #[tokio::test]
    async fn importing_creates_a_draft_and_keeps_existing_home() {
        let dir = tempfile::tempdir().unwrap();
        let h = Home::new(None, dir.path().join("home.json"));
        let original = h.state.lock().unwrap().library.clone();
        let package = Dashboard::starter("source")
            .export("Team work".into())
            .unwrap();
        h.import(&serde_json::to_vec(&package).unwrap())
            .await
            .unwrap();
        assert_eq!(h.state.lock().unwrap().library, original);
        h.view().validate().unwrap();
        h.action("cancel").await.unwrap();
        assert_eq!(h.state.lock().unwrap().library, original);
    }
}
