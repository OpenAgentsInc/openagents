//! Accessibility Support for wgpui
//!
//! Provides accessibility features for screen readers and assistive technologies.
//!
//! # Features
//! - Semantic tree building for accessibility APIs
//! - ARIA-like role and property system
//! - Focus management and keyboard navigation
//! - Live region announcements
//! - High contrast mode support

use crate::{Bounds, Point};
use std::collections::HashMap;

/// Unique identifier for an accessible node
pub type AccessibleId = u64;

/// Role of an accessible element (similar to ARIA roles)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Role {
    /// Generic container
    #[default]
    Group,
    /// Main content region
    Main,
    /// Navigation region
    Navigation,
    /// Complementary content
    Complementary,
    /// Banner/header region
    Banner,
    /// Content information/footer
    ContentInfo,
    /// Search region
    Search,
    /// Form region
    Form,
    /// Interactive button
    Button,
    /// Link/anchor
    Link,
    /// Text input field
    TextInput,
    /// Multi-line text area
    TextArea,
    /// Checkbox
    Checkbox,
    /// Radio button
    Radio,
    /// Dropdown/select
    ComboBox,
    /// List container
    List,
    /// List item
    ListItem,
    /// Menu container
    Menu,
    /// Menu item
    MenuItem,
    /// Tab list container
    TabList,
    /// Tab button
    Tab,
    /// Tab content panel
    TabPanel,
    /// Dialog/modal
    Dialog,
    /// Alert dialog
    AlertDialog,
    /// Tooltip
    Tooltip,
    /// Status message
    Status,
    /// Alert message
    Alert,
    /// Progress indicator
    ProgressBar,
    /// Slider
    Slider,
    /// Scrollable region
    ScrollArea,
    /// Grid/table
    Grid,
    /// Grid cell
    Cell,
    /// Row in grid
    Row,
    /// Column header
    ColumnHeader,
    /// Row header
    RowHeader,
    /// Image
    Image,
    /// Heading (level specified in properties)
    Heading,
    /// Static text
    StaticText,
    /// Separator/divider
    Separator,
    /// Tree view
    Tree,
    /// Tree item
    TreeItem,
    /// Application region
    Application,
    /// Document content
    Document,
    /// Article
    Article,
    /// Presentation (decorative, no semantics)
    Presentation,
    /// None (hidden from accessibility tree)
    None,
}

/// State of an accessible element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Element is disabled
    Disabled,
    /// Element is expanded (for expandable items)
    Expanded,
    /// Element is collapsed
    Collapsed,
    /// Element is selected
    Selected,
    /// Element is checked
    Checked,
    /// Element is unchecked
    Unchecked,
    /// Element is in mixed/indeterminate state
    Mixed,
    /// Element is pressed
    Pressed,
    /// Element is focused
    Focused,
    /// Element is hovered
    Hovered,
    /// Element is busy/loading
    Busy,
    /// Element is required
    Required,
    /// Element is invalid
    Invalid,
    /// Element is hidden
    Hidden,
    /// Element is read-only
    ReadOnly,
    /// Element is current (for navigation)
    Current,
}

/// Live region politeness level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveRegion {
    /// No announcements
    #[default]
    Off,
    /// Announce when idle
    Polite,
    /// Announce immediately
    Assertive,
}

