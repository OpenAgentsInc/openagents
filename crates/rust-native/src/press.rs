//! Activations that outlive the view revision they began on.
//!
//! A view that streams (a Coder run, a reply being written) is replaced
//! every few hundred milliseconds, so a click can begin on one revision and
//! end on the next, or a tap can name a revision the application has already
//! replaced. [`ValidatedView::activate`] refuses both as stale. For the few
//! controls that must work while a view streams (stop, approve, deny, a
//! queued message's controls, choosing a folder), the application may admit
//! such a late activation here, and only under a narrow rule:
//!
//! - [`Press`], for a host that sees a press begin and end (a desktop
//!   pointer): the release runs only on the press's own revision, or on the
//!   revision right after it when the control there still offers the same
//!   action for the same target and the application admits that action late.
//! - [`ValidatedView::activate_late`], for a host that only sees the
//!   release (a phone tap naming a revision): the activation resolves against
//!   the current view only when the application admits the intent it resolves
//!   to there. Use it only for intents that name no stale target, such as a
//!   stop that stops whatever the current view runs.
//!
//! Every other stale activation stays refused.

use crate::{Activation, ValidatedView, ViewError};

/// A control pressed while one revision of a view was shown.
///
/// `action` is what the control did when pressed, naming its target (a
/// task, a card, a queued message), so a later revision can be checked to
/// offer the same thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Press<A> {
    /// The revision the press began on.
    pub revision: u64,
    /// The control's key.
    pub node: String,
    /// What the control did, and to what, when pressed.
    pub action: A,
}

impl<A: PartialEq> Press<A> {
    pub fn new(revision: u64, node: impl Into<String>, action: A) -> Self {
        Self {
            revision,
            node: node.into(),
            action,
        }
    }

    /// The action a release runs, if any. The release is on `node` while
    /// `revision` is shown, where the control does `offered`. It runs when
    /// `offered` is the pressed action for the same target, on the press's
    /// revision, or on the revision right after when `late` admits it.
    pub fn release<'a>(
        &self,
        revision: u64,
        node: &str,
        offered: Option<&'a A>,
        late: impl FnOnce(&A) -> bool,
    ) -> Option<&'a A> {
        let offered = offered.filter(|offered| node == self.node && **offered == self.action)?;
        let next = self.revision.checked_add(1);
        (revision == self.revision || (Some(revision) == next && late(offered))).then_some(offered)
    }
}

impl<I> ValidatedView<I> {
    /// Resolve an activation that names an earlier revision of this view's
    /// instance against this view, when `late` admits the intent the node
    /// resolves to here. An activation of this revision resolves as
    /// [`ValidatedView::activate`] does; a later or foreign one is stale.
    pub fn activate_late(
        &self,
        event: &Activation,
        late: impl FnOnce(&I) -> bool,
    ) -> Result<&I, ViewError> {
        let current = self.view();
        if event.instance != current.instance || event.revision > current.revision {
            return Err(ViewError::StaleActivation);
        }
        let fresh = Activation {
            revision: current.revision,
            ..event.clone()
        };
        let intent = self.activate(&fresh)?;
        if event.revision == current.revision || late(intent) {
            Ok(intent)
        } else {
            Err(ViewError::StaleActivation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;
    use crate::{Element, Node, View};

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
    enum Intent {
        Stop(&'static str),
        Open,
    }

    fn view(revision: u64, stop: &'static str) -> ValidatedView<Intent> {
        let button = |key: &str, intent| Node {
            key: key.into(),
            style: Style::default(),
            element: Element::Button {
                label: "Go".into(),
                enabled: true,
                icon: None,
                intent,
            },
        };
        View::new(
            "screen",
            revision,
            Node {
                key: "root".into(),
                style: Style::default(),
                element: Element::Stack {
                    axis: crate::Axis::Vertical,
                    children: vec![
                        button("stop", Intent::Stop(stop)),
                        button("open", Intent::Open),
                    ],
                },
            },
        )
        .validate()
        .unwrap()
    }

    #[test]
    fn a_press_runs_on_its_revision_or_the_next_for_the_same_late_action_only() {
        let late = |intent: &Intent| matches!(intent, Intent::Stop(_));
        let stop = Press::new(4, "stop", Intent::Stop("task-a"));
        let same = Intent::Stop("task-a");
        let other = Intent::Stop("task-b");
        assert_eq!(stop.release(4, "stop", Some(&same), |_| false), Some(&same));
        assert_eq!(stop.release(5, "stop", Some(&same), late), Some(&same));
        // Not a later revision, another target, another node, or no admission.
        assert_eq!(stop.release(6, "stop", Some(&same), late), None);
        assert_eq!(stop.release(3, "stop", Some(&same), late), None);
        assert_eq!(stop.release(5, "stop", Some(&other), late), None);
        assert_eq!(stop.release(5, "open", Some(&same), late), None);
        assert_eq!(stop.release(5, "stop", None, late), None);
        let open = Press::new(4, "open", Intent::Open);
        assert_eq!(open.release(5, "open", Some(&Intent::Open), late), None);
        assert_eq!(
            open.release(4, "open", Some(&Intent::Open), late),
            Some(&Intent::Open)
        );
        assert_eq!(
            Press::new(u64::MAX, "stop", same.clone()).release(0, "stop", Some(&same), late),
            None
        );
    }

    #[test]
    fn a_late_activation_resolves_only_admitted_intents_of_the_current_view() {
        let current = view(7, "task-a");
        let event = |revision, node: &str| Activation {
            instance: "screen".into(),
            revision,
            node: node.into(),
        };
        let late = |intent: &Intent| matches!(intent, Intent::Stop(_));
        assert_eq!(
            current.activate_late(&event(3, "stop"), late),
            Ok(&Intent::Stop("task-a"))
        );
        assert_eq!(
            current.activate_late(&event(3, "open"), late),
            Err(ViewError::StaleActivation)
        );
        assert_eq!(
            current.activate_late(&event(7, "open"), late),
            Ok(&Intent::Open)
        );
        assert_eq!(
            current.activate_late(&event(8, "stop"), late),
            Err(ViewError::StaleActivation)
        );
        let foreign = Activation {
            instance: "other".into(),
            ..event(3, "stop")
        };
        assert_eq!(
            current.activate_late(&foreign, late),
            Err(ViewError::StaleActivation)
        );
    }
}
