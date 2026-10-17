//! The account's saved chats in the one Sessions list (#11960).
//!
//! Web, terminal and phone chats from the account join this computer's
//! sessions by last activity, and archived ones join Archived. A saved chat
//! that a later part continues shows once, as that later part (#11898).
//! Opening a row opens the normal chat view and starts nothing (#11892).
use super::*;
use openagents_account::history::LoadState;
use std::collections::HashSet;

/// Start loading the next page while the list end is this close below view.
const LOAD_AHEAD: f32 = 600.0;
/// Space above a device or project group (the sidebar's section gap).
const GROUP_GAP: f32 = 12.0;

/// One saved chat as the sidebar draws it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct AccountRow {
    pub id: String,
    pub title: String,
    pub updated_unix: u64,
    pub archived: bool,
    /// "Web", "Terminal" or "Phone".
    pub source: Option<&'static str>,
    /// The project's name.
    pub project: Option<String>,
    /// A later part continues this chat.
    pub continued: bool,
}

impl AccountRow {
    fn new(row: &openagents_account::openagents::ChatSummary, project: Option<String>) -> Self {
        use openagents_account::openagents::ChatSource;
        Self {
            id: row.chat.id().to_owned(),
            title: row.title.clone(),
            updated_unix: row.updated_unix,
            archived: row.archived_unix.is_some(),
            source: match row.source {
                ChatSource::Web => Some("Web"),
                ChatSource::Terminal => Some("Terminal"),
                ChatSource::Phone => Some("Phone"),
                ChatSource::Other(_) => None,
            },
            project,
            continued: row.links.continued_by.is_some(),
        }
    }

    /// The small source marker: "Web · Video Lab".
    pub(super) fn marker(&self) -> String {
        self.source
            .into_iter()
            .map(str::to_owned)
            .chain(self.project.clone())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    fn time(&self) -> chrono::DateTime<Utc> {
        chrono::DateTime::from_timestamp(self.updated_unix.min(i64::MAX as u64) as i64, 0)
            .unwrap_or_default()
    }
}

/// Keep only the latest part of each linked chat: drop saved chats that a
/// later part continues, on the web or in a session here, and saved copies of
/// sessions this computer already lists.
pub(super) fn latest_parts(
    rows: Vec<AccountRow>,
    local: &[openagents_proto::Chat],
) -> Vec<AccountRow> {
    let superseded: HashSet<&str> = local
        .iter()
        .flat_map(|chat| {
            std::iter::once(chat.id.as_str())
                .chain(chat.links.continues.as_ref().map(|part| part.id.as_str()))
        })
        .collect();
    rows.into_iter()
        .filter(|row| !row.continued && !superseded.contains(row.id.as_str()))
        .collect()
}

pub(super) enum Merged<L> {
    Local(L),
    Account(AccountRow),
}

/// Merge two newest-first lists by activity time; local rows win ties.
pub(super) fn merge_by_activity<L>(
    local: Vec<(i64, L)>,
    account: Vec<AccountRow>,
) -> Vec<Merged<L>> {
    let mut account = account.into_iter().peekable();
    let mut merged = Vec::new();
    for (time, row) in local {
        while let Some(next) = account.next_if(|next| next.time().timestamp() > time) {
            merged.push(Merged::Account(next));
        }
        merged.push(Merged::Local(row));
    }
    merged.extend(account.map(Merged::Account));
    merged
}

/// Fixture rows stand in for the signed-in account's history.
#[cfg(any(test, feature = "account-fixture"))]
pub(super) struct AccountRowsFixture(pub Vec<AccountRow>);
#[cfg(any(test, feature = "account-fixture"))]
impl gpui::Global for AccountRowsFixture {}

impl Shell {
    /// A local chat's time in the sidebar's chosen order.
    pub(super) fn sidebar_activity(&self, chat: &openagents_proto::Chat) -> i64 {
        match self.settings.sidebar_sort {
            SidebarSort::Created => chat.created_at.timestamp(),
            SidebarSort::LastUpdated => chat.last_message_at.unwrap_or(chat.created_at).timestamp(),
        }
    }