/// Accessible node in the accessibility tree
#[derive(Debug, Clone)]
pub struct AccessibleNode {
    /// Unique identifier
    pub id: AccessibleId,
    /// Role of this element
    pub role: Role,
    /// Human-readable label
    pub label: Option<String>,
    /// Detailed description
    pub description: Option<String>,
    /// Value (for inputs, sliders, etc.)
    pub value: Option<String>,
    /// Placeholder text
    pub placeholder: Option<String>,
    /// Keyboard shortcut
    pub keyboard_shortcut: Option<String>,
    /// Bounding rectangle
    pub bounds: Bounds,
    /// Current states
    pub states: Vec<State>,
    /// Live region setting
    pub live_region: LiveRegion,
    /// Whether live region is atomic
    pub live_atomic: bool,
    /// Child node IDs
    pub children: Vec<AccessibleId>,
    /// Parent node ID
    pub parent: Option<AccessibleId>,
    /// Tab index for focus order (-1 = not focusable, 0+ = focusable)
    pub tab_index: i32,
    /// Heading level (1-6 for Heading role)
    pub heading_level: Option<u8>,
    /// Value range (for sliders, progress bars)
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    pub value_now: Option<f32>,
    /// Position in set (for list items, etc.)
    pub pos_in_set: Option<u32>,
    /// Size of set
    pub set_size: Option<u32>,
    /// Row/column for grid cells
    pub row_index: Option<u32>,
    pub column_index: Option<u32>,
    pub row_span: Option<u32>,
    pub column_span: Option<u32>,
    /// ID of element this controls
    pub controls: Option<AccessibleId>,
    /// ID of element that labels this
    pub labelled_by: Option<AccessibleId>,
    /// ID of element that describes this
    pub described_by: Option<AccessibleId>,
    /// IDs of elements that are members (for groups)
    pub owns: Vec<AccessibleId>,
    /// Custom properties
    pub properties: HashMap<String, String>,
}

impl AccessibleNode {
    /// Create a new accessible node
    pub fn new(id: AccessibleId, role: Role, bounds: Bounds) -> Self {
        Self {
            id,
            role,
            label: None,
            description: None,
            value: None,
            placeholder: None,
            keyboard_shortcut: None,
            bounds,
            states: Vec::new(),
            live_region: LiveRegion::Off,
            live_atomic: false,
            children: Vec::new(),
            parent: None,
            tab_index: -1,
            heading_level: None,
            value_min: None,
            value_max: None,
            value_now: None,
            pos_in_set: None,
            set_size: None,
            row_index: None,
            column_index: None,
            row_span: None,
            column_span: None,
            controls: None,
            labelled_by: None,
            described_by: None,
            owns: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the value
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set placeholder text
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set keyboard shortcut
    pub fn keyboard_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.keyboard_shortcut = Some(shortcut.into());
        self
    }

    /// Add a state
    pub fn state(mut self, state: State) -> Self {
        if !self.states.contains(&state) {
            self.states.push(state);
        }
        self
    }

    /// Set multiple states
    pub fn states(mut self, states: Vec<State>) -> Self {
        self.states = states;
        self
    }

    /// Set live region behavior
    pub fn live(mut self, live: LiveRegion) -> Self {
        self.live_region = live;
        self
    }

    /// Set live region as atomic
    pub fn atomic(mut self) -> Self {
        self.live_atomic = true;
        self
    }

    /// Make focusable (tab_index = 0)
    pub fn focusable(mut self) -> Self {
        self.tab_index = 0;
        self
    }

    /// Set explicit tab index
    pub fn tab_index(mut self, index: i32) -> Self {
        self.tab_index = index;
        self
    }

    /// Set heading level
    pub fn heading_level(mut self, level: u8) -> Self {
        self.heading_level = Some(level.clamp(1, 6));
        self
    }

    /// Set value range
    pub fn value_range(mut self, min: f32, max: f32, now: f32) -> Self {
        self.value_min = Some(min);
        self.value_max = Some(max);
        self.value_now = Some(now);
        self
    }

    /// Set position in set
    pub fn position(mut self, pos: u32, size: u32) -> Self {
        self.pos_in_set = Some(pos);
        self.set_size = Some(size);
        self
    }

    /// Set grid cell position
    pub fn cell_position(mut self, row: u32, col: u32) -> Self {
        self.row_index = Some(row);
        self.column_index = Some(col);
        self
    }

    /// Set grid cell span
    pub fn cell_span(mut self, row_span: u32, col_span: u32) -> Self {
        self.row_span = Some(row_span);
        self.column_span = Some(col_span);
        self
    }

    /// Set controls relationship
    pub fn controls(mut self, id: AccessibleId) -> Self {
        self.controls = Some(id);
        self
    }

    /// Set labelled-by relationship
    pub fn labelled_by(mut self, id: AccessibleId) -> Self {
        self.labelled_by = Some(id);
        self
    }

    /// Set described-by relationship
    pub fn described_by(mut self, id: AccessibleId) -> Self {
        self.described_by = Some(id);
        self
    }

    /// Add custom property
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Check if node has a specific state
    pub fn has_state(&self, state: State) -> bool {
        self.states.contains(&state)
    }

    /// Check if node is focusable
    pub fn is_focusable(&self) -> bool {
        self.tab_index >= 0 && !self.has_state(State::Disabled) && !self.has_state(State::Hidden)
    }

    /// Get accessible name (label or value)
    pub fn accessible_name(&self) -> Option<&str> {
        self.label.as_deref().or(self.value.as_deref())
    }
}

/// Accessibility tree containing all accessible nodes
#[derive(Debug, Default)]
pub struct AccessibilityTree {
    /// All nodes indexed by ID
    nodes: HashMap<AccessibleId, AccessibleNode>,
    /// Root node ID
    root: Option<AccessibleId>,
    /// Currently focused node ID
    focused: Option<AccessibleId>,
    /// Next available ID
    next_id: AccessibleId,
    /// Focus order cache
    focus_order: Vec<AccessibleId>,
    /// Whether focus order needs rebuild
    focus_dirty: bool,
}

impl AccessibilityTree {
    /// Create a new empty accessibility tree
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate next unique ID
    pub fn next_id(&mut self) -> AccessibleId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Set the root node
    pub fn set_root(&mut self, id: AccessibleId) {
        self.root = Some(id);
    }

