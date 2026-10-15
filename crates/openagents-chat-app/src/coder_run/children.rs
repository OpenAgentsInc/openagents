//! Read-only child activity and bounded transcript pages.

use route_contract::{Digest, children as wire, digest_of};
use rust_native::{Axis, Element, Node, TextRole};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Control {
    Select(Digest),
    Close,
    Refresh,
    Previous,
    Next,
    EarlierChildren,
    MoreChildren,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub view: Digest,
    pub control: Control,
}
#[derive(Default)]
pub struct Inspector {
    pub activity: openagents_chat::children::Projection,
    generation: u64,
    selected: Option<wire::Reference>,
    pages: Vec<wire::Page>,
    page: usize,
    fragment: usize,
    list: usize,
    expected: Option<wire::Read>,
    error: Option<String>,
}
impl Inspector {
    pub fn new(task: &str) -> Self {
        Self {
            activity: openagents_chat::children::Projection::new(super::LOCAL, task),
            ..Self::default()
        }
    }
    fn identity(&self) -> Digest {
        digest_of(&(
            self.generation,
            &self.selected,
            self.page,
            self.fragment,
            self.list,
            &self.expected,
            &self.error,
            self.activity
                .children
                .iter()
                .map(|child| &child.reference)
                .collect::<Vec<_>>(),
        ))
    }
    pub fn target(&self, control: Control) -> Target {
        Target {
            view: self.identity(),
            control,
        }
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn open(&self) -> bool {
        self.selected.is_some()
    }
    pub fn fail(&mut self, generation: u64, read: &wire::Read, error: String) {
        if generation == self.generation && self.expected.as_ref() == Some(read) {
            self.expected = None;
            self.error = Some(error);
        }
    }
    fn read(&mut self, cursor: Option<wire::Cursor>) -> Option<wire::Read> {
        let read = wire::Read {
            reference: self.selected.clone()?,
            cursor,
        };
        self.expected = Some(read.clone());
        self.error = None;
        Some(read)
    }
    pub fn received(&mut self, generation: u64, read: &wire::Read, page: wire::Page) -> bool {
        if generation != self.generation
            || self.expected.as_ref() != Some(read)
            || self.selected.as_ref() != Some(&read.reference)
        {
            return false;
        }
        self.expected = None;
        if !page.valid_for(read) {
            self.error = Some("This child task changed. Refresh its messages.".into());
            return false;
        }
        if read.cursor.is_none() {
            self.pages.clear();
        }
        self.page = self.pages.len();
        self.fragment = 0;
        self.pages.push(page);
        true
    }
    fn fragments(&self) -> Vec<String> {
        self.pages
            .get(self.page)
            .map(|page| {
                page.text
                    .lines()
                    .flat_map(|line| {
                        let chars: Vec<_> = line.chars().collect();
                        if chars.is_empty() {
                            vec![String::new()]
                        } else {
                            chars
                                .chunks(140)
                                .map(|part| part.iter().collect())
                                .collect()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn apply(&mut self, target: &Target) -> Option<wire::Read> {
        if target.view != self.identity() {
            return None;
        }
        match &target.control {
            Control::Select(id) => {
                self.generation += 1;
                self.selected = Some(
                    self.activity
                        .children
                        .iter()
                        .find(|child| child.reference.id == *id)?
                        .reference
                        .clone(),
                );
                self.pages.clear();
                self.page = 0;
                self.fragment = 0;
                self.expected = None;
                return self.read(None);
            }
            Control::Close => {
                self.generation += 1;
                self.selected = None;
                self.pages.clear();
                self.expected = None;
                self.error = None;
            }
            Control::Refresh => {
                if self.expected.is_none() {
                    self.generation += 1;
                    return self.read(None);
                }
            }
            Control::Previous => {
                if self.fragment > 0 {
                    self.fragment -= 1;
                } else if self.page > 0 {
                    self.page -= 1;
                    self.fragment = self.fragments().len().saturating_sub(1) / 12;
                }
            }
            Control::Next => {
                if self.expected.is_some() {
                    return None;
                }
                if (self.fragment + 1) * 12 < self.fragments().len() {
                    self.fragment += 1;
                } else if self.page + 1 < self.pages.len() {
                    self.page += 1;
                    self.fragment = 0;
                } else {
                    return self.read(Some(self.pages.get(self.page)?.next.clone()?));
                }
            }
            Control::EarlierChildren => self.list = self.list.saturating_sub(1),
            Control::MoreChildren => {
                if (self.list + 1) * 5 < self.activity.children.len() {
                    self.list += 1;
                }
            }
        }
        None
    }
    pub fn view<I>(&self, intent: impl Fn(Target) -> I) -> Option<Node<I>> {
        if self.activity.children.is_empty() {
            return None;
        }
        let text = |key: &str, value: String| Node {
            key: format!("children-{key}"),
            style: Default::default(),
            element: Element::Text {
                value,
                role: TextRole::Body,
            },
        };
        let button = |key: &str, label: String, control, enabled| Node {
            key: format!("children-{key}"),
            style: Default::default(),
            element: Element::Button {
                label,
                enabled,
                intent: intent(self.target(control)),
                shortcut: None,
                icon: None,
            },
        };
        let mut nodes = vec![text("title", "Child tasks".into())];
        for child in self
            .activity
            .children
            .iter()
            .rev()
            .skip(self.list * 5)
            .take(5)
        {
            let status = match child.status {
                wire::Status::Running => "Running",
                wire::Status::Completed => "Finished",
                wire::Status::Failed => "Failed",
                wire::Status::Interrupted => "Stopped",
                wire::Status::Unknown => "Status unavailable",
            };
            nodes.push(button(
                child.reference.id.as_str(),
                format!("{} · {status}", child.title),
                Control::Select(child.reference.id.clone()),
                true,
            ));
        }
        if self.activity.children.len() > 5 {
            nodes.push(button(
                "earlier",
                "Previous tasks".into(),
                Control::EarlierChildren,
                self.list > 0,
            ));
            nodes.push(button(
                "more",
                "More tasks".into(),
                Control::MoreChildren,
                (self.list + 1) * 5 < self.activity.children.len(),
            ));
        }
        if self.activity.truncated {
            nodes.push(text(
                "limited",
                "Some older child tasks are not shown.".into(),
            ));
        }
        if let Some(reference) = &self.selected {
            if let Some(child) = self
                .activity
                .children
                .iter()
                .find(|child| child.reference == *reference)
            {
                nodes.push(text(
                    "selected",
                    format!(
                        "{} · {}{}",
                        child.title,
                        child.harness,
                        child
                            .model
                            .as_ref()
                            .map(|model| format!(" · {model}"))
                            .unwrap_or_default()
                    ),
                ));
                if child.incomplete {
                    nodes.push(text("incomplete", "Some messages are unavailable.".into()));
                }
            }
            nodes.push(button(
                "refresh",
                "Refresh messages".into(),
                Control::Refresh,
                self.expected.is_none(),
            ));
            nodes.push(button(
                "close",
                "Close child task".into(),
                Control::Close,
                true,
            ));
            if let Some(error) = &self.error {
                nodes.push(text("error", error.clone()));
            }
            if self.pages.is_empty() {
                nodes.push(text(
                    "loading",
                    if self.expected.is_some() {
                        "Loading messages…"
                    } else {
                        "No messages available."
                    }
                    .into(),
                ));
            } else {
                let fragments = self.fragments();
                nodes.push(text(
                    "messages",
                    if fragments.is_empty() {
                        "No messages available.".into()
                    } else {
                        fragments
                            .iter()
                            .skip(self.fragment * 12)
                            .take(12)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n")
                    },
                ));
                nodes.push(button(
                    "previous",
                    "Previous messages".into(),
                    Control::Previous,
                    self.page > 0 || self.fragment > 0,
                ));
                nodes.push(button(
                    "next",
                    "More messages".into(),
                    Control::Next,
                    self.expected.is_none()
                        && ((self.fragment + 1) * 12 < fragments.len()
                            || self.page + 1 < self.pages.len()
                            || self
                                .pages
                                .get(self.page)
                                .is_some_and(|page| page.next.is_some())),
                ));
                if self.pages.get(self.page).is_some_and(|page| page.truncated) {
                    nodes.push(text("truncated", "Some messages are unavailable.".into()));
                }
            }
        }
        Some(Node {
            key: "child-tasks".into(),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: nodes,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inspector() -> Inspector {
        let mut inspector = Inspector::new("task");
        inspector.activity.children.push(wire::Child {
            reference: wire::Reference::new(
                route_contract::observation::Scope {
                    host: "local".into(),
                    task: "task".into(),
                    turn: 1,
                    generation: 1,
                },
                wire::Source::Native,
                "child".into(),
            ),
            parent: None,
            title: "Inspect".into(),
            harness: "claude-code".into(),
            model: None,
            status: wire::Status::Running,
            summary: String::new(),
            transcript: true,
            incomplete: false,
        });
        inspector
    }
    #[test]
    fn closed_reopened_child_rejects_old_pages_and_controls() {
        let mut inspector = inspector();
        let select = inspector.target(Control::Select(
            inspector.activity.children[0].reference.id.clone(),
        ));
        let read = inspector.apply(&select).unwrap();
        let old = inspector.generation();
        inspector.apply(&inspector.target(Control::Close));
        let select = inspector.target(select.control);
        let fresh = inspector.apply(&select).unwrap();
        assert_eq!(read, fresh);
        assert_ne!(old, inspector.generation());
        let page = wire::Page {
            reference: read.reference.id.clone(),
            offset: 0,
            text: "Old".into(),
            next: None,
            truncated: false,
            running: true,
        };
        assert!(!inspector.received(old, &read, page.clone()));
        assert!(inspector.received(inspector.generation(), &fresh, page));
        assert!(inspector.apply(&select).is_none());
        assert_eq!(inspector.pages.len(), 1);
    }
    #[test]
    fn page_navigation_and_copy_guard_keep_child_views_read_only() {
        let mut inspector = inspector();
        let read = inspector
            .apply(&inspector.target(Control::Select(
                inspector.activity.children[0].reference.id.clone(),
            )))
            .unwrap();
        let page = wire::Page {
            reference: read.reference.id.clone(),
            offset: 0,
            text: "line\n".repeat(14),
            next: Some(wire::Cursor {
                offset: 70,
                prefix: Digest::of_bytes(b"prefix"),
            }),
            truncated: false,
            running: true,
        };
        assert!(inspector.received(inspector.generation(), &read, page));
        assert!(inspector.apply(&inspector.target(Control::Next)).is_none());
        assert_eq!(inspector.fragment, 1);
        let next = inspector.apply(&inspector.target(Control::Next)).unwrap();
        assert_eq!(next.cursor.unwrap().offset, 70);
        let node = inspector.view(|target| target).unwrap();
        fn copy(node: &Node<Target>) {
            match &node.element {
                Element::Text { value, .. } => {
                    assert!(oa_copy::violations(value, &[]).is_empty(), "{value}")
                }
                Element::Button { label, .. } => {
                    assert!(oa_copy::violations(label, &[]).is_empty(), "{label}")
                }
                Element::Stack { children, .. } => {
                    for child in children {
                        copy(child)
                    }
                }
                _ => panic!("unexpected child control"),
            }
        }
        copy(&node);
    }
}
