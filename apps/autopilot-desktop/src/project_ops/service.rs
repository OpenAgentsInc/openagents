use super::contract::{
    PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_WORK_ITEMS_STREAM_ID,
    ProjectOpsAcceptedEvent, ProjectOpsAcceptedEventEnvelope, ProjectOpsActor, ProjectOpsCommand,
    ProjectOpsCommandEnvelope, ProjectOpsCommandId, ProjectOpsEditWorkItemFieldsPatch,
    ProjectOpsErrorCode, project_ops_error,
};
use super::projection::{ProjectOpsActivityRow, ProjectOpsProjectionStore};
use super::schema::{ProjectOpsWorkItem, ProjectOpsWorkItemId, ProjectOpsWorkItemStatus};

pub struct ProjectOpsService {
    pub projections: ProjectOpsProjectionStore,
}

#[derive(Debug)]
pub struct ProjectOpsCommandApplyResult {
    pub work_item_id: ProjectOpsWorkItemId,
    pub accepted_events: Vec<ProjectOpsAcceptedEventEnvelope>,
}

#[derive(Debug)]
pub enum ProjectOpsCommandResult {
    Applied(ProjectOpsCommandApplyResult),
    DuplicateCommand { command_id: ProjectOpsCommandId },
}

impl Default for ProjectOpsService {
    fn default() -> Self {
        Self {
            projections: ProjectOpsProjectionStore::load_or_bootstrap_default(),
        }
    }
}

impl ProjectOpsService {
    pub fn from_projection_store(projections: ProjectOpsProjectionStore) -> Self {
        Self { projections }
    }

    pub fn into_projection_store(self) -> ProjectOpsProjectionStore {
        self.projections
    }

    pub fn apply_command_to_store(
        projections: &mut ProjectOpsProjectionStore,
        envelope: ProjectOpsCommandEnvelope,
    ) -> Result<ProjectOpsCommandResult, String> {
        let mut service = Self::from_projection_store(std::mem::replace(
            projections,
            ProjectOpsProjectionStore::disabled(),
        ));
        let result = service.apply_command(envelope);
        *projections = service.into_projection_store();
        result
    }

