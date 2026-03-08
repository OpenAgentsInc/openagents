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
    chips.extend(parse_search_query(search_query).chips);
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
    let query = parse_search_query(search_query);

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
        .filter(|item| {
            matches_search(
                item,
                &query,
                operator_label.as_str(),
                active_cycle_id.as_deref(),
            )
        })
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

#[derive(Default)]
struct ProjectOpsSearchQuery {
    text_terms: Vec<String>,
    status_filters: Vec<ProjectOpsStatusFilter>,
    assignee_filters: Vec<ProjectOpsAssigneeFilter>,
    priority_filters: Vec<ProjectOpsPriorityFilter>,
    cycle_filters: Vec<ProjectOpsCycleFilter>,
    blocked_filters: Vec<bool>,
    tag_filters: Vec<String>,
    chips: Vec<String>,
}

#[derive(Clone, Copy)]
enum ProjectOpsStatusFilter {
    Active,
    Exact(ProjectOpsWorkItemStatus),
}

enum ProjectOpsAssigneeFilter {
    Me,
    None,
    Exact(String),
}

#[derive(Clone, Copy)]
enum ProjectOpsPriorityFilter {
    Exact(super::schema::ProjectOpsPriority),
}

enum ProjectOpsCycleFilter {
    Active,
    None,
    Exact(String),
}

fn parse_search_query(search_query: &str) -> ProjectOpsSearchQuery {
    let mut parsed = ProjectOpsSearchQuery::default();
    for raw_token in search_query.split_whitespace() {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }
        let Some((key, value)) = token.split_once(':') else {
            let normalized = token.to_ascii_lowercase();
            parsed.text_terms.push(normalized.clone());
            parsed.chips.push(format!("search:{normalized}"));
            continue;
        };
        let normalized_key = key.trim().to_ascii_lowercase();
        let normalized_value = normalize_query_value(value);
        if normalized_value.is_empty() {
            let normalized = token.to_ascii_lowercase();
            parsed.text_terms.push(normalized.clone());
            parsed.chips.push(format!("search:{normalized}"));
            continue;
        }
        match normalized_key.as_str() {
            "state" | "status" => {
                if let Some(filter) = parse_status_filter(normalized_value.as_str()) {
                    parsed.status_filters.push(filter);
                    parsed.chips.push(format!("status:{normalized_value}"));
                } else {
                    parsed.text_terms.push(token.to_ascii_lowercase());
                    parsed
                        .chips
                        .push(format!("search:{}", token.to_ascii_lowercase()));
                }
            }
            "assignee" => {
                parsed
                    .assignee_filters
                    .push(parse_assignee_filter(normalized_value.as_str()));
                parsed.chips.push(format!("assignee:{normalized_value}"));
            }
            "priority" => {
                if let Some(filter) = parse_priority_filter(normalized_value.as_str()) {
                    parsed.priority_filters.push(filter);
                    parsed.chips.push(format!("priority:{normalized_value}"));
                } else {
                    parsed.text_terms.push(token.to_ascii_lowercase());
                    parsed
                        .chips
                        .push(format!("search:{}", token.to_ascii_lowercase()));
                }
            }
            "cycle" => {
                parsed
                    .cycle_filters
                    .push(parse_cycle_filter(normalized_value.as_str()));
                parsed.chips.push(format!("cycle:{normalized_value}"));
            }
            "blocked" => {
                if let Some(value) = parse_bool_filter(normalized_value.as_str()) {
                    parsed.blocked_filters.push(value);
                    parsed.chips.push(format!("blocked:{normalized_value}"));
                } else {
                    parsed.text_terms.push(token.to_ascii_lowercase());
                    parsed
                        .chips
                        .push(format!("search:{}", token.to_ascii_lowercase()));
                }
            }
            "tag" | "area" => {
                parsed.tag_filters.push(normalized_value.clone());
                parsed.chips.push(format!("tag:{normalized_value}"));
            }
            "sort" => {
                parsed.chips.push(format!("sort:{normalized_value}"));
            }
            _ => {
                parsed.text_terms.push(token.to_ascii_lowercase());
                parsed
                    .chips
                    .push(format!("search:{}", token.to_ascii_lowercase()));
            }
        }
    }
    parsed
}

