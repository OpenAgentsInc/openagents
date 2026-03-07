use super::schema::{
    ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsTeamKey, ProjectOpsWorkItem,
    ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsQuickCreateDraft {
    pub title: String,
    pub description: String,
    pub priority: ProjectOpsPriority,
    pub team_key: ProjectOpsTeamKey,
}

impl Default for ProjectOpsQuickCreateDraft {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            priority: ProjectOpsPriority::Medium,
            team_key: ProjectOpsTeamKey::new("desktop")
                .unwrap_or_else(|_| ProjectOpsTeamKey::new("pm").expect("fallback team")),
        }
    }
}

impl ProjectOpsQuickCreateDraft {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("quick create title must not be empty".to_string());
        }
        if self.description.trim().is_empty() {
            return Err("quick create description must not be empty".to_string());
        }
        Ok(())
    }

    pub fn to_work_item_draft(
        &self,
        work_item_id: ProjectOpsWorkItemId,
    ) -> crate::project_ops::contract::ProjectOpsWorkItemDraft {
        crate::project_ops::contract::ProjectOpsWorkItemDraft {
            work_item_id,
            title: self.title.clone(),
            description: self.description.clone(),
            status: ProjectOpsWorkItemStatus::Backlog,
            priority: self.priority,
            assignee: None,
            team_key: self.team_key.clone(),
            cycle_id: None,
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsDetailDraft {
    pub work_item_id: ProjectOpsWorkItemId,
    pub title: String,
    pub description: String,
    pub status: ProjectOpsWorkItemStatus,
    pub priority: ProjectOpsPriority,
    pub assignee: Option<String>,
    pub cycle_id: Option<ProjectOpsCycleId>,
    pub parent_id: Option<ProjectOpsWorkItemId>,
    pub blocked_reason: Option<String>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub dirty: bool,
}

impl ProjectOpsDetailDraft {
    pub fn from_work_item(work_item: &ProjectOpsWorkItem) -> Self {
        Self {
            work_item_id: work_item.work_item_id.clone(),
            title: work_item.title.clone(),
            description: work_item.description.clone(),
            status: work_item.status,
            priority: work_item.priority,
            assignee: work_item.assignee.clone(),
            cycle_id: work_item.cycle_id.clone(),
            parent_id: work_item.parent_id.clone(),
            blocked_reason: work_item.blocked_reason.clone(),
            created_at_unix_ms: work_item.created_at_unix_ms,
            updated_at_unix_ms: work_item.updated_at_unix_ms,
            dirty: false,
        }
    }
}