    pub fn apply_command(
        &mut self,
        envelope: ProjectOpsCommandEnvelope,
    ) -> Result<ProjectOpsCommandResult, String> {
        envelope
            .validate()
            .map_err(|error| project_ops_error(ProjectOpsErrorCode::InvalidCommand, error))?;
        self.projections.reload_shared_checkpoints()?;
        if self.command_already_applied(&envelope.command_id) {
            return Ok(ProjectOpsCommandResult::DuplicateCommand {
                command_id: envelope.command_id,
            });
        }

        let work_items_seq = self
            .projections
            .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);
        let activity_seq = self
            .projections
            .checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID)
            .unwrap_or(0)
            .saturating_add(1);

        let (work_item_id, next_work_items, accepted_event) =
            self.reduce_command(&envelope, envelope.command.clone())?;
        let activity_row = build_activity_row(
            &accepted_event,
            envelope.command_id.as_str(),
            &envelope.actor,
            envelope.issued_at_unix_ms,
        );

        self.projections
            .apply_work_items_projection(work_items_seq, next_work_items)?;
        self.projections.apply_activity_projection(
            activity_seq,
            push_activity_row(self.projections.activity_rows.clone(), activity_row),
        )?;

        Ok(ProjectOpsCommandResult::Applied(
            ProjectOpsCommandApplyResult {
                work_item_id,
                accepted_events: vec![ProjectOpsAcceptedEventEnvelope {
                    stream_id: PROJECT_OPS_WORK_ITEMS_STREAM_ID.to_string(),
                    seq: work_items_seq,
                    command_id: envelope.command_id,
                    emitted_at_unix_ms: envelope.issued_at_unix_ms,
                    actor: envelope.actor,
                    event: accepted_event,
                }],
            },
        ))
    }

    fn command_already_applied(&self, command_id: &ProjectOpsCommandId) -> bool {
        self.projections
            .activity_rows
            .iter()
            .any(|row| row.command_id == command_id.as_str())
    }

    fn reduce_command(
        &self,
        envelope: &ProjectOpsCommandEnvelope,
        command: ProjectOpsCommand,
    ) -> Result<
        (
            ProjectOpsWorkItemId,
            Vec<ProjectOpsWorkItem>,
            ProjectOpsAcceptedEvent,
        ),
        String,
    > {
        let mut work_items = self.projections.work_items.clone();
        match command {
            ProjectOpsCommand::CreateWorkItem(command) => {
                if work_items
                    .iter()
                    .any(|existing| existing.work_item_id == command.draft.work_item_id)
                {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::WorkItemExists,
                        format!(
                            "work item {} already exists",
                            command.draft.work_item_id.as_str()
                        ),
                    ));
                }
                let work_item_id = command.draft.work_item_id.clone();
                if let Some(project_id) = command.draft.project_id.as_ref() {
                    if !self
                        .projections
                        .projects
                        .iter()
                        .any(|project| &project.project_id == project_id)
                    {
                        return Err(project_ops_error(
                            ProjectOpsErrorCode::DependencyMissing,
                            format!(
                                "project {} does not exist in the PM projects projection",
                                project_id.as_str()
                            ),
                        ));
                    }
                }
                let work_item = ProjectOpsWorkItem {
                    work_item_id: work_item_id.clone(),
                    title: command.draft.title,
                    description: command.draft.description,
                    status: command.draft.status,
                    priority: command.draft.priority,
                    assignee: command.draft.assignee,
                    team_key: command.draft.team_key,
                    project_id: command.draft.project_id,
                    cycle_id: command.draft.cycle_id,
                    parent_id: command.draft.parent_id,
                    area_tags: command.draft.area_tags,
                    blocked_reason: command.draft.blocked_reason,
                    due_at_unix_ms: command.draft.due_at_unix_ms,
                    created_at_unix_ms: envelope.issued_at_unix_ms,
                    updated_at_unix_ms: envelope.issued_at_unix_ms,
                    archived_at_unix_ms: None,
                };
                work_item.validate().map_err(|error| {
                    project_ops_error(ProjectOpsErrorCode::InvalidCommand, error)
                })?;
                work_items.push(work_item.clone());
                Ok((
                    work_item_id,
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemCreated { work_item },
                ))
            }
            ProjectOpsCommand::EditWorkItemFields(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "edit fields")?;
                let mut next = current.clone();
                apply_edit_patch(&mut next, &command.patch)?;
                if let Some(project_id) = next.project_id.as_ref() {
                    if !self
                        .projections
                        .projects
                        .iter()
                        .any(|project| &project.project_id == project_id)
                    {
                        return Err(project_ops_error(
                            ProjectOpsErrorCode::DependencyMissing,
                            format!(
                                "project {} does not exist in the PM projects projection",
                                project_id.as_str()
                            ),
                        ));
                    }
                }
                if next == *current {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} edit patch did not change any fields",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                next.validate().map_err(|error| {
                    project_ops_error(ProjectOpsErrorCode::InvalidCommand, error)
                })?;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemFieldsEdited(
                        super::contract::ProjectOpsWorkItemFieldsEdited {
                            work_item_id: command.work_item_id,
                            patch: command.patch,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::ChangeWorkItemStatus(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "change status")?;
                validate_status_transition(current.status, command.status)?;
                let mut next = current.clone();
                next.status = command.status;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemStatusChanged(
                        super::contract::ProjectOpsWorkItemStatusChanged {
                            work_item_id: command.work_item_id,
                            status: command.status,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::AssignWorkItem(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "assign work item")?;
                if current.assignee.as_deref() == Some(command.assignee.as_str()) {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} is already assigned to {}",
                            command.work_item_id.as_str(),
                            command.assignee
                        ),
                    ));
                }
                let mut next = current.clone();
                next.assignee = Some(command.assignee.clone());
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemAssigned(
                        super::contract::ProjectOpsWorkItemAssigned {
                            work_item_id: command.work_item_id,
                            assignee: command.assignee,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::ClearAssignee(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "clear assignee")?;
                if current.assignee.is_none() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} does not have an assignee",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.assignee = None;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemAssigneeCleared(command),
                ))
            }
            ProjectOpsCommand::SetWorkItemCycle(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "set cycle")?;
                if !self
                    .projections
                    .cycles
                    .iter()
                    .any(|cycle| cycle.cycle_id == command.cycle_id)
                {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::DependencyMissing,
                        format!(
                            "cycle {} does not exist in the PM cycles projection",
                            command.cycle_id.as_str()
                        ),
                    ));
                }
                if current.cycle_id.as_ref() == Some(&command.cycle_id) {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} is already in cycle {}",
                            command.work_item_id.as_str(),
                            command.cycle_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.cycle_id = Some(command.cycle_id.clone());
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemCycleSet(
                        super::contract::ProjectOpsWorkItemCycleSet {
                            work_item_id: command.work_item_id,
                            cycle_id: command.cycle_id,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::ClearWorkItemCycle(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "clear cycle")?;
                if current.cycle_id.is_none() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} does not have a cycle",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.cycle_id = None;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemCycleCleared(command),
                ))
            }
            ProjectOpsCommand::SetBlockedReason(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "set blocked reason")?;
                if current.blocked_reason.as_deref() == Some(command.blocked_reason.as_str()) {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} already has that blocked reason",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.blocked_reason = Some(command.blocked_reason.clone());
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemBlocked(
                        super::contract::ProjectOpsWorkItemBlocked {
                            work_item_id: command.work_item_id,
                            blocked_reason: command.blocked_reason,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::ClearBlockedReason(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "clear blocked reason")?;
                if current.blocked_reason.is_none() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!("work item {} is not blocked", command.work_item_id.as_str()),
                    ));
                }
                let mut next = current.clone();
                next.blocked_reason = None;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemUnblocked(command),
                ))
            }
            ProjectOpsCommand::SetParentWorkItem(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "set parent")?;
                if command.work_item_id == command.parent_id {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::InvalidCommand,
                        "work item cannot be its own parent",
                    ));
                }
                if !work_items
                    .iter()
                    .any(|item| item.work_item_id == command.parent_id)
                {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::DependencyMissing,
                        format!(
                            "parent work item {} does not exist",
                            command.parent_id.as_str()
                        ),
                    ));
                }
                if parent_assignment_creates_cycle(
                    work_items.as_slice(),
                    &command.work_item_id,
                    &command.parent_id,
                ) {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::InvalidCommand,
                        format!(
                            "setting parent {} for {} would create a parent cycle",
                            command.parent_id.as_str(),
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                if current.parent_id.as_ref() == Some(&command.parent_id) {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} already has parent {}",
                            command.work_item_id.as_str(),
                            command.parent_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.parent_id = Some(command.parent_id.clone());
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemParentSet(
                        super::contract::ProjectOpsWorkItemParentSet {
                            work_item_id: command.work_item_id,
                            parent_id: command.parent_id,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::ClearParentWorkItem(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                reject_archived_mutation(current, "clear parent")?;
                if current.parent_id.is_none() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} does not have a parent",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.parent_id = None;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemParentCleared(command),
                ))
            }
            ProjectOpsCommand::ArchiveWorkItem(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                if current.archived_at_unix_ms.is_some() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::ArchivedMutation,
                        format!(
                            "work item {} is already archived",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.archived_at_unix_ms = Some(envelope.issued_at_unix_ms);
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemArchived(
                        super::contract::ProjectOpsWorkItemArchived {
                            work_item_id: command.work_item_id,
                            archived_at_unix_ms: envelope.issued_at_unix_ms,
                        },
                    ),
                ))
            }
            ProjectOpsCommand::UnarchiveWorkItem(command) => {
                let index = find_work_item_index(work_items.as_slice(), &command.work_item_id)?;
                let current = &work_items[index];
                if current.archived_at_unix_ms.is_none() {
                    return Err(project_ops_error(
                        ProjectOpsErrorCode::NoopMutation,
                        format!(
                            "work item {} is not archived",
                            command.work_item_id.as_str()
                        ),
                    ));
                }
                let mut next = current.clone();
                next.archived_at_unix_ms = None;
                next.updated_at_unix_ms = envelope.issued_at_unix_ms;
                work_items[index] = next;
                Ok((
                    command.work_item_id.clone(),
                    work_items,
                    ProjectOpsAcceptedEvent::WorkItemUnarchived(command),
                ))
            }
        }
    }
}