    /// The account's chats for one shelf, newest first, under the sidebar's
    /// project filter.
    pub(super) fn account_rows(&self, archived: bool, cx: &App) -> Vec<AccountRow> {
        #[cfg(any(test, feature = "account-fixture"))]
        let fixture = cx
            .try_global::<AccountRowsFixture>()
            .map(|rows| rows.0.clone());
        #[cfg(not(any(test, feature = "account-fixture")))]
        let fixture: Option<Vec<AccountRow>> = None;
        let rows = match fixture {
            Some(rows) => rows
                .into_iter()
                .filter(|row| row.archived == archived)
                .collect(),
            None => {
                let Some(panel) = &self.saved_history else {
                    return Vec::new();
                };
                let snapshot = &panel.read(cx).snapshot;
                if !snapshot.signed_in {
                    return Vec::new();
                }
                snapshot
                    .rows
                    .iter()
                    .filter(|row| row.archived_unix.is_some() == archived)
                    .map(|row| {
                        let project = row.project.as_ref().and_then(|id| {
                            snapshot
                                .projects
                                .iter()
                                .find(|p| p.id() == id)
                                .map(|p| p.name.clone())
                        });
                        AccountRow::new(row, project)
                    })
                    .collect()
            }
        };
        let state = self.state.read(cx);
        let filter = self.settings.space_filter.as_deref().and_then(|id| {
            state.spaces.iter().find(|s| s.id == id).map(|s| {
                s.name
                    .clone()
                    .unwrap_or_else(|| {
                        s.path
                            .rsplit(['/', '\\'])
                            .find(|p| !p.is_empty())
                            .unwrap_or_default()
                            .to_owned()
                    })
                    .trim()
                    .to_lowercase()
            })
        });
        let mut rows: Vec<AccountRow> = latest_parts(rows, &state.chats)
            .into_iter()
            .filter(|row| {
                filter.as_ref().is_none_or(|name| {
                    row.project
                        .as_ref()
                        .is_some_and(|project| project.trim().to_lowercase() == *name)
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            b.updated_unix
                .cmp(&a.updated_unix)
                .then_with(|| a.id.cmp(&b.id))
        });
        rows
    }

    /// Add the account's chats to this computer's session rows: by activity in
    /// one list, or as their own groups when the sidebar groups sessions.
    pub(super) fn merge_account_rows(
        &mut self,
        rows: &mut SidebarSessionRows,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) {
        let account = self.account_rows(false, cx);
        if account.is_empty() {
            return;
        }
        let height = self.account_row_height();
        if self.settings.sidebar_organization == SidebarOrganization::InOneList {
            let start = rows.pinned_count + rows.custom_count;
            let mut regular = rows.rows.split_off(start.min(rows.rows.len()));
            // A drag's trailing gap stays last.
            let tail = regular
                .iter()
                .rposition(|(key, _, _)| key.starts_with("c:"))
                .map_or(0, |last| last + 1);
            let tail = regular.split_off(tail);
            let local = {
                let state = self.state.read(cx);
                regular
                    .into_iter()
                    .map(|row| {
                        let time = row
                            .0
                            .strip_prefix("c:")
                            .and_then(|id| state.chats.iter().find(|chat| chat.id == id))
                            .map_or(i64::MAX, |chat| self.sidebar_activity(chat));
                        (time, row)
                    })
                    .collect()
            };
            rows.regular_count += account.len();
            for item in merge_by_activity(local, account) {
                rows.rows.push(match item {
                    Merged::Local(row) => row,
                    Merged::Account(row) => (
                        format!("a:{}", row.id),
                        height,
                        self.render_account_row(&row, theme, cx),
                    ),
                });
            }
            rows.rows.extend(tail);
            return;
        }
        let mut groups: Vec<(String, Vec<AccountRow>)> = Vec::new();
        for row in account {
            let label = match self.settings.sidebar_organization {
                SidebarOrganization::ByProject => row.project.clone(),
                _ => None,
            }
            .or_else(|| row.source.map(str::to_owned))
            .unwrap_or_else(|| "OpenAgents".into());
            match groups.iter_mut().find(|(group, _)| *group == label) {
                Some((_, rows)) => rows.push(row),
                None => groups.push((label, vec![row])),
            }
        }
        for (label, group) in groups {
            let key = format!("account:{label}");
            let motion_key = format!("group:{key}");
            let collapsed = self.sidebar_collapsed_groups.contains(&key);
            let count = group.len();
            let body_height = spaces::SIDEBAR_DISCLOSURE_BODY_INSET
                + count as f32 * height
                + SIDEBAR_LIST_GAP * count.saturating_sub(1) as f32;
            self.begin_queued_sidebar_reveal(&motion_key, !collapsed, body_height);
            let items: Vec<AnyElement> = group
                .iter()
                .map(|row| self.render_account_row(row, theme, cx))
                .collect();
            let body = div()
                .w_full()
                .flex()
                .flex_col()
                .pt(px(spaces::SIDEBAR_DISCLOSURE_BODY_INSET))
                .gap(px(SIDEBAR_LIST_GAP))
                .children(items)
                .into_any_element();
            let chevron = self.sidebar_disclosure_chevron(&motion_key, !collapsed, theme);
            let toggle_key = key.clone();
            let toggle_motion_key = motion_key.clone();
            let header = spaces::sidebar_disclosure_header(
                theme,
                None,
                label.into(),
                collapsed.then_some(count),
                None,
                chevron,
            )
            .id(SharedString::from(format!("sidebar-group-{key}")))
            .on_click(cx.listener(move |this, _, _, cx| {
                let was_open = !this.sidebar_collapsed_groups.contains(&toggle_key);
                this.begin_sidebar_disclosure_motion(
                    &toggle_motion_key,
                    if was_open { body_height } else { 0.0 },
                    if was_open { 0.0 } else { body_height },
                );
                if was_open {
                    this.sidebar_collapsed_groups.insert(toggle_key.clone());
                } else {
                    this.sidebar_collapsed_groups.remove(&toggle_key);
                }
                cx.notify();
            }));
            let body =
                self.render_sidebar_disclosure_body(&motion_key, !collapsed, body_height, body);
            rows.rows.push((
                format!("g:{key}"),
                GROUP_GAP
                    + spaces::SIDEBAR_DISCLOSURE_HEADER_HEIGHT
                    + if collapsed { 0.0 } else { body_height },
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .pt(px(GROUP_GAP))
                    .child(header)
                    .child(body)
                    .into_any_element(),
            ));
        }
    }

    pub(super) fn account_row_height(&self) -> f32 {
        sidebar_row_height(
            self.settings.sidebar_compact,
            self.settings.sidebar_show_project_label,
            false,
            false,
        )
    }

    fn account_row_selected(&self, id: &str, cx: &App) -> bool {
        matches!(self.route, Route::SavedHistory)
            && self.saved_history.as_ref().is_some_and(|panel| {
                panel
                    .read(cx)
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.chat.id() == id && selected.is_current())
            })
    }

    /// A saved chat in the session row's layout: the source marker where a
    /// session shows its project, then the title, with the time in the corner.
    pub(super) fn render_account_row(
        &self,
        row: &AccountRow,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = self.settings.sidebar_compact;
        let show_label = self.settings.sidebar_show_project_label;
        let selected = self.account_row_selected(&row.id, cx);
        let row_id = format!("account-chat-{}", row.id);
        let fade_key = format!("{row_id}-hover");
        let subline = theme.text_muted.opacity(0.5);
        let text = theme.text;
        let rest_text = if selected {
            text
        } else if row.archived {
            text.opacity(0.55)
        } else {
            text.opacity(0.8)
        };
        let selected_wash = crate::theme::glass_selected_bg();
        let rest_bg = if selected {
            selected_wash
        } else {
            crate::theme::wash(0.0)
        };
        let hover_bg = if selected {
            selected_wash
        } else {
            theme.glass_hover()
        };
        let title: SharedString = transcript::single_line(if row.title.trim().is_empty() {
            "Untitled chat"
        } else {
            &row.title
        })
        .into();
        let marker: SharedString = row.marker().into();
        let time: SharedString = format_time_ago(row.time(), Utc::now()).into();
        let corner = |compact: bool| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_color(subline)
                .when(compact, |el| {
                    el.min_w(px(30.0))
                        .text_right()
                        .text_size(crate::typography::ui_rems(11.0))
                })
                .when(!compact, |el| {
                    el.text_size(crate::typography::ui_rems(10.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                })
                .child(time.clone())
        };
        let open_id = row.id.clone();
        div()
            .id(SharedString::from(row_id.clone()))
            .debug_selector({
                let row_id = row_id.clone();
                move || row_id.clone()
            })
            .role(gpui::Role::Button)
            .aria_label(title.clone())
            .tab_index(0)
            .h(px(self.account_row_height()))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .rounded(px(8.0))
            .px(px(Theme::SPACE_SM))
            .py(px(6.0))
            .text_color(motion::hover_blend(&fade_key, rest_text, text))
            .bg(motion::hover_blend(&fade_key, rest_bg, hover_bg))
            .on_hover(motion::hover_listener(fade_key.clone()))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.open_account_chat(&open_id, cx)))
            // Line 1: where the chat comes from, time on the right.
            .when(!compact && show_label, |el| {
                el.child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Theme::SPACE_SM))
                        .child(sidebar_faded_label(
                            format!("{row_id}-source").into(),
                            true,
                            div()
                                .text_size(crate::typography::ui_rems(11.0))
                                .line_height(px(14.0))
                                .text_color(subline)
                                .child(marker.clone()),
                        ))
                        .child(corner(false)),
                )
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(if compact {
                        4.0
                    } else {
                        SIDEBAR_ACTIVE_HARNESS_TITLE_GAP
                    }))
                    // The status slot stays empty: nothing runs on open.
                    .when(compact, |el| el.child(div().size(px(13.0)).flex_none()))
                    // Where the project icon sits on a session row.
                    .child(
                        icon(match row.source {
                            Some("Terminal") => icons::TERMINAL,
                            Some("Phone") => icons::SMARTPHONE,
                            _ => icons::GLOBE,
                        })
                        .size(px(SIDEBAR_ACTIVE_HARNESS_ICON_SIZE))
                        .flex_none()
                        .text_color(subline),
                    )
                    .child(sidebar_faded_label(
                        format!("{row_id}-title").into(),
                        true,
                        div()
                            .text_size(crate::typography::ui_rems(13.0))
                            .line_height(px(17.0))
                            .child(title),
                    ))
                    // Without the label line the marker rides beside the title.
                    .when(compact || !show_label, |el| {
                        el.when(!marker.is_empty(), |el| {
                            el.child(
                                div().flex_none().max_w(px(80.0)).child(sidebar_faded_label(
                                    format!("{row_id}-marker").into(),
                                    false,
                                    div()
                                        .text_size(crate::typography::ui_rems(10.0))
                                        .text_color(subline)
                                        .child(marker.clone()),
                                )),
                            )
                        })
                        .child(corner(compact))
                    }),
            )
            .into_any_element()
    }

    fn open_account_chat(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(panel) = self.saved_history.clone() {
            panel.update(cx, |panel, cx| panel.open(id, cx));
        }
    }

    /// Load the account's next page once the list end scrolls near view.
    pub(super) fn account_chats_sentinel(&self, cx: &App) -> Option<AnyElement> {
        let panel = self.saved_history.clone()?;
        let snapshot = &panel.read(cx).snapshot;
        let shown = match self.settings.sidebar_organization {
            SidebarOrganization::InOneList => self.sessions_open,
            _ => true,
        } || self.archived_open;
        if !(snapshot.signed_in && snapshot.has_more && snapshot.state == LoadState::Ready && shown)
        {
            return None;
        }
        let viewport = self.sidebar_scroll.clone();
        Some(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    if bounds.top() <= viewport.bounds().bottom() + px(LOAD_AHEAD) {
                        let panel = panel.clone();
                        window.defer(cx, move |_, cx| {
                            panel.update(cx, |panel, cx| panel.load_more(cx));
                        });
                    }
                },
            )
            .w_full()
            .h(px(1.0))
            .into_any_element(),
        )
    }
}

