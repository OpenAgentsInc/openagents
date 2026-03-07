use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ProjectOpsPresentationMode, ProjectOpsSortPreference, PROJECT_OPS_DEFAULT_VIEW_ID};

pub const PROJECT_OPS_PREFERENCES_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProjectOpsPreferencesDocumentV1 {
    schema_version: u16,
    active_saved_view_id: String,
    presentation_mode: ProjectOpsPresentationMode,
    sort_preference: ProjectOpsSortPreference,
    search_query: String,
}

pub struct ProjectOpsPreferencesState {
    pub active_saved_view_id: String,
    pub presentation_mode: ProjectOpsPresentationMode,
    pub sort_preference: ProjectOpsSortPreference,
    pub search_query: String,
    preferences_path: PathBuf,
    persist_enabled: bool,
}

impl ProjectOpsPreferencesState {
    pub fn disabled() -> Self {
        Self {
            active_saved_view_id: PROJECT_OPS_DEFAULT_VIEW_ID.to_string(),
            presentation_mode: ProjectOpsPresentationMode::List,
            sort_preference: ProjectOpsSortPreference::UpdatedDesc,
            search_query: String::new(),
            preferences_path: PathBuf::new(),
            persist_enabled: false,
        }
    }

    pub fn default_enabled() -> Self {
        Self {
            preferences_path: default_preferences_path(),
            persist_enabled: true,
            ..Self::disabled()
        }
    }

    pub fn load_or_new_default() -> Result<Self, String> {
        Self::load_or_new(default_preferences_path())
    }

    #[cfg(test)]
    pub(crate) fn from_preferences_path_for_tests(
        preferences_path: PathBuf,
    ) -> Result<Self, String> {
        Self::load_or_new(preferences_path)
    }

    fn load_or_new(preferences_path: PathBuf) -> Result<Self, String> {
        let mut state = Self {
            preferences_path,
            persist_enabled: true,
            ..Self::disabled()
        };
        let Some(document) = load_preferences_document(state.preferences_path.as_path())? else {
            state.persist()?;
            return Ok(state);
        };
        if document.schema_version != PROJECT_OPS_PREFERENCES_SCHEMA_VERSION {
            state.persist()?;
            return Ok(state);
        }
        state.active_saved_view_id = normalize_active_saved_view_id(document.active_saved_view_id);
        state.presentation_mode = document.presentation_mode;
        state.sort_preference = document.sort_preference;
        state.search_query = document.search_query.trim().to_string();
        Ok(state)
    }

    pub fn set_active_saved_view_id(&mut self, active_saved_view_id: &str) -> Result<(), String> {
        self.active_saved_view_id =
            normalize_active_saved_view_id(active_saved_view_id.to_string());
        self.persist()
    }

    pub fn set_presentation_mode(
        &mut self,
        presentation_mode: ProjectOpsPresentationMode,
    ) -> Result<(), String> {
        self.presentation_mode = presentation_mode;
        self.persist()
    }

    pub fn set_sort_preference(
        &mut self,
        sort_preference: ProjectOpsSortPreference,
    ) -> Result<(), String> {
        self.sort_preference = sort_preference;
        self.persist()
    }

    pub fn set_search_query(&mut self, search_query: &str) -> Result<(), String> {
        self.search_query = search_query.trim().to_string();
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        if !self.persist_enabled {
            return Ok(());
        }
        if let Some(parent) = self.preferences_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create PM preferences dir: {error}"))?;
        }
        let document = ProjectOpsPreferencesDocumentV1 {
            schema_version: PROJECT_OPS_PREFERENCES_SCHEMA_VERSION,
            active_saved_view_id: self.active_saved_view_id.clone(),
            presentation_mode: self.presentation_mode,
            sort_preference: self.sort_preference,
            search_query: self.search_query.clone(),
        };
        let payload = serde_json::to_string_pretty(&document)
            .map_err(|error| format!("Failed to encode PM preferences: {error}"))?;
        let temp_path = self.preferences_path.with_extension("tmp");
        fs::write(temp_path.as_path(), payload)
            .map_err(|error| format!("Failed to write PM preferences temp file: {error}"))?;
        fs::rename(temp_path.as_path(), self.preferences_path.as_path())
            .map_err(|error| format!("Failed to persist PM preferences: {error}"))?;
        Ok(())
    }
}

fn normalize_active_saved_view_id(active_saved_view_id: String) -> String {
    let normalized = active_saved_view_id.trim();
    if normalized.is_empty() {
        PROJECT_OPS_DEFAULT_VIEW_ID.to_string()
    } else {
        normalized.to_string()
    }
}

fn default_preferences_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-pm-preferences-v1.json")
}

fn load_preferences_document(
    path: &Path,
) -> Result<Option<ProjectOpsPreferencesDocumentV1>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read PM preferences: {error}")),
    };
    let document = serde_json::from_str::<ProjectOpsPreferencesDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse PM preferences: {error}"))?;
    Ok(Some(document))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::ProjectOpsPreferencesState;
    use crate::project_ops::{ProjectOpsPresentationMode, ProjectOpsSortPreference};

    static UNIQUE_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let counter = UNIQUE_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openagents-project-ops-preferences-{name}-{nanos}-{counter}.json"
        ))
    }

    #[test]
    fn preferences_persist_and_reload() {
        let path = unique_temp_path("prefs");
        let mut preferences =
            ProjectOpsPreferencesState::from_preferences_path_for_tests(path.clone())
                .expect("preferences should initialize");
        preferences
            .set_active_saved_view_id("focus")
            .expect("saved view id should persist");
        preferences
            .set_presentation_mode(ProjectOpsPresentationMode::Board)
            .expect("presentation mode should persist");
        preferences
            .set_sort_preference(ProjectOpsSortPreference::PriorityDesc)
            .expect("sort preference should persist");
        preferences
            .set_search_query("blocked:true priority:high")
            .expect("search query should persist");

        let restored = ProjectOpsPreferencesState::from_preferences_path_for_tests(path)
            .expect("preferences should reload");
        assert_eq!(restored.active_saved_view_id, "focus");
        assert_eq!(
            restored.presentation_mode,
            ProjectOpsPresentationMode::Board
        );
        assert_eq!(
            restored.sort_preference,
            ProjectOpsSortPreference::PriorityDesc
        );
        assert_eq!(restored.search_query, "blocked:true priority:high");
    }

    #[test]
    fn unsupported_schema_versions_reset_to_defaults() {
        let path = unique_temp_path("prefs-unsupported");
        std::fs::write(
            &path,
            serde_json::json!({
                "schema_version": super::PROJECT_OPS_PREFERENCES_SCHEMA_VERSION + 1,
                "active_saved_view_id": "focus",
                "presentation_mode": "board",
                "sort_preference": "priority_desc",
                "search_query": "blocked:true",
            })
            .to_string(),
        )
        .expect("unsupported preferences doc should write");

        let restored = ProjectOpsPreferencesState::from_preferences_path_for_tests(path)
            .expect("preferences should fall back to defaults");
        assert_eq!(restored.active_saved_view_id, "my-work");
        assert_eq!(restored.presentation_mode, ProjectOpsPresentationMode::List);
        assert_eq!(
            restored.sort_preference,
            ProjectOpsSortPreference::UpdatedDesc
        );
        assert!(restored.search_query.is_empty());
    }
}
