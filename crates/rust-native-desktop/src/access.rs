//! The accessibility tree: the validated view, as the window laid it out,
//! for screen readers (VoiceOver, Orca through AT-SPI, Narrator).
//!
//! Every node the view carries becomes an [`accesskit`] node with a role,
//! a name, its bounds, and the actions it admits:
//!
//! - a button is a `Button` (a `CheckBox` with the checkbox glyphs) named
//!   by its label, disabled when the view
//!   disables it, and clickable and focusable when it is enabled;
//! - a composer is a `MultilineTextInput` named by its placeholder, with
//!   the text the person has typed as its value;
//! - text is a `Label` whose value is the text, and a heading a `Heading`;
//! - a list is a `List` named by its label, a card a `Group`, a
//!   transcript a `Log`, a message an `Article` named by who
//!   wrote it, a tool a `Group`, and the working row a polite `Status`;
//! - a surface the application paints itself is an `Image` named by its
//!   label, unless the application describes what it paints as semantic
//!   rows ([`Content`], such as a transcript's messages and controls),
//!   which then appear under it as the same kinds of nodes.
//!
//! Nothing here runs an intent. A screen reader's request is answered as
//! a [`Request`]: the same activation a click or Enter makes, the same
//! focus Tab moves, or the same pointer and text input a person gives a
//! surface. The window resolves it as it resolves that input, against the
//! current view, before the application sees anything.

use crate::input::{SurfaceInput, TextInput};
use crate::layout::{Op, Rect, Scene};
use accesskit::{
    Action, ActionData, ActionRequest, Live, Node as Ak, NodeId, Role, Toggled, TreeId, TreeInfo,
    TreeUpdate,
};
use rust_native::{Element, Glyph, MessageRole, Node, TextRole, ToolState, View};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

pub use accesskit;

/// The window: the tree's root.
pub const ROOT: NodeId = NodeId(0);

/// What a surface paints, as semantic rows, for assistive technology only.
/// The rows are never activated from here: a control in them is reached by
/// a click at its bounds, the input the surface already handles.
#[derive(Clone, Debug, Default)]
pub struct Content {
    /// The rows, in reading order.
    pub rows: Vec<Arc<Node<()>>>,
    /// Surface-local bounds, in points, of the rows and controls now on
    /// screen, by node key. A control without bounds cannot be clicked.
    pub bounds: HashMap<String, Rect>,
    /// The rows are a conversation, read as a log.
    pub conversation: bool,
}

/// A screen reader's request, as input the window already handles.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Activate the view node `key`, as a click or Enter does.
    Activate(String),
    /// Move keyboard focus to the view node `key`, as Tab does.
    Focus(String),
    /// Press and release at `x`, `y` in the surface `resource`, in
    /// surface-local points.
    Click { resource: String, x: f32, y: f32 },
    /// Replace the text of the composer `key` (its surface `resource`,
    /// focused by a click at `x`, `y` when it is not already).
    SetValue {
        key: String,
        resource: String,
        x: f32,
        y: f32,
        value: String,
    },
    /// Type `text` into the composer `key` at its caret, focusing it first
    /// as [`Request::SetValue`] does.
    Insert {
        key: String,
        resource: String,
        x: f32,
        y: f32,
        text: String,
    },
}

/// What an accessible node's actions reach.
#[derive(Clone, Debug)]
enum Target {
    /// A button in the view.
    Button(String),
    /// A composer in the view, painted in its surface; the point puts the
    /// caret at the end of its text.
    Field {
        key: String,
        resource: String,
        at: (f32, f32),
    },
    /// A control painted in a surface, clicked at its center.
    Control { resource: String, at: (f32, f32) },
}

/// Where the tree's names, values, bounds, and focus come from.
pub struct Source<'a> {
    /// The window's title, the root's name.
    pub title: &'a str,
    /// The laid-out view.
    pub scene: &'a Scene,
    /// Pixels a point: accessibility bounds are in the window's pixels.
    pub scale: f32,
    /// How far the window's single scroll region has scrolled, in points.
    pub scroll: f32,
    /// The view node holding the window's keyboard focus (Tab), if any.
    pub focus: Option<&'a str>,
    /// The view node holding the application's text cursor, such as a
    /// composer; it wins over `focus` when the view has it.
    pub text_focus: Option<&'a str>,
    /// The live text of a composer, by node key.
    pub value: &'a dyn Fn(&str) -> Option<String>,
    /// What a surface paints, by resource.
    pub content: &'a dyn Fn(&str) -> Option<Content>,
}

/// One accessibility tree and what its actions reach.
#[derive(Clone, Debug)]
pub struct Tree {
    /// A complete tree, focus included.
    pub update: TreeUpdate,
    targets: HashMap<NodeId, Target>,
    ids: HashMap<String, NodeId>,
}

