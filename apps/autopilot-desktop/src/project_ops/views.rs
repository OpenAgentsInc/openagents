use super::projection::ProjectOpsCycleRow;
use super::schema::{ProjectOpsWorkItem, ProjectOpsWorkItemStatus};

pub const PROJECT_OPS_MY_WORK_VIEW_ID: &str = "my-work";
pub const PROJECT_OPS_CURRENT_CYCLE_VIEW_ID: &str = "current-cycle";
pub const PROJECT_OPS_BLOCKED_VIEW_ID: &str = "blocked";
pub const PROJECT_OPS_BACKLOG_VIEW_ID: &str = "backlog";
pub const PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID: &str = "recently-updated";
pub const PROJECT_OPS_DEFAULT_VIEW_ID: &str = PROJECT_OPS_MY_WORK_VIEW_ID;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectOpsBuiltinSavedViewSpec {
    pub view_id: &'static str,
    pub title: &'static str,
    pub query: &'static str,
    pub filters: &'static [&'static str],
}

const BUILTIN_SAVED_VIEW_SPECS: [ProjectOpsBuiltinSavedViewSpec; 5] = [
    ProjectOpsBuiltinSavedViewSpec {
        view_id: PROJECT_OPS_MY_WORK_VIEW_ID,
        title: "My Work",
        query: "assignee:me",
        filters: &["assignee:me", "status:active"],
    },
    ProjectOpsBuiltinSavedViewSpec {
        view_id: PROJECT_OPS_CURRENT_CYCLE_VIEW_ID,
        title: "Current Cycle",
        query: "cycle:active",
        filters: &["cycle:active"],
    },
    ProjectOpsBuiltinSavedViewSpec {
        view_id: PROJECT_OPS_BLOCKED_VIEW_ID,
        title: "Blocked",
        query: "blocked:true",
        filters: &["blocked:true"],
    },
    ProjectOpsBuiltinSavedViewSpec {
        view_id: PROJECT_OPS_BACKLOG_VIEW_ID,
        title: "Backlog",
        query: "status:backlog cycle:none",
        filters: &["status:backlog", "cycle:none"],
    },
    ProjectOpsBuiltinSavedViewSpec {
        view_id: PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID,
        title: "Recently Updated",
        query: "sort:updated_desc status:active",
        filters: &["sort:updated_desc", "status:active"],
    },
];

pub fn builtin_saved_view_specs() -> &'static [ProjectOpsBuiltinSavedViewSpec] {
    &BUILTIN_SAVED_VIEW_SPECS
}

pub fn view_title_for_id(view_id: &str) -> Option<&'static str> {
    builtin_saved_view_specs()
        .iter()
        .find(|spec| spec.view_id == view_id)
        .map(|spec| spec.title)
}