    /// Get the root node ID
    pub fn root(&self) -> Option<AccessibleId> {
        self.root
    }

    /// Add a node to the tree
    pub fn add_node(&mut self, node: AccessibleNode) {
        self.focus_dirty = true;
        self.nodes.insert(node.id, node);
    }

    /// Remove a node from the tree
    pub fn remove_node(&mut self, id: AccessibleId) {
        if let Some(node) = self.nodes.remove(&id) {
            // Remove from parent's children
            if let Some(parent_id) = node.parent
                && let Some(parent) = self.nodes.get_mut(&parent_id)
            {
                parent.children.retain(|&child_id| child_id != id);
            }
            // Remove children
            for child_id in node.children {
                self.remove_node(child_id);
            }
            self.focus_dirty = true;
        }
    }

    /// Get a node by ID
    pub fn get(&self, id: AccessibleId) -> Option<&AccessibleNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable node by ID
    pub fn get_mut(&mut self, id: AccessibleId) -> Option<&mut AccessibleNode> {
        self.nodes.get_mut(&id)
    }

    /// Add a child to a parent node
    pub fn add_child(&mut self, parent_id: AccessibleId, child_id: AccessibleId) {
        if let Some(parent) = self.nodes.get_mut(&parent_id)
            && !parent.children.contains(&child_id)
        {
            parent.children.push(child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        self.focus_dirty = true;
    }

    /// Get currently focused node
    pub fn focused(&self) -> Option<&AccessibleNode> {
        self.focused.and_then(|id| self.nodes.get(&id))
    }

    /// Get currently focused node ID
    pub fn focused_id(&self) -> Option<AccessibleId> {
        self.focused
    }

    /// Set focus to a node
    pub fn set_focus(&mut self, id: AccessibleId) -> bool {
        if let Some(node) = self.nodes.get(&id)
            && node.is_focusable()
        {
            // Remove focus from previous
            if let Some(prev_id) = self.focused
                && let Some(prev) = self.nodes.get_mut(&prev_id)
            {
                prev.states.retain(|&s| s != State::Focused);
            }
            // Set focus on new
            if let Some(new) = self.nodes.get_mut(&id)
                && !new.states.contains(&State::Focused)
            {
                new.states.push(State::Focused);
            }
            self.focused = Some(id);
            return true;
        }
        false
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        if let Some(id) = self.focused
            && let Some(node) = self.nodes.get_mut(&id)
        {
            node.states.retain(|&s| s != State::Focused);
        }
        self.focused = None;
    }

    /// Move focus to next focusable element
    pub fn focus_next(&mut self) -> Option<AccessibleId> {
        self.rebuild_focus_order_if_needed();

        if self.focus_order.is_empty() {
            return None;
        }

        let current_idx = self
            .focused
            .and_then(|id| self.focus_order.iter().position(|&fid| fid == id));

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % self.focus_order.len(),
            None => 0,
        };

        let next_id = self.focus_order[next_idx];
        if self.set_focus(next_id) {
            Some(next_id)
        } else {
            None
        }
    }

    /// Move focus to previous focusable element
    pub fn focus_prev(&mut self) -> Option<AccessibleId> {
        self.rebuild_focus_order_if_needed();

        if self.focus_order.is_empty() {
            return None;
        }

        let current_idx = self
            .focused
            .and_then(|id| self.focus_order.iter().position(|&fid| fid == id));

        let prev_idx = match current_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) => self.focus_order.len() - 1,
            None => self.focus_order.len() - 1,
        };