impl Tree {
    /// The tree for `view` as `source` laid it out.
    pub fn build<I>(view: &View<I>, source: &Source<'_>) -> Tree {
        let mut builder = Builder {
            source,
            nodes: Vec::new(),
            targets: HashMap::new(),
            ids: HashMap::new(),
            used: HashSet::from([ROOT.0]),
            surfaces: source
                .scene
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::Surface { resource, rect, .. } => Some((resource.clone(), *rect)),
                    _ => None,
                })
                .collect(),
        };
        let child = builder.node(&view.root, None);
        let mut root = Ak::new(Role::Window);
        root.set_label(source.title);
        root.set_children(vec![child]);
        builder.nodes.push((ROOT, root));
        let focus = [source.text_focus, source.focus]
            .into_iter()
            .flatten()
            .find_map(|key| builder.ids.get(key).copied())
            .unwrap_or(ROOT);
        let mut tree = TreeInfo::new(ROOT);
        tree.toolkit_name = Some("Rust Native".into());
        Tree {
            update: TreeUpdate {
                nodes: builder.nodes,
                tree: Some(tree),
                tree_id: TreeId::ROOT,
                focus,
            },
            targets: builder.targets,
            ids: builder.ids,
        }
    }

    /// The tree for `app`'s current view, laid out as `scene`, with its
    /// composers' text and its surfaces' rows from the application.
    pub fn of<A: crate::App>(
        app: &A,
        scene: &Scene,
        focus: Option<&str>,
        scale: f32,
        scroll: f32,
    ) -> Tree {
        let title = app.title();
        let own = app.access_focus();
        Tree::build(
            app.view().view(),
            &Source {
                title: &title,
                scene,
                scale,
                scroll,
                focus,
                text_focus: own.as_deref(),
                value: &|key| app.access_value(key),
                content: &|resource| app.access_content(resource),
            },
        )
    }

    /// The node for the view node `key`, or for `key` among the rows of
    /// the surface `resource` as `resource/key`.
    pub fn id(&self, key: &str) -> Option<NodeId> {
        self.ids.get(key).copied()
    }

    /// The node `id`, as the update carries it.
    pub fn node(&self, id: NodeId) -> Option<&Ak> {
        self.update
            .nodes
            .iter()
            .find(|(found, _)| *found == id)
            .map(|(_, node)| node)
    }

    /// Answers a screen reader's `request` as the input it stands for, or
    /// nothing when this tree's node does not admit it.
    pub fn request(&self, request: &ActionRequest) -> Option<Request> {
        let target = self.targets.get(&request.target_node)?;
        let text = match &request.data {
            Some(ActionData::Value(value)) => Some(value.to_string()),
            _ => None,
        };
        match (target, request.action) {
            (Target::Button(key), Action::Click) => Some(Request::Activate(key.clone())),
            (Target::Button(key), Action::Focus) => Some(Request::Focus(key.clone())),
            (Target::Field { resource, at, .. }, Action::Click | Action::Focus) => {
                Some(Request::Click {
                    resource: resource.clone(),
                    x: at.0,
                    y: at.1,
                })
            }
            (Target::Field { key, resource, at }, Action::SetValue) => Some(Request::SetValue {
                key: key.clone(),
                resource: resource.clone(),
                x: at.0,
                y: at.1,
                value: text?,
            }),
            (Target::Field { key, resource, at }, Action::ReplaceSelectedText) => {
                Some(Request::Insert {
                    key: key.clone(),
                    resource: resource.clone(),
                    x: at.0,
                    y: at.1,
                    text: text?,
                })
            }
            (Target::Control { resource, at }, Action::Click) => Some(Request::Click {
                resource: resource.clone(),
                x: at.0,
                y: at.1,
            }),
            _ => None,
        }
    }
}

