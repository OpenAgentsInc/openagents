//! Local world-screen selection. This state never enters world presence.
use terminal_core::opening::{Kind, Opening, ResourceRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Watch,
    Drive,
}

/// A renderer's declared resource support. Other resources use the ordinary overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Studio,
    Overlay,
}

#[derive(Clone, Debug)]
pub struct Screen {
    pub resource: ResourceRef,
    pub opening: Opening,
    pub mode: Mode,
    disclosure: String,
    last_frame: Option<u64>,
}

impl Screen {
    pub fn new(
        resource: ResourceRef,
        opening: Opening,
        mode: Mode,
        disclosure: String,
    ) -> Result<Self, String> {
        resource.check().map_err(|error| error.to_string())?;
        opening.check()?;
        let selected = [
            &opening.goal,
            &opening.seat,
            &opening.task,
            &opening.thread,
            &opening.review,
        ]
        .into_iter()
        .flatten()
        .any(|reference| reference == &resource);
        if resource.host != opening.host || !selected || disclosure.is_empty() {
            return Err("screen resource is not admitted by this workbench context".into());
        }
        Ok(Self {
            resource,
            opening,
            mode,
            disclosure,
            last_frame: None,
        })
    }

    /// Rechecks the source and disclosure before any private drawing or input.
    pub fn admitted(&self, stream: &str, disclosure: &str, observe: bool) -> bool {
        observe && stream == self.opening.stream && disclosure == self.disclosure
    }

    pub fn surface(&self) -> Surface {
        if self.resource.kind == Kind::Studio {
            Surface::Studio
        } else {
            Surface::Overlay
        }
    }

    /// Visible screens draw at most 30 times a second; inactive screens draw nothing.
    pub fn frame(&mut self, now_ms: u64, visible: bool, active: bool) -> bool {
        if !visible
            || !active
            || self
                .last_frame
                .is_some_and(|last| now_ms.saturating_sub(last) < 34)
        {
            return false;
        }
        self.last_frame = Some(now_ms);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::opening::{Host, OPENING};
    fn fixture() -> (ResourceRef, Opening) {
        let host = Host::Local {
            instance: "a".repeat(64),
        };
        let resource = ResourceRef::new(Kind::Studio, host.clone(), "fixture")
            .studio(terminal_core::opening::StudioPart::Goal);
        let opening = Opening {
            v: OPENING.into(),
            host,
            stream: "ab".into(),
            workspace: None,
            goal: Some(resource.clone()),
            seat: None,
            task: None,
            thread: None,
            review: None,
        };
        (resource, opening)
    }
    #[test]
    fn exact_context_disclosure_revocation_and_fallback() {
        let (resource, opening) = fixture();
        let screen = Screen::new(
            resource.clone(),
            opening.clone(),
            Mode::Watch,
            "grant1".into(),
        )
        .unwrap();
        let unrelated = ResourceRef::new(Kind::Studio, opening.host.clone(), "unrelated")
            .studio(terminal_core::opening::StudioPart::Goal);
        assert!(Screen::new(unrelated, opening.clone(), Mode::Watch, "grant1".into()).is_err());
        assert_eq!(screen.opening, opening);
        assert_eq!(screen.resource, resource);
        assert!(screen.admitted("ab", "grant1", true));
        assert!(!screen.admitted("ab", "grant2", true));
        assert!(!screen.admitted("restart", "grant1", true));
        assert!(!screen.admitted("ab", "grant1", false));
        let other = ResourceRef::new(Kind::Thread, screen.opening.host.clone(), "existing");
        assert_eq!(
            Screen::new(
                other.clone(),
                Opening {
                    thread: Some(other),
                    ..screen.opening
                },
                Mode::Watch,
                "grant1".into()
            )
            .unwrap()
            .surface(),
            Surface::Overlay
        );
    }
    #[test]
    fn inactive_and_visible_frame_budget() {
        let (resource, opening) = fixture();
        let mut screen = Screen::new(resource, opening, Mode::Watch, "grant1".into()).unwrap();
        assert!(!screen.frame(0, true, false));
        assert!(!screen.frame(0, false, true));
        assert!(screen.frame(0, true, true));
        assert!(!screen.frame(33, true, true));
        assert!(screen.frame(34, true, true));
    }
}