fn matches_search(
    item: &ProjectOpsWorkItem,
    query: &ProjectOpsSearchQuery,
    operator_label: &str,
    active_cycle_id: Option<&str>,
) -> bool {
    if !query.status_filters.is_empty()
        && !query
            .status_filters
            .iter()
            .any(|filter| matches_status_filter(item, *filter))
    {
        return false;
    }
    if !query.assignee_filters.is_empty()
        && !query
            .assignee_filters
            .iter()
            .any(|filter| matches_assignee_filter(item, filter, operator_label))
    {
        return false;
    }
    if !query.priority_filters.is_empty()
        && !query
            .priority_filters
            .iter()
            .any(|filter| matches_priority_filter(item, *filter))
    {
        return false;
    }
    if !query.cycle_filters.is_empty()
        && !query
            .cycle_filters
            .iter()
            .any(|filter| matches_cycle_filter(item, filter, active_cycle_id))
    {
        return false;
    }
    if !query.blocked_filters.is_empty()
        && !query
            .blocked_filters
            .iter()
            .any(|blocked| item.is_blocked() == *blocked)
    {
        return false;
    }
    if !query.tag_filters.iter().all(|tag| {
        item.area_tags
            .iter()
            .any(|item_tag| item_tag.eq_ignore_ascii_case(tag))
    }) {
        return false;
    }
    if query.text_terms.is_empty() {
        return true;
    }

    query
        .text_terms
        .iter()
        .all(|term| matches_text_term(item, term.as_str()))
}

fn matches_text_term(item: &ProjectOpsWorkItem, query: &str) -> bool {
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

fn normalize_query_value(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

fn parse_status_filter(value: &str) -> Option<ProjectOpsStatusFilter> {
    match value {
        "active" => Some(ProjectOpsStatusFilter::Active),
        "backlog" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::Backlog,
        )),
        "todo" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::Todo,
        )),
        "in_progress" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::InProgress,
        )),
        "in_review" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::InReview,
        )),
        "done" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::Done,
        )),
        "cancelled" => Some(ProjectOpsStatusFilter::Exact(
            ProjectOpsWorkItemStatus::Cancelled,
        )),
        _ => None,
    }
}

fn matches_status_filter(item: &ProjectOpsWorkItem, filter: ProjectOpsStatusFilter) -> bool {
    match filter {
        ProjectOpsStatusFilter::Active => !item.status.is_terminal(),
        ProjectOpsStatusFilter::Exact(status) => item.status == status,
    }
}

fn parse_assignee_filter(value: &str) -> ProjectOpsAssigneeFilter {
    match value {
        "me" => ProjectOpsAssigneeFilter::Me,
        "none" | "unassigned" => ProjectOpsAssigneeFilter::None,
        _ => ProjectOpsAssigneeFilter::Exact(value.to_string()),
    }
}

fn matches_assignee_filter(
    item: &ProjectOpsWorkItem,
    filter: &ProjectOpsAssigneeFilter,
    operator_label: &str,
) -> bool {
    match filter {
        ProjectOpsAssigneeFilter::Me => item
            .assignee
            .as_deref()
            .map(|assignee| assignee.eq_ignore_ascii_case(operator_label))
            .unwrap_or(false),
        ProjectOpsAssigneeFilter::None => item.assignee.is_none(),
        ProjectOpsAssigneeFilter::Exact(value) => item
            .assignee
            .as_deref()
            .map(|assignee| assignee.eq_ignore_ascii_case(value))
            .unwrap_or(false),
    }
}

fn parse_priority_filter(value: &str) -> Option<ProjectOpsPriorityFilter> {
    use super::schema::ProjectOpsPriority;

    match value {
        "urgent" => Some(ProjectOpsPriorityFilter::Exact(ProjectOpsPriority::Urgent)),
        "high" => Some(ProjectOpsPriorityFilter::Exact(ProjectOpsPriority::High)),
        "medium" => Some(ProjectOpsPriorityFilter::Exact(ProjectOpsPriority::Medium)),
        "low" => Some(ProjectOpsPriorityFilter::Exact(ProjectOpsPriority::Low)),
        "none" => Some(ProjectOpsPriorityFilter::Exact(ProjectOpsPriority::None)),
        _ => None,
    }
}