        let prev_id = self.focus_order[prev_idx];
        if self.set_focus(prev_id) {
            Some(prev_id)
        } else {
            None
        }
    }

    /// Rebuild focus order if dirty
    fn rebuild_focus_order_if_needed(&mut self) {
        if !self.focus_dirty {
            return;
        }

        self.focus_order.clear();

        // Collect focusable nodes with their tab indices
        let mut focusable: Vec<(AccessibleId, i32, Bounds)> = self
            .nodes
            .values()
            .filter(|n| n.is_focusable())
            .map(|n| (n.id, n.tab_index, n.bounds))
            .collect();

        // Sort by tab index, then by position (top to bottom, left to right)
        focusable.sort_by(|a, b| {
            match a.1.cmp(&b.1) {
                std::cmp::Ordering::Equal => {
                    // Sort by Y position first, then X
                    match a.2.origin.y.partial_cmp(&b.2.origin.y) {
                        Some(std::cmp::Ordering::Equal) => {
                            a.2.origin
                                .x
                                .partial_cmp(&b.2.origin.x)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        }
                        Some(ord) => ord,
                        None => std::cmp::Ordering::Equal,
                    }
                }
                ord => ord,
            }
        });

        self.focus_order = focusable.into_iter().map(|(id, _, _)| id).collect();
        self.focus_dirty = false;
    }