/// Runs a screen reader's `request` on `app` as the input it stands for:
/// an activation resolved against the current view (a click's), a press
/// and release in a surface, or the keys and text a person types. Moving
/// the window's focus is the window's own: that comes back as `Some(key)`.
pub fn answer<A: crate::App>(app: &mut A, request: Request, now: Instant) -> Option<String> {
    let click = |app: &mut A, resource: &str, x: f32, y: f32| {
        app.surface_input(resource, SurfaceInput::Down { x, y, shift: false }, now);
        app.surface_input(resource, SurfaceInput::Up { x, y }, now);
    };
    let key = |app: &mut A, key: &str, command: bool| {
        app.text_input(
            TextInput::Key {
                key,
                text: None,
                control: false,
                command,
                alt: false,
                shift: false,
            },
            now,
        );
    };
    match request {
        Request::Activate(node) => {
            let view = app.view().view();
            let activation = rust_native::Activation {
                instance: view.instance.clone(),
                revision: view.revision,
                node,
            };
            if let Ok(intent) = app.view().activate(&activation) {
                let intent = intent.clone();
                app.activate(intent, now);
            }
        }
        Request::Focus(node) => return app.allows_focus(&node).then_some(node),
        Request::Click { resource, x, y } => click(app, &resource, x, y),
        Request::SetValue {
            key: node,
            resource,
            x,
            y,
            value,
        } => {
            if app.access_focus().as_deref() != Some(&node) {
                click(app, &resource, x, y);
            }
            key(app, "a", true);
            if value.is_empty() {
                key(app, "Backspace", false);
            } else {
                app.text_input(TextInput::Commit(&value), now);
            }
        }
        Request::Insert {
            key: node,
            resource,
            x,
            y,
            text,
        } => {
            if app.access_focus().as_deref() != Some(&node) {
                click(app, &resource, x, y);
            }
            app.text_input(TextInput::Commit(&text), now);
        }
    }
    None
}

/// Inside a surface: its resource, what it paints, and its origin in
/// scene points.
struct Painted<'c> {
    resource: &'c str,
    content: &'c Content,
    origin: (f32, f32),
}

struct Builder<'a, 's> {
    source: &'a Source<'s>,
    nodes: Vec<(NodeId, Ak)>,
    targets: HashMap<NodeId, Target>,
    ids: HashMap<String, NodeId>,
    used: HashSet<u64>,
    surfaces: HashMap<String, Rect>,
}

impl Builder<'_, '_> {
    /// A stable id for `name`: its FNV-1a hash, moved on past any id this
    /// tree already used.
    fn id(&mut self, name: &str) -> NodeId {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in name.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        while !self.used.insert(hash) {
            hash = hash.wrapping_add(1);
        }
        let id = NodeId(hash);
        self.ids.insert(name.to_owned(), id);
        id
    }

    /// `rect` in scene points as window pixels.
    fn bounds(&self, rect: Rect) -> accesskit::Rect {
        let scale = f64::from(self.source.scale);
        let (x, y) = (f64::from(rect.x), f64::from(rect.y - self.source.scroll));
        accesskit::Rect {
            x0: x * scale,
            y0: y * scale,
            x1: (x + f64::from(rect.w)) * scale,
            y1: (y + f64::from(rect.h)) * scale,
        }
    }

