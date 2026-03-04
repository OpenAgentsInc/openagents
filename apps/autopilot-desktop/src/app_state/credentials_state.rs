use super::{CredentialsPaneInputs, PaneLoadState, PaneStatusAccess};

pub struct CredentialsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub entries: Vec<crate::credentials::CredentialRecord>,
    pub selected_name: Option<String>,
    repository: crate::credentials::CredentialRepository,
}

impl Default for CredentialsState {
    fn default() -> Self {
        Self::load_from_disk()
    }
}

impl CredentialsState {
    pub fn load_from_disk() -> Self {
        let repository = crate::credentials::CredentialRepository::new();
        let mut state = Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Credential manager ready.".to_string()),
            entries: Vec::new(),
            selected_name: None,
            repository,
        };

        match state.repository.load_records() {
            Ok(records) => {
                state.entries = records;
                state.selected_name = state.entries.first().map(|entry| entry.name.clone());
                state.pane_set_ready(format!("Loaded {} credential slots.", state.entries.len()));
            }
            Err(error) => {
                state.entries = crate::credentials::CREDENTIAL_TEMPLATES
                    .iter()
                    .map(|template| crate::credentials::CredentialRecord {
                        name: template.name.to_string(),
                        enabled: true,
                        secret: template.secret,
                        template: true,
                        scopes: template.scopes,
                        has_value: false,
                    })
                    .collect();
                state.selected_name = state.entries.first().map(|entry| entry.name.clone());
                let _ = state.pane_set_error(format!("Credential metadata load error: {error}"));
                *state.pane_last_action_mut() = Some("Using template credential slots".to_string());
            }
        }

