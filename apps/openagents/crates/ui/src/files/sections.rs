//! The explorer's footer: a collapsible **Subagents** section docked under
//! the file tree (the spawn chips of the active chat's transcript, with their
//! live status). Rows borrow the left sidebar's compact session row — 29px,
//! status glyph, title, time — minus the harness, project and device icons,
//! which say nothing here. Clicking a row opens it in the right pane's
//! surface host; the section chrome animates with the same collapse motion
//! as the sidebar's disclosures.
//!
//! The footer has a fixed height budget that the section scrolls inside, and
//! like the sidebar's Archived shelf it shows ten rows before a "Show N more"
//! row pages by ten.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, EntityId, ScrollHandle,
    SharedString, div, prelude::*, px,
};
use openagents_doc::{MessagePart, SubagentStatus};
use openagents_proto::ChatIndicator;

use crate::icons::{self, icon};
use crate::state::AppState;
use crate::theme::Theme;
use crate::{loaders, motion};

use super::subagents::{Group, GroupState, SubagentPlan};
use super::{FilesEvent, FilesSurface};

const SECTION_HEADER_HEIGHT: f32 = 28.0;
const SECTION_BODY_INSET: f32 = 4.0;
const ROW_HEIGHT: f32 = 29.0;
const ROW_GAP: f32 = 2.0;
/// Empty state: the copy (two 16px lines so it can wrap in a narrow
/// explorer), left-aligned like the rows it stands in for.
const EMPTY_COPY_HEIGHT: f32 = 36.0;
const EMPTY_PAD: f32 = 10.0;
/// Fade band under a section list's edges (the sidebar's treatment, scaled
/// to the shorter lists).
const LIST_FADE_BAND: f32 = 16.0;
/// Hover group of a section header (reveals its actions).
const HEADER_GROUP: &str = "files-section-header";
/// How far each level of the Finished dropdown steps in.
const GROUP_INDENT: f32 = 10.0;
/// Rows a section shows before "Show more" pages it, and the page size —
/// the sidebar's Archived shelf numbers.
pub(super) const INITIAL_ROWS: usize = 10;
pub(super) const PAGE_ROWS: usize = 10;
/// An open section never shrinks below this, so one or two rows still
/// leave the section room to breathe.
const MIN_BODY_HEIGHT: f32 = 120.0;
const FOOTER_PAD_TOP: f32 = 4.0;
const FOOTER_PAD_BOTTOM: f32 = 6.0;
/// The footer's height budget; shorter content shrinks the footer to fit.
const FOOTER_HEIGHT: f32 = 510.0;
const TWEEN_GRACE: std::time::Duration = std::time::Duration::from_millis(120);

/// Which footer section a motion or toggle addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Section {
    Subagents,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::Subagents => "subagents",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Subagents => "Subagents",
        }
    }
}

/// One in-flight open/close of a section body (the sidebar's
/// `SidebarDisclosureMotion`, kept local so the explorer owns its own
/// epochs). A re-toggle mid-flight picks up from the painted height.
#[derive(Debug, Clone, Copy)]
struct DisclosureMotion {
    epoch: u64,
    from: f32,
    to: f32,
    started: std::time::Instant,
}

impl DisclosureMotion {
    fn current(self) -> f32 {
        let total = motion::COLLAPSE.total().as_secs_f32();
        let raw = if total > 0.0 {
            self.started.elapsed().as_secs_f32() / total
        } else {
            1.0
        };
        motion::lerp(self.from, self.to, motion::COLLAPSE.progress(raw))
    }

    fn animating(self) -> bool {
        self.started.elapsed() < motion::COLLAPSE.total() + TWEEN_GRACE
    }
}

/// Footer state on the explorer surface.
#[derive(Debug)]
pub(super) struct ExplorerSections {
    open: HashMap<Section, bool>,
    motion: HashMap<Section, DisclosureMotion>,
    /// One scroll handle per section list, so the edge fades can read
    /// overflow at paint time.
    scroll: HashMap<Section, ScrollHandle>,
    /// Open/closed and paging state of the Finished dropdown.
    groups: GroupState,
    /// Hash of what the footer would draw, so the state observer only
    /// re-renders the explorer when a section's contents actually changed —
    /// not on every streamed transcript delta.
    fingerprint: u64,
}

