//! Layout engine using Taffy for CSS Flexbox/Grid

use slotmap::{DefaultKey, SlotMap};
use std::collections::HashMap;
use taffy::prelude::*;

/// Unique identifier for a layout node
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayoutId(pub DefaultKey);

/// 2D size
#[derive(Clone, Copy, Debug, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub const ZERO: Self = Self::new(0.0, 0.0);
}

/// 2D point
#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const ZERO: Self = Self::new(0.0, 0.0);
}

/// Bounding rectangle
#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub origin: Point,
    pub size: Size,
}

impl Bounds {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }

    pub fn y(&self) -> f32 {
        self.origin.y
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.x <= self.right()
            && point.y >= self.origin.y
            && point.y <= self.bottom()
    }
}

/// Length value for styling (pixels, percent, auto)
#[derive(Clone, Copy, Debug)]
pub enum Length {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent
    Percent(f32),
    /// Auto sizing
    Auto,
}

impl Default for Length {
    fn default() -> Self {
        Length::Auto
    }
}

impl Length {
    pub fn to_taffy_dimension(self) -> taffy::Dimension {
        match self {
            Length::Px(px) => taffy::Dimension::length(px),
            Length::Percent(pct) => taffy::Dimension::percent(pct / 100.0),
            Length::Auto => taffy::Dimension::auto(),
        }
    }

    pub fn to_taffy_length_percentage(self) -> taffy::LengthPercentage {
        match self {
            Length::Px(px) => taffy::LengthPercentage::length(px),
            Length::Percent(pct) => taffy::LengthPercentage::percent(pct / 100.0),
            Length::Auto => taffy::LengthPercentage::length(0.0),
        }
    }

    pub fn to_taffy_length_percentage_auto(self) -> taffy::LengthPercentageAuto {
        match self {
            Length::Px(px) => taffy::LengthPercentageAuto::length(px),
            Length::Percent(pct) => taffy::LengthPercentageAuto::percent(pct / 100.0),
            Length::Auto => taffy::LengthPercentageAuto::auto(),
        }
    }
}

/// Helper to create pixel length
pub fn px(value: f32) -> Length {
    Length::Px(value)
}

/// Helper to create percentage length
pub fn pct(value: f32) -> Length {
    Length::Percent(value)
}

/// Layout engine wrapping Taffy
pub struct LayoutEngine {
    taffy: TaffyTree<()>,
    nodes: SlotMap<DefaultKey, NodeId>,
    node_to_layout: HashMap<NodeId, LayoutId>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: SlotMap::new(),
            node_to_layout: HashMap::new(),
        }
    }

    /// Request layout for an element with the given style
    pub fn request_layout(&mut self, style: &crate::styled::Style) -> LayoutId {
        let taffy_style = style.to_taffy_style();
        let node = self.taffy.new_leaf(taffy_style).unwrap();
        let key = self.nodes.insert(node);
        let layout_id = LayoutId(key);
        self.node_to_layout.insert(node, layout_id);
        layout_id
    }

    /// Request layout with a pre-measured size (for text elements)
    pub fn request_measured_layout(&mut self, width: f32, height: f32) -> LayoutId {
        let style = taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::length(width),
                height: taffy::Dimension::length(height),
            },
            ..Default::default()
        };
        let node = self.taffy.new_leaf(style).unwrap();
        let key = self.nodes.insert(node);
        let layout_id = LayoutId(key);
        self.node_to_layout.insert(node, layout_id);
        layout_id
    }

    /// Request layout for an element with children
    pub fn request_layout_with_children(
        &mut self,
        style: &crate::styled::Style,
        children: &[LayoutId],
    ) -> LayoutId {
        let taffy_style = style.to_taffy_style();
        let child_nodes: Vec<NodeId> = children
            .iter()
            .map(|id| self.nodes[id.0])
            .collect();
        let node = self.taffy.new_with_children(taffy_style, &child_nodes).unwrap();
        let key = self.nodes.insert(node);
        let layout_id = LayoutId(key);
        self.node_to_layout.insert(node, layout_id);
        layout_id
    }

    /// Compute layout for the tree starting at root
    pub fn compute_layout(&mut self, root: LayoutId, available_size: Size) {
        let root_node = self.nodes[root.0];
        self.taffy
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(available_size.width),
                    height: AvailableSpace::Definite(available_size.height),
                },
            )
            .unwrap();
    }

    /// Get the computed bounds for a layout node
    pub fn layout(&self, id: LayoutId) -> Bounds {
        let node = self.nodes[id.0];
        let layout = self.taffy.layout(node).unwrap();
        Bounds {
            origin: Point::new(layout.location.x, layout.location.y),
            size: Size::new(layout.size.width, layout.size.height),
        }
    }

    /// Clear all layout nodes for the next frame
    pub fn clear(&mut self) {
        self.taffy.clear();
        self.nodes.clear();
        self.node_to_layout.clear();
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}
