//! WGPUI GitAfter view.

use std::cell::RefCell;
use std::rc::Rc;

use wgpui::components::atoms::{BountyBadge, IssueStatus, PrStatus, PrStatusBadge};
use wgpui::components::molecules::{IssueInfo, IssueRow, RepoCard, RepoInfo, RepoVisibility};
use wgpui::components::organisms::{DiffLine, DiffLineKind, DiffToolCall};
use wgpui::components::{Component, EventResult, Text};
use wgpui::{
    Bounds, EventContext, Hsla, InputEvent, MouseButton, Point, Quad, ScrollContainer, Size, theme,
};

use crate::views::diff::{DiffLineType, parse_diff_lines};

use super::types::{
    ConnectionStatus, GitafterCommand, GitafterTab, GitafterUpdate, IssueSummary, PrSummary,
    RepoSummary,
};

const HEADER_HEIGHT: f32 = 64.0;
const TAB_HEIGHT: f32 = 40.0;
const PADDING: f32 = 20.0;
const LIST_GAP: f32 = 12.0;
const LIST_MIN_WIDTH: f32 = 320.0;
const LIST_MAX_WIDTH: f32 = 480.0;
const PR_ROW_HEIGHT: f32 = 72.0;
const DETAIL_HEADER_HEIGHT: f32 = 160.0;
const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
enum GitafterUiEvent {
    SelectTab(GitafterTab),
    SelectRepo(String),
    SelectIssue(String),
    SelectPr(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoticeKind {
    Error,
}

#[derive(Debug, Clone)]
struct Notice {
    kind: NoticeKind,
    message: String,
}

impl Notice {
    fn color(&self) -> Hsla {
        match self.kind {
            NoticeKind::Error => theme::status::ERROR,
        }
    }
}

struct GitafterLayout {
    header: Bounds,
    tabs: Bounds,
    list: Bounds,
    detail: Bounds,
    repos_tab: Bounds,
    issues_tab: Bounds,
    prs_tab: Bounds,
}

struct PrRow {
    id: String,
    title: String,
    status: PrStatus,
    author: String,
    created_at: String,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl PrRow {
    fn new(summary: &PrSummary) -> Self {
        Self {
            id: summary.id.clone(),
            title: summary.title.clone(),
            status: pr_status_from_str(&summary.status),
            author: summary.author.clone(),
            created_at: summary.created_at.clone(),
            hovered: false,
            on_click: None,
        }
    }

    fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let padding = 12.0;
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let status_color = self.status.color();
        let bar_w = 3.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bar_w,
                bounds.size.height,
            ))
            .with_background(status_color),
        );