impl Default for ExplorerSections {
    fn default() -> Self {
        Self {
            open: [(Section::Subagents, true)].into_iter().collect(),
            motion: HashMap::new(),
            scroll: [(Section::Subagents, ScrollHandle::new())]
                .into_iter()
                .collect(),
            groups: GroupState::default(),
            fingerprint: 0,
        }
    }
}

impl ExplorerSections {
    pub(super) fn is_open(&self, section: Section) -> bool {
        self.open.get(&section).copied().unwrap_or(true)
    }

    fn toggle(&mut self, section: Section, resting: f32, target: f32) {
        let previous = self.motion.get(&section).copied();
        let from = previous
            .filter(|m| m.animating())
            .map(DisclosureMotion::current)
            .unwrap_or(resting);
        let epoch = previous.map_or(1, |m| m.epoch + 1);
        self.motion.insert(
            section,
            DisclosureMotion {
                epoch,
                from,
                to: target,
                started: std::time::Instant::now(),
            },
        );
        let open = self.is_open(section);
        self.open.insert(section, !open);
    }

    fn scroll(&self, section: Section) -> ScrollHandle {
        self.scroll
            .get(&section)
            .cloned()
            .unwrap_or_else(ScrollHandle::new)
    }

    fn live_motion(&self, section: Section) -> Option<DisclosureMotion> {
        self.motion.get(&section).copied().filter(|m| m.animating())
    }
}

/// A spawn chip of the active transcript, as the footer lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SubagentRow {
    pub doc_id: String,
    pub title: SharedString,
    pub status: Option<SubagentStatus>,
    /// When the latest turn that spawned (or steered) it was written — the
    /// closest thing a subagent has to a last-updated time.
    pub spawned_at: DateTime<Utc>,
}

impl SubagentRow {
    /// A settled subagent is frozen: the tab reads its blob first.
    pub fn frozen(&self) -> bool {
        matches!(
            self.status,
            Some(SubagentStatus::Done) | Some(SubagentStatus::Failed)
        )
    }

    fn indicator(&self) -> ChatIndicator {
        match self.status {
            Some(SubagentStatus::Running) => ChatIndicator::Working,
            Some(SubagentStatus::Done) => ChatIndicator::Completed,
            Some(SubagentStatus::Failed) => ChatIndicator::Errored,
            None => ChatIndicator::Idle,
        }
    }
}

/// The active chat's subagents, one row per subagent doc: running ones first,
/// longest-running leading, then the settled ones most recently updated first
/// (later spawns lead within one turn).
/// Only genuine spawn chips with a stamped doc ref qualify — the chip IS the
/// index (there is no listing endpoint), and a stray ref on a non-Agent tool
/// must not surface as a phantom subagent.
pub(super) fn subagent_rows(state: &AppState, chat_id: &str) -> Vec<SubagentRow> {
    if state.selected_chat.as_deref() != Some(chat_id) {
        return Vec::new();
    }
    let mut rows: Vec<SubagentRow> = Vec::new();
    for entry in &state.transcript {
        let spawned_at =
            DateTime::<Utc>::from_timestamp_millis(entry.created_at).unwrap_or_else(Utc::now);
        for part in &entry.parts {
            let MessagePart::Tool {
                call,
                subagent_ref: Some(doc_id),
                subagent_status,
                ..
            } = part
            else {
                continue;
            };
            if !call.is_subagent_spawn() {
                continue;
            }
            let row = SubagentRow {
                doc_id: doc_id.clone(),
                title: crate::transcript::subagent_tab_title(call),
                status: *subagent_status,
                spawned_at,
            };
            match rows.iter_mut().find(|r| r.doc_id == row.doc_id) {
                // A reopened (steered) subagent updates its row in place.
                Some(existing) => *existing = row,
                None => rows.push(row),
            }
        }
    }
    // Stable sort over the reversed spawn order: ties keep the later spawn
    // on top.
    rows.reverse();
    rows.sort_by_key(|row| std::cmp::Reverse(row.spawned_at));
    // Running subagents lead, longest-running first: one started long ago is
    // buried under everything spawned since, and is the one to find and steer.
    // Oldest-first also keeps the group still as new subagents join at its
    // foot. The settled tail keeps the newest-first order above.
    let (mut running, settled): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .partition(|row| row.status == Some(SubagentStatus::Running));
    // Ties keep spawn order, like the group's oldest-first order.
    running.reverse();
    running.sort_by_key(|row| row.spawned_at);
    running.extend(settled);
    running
}

