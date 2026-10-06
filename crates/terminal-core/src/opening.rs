//! Private navigation context for opening the shared workbench. References
//! select existing records and grant no execution, review, or shell rights.

use serde::{Deserialize, Serialize};
pub use workbench::{Host, Kind, ResourceRef, Revision, StudioPart};

/// The portable opening schema.
pub const OPENING: &str = "openagents.workbench-opening.v1";

/// A workshop opening, scoped to one observed host process. Workspace is
/// the admitted source's display label, never a repository path or grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub v: String,
    pub host: Host,
    pub stream: String,
    pub workspace: Option<String>,
    pub goal: Option<ResourceRef>,
    pub seat: Option<ResourceRef>,
    pub task: Option<ResourceRef>,
    pub thread: Option<ResourceRef>,
    pub review: Option<ResourceRef>,
}

impl Opening {
    /// Validates bounded identities and refuses references to other hosts.
    pub fn check(&self) -> Result<(), String> {
        if self.v != OPENING {
            return Err("unsupported workbench opening version".into());
        }
        if self.stream.is_empty()
            || self.stream.len() > 64
            || !self
                .stream
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("invalid studio stream".into());
        }
        // Validate the host through the shared reference contract.
        ResourceRef::new(Kind::Thread, self.host.clone(), "opening")
            .check()
            .map_err(|e| e.to_string())?;
        if self
            .workspace
            .as_ref()
            .is_some_and(|w| w.is_empty() || w.len() > 128 || w.chars().any(char::is_control))
        {
            return Err("invalid workspace label".into());
        }
        for (reference, part) in [
            (&self.goal, Some(StudioPart::Goal)),
            (&self.seat, Some(StudioPart::Seat)),
            (&self.task, Some(StudioPart::Task)),
            (&self.thread, None),
            (&self.review, Some(StudioPart::Review)),
        ] {
            if let Some(reference) = reference {
                reference.check().map_err(|e| e.to_string())?;
                if reference.host != self.host
                    || reference.part != part
                    || reference.kind
                        != if part.is_some() {
                            Kind::Studio
                        } else {
                            Kind::Thread
                        }
                {
                    return Err("opening reference belongs to another host or record kind".into());
                }
            }
        }
        if self.review.as_ref().is_some_and(|r| r.revision.is_none()) {
            return Err("a review opening requires its exact revision".into());
        }
        Ok(())
    }

    /// A display label only; it is never request context sent to an engine.
    pub fn label(&self) -> String {
        let mut words = vec!["STUDIO".to_owned()];
        if let Some(workspace) = &self.workspace {
            words.push(workspace.chars().take(20).collect());
        }
        for (name, reference) in [
            ("goal", &self.goal),
            ("seat", &self.seat),
            ("task", &self.task),
        ] {
            if let Some(reference) = reference {
                words.push(format!(
                    "{name} {}",
                    reference.id.chars().take(12).collect::<String>()
                ));
            }
        }
        words.join(" ")
    }
}

impl crate::Application {
    /// Opens existing navigation context under the caller's current read
    /// admission. This never creates a shell, goal, task, or engine run.
    pub fn open_workshop(&mut self, opening: Opening, may_observe: bool) -> Result<(), String> {
        if !may_observe {
            self.workshop = None;
            return Err("studio observation is not admitted".into());
        }
        opening.check()?;
        self.workshop = Some(opening);
        self.open = true;
        self.focused = true;
        self.prefix = false;
        Ok(())
    }

    /// Drops private context on revocation, leaving the ordinary terminal.
    pub fn clear_workshop(&mut self) {
        self.workshop = None;
    }

    #[must_use]
    pub fn workshop(&self) -> Option<&Opening> {
        self.workshop.as_ref()
    }
}
