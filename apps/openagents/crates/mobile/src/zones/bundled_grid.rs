//! The bundled Aethyr Grid: shell policy and local suggestions, without account grants.
use super::{ZoneError, ZoneRunnerAction, invalid};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

#[derive(Clone, Copy, Serialize)]
enum Intent {
    Retry,
    Close,
}
struct State {
    ready: bool,
    failed: bool,
    revision: u64,
    view: Option<ValidatedView<Intent>>,
}

#[derive(uniffi::Object)]
pub struct CoreBundledGrid {
    state: Mutex<State>,
    storage: PathBuf,
    writes: Mutex<()>,
    instance: String,
}

#[uniffi::export]
impl CoreBundledGrid {
    #[uniffi::constructor]
    pub fn new(storage_directory: String) -> Self {
        Self {
            state: Mutex::new(State {
                ready: false,
                failed: false,
                revision: 0,
                view: None,
            }),
            storage: storage_directory.into(),
            writes: Mutex::new(()),
            instance: format!("grid-{}", uuid::Uuid::new_v4().simple()),
        }
    }
    pub fn ios_script(&self) -> String {
        include_str!("bundled_grid_ios.js").into()
    }
    pub fn ready(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.ready = true;
            state.failed = false;
        }
    }
    pub fn failed(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.ready = false;
            state.failed = true;
        }
    }
    pub fn retry(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.ready = false;
            state.failed = false;
            state.view = None;
        }
    }
    pub fn allows_navigation(&self, url: String) -> bool {
        reqwest::Url::parse(&url).is_ok_and(|u| {
            u.scheme() == "aethyr-grid"
                && u.host_str() == Some("bundle")
                && u.path() == "/grid.html"
                && u.username().is_empty()
                && u.password().is_none()
                && u.port().is_none()
        })
    }
    pub fn external_url(&self, url: String) -> Option<String> {
        reqwest::Url::parse(&url)
            .ok()
            .filter(|u| {
                u.scheme() == "https"
                    && u.host_str() == Some("openagents.com")
                    && u.username().is_empty()
                    && u.password().is_none()
                    && u.port_or_known_default() == Some(443)
            })
            .map(|u| u.to_string())
    }
    pub fn service_url(&self, path: String) -> Option<String> {
        let suffix = match path.as_str() {
            "/api/grid/service/pylons" => "/pylons/live",
            "/api/grid/service/foreman" => "/v1/foreman/status",
            "/api/grid/service/activity" => "/v1/foreman/activity",
            _ => return None,
        };
        Some(format!("https://openagents.com{suffix}"))
    }
    pub fn suggestion(&self, body: String) -> Result<String, ZoneError> {
        #[derive(Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        struct Suggestion {
            text: String,
            category: String,
            name: String,
        }
        if body.len() > 20_000 {
            return Err(invalid("That suggestion is too long."));
        }
        let mut record: Suggestion = serde_json::from_str(&body)
            .map_err(|_| invalid("The suggestion could not be read."))?;
        record.text = record.text.trim().into();
        if record.text.is_empty()
            || record.text.len() > 16_000
            || record.text.chars().count() > 4000
            || record.category.is_empty()
            || record.category.len() > 40
            || record.name.len() > 160
            || record
                .text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(invalid("Check your suggestion and try again."));
        }
        let _guard = self
            .writes
            .lock()
            .map_err(|_| invalid("The suggestion could not be saved."))?;
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(&self.storage)?;
            let path = self.storage.join("suggestions.json");
            let mut records: Vec<Suggestion> = if path.exists() {
                if std::fs::symlink_metadata(&path)?.file_type().is_symlink()
                    || path.metadata()?.len() > 4 * 1024 * 1024
                {
                    return Err("invalid suggestion store".into());
                }
                serde_json::from_slice(&std::fs::read(&path)?)?
            } else {
                vec![]
            };
            if records.len() >= 200 {
                records.drain(..records.len() - 199);
            }
            records.push(record);
            let temporary = self
                .storage
                .join(format!("suggestions-{}.tmp", uuid::Uuid::new_v4()));
            std::fs::write(&temporary, serde_json::to_vec(&records)?)?;
            std::fs::rename(temporary, path)?;
            Ok(())
        };
        save().map_err(|_| invalid("The suggestion could not be saved."))?;
        Ok("{\"ok\":true}".into())
    }
    pub fn card(&self) -> Result<Option<String>, ZoneError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| invalid("The Grid could not open."))?;
        if state.ready {
            state.view = None;
            return Ok(None);
        }
        state.revision += 1;
        let mut children = vec![
            Node {
                key: "heading".into(),
                style: Default::default(),
                element: Element::Text {
                    value: if state.failed {
                        "The Grid didn't open"
                    } else {
                        "Opening the Grid"
                    }
                    .into(),
                    role: TextRole::Heading,
                },
            },
            Node {
                key: "body".into(),
                style: Default::default(),
                element: Element::Text {
                    value: if state.failed {
                        "Try opening it again."
                    } else {
                        "Getting the world ready."
                    }
                    .into(),
                    role: TextRole::Body,
                },
            },
        ];
        for (key, label, intent) in if state.failed {
            vec![
                ("retry", "Try again", Intent::Retry),
                ("close", "Close", Intent::Close),
            ]
        } else {
            vec![("close", "Close", Intent::Close)]
        } {
            children.push(Node {
                key: key.into(),
                style: Default::default(),
                element: Element::Button {
                    label: label.into(),
                    enabled: true,
                    icon: None,
                    shortcut: None,
                    intent,
                },
            });
        }
        let view = View::new(
            &self.instance,
            state.revision,
            Node {
                key: "grid-card".into(),
                style: Default::default(),
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        )
        .validate()
        .map_err(|_| invalid("The Grid could not open."))?;
        let bytes = view
            .to_json()
            .map_err(|_| invalid("The Grid could not open."))?;
        state.view = Some(view);
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| invalid("The Grid could not open."))
    }
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> Result<ZoneRunnerAction, ZoneError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| invalid("The Grid could not open."))?;
        let intent = *state
            .view
            .as_ref()
            .ok_or_else(|| invalid("This card changed. Try again."))?
            .activate(&Activation {
                instance,
                revision,
                node,
            })
            .map_err(|_| invalid("This card changed. Try again."))?;
        match intent {
            Intent::Retry => {
                state.ready = false;
                state.failed = false;
                state.view = None;
                Ok(ZoneRunnerAction::Retry)
            }
            Intent::Close => Ok(ZoneRunnerAction::Close),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_grid_keeps_navigation_services_and_account_authority_separate() {
        let temp = tempfile::tempdir().unwrap();
        let grid = CoreBundledGrid::new(temp.path().to_string_lossy().into());
        assert!(grid.allows_navigation("aethyr-grid://bundle/grid.html?boot=test".into()));
        for url in [
            "https://openagents.com/grid",
            "aethyr-grid://other/grid.html",
            "aethyr-grid://bundle/index.html",
            "aethyr-grid://user@bundle/grid.html",
        ] {
            assert!(!grid.allows_navigation(url.into()));
        }
        assert!(
            grid.external_url("https://openagents.com/pylons".into())
                .is_some()
        );
        assert!(
            grid.external_url("https://openagents.com.evil.test/".into())
                .is_none()
        );
        assert!(grid.service_url("/api/session/token".into()).is_none());
        assert_eq!(
            grid.service_url("/api/grid/service/pylons".into())
                .as_deref(),
            Some("https://openagents.com/pylons/live")
        );
        assert!(grid.card().unwrap().is_some());
        grid.ready();
        assert!(grid.card().unwrap().is_none());
        grid.failed();
        assert!(grid.card().unwrap().unwrap().contains("Try again"));
    }
    #[test]
    fn suggestions_are_bounded_local_data_and_bad_input_preserves_them() {
        let temp = tempfile::tempdir().unwrap();
        let grid = CoreBundledGrid::new(temp.path().to_string_lossy().into());
        grid.suggestion(r#"{"text":"A local idea","category":"idea","name":"Visitor"}"#.into())
            .unwrap();
        let path = temp.path().join("suggestions.json");
        let original = std::fs::read(&path).unwrap();
        assert!(
            grid.suggestion(r#"{"text":"","category":"idea","name":"Visitor"}"#.into())
                .is_err()
        );
        assert!(
            grid.suggestion(
                r#"{"text":"x","category":"idea","name":"Visitor","token":"bad"}"#.into()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