    /// Find node at a point (for hit testing)
    pub fn node_at_point(&self, point: Point) -> Option<&AccessibleNode> {
        // Search from deepest to root
        self.nodes
            .values()
            .filter(|n| {
                n.bounds.contains(point) && n.role != Role::Presentation && n.role != Role::None
            })
            .min_by(|a, b| {
                // Prefer smaller (more specific) nodes
                let area_a = a.bounds.size.width * a.bounds.size.height;
                let area_b = b.bounds.size.width * b.bounds.size.height;
                area_a
                    .partial_cmp(&area_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Clear all nodes
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.focused = None;
        self.focus_order.clear();
        self.focus_dirty = true;
    }

    /// Get all nodes
    pub fn nodes(&self) -> impl Iterator<Item = &AccessibleNode> {
        self.nodes.values()
    }

    /// Get node count
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Live region announcement
#[derive(Debug, Clone)]
pub struct Announcement {
    /// Message to announce
    pub message: String,
    /// Politeness level
    pub politeness: LiveRegion,
    /// Whether to clear pending announcements
    pub clear_queue: bool,
}

impl Announcement {
    /// Create a new polite announcement
    pub fn polite(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            politeness: LiveRegion::Polite,
            clear_queue: false,
        }
    }

    /// Create a new assertive announcement
    pub fn assertive(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            politeness: LiveRegion::Assertive,
            clear_queue: false,
        }
    }

    /// Clear pending announcements before this one
    pub fn clear_queue(mut self) -> Self {
        self.clear_queue = true;
        self
    }
}

/// Accessibility context for building accessible UI
#[derive(Debug, Default)]
pub struct AccessibilityContext {
    /// The accessibility tree
    pub tree: AccessibilityTree,
    /// Pending announcements
    pub announcements: Vec<Announcement>,
    /// High contrast mode enabled
    pub high_contrast: bool,
    /// Reduced motion preference
    pub reduced_motion: bool,
}

impl AccessibilityContext {
    /// Create a new accessibility context
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable high contrast mode
    pub fn set_high_contrast(&mut self, enabled: bool) {
        self.high_contrast = enabled;
    }

    /// Enable reduced motion
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    /// Queue an announcement
    pub fn announce(&mut self, announcement: Announcement) {
        if announcement.clear_queue {
            self.announcements.clear();
        }
        self.announcements.push(announcement);
    }

    /// Announce a polite message
    pub fn announce_polite(&mut self, message: impl Into<String>) {
        self.announce(Announcement::polite(message));
    }

    /// Announce an assertive message
    pub fn announce_assertive(&mut self, message: impl Into<String>) {
        self.announce(Announcement::assertive(message));
    }

    /// Take pending announcements (for platform to process)
    pub fn take_announcements(&mut self) -> Vec<Announcement> {
        std::mem::take(&mut self.announcements)
    }

    /// Create a button node
    pub fn button(&mut self, bounds: Bounds, label: impl Into<String>) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::Button, bounds)
            .label(label)
            .focusable()
    }

