//! Accepted saved revisions. CAS storage and current project membership are supplied by the host.
use crate::{ProjectDocument, WorldNamespace, project_hash};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_SAVED_VERSIONS: usize = 128;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedRevision {
    pub version: u64,
    pub command_id: String,
    pub request_sha256: String,
    pub actor: String,
    pub created_unix: u64,
    pub summary: String,
    pub document_sha256: String,
    pub document: ProjectDocument,
    pub undo_of: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectHistory {
    pub schema: String,
    pub namespace: WorldNamespace,
    pub revisions: Vec<SavedRevision>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectFault {
    Invalid,
    Stale,
    ReusedCommand,
    Full,
    ChangedField,
    NothingToUndo,
}
impl ProjectHistory {
    pub fn new(namespace: WorldNamespace) -> Self {
        Self {
            schema: "openagents.verse.project-history.v1".into(),
            namespace,
            revisions: Vec::new(),
        }
    }
    pub fn head(&self) -> Option<&SavedRevision> {
        self.revisions.last()
    }
    pub fn version(&self) -> u64 {
        self.head().map_or(0, |r| r.version)
    }
    pub fn validate(&self) -> Result<(), ProjectFault> {
        if self.schema != "openagents.verse.project-history.v1"
            || self.revisions.len() > MAX_SAVED_VERSIONS
        {
            return Err(ProjectFault::Invalid);
        }
        for (i, r) in self.revisions.iter().enumerate() {
            if r.version != i as u64 + 1
                || r.document.namespace != self.namespace
                || r.document.sha256().ok().as_ref() != Some(&r.document_sha256)
                || self.revisions[..i]
                    .iter()
                    .any(|p| p.command_id == r.command_id)
            {
                return Err(ProjectFault::Invalid);
            }
        }
        Ok(())
    }
    fn retry(&self, id: &str, request: &str) -> Result<Option<SavedRevision>, ProjectFault> {
        match self.revisions.iter().find(|r| r.command_id == id) {
            Some(r) if r.request_sha256 == request => Ok(Some(r.clone())),
            Some(_) => Err(ProjectFault::ReusedCommand),
            None => Ok(None),
        }
    }
    pub fn save(
        &mut self,
        actor: &str,
        at: u64,
        command: &str,
        expected: u64,
        document: ProjectDocument,
        summary: &str,
    ) -> Result<SavedRevision, ProjectFault> {
        let request = project_hash(
            &serde_json::to_vec(&("save", actor, expected, &document, summary))
                .map_err(|_| ProjectFault::Invalid)?,
        );
        if let Some(r) = self.retry(command, &request)? {
            return Ok(r);
        }
        self.append(
            actor, at, command, expected, document, summary, None, request,
        )
    }
    fn append(
        &mut self,
        actor: &str,
        at: u64,
        command: &str,
        expected: u64,
        document: ProjectDocument,
        summary: &str,
        undo_of: Option<u64>,
        request: String,
    ) -> Result<SavedRevision, ProjectFault> {
        if !crate::protocol::id(actor)
            || !crate::protocol::id(command)
            || summary.trim().is_empty()
            || summary.len() > 240
            || summary.contains('\0')
            || document.namespace != self.namespace
            || document.validate().is_err()
        {
            return Err(ProjectFault::Invalid);
        }
        if self.version() != expected {
            return Err(ProjectFault::Stale);
        }
        if self.revisions.len() >= MAX_SAVED_VERSIONS {
            return Err(ProjectFault::Full);
        }
        if self.head().is_some_and(|h| {
            h.document.template.world != document.template.world
                || h.document.template.asset_set_sha256 != document.template.asset_set_sha256
                || h.document.template.scene_sha256 != document.template.scene_sha256
        }) {
            return Err(ProjectFault::Invalid);
        }
        let r = SavedRevision {
            version: expected + 1,
            command_id: command.into(),
            request_sha256: request,
            actor: actor.into(),
            created_unix: at,
            summary: summary.into(),
            document_sha256: document.sha256().map_err(|_| ProjectFault::Invalid)?,
            document,
            undo_of,
        };
        self.revisions.push(r.clone());
        Ok(r)
    }
    /// Reverts the person's last still-applied saved change. Other authors' fields survive;
    /// a subsequent edit of the same field requires an explicit new edit instead.
    pub fn undo_own(
        &mut self,
        actor: &str,
        at: u64,
        command: &str,
        expected: u64,
    ) -> Result<SavedRevision, ProjectFault> {
        let request = project_hash(
            &serde_json::to_vec(&("undo_own", actor, expected))
                .map_err(|_| ProjectFault::Invalid)?,
        );
        if let Some(r) = self.retry(command, &request)? {
            return Ok(r);
        }
        if self.version() != expected {
            return Err(ProjectFault::Stale);
        }
        let Some(index) = (1..self.revisions.len()).rev().find(|i| {
            let r = &self.revisions[*i];
            r.actor == actor
                && r.undo_of.is_none()
                && !self.revisions.iter().any(|u| u.undo_of == Some(r.version))
        }) else {
            return Err(ProjectFault::NothingToUndo);
        };
        let mut before = serde_json::to_value(&self.revisions[index - 1].document)
            .map_err(|_| ProjectFault::Invalid)?;
        let mut after = serde_json::to_value(&self.revisions[index].document)
            .map_err(|_| ProjectFault::Invalid)?;
        let mut current =
            serde_json::to_value(&self.head().ok_or(ProjectFault::NothingToUndo)?.document)
                .map_err(|_| ProjectFault::Invalid)?;
        for value in [&mut before, &mut after, &mut current] {
            value
                .as_object_mut()
                .ok_or(ProjectFault::Invalid)?
                .remove("source_revision");
        }
        compensate(&before, &after, &mut current)?;
        // The engine's local revision is separate from the saved project's version.
        current["source_revision"] =
            serde_json::json!(self.head().unwrap().document.source_revision);
        let document = serde_json::from_value(current).map_err(|_| ProjectFault::Invalid)?;
        let target = self.revisions[index].version;
        self.append(
            actor,
            at,
            command,
            expected,
            document,
            "Undid my saved change",
            Some(target),
            request,
        )
    }
}
fn compensate(before: &Value, after: &Value, current: &mut Value) -> Result<(), ProjectFault> {
    if before == after {
        return Ok(());
    }
    if let (Some(before), Some(after), Some(now)) = (
        before.as_object(),
        after.as_object(),
        current.as_object_mut(),
    ) {
        let keys: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
        for key in keys {
            if before.get(key) == after.get(key) {
                continue;
            }
            match (before.get(key), after.get(key)) {
                (Some(b), Some(a)) => {
                    let Some(c) = now.get_mut(key) else {
                        return Err(ProjectFault::ChangedField);
                    };
                    compensate(b, a, c)?;
                }
                (None, Some(a)) => {
                    if now.get(key) != Some(a) {
                        return Err(ProjectFault::ChangedField);
                    }
                    now.remove(key);
                }
                (Some(b), None) => {
                    if now.contains_key(key) {
                        return Err(ProjectFault::ChangedField);
                    }
                    now.insert(key.clone(), b.clone());
                }
                _ => {}
            }
        }
        return Ok(());
    }
    if current != after {
        return Err(ProjectFault::ChangedField);
    }
    *current = before.clone();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::tests::document;
    #[test]
    fn retries_and_interleaved_authors_cannot_silently_overwrite_a_head() {
        let mut h = ProjectHistory::new(WorldNamespace::HauntedHouse);
        let d = document();
        h.save("alice", 1, "first", 0, d.clone(), "Started the house")
            .unwrap();
        let mut a = d.clone();
        a.environment.insert("fogDensity".into(), 0.02);
        a.source_revision = 1;
        h.save("alice", 2, "alice-edit", 1, a.clone(), "Changed fog")
            .unwrap();
        let mut b = a.clone();
        b.materials
            .get_mut("fur")
            .unwrap()
            .insert("roughness".into(), 0.9);
        b.source_revision = 2;
        h.save("bob", 3, "bob-edit", 2, b.clone(), "Changed fur")
            .unwrap();
        assert_eq!(
            h.save("alice", 4, "alice-edit", 1, a.clone(), "Changed fog")
                .unwrap()
                .version,
            2
        );
        assert_eq!(
            h.save("alice", 4, "alice-edit", 1, b.clone(), "Changed fog")
                .unwrap_err(),
            ProjectFault::ReusedCommand
        );
        assert_eq!(
            h.save("alice", 4, "stale", 1, a, "Changed fog")
                .unwrap_err(),
            ProjectFault::Stale
        );
        let undone = h.undo_own("alice", 4, "undo-alice", 3).unwrap();
        assert_eq!(undone.undo_of, Some(2));
        assert_eq!(undone.document.environment["fogDensity"], 0.01);
        assert_eq!(undone.document.materials["fur"]["roughness"], 0.9);
        assert_eq!(undone.document.source_revision, 2);
        assert_eq!(h.undo_own("alice", 5, "undo-alice", 3).unwrap().version, 4);
        h.validate().unwrap();
    }
    #[test]
    fn compensating_undo_refuses_a_field_another_author_changed() {
        let mut h = ProjectHistory::new(WorldNamespace::HauntedHouse);
        let mut d = document();
        h.save("alice", 1, "start", 0, d.clone(), "Started house")
            .unwrap();
        d.environment.insert("fogDensity".into(), 0.02);
        h.save("alice", 2, "a", 1, d.clone(), "Fog").unwrap();
        d.environment.insert("fogDensity".into(), 0.03);
        h.save("bob", 3, "b", 2, d, "Fog").unwrap();
        assert_eq!(
            h.undo_own("alice", 4, "undo", 3).unwrap_err(),
            ProjectFault::ChangedField
        );
        assert_eq!(h.version(), 3);
    }
    #[test]
    fn compensation_preserves_another_authors_unrelated_deletion_and_history_full_keeps_head() {
        let mut h = ProjectHistory::new(WorldNamespace::HauntedHouse);
        let mut d = document();
        h.save("alice", 1, "start", 0, d.clone(), "Started")
            .unwrap();
        d.environment.insert("fogDensity".into(), 0.02);
        h.save("alice", 2, "alice", 1, d.clone(), "Fog").unwrap();
        d.entities.clear();
        h.save("bob", 3, "bob", 2, d.clone(), "Removed prop")
            .unwrap();
        let undone = h.undo_own("alice", 4, "undo", 3).unwrap();
        assert!(undone.document.entities.is_empty());
        assert_eq!(undone.document.environment["fogDensity"], 0.01);
        for i in 4..MAX_SAVED_VERSIONS {
            h.save(
                "bob",
                i as u64 + 1,
                &format!("save-{i}"),
                i as u64,
                d.clone(),
                "Saved",
            )
            .unwrap();
        }
        let hash = h.head().unwrap().document_sha256.clone();
        assert_eq!(
            h.save("bob", 999, "overflow", 128, d, "Too many")
                .unwrap_err(),
            ProjectFault::Full
        );
        assert_eq!(h.head().unwrap().document_sha256, hash);
        assert_eq!(h.version(), 128);
    }
}