/// What the footer would draw for `chat_id`, hashed. Cheap enough to run on
/// every state notification.
pub(super) fn fingerprint(state: &AppState, chat_id: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in subagent_rows(state, chat_id) {
        row.doc_id.hash(&mut hasher);
        row.title.as_ref().hash(&mut hasher);
        (row.status.map(|s| s as u8)).hash(&mut hasher);
    }
    hasher.finish()
}

/// The height a section body wants for `count` rows with `shown` revealed:
/// inset, the visible rows, and a "Show more" row while more remain. Empty
/// sections want their empty-state copy.
pub(super) fn content_height(count: usize, shown: usize) -> f32 {
    content_height_unfloored(count, shown).max(MIN_BODY_HEIGHT)
}

fn content_height_unfloored(count: usize, shown: usize) -> f32 {
    if count == 0 {
        return SECTION_BODY_INSET + EMPTY_PAD * 2.0 + EMPTY_COPY_HEIGHT;
    }
    let visible = count.min(shown);
    let more = if count > shown { 1 } else { 0 };
    rows_height(visible + more)
}

/// The height of a list of `slots` row-sized items: the inset, the rows and
/// the gaps between them.
fn rows_height(slots: usize) -> f32 {
    SECTION_BODY_INSET + slots as f32 * ROW_HEIGHT + slots.saturating_sub(1) as f32 * ROW_GAP
}

/// The Subagents body's wanted height: its rows and group headers, or the
/// empty-state copy when nothing is left to list.
fn subagents_height(plan: &SubagentPlan, groups: &GroupState) -> f32 {
    match plan.slots(groups) {
        0 => content_height(0, INITIAL_ROWS),
        slots => rows_height(slots).max(MIN_BODY_HEIGHT),
    }
}

/// The footer's chrome outside the body: padding and the header.
fn chrome_height() -> f32 {
    FOOTER_PAD_TOP + FOOTER_PAD_BOTTOM + SECTION_HEADER_HEIGHT
}

impl FilesSurface {
    /// Re-render only when the footer's contents changed.
    pub(super) fn refresh_sections(&mut self, cx: &mut Context<Self>) {
        let fingerprint = fingerprint(self.state.read(cx), &self.chat_id);
        if fingerprint != self.sections.fingerprint {
            self.sections.fingerprint = fingerprint;
            cx.notify();
        }
    }