        let content_x = bounds.origin.x + padding + bar_w;
        let title = if self.title.len() > 50 {
            format!("{}...", &self.title[..47])
        } else {
            self.title.clone()
        };
        let title_run = cx.text.layout(
            &title,
            Point::new(content_x, bounds.origin.y + 12.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        let meta_y = bounds.origin.y + 36.0;
        let author_run = cx.text.layout(
            &format!("by {}", self.author),
            Point::new(content_x, meta_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(author_run);

        let time_run = cx.text.layout(
            &self.created_at,
            Point::new(content_x + 140.0, meta_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);

        let mut badge = PrStatusBadge::new(self.status).compact(true);
        let badge_bounds = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 26.0,
            bounds.origin.y + 10.0,
            24.0,
            22.0,
        );
        badge.paint(badge_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.id.clone());
                    }
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }
}

pub struct GitafterView {
    tab: GitafterTab,
    hovered_tab: Option<GitafterTab>,
    connection_status: ConnectionStatus,
    status_message: Option<String>,
    notice: Option<Notice>,
    repos: Vec<RepoSummary>,
    repo_cards: Vec<RepoCard>,
    selected_repo: Option<String>,
    issues: Vec<IssueSummary>,
    issue_rows: Vec<IssueRow>,
    selected_issue: Option<String>,
    prs: Vec<PrSummary>,
    pr_rows: Vec<PrRow>,
    selected_pr: Option<String>,
    pr_diff_id: Option<String>,
    diff_tool_calls: Vec<DiffToolCall>,
    repo_scroll: ScrollContainer,
    issue_scroll: ScrollContainer,
    pr_scroll: ScrollContainer,
    diff_scroll: ScrollContainer,
    cursor_position: Point,
    ui_events: Rc<RefCell<Vec<GitafterUiEvent>>>,
    commands: Vec<GitafterCommand>,
    pending_repo_id: Option<String>,
}

impl GitafterView {
    pub fn new(initial_tab: GitafterTab, pending_repo_id: Option<String>) -> Self {
        let ui_events = Rc::new(RefCell::new(Vec::new()));
        let mut commands = Vec::new();
        commands.push(GitafterCommand::LoadRepositories {
            limit: DEFAULT_LIMIT,
        });

        if matches!(initial_tab, GitafterTab::Issues) {
            commands.push(GitafterCommand::LoadIssues {
                repo_address: None,
                limit: DEFAULT_LIMIT,
            });
        }

        if matches!(initial_tab, GitafterTab::PullRequests) {
            commands.push(GitafterCommand::LoadPullRequests {
                repo_address: None,
                limit: DEFAULT_LIMIT,
            });
        }

        Self {
            tab: initial_tab,
            hovered_tab: None,
            connection_status: ConnectionStatus::Connecting,
            status_message: None,
            notice: None,
            repos: Vec::new(),
            repo_cards: Vec::new(),
            selected_repo: None,
            issues: Vec::new(),
            issue_rows: Vec::new(),
            selected_issue: None,
            prs: Vec::new(),
            pr_rows: Vec::new(),
            selected_pr: None,
            pr_diff_id: None,
            diff_tool_calls: Vec::new(),
            repo_scroll: ScrollContainer::vertical(Bounds::ZERO),
            issue_scroll: ScrollContainer::vertical(Bounds::ZERO),
            pr_scroll: ScrollContainer::vertical(Bounds::ZERO),
            diff_scroll: ScrollContainer::vertical(Bounds::ZERO),
            cursor_position: Point::ZERO,
            ui_events,
            commands,
            pending_repo_id,
        }
    }

    pub fn apply_update(&mut self, update: GitafterUpdate) {
        match update {
            GitafterUpdate::RepositoriesLoaded { repos } => {
                self.repos = repos;
                self.rebuild_repo_cards();

                if let Some(repo_id) = self.pending_repo_id.take() {
                    if self.select_repo_by_id(&repo_id) {
                        return;
                    }
                }

                if self.selected_repo.is_none() && !self.repos.is_empty() {
                    let first_id = self.repos[0].id.clone();
                    self.select_repo_by_id(&first_id);
                }
            }
            GitafterUpdate::IssuesLoaded { issues } => {
                self.issues = issues;
                self.rebuild_issue_rows();
                if self.selected_issue.is_none() && !self.issues.is_empty() {
                    self.selected_issue = Some(self.issues[0].id.clone());
                }
            }
            GitafterUpdate::PullRequestsLoaded { pull_requests } => {
                self.prs = pull_requests;
                self.rebuild_pr_rows();
                if self.selected_pr.is_none() && !self.prs.is_empty() {
                    self.selected_pr = Some(self.prs[0].id.clone());
                }
            }
            GitafterUpdate::PullRequestDiffLoaded { pr_id, diff } => {
                if self.selected_pr.as_deref() == Some(pr_id.as_str()) {
                    self.pr_diff_id = Some(pr_id);
                    self.diff_tool_calls = diff
                        .as_deref()
                        .map(build_diff_tool_calls)
                        .unwrap_or_default();
                }
            }
            GitafterUpdate::ConnectionStatus { status, message } => {
                self.connection_status = status;
                self.status_message = message;
            }
            GitafterUpdate::Error { message } => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Error,
                    message,
                });
            }
        }
    }

    pub fn drain_commands(&mut self) -> Vec<GitafterCommand> {
        std::mem::take(&mut self.commands)
    }

    fn rebuild_repo_cards(&mut self) {
        let ui_events = self.ui_events.clone();
        self.repo_cards = self
            .repos
            .iter()
            .map(|repo| {
                let repo_id = repo.id.clone();
                let mut info = RepoInfo::new(&repo.id, &repo.name)
                    .visibility(RepoVisibility::Public)
                    .updated_at(repo.updated_at.clone());
                if let Some(description) = &repo.description {
                    info = info.description(description.clone());
                }
                if let Some(language) = &repo.language {
                    info = info.language(language.clone());
                }
                RepoCard::new(info).on_click({
                    let ui_events = ui_events.clone();
                    move |_| {
                        ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectRepo(repo_id.clone()));
                    }
                })
            })
            .collect();
    }

    fn rebuild_issue_rows(&mut self) {
        let ui_events = self.ui_events.clone();
        self.issue_rows = self
            .issues
            .iter()
            .enumerate()
            .map(|(idx, issue)| {
                let issue_id = issue.id.clone();
                let status = issue_status_from_str(&issue.status);
                let mut info = IssueInfo::new(&issue.id, (idx + 1) as u32, &issue.title)
                    .status(status)
                    .author(issue.author.clone())
                    .created_at(issue.created_at.clone())
                    .comments(0);
                if let Some(bounty) = issue.bounty_sats {
                    info = info.bounty(bounty);
                }

                IssueRow::new(info).on_click({
                    let ui_events = ui_events.clone();
                    move |_| {
                        ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectIssue(issue_id.clone()));
                    }
                })
            })
            .collect();
    }

    fn rebuild_pr_rows(&mut self) {
        let ui_events = self.ui_events.clone();
        self.pr_rows = self
            .prs
            .iter()
            .map(|pr| {
                let pr_id = pr.id.clone();
                PrRow::new(pr).on_click({
                    let ui_events = ui_events.clone();
                    move |_| {
                        ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectPr(pr_id.clone()));
                    }
                })
            })
            .collect();
    }

    fn select_repo_by_id(&mut self, repo_id: &str) -> bool {
        if let Some(repo) = self.repos.iter().find(|repo| repo.id == repo_id) {
            self.selected_repo = Some(repo.id.clone());
            self.selected_issue = None;
            self.selected_pr = None;
            self.pr_diff_id = None;
            self.diff_tool_calls.clear();

            self.commands.push(GitafterCommand::LoadIssues {
                repo_address: Some(repo.address.clone()),
                limit: DEFAULT_LIMIT,
            });
            self.commands.push(GitafterCommand::LoadPullRequests {
                repo_address: Some(repo.address.clone()),
                limit: DEFAULT_LIMIT,
            });
            true
        } else {
            false
        }
    }

    fn handle_ui_events(&mut self) -> bool {
        let mut changed = false;
        let events: Vec<GitafterUiEvent> = self.ui_events.borrow_mut().drain(..).collect();
        for event in events {
            changed = true;
            match event {
                GitafterUiEvent::SelectTab(tab) => {
                    self.tab = tab;
                    self.notice = None;
                    match tab {
                        GitafterTab::Issues => {
                            let repo_address = self
                                .selected_repo
                                .as_deref()
                                .and_then(|id| self.repos.iter().find(|r| r.id == id))
                                .map(|repo| repo.address.clone());
                            self.commands.push(GitafterCommand::LoadIssues {
                                repo_address,
                                limit: DEFAULT_LIMIT,
                            });
                        }
                        GitafterTab::PullRequests => {
                            let repo_address = self
                                .selected_repo
                                .as_deref()
                                .and_then(|id| self.repos.iter().find(|r| r.id == id))
                                .map(|repo| repo.address.clone());
                            self.commands.push(GitafterCommand::LoadPullRequests {
                                repo_address,
                                limit: DEFAULT_LIMIT,
                            });
                        }
                        GitafterTab::Repos => {}
                    }
                }
                GitafterUiEvent::SelectRepo(repo_id) => {
                    self.select_repo_by_id(&repo_id);
                }
                GitafterUiEvent::SelectIssue(issue_id) => {
                    self.selected_issue = Some(issue_id);
                }
                GitafterUiEvent::SelectPr(pr_id) => {
                    self.selected_pr = Some(pr_id.clone());
                    self.pr_diff_id = None;
                    self.diff_tool_calls.clear();
                    let repo_identifier = self
                        .prs
                        .iter()
                        .find(|pr| pr.id == pr_id)
                        .and_then(|pr| pr.repo_identifier.clone());
                    self.commands.push(GitafterCommand::LoadPullRequestDiff {
                        pr_id,
                        repo_identifier,
                    });
                }
            }
        }
        changed
    }

    fn layout(&self, bounds: Bounds) -> GitafterLayout {
        let header = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            HEADER_HEIGHT,
        );
        let tabs = Bounds::new(
            bounds.origin.x,
            header.max_y(),
            bounds.size.width,
            TAB_HEIGHT,
        );
        let content = Bounds::new(
            bounds.origin.x,
            tabs.max_y(),
            bounds.size.width,
            bounds.size.height - HEADER_HEIGHT - TAB_HEIGHT,
        );

        let stacked = content.size.width < 860.0;
        let list_width = (content.size.width * 0.38)
            .clamp(LIST_MIN_WIDTH, LIST_MAX_WIDTH)
            .min(content.size.width - PADDING * 2.0);

        let (list, detail) = if stacked {
            let list_height = (content.size.height * 0.42).max(220.0);
            let list = Bounds::new(
                content.origin.x + PADDING,
                content.origin.y + PADDING,
                content.size.width - PADDING * 2.0,
                list_height,
            );
            let detail = Bounds::new(
                content.origin.x + PADDING,
                list.max_y() + PADDING,
                content.size.width - PADDING * 2.0,
                content.size.height - list_height - PADDING * 3.0,
            );
            (list, detail)
        } else {
            let list = Bounds::new(
                content.origin.x + PADDING,
                content.origin.y + PADDING,
                list_width,
                content.size.height - PADDING * 2.0,
            );
            let detail = Bounds::new(
                list.max_x() + PADDING,
                content.origin.y + PADDING,
                content.size.width - list_width - PADDING * 3.0,
                content.size.height - PADDING * 2.0,
            );
            (list, detail)
        };

        let tab_width = 110.0;
        let tab_height = TAB_HEIGHT - 12.0;
        let tab_y = tabs.origin.y + 6.0;
        let repos_tab = Bounds::new(tabs.origin.x + PADDING, tab_y, tab_width, tab_height);
        let issues_tab = Bounds::new(repos_tab.max_x() + 8.0, tab_y, tab_width, tab_height);
        let prs_tab = Bounds::new(issues_tab.max_x() + 8.0, tab_y, tab_width, tab_height);

        GitafterLayout {
            header,
            tabs,
            list,
            detail,
            repos_tab,
            issues_tab,
            prs_tab,
        }
    }

    fn draw_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: Hsla,
        cx: &mut wgpui::PaintContext,
    ) {
        let run = cx.text.layout(text, Point::new(x, y), size, color);
        cx.scene.draw_text(run);
    }

    fn paint_header(&self, layout: &GitafterLayout, cx: &mut wgpui::PaintContext) {
        cx.scene.draw_quad(
            Quad::new(layout.header)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        self.draw_text(
            "GITAFTER",
            layout.header.origin.x + PADDING,
            layout.header.origin.y + 18.0,
            theme::font_size::SM,
            theme::text::MUTED,
            cx,
        );

        let title_run = cx.text.layout(
            "Nostr Git Collaboration",
            Point::new(
                layout.header.origin.x + PADDING,
                layout.header.origin.y + 36.0,
            ),
            theme::font_size::LG,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        let status = self.connection_status;
        let status_color = match status {
            ConnectionStatus::Connected => theme::status::SUCCESS,
            ConnectionStatus::Error => theme::status::ERROR,
            ConnectionStatus::Connecting => theme::status::INFO,
        };
        let status_text = self
            .status_message
            .clone()
            .unwrap_or_else(|| status.label().to_string());

        let status_width = status_text.len() as f32 * 6.0 + 20.0;
        let status_bounds = Bounds::new(
            layout.header.max_x() - PADDING - status_width,
            layout.header.origin.y + 18.0,
            status_width,
            22.0,
        );
        cx.scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(status_color.with_alpha(0.2))
                .with_border(status_color, 1.0),
        );
        let status_run = cx.text.layout(
            &status_text,
            Point::new(status_bounds.origin.x + 8.0, status_bounds.origin.y + 4.0),
            theme::font_size::XS,
            status_color,
        );
        cx.scene.draw_text(status_run);
    }

    fn paint_tabs(&self, layout: &GitafterLayout, cx: &mut wgpui::PaintContext) {
        cx.scene.draw_quad(
            Quad::new(layout.tabs)
                .with_background(theme::bg::APP)
                .with_border(theme::border::SUBTLE, 1.0),
        );

        self.draw_tab(layout.repos_tab, GitafterTab::Repos, cx);
        self.draw_tab(layout.issues_tab, GitafterTab::Issues, cx);
        self.draw_tab(layout.prs_tab, GitafterTab::PullRequests, cx);
    }

    fn draw_tab(&self, bounds: Bounds, tab: GitafterTab, cx: &mut wgpui::PaintContext) {
        let is_active = self.tab == tab;
        let is_hovered = self.hovered_tab == Some(tab);
        let bg = if is_active {
            theme::bg::SURFACE
        } else if is_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::APP
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let text = tab.to_string();
        let text_w = text.len() as f32 * 6.8;
        let x = bounds.origin.x + (bounds.size.width - text_w) / 2.0;
        let y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;
        self.draw_text(&text, x, y, theme::font_size::SM, theme::text::PRIMARY, cx);
    }

    fn sync_scroll(scroll: &mut ScrollContainer, viewport: Bounds, content_height: f32) {
        scroll.set_viewport(viewport);
        scroll.set_content_size(Size::new(viewport.size.width, content_height));
    }

    fn paint_repo_list(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let total_height = self
            .repo_cards
            .iter()
            .map(|card| card.size_hint().1.unwrap_or(90.0))
            .sum::<f32>()
            + (self.repo_cards.len().saturating_sub(1) as f32 * LIST_GAP);
        Self::sync_scroll(&mut self.repo_scroll, bounds, total_height);

        let mut y = bounds.origin.y - self.repo_scroll.scroll_offset.y;
        for (idx, card) in self.repo_cards.iter_mut().enumerate() {
            let height = card.size_hint().1.unwrap_or(90.0);
            let card_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, height);
            if card_bounds.max_y() >= bounds.min_y() - 60.0
                && card_bounds.min_y() <= bounds.max_y() + 60.0
            {
                card.paint(card_bounds, cx);
                if self
                    .selected_repo
                    .as_deref()
                    .is_some_and(|id| id == self.repos[idx].id)
                {
                    cx.scene.draw_quad(
                        Quad::new(card_bounds)
                            .with_border(theme::accent::PRIMARY, 2.0)
                            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0)),
                    );
                }
            }
            y += height + LIST_GAP;
        }
    }

    fn paint_issue_list(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let total_height = self.issue_rows.len() as f32 * 80.0
            + (self.issue_rows.len().saturating_sub(1) as f32 * LIST_GAP);
        Self::sync_scroll(&mut self.issue_scroll, bounds, total_height);

        let mut y = bounds.origin.y - self.issue_scroll.scroll_offset.y;
        for (idx, row) in self.issue_rows.iter_mut().enumerate() {
            let row_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, 80.0);
            if row_bounds.max_y() >= bounds.min_y() - 60.0
                && row_bounds.min_y() <= bounds.max_y() + 60.0
            {
                row.paint(row_bounds, cx);
                if self
                    .selected_issue
                    .as_deref()
                    .is_some_and(|id| id == self.issues[idx].id)
                {
                    cx.scene.draw_quad(
                        Quad::new(row_bounds)
                            .with_border(theme::accent::PRIMARY, 2.0)
                            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0)),
                    );
                }
            }
            y += 80.0 + LIST_GAP;
        }
    }

    fn paint_pr_list(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let total_height = self.pr_rows.len() as f32 * PR_ROW_HEIGHT
            + (self.pr_rows.len().saturating_sub(1) as f32 * LIST_GAP);
        Self::sync_scroll(&mut self.pr_scroll, bounds, total_height);

        let mut y = bounds.origin.y - self.pr_scroll.scroll_offset.y;
        for (idx, row) in self.pr_rows.iter_mut().enumerate() {
            let row_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, PR_ROW_HEIGHT);
            if row_bounds.max_y() >= bounds.min_y() - 60.0
                && row_bounds.min_y() <= bounds.max_y() + 60.0
            {
                row.paint(row_bounds, cx);
                if self
                    .selected_pr
                    .as_deref()
                    .is_some_and(|id| id == self.prs[idx].id)
                {
                    cx.scene.draw_quad(
                        Quad::new(row_bounds)
                            .with_border(theme::accent::PRIMARY, 2.0)
                            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0)),
                    );
                }
            }
            y += PR_ROW_HEIGHT + LIST_GAP;
        }
    }

    fn paint_repo_detail(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let Some(repo_id) = self.selected_repo.as_deref() else {
            self.draw_text(
                "Select a repository to view details.",
                bounds.origin.x + PADDING,
                bounds.origin.y + 24.0,
                theme::font_size::SM,
                theme::text::MUTED,
                cx,
            );
            return;
        };

        let Some(repo) = self.repos.iter().find(|repo| repo.id == repo_id) else {
            return;
        };

        let mut y = bounds.origin.y + PADDING;
        self.draw_text(
            &repo.name,
            bounds.origin.x + PADDING,
            y,
            theme::font_size::LG,
            theme::text::PRIMARY,
            cx,
        );
        y += 26.0;

        if let Some(desc) = &repo.description {
            let desc_text = if desc.len() > 240 {
                format!("{}...", &desc[..237])
            } else {
                desc.clone()
            };
            self.draw_text(
                &desc_text,
                bounds.origin.x + PADDING,
                y,
                theme::font_size::SM,
                theme::text::MUTED,
                cx,
            );
            y += 24.0;
        }

        if let Some(language) = &repo.language {
            self.draw_text(
                &format!("Language: {}", language),
                bounds.origin.x + PADDING,
                y,
                theme::font_size::SM,
                theme::text::PRIMARY,
                cx,
            );
            y += 20.0;
        }

        self.draw_text(
            &format!("Author: {}", repo.pubkey),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::XS,
            theme::text::DISABLED,
            cx,
        );
        y += 18.0;

        self.draw_text(
            &format!("Address: {}", repo.address),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::XS,
            theme::text::DISABLED,
            cx,
        );
    }

    fn paint_issue_detail(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let Some(issue_id) = self.selected_issue.as_deref() else {
            self.draw_text(
                "Select an issue to view details.",
                bounds.origin.x + PADDING,
                bounds.origin.y + 24.0,
                theme::font_size::SM,
                theme::text::MUTED,
                cx,
            );
            return;
        };

        let Some(issue) = self.issues.iter().find(|issue| issue.id == issue_id) else {
            return;
        };

        let mut y = bounds.origin.y + PADDING;
        self.draw_text(
            &issue.title,
            bounds.origin.x + PADDING,
            y,
            theme::font_size::LG,
            theme::text::PRIMARY,
            cx,
        );
        y += 24.0;

        let status = issue_status_from_str(&issue.status);
        let status_text = format!("Status: {}", status.label());
        self.draw_text(
            &status_text,
            bounds.origin.x + PADDING,
            y,
            theme::font_size::SM,
            status.color(),
            cx,
        );
        y += 22.0;

        self.draw_text(
            &format!("Author: {}", issue.author),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::SM,
            theme::text::PRIMARY,
            cx,
        );
        y += 20.0;

        self.draw_text(
            &format!("Created: {}", issue.created_at),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::XS,
            theme::text::MUTED,
            cx,
        );
        y += 24.0;

        if let Some(bounty) = issue.bounty_sats {
            let badge_bounds = Bounds::new(bounds.origin.x + PADDING, y, 120.0, 24.0);
            let mut badge = BountyBadge::new(bounty);
            badge.paint(badge_bounds, cx);
            y += 32.0;
        }

        if !issue.content.is_empty() {
            let mut text = Text::new(&issue.content)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            let text_bounds = Bounds::new(
                bounds.origin.x + PADDING,
                y,
                bounds.size.width - PADDING * 2.0,
                bounds.size.height - y + bounds.origin.y - PADDING,
            );
            text.paint(text_bounds, cx);
        }
    }

    fn paint_pr_detail(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let Some(pr_id) = self.selected_pr.as_deref() else {
            self.draw_text(
                "Select a pull request to review.",
                bounds.origin.x + PADDING,
                bounds.origin.y + 24.0,
                theme::font_size::SM,
                theme::text::MUTED,
                cx,
            );
            return;
        };

        let Some(pr) = self.prs.iter().find(|pr| pr.id == pr_id) else {
            return;
        };

        let mut y = bounds.origin.y + PADDING;
        self.draw_text(
            &pr.title,
            bounds.origin.x + PADDING,
            y,
            theme::font_size::LG,
            theme::text::PRIMARY,
            cx,
        );
        y += 24.0;

        let status = pr_status_from_str(&pr.status);
        let status_text = format!("Status: {}", status.label());
        self.draw_text(
            &status_text,
            bounds.origin.x + PADDING,
            y,
            theme::font_size::SM,
            status.color(),
            cx,
        );
        y += 22.0;

        self.draw_text(
            &format!("Author: {}", pr.author),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::SM,
            theme::text::PRIMARY,
            cx,
        );
        y += 20.0;

        self.draw_text(
            &format!("Created: {}", pr.created_at),
            bounds.origin.x + PADDING,
            y,
            theme::font_size::XS,
            theme::text::MUTED,
            cx,
        );
        y += 20.0;

        if let Some(commit_id) = &pr.commit_id {
            self.draw_text(
                &format!("Commit: {}", commit_id),
                bounds.origin.x + PADDING,
                y,
                theme::font_size::XS,
                theme::text::DISABLED,
                cx,
            );
        }

        let diff_bounds = Bounds::new(
            bounds.origin.x + PADDING,
            bounds.origin.y + DETAIL_HEADER_HEIGHT,
            bounds.size.width - PADDING * 2.0,
            bounds.size.height - DETAIL_HEADER_HEIGHT - PADDING,
        );

        if self.pr_diff_id.as_deref() != Some(pr_id) || self.diff_tool_calls.is_empty() {
            let message = if self.pr_diff_id.as_deref() == Some(pr_id) {
                "Diff unavailable. Clone repo to view."
            } else {
                "Loading diff..."
            };
            self.draw_text(
                message,
                diff_bounds.origin.x + 12.0,
                diff_bounds.origin.y + 12.0,
                theme::font_size::SM,
                theme::text::MUTED,
                cx,
            );
            return;
        }

        let total_height = self
            .diff_tool_calls
            .iter()
            .map(|tool| tool.size_hint().1.unwrap_or(80.0))
            .sum::<f32>()
            + (self.diff_tool_calls.len().saturating_sub(1) as f32 * LIST_GAP);
        Self::sync_scroll(&mut self.diff_scroll, diff_bounds, total_height);

        let mut y = diff_bounds.origin.y - self.diff_scroll.scroll_offset.y;
        for tool in self.diff_tool_calls.iter_mut() {
            let height = tool.size_hint().1.unwrap_or(80.0);
            let tool_bounds = Bounds::new(diff_bounds.origin.x, y, diff_bounds.size.width, height);
            if tool_bounds.max_y() >= diff_bounds.min_y() - 60.0
                && tool_bounds.min_y() <= diff_bounds.max_y() + 60.0
            {
                tool.paint(tool_bounds, cx);
            }
            y += height + LIST_GAP;
        }
    }

    fn paint_notice(&self, layout: &GitafterLayout, cx: &mut wgpui::PaintContext) {
        let Some(notice) = &self.notice else {
            return;
        };

        let width = (notice.message.len() as f32 * 6.5).min(layout.header.size.width - 40.0) + 24.0;
        let bounds = Bounds::new(
            layout.header.origin.x + PADDING,
            layout.header.max_y() - 26.0,
            width,
            20.0,
        );
        let color = notice.color();
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(color.with_alpha(0.2))
                .with_border(color, 1.0),
        );
        let run = cx.text.layout(
            &notice.message,
            Point::new(bounds.origin.x + 8.0, bounds.origin.y + 3.0),
            theme::font_size::XS,
            color,
        );
        cx.scene.draw_text(run);
    }
}