fn push_activity_row(
    mut activity_rows: Vec<ProjectOpsActivityRow>,
    row: ProjectOpsActivityRow,
) -> Vec<ProjectOpsActivityRow> {
    activity_rows.push(row);
    activity_rows
}

fn build_activity_row(
    event: &ProjectOpsAcceptedEvent,
    command_id: &str,
    actor: &ProjectOpsActor,
    occurred_at_unix_ms: u64,
) -> ProjectOpsActivityRow {
    ProjectOpsActivityRow {
        event_id: format!(
            "pm.activity:{}:{}:{}",
            command_id,
            event.name().label(),
            occurred_at_unix_ms
        ),
        work_item_id: work_item_id_for_event(event),
        event_name: event.name(),
        summary: activity_summary_for_event(event),
        actor_label: actor_display_label(actor),
        command_id: command_id.to_string(),
        occurred_at_unix_ms,
    }
}

fn work_item_id_for_event(event: &ProjectOpsAcceptedEvent) -> ProjectOpsWorkItemId {
    match event {
        ProjectOpsAcceptedEvent::WorkItemCreated { work_item } => work_item.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemFieldsEdited(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemStatusChanged(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemAssigned(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemAssigneeCleared(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemCycleSet(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemCycleCleared(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemBlocked(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemUnblocked(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemParentSet(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemParentCleared(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemArchived(event) => event.work_item_id.clone(),
        ProjectOpsAcceptedEvent::WorkItemUnarchived(event) => event.work_item_id.clone(),
    }
}

fn activity_summary_for_event(event: &ProjectOpsAcceptedEvent) -> String {
    match event {
        ProjectOpsAcceptedEvent::WorkItemCreated { work_item } => {
            format!(
                "Created {} ({})",
                work_item.work_item_id.as_str(),
                work_item.title
            )
        }
        ProjectOpsAcceptedEvent::WorkItemFieldsEdited(event) => {
            format!("Edited fields on {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemStatusChanged(event) => format!(
            "Changed {} status to {}",
            event.work_item_id.as_str(),
            event.status.label()
        ),
        ProjectOpsAcceptedEvent::WorkItemAssigned(event) => format!(
            "Assigned {} to {}",
            event.work_item_id.as_str(),
            event.assignee
        ),
        ProjectOpsAcceptedEvent::WorkItemAssigneeCleared(event) => {
            format!("Cleared assignee on {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemCycleSet(event) => format!(
            "Moved {} into cycle {}",
            event.work_item_id.as_str(),
            event.cycle_id.as_str()
        ),
        ProjectOpsAcceptedEvent::WorkItemCycleCleared(event) => {
            format!("Cleared cycle on {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemBlocked(event) => format!(
            "Blocked {}: {}",
            event.work_item_id.as_str(),
            event.blocked_reason
        ),
        ProjectOpsAcceptedEvent::WorkItemUnblocked(event) => {
            format!("Unblocked {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemParentSet(event) => format!(
            "Set {} parent to {}",
            event.work_item_id.as_str(),
            event.parent_id.as_str()
        ),
        ProjectOpsAcceptedEvent::WorkItemParentCleared(event) => {
            format!("Cleared parent on {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemArchived(event) => {
            format!("Archived {}", event.work_item_id.as_str())
        }
        ProjectOpsAcceptedEvent::WorkItemUnarchived(event) => {
            format!("Unarchived {}", event.work_item_id.as_str())
        }
    }
}

fn actor_display_label(actor: &ProjectOpsActor) -> String {
    actor
        .actor_label
        .clone()
        .or_else(|| actor.actor_id.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn parent_assignment_creates_cycle(
    work_items: &[ProjectOpsWorkItem],
    work_item_id: &ProjectOpsWorkItemId,
    candidate_parent_id: &ProjectOpsWorkItemId,
) -> bool {
    let mut cursor = Some(candidate_parent_id);
    while let Some(current_parent_id) = cursor {
        if current_parent_id == work_item_id {
            return true;
        }
        cursor = work_items
            .iter()
            .find(|item| &item.work_item_id == current_parent_id)
            .and_then(|item| item.parent_id.as_ref());
    }
    false
}

fn reject_archived_mutation(work_item: &ProjectOpsWorkItem, action: &str) -> Result<(), String> {
    if work_item.archived_at_unix_ms.is_some() {
        return Err(project_ops_error(
            ProjectOpsErrorCode::ArchivedMutation,
            format!(
                "cannot {} archived work item {}",
                action,
                work_item.work_item_id.as_str()
            ),
        ));
    }
    Ok(())
}

fn find_work_item_index(
    work_items: &[ProjectOpsWorkItem],
    work_item_id: &ProjectOpsWorkItemId,
) -> Result<usize, String> {
    work_items
        .iter()
        .position(|item| &item.work_item_id == work_item_id)
        .ok_or_else(|| {
            project_ops_error(
                ProjectOpsErrorCode::WorkItemMissing,
                format!("work item {} does not exist", work_item_id.as_str()),
            )
        })
}

fn apply_edit_patch(
    work_item: &mut ProjectOpsWorkItem,
    patch: &ProjectOpsEditWorkItemFieldsPatch,
) -> Result<(), String> {
    patch
        .validate()
        .map_err(|error| project_ops_error(ProjectOpsErrorCode::InvalidCommand, error))?;
    if let Some(title) = patch.title.as_ref() {
        work_item.title = title.clone();
    }
    if let Some(description) = patch.description.as_ref() {
        work_item.description = description.clone();
    }
    if let Some(priority) = patch.priority {
        work_item.priority = priority;
    }
    if let Some(project_id) = patch.project_id.as_ref() {
        work_item.project_id = project_id.clone();
    }
    if let Some(due_at_unix_ms) = patch.due_at_unix_ms {
        work_item.due_at_unix_ms = due_at_unix_ms;
    }
    if let Some(area_tags) = patch.area_tags.as_ref() {
        work_item.area_tags = area_tags.clone();
    }
    Ok(())
}

fn validate_status_transition(
    current: ProjectOpsWorkItemStatus,
    target: ProjectOpsWorkItemStatus,
) -> Result<(), String> {
    if current == target {
        return Err(project_ops_error(
            ProjectOpsErrorCode::NoopMutation,
            format!("work item is already in status {}", target.label()),
        ));
    }
    let allowed = allowed_status_targets(current);
    if allowed.contains(&target) {
        return Ok(());
    }
    Err(project_ops_error(
        ProjectOpsErrorCode::InvalidTransition,
        format!(
            "invalid status transition: {} -> {}",
            current.label(),
            target.label()
        ),
    ))
}

fn allowed_status_targets(
    current: ProjectOpsWorkItemStatus,
) -> &'static [ProjectOpsWorkItemStatus] {
    use ProjectOpsWorkItemStatus as Status;

    match current {
        Status::Backlog => &[Status::Todo, Status::InProgress, Status::Cancelled],
        Status::Todo => &[Status::Backlog, Status::InProgress, Status::Cancelled],
        Status::InProgress => &[Status::Todo, Status::InReview, Status::Cancelled],
        Status::InReview => &[Status::InProgress, Status::Done, Status::Cancelled],
        Status::Done => &[Status::InReview, Status::Cancelled],
        Status::Cancelled => &[Status::Backlog],
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{ProjectOpsCommandResult, ProjectOpsService};
    use crate::project_ops::contract::{
        PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, PROJECT_OPS_WORK_ITEMS_STREAM_ID,
        ProjectOpsActor, ProjectOpsAssignWorkItem, ProjectOpsChangeWorkItemStatus,
        ProjectOpsCommand, ProjectOpsCommandEnvelope, ProjectOpsCommandId,
        ProjectOpsCreateWorkItem, ProjectOpsEditWorkItemFields, ProjectOpsEditWorkItemFieldsPatch,
        ProjectOpsSetParentWorkItem, ProjectOpsSetWorkItemCycle, ProjectOpsWorkItemDraft,
    };
    use crate::project_ops::projection::{ProjectOpsCycleRow, ProjectOpsProjectionStore};
    use crate::project_ops::schema::{
        ProjectOpsCycleId, ProjectOpsPriority, ProjectOpsProjectId, ProjectOpsTeamKey,
        ProjectOpsWorkItemStatus,
    };
    use crate::sync_apply::StreamApplyDecision;

    static UNIQUE_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = UNIQUE_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openagents-project-ops-service-{name}-{nanos}-{counter}.json"
        ))
    }

    fn actor() -> ProjectOpsActor {
        ProjectOpsActor {
            actor_id: Some("npub1actor".to_string()),
            actor_label: Some("cdavid".to_string()),
        }
    }

    fn command_envelope(
        command_id: &str,
        issued_at_unix_ms: u64,
        command: ProjectOpsCommand,
    ) -> ProjectOpsCommandEnvelope {
        ProjectOpsCommandEnvelope {
            command_id: ProjectOpsCommandId::new(command_id).expect("command id"),
            issued_at_unix_ms,
            actor: actor(),
            command,
        }
    }

    fn draft(status: ProjectOpsWorkItemStatus) -> ProjectOpsWorkItemDraft {
        ProjectOpsWorkItemDraft {
            work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                .expect("work item id"),
            title: "Ship PM reducer".to_string(),
            description: "Accept PM commands and emit PM events.".to_string(),
            status,
            priority: ProjectOpsPriority::High,
            assignee: None,
            team_key: ProjectOpsTeamKey::new("desktop").expect("team key"),
            project_id: Some(ProjectOpsProjectId::new("desktop-pm").expect("project id")),
            cycle_id: None,
            parent_id: None,
            area_tags: vec!["pm".to_string()],
            blocked_reason: None,
            due_at_unix_ms: None,
        }
    }

    fn cycle_row() -> ProjectOpsCycleRow {
        ProjectOpsCycleRow {
            cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
            title: "Week 10".to_string(),
            goal: Some("Land the PM service loop".to_string()),
            starts_at_unix_ms: 1_761_998_400_000,
            ends_at_unix_ms: 1_762_603_200_000,
            is_active: true,
        }
    }

    fn service() -> ProjectOpsService {
        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            unique_temp_path("work-items"),
            unique_temp_path("activity"),
            unique_temp_path("cycles"),
            unique_temp_path("saved-views"),
            unique_temp_path("projects"),
            unique_temp_path("teams"),
            unique_temp_path("checkpoints"),
        );
        ProjectOpsService::from_projection_store(store)
    }

    #[test]
    fn create_and_edit_flow_emits_events_and_updates_projections() {
        let mut service = service();
        let create = command_envelope(
            "cmd-1",
            1_762_000_000_000,
            ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                draft: draft(ProjectOpsWorkItemStatus::Backlog),
            }),
        );
        let result = service
            .apply_command(create)
            .expect("create should succeed");
        let ProjectOpsCommandResult::Applied(result) = result else {
            panic!("expected applied result");
        };
        assert_eq!(result.accepted_events.len(), 1);
        assert_eq!(service.projections.work_items.len(), 1);
        assert_eq!(service.projections.activity_rows.len(), 1);
        assert_eq!(
            service
                .projections
                .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(1)
        );
        assert_eq!(
            service
                .projections
                .checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(1)
        );

        let edit = command_envelope(
            "cmd-2",
            1_762_000_100_000,
            ProjectOpsCommand::EditWorkItemFields(ProjectOpsEditWorkItemFields {
                work_item_id: result.work_item_id,
                patch: ProjectOpsEditWorkItemFieldsPatch {
                    title: Some("Ship deterministic PM reducer".to_string()),
                    description: None,
                    priority: None,
                    project_id: None,
                    due_at_unix_ms: Some(Some(1_762_600_000_000)),
                    area_tags: None,
                },
            }),
        );
        let result = service.apply_command(edit).expect("edit should succeed");
        let ProjectOpsCommandResult::Applied(result) = result else {
            panic!("expected applied edit");
        };
        assert_eq!(result.accepted_events.len(), 1);
        assert_eq!(
            service.projections.work_items[0].title,
            "Ship deterministic PM reducer"
        );
        assert_eq!(service.projections.activity_rows.len(), 2);
    }

    #[test]
    fn apply_command_reloads_shared_checkpoints_before_assigning_seq() {
        let work_items_path = unique_temp_path("reload-work-items");
        let activity_path = unique_temp_path("reload-activity");
        let cycles_path = unique_temp_path("reload-cycles");
        let saved_views_path = unique_temp_path("reload-saved-views");
        let projects_path = unique_temp_path("reload-projects");
        let teams_path = unique_temp_path("reload-teams");
        let checkpoint_path = unique_temp_path("reload-checkpoints");
        let store = ProjectOpsProjectionStore::from_paths_for_tests(
            work_items_path,
            activity_path,
            cycles_path,
            saved_views_path,
            projects_path,
            teams_path,
            checkpoint_path.clone(),
        );
        let mut external = crate::sync_apply::SyncApplyEngine::load_or_new(
            checkpoint_path,
            crate::sync_apply::SyncApplyPolicy::default(),
        )
        .expect("external checkpoint engine should initialize");
        external
            .adopt_checkpoint_if_newer(PROJECT_OPS_WORK_ITEMS_STREAM_ID, 4)
            .expect("work item checkpoint should adopt");
        external
            .adopt_checkpoint_if_newer(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID, 8)
            .expect("activity checkpoint should adopt");

        let mut service = ProjectOpsService::from_projection_store(store);
        let result = service
            .apply_command(command_envelope(
                "cmd-reload",
                1_762_000_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: draft(ProjectOpsWorkItemStatus::Backlog),
                }),
            ))
            .expect("create should succeed with reloaded checkpoints");

        let ProjectOpsCommandResult::Applied(result) = result else {
            panic!("expected applied result");
        };
        assert_eq!(result.accepted_events[0].seq, 5);
        assert_eq!(
            service
                .projections
                .checkpoint_for(PROJECT_OPS_WORK_ITEMS_STREAM_ID),
            Some(5)
        );
        assert_eq!(
            service
                .projections
                .checkpoint_for(PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
            Some(9)
        );
    }

    #[test]
    fn invalid_transitions_are_rejected_clearly() {
        let mut service = service();
        service
            .apply_command(command_envelope(
                "cmd-1",
                1_762_000_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: draft(ProjectOpsWorkItemStatus::Backlog),
                }),
            ))
            .expect("create should succeed");

        let error = service
            .apply_command(command_envelope(
                "cmd-2",
                1_762_000_100_000,
                ProjectOpsCommand::ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    status: ProjectOpsWorkItemStatus::Done,
                }),
            ))
            .expect_err("backlog -> done should reject");
        assert!(error.contains("project_ops.invalid_transition:"));
        assert!(error.contains("invalid status transition"));

        let error = service
            .apply_command(command_envelope(
                "cmd-3",
                1_762_000_200_000,
                ProjectOpsCommand::AssignWorkItem(ProjectOpsAssignWorkItem {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("missing")
                        .expect("work item id"),
                    assignee: "cdavid".to_string(),
                }),
            ))
            .expect_err("missing work item should reject");
        assert!(error.contains("project_ops.work_item_missing:"));
        assert!(error.contains("does not exist"));
    }

    #[test]
    fn parent_assignment_rejects_indirect_cycles() {
        let mut service = service();
        let _ = service
            .apply_command(command_envelope(
                "cmd-1",
                1_762_000_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: draft(ProjectOpsWorkItemStatus::Todo),
                }),
            ))
            .expect("first item should create");
        let _ = service
            .apply_command(command_envelope(
                "cmd-2",
                1_762_000_010_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: ProjectOpsWorkItemDraft {
                        work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-2")
                            .expect("work item id"),
                        ..draft(ProjectOpsWorkItemStatus::Todo)
                    },
                }),
            ))
            .expect("second item should create");
        let _ = service
            .apply_command(command_envelope(
                "cmd-3",
                1_762_000_020_000,
                ProjectOpsCommand::SetParentWorkItem(ProjectOpsSetParentWorkItem {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    parent_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-2")
                        .expect("work item id"),
                }),
            ))
            .expect("first parent assignment should succeed");

        let error = service
            .apply_command(command_envelope(
                "cmd-4",
                1_762_000_030_000,
                ProjectOpsCommand::SetParentWorkItem(ProjectOpsSetParentWorkItem {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-2")
                        .expect("work item id"),
                    parent_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                }),
            ))
            .expect_err("cyclic parent assignment should reject");
        assert!(error.contains("project_ops.invalid_command:"));
        assert!(error.contains("would create a parent cycle"));
    }

    #[test]
    fn duplicate_command_id_is_ignored() {
        let mut service = service();
        let create = command_envelope(
            "cmd-1",
            1_762_000_000_000,
            ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                draft: draft(ProjectOpsWorkItemStatus::Backlog),
            }),
        );
        let _ = service
            .apply_command(create.clone())
            .expect("first apply should succeed");
        let duplicate = service
            .apply_command(create)
            .expect("duplicate command should be handled");
        match duplicate {
            ProjectOpsCommandResult::DuplicateCommand { command_id } => {
                assert_eq!(command_id.as_str(), "cmd-1");
            }
            ProjectOpsCommandResult::Applied(_) => panic!("expected duplicate result"),
        }
        assert_eq!(service.projections.work_items.len(), 1);
        assert_eq!(service.projections.activity_rows.len(), 1);
    }

    #[test]
    fn cycle_assignment_requires_existing_cycle_projection() {
        let mut service = service();
        let _ = service
            .apply_command(command_envelope(
                "cmd-1",
                1_762_000_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: draft(ProjectOpsWorkItemStatus::Todo),
                }),
            ))
            .expect("create should succeed");

        let error = service
            .apply_command(command_envelope(
                "cmd-2",
                1_762_000_100_000,
                ProjectOpsCommand::SetWorkItemCycle(ProjectOpsSetWorkItemCycle {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
                }),
            ))
            .expect_err("missing cycle should reject");
        assert!(error.contains("project_ops.dependency_missing:"));
        assert!(error.contains("does not exist in the PM cycles projection"));

        assert!(matches!(
            service
                .projections
                .apply_cycles_projection(1, vec![cycle_row()])
                .expect("cycle projection should apply"),
            StreamApplyDecision::Applied { .. }
        ));

        let result = service
            .apply_command(command_envelope(
                "cmd-3",
                1_762_000_200_000,
                ProjectOpsCommand::SetWorkItemCycle(ProjectOpsSetWorkItemCycle {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    cycle_id: ProjectOpsCycleId::new("2026-w10").expect("cycle id"),
                }),
            ))
            .expect("existing cycle should allow assignment");
        assert!(matches!(result, ProjectOpsCommandResult::Applied(_)));
        assert_eq!(
            service.projections.work_items[0]
                .cycle_id
                .as_ref()
                .map(|cycle| cycle.as_str()),
            Some("2026-w10")
        );
    }

    #[test]
    fn archived_and_noop_rejections_use_stable_error_codes() {
        let mut service = service();
        let _ = service
            .apply_command(command_envelope(
                "cmd-1",
                1_762_000_000_000,
                ProjectOpsCommand::CreateWorkItem(ProjectOpsCreateWorkItem {
                    draft: draft(ProjectOpsWorkItemStatus::Todo),
                }),
            ))
            .expect("create should succeed");
        let _ = service
            .apply_command(command_envelope(
                "cmd-2",
                1_762_000_050_000,
                ProjectOpsCommand::ArchiveWorkItem(
                    crate::project_ops::contract::ProjectOpsWorkItemRef {
                        work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                            .expect("work item id"),
                    },
                ),
            ))
            .expect("archive should succeed");

        let archived_error = service
            .apply_command(command_envelope(
                "cmd-3",
                1_762_000_100_000,
                ProjectOpsCommand::ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    status: ProjectOpsWorkItemStatus::InProgress,
                }),
            ))
            .expect_err("archived mutation should reject");
        assert!(archived_error.contains("project_ops.archived_mutation:"));

        let noop_error = service
            .apply_command(command_envelope(
                "cmd-4",
                1_762_000_150_000,
                ProjectOpsCommand::UnarchiveWorkItem(
                    crate::project_ops::contract::ProjectOpsWorkItemRef {
                        work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                            .expect("work item id"),
                    },
                ),
            ))
            .expect("unarchive should succeed");
        assert!(matches!(noop_error, ProjectOpsCommandResult::Applied(_)));

        let noop_error = service
            .apply_command(command_envelope(
                "cmd-5",
                1_762_000_200_000,
                ProjectOpsCommand::ChangeWorkItemStatus(ProjectOpsChangeWorkItemStatus {
                    work_item_id: crate::project_ops::schema::ProjectOpsWorkItemId::new("wi-1")
                        .expect("work item id"),
                    status: ProjectOpsWorkItemStatus::Todo,
                }),
            ))
            .expect_err("same status should reject");
        assert!(noop_error.contains("project_ops.noop_mutation:"));
    }
}