fn matches_priority_filter(item: &ProjectOpsWorkItem, filter: ProjectOpsPriorityFilter) -> bool {
    match filter {
        ProjectOpsPriorityFilter::Exact(priority) => item.priority == priority,
    }
}

fn parse_cycle_filter(value: &str) -> ProjectOpsCycleFilter {
    match value {
        "active" => ProjectOpsCycleFilter::Active,
        "none" => ProjectOpsCycleFilter::None,
        _ => ProjectOpsCycleFilter::Exact(value.to_string()),
    }
}

fn matches_cycle_filter(
    item: &ProjectOpsWorkItem,
    filter: &ProjectOpsCycleFilter,
    active_cycle_id: Option<&str>,
) -> bool {
    match filter {
        ProjectOpsCycleFilter::Active => active_cycle_id.is_some_and(|cycle_id| {
            item.cycle_id
                .as_ref()
                .map(|item_cycle| item_cycle.as_str() == cycle_id)
                .unwrap_or(false)
        }),
        ProjectOpsCycleFilter::None => item.cycle_id.is_none(),
        ProjectOpsCycleFilter::Exact(value) => item
            .cycle_id
            .as_ref()
            .map(|cycle_id| cycle_id.as_str().eq_ignore_ascii_case(value))
            .unwrap_or(false),
    }
}

fn parse_bool_filter(value: &str) -> Option<bool> {
    match value {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
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
            filter_chips_for_view(
                PROJECT_OPS_MY_WORK_VIEW_ID,
                "wallet status:todo blocked:true"
            ),
            vec![
                "assignee:me".to_string(),
                "status:active".to_string(),
                "search:wallet".to_string(),
                "status:todo".to_string(),
                "blocked:true".to_string()
            ]
        );
        assert_eq!(
            empty_state_copy_for_view(PROJECT_OPS_BLOCKED_VIEW_ID),
            "No blocked work."
        );
        assert!(!current_operator_label().trim().is_empty());
    }

    #[test]
    fn advanced_query_filters_apply_structured_tokens() {
        let work_items = vec![
            work_item(
                "wi-1",
                "Search parser board",
                ProjectOpsWorkItemStatus::Todo,
                Some("cdavid"),
                Some("2026-w10"),
                None,
                30,
            ),
            work_item(
                "wi-2",
                "Blocked sync work",
                ProjectOpsWorkItemStatus::InProgress,
                Some("teammate"),
                Some("2026-w10"),
                Some("Waiting on design"),
                40,
            ),
            work_item(
                "wi-3",
                "Low-priority backlog",
                ProjectOpsWorkItemStatus::Backlog,
                None,
                None,
                None,
                20,
            ),
        ];
        let cycles = cycles();

        let structured = filter_work_items_for_view(
            work_items.as_slice(),
            cycles.as_slice(),
            "custom",
            "cdavid",
            "assignee:teammate priority:high blocked:true cycle:active area:pm state:in_progress",
        );
        assert_eq!(
            structured
                .iter()
                .map(|item| item.work_item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["wi-2"]
        );

        let me_active = filter_work_items_for_view(
            work_items.as_slice(),
            cycles.as_slice(),
            "custom",
            "cdavid",
            "assignee:me status:active search",
        );
        assert_eq!(
            me_active
                .iter()
                .map(|item| item.work_item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["wi-1"]
        );

        let no_cycle = filter_work_items_for_view(
            work_items.as_slice(),
            cycles.as_slice(),
            "custom",
            "cdavid",
            "cycle:none assignee:none priority:low",
        );
        assert!(no_cycle.is_empty());
        let backlog_none = filter_work_items_for_view(
            work_items.as_slice(),
            cycles.as_slice(),
            "custom",
            "cdavid",
            "cycle:none assignee:none state:backlog",
        );
        assert_eq!(
            backlog_none
                .iter()
                .map(|item| item.work_item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["wi-3"]
        );
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