impl Component for GitafterView {
    fn paint(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let layout = self.layout(bounds);

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP));
        self.paint_header(&layout, cx);
        self.paint_tabs(&layout, cx);

        match self.tab {
            GitafterTab::Repos => {
                self.paint_repo_list(layout.list, cx);
                self.paint_repo_detail(layout.detail, cx);
            }
            GitafterTab::Issues => {
                self.paint_issue_list(layout.list, cx);
                self.paint_issue_detail(layout.detail, cx);
            }
            GitafterTab::PullRequests => {
                self.paint_pr_list(layout.list, cx);
                self.paint_pr_detail(layout.detail, cx);
            }
        }

        self.paint_notice(&layout, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let layout = self.layout(bounds);
        let mut result = EventResult::Ignored;

        match event {
            InputEvent::MouseMove { x, y } => {
                self.cursor_position = Point::new(*x, *y);
                let hovered = if layout.repos_tab.contains(self.cursor_position) {
                    Some(GitafterTab::Repos)
                } else if layout.issues_tab.contains(self.cursor_position) {
                    Some(GitafterTab::Issues)
                } else if layout.prs_tab.contains(self.cursor_position) {
                    Some(GitafterTab::PullRequests)
                } else {
                    None
                };
                if hovered != self.hovered_tab {
                    self.hovered_tab = hovered;
                    result = EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    if layout.repos_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectTab(GitafterTab::Repos));
                        result = EventResult::Handled;
                    } else if layout.issues_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectTab(GitafterTab::Issues));
                        result = EventResult::Handled;
                    } else if layout.prs_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(GitafterUiEvent::SelectTab(GitafterTab::PullRequests));
                        result = EventResult::Handled;
                    }
                }
            }
            InputEvent::Scroll { dx, dy } => {
                let point = self.cursor_position;
                if layout.list.contains(point) {
                    match self.tab {
                        GitafterTab::Repos => {
                            self.repo_scroll.scroll_by(Point::new(*dx, *dy));
                        }
                        GitafterTab::Issues => {
                            self.issue_scroll.scroll_by(Point::new(*dx, *dy));
                        }
                        GitafterTab::PullRequests => {
                            self.pr_scroll.scroll_by(Point::new(*dx, *dy));
                        }
                    }
                    result = EventResult::Handled;
                } else if self.tab == GitafterTab::PullRequests {
                    let diff_bounds = Bounds::new(
                        layout.detail.origin.x + PADDING,
                        layout.detail.origin.y + DETAIL_HEADER_HEIGHT,
                        layout.detail.size.width - PADDING * 2.0,
                        layout.detail.size.height - DETAIL_HEADER_HEIGHT - PADDING,
                    );
                    if diff_bounds.contains(point) {
                        self.diff_scroll.scroll_by(Point::new(*dx, *dy));
                        result = EventResult::Handled;
                    }
                }
            }
            _ => {}
        }

        match self.tab {
            GitafterTab::Repos => {
                let mut y = layout.list.origin.y - self.repo_scroll.scroll_offset.y;
                for card in self.repo_cards.iter_mut() {
                    let height = card.size_hint().1.unwrap_or(90.0);
                    let card_bounds =
                        Bounds::new(layout.list.origin.x, y, layout.list.size.width, height);
                    result = result.or(card.event(event, card_bounds, cx));
                    y += height + LIST_GAP;
                }
            }
            GitafterTab::Issues => {
                let mut y = layout.list.origin.y - self.issue_scroll.scroll_offset.y;
                for row in self.issue_rows.iter_mut() {
                    let row_bounds =
                        Bounds::new(layout.list.origin.x, y, layout.list.size.width, 80.0);
                    result = result.or(row.event(event, row_bounds, cx));
                    y += 80.0 + LIST_GAP;
                }
            }
            GitafterTab::PullRequests => {
                let mut y = layout.list.origin.y - self.pr_scroll.scroll_offset.y;
                for row in self.pr_rows.iter_mut() {
                    let row_bounds = Bounds::new(
                        layout.list.origin.x,
                        y,
                        layout.list.size.width,
                        PR_ROW_HEIGHT,
                    );
                    result = result.or(row.event(event, row_bounds));
                    y += PR_ROW_HEIGHT + LIST_GAP;
                }
            }
        }

        if self.handle_ui_events() {
            result = EventResult::Handled;
        }

        result
    }
}

