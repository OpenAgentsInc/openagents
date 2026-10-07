//! Cloud plugin preferences contain configuration and credential names only.
use crate::Draft;
use coder_cloud::{Mode, Placement, Spec};
use crossterm::event::{KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub enabled: bool,
    pub mode: Mode,
    pub size: String,
    pub template: Option<String>,
    pub credential_names: Vec<String>,
    pub workspace_paths: Vec<String>,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Mode::Integrated,
            size: "default".into(),
            template: None,
            credential_names: vec![],
            workspace_paths: vec![],
        }
    }
}
impl Configuration {
    pub fn gce() -> Self {
        Self {
            mode: Mode::Coder,
            ..Self::default()
        }
    }
    pub fn valid(&self, placement: Placement) -> bool {
        if !matches!(self.size.as_str(), "small" | "default" | "large" | "xlarge")
            || (placement == Placement::Gce && (self.template.is_some() || self.size != "default"))
        {
            return false;
        }
        if self
            .template
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128 || s.contains('\0'))
        {
            return false;
        }
        if self.workspace_paths.len() > 128 || self.credential_names.len() > 64 {
            return false;
        }
        if self
            .workspace_paths
            .iter()
            .any(|p| coder_cloud::workspace::validate_path(p).is_err())
        {
            return false;
        }
        if self
            .credential_names
            .iter()
            .any(|n| crate::cloud::credential_name(n).is_err())
        {
            return false;
        }
        Spec {
            placement,
            mode: self.mode,
            agent: "codex".into(),
            task: "validate".into(),
            model: None,
            reasoning: None,
            cwd: ".".into(),
            timeout_seconds: 3600,
            size: self.size.clone(),
            template: self.template.clone(),
            credential_names: self.credential_names.clone(),
        }
        .validate()
        .is_ok()
    }
    pub fn update(&self, value: Value, placement: Placement) -> Result<Self, String> {
        let incoming = value.as_object().ok_or("Use a cloud settings object.")?;
        let mut stored = serde_json::to_value(self).unwrap();
        let object = stored.as_object_mut().unwrap();
        for (name, value) in incoming {
            if !object.contains_key(name) {
                return Err("Unknown cloud settings field. Keys are configured through named environment variables.".into());
            }
            object.insert(name.clone(), value.clone());
        }
        let config: Self =
            serde_json::from_value(stored).map_err(|_| "Invalid cloud settings fields.")?;
        if !config.valid(placement) {
            return Err("Invalid cloud execution mode, size, template, credential names, or workspace paths.".into());
        }
        Ok(config)
    }
}
pub fn placement(id: &str) -> Option<Placement> {
    match id {
        "boat-cloud" => Some(Placement::Boat),
        "gce-cloud" => Some(Placement::Gce),
        _ => None,
    }
}
pub struct Editor {
    pub placement: Placement,
    pub config: Configuration,
    pub focus: usize,
    pub template: Draft,
    pub credentials: Draft,
    pub paths: Draft,
    pub error: Option<String>,
}
impl Editor {
    pub fn new(placement: Placement, config: Configuration) -> Self {
        let draft = |text: String| {
            let cursor = text.len();
            Draft {
                text,
                cursor,
                ..Default::default()
            }
        };
        Self {
            placement,
            template: draft(config.template.clone().unwrap_or_default()),
            credentials: draft(config.credential_names.join(", ")),
            paths: draft(config.workspace_paths.join(", ")),
            config,
            focus: 0,
            error: None,
        }
    }
    fn draft(&mut self) -> Option<&mut Draft> {
        match self.focus {
            2 if self.placement == Placement::Boat => Some(&mut self.template),
            3 => Some(&mut self.credentials),
            4 => Some(&mut self.paths),
            _ => None,
        }
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(d) = self.draft() {
            d.insert(text);
        }
    }
    pub fn value(&self) -> Result<Configuration, String> {
        let split = |s: &str| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect::<Vec<String>>()
        };
        self.config.update(serde_json::json!({"template":if self.template.text.trim().is_empty(){None}else{Some(self.template.text.trim())},"credential_names":split(&self.credentials.text),"workspace_paths":split(&self.paths.text)}),self.placement)
    }
    /// Return true when the editor closes; saving is handled by the settings store.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => return true,
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % 7,
            KeyCode::BackTab | KeyCode::Up => self.focus = (self.focus + 6) % 7,
            KeyCode::Enter if self.focus == 6 => return true,
            KeyCode::Enter | KeyCode::Char(' ') if self.focus < 2 => {
                if self.placement == Placement::Boat {
                    if self.focus == 0 {
                        self.config.mode = if self.config.mode == Mode::Coder {
                            Mode::Integrated
                        } else {
                            Mode::Coder
                        };
                    } else {
                        let choices = ["small", "default", "large", "xlarge"];
                        let at = choices
                            .iter()
                            .position(|s| *s == self.config.size)
                            .unwrap_or(1);
                        self.config.size = choices[(at + 1) % 4].into();
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(d) = self.draft() {
                    d.insert(&c.to_string());
                }
            }
            KeyCode::Backspace => {
                if let Some(d) = self.draft() {
                    d.backspace();
                }
            }
            KeyCode::Delete => {
                if let Some(d) = self.draft() {
                    d.delete();
                }
            }
            KeyCode::Left => {
                if let Some(d) = self.draft() {
                    d.cursor = d.previous();
                }
            }
            KeyCode::Right => {
                if let Some(d) = self.draft() {
                    d.cursor = d.next();
                }
            }
            KeyCode::Home => {
                if let Some(d) = self.draft() {
                    d.cursor = 0;
                }
            }
            KeyCode::End => {
                if let Some(d) = self.draft() {
                    d.cursor = d.text.len();
                }
            }
            _ => {}
        }
        if self.placement == Placement::Gce && self.focus == 2 {
            self.focus = if matches!(key.code, KeyCode::BackTab | KeyCode::Up) {
                1
            } else {
                3
            };
        }
        false
    }
}
