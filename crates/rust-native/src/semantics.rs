//! Read-only native semantics and focus. Intents and account authority stay with the application.
use crate::view::{Element, Node, TextRole, ToolState, ValidatedView};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Group,
    List,
    Text,
    Heading,
    Status,
    Progress,
    Button,
    Checkbox,
    Radio,
    Field,
    Dialog,
    Image,
    Composer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticNode {
    pub key: String,
    pub role: Role,
    pub name: String,
    pub enabled: bool,
    pub selected: Option<bool>,
    pub checked: Option<bool>,
    pub password: bool,
    pub focusable: bool,
    pub hidden: bool,
    pub live: bool,
    pub state: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticTree {
    pub schema: String,
    pub instance: String,
    pub revision: u64,
    pub nodes: Vec<SemanticNode>,
    pub order: Vec<String>,
    pub modal: Option<String>,
}
impl SemanticTree {
    /// Derive from an already validated local view. No application intent or field value is exported.
    pub fn of<I>(view: &ValidatedView<I>) -> Self {
        fn walk<I>(
            node: &Node<I>,
            hidden: bool,
            nodes: &mut Vec<SemanticNode>,
            modal: &mut Option<String>,
        ) {
            let (
                role,
                name,
                enabled,
                selected,
                checked,
                password,
                focusable,
                live,
                state,
                children,
            ): (
                Role,
                String,
                bool,
                Option<bool>,
                Option<bool>,
                bool,
                bool,
                bool,
                Option<String>,
                &[Node<I>],
            ) = match &node.element {
                Element::Stack { children, .. } => (
                    Role::Group,
                    String::new(),
                    true,
                    None,
                    None,
                    false,
                    false,
                    false,
                    None,
                    children,
                ),
                Element::List { label, children }
                | Element::Transcript {
                    label, children, ..
                } => (
                    Role::List,
                    label.clone(),
                    true,
                    None,
                    None,
                    false,
                    false,
                    false,
                    None,
                    children,
                ),
                Element::Text { value, role } => (
                    text_role(*role),
                    value.clone(),
                    true,
                    None,
                    None,
                    false,
                    !value.is_empty(),
                    *role == TextRole::Status,
                    None,
                    &[],
                ),
                Element::RichText { runs, role } => (
                    text_role(*role),
                    runs.iter().map(|r| r.text.as_str()).collect(),
                    true,
                    None,
                    None,
                    false,
                    true,
                    *role == TextRole::Status,
                    None,
                    &[],
                ),
                Element::Field {
                    label,
                    enabled,
                    secret,
                    ..
                } => (
                    Role::Field,
                    label.clone(),
                    *enabled,
                    None,
                    None,
                    *secret,
                    true,
                    false,
                    None,
                    &[],
                ),
                Element::Choice {
                    label,
                    selected,
                    enabled,
                    children,
                    ..
                } => (
                    Role::Radio,
                    label.clone(),
                    *enabled,
                    Some(*selected),
                    Some(*selected),
                    false,
                    true,
                    false,
                    None,
                    children,
                ),
                Element::Dialog {
                    label,
                    open,
                    children,
                    ..
                } => {
                    if *open && !hidden {
                        *modal = Some(node.key.clone());
                    }
                    (
                        Role::Dialog,
                        label.clone(),
                        *open,
                        None,
                        None,
                        false,
                        *open,
                        false,
                        None,
                        children,
                    )
                }
                Element::Button {
                    label,
                    enabled,
                    icon,
                    ..
                } => {
                    let checked = icon.as_ref().and_then(|i| match i.glyph {
                        crate::Glyph::Checked => Some(true),
                        crate::Glyph::Unchecked => Some(false),
                        _ => None,
                    });
                    (
                        if checked.is_some() {
                            Role::Checkbox
                        } else {
                            Role::Button
                        },
                        label.clone(),
                        *enabled,
                        None,
                        checked,
                        false,
                        true,
                        false,
                        None,
                        &[],
                    )
                }
                Element::Surface { label, .. } => (
                    Role::Image,
                    label.clone(),
                    true,
                    None,
                    None,
                    false,
                    true,
                    false,
                    None,
                    &[],
                ),
                Element::Working { label } => (
                    Role::Progress,
                    label.clone(),
                    true,
                    None,
                    None,
                    false,
                    true,
                    true,
                    Some("Working".into()),
                    &[],
                ),
                Element::Message { children, note, .. } => (
                    Role::Group,
                    note.clone().unwrap_or_default(),
                    true,
                    None,
                    None,
                    false,
                    false,
                    false,
                    None,
                    children,
                ),
                Element::Tool {
                    name,
                    detail,
                    state,
                    children,
                } => (
                    Role::Group,
                    format!("{name}: {detail}"),
                    true,
                    None,
                    None,
                    false,
                    false,
                    *state == ToolState::Running,
                    Some(
                        match state {
                            ToolState::Running => "Working",
                            ToolState::Done => "Done",
                            ToolState::Failed => "Failed",
                        }
                        .into(),
                    ),
                    children,
                ),
                Element::Markdown { blocks } => (
                    Role::Text,
                    crate::markdown::plain(blocks),
                    true,
                    None,
                    None,
                    false,
                    true,
                    false,
                    None,
                    &[],
                ),
                Element::Composer {
                    placeholder,
                    enabled,
                    busy,
                    ..
                } => (
                    Role::Composer,
                    placeholder.clone(),
                    *enabled,
                    None,
                    None,
                    false,
                    true,
                    false,
                    if *busy { Some("Working".into()) } else { None },
                    &[],
                ),
            };
            let hidden = hidden || matches!(&node.element, Element::Dialog { open: false, .. });
            let at = nodes.len();
            nodes.push(SemanticNode {
                key: node.key.clone(),
                role,
                name,
                enabled,
                selected,
                checked,
                password,
                focusable,
                hidden,
                live,
                state,
            });
            for child in children {
                walk(child, hidden, nodes, modal);
            }
            // A closed dialog never contributes a focusable descendant.
            if hidden {
                for item in &mut nodes[at..] {
                    item.hidden = true;
                }
            }
        }
        let source = view.view();
        let mut nodes = Vec::new();
        let mut modal = None;
        walk(&source.root, false, &mut nodes, &mut modal);
        if modal.is_some() {
            // Descendants of the active modal are contiguous in preorder; later siblings are inert too.
            let modal_key = modal.as_deref().unwrap();
            fn contains<I>(node: &Node<I>, key: &str) -> bool {
                if node.key == key {
                    return true;
                }
                children(node).iter().any(|c| contains(c, key))
            }
            fn mark<I>(
                node: &Node<I>,
                key: &str,
                inside: bool,
                keys: &mut std::collections::BTreeSet<String>,
            ) {
                let inside = inside || node.key == key;
                if inside {
                    keys.insert(node.key.clone());
                }
                for c in children(node) {
                    mark(c, key, inside, keys);
                }
            }
            let mut active = std::collections::BTreeSet::new();
            if contains(&source.root, modal_key) {
                mark(&source.root, modal_key, false, &mut active);
            }
            for item in &mut nodes {
                if !active.contains(&item.key) {
                    item.hidden = true;
                }
            }
        }
        let order = nodes
            .iter()
            .filter(|n| n.focusable && !n.hidden)
            .map(|n| n.key.clone())
            .collect();
        Self {
            schema: "rust-native.semantics.v1".into(),
            instance: source.instance.clone(),
            revision: source.revision,
            nodes,
            order,
            modal,
        }
    }
    pub fn node(&self, key: &str) -> Option<&SemanticNode> {
        self.nodes
            .iter()
            .find(|n| n.key == key && !n.hidden && n.focusable)
    }
}
fn text_role(role: TextRole) -> Role {
    match role {
        TextRole::Heading => Role::Heading,
        TextRole::Status => Role::Status,
        _ => Role::Text,
    }
}
fn children<I>(node: &Node<I>) -> &[Node<I>] {
    match &node.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Transcript { children, .. }
        | Element::Dialog { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. }
        | Element::Choice { children, .. } => children,
        _ => &[],
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FocusBookmark {
    pub scope: String,
    pub key: String,
}
#[derive(Default)]
pub struct FocusTracker {
    tree: Option<SemanticTree>,
    pending: Option<SemanticTree>,
    focus: Option<String>,
    return_to: Option<String>,
    restore: Option<String>,
    closed: bool,
}
impl FocusTracker {
    pub fn prepare(&mut self, tree: SemanticTree) -> Result<(), &'static str> {
        if self.closed {
            return Err("closed");
        }
        for previous in [self.tree.as_ref(), self.pending.as_ref()]
            .into_iter()
            .flatten()
        {
            if previous.instance != tree.instance || tree.revision <= previous.revision {
                return Err("stale");
            }
        }
        self.pending = Some(tree);
        Ok(())
    }
    pub fn acknowledge(
        &mut self,
        instance: &str,
        revision: u64,
    ) -> Result<Option<String>, &'static str> {
        let next = self.pending.as_ref().ok_or("not prepared")?;
        if next.instance != instance || next.revision != revision || self.closed {
            return Err("stale");
        }
        let next = self.pending.take().unwrap();
        let old_modal = self.tree.as_ref().and_then(|t| t.modal.as_ref());
        if old_modal.is_none() && next.modal.is_some() {
            self.return_to = self.focus.clone();
        }
        let entering_modal = old_modal != next.modal.as_ref() && next.modal.is_some();
        let requested = if entering_modal {
            next.order
                .iter()
                .find(|k| {
                    next.node(k).is_some_and(|n| {
                        n.enabled
                            && matches!(
                                n.role,
                                Role::Button | Role::Field | Role::Checkbox | Role::Radio
                            )
                    })
                })
                .cloned()
        } else if old_modal.is_some() && next.modal.is_none() {
            self.return_to.take()
        } else {
            self.restore.take().or_else(|| self.focus.clone())
        };
        let old_index = self
            .tree
            .as_ref()
            .and_then(|t| {
                self.focus
                    .as_ref()
                    .and_then(|k| t.order.iter().position(|x| x == k))
            })
            .unwrap_or(0);
        self.focus = requested.filter(|k| next.node(k).is_some()).or_else(|| {
            next.order
                .get(old_index.min(next.order.len().saturating_sub(1)))
                .cloned()
        });
        self.tree = Some(next);
        Ok(self.focus.clone())
    }
    pub fn focus(&mut self, instance: &str, revision: u64, key: &str) -> bool {
        if self.closed {
            return false;
        }
        let Some(t) = &self.tree else {
            return false;
        };
        if t.instance != instance || t.revision != revision || t.node(key).is_none() {
            return false;
        }
        self.focus = Some(key.into());
        true
    }
    pub fn bookmark(&self, scope: &str) -> Option<FocusBookmark> {
        if self.closed {
            return None;
        }
        self.focus.clone().map(|key| FocusBookmark {
            scope: scope.into(),
            key,
        })
    }
    pub fn restore(&mut self, scope: &str, bookmark: FocusBookmark) -> bool {
        if self.closed || bookmark.scope != scope || bookmark.key.len() > 96 {
            return false;
        }
        self.restore = Some(bookmark.key);
        true
    }
    pub fn invalidate(&mut self) {
        self.tree = None;
        self.pending = None;
        self.focus = None;
        self.return_to = None;
        self.restore = None;
    }
    pub fn close(&mut self) {
        self.invalidate();
        self.closed = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleEvent {
    Rotation,
    ProcessRecreated,
    Background,
    Foreground,
    Offline,
    Reconnected,
    PermissionRevoked,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recovery {
    pub restore_focus: bool,
    pub pause_media: bool,
    pub revalidate_session: bool,
    pub revalidate_permissions: bool,
    pub retire_callbacks: bool,
}
impl LifecycleEvent {
    /// Effects request existing Rust owners to refresh. They never clear drafts, grants, or identity.
    pub fn recovery(self) -> Recovery {
        Recovery {
            restore_focus: matches!(
                self,
                Self::Rotation | Self::ProcessRecreated | Self::Foreground
            ),
            pause_media: matches!(self, Self::Background | Self::PermissionRevoked),
            revalidate_session: matches!(
                self,
                Self::ProcessRecreated | Self::Foreground | Self::Reconnected
            ),
            revalidate_permissions: matches!(
                self,
                Self::ProcessRecreated | Self::Foreground | Self::PermissionRevoked
            ),
            retire_callbacks: matches!(
                self,
                Self::Rotation
                    | Self::ProcessRecreated
                    | Self::Background
                    | Self::PermissionRevoked
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, View};
    fn node(key: &str, element: Element<()>) -> Node<()> {
        Node {
            key: key.into(),
            style: Default::default(),
            element,
        }
    }
    fn tree(rev: u64, modal: bool) -> SemanticTree {
        let mut children = vec![
            node(
                "heading",
                Element::Text {
                    value: "Settings".into(),
                    role: TextRole::Heading,
                },
            ),
            node(
                "secret",
                Element::Field {
                    label: "Password".into(),
                    value: String::new(),
                    placeholder: String::new(),
                    secret: true,
                    multiline: false,
                    enabled: true,
                    max_bytes: 64,
                    on_change: (),
                },
            ),
            node(
                "save",
                Element::Button {
                    label: "Save".into(),
                    enabled: true,
                    icon: None,
                    shortcut: None,
                    intent: (),
                },
            ),
        ];
        if modal {
            children.push(node(
                "dialog",
                Element::Dialog {
                    label: "Confirm".into(),
                    open: true,
                    on_close: (),
                    children: vec![node(
                        "cancel",
                        Element::Button {
                            label: "Cancel".into(),
                            enabled: true,
                            icon: None,
                            shortcut: None,
                            intent: (),
                        },
                    )],
                },
            ));
        }
        SemanticTree::of(
            &View::new_v3(
                "test",
                rev,
                node(
                    "root",
                    Element::Stack {
                        axis: Axis::Vertical,
                        children,
                    },
                ),
            )
            .validate()
            .unwrap(),
        )
    }
    #[test]
    fn roles_names_and_states_never_export_field_values() {
        let t = tree(1, false);
        assert_eq!(t.node("heading").unwrap().role, Role::Heading);
        assert!(t.node("secret").unwrap().password);
        assert!(
            !serde_json::to_string(&t)
                .unwrap()
                .contains("synthetic-secret")
        );
    }
    #[test]
    fn modal_focus_restores_origin_and_rejects_stale_focus_events() {
        let mut f = FocusTracker::default();
        f.prepare(tree(1, false)).unwrap();
        f.acknowledge("test", 1).unwrap();
        assert!(f.focus("test", 1, "save"));
        f.prepare(tree(2, true)).unwrap();
        assert_eq!(f.acknowledge("test", 2).unwrap().as_deref(), Some("cancel"));
        assert!(!f.focus("test", 1, "save"));
        f.prepare(tree(3, false)).unwrap();
        assert_eq!(f.acknowledge("test", 3).unwrap().as_deref(), Some("save"));
    }
    #[test]
    fn bookmark_is_only_scoped_identity_and_process_restore_needs_a_new_live_tree() {
        let mut f = FocusTracker::default();
        f.prepare(tree(1, false)).unwrap();
        f.acknowledge("test", 1).unwrap();
        f.focus("test", 1, "save");
        let b = f.bookmark("account-a/settings").unwrap();
        let bytes = serde_json::to_string(&b).unwrap();
        assert!(!bytes.contains("Password"));
        let mut fresh = FocusTracker::default();
        assert!(!fresh.restore("account-b/settings", b.clone()));
        assert!(fresh.restore("account-a/settings", b));
        assert!(!fresh.focus("test", 1, "save"));
        fresh.prepare(tree(1, false)).unwrap();
        assert_eq!(
            fresh.acknowledge("test", 1).unwrap().as_deref(),
            Some("save")
        );
    }
    #[test]
    fn stream_updates_keep_focus_and_removal_has_a_deterministic_neighbor() {
        let mut f = FocusTracker::default();
        f.prepare(tree(1, false)).unwrap();
        f.acknowledge("test", 1).unwrap();
        assert!(f.focus("test", 1, "secret"));
        f.prepare(tree(2, false)).unwrap();
        assert!(!f.focus("test", 2, "save"));
        assert_eq!(f.acknowledge("test", 2).unwrap().as_deref(), Some("secret"));
        let mut next = tree(3, false);
        next.nodes.retain(|n| n.key != "secret");
        next.order.retain(|k| k != "secret");
        f.prepare(next).unwrap();
        assert_eq!(f.acknowledge("test", 3).unwrap().as_deref(), Some("save"));
        assert!(f.prepare(tree(2, false)).is_err());
        f.close();
        assert!(f.prepare(tree(4, false)).is_err());
    }
    #[test]
    fn other_instances_and_unmounted_revisions_cannot_move_focus() {
        let mut f = FocusTracker::default();
        f.prepare(tree(1, false)).unwrap();
        assert!(f.acknowledge("test", 2).is_err());
        assert!(!f.focus("test", 1, "save"));
        f.acknowledge("test", 1).unwrap();
        assert!(!f.focus("other", 1, "save"));
        let mut other = tree(2, false);
        other.instance = "other".into();
        assert!(f.prepare(other).is_err());
    }
    #[test]
    fn permission_revocation_retires_callbacks_without_clearing_domain_state() {
        let r = LifecycleEvent::PermissionRevoked.recovery();
        assert!(r.pause_media && r.retire_callbacks && r.revalidate_permissions);
        let mut f = FocusTracker::default();
        f.prepare(tree(1, false)).unwrap();
        f.acknowledge("test", 1).unwrap();
        f.invalidate();
        assert!(!f.focus("test", 1, "save"));
        let json = serde_json::to_string(&r).unwrap();
        for forbidden in ["delete", "clear_draft", "revoke_grant", "credential"] {
            assert!(!json.contains(forbidden));
        }
    }
}

/// Native transcript actions page through measured Rust rows, never guessed pixel offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Earlier,
    Later,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub start: u32,
    pub end: u32,
    pub earlier: bool,
    pub later: bool,
}
pub fn page(total: u32, anchor: u32, direction: Direction, count: u32) -> Page {
    let count = count.clamp(1, 32);
    let anchor = anchor.min(total.saturating_sub(1));
    let start = match direction {
        Direction::Earlier => anchor.saturating_sub(count),
        Direction::Later => anchor.saturating_add(1).min(total),
    };
    let end = match direction {
        Direction::Earlier => anchor.min(total),
        Direction::Later => start.saturating_add(count).min(total),
    };
    Page {
        start,
        end,
        earlier: start > 0,
        later: end < total,
    }
}

/// Layout inputs, not a second text measurer. Adapters must request a fresh Rust layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub font_scale: f32,
    pub density: f32,
    pub reduced_motion: bool,
}
impl Environment {
    pub fn valid(&self) -> bool {
        self.font_scale.is_finite()
            && self.font_scale > 0.0
            && self.density.is_finite()
            && self.density > 0.0
    }
}
#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn huge_transcript_has_bounded_pages_and_stable_boundaries() {
        assert_eq!(
            page(100_000, 50_000, Direction::Earlier, 500),
            Page {
                start: 49968,
                end: 50000,
                earlier: true,
                later: true
            }
        );
        assert_eq!(page(100_000, 99999, Direction::Later, 32).start, 100000);
        assert_eq!(
            page(0, 0, Direction::Later, 32),
            Page {
                start: 0,
                end: 0,
                earlier: false,
                later: false
            }
        );
    }
    #[test]
    fn font_display_scale_and_reduced_motion_are_independent_layout_inputs() {
        for font_scale in [1.0, 1.3, 2.0, 3.5] {
            for density in [1.0, 2.0, 4.0] {
                for reduced_motion in [false, true] {
                    assert!(
                        Environment {
                            font_scale,
                            density,
                            reduced_motion
                        }
                        .valid()
                    );
                }
            }
        }
        assert!(
            !Environment {
                font_scale: f32::NAN,
                density: 1.0,
                reduced_motion: false
            }
            .valid()
        );
    }
    #[test]
    fn interruption_effects_refresh_existing_owners() {
        for e in [
            LifecycleEvent::Rotation,
            LifecycleEvent::ProcessRecreated,
            LifecycleEvent::Background,
            LifecycleEvent::Foreground,
            LifecycleEvent::Offline,
            LifecycleEvent::Reconnected,
            LifecycleEvent::PermissionRevoked,
        ] {
            let r = e.recovery();
            if e == LifecycleEvent::ProcessRecreated {
                assert!(
                    r.restore_focus
                        && r.revalidate_session
                        && r.revalidate_permissions
                        && r.retire_callbacks
                );
            }
            if e == LifecycleEvent::Reconnected {
                assert!(r.revalidate_session);
            }
        }
    }
}