fn issue_status_from_str(status: &str) -> IssueStatus {
    match status.to_lowercase().as_str() {
        "closed" => IssueStatus::Closed,
        "claimed" => IssueStatus::Claimed,
        "in_progress" | "in progress" => IssueStatus::InProgress,
        "draft" => IssueStatus::Draft,
        _ => IssueStatus::Open,
    }
}

fn pr_status_from_str(status: &str) -> PrStatus {
    match status.to_lowercase().as_str() {
        "merged" => PrStatus::Merged,
        "closed" => PrStatus::Closed,
        "draft" => PrStatus::Draft,
        _ => PrStatus::Open,
    }
}

fn build_diff_tool_calls(diff_text: &str) -> Vec<DiffToolCall> {
    let parsed = parse_diff_lines(diff_text);
    let mut groups: Vec<(String, Vec<DiffLine>)> = Vec::new();

    for line in parsed {
        let file = if line.file_path.is_empty() {
            "diff".to_string()
        } else {
            line.file_path.clone()
        };
        if groups.last().map(|(f, _)| f != &file).unwrap_or(true) {
            groups.push((file.clone(), Vec::new()));
        }

        let kind = match line.line_type {
            DiffLineType::Addition => DiffLineKind::Addition,
            DiffLineType::Deletion => DiffLineKind::Deletion,
            DiffLineType::Context => DiffLineKind::Context,
            DiffLineType::Header => DiffLineKind::Header,
        };

        let content = match line.line_type {
            DiffLineType::Addition | DiffLineType::Deletion | DiffLineType::Context => {
                line.content.chars().skip(1).collect::<String>()
            }
            DiffLineType::Header => line.content.clone(),
        };

        groups
            .last_mut()
            .expect("diff group exists")
            .1
            .push(DiffLine {
                kind,
                content,
                old_line: None,
                new_line: None,
            });
    }

    groups
        .into_iter()
        .map(|(file, lines)| DiffToolCall::new(file).lines(lines))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_repo_triggers_loads() {
        let mut view = GitafterView::new(GitafterTab::Repos, None);
        view.drain_commands();

        let repos = vec![RepoSummary {
            id: "repo-1".to_string(),
            name: "Repo One".to_string(),
            description: None,
            language: None,
            pubkey: "abcd".to_string(),
            address: "30617:abcd:repo-1".to_string(),
            updated_at: "just now".to_string(),
        }];

        view.apply_update(GitafterUpdate::RepositoriesLoaded { repos });
        let commands = view.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, GitafterCommand::LoadIssues { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, GitafterCommand::LoadPullRequests { .. }))
        );
    }

    #[test]
    fn test_diff_builds_tool_calls() {
        let diff = "diff --git a/file.rs b/file.rs\n@@ -1,2 +1,2 @@\n- old\n+ new\n";
        let tools = build_diff_tool_calls(diff);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].file_path(), "file.rs");
        assert!(tools[0].get_additions() > 0);
    }
}