    pub(super) fn render_sections(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let subagents = subagent_rows(self.state.read(cx), &self.chat_id);
        self.sections.fingerprint = fingerprint(self.state.read(cx), &self.chat_id);
        let plan = SubagentPlan::new(subagents);
        let wanted = subagents_height(&plan, &self.sections.groups);
        let height = if self.sections.is_open(Section::Subagents) {
            wanted.min((FOOTER_HEIGHT - chrome_height()).max(0.0))
        } else {
            0.0
        };
        let view = cx.entity_id();
        let subagent_body = self.render_subagent_rows(&plan, view, theme, cx);
        let subagent_badge = (plan.running() > 0).then(|| {
            crate::running_pill::running_pill(
                "files-subagents-header",
                plan.running() as u32,
                theme,
            )
        });
        div()
            .id("files-sections")
            .relative()
            .flex_none()
            .w_full()
            .flex()
            .flex_col()
            .px(px(6.0))
            .pt(px(FOOTER_PAD_TOP))
            .pb(px(FOOTER_PAD_BOTTOM))
            .child(self.render_section(
                Section::Subagents,
                plan.total(),
                wanted,
                height,
                None,
                subagent_badge,
                subagent_body,
                theme,
                cx,
            ))
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_section(
        &mut self,
        section: Section,
        count: usize,
        wanted: f32,
        height: f32,
        actions: Option<AnyElement>,
        badge: Option<AnyElement>,
        body: AnyElement,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.sections.is_open(section);
        // The collapsed header carries the count; open, the rows speak.
        let label: SharedString = if open || count == 0 {
            section.label().into()
        } else {
            format!("{} ({count})", section.label()).into()
        };
        // Toggling animates between 0 and the height this section gets from
        // the budget (the body scrolls inside it); a closed section reopens
        // to what it wants within the whole budget.
        let full = if open {
            height
        } else {
            wanted.min(FOOTER_HEIGHT - chrome_height()).max(0.0)
        };
        let header = div()
            .id(SharedString::from(format!(
                "files-section-{}",
                section.key()
            )))
            .group(HEADER_GROUP)
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(format!(
                "{} {}",
                if open { "Collapse" } else { "Expand" },
                section.label()
            )))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(SECTION_HEADER_HEIGHT))
            .pl(px(Theme::SPACE_SM))
            .pr(px(4.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                // Open → close runs from the painted body height to 0, and
                // back up to what the budget allows.
                let was_open = this.sections.is_open(section);
                let (resting, target) = if was_open { (full, 0.0) } else { (0.0, full) };
                this.sections.toggle(section, resting, target);
                cx.notify();
            }))
            .child(
                // The badge trails the title, not the caret: the title
                // truncates first, the badge never does.
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(crate::typography::ui_rems(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text_muted.opacity(0.5))
                            .child(label),
                    )
                    .children(badge),
            )
            .children(actions)
            .child(self.render_chevron(section, open, theme));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .child(header)
            .child(self.render_disclosure_body(section, open, height, body))
            .into_any_element()
    }

    fn render_chevron(&self, section: Section, open: bool, theme: &Theme) -> AnyElement {
        let chevron = icon(icons::ALT_ARROW_RIGHT)
            .size(px(12.0))
            .text_color(theme.text_muted.opacity(0.5));
        let frame = div()
            .flex_none()
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center();
        if let Some(tween) = self.sections.live_motion(section) {
            let denominator = tween.from.max(tween.to).max(1.0);
            let from = (tween.from / denominator).clamp(0.0, 1.0);
            let to = (tween.to / denominator).clamp(0.0, 1.0);
            frame
                .child(chevron.with_animation(
                    SharedString::from(format!(
                        "files-section-chevron-{}-{}",
                        section.key(),
                        tween.epoch
                    )),
                    collapse_animation(),
                    move |el, t| {
                        let reveal = motion::lerp(from, to, t);
                        el.with_transformation(gpui::Transformation::rotate(gpui::percentage(
                            reveal * 0.25,
                        )))
                    },
                ))
                .into_any_element()
        } else {
            let resting = if open { 0.25 } else { 0.0 };
            frame
                .child(
                    chevron.with_transformation(gpui::Transformation::rotate(gpui::percentage(
                        resting,
                    ))),
                )
                .into_any_element()
        }
    }

    fn render_disclosure_body(
        &self,
        section: Section,
        open: bool,
        height: f32,
        content: AnyElement,
    ) -> AnyElement {
        let target = if open { height } else { 0.0 };
        let frame = div().w_full().flex_none().overflow_hidden().child(content);
        let Some(tween) = self.sections.live_motion(section) else {
            return frame.h(px(target)).into_any_element();
        };
        let denominator = tween.from.max(tween.to).max(1.0);
        frame
            .with_animation(
                SharedString::from(format!(
                    "files-section-body-{}-{}",
                    section.key(),
                    tween.epoch
                )),
                collapse_animation(),
                move |el, t| {
                    let height = motion::lerp(tween.from, tween.to, t);
                    let reveal = (height / denominator).clamp(0.0, 1.0);
                    el.h(px(height))
                        .opacity(0.35 + 0.65 * reveal)
                        .relative()
                        .top(px(-3.0 * (1.0 - reveal)))
                },
            )
            .into_any_element()
    }

    fn render_subagent_rows(
        &self,
        plan: &SubagentPlan,
        view: EntityId,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if plan.total() == 0 {
            return empty_state(
                "Subagents will appear here when they are created",
                None,
                theme,
            );
        }
        let now = Utc::now();
        let scroll = self.sections.scroll(Section::Subagents);
        let mut list = row_list("files-subagent-rows", &scroll);
        for row in &plan.active {
            list = list.child(self.render_subagent_row(row, 0.0, now, view, theme, cx));
        }
        if plan.finished() > 0 {
            list = list.child(self.render_group_header(
                Group::Finished,
                plan.finished(),
                0.0,
                theme,
                cx,
            ));
            if self.sections.groups.is_open(Group::Finished) {
                for group in [Group::Completed, Group::Failed] {
                    let rows = plan.rows(group);
                    if rows.is_empty() {
                        continue;
                    }
                    list = list.child(self.render_group_header(
                        group,
                        rows.len(),
                        GROUP_INDENT,
                        theme,
                        cx,
                    ));
                    if !self.sections.groups.is_open(group) {
                        continue;
                    }
                    let shown = self.sections.groups.shown(group);
                    for row in rows.iter().take(shown) {
                        list = list.child(self.render_subagent_row(
                            row,
                            2.0 * GROUP_INDENT,
                            now,
                            view,
                            theme,
                            cx,
                        ));
                    }
                    if rows.len() > shown {
                        list = list.child(self.render_group_show_more(
                            group,
                            rows.len() - shown,
                            2.0 * GROUP_INDENT,
                            theme,
                            cx,
                        ));
                    }
                }
            }
        }
        faded_list(list, &scroll)
    }

    /// One subagent: status glyph, title, and the time it was last updated.
    fn render_subagent_row(
        &self,
        row: &SubagentRow,
        indent: f32,
        now: DateTime<Utc>,
        view: EntityId,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let glyph = status_glyph(
            format!("files-subagent-{}", row.doc_id),
            row.indicator(),
            view,
            theme,
            cx,
        );
        let open = row.clone();
        compact_row(format!("files-subagent-{}", row.doc_id), theme)
            .pl(px(Theme::SPACE_SM + indent))
            .aria_label(SharedString::from(format!("Open subagent {}", row.title)))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                cx.emit(FilesEvent::OpenSubagent {
                    doc_id: open.doc_id.clone(),
                    title: open.title.to_string(),
                    frozen: open.frozen(),
                });
            }))
            .child(glyph)
            .child(row_title(
                format!("files-subagent-title-{}", row.doc_id),
                row.title.clone(),
            ))
            .child(time_ago_label(
                openagents_proto::view::format_time_ago(row.spawned_at, now).into(),
                theme,
            ))
            .into_any_element()
    }

    /// A dropdown header inside the Subagents body: caret and
    /// "Label (count)". Clicking toggles the group.
    fn render_group_header(
        &self,
        group: Group,
        count: usize,
        indent: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.sections.groups.is_open(group);
        let chevron = icon(icons::ALT_ARROW_RIGHT)
            .size(px(12.0))
            .text_color(theme.text_muted.opacity(0.5))
            .with_transformation(gpui::Transformation::rotate(gpui::percentage(if open {
                0.25
            } else {
                0.0
            })));
        div()
            .id(SharedString::from(format!(
                "files-subagent-group-{}",
                group.key()
            )))
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(format!(
                "{} {} subagents",
                if open { "Collapse" } else { "Expand" },
                group.label()
            )))
            .flex_none()
            .h(px(ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .rounded(px(8.0))
            .pl(px(Theme::SPACE_SM + indent - 4.0))
            .pr(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(theme.glass_hover()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.sections.groups.toggle(group);
                cx.notify();
            }))
            .child(
                div()
                    .flex_none()
                    .size(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(chevron),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(crate::typography::ui_rems(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted.opacity(0.7))
                    .child(SharedString::from(format!("{} ({count})", group.label()))),
            )
            .into_any_element()
    }

    /// "Show N more" under one finished category, indented with its rows.
    fn render_group_show_more(
        &self,
        group: Group,
        remaining: usize,
        indent: f32,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        show_more_row(
            format!("files-subagent-group-{}-more", group.key()),
            remaining,
            indent,
            theme,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.sections.groups.page_up(group);
            cx.notify();
        }))
        .into_any_element()
    }

}