pub fn current_operator_label() -> String {
    std::env::var("OPENAGENTS_PROJECT_OPS_ACTOR_LABEL")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("LOGNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "me".to_string())
}

pub fn filter_chips_for_view(view_id: &str, search_query: &str) -> Vec<String> {
    let mut chips: Vec<String> = builtin_saved_view_specs()
        .iter()
        .find(|spec| spec.view_id == view_id)
        .map(|spec| {
            spec.filters
                .iter()
                .map(|filter| (*filter).to_string())
                .collect()
        })
        .unwrap_or_default();
    let query = search_query.trim();
    if !query.is_empty() {
        chips.push(format!("search:{query}"));
    }
    chips
}

pub fn empty_state_copy_for_view(view_id: &str) -> &'static str {
    match view_id {
        PROJECT_OPS_MY_WORK_VIEW_ID => "No assigned work right now.",
        PROJECT_OPS_BLOCKED_VIEW_ID => "No blocked work.",
        PROJECT_OPS_BACKLOG_VIEW_ID => "No captured backlog items.",
        PROJECT_OPS_CURRENT_CYCLE_VIEW_ID => "No items in the active cycle.",
        PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID => "No recently updated active work.",
        _ => "No work items match this view.",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsBoardLane {
    pub status: ProjectOpsWorkItemStatus,
    pub title: &'static str,
    pub work_item_count: usize,
    pub blocked_count: usize,
    pub items: Vec<ProjectOpsWorkItem>,
    pub empty_state_copy: String,
}

pub fn project_board_lanes(work_items: &[ProjectOpsWorkItem]) -> Vec<ProjectOpsBoardLane> {
    ProjectOpsWorkItemStatus::workflow()
        .iter()
        .copied()
        .map(|status| {
            let items = work_items
                .iter()
                .filter(|item| item.status == status)
                .cloned()
                .collect::<Vec<_>>();
            let blocked_count = items.iter().filter(|item| item.is_blocked()).count();
            ProjectOpsBoardLane {
                status,
                title: board_lane_title(status),
                work_item_count: items.len(),
                blocked_count,
                items,
                empty_state_copy: format!("No {} items in this view.", board_lane_title(status)),
            }
        })
        .collect()
}

pub fn filter_work_items_for_view(
    work_items: &[ProjectOpsWorkItem],
    cycles: &[ProjectOpsCycleRow],
    view_id: &str,
    operator_label: &str,
    search_query: &str,
) -> Vec<ProjectOpsWorkItem> {
    let active_cycle_id = cycles
        .iter()
        .find(|cycle| cycle.is_active)
        .map(|cycle| cycle.cycle_id.as_str().to_string());
    let operator_label = operator_label.trim().to_ascii_lowercase();
    let query = search_query.trim().to_ascii_lowercase();

    let mut rows = work_items
        .iter()
        .filter(|item| item.archived_at_unix_ms.is_none())
        .filter(|item| {
            matches_view(
                item,
                view_id,
                operator_label.as_str(),
                active_cycle_id.as_deref(),
            )
        })
        .filter(|item| matches_search(item, query.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|lhs, rhs| {
        rhs.updated_at_unix_ms
            .cmp(&lhs.updated_at_unix_ms)
            .then_with(|| lhs.work_item_id.as_str().cmp(rhs.work_item_id.as_str()))
    });
    rows
}

fn matches_view(
    item: &ProjectOpsWorkItem,
    view_id: &str,
    operator_label: &str,
    active_cycle_id: Option<&str>,
) -> bool {
    match view_id {
        PROJECT_OPS_MY_WORK_VIEW_ID => {
            item.assignee
                .as_deref()
                .map(|assignee| assignee.eq_ignore_ascii_case(operator_label))
                .unwrap_or(false)
                && !item.status.is_terminal()
        }
        PROJECT_OPS_CURRENT_CYCLE_VIEW_ID => active_cycle_id.is_some_and(|active_cycle_id| {
            item.cycle_id
                .as_ref()
                .map(|cycle_id| cycle_id.as_str() == active_cycle_id)
                .unwrap_or(false)
        }),
        PROJECT_OPS_BLOCKED_VIEW_ID => item.is_blocked(),
        PROJECT_OPS_BACKLOG_VIEW_ID => {
            item.status == ProjectOpsWorkItemStatus::Backlog && item.cycle_id.is_none()
        }
        PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID => !item.status.is_terminal(),
        _ => true,
    }
}

fn matches_search(item: &ProjectOpsWorkItem, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    item.title.to_ascii_lowercase().contains(query)
        || item.description.to_ascii_lowercase().contains(query)
        || item
            .assignee
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(query)
        || item
            .cycle_id
            .as_ref()
            .map(|cycle_id| cycle_id.as_str().to_ascii_lowercase().contains(query))
            .unwrap_or(false)
        || item
            .area_tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(query))
}

fn board_lane_title(status: ProjectOpsWorkItemStatus) -> &'static str {
    match status {
        ProjectOpsWorkItemStatus::Backlog => "Backlog",
        ProjectOpsWorkItemStatus::Todo => "Todo",
        ProjectOpsWorkItemStatus::InProgress => "In Progress",
        ProjectOpsWorkItemStatus::InReview => "In Review",
        ProjectOpsWorkItemStatus::Done => "Done",
        ProjectOpsWorkItemStatus::Cancelled => "Cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PROJECT_OPS_BACKLOG_VIEW_ID, PROJECT_OPS_BLOCKED_VIEW_ID,
        PROJECT_OPS_CURRENT_CYCLE_VIEW_ID, PROJECT_OPS_MY_WORK_VIEW_ID,
        PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID, builtin_saved_view_specs, current_operator_label,
        empty_state_copy_for_view, filter_chips_for_view, filter_work_items_for_view,
        project_board_lanes, view_title_for_id,
    };
    use crate::project_ops::projection::ProjectOpsCycleRow;
    use crate::project_ops::schema::{
        ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
        ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
    };

    fn work_item(
        work_item_id: &str,
        title: &str,
        status: ProjectOpsWorkItemStatus,
        assignee: Option<&str>,
        cycle_id: Option<&str>,
        blocked_reason: Option<&str>,
        updated_at_unix_ms: u64,
    ) -> ProjectOpsWorkItem {
        ProjectOpsWorkItem {
            work_item_id: ProjectOpsWorkItemId::new(work_item_id).expect("work item id"),
            title: title.to_string(),
            description: format!("{title} description"),
            status,
            priority: ProjectOpsPriority::High,
            assignee: assignee.map(ToString::to_string),
            team_key: ProjectOpsTeamKey::new("desktop").expect("team key"),
            cycle_id: cycle_id.map(|cycle_id| ProjectOpsCycleId::new(cycle_id).expect("cycle id")),
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: blocked_reason.map(ToString::to_string),
            due_at_unix_ms: None,
            created_at_unix_ms: 1_762_000_000_000,
            updated_at_unix_ms,
            archived_at_unix_ms: None,
        }
    }

    fn cycles() -> Vec<ProjectOpsCycleRow> {
        vec![ProjectOpsCycleRow {
            cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
            title: "Week 10".to_string(),
            goal: Some("Ship the PM toolbar".to_string()),
            starts_at_unix_ms: 1_761_998_400_000,
            ends_at_unix_ms: 1_762_603_200_000,
            is_active: true,
        }]
    }

    #[test]
    fn builtin_saved_views_match_step0_spec() {
        let specs = builtin_saved_view_specs();
        assert_eq!(specs.len(), 5);
        assert_eq!(
            view_title_for_id(PROJECT_OPS_MY_WORK_VIEW_ID),
            Some("My Work")
        );
        assert_eq!(
            view_title_for_id(PROJECT_OPS_CURRENT_CYCLE_VIEW_ID),
            Some("Current Cycle")
        );
        assert_eq!(
            view_title_for_id(PROJECT_OPS_BLOCKED_VIEW_ID),
            Some("Blocked")
        );
        assert_eq!(
            view_title_for_id(PROJECT_OPS_BACKLOG_VIEW_ID),
            Some("Backlog")
        );
        assert_eq!(
            view_title_for_id(PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID),
            Some("Recently Updated")
        );
    }

    #[test]
    fn filter_logic_matches_builtin_view_semantics() {
        let work_items = vec![
            work_item(
                "wi-1",
                "My task",
                ProjectOpsWorkItemStatus::Todo,
                Some("cdavid"),
                Some("2026-w10"),
                None,
                30,
            ),
            work_item(
                "wi-2",
                "Blocked task",
                ProjectOpsWorkItemStatus::InProgress,
                Some("teammate"),
                Some("2026-w10"),
                Some("Waiting on design"),
                40,
            ),
            work_item(
                "wi-3",
                "Backlog task",
                ProjectOpsWorkItemStatus::Backlog,
                None,
                None,
                None,
                20,
            ),
            work_item(
                "wi-4",
                "Completed task",
                ProjectOpsWorkItemStatus::Done,
                Some("cdavid"),
                Some("2026-w10"),
                None,
                50,
            ),
        ];
        let cycles = cycles();

        assert_eq!(
            filter_work_items_for_view(
                work_items.as_slice(),
                cycles.as_slice(),
                PROJECT_OPS_MY_WORK_VIEW_ID,
                "cdavid",
                "",
            )
            .iter()
            .map(|item| item.work_item_id.as_str())
            .collect::<Vec<_>>(),
            vec!["wi-1"]
        );
        assert_eq!(
            filter_work_items_for_view(
                work_items.as_slice(),
                cycles.as_slice(),
                PROJECT_OPS_CURRENT_CYCLE_VIEW_ID,
                "cdavid",
                "",
            )
            .len(),
            3
        );
        assert_eq!(
            filter_work_items_for_view(
                work_items.as_slice(),
                cycles.as_slice(),
                PROJECT_OPS_BLOCKED_VIEW_ID,
                "cdavid",
                "",
            )
            .iter()
            .map(|item| item.work_item_id.as_str())
            .collect::<Vec<_>>(),
            vec!["wi-2"]
        );
        assert_eq!(
            filter_work_items_for_view(
                work_items.as_slice(),
                cycles.as_slice(),
                PROJECT_OPS_BACKLOG_VIEW_ID,
                "cdavid",
                "",
            )
            .iter()
            .map(|item| item.work_item_id.as_str())
            .collect::<Vec<_>>(),
            vec!["wi-3"]
        );
        assert_eq!(
            filter_work_items_for_view(
                work_items.as_slice(),
                cycles.as_slice(),
                PROJECT_OPS_RECENTLY_UPDATED_VIEW_ID,
                "cdavid",
                "blocked",
            )
            .iter()
            .map(|item| item.work_item_id.as_str())
            .collect::<Vec<_>>(),
            vec!["wi-2"]
        );
    }

    #[test]
    fn filter_chips_and_empty_state_copy_are_specific_to_views() {
        assert_eq!(
            filter_chips_for_view(PROJECT_OPS_MY_WORK_VIEW_ID, "wallet"),
            vec![
                "assignee:me".to_string(),
                "status:active".to_string(),
                "search:wallet".to_string()
            ]
        );
        assert_eq!(
            empty_state_copy_for_view(PROJECT_OPS_BLOCKED_VIEW_ID),
            "No blocked work."
        );
        assert!(!current_operator_label().trim().is_empty());
    }

    #[test]
    fn board_projection_groups_rows_by_workflow_lane() {
        let rows = vec![
            work_item(
                "wi-1",
                "Board backlog",
                ProjectOpsWorkItemStatus::Backlog,
                Some("cdavid"),
                None,
                None,
                30,
            ),
            work_item(
                "wi-2",
                "Board todo",
                ProjectOpsWorkItemStatus::Todo,
                Some("cdavid"),
                Some("2026-w10"),
                None,
                20,
            ),
            work_item(
                "wi-3",
                "Board in progress",
                ProjectOpsWorkItemStatus::InProgress,
                Some("teammate"),
                Some("2026-w10"),
                Some("Waiting on API"),
                10,
            ),
            work_item(
                "wi-4",
                "Board done",
                ProjectOpsWorkItemStatus::Done,
                Some("cdavid"),
                Some("2026-w10"),
                None,
                5,
            ),
        ];

        let lanes = project_board_lanes(rows.as_slice());
        assert_eq!(lanes.len(), ProjectOpsWorkItemStatus::workflow().len());
        assert_eq!(lanes[0].title, "Backlog");
        assert_eq!(lanes[0].items.len(), 1);
        assert_eq!(lanes[0].items[0].work_item_id.as_str(), "wi-1");
        assert_eq!(lanes[2].title, "In Progress");
        assert_eq!(lanes[2].blocked_count, 1);
        assert_eq!(lanes[2].items[0].work_item_id.as_str(), "wi-3");
        assert_eq!(lanes[4].title, "Done");
        assert_eq!(lanes[4].work_item_count, 1);
        assert_eq!(lanes[5].title, "Cancelled");
        assert!(lanes[5].items.is_empty());
    }
}
