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
    pub area_tags: Vec<String>,
    pub due_at_unix_ms: Option<u64>,
    pub promotion_ledger: Option<PromotionLedger>,
}

impl Default for ProjectOpsQuickCreateDraft {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            priority: ProjectOpsPriority::Medium,
            team_key: ProjectOpsTeamKey::new("desktop")
                .unwrap_or_else(|_| ProjectOpsTeamKey::new("pm").expect("fallback team")),
            area_tags: vec!["pm".to_string()],
            due_at_unix_ms: None,
            promotion_ledger: None,
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
        validate_area_tags(&self.area_tags, "quick create")?;
        if let Some(due_at_unix_ms) = self.due_at_unix_ms {
            if due_at_unix_ms == 0 {
                return Err("quick create due_at_unix_ms must be > 0 when present".to_string());
            }
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
            area_tags: self.area_tags.clone(),
            blocked_reason: None,
            due_at_unix_ms: self.due_at_unix_ms,
            promotion_ledger: self.promotion_ledger.clone(),
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
    pub area_tags: Vec<String>,
    pub blocked_reason: Option<String>,
    pub due_at_unix_ms: Option<u64>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub dirty: bool,
    pub promotion_ledger: Option<PromotionLedger>,
    pub shadow_rollout_state: Option<ShadowRolloutState>,
    pub rollback_history: Vec<RollbackHistory>,
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
            area_tags: work_item.area_tags.clone(),
            blocked_reason: work_item.blocked_reason.clone(),
            due_at_unix_ms: work_item.due_at_unix_ms,
            created_at_unix_ms: work_item.created_at_unix_ms,
            updated_at_unix_ms: work_item.updated_at_unix_ms,
            dirty: false,
            promotion_ledger: work_item.promotion_ledger.clone(),
            shadow_rollout_state: work_item.shadow_rollout_state.clone(),
            rollback_history: work_item.rollback_history.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionLedger {
    pub admitted_improvements: Vec<AdmittedImprovement>,
    pub promoted_revisions: Vec<PromotedRevision>,
    pub rollback_history: Vec<RollbackHistory>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedImprovement {
    pub id: String,
    pub description: String,
    pub admission_decision: AdmissionDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionDecision {
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotedRevision {
    pub id: String,
    pub description: String,
    pub promotion_date: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackHistory {
    pub id: String,
    pub description: String,
    pub rollback_date: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShadowRolloutState {
    Shadow,
    Promoted,
    RolledBack,
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
            normalize_area_tags(vec!["  a", "b", "a", "  ", "c"]),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn validate_area_tags() {
        let valid_tags = vec!["a".to_string(), "b".to_string()];
        assert!(validate_area_tags(&valid_tags, "quick create").is_ok());

        let too_many_tags = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(validate_area_tags(&too_many_tags, "quick create").is_err());

        let empty_tag = vec!["".to_string()];
        assert!(validate_area_tags(&empty_tag, "quick create").is_err());
    }
}