/// A sidebar with this computer's sessions and fixture account rows, for the
/// GPUI test and the offscreen capture.
#[cfg(any(test, feature = "account-fixture"))]
mod fixture {
    use super::*;

    pub(super) struct SidebarHost(pub Entity<Shell>);

    impl Render for SidebarHost {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::of(cx).clone();
            self.0.update(cx, |shell, cx| {
                div()
                    .size_full()
                    .bg(theme.surface)
                    .text_color(theme.text)
                    .font_family(crate::typography::effective_family_name(cx))
                    .child(shell.render_chat_sidebar(&theme, cx))
            })
        }
    }

    pub(super) fn init(rows: Vec<AccountRow>, data: &std::path::Path, cx: &mut App) {
        gpui_base::init(cx);
        cx.set_global(Theme::default());
        // The real menus need the macOS app delegate; the capture has none.
        #[cfg(test)]
        crate::app_menus::init(cx);
        settings::init(UiSettings::default(), data, cx);
        crate::history::init(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            cx,
        );
        cx.set_global(AccountRowsFixture(rows));
    }

    pub(super) fn shell(
        chats: Vec<openagents_proto::Chat>,
        data: &std::path::Path,
        cx: &mut App,
    ) -> Entity<Shell> {
        cx.new(|cx| {
            let state = cx.new(|_| AppState::new());
            let mut shell = Shell::new(
                state,
                EngineBootConfig {
                    data_dir: data.into(),
                    ipc_port: 0,
                    edge_url: "http://127.0.0.1:1".into(),
                    edge_token: None,
                    org_id: None,
                    workos_client_id: None,
                    default_harness: openagents_proto::HarnessId::Mock,
                },
                cx,
            );
            shell.settings.sidebar_organization = SidebarOrganization::InOneList;
            shell.reduced_motion = true;
            shell.sessions_open = true;
            shell.archived_open = true;
            shell.state.update(cx, |state, _| {
                state.workspace_scope = Some(WorkspaceScope::Local);
                state.local_device_id = Some("local".into());
                state.no_project = true;
                state.chats = chats;
            });
            shell
        })
    }

    pub(super) fn account(
        id: &str,
        title: &str,
        minutes_ago: i64,
        source: &'static str,
        project: Option<&str>,
        archived: bool,
    ) -> AccountRow {
        AccountRow {
            id: id.into(),
            title: title.into(),
            updated_unix: (Utc::now() - chrono::Duration::minutes(minutes_ago)).timestamp() as u64,
            archived,
            source: Some(source),
            project: project.map(Into::into),
            continued: false,
        }
    }

    pub(super) fn local(
        id: &str,
        title: &str,
        minutes_ago: i64,
        continues: Option<&str>,
    ) -> openagents_proto::Chat {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "deviceId": "local", "archived": false,
            "createdAt": Utc::now() - chrono::Duration::minutes(minutes_ago),
            "continues": continues.map(|id| serde_json::json!({"id": id, "home": "cloud"})),
        }))
        .unwrap()
    }
}

