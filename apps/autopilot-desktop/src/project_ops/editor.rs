use super::schema::{
    ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsProjectId, ProjectOpsTeamKey,
    ProjectOpsWorkItem, ProjectOpsWorkItemId, ProjectOpsWorkItemStatus,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOpsQuickCreateDraft {
    pub title: String,
    pub description: String,
    pub priority: ProjectOpsPriority,
    pub team_key: ProjectOpsTeamKey,
    pub project_id: Option<ProjectOpsProjectId>,
    pub area_tags: Vec<String>,
    pub due_at_unix_ms: Option<u64>,
}

impl Default for ProjectOpsQuickCreateDraft {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            priority: ProjectOpsPriority::Medium,
            team_key: ProjectOpsTeamKey::new("desktop")
                .unwrap_or_else(|_| ProjectOpsTeamKey::new("pm").expect("fallback team")),
            project_id: None,
            area_tags: vec!["pm".to_string()],
            due_at_unix_ms: None,
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
        validate_area_tags(self.area_tags.as_slice(), "quick create")?;
        if self.due_at_unix_ms == Some(0) {
            return Err("quick create due_at_unix_ms must be > 0 when present".to_string());
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
            project_id: self.project_id.clone(),
            cycle_id: None,
            parent_id: None,
            area_tags: self.area_tags.clone(),
            blocked_reason: None,
            due_at_unix_ms: self.due_at_unix_ms,
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
    pub project_id: Option<ProjectOpsProjectId>,
    pub cycle_id: Option<ProjectOpsCycleId>,
    pub parent_id: Option<ProjectOpsWorkItemId>,
    pub area_tags: Vec<String>,
    pub blocked_reason: Option<String>,
    pub due_at_unix_ms: Option<u64>,
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
            project_id: work_item.project_id.clone(),
            cycle_id: work_item.cycle_id.clone(),
            parent_id: work_item.parent_id.clone(),
            area_tags: work_item.area_tags.clone(),
            blocked_reason: work_item.blocked_reason.clone(),
            due_at_unix_ms: work_item.due_at_unix_ms,
            created_at_unix_ms: work_item.created_at_unix_ms,
            updated_at_unix_ms: work_item.updated_at_unix_ms,
            dirty: false,
        }
    }
}

pub(crate) fn normalize_area_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for tag in tags {
        let trimmed = tag.as_ref().trim();
        if trimmed.is_empty() || normalized.iter().any(|existing| existing == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn validate_area_tags(tags: &[String], context: &str) -> Result<(), String> {
    if tags.len() > 2 {
        return Err(format!("{context} supports at most two area_tags"));
    }
    if tags.iter().any(|tag| tag.trim().is_empty()) {
        return Err(format!("{context} area_tags must not contain blank values"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProjectOpsQuickCreateDraft, normalize_area_tags};

    #[test]
    fn normalize_area_tags_trims_blanks_and_preserves_first_unique_values() {
        assert_eq!(
            normalize_area_tags([" pm ", "", "sync", "pm"]),
            vec!["pm".to_string(), "sync".to_string()]
        );
    }

    #[test]
    fn quick_create_validation_rejects_invalid_due_and_tags() {
        let mut draft = ProjectOpsQuickCreateDraft::default();
        draft.title = "Ship PM metadata".to_string();
        draft.description = "Add due date and tags.".to_string();
        draft.due_at_unix_ms = Some(0);
        assert_eq!(
            draft.validate(),
            Err("quick create due_at_unix_ms must be > 0 when present".to_string())
        );

        draft.due_at_unix_ms = Some(1);
        draft.area_tags = vec!["pm".to_string(), "sync".to_string(), "desktop".to_string()];
        assert_eq!(
            draft.validate(),
            Err("quick create supports at most two area_tags".to_string())
        );
    }
}