fn collapse_animation() -> Animation {
    motion::COLLAPSE.animation()
}

/// A section's "Show N more" row (a page is [`PAGE_ROWS`]), indented to line
/// up with the rows it pages.
fn show_more_row(
    id: String,
    remaining: usize,
    indent: f32,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .role(gpui::Role::Button)
        .flex_none()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .pl(px(Theme::SPACE_SM + indent))
        .pr(px(Theme::SPACE_SM))
        .rounded(px(8.0))
        .cursor_pointer()
        .text_size(crate::typography::ui_rems(12.0))
        .text_color(theme.text_muted.opacity(0.7))
        .hover(|s| s.bg(theme.glass_hover()).text_color(theme.text_muted))
        .child(format!("Show {} more", remaining.min(PAGE_ROWS)))
}

/// The scrolling column an open section's rows live in; the body frame
/// sets the height, the list fills it and reports its overflow through
/// `scroll` for the edge fades.
fn row_list(id: &'static str, scroll: &ScrollHandle) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size_full()
        .flex()
        .flex_col()
        .gap(px(ROW_GAP))
        .pt(px(SECTION_BODY_INSET))
        .overflow_y_scroll()
        .track_scroll(scroll)
}

/// A section list under the sidebar's overflow fades: rows dissolve at the
/// edges while there is more to scroll to, plain when everything fits.
fn faded_list(list: gpui::Stateful<gpui::Div>, scroll: &ScrollHandle) -> AnyElement {
    crate::edge_fade::edge_faded(
        LIST_FADE_BAND,
        true,
        true,
        div().relative().size_full().child(list),
    )
    .fade_overflow_y(scroll)
    .into_any_element()
}

