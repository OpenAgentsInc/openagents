//! Host-owned window layout and unsent fields; opening a tool never opens a new world.
use crate::{Checkpoint, Error};
use serde::{Deserialize, Serialize};
pub const PANELS: [&str; 5] = ["objects", "materials", "request", "compile", "history"];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StudioVisit {
    pub owner: Option<String>,
    pub checkpoint: Option<Checkpoint>,
    pub selection: Option<String>,
    pub open: bool,
    #[serde(default)]
    pub mesh: bool,
    pub panels: Vec<String>,
    pub active: String,
    pub note: String,
    pub source: String,
    pub request: String,
}
impl Default for StudioVisit {
    fn default() -> Self {
        Self {
            owner: None,
            checkpoint: None,
            selection: None,
            open: false,
            mesh: false,
            panels: PANELS.iter().map(|s| s.to_string()).collect(),
            active: "objects".into(),
            note: String::new(),
            source: String::new(),
            request: String::new(),
        }
    }
}
impl StudioVisit {
    /// A device copy can suggest a place only for its matching account.
    /// The confirmed restore still checks whether that place is safe in the world.
    pub fn owned_checkpoint(&self, account: Option<&str>) -> Option<Checkpoint> {
        (self.owner.as_deref() == account && self.validate().is_ok())
            .then(|| self.checkpoint.clone())
            .flatten()
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.panels.len() > 5
            || !PANELS.contains(&self.active.as_str())
            || self
                .panels
                .iter()
                .enumerate()
                .any(|(i, p)| !PANELS.contains(&p.as_str()) || self.panels[..i].contains(p))
            || self.note.len() > 48_000
            || self.source.len() > 4096
            || self.request.len() > 4800
            || self.owner.as_ref().is_some_and(|s| !crate::protocol::id(s))
            || self
                .selection
                .as_ref()
                .is_some_and(|s| !crate::protocol::id(s))
        {
            return Err(Error::Invalid("studio visit"));
        }
        if let Some(pose) = &self.checkpoint {
            pose.validate()?;
        }
        Ok(())
    }
    pub fn show(&mut self, panel: &str) {
        if !PANELS.contains(&panel) {
            return;
        }
        if !self.panels.iter().any(|p| p == panel) {
            self.panels.push(panel.into());
        }
        self.active = panel.into();
        self.open = true;
    }
    pub fn hide(&mut self, panel: &str) {
        self.panels.retain(|p| p != panel);
        if self.active == panel {
            self.active = self
                .panels
                .first()
                .cloned()
                .unwrap_or_else(|| "objects".into());
        }
        if self.panels.is_empty() {
            self.open = false;
        }
    }
    pub fn discard_for(&mut self, owner: Option<String>) {
        *self = Self {
            owner,
            ..Self::default()
        };
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_places_require_a_valid_visit_and_matching_owner() {
        let mut visit = StudioVisit {
            owner: Some("artist".into()),
            checkpoint: Some(Checkpoint {
                eye: [-2.3, 1.8, -12.3],
                target: [-2.4, 1.8, -14.8],
                mode: crate::CameraMode::Fly,
            }),
            source: "unfinished draft".into(),
            ..StudioVisit::default()
        };
        assert_eq!(visit.owned_checkpoint(Some("artist")), visit.checkpoint);
        assert!(visit.owned_checkpoint(Some("other")).is_none());
        assert!(visit.owned_checkpoint(None).is_none());
        visit.checkpoint.as_mut().unwrap().eye[0] = f64::INFINITY;
        assert!(visit.owned_checkpoint(Some("artist")).is_none());
        visit.checkpoint.as_mut().unwrap().eye[0] = -2.3;
        visit.panels.push("unavailable".into());
        assert!(visit.owned_checkpoint(Some("artist")).is_none());
    }
    #[test]
    fn windows_close_without_losing_fields_or_pose_and_reopen_in_reading_order() {
        let mut visit = StudioVisit::default();
        visit.note = "Move the candle".into();
        visit.source = "unsent source".into();
        visit.checkpoint = Some(Checkpoint {
            eye: [1., 2., 3.],
            target: [1., 2., 0.],
            mode: crate::CameraMode::Walk,
        });
        let pose = visit.checkpoint.clone();
        visit.show("compile");
        visit.hide("compile");
        visit.open = false;
        visit.show("compile");
        assert_eq!(visit.note, "Move the candle");
        assert_eq!(visit.source, "unsent source");
        assert_eq!(visit.checkpoint, pose);
        assert_eq!(visit.panels.iter().filter(|s| *s == "compile").count(), 1);
        let encoded = serde_json::to_string(&visit).unwrap();
        assert_eq!(
            serde_json::from_str::<StudioVisit>(&encoded).unwrap(),
            visit
        );
        visit.validate().unwrap();
    }
    #[test]
    fn another_account_does_not_inherit_private_tools_or_text() {
        let mut visit = StudioVisit::default();
        visit.owner = Some("account-a".into());
        visit.source = "private draft".into();
        visit.request = "private request".into();
        visit.discard_for(Some("account-b".into()));
        assert!(visit.source.is_empty() && visit.request.is_empty());
        assert!(!visit.open);
        visit.panels.push("engine-files".into());
        assert!(visit.validate().is_err());
    }
}