/// Draw the merged sidebar offscreen and save it as a PNG (#11960). No window
/// is shown: the headless context renders with Metal into an image.
#[cfg(feature = "account-fixture")]
pub fn capture_sidebar(
    cx: &mut gpui::HeadlessAppContext,
    data: &std::path::Path,
    output: &std::path::Path,
) -> anyhow::Result<()> {
    use fixture::{account, local};
    let rows = vec![
        account(
            "a1",
            "Episode 290 · Video Lab workspace",
            25,
            "Web",
            Some("Video Lab"),
            false,
        ),
        account(
            "a2",
            "Video Lab · Kiki Devlog 001",
            50,
            "Web",
            Some("Kiki Studios"),
            false,
        ),
        account(
            "a3",
            "read in my kiki studios project the chat",
            190,
            "Terminal",
            Some("openagents"),
            false,
        ),
        account("a4", "Are you there?", 260, "Phone", None, false),
        // Continued below as a session on this computer: one row, not two.
        account(
            "a5",
            "Plan the launch film",
            400,
            "Web",
            Some("Video Lab"),
            false,
        ),
        account("a6", "Old pricing notes", 900, "Web", None, true),
    ];
    let chats = vec![
        local("l1", "Coder 1.0.0 handoff: close the open issues", 5, None),
        local("l2", "Fix composer Runs-on block and sidebar", 40, None),
        local("l3", "Plan the launch film", 120, Some("a5")),
        local("l4", "Release notes for 1.0", 300, None),
    ];
    cx.update(|cx| {
        fixture::init(rows, data, cx);
        let settings = UiSettings::default();
        let fonts = crate::typography::register_fonts(cx);
        crate::typography::init(
            settings.ui_font_family.clone(),
            settings.ui_font_size,
            settings.terminal_font_family.clone(),
            settings.terminal_font_size,
            settings.code_font_family.clone(),
            settings.code_font_size,
            fonts,
            cx,
        );
    });
    let shell = cx.update(|cx| fixture::shell(chats, data, cx));
    let window = cx.open_window(gpui::size(px(300.0), px(760.0)), |_, cx| {
        cx.new(|_| fixture::SidebarHost(shell))
    })?;
    for _ in 0..2 {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())?;
        cx.run_until_parked();
    }
    cx.capture_screenshot(window.into())?.save(output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fixture::{SidebarHost, account, local};
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    fn web(id: &str, minutes_ago: i64, archived: bool) -> AccountRow {
        account(id, id, minutes_ago, "Web", Some("Video Lab"), archived)
    }

    fn setup(
        cx: &mut TestAppContext,
        chats: Vec<openagents_proto::Chat>,
        rows: Vec<AccountRow>,
    ) -> &mut VisualTestContext {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| super::fixture::init(rows, dir.path(), cx));
        let (_, cx) =
            cx.add_window_view(|_, cx| SidebarHost(super::fixture::shell(chats, dir.path(), cx)));
        cx.update(|window, cx| {
            window.resize(gpui::size(px(280.0), px(1600.0)));
            window.activate_window();
            window.draw(cx).clear();
        });
        cx
    }

    fn top(cx: &mut VisualTestContext, selector: &'static str) -> Pixels {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not drawn"))
            .top()
    }

    #[test]
    fn merge_keeps_activity_order_and_local_ties_first() {
        let at = |id: &str, updated_unix| AccountRow {
            updated_unix,
            ..web(id, 0, false)
        };
        let rows = merge_by_activity(
            vec![(300, "c"), (100, "a")],
            vec![at("d", 400), at("tie", 300), at("b", 200), at("z", 50)],
        );
        let order: Vec<String> = rows
            .into_iter()
            .map(|row| match row {
                Merged::Local(id) => id.to_owned(),
                Merged::Account(row) => row.id,
            })
            .collect();
        assert_eq!(order, ["d", "c", "tie", "b", "a", "z"]);
    }

    #[test]
    fn a_continued_saved_chat_shows_only_its_latest_part() {
        let rows = latest_parts(
            vec![
                web("saved", 5, false),
                AccountRow {
                    continued: true,
                    ..web("moved-to-web", 5, false)
                },
                web("same-session", 5, false),
                web("kept", 5, false),
            ],
            &[
                local("session", "session", 1, Some("saved")),
                local("same-session", "same-session", 1, None),
            ],
        );
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["kept"]);
    }

    #[gpui::test]
    fn a_forked_chat_takes_its_place_in_the_merged_list(cx: &mut TestAppContext) {
        // A fork is a standalone chat (#11963); one saved by an older build
        // still names its source in `parent_chat_id`.
        let mut older_fork = local("older-fork", "older fork", 25, None);
        older_fork.parent_chat_id = Some("source".into());
        let cx = setup(
            cx,
            vec![
                local("source", "source", 40, None),
                local("fork", "fork", 2, None),
                older_fork,
            ],
            vec![
                web("saved-between", 10, false),
                web("saved-oldest", 60, false),
            ],
        );
        let order = [
            top(cx, "chat-fork"),
            top(cx, "account-chat-saved-between"),
            top(cx, "chat-older-fork"),
            top(cx, "chat-source"),
            top(cx, "account-chat-saved-oldest"),
        ];
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
    }

    #[gpui::test]
    fn account_chats_join_the_sessions_list_by_activity(cx: &mut TestAppContext) {
        let cx = setup(
            cx,
            vec![
                local("oldest", "oldest", 30, None),
                local("newest", "newest", 1, None),
                local(
                    "continuation",
                    "continuation",
                    10,
                    Some("saved-earlier-part"),
                ),
            ],
            vec![
                web("saved-recent", 5, false),
                web("saved-old", 60, false),
                web("saved-earlier-part", 20, false),
                web("saved-archived", 3, true),
            ],
        );
        // One list, newest first: saved chats sit between this computer's
        // sessions by their own activity time.
        let order = [
            top(cx, "chat-newest"),
            top(cx, "account-chat-saved-recent"),
            top(cx, "chat-continuation"),
            top(cx, "chat-oldest"),
            top(cx, "account-chat-saved-old"),
        ];
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
        // The continued saved chat is one row: its later part on this computer.
        assert!(cx.debug_bounds("account-chat-saved-earlier-part").is_none());
        // Archived saved chats wait in Archived, below the list.
        assert!(top(cx, "account-chat-saved-archived") > top(cx, "archived-toggle"));
        // No separate section.
        assert!(cx.debug_bounds("openagents-history").is_none());
    }
}
