//! Read-only workspace inspection over the task owner's bounded service.

use route_contract::{Digest, digest_of, workspace as wire};
use rust_native::{Axis, Element, Node, TextRole};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Control {
    Open,
    Close,
    Refresh,
    Parent,
    Search,
    Row(usize),
    Previous,
    Next,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub view: Digest,
    pub control: Control,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    Reference,
    Read(wire::Request),
}
#[derive(Default)]
pub struct Inspector {
    reference: Option<wire::Reference>,
    output: Option<(wire::Request, wire::Output)>,
    expected: Option<Digest>,
    pub query: String,
    pub visible: bool,
    page: usize,
    error: Option<String>,
}
impl Inspector {
    pub fn reference_turn(&self) -> Option<usize> {
        self.reference.as_ref().map(|reference| reference.turn)
    }
    fn identity(&self) -> Digest {
        digest_of(&(
            &self.reference,
            &self.output,
            &self.expected,
            &self.query,
            self.visible,
            self.page,
            &self.error,
        ))
    }
    pub fn target(&self, control: Control) -> Target {
        Target {
            view: self.identity(),
            control,
        }
    }
    pub fn reset(&mut self) {
        self.reference = None;
        self.output = None;
        self.expected = None;
        self.page = 0;
        self.error = None;
    }
    pub fn set_query(&mut self, query: &str) {
        self.query = query.chars().filter(|c| !c.is_control()).collect();
        while self.query.len() > 256 {
            self.query.pop();
        }
    }
    pub fn fail(&mut self, error: String) {
        self.error = Some(error);
    }
    pub fn referenced(&mut self, reference: wire::Reference, task: &str) -> Option<Need> {
        if !self.visible
            || !reference.valid()
            || reference.host != super::LOCAL
            || reference.task != task
        {
            return None;
        }
        self.reference = Some(reference);
        self.output = None;
        self.expected = None;
        self.page = 0;
        self.error = None;
        self.read(wire::Operation::List {
            path: String::new(),
        })
    }
    pub fn received(&mut self, request: &wire::Request, output: wire::Output) -> bool {
        if !self.visible
            || self.reference.as_ref() != Some(&request.reference)
            || !output.valid_for(request)
            || self.expected.as_ref() != Some(&request.operation.id())
            || matches!(&request.operation,wire::Operation::Search{query,..} if query!=&self.query)
        {
            return false;
        }
        self.output = Some((request.clone(), output));
        self.page = self.page.min(self.pages() - 1);
        self.error = None;
        true
    }
    fn read(&mut self, operation: wire::Operation) -> Option<Need> {
        let reference = self.reference.clone()?;
        self.expected = Some(operation.id());
        self.error = None;
        Some(Need::Read(wire::Request {
            reference,
            operation,
        }))
    }
    fn folder(&self) -> String {
        self.output
            .as_ref()
            .map(|(request, _)| match &request.operation {
                wire::Operation::Read { path } => path
                    .rsplit_once('/')
                    .map_or("", |(parent, _)| parent)
                    .into(),
                op => op.path().into(),
            })
            .unwrap_or_default()
    }
    fn fragments(text: &str) -> Vec<String> {
        text.lines()
            .enumerate()
            .flat_map(|(line, value)| {
                let chars: Vec<_> = value.chars().collect();
                if chars.is_empty() {
                    vec![format!("{}  ", line + 1)]
                } else {
                    chars
                        .chunks(160)
                        .map(|part| format!("{}  {}", line + 1, part.iter().collect::<String>()))
                        .collect()
                }
            })
            .collect()
    }
    fn pages(&self) -> usize {
        self.output
            .as_ref()
            .map_or(1, |(_, output)| {
                output
                    .text
                    .as_ref()
                    .map_or(output.rows.len().div_ceil(5), |text| {
                        Self::fragments(text).len().div_ceil(12)
                    })
            })
            .max(1)
    }
    pub fn apply(&mut self, target: &Target) -> Option<Need> {
        if target.view != self.identity() {
            return None;
        }
        match target.control {
            Control::Open => {
                self.visible = true;
                self.reset();
                return Some(Need::Reference);
            }
            Control::Close => {
                self.visible = false;
                self.reset();
            }
            Control::Refresh => {
                self.reset();
                return Some(Need::Reference);
            }
            Control::Previous => self.page = self.page.saturating_sub(1),
            Control::Next if self.page + 1 < self.pages() => self.page += 1,
            Control::Parent => {
                let folder = self.folder();
                let path = folder
                    .rsplit_once('/')
                    .map_or("", |(parent, _)| parent)
                    .to_owned();
                self.page = 0;
                return self.read(wire::Operation::List { path });
            }
            Control::Search => {
                self.page = 0;
                return self.read(wire::Operation::Search {
                    path: self.folder(),
                    query: self.query.clone(),
                });
            }
            Control::Row(index) => {
                let row = self.output.as_ref()?.1.rows.get(index)?.clone();
                self.page = row.line.unwrap_or(1).saturating_sub(1) / 12;
                return self.read(if row.kind == wire::Kind::Directory {
                    wire::Operation::List { path: row.path }
                } else {
                    wire::Operation::Read { path: row.path }
                });
            }
            _ => {}
        }
        None
    }
    pub fn view<I>(
        &self,
        prefix: &str,
        enabled: bool,
        search: Option<Node<I>>,
        intent: impl Fn(Target) -> I,
    ) -> Node<I> {
        let view = self.identity();
        let button = |key: &str, label: String, control: Control, supported: bool| Node {
            key: format!("{prefix}-{key}"),
            style: Default::default(),
            element: Element::Button {
                label,
                enabled: enabled && supported,
                intent: intent(Target {
                    view: view.clone(),
                    control,
                }),
                shortcut: None,
                icon: None,
            },
        };
        let text = |key: &str, value: String| Node {
            key: format!("{prefix}-{key}"),
            style: Default::default(),
            element: Element::Text {
                value,
                role: TextRole::Body,
            },
        };
        if !self.visible {
            return button("open", "Browse workspace".into(), Control::Open, true);
        }
        let mut children = vec![Node {
            key: format!("{prefix}-header"),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Wrap,
                children: vec![
                    button("refresh", "Refresh files".into(), Control::Refresh, true),
                    button(
                        "parent",
                        "Parent folder".into(),
                        Control::Parent,
                        self.reference.is_some(),
                    ),
                    button("close", "Close files".into(), Control::Close, true),
                ],
            },
        }];
        if let Some(reference) = &self.reference {
            children.push(text(
                "root",
                format!("Project: {}", reference.authority.checkout),
            ));
            if let Some(search) = search {
                children.push(search);
            }
            children.push(button(
                "search",
                "Search files".into(),
                Control::Search,
                !self.query.is_empty(),
            ));
        }
        if let Some(error) = &self.error {
            children.push(text("error", error.clone()));
        }
        if let Some((request, output)) = &self.output {
            if !request.operation.path().is_empty() {
                children.push(text("path", request.operation.path().into()));
            }
            if let Some(content) = &output.text {
                let value = Self::fragments(content)
                    .into_iter()
                    .skip(self.page * 12)
                    .take(12)
                    .collect::<Vec<_>>()
                    .join("\n");
                children.push(text(
                    "text",
                    if value.is_empty() {
                        "This file is empty.".into()
                    } else {
                        value
                    },
                ));
            } else {
                for (index, row) in output.rows.iter().enumerate().skip(self.page * 5).take(5) {
                    let label = match row.kind {
                        wire::Kind::Directory => format!("{}/", row.path),
                        wire::Kind::File => row.path.clone(),
                        wire::Kind::Match => {
                            format!("{}:{}  {}", row.path, row.line.unwrap_or(1), row.text)
                        }
                    };
                    children.push(button(
                        &format!("row-{index}"),
                        label,
                        Control::Row(index),
                        true,
                    ));
                }
                if output.rows.is_empty() {
                    children.push(text(
                        "empty",
                        if matches!(request.operation, wire::Operation::Search { .. }) {
                            "No matches found."
                        } else {
                            "This folder is empty."
                        }
                        .into(),
                    ));
                }
            }
            if self.pages() > 1 {
                children.push(Node {
                    key: format!("{prefix}-pages"),
                    style: Default::default(),
                    element: Element::Stack {
                        axis: Axis::Wrap,
                        children: vec![
                            button(
                                "previous",
                                "Previous page".into(),
                                Control::Previous,
                                self.page > 0,
                            ),
                            text(
                                "page",
                                format!("Page {} of {}", self.page + 1, self.pages()),
                            ),
                            button(
                                "next",
                                "Next page".into(),
                                Control::Next,
                                self.page + 1 < self.pages(),
                            ),
                        ],
                    },
                });
            }
            if output.truncated {
                children.push(text("truncated","More results may be available. Search a smaller folder or use more specific text.".into()));
            }
        }
        Node {
            key: format!("{prefix}-panel"),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(turn: usize) -> wire::Reference {
        let mut reference = wire::Reference {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            host: "local".into(),
            task: "task".into(),
            turn,
            authority: route_contract::pending::Authority {
                intent: "intent".into(),
                grant: format!("grant-{turn}"),
                checkout: "/fixture".into(),
                target: "inode".into(),
            },
        };
        reference.seal();
        reference
    }
    fn opened() -> (Inspector, wire::Request) {
        let mut inspector = Inspector::default();
        inspector.apply(&inspector.target(Control::Open));
        let Some(Need::Read(request)) = inspector.referenced(reference(1), "task") else {
            panic!("read")
        };
        (inspector, request)
    }
    #[test]
    fn stale_results_and_old_row_clicks_cannot_change_current_inspection() {
        let (mut inspector, request) = opened();
        let output = wire::Output {
            reference: request.reference.id.clone(),
            operation: request.operation.id(),
            rows: vec![wire::Row::new(
                "src".into(),
                wire::Kind::Directory,
                None,
                String::new(),
            )],
            text: None,
            content: None,
            truncated: false,
        };
        assert!(inspector.received(&request, output.clone()));
        let old = inspector.target(Control::Row(0));
        inspector.referenced(reference(2), "task");
        assert!(!inspector.received(&request, output));
        assert!(inspector.apply(&old).is_none());
        inspector.apply(&inspector.target(Control::Close));
        assert!(!inspector.visible);
        assert!(inspector.referenced(reference(1), "task").is_none());
    }
    #[test]
    fn a_late_search_cannot_replace_a_newer_query() {
        let (mut inspector, _) = opened();
        inspector.set_query("first");
        let Some(Need::Read(request)) = inspector.apply(&inspector.target(Control::Search)) else {
            panic!("search")
        };
        let output = wire::Output {
            reference: request.reference.id.clone(),
            operation: request.operation.id(),
            rows: vec![],
            text: None,
            content: None,
            truncated: false,
        };
        inspector.set_query("second");
        assert!(!inspector.received(&request, output));
        assert_eq!(inspector.query, "second");
    }
    #[test]
    fn search_keeps_its_own_bounded_query_and_file_text_has_bounded_pages() {
        let (mut inspector, _) = opened();
        inspector.set_query(&"猫".repeat(200));
        assert!(inspector.query.len() <= 256);
        inspector.set_query("needle");
        let Some(Need::Read(request)) = inspector.apply(&inspector.target(Control::Search)) else {
            panic!("search")
        };
        assert!(matches!(request.operation,wire::Operation::Search{query,..} if query=="needle"));
        let text = "x".repeat(64 * 1024);
        assert!(
            Inspector::fragments(&text)
                .iter()
                .all(|part| part.len() < 200)
        );
        assert_eq!(Inspector::fragments(&text).len(), 410);
        inspector.reset();
        assert_eq!(inspector.query, "needle");
    }
}