/// An empty section: its copy where the first row would sit, and any
/// actions on a row below it.
fn empty_state(copy: &'static str, actions: Option<AnyElement>, theme: &Theme) -> AnyElement {
    div()
        .pt(px(SECTION_BODY_INSET + EMPTY_PAD))
        .pb(px(EMPTY_PAD))
        .px(px(Theme::SPACE_SM))
        .flex()
        .flex_col()
        .child(
            div()
                .min_h(px(EMPTY_COPY_HEIGHT))
                .max_h(px(EMPTY_COPY_HEIGHT))
                .overflow_hidden()
                .py(px(2.0))
                .text_size(crate::typography::ui_rems(12.0))
                .line_height(px(16.0))
                .text_color(theme.text_muted.opacity(0.5))
                .child(copy),
        )
        .children(actions)
        .into_any_element()
}

/// The sidebar's compact session row, stripped to status + title (+ time):
/// 29px, 8px radius, the glass hover wash, 13px title on a 17px line.
fn compact_row(id: String, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .role(gpui::Role::Button)
        .flex_none()
        .h(px(ROW_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .rounded(px(8.0))
        .px(px(Theme::SPACE_SM))
        .cursor_pointer()
        .text_color(theme.text.opacity(0.8))
        .hover(|s| s.bg(theme.glass_hover()).text_color(theme.text))
}

/// The compact row's trailing time, 11px in the faded subline color.
fn time_ago_label(text: SharedString, theme: &Theme) -> gpui::Div {
    div()
        .flex_none()
        .text_size(crate::typography::ui_rems(11.0))
        .text_color(theme.text_muted.opacity(0.5))
        .child(text)
}

/// The sidebar's fading label: overflow dissolves at the right edge instead
/// of an ellipsis.
fn row_title(id: String, title: SharedString) -> impl IntoElement {
    crate::shell::sidebar_faded_label(
        id.into(),
        true,
        div()
            .text_size(crate::typography::ui_rems(13.0))
            .line_height(px(17.0))
            .child(title),
    )
}

/// The compact row's 13px status slot: Working animates the glyph spinner,
/// Completed wears the check, the rest a 6px dot in the status color.
fn status_glyph(
    key: String,
    status: ChatIndicator,
    view: EntityId,
    theme: &Theme,
    cx: &mut gpui::App,
) -> AnyElement {
    let color = crate::shell::spaces::status_dot_color(status, theme);
    let glyph: AnyElement = match status {
        ChatIndicator::Completed => icon(icons::CHECK)
            .size(px(11.0))
            .flex_none()
            .text_color(color)
            .into_any_element(),
        ChatIndicator::Working => {
            loaders::mini_glyph_spinner(format!("{key}-working"), 2.0, theme.glyph, view, cx)
                .into_any_element()
        }
        _ => div()
            .size(px(6.0))
            .flex_none()
            .rounded_full()
            .bg(color)
            .into_any_element(),
    };
    div()
        .flex_none()
        .size(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_doc::{MessageRole, MessageStatus, SessionMessageEntry};
    use openagents_proto::ToolCall;

    fn spawn(
        id: &str,
        name: &str,
        doc: Option<&str>,
        status: Option<SubagentStatus>,
    ) -> MessagePart {
        MessagePart::Tool {
            id: id.into(),
            call: ToolCall::Unknown {
                name: name.into(),
                input: Some(serde_json::json!({ "description": "verify the marker pipeline" })),
            },
            is_error: false,
            resolved: true,
            output: None,
            diff: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            diff_stats: None,
            subagent_ref: doc.map(str::to_owned),
            subagent_status: status,
            subagent_tail: None,
        }
    }

    fn entry(parts: Vec<MessagePart>) -> SessionMessageEntry {
        SessionMessageEntry {
            duration_ms: None,
            id: "e1".into(),
            role: MessageRole::Assistant,
            parts,
            created_at: (Utc::now() - chrono::Duration::minutes(3)).timestamp_millis(),
            device_id: "dev".into(),
            status: Some(MessageStatus::Complete),
            continuation_of: None,
        }
    }

    #[test]
    fn subagent_rows_list_only_stamped_spawn_chips_of_the_selected_chat() {
        let mut state = AppState::new();
        state.selected_chat = Some("main".into());
        state.transcript = vec![entry(vec![
            spawn(
                "t1",
                "Agent: verify",
                Some("main--sub--t1"),
                Some(SubagentStatus::Running),
            ),
            // No doc ref yet: the engine stamps it asynchronously.
            spawn("t2", "Agent: later", None, None),
            // A stray ref on a non-spawn tool never surfaces.
            spawn(
                "t3",
                "Read",
                Some("main--sub--t3"),
                Some(SubagentStatus::Done),
            ),
            spawn(
                "t4",
                "Agent: done",
                Some("main--sub--t4"),
                Some(SubagentStatus::Done),
            ),
        ])];
        let rows = subagent_rows(&state, "main");
        assert_eq!(
            rows.iter().map(|r| r.doc_id.as_str()).collect::<Vec<_>>(),
            ["main--sub--t1", "main--sub--t4"]
        );
        // The bare task, genus stripped — the same title the tab wears.
        assert_eq!(rows[0].title.as_ref(), "verify");
        assert!(rows[1].frozen());
        assert!(!rows[0].frozen());
        // Spawn time comes from the turn that carried the chip.
        assert!((Utc::now() - rows[0].spawned_at).num_minutes() >= 2);
        // Another chat's explorer sees nothing of this transcript.
        assert!(subagent_rows(&state, "other").is_empty());
    }

    #[test]
    fn subagent_rows_are_most_recently_updated_first() {
        let at = |id: &str, minutes_ago: i64, parts| SessionMessageEntry {
            id: id.into(),
            created_at: (Utc::now() - chrono::Duration::minutes(minutes_ago)).timestamp_millis(),
            ..entry(parts)
        };
        let mut state = AppState::new();
        state.selected_chat = Some("main".into());
        state.transcript = vec![
            at(
                "e1",
                30,
                vec![
                    spawn(
                        "a",
                        "Agent: a",
                        Some("main--sub--a"),
                        Some(SubagentStatus::Done),
                    ),
                    spawn(
                        "b",
                        "Agent: b",
                        Some("main--sub--b"),
                        Some(SubagentStatus::Done),
                    ),
                ],
            ),
            at(
                "e2",
                20,
                vec![spawn(
                    "c",
                    "Agent: c",
                    Some("main--sub--c"),
                    Some(SubagentStatus::Done),
                )],
            ),
            // `a` is steered again: its row moves up with the newer turn.
            at(
                "e3",
                10,
                vec![spawn(
                    "a",
                    "Agent: a",
                    Some("main--sub--a"),
                    Some(SubagentStatus::Running),
                )],
            ),
        ];
        let rows = subagent_rows(&state, "main");
        assert_eq!(
            rows.iter().map(|r| r.doc_id.as_str()).collect::<Vec<_>>(),
            ["main--sub--a", "main--sub--c", "main--sub--b"]
        );
        assert_eq!(rows[0].status, Some(SubagentStatus::Running));
    }

    #[test]
    fn running_subagents_lead_longest_running_first() {
        let at = |id: &str, minutes_ago: i64, parts| SessionMessageEntry {
            id: id.into(),
            created_at: (Utc::now() - chrono::Duration::minutes(minutes_ago)).timestamp_millis(),
            ..entry(parts)
        };
        let sub = |id: &str, status| {
            spawn(
                id,
                &format!("Agent: {id}"),
                Some(&format!("main--sub--{id}")),
                Some(status),
            )
        };
        let mut state = AppState::new();
        state.selected_chat = Some("main".into());
        state.transcript = vec![
            at("e1", 90, vec![sub("old-run", SubagentStatus::Running)]),
            at("e2", 60, vec![sub("old-done", SubagentStatus::Done)]),
            at("e3", 30, vec![sub("mid-run", SubagentStatus::Running)]),
            at("e4", 20, vec![sub("mid-fail", SubagentStatus::Failed)]),
            at("e5", 10, vec![sub("new-done", SubagentStatus::Done)]),
            at("e6", 5, vec![sub("new-run", SubagentStatus::Running)]),
        ];
        let ids: Vec<_> = subagent_rows(&state, "main")
            .into_iter()
            .map(|r| r.doc_id)
            .collect();
        // Running first, longest-running leading; the settled tail keeps the
        // newest-first order it always had.
        assert_eq!(
            ids,
            [
                "main--sub--old-run",
                "main--sub--mid-run",
                "main--sub--new-run",
                "main--sub--new-done",
                "main--sub--mid-fail",
                "main--sub--old-done",
            ]
        );
    }

    #[test]
    fn content_height_pages_at_ten_rows_and_counts_the_show_more_row() {
        // Short lists are floored so a section keeps its presence.
        let one = content_height(1, INITIAL_ROWS);
        assert_eq!(one, MIN_BODY_HEIGHT);
        let ten = content_height(10, INITIAL_ROWS);
        // Eleven rows: ten visible plus the "Show more" slot.
        let eleven = content_height(11, INITIAL_ROWS);
        assert_eq!(eleven - ten, ROW_HEIGHT + ROW_GAP);
        // Paging once reveals up to 20 rows before the next "Show more".
        let paged = content_height(40, INITIAL_ROWS + PAGE_ROWS);
        assert_eq!(
            paged,
            SECTION_BODY_INSET + 21.0 * ROW_HEIGHT + 20.0 * ROW_GAP
        );
        assert!(content_height(0, INITIAL_ROWS) >= MIN_BODY_HEIGHT);
    }
}