        state
    }

    pub fn selected_entry(&self) -> Option<&crate::credentials::CredentialRecord> {
        let selected_name = self.selected_name.as_deref()?;
        self.entries
            .iter()
            .find(|entry| entry.name == selected_name)
    }

    pub fn select_row(&mut self, row_index: usize) -> Result<(), String> {
        let Some(entry) = self.entries.get(row_index) else {
            return Err(self.pane_set_error(format!("Credential row {row_index} is out of range")));
        };
        self.selected_name = Some(entry.name.clone());
        self.pane_set_ready(format!("Selected credential {}", entry.name));
        Ok(())
    }

    pub fn sync_inputs(&self, inputs: &mut CredentialsPaneInputs) {
        if let Some(entry) = self.selected_entry() {
            inputs.variable_name.set_value(entry.name.clone());
        }
    }

    pub fn add_custom_entry(&mut self, raw_name: &str) -> Result<(), String> {
        let normalized = crate::credentials::normalize_env_var_name(raw_name);
        if !crate::credentials::is_valid_env_var_name(normalized.as_str()) {
            return Err(
                self.pane_set_error("Credential name must match [A-Z_][A-Z0-9_]*".to_string())
            );
        }

        if self.entries.iter().any(|entry| entry.name == normalized) {
            self.selected_name = Some(normalized.clone());
            self.pane_set_ready(format!("Credential {normalized} already exists."));
            return Ok(());
        }

        self.entries.push(crate::credentials::CredentialRecord {
            name: normalized.clone(),
            enabled: true,
            secret: crate::credentials::infer_secret_from_name(normalized.as_str()),
            template: false,
            scopes: crate::credentials::CREDENTIAL_SCOPE_ALL,
            has_value: false,
        });
        self.selected_name = Some(normalized.clone());
        self.persist_metadata()?;
        self.pane_set_ready(format!("Added custom credential slot {normalized}."));
        Ok(())
    }

    pub fn set_selected_value(&mut self, value: &str) -> Result<(), String> {
        let Some(selected_name) = self.selected_name.clone() else {
            return Err(
                self.pane_set_error("Select a credential slot before saving a value.".to_string())
            );
        };
        self.repository
            .set_value(selected_name.as_str(), value)
            .map_err(|error| self.pane_set_error(error))?;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.name == selected_name)
        {
            entry.has_value = true;
        }
        self.persist_metadata()?;
        self.pane_set_ready(format!(
            "Stored value for {} in secure storage.",
            selected_name
        ));
        Ok(())
    }

    pub fn delete_or_clear_selected(&mut self) -> Result<(), String> {
        let Some(selected_name) = self.selected_name.clone() else {
            return Err(self
                .pane_set_error("Select a credential slot before deleting/clearing.".to_string()));
        };
        let Some(selected_index) = self
            .entries
            .iter()
            .position(|entry| entry.name == selected_name)
        else {
            return Err(
                self.pane_set_error(format!("Credential {} is not available", selected_name))
            );
        };

        self.repository
            .delete_value(selected_name.as_str())
            .map_err(|error| self.pane_set_error(error))?;

        let template = self
            .entries
            .get(selected_index)
            .is_some_and(|entry| entry.template);
        if template {
            if let Some(entry) = self.entries.get_mut(selected_index) {
                entry.has_value = false;
            }
            self.persist_metadata()?;
            self.pane_set_ready(format!("Cleared value for {}.", selected_name));
            return Ok(());
        }

        self.entries.remove(selected_index);
        self.persist_metadata()?;
        self.selected_name = if self.entries.is_empty() {
            None
        } else if selected_index < self.entries.len() {
            Some(self.entries[selected_index].name.clone())
        } else {
            Some(
                self.entries[self.entries.len().saturating_sub(1)]
                    .name
                    .clone(),
            )
        };
        self.pane_set_ready(format!("Removed custom credential {}.", selected_name));
        Ok(())
    }

    pub fn toggle_selected_enabled(&mut self) -> Result<(), String> {
        let Some(selected_name) = self.selected_name.clone() else {
            return Err(self
                .pane_set_error("Select a credential slot before toggling enabled.".to_string()));
        };
        let enabled = if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.name == selected_name)
        {
            entry.enabled = !entry.enabled;
            entry.enabled
        } else {
            return Err(
                self.pane_set_error(format!("Credential {} is not available", selected_name))
            );
        };
        self.persist_metadata()?;
        self.pane_set_ready(format!(
            "{} {}",
            if enabled { "Enabled" } else { "Disabled" },
            selected_name
        ));
        Ok(())
    }

    pub fn toggle_selected_scope(&mut self, scope_bit: u8) -> Result<(), String> {
        if scope_bit == 0 || (scope_bit & crate::credentials::CREDENTIAL_SCOPE_ALL) == 0 {
            return Err(self.pane_set_error("Invalid credential scope".to_string()));
        }
        let Some(selected_name) = self.selected_name.clone() else {
            return Err(
                self.pane_set_error("Select a credential slot before toggling scope.".to_string())
            );
        };
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.name == selected_name)
        else {
            return Err(
                self.pane_set_error(format!("Credential {} is not available", selected_name))
            );
        };
        if (entry.scopes & scope_bit) == 0 {
            entry.scopes |= scope_bit;
        } else {
            entry.scopes &= !scope_bit;
        }
        self.persist_metadata()?;
        self.pane_set_ready(format!("Updated scopes for {}.", selected_name));
        Ok(())
    }

    pub fn import_from_process_env(&mut self) -> Result<usize, String> {
        let mut imported = 0usize;
        for index in 0..self.entries.len() {
            let name = self.entries[index].name.clone();
            let from_env = std::env::var(name.as_str())
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let Some(value) = from_env else {
                continue;
            };
            if let Err(error) = self.repository.set_value(name.as_str(), value.as_str()) {
                return Err(self.pane_set_error(error));
            }
            self.entries[index].has_value = true;
            imported = imported.saturating_add(1);
        }

        if imported == 0 {
            self.pane_set_ready("No matching process env variables to import.");
        } else {
            self.pane_set_ready(format!(
                "Imported {imported} credentials from process environment."
            ));
        }
        Ok(imported)
    }

    pub fn resolve_env_for_scope(&self, scope: u8) -> Result<Vec<(String, String)>, String> {
        self.repository
            .resolve_env_for_scope(self.entries.as_slice(), scope)
    }

    pub fn read_secure_value(&self, name: &str) -> Result<Option<String>, String> {
        self.repository.read_value_secure(name)
    }

    pub fn set_value_for_name(&mut self, name: &str, value: &str) -> Result<(), String> {
        let normalized = crate::credentials::normalize_env_var_name(name);
        let Some(entry_index) = self
            .entries
            .iter()
            .position(|entry| entry.name == normalized)
        else {
            return Err(self.pane_set_error(format!("Credential {} is not available", normalized)));
        };

        self.repository
            .set_value(normalized.as_str(), value)
            .map_err(|error| self.pane_set_error(error))?;
        if let Some(entry) = self.entries.get_mut(entry_index) {
            entry.has_value = true;
        }
        self.persist_metadata()?;
        self.pane_set_ready(format!(
            "Stored value for {} in secure storage.",
            normalized
        ));
        Ok(())
    }

    fn persist_metadata(&mut self) -> Result<(), String> {
        self.repository
            .persist_records(self.entries.as_slice())
            .map_err(|error| self.pane_set_error(error))?;
        Ok(())
    }
}

impl CredentialsPaneInputs {
    pub fn from_state(credentials: &CredentialsState) -> Self {
        let mut inputs = Self::default();
        if let Some(entry) = credentials.selected_entry() {
            inputs.variable_name.set_value(entry.name.clone());
        }
        inputs
    }
}