    fn node<I>(&mut self, node: &Node<I>, painted: Option<&Painted<'_>>) -> NodeId {
        let id = match painted {
            Some(painted) => self.id(&format!("{}/{}", painted.resource, node.key)),
            None => self.id(&node.key),
        };
        let rect = match painted {
            Some(painted) => painted.content.bounds.get(&node.key).map(|rect| Rect {
                x: rect.x + painted.origin.0,
                y: rect.y + painted.origin.1,
                ..*rect
            }),
            None => self.source.scene.bounds.get(&node.key).copied(),
        };
        let mut children = Vec::new();
        let mut out = match &node.element {
            Element::Surface { resource, label } => {
                let mut out;
                let surface = self.surfaces.get(resource).copied().or(rect);
                if let (None, Some(content)) = (painted, (self.source.content)(resource)) {
                    out = Ak::new(if content.conversation {
                        Role::Log
                    } else {
                        Role::Group
                    });
                    let origin = surface.map_or((0.0, 0.0), |rect| (rect.x, rect.y));
                    let inner = Painted {
                        resource,
                        content: &content,
                        origin,
                    };
                    for row in &content.rows {
                        children.push(self.node(row.as_ref(), Some(&inner)));
                    }
                } else {
                    out = Ak::new(Role::Image);
                }
                out.set_label(label.as_str());
                out
            }
            Element::Stack {
                children: nodes, ..
            } => {
                let card = node.style.background.is_some_and(|color| color.alpha > 0);
                for child in nodes {
                    children.push(self.node(child, painted));
                }
                Ak::new(if card {
                    Role::Group
                } else {
                    Role::GenericContainer
                })
            }
            Element::List {
                label,
                children: nodes,
            } => {
                for child in nodes {
                    children.push(self.node(child, painted));
                }
                let mut out = Ak::new(Role::List);
                out.set_label(label.as_str());
                out
            }
            Element::Text { value, role } => {
                if *role == TextRole::Heading {
                    let mut out = Ak::new(Role::Heading);
                    out.set_label(value.as_str());
                    out
                } else {
                    let mut out = Ak::new(Role::Label);
                    out.set_value(value.as_str());
                    out
                }
            }
            Element::Button {
                label,
                enabled,
                icon,
                ..
            } => {
                let glyph = icon.map(|icon| icon.glyph);
                let mut out = Ak::new(match glyph {
                    Some(Glyph::Checked | Glyph::Unchecked) => Role::CheckBox,
                    _ => Role::Button,
                });
                match glyph {
                    Some(Glyph::Checked) => out.set_toggled(Toggled::True),
                    Some(Glyph::Unchecked) => out.set_toggled(Toggled::False),
                    _ => {}
                }
                out.set_label(label.as_str());
                if !*enabled {
                    out.set_disabled();
                } else if let Some(painted) = painted {
                    if let Some(rect) = rect {
                        out.add_action(Action::Click);
                        self.targets.insert(
                            id,
                            Target::Control {
                                resource: painted.resource.to_owned(),
                                at: (
                                    rect.x - painted.origin.0 + rect.w / 2.0,
                                    rect.y - painted.origin.1 + rect.h / 2.0,
                                ),
                            },
                        );
                    }
                } else {
                    out.add_action(Action::Click);
                    out.add_action(Action::Focus);
                    self.targets.insert(id, Target::Button(node.key.clone()));
                }
                out
            }
            Element::Transcript {
                label,
                children: nodes,
                earlier,
                ..
            } => {
                if let Some(earlier) = earlier {
                    let earlier_id = self.id(&format!("{}/earlier", node.key));
                    let mut button = Ak::new(Role::Button);
                    button.set_label(earlier.label.as_str());
                    if earlier.loading || painted.is_some() {
                        button.set_disabled();
                    } else {
                        button.add_action(Action::Click);
                        self.targets
                            .insert(earlier_id, Target::Button(node.key.clone()));
                    }
                    self.nodes.push((earlier_id, button));
                    children.push(earlier_id);
                }
                for child in nodes {
                    children.push(self.node(child, painted));
                }
                let mut out = Ak::new(Role::Log);
                out.set_label(label.as_str());
                out
            }
            Element::Message {
                role,
                note,
                children: nodes,
            } => {
                for child in nodes {
                    children.push(self.node(child, painted));
                }
                let mut out = Ak::new(Role::Article);
                out.set_label(match role {
                    MessageRole::User => "You",
                    MessageRole::Assistant => "Reply",
                    MessageRole::System => "Notice",
                });
                if let Some(note) = note {
                    out.set_description(note.as_str());
                }
                out
            }
            Element::Markdown { blocks } => {
                let mut out = Ak::new(Role::Label);
                out.set_value(rust_native::markdown::plain(blocks));
                out
            }
            Element::Tool {
                name,
                detail,
                state,
                children: nodes,
            } => {
                for child in nodes {
                    children.push(self.node(child, painted));
                }
                let mut out = Ak::new(Role::Group);
                out.set_label(if detail.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {detail}")
                });
                out.set_description(match state {
                    ToolState::Running => "Running",
                    ToolState::Done => "Done",
                    ToolState::Failed => "Failed",
                });
                out
            }
            Element::Working { label } => {
                let mut out = Ak::new(Role::Status);
                out.set_label(label.as_str());
                out.set_live(Live::Polite);
                out
            }
            Element::Composer {
                placeholder,
                enabled,
                draft,
                ..
            } => {
                let mut out = Ak::new(Role::MultilineTextInput);
                out.set_label(placeholder.as_str());
                out.set_placeholder(placeholder.as_str());
                let value = (self.source.value)(&node.key)
                    .or_else(|| draft.clone())
                    .unwrap_or_default();
                out.set_value(value);
                let resource = format!("composer:{}", node.key);
                let field = self.surfaces.get(&resource).copied().or(rect);
                if !*enabled {
                    out.set_disabled();
                } else if painted.is_none()
                    && let Some(field) = field
                {
                    out.add_action(Action::Focus);
                    out.add_action(Action::Click);
                    out.add_action(Action::SetValue);
                    out.add_action(Action::ReplaceSelectedText);
                    self.targets.insert(
                        id,
                        Target::Field {
                            key: node.key.clone(),
                            resource,
                            at: ((field.w - 2.0).max(0.0), (field.h - 2.0).max(0.0)),
                        },
                    );
                }
                out
            }
            Element::RichText { .. }
            | Element::Field { .. }
            | Element::Choice { .. }
            | Element::Dialog { .. } => {
                let mut out = Ak::new(Role::Label);
                out.set_value("Unsupported v3 component");
                out
            }
        };
        if let Some(rect) = rect {
            out.set_bounds(self.bounds(rect));
        }
        if !children.is_empty() {
            out.set_children(children);
        }
        self.nodes.push((id, out));
        id
    }
}

#[cfg(test)]
mod tests;