    /// Create a text input node
    pub fn text_input(
        &mut self,
        bounds: Bounds,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::TextInput, bounds)
            .label(label)
            .value(value)
            .focusable()
    }

    /// Create a heading node
    pub fn heading(
        &mut self,
        bounds: Bounds,
        text: impl Into<String>,
        level: u8,
    ) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::Heading, bounds)
            .label(text)
            .heading_level(level)
    }

    /// Create a static text node
    pub fn text(&mut self, bounds: Bounds, text: impl Into<String>) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::StaticText, bounds).label(text)
    }

    /// Create a list node
    pub fn list(&mut self, bounds: Bounds, label: impl Into<String>) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::List, bounds).label(label)
    }

    /// Create a list item node
    pub fn list_item(
        &mut self,
        bounds: Bounds,
        text: impl Into<String>,
        position: u32,
        size: u32,
    ) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::ListItem, bounds)
            .label(text)
            .position(position, size)
    }

    /// Create a dialog node
    pub fn dialog(&mut self, bounds: Bounds, title: impl Into<String>) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::Dialog, bounds).label(title)
    }

    /// Create a progress bar node
    pub fn progress(
        &mut self,
        bounds: Bounds,
        label: impl Into<String>,
        min: f32,
        max: f32,
        value: f32,
    ) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::ProgressBar, bounds)
            .label(label)
            .value_range(min, max, value)
    }

    /// Create a status region node
    pub fn status(&mut self, bounds: Bounds) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::Status, bounds).live(LiveRegion::Polite)
    }

    /// Create an alert region node
    pub fn alert(&mut self, bounds: Bounds, message: impl Into<String>) -> AccessibleNode {
        let id = self.tree.next_id();
        AccessibleNode::new(id, Role::Alert, bounds)
            .label(message)
            .live(LiveRegion::Assertive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bounds() -> Bounds {
        Bounds::new(0.0, 0.0, 100.0, 50.0)
    }

    #[test]
    fn test_accessible_node_creation() {
        let node = AccessibleNode::new(1, Role::Button, test_bounds())
            .label("Click me")
            .description("A button that does something")
            .focusable();

        assert_eq!(node.id, 1);
        assert_eq!(node.role, Role::Button);
        assert_eq!(node.label, Some("Click me".to_string()));
        assert!(node.is_focusable());
    }

    #[test]
    fn test_accessible_node_states() {
        let node = AccessibleNode::new(1, Role::Checkbox, test_bounds())
            .state(State::Checked)
            .state(State::Focused);

        assert!(node.has_state(State::Checked));
        assert!(node.has_state(State::Focused));
        assert!(!node.has_state(State::Disabled));
    }

    #[test]
    fn test_accessibility_tree() {
        let mut tree = AccessibilityTree::new();

        let root_id = tree.next_id();
        let button_id = tree.next_id();

        let root = AccessibleNode::new(root_id, Role::Main, Bounds::new(0.0, 0.0, 800.0, 600.0))
            .label("Main content");
        let button = AccessibleNode::new(
            button_id,
            Role::Button,
            Bounds::new(10.0, 10.0, 100.0, 40.0),
        )
        .label("Submit")
        .focusable();

        tree.add_node(root);
        tree.add_node(button);
        tree.set_root(root_id);
        tree.add_child(root_id, button_id);

        assert_eq!(tree.len(), 2);
        assert_eq!(tree.root(), Some(root_id));
        assert!(tree.get(button_id).unwrap().is_focusable());
    }

    #[test]
    fn test_focus_management() {
        let mut tree = AccessibilityTree::new();

        let btn1_id = tree.next_id();
        let btn2_id = tree.next_id();
        let btn3_id = tree.next_id();

        tree.add_node(
            AccessibleNode::new(btn1_id, Role::Button, Bounds::new(0.0, 0.0, 100.0, 40.0))
                .label("Button 1")
                .focusable(),
        );
        tree.add_node(
            AccessibleNode::new(btn2_id, Role::Button, Bounds::new(0.0, 50.0, 100.0, 40.0))
                .label("Button 2")
                .focusable(),
        );
        tree.add_node(
            AccessibleNode::new(btn3_id, Role::Button, Bounds::new(0.0, 100.0, 100.0, 40.0))
                .label("Button 3")
                .focusable(),
        );

        // Set initial focus
        assert!(tree.set_focus(btn1_id));
        assert_eq!(tree.focused_id(), Some(btn1_id));

        // Focus next
        let next = tree.focus_next();
        assert_eq!(next, Some(btn2_id));
        assert_eq!(tree.focused_id(), Some(btn2_id));

        // Focus prev
        let prev = tree.focus_prev();
        assert_eq!(prev, Some(btn1_id));

        // Clear focus
        tree.clear_focus();
        assert_eq!(tree.focused_id(), None);
    }

    #[test]
    fn test_hit_testing() {
        let mut tree = AccessibilityTree::new();

        let container_id = tree.next_id();
        let button_id = tree.next_id();

        tree.add_node(AccessibleNode::new(
            container_id,
            Role::Group,
            Bounds::new(0.0, 0.0, 200.0, 200.0),
        ));
        tree.add_node(
            AccessibleNode::new(
                button_id,
                Role::Button,
                Bounds::new(50.0, 50.0, 100.0, 40.0),
            )
            .label("Click"),
        );

        // Point inside button should find button (more specific)
        let hit = tree.node_at_point(Point::new(75.0, 70.0));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, button_id);

        // Point outside button but inside container should find container
        let hit = tree.node_at_point(Point::new(10.0, 10.0));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, container_id);

        // Point outside both should find nothing
        let hit = tree.node_at_point(Point::new(300.0, 300.0));
        assert!(hit.is_none());
    }

    #[test]
    fn test_announcements() {
        let mut ctx = AccessibilityContext::new();

        ctx.announce_polite("Form saved");
        ctx.announce_assertive("Error occurred!");

        let announcements = ctx.take_announcements();
        assert_eq!(announcements.len(), 2);
        assert_eq!(announcements[0].politeness, LiveRegion::Polite);
        assert_eq!(announcements[1].politeness, LiveRegion::Assertive);

        // Queue should be empty after take
        assert!(ctx.take_announcements().is_empty());
    }

    #[test]
    fn test_context_helpers() {
        let mut ctx = AccessibilityContext::new();
        let bounds = Bounds::new(0.0, 0.0, 100.0, 40.0);

        let button = ctx.button(bounds, "Submit");
        assert_eq!(button.role, Role::Button);
        assert!(button.is_focusable());

        let input = ctx.text_input(bounds, "Email", "test@example.com");
        assert_eq!(input.role, Role::TextInput);
        assert_eq!(input.value, Some("test@example.com".to_string()));

        let heading = ctx.heading(bounds, "Welcome", 1);
        assert_eq!(heading.role, Role::Heading);
        assert_eq!(heading.heading_level, Some(1));

        let progress = ctx.progress(bounds, "Loading", 0.0, 100.0, 50.0);
        assert_eq!(progress.value_now, Some(50.0));
    }

    #[test]
    fn test_value_range() {
        let node = AccessibleNode::new(1, Role::Slider, test_bounds())
            .label("Volume")
            .value_range(0.0, 100.0, 75.0)
            .focusable();

        assert_eq!(node.value_min, Some(0.0));
        assert_eq!(node.value_max, Some(100.0));
        assert_eq!(node.value_now, Some(75.0));
    }

    #[test]
    fn test_grid_position() {
        let node = AccessibleNode::new(1, Role::Cell, test_bounds())
            .cell_position(2, 3)
            .cell_span(1, 2);

        assert_eq!(node.row_index, Some(2));
        assert_eq!(node.column_index, Some(3));
        assert_eq!(node.row_span, Some(1));
        assert_eq!(node.column_span, Some(2));
    }

    #[test]
    fn test_list_position() {
        let node = AccessibleNode::new(1, Role::ListItem, test_bounds())
            .label("Item 3")
            .position(3, 10);

        assert_eq!(node.pos_in_set, Some(3));
        assert_eq!(node.set_size, Some(10));
    }

    #[test]
    fn test_live_region() {
        let node = AccessibleNode::new(1, Role::Status, test_bounds())
            .live(LiveRegion::Polite)
            .atomic();

        assert_eq!(node.live_region, LiveRegion::Polite);
        assert!(node.live_atomic);
    }

    #[test]
    fn test_relationships() {
        let node = AccessibleNode::new(1, Role::TextInput, test_bounds())
            .labelled_by(100)
            .described_by(101)
            .controls(102);

        assert_eq!(node.labelled_by, Some(100));
        assert_eq!(node.described_by, Some(101));
        assert_eq!(node.controls, Some(102));
    }

    #[test]
    fn test_disabled_not_focusable() {
        let node = AccessibleNode::new(1, Role::Button, test_bounds())
            .focusable()
            .state(State::Disabled);

        assert!(!node.is_focusable());
    }

    #[test]
    fn test_hidden_not_focusable() {
        let node = AccessibleNode::new(1, Role::Button, test_bounds())
            .focusable()
            .state(State::Hidden);

        assert!(!node.is_focusable());
    }

    #[test]
    fn test_preferences() {
        let mut ctx = AccessibilityContext::new();

        assert!(!ctx.high_contrast);
        assert!(!ctx.reduced_motion);

        ctx.set_high_contrast(true);
        ctx.set_reduced_motion(true);

        assert!(ctx.high_contrast);
        assert!(ctx.reduced_motion);
    }
}
