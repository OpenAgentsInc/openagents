//! Layout engine using Taffy for CSS Flexbox layout.

use crate::geometry::{Bounds, Size};
use slotmap::{SlotMap, new_key_type};
use taffy::Overflow;
use taffy::prelude::*;

new_key_type! {
    /// Identifier for a layout node.
    pub struct LayoutId;
}

/// Style for layout computation.
#[derive(Clone, Debug)]
pub struct LayoutStyle {
    /// Display mode (flex, block, none)
    pub display: Display,
    /// Position type (relative, absolute)
    pub position: Position,
    /// Flex direction
    pub flex_direction: FlexDirection,
    /// Flex wrap
    pub flex_wrap: FlexWrap,
    /// Justify content (main axis)
    pub justify_content: Option<JustifyContent>,
    /// Align items (cross axis)
    pub align_items: Option<AlignItems>,
    /// Align self (override parent's align_items)
    pub align_self: Option<AlignSelf>,
    /// Gap between items
    pub gap: taffy::Size<LengthPercentage>,
    /// Width
    pub width: Dimension,
    /// Height
    pub height: Dimension,
    /// Min width
    pub min_width: Dimension,
    /// Max width
    pub max_width: Dimension,
    /// Min height
    pub min_height: Dimension,
    /// Max height
    pub max_height: Dimension,
    /// Padding
    pub padding: Rect<LengthPercentage>,
    /// Margin
    pub margin: Rect<LengthPercentageAuto>,
    /// Flex grow
    pub flex_grow: f32,
    /// Flex shrink
    pub flex_shrink: f32,
    /// Flex basis
    pub flex_basis: Dimension,
    /// Overflow behavior
    pub overflow: taffy::Point<Overflow>,
    /// Inset (for absolute positioning)
    pub inset: Rect<LengthPercentageAuto>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            position: Position::Relative,
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: None,
            align_items: None,
            align_self: None,
            gap: taffy::Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(0.0),
            },
            width: Dimension::auto(),
            height: Dimension::auto(),
            min_width: Dimension::auto(),
            max_width: Dimension::auto(),
            min_height: Dimension::auto(),
            max_height: Dimension::auto(),
            padding: Rect {
                left: LengthPercentage::length(0.0),
                right: LengthPercentage::length(0.0),
                top: LengthPercentage::length(0.0),
                bottom: LengthPercentage::length(0.0),
            },
            margin: Rect {
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0),
                top: LengthPercentageAuto::length(0.0),
                bottom: LengthPercentageAuto::length(0.0),
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::auto(),
            overflow: taffy::Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            },
            inset: Rect {
                left: LengthPercentageAuto::auto(),
                right: LengthPercentageAuto::auto(),
                top: LengthPercentageAuto::auto(),
                bottom: LengthPercentageAuto::auto(),
            },
        }
    }
}

impl LayoutStyle {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<&LayoutStyle> for Style {
    fn from(s: &LayoutStyle) -> Self {
        Style {
            display: s.display,
            position: s.position,
            flex_direction: s.flex_direction,
            flex_wrap: s.flex_wrap,
            justify_content: s.justify_content,
            align_items: s.align_items,
            align_self: s.align_self,
            gap: s.gap,
            size: taffy::Size {
                width: s.width,
                height: s.height,
            },
            min_size: taffy::Size {
                width: s.min_width,
                height: s.min_height,
            },
            max_size: taffy::Size {
                width: s.max_width,
                height: s.max_height,
            },
            padding: s.padding,
            margin: s.margin,
            flex_grow: s.flex_grow,
            flex_shrink: s.flex_shrink,
            flex_basis: s.flex_basis,
            overflow: s.overflow,
            inset: s.inset,
            ..Default::default()
        }
    }
}

/// Layout engine using Taffy.
pub struct LayoutEngine {
    taffy: TaffyTree<()>,
    nodes: SlotMap<LayoutId, NodeId>,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: SlotMap::with_key(),
        }
    }

    /// Clear all nodes for a new frame.
    pub fn clear(&mut self) {
        self.taffy.clear();
        self.nodes.clear();
    }

    /// Request a layout node with the given style and children.
    pub fn request_layout(&mut self, style: &LayoutStyle, children: &[LayoutId]) -> LayoutId {
        let taffy_children: Vec<NodeId> = children
            .iter()
            .filter_map(|id| self.nodes.get(*id).copied())
            .collect();

        let taffy_style: Style = style.into();
        let node_id = self
            .taffy
            .new_with_children(taffy_style, &taffy_children)
            .expect("Failed to create layout node");

        self.nodes.insert(node_id)
    }

    /// Request a leaf node (no children, explicit size).
    pub fn request_leaf(&mut self, style: &LayoutStyle) -> LayoutId {
        self.request_layout(style, &[])
    }

    /// Request a measured leaf node (size computed by callback).
    pub fn request_measured<F>(&mut self, style: &LayoutStyle, _measure: F) -> LayoutId
    where
        F: Fn(
                taffy::Size<Option<f32>>,
                taffy::Size<AvailableSpace>,
                NodeId,
                Option<&mut ()>,
                &Style,
            ) -> taffy::Size<f32>
            + Send
            + Sync
            + 'static,
    {
        let taffy_style: Style = style.into();
        let node_id = self
            .taffy
            .new_leaf_with_context(taffy_style, ())
            .expect("Failed to create measured node");

        // Set measure function
        self.taffy
            .set_node_context(node_id, Some(()))
            .expect("Failed to set context");

        self.nodes.insert(node_id)
    }

    /// Compute layout for the tree rooted at the given node.
    pub fn compute_layout(&mut self, root: LayoutId, available_space: Size) {
        if let Some(&node_id) = self.nodes.get(root) {
            self.taffy
                .compute_layout(
                    node_id,
                    taffy::Size {
                        width: AvailableSpace::Definite(available_space.width),
                        height: AvailableSpace::Definite(available_space.height),
                    },
                )
                .expect("Failed to compute layout");
        }
    }

    /// Get the computed bounds for a layout node.
    pub fn layout(&self, id: LayoutId) -> Bounds {
        if let Some(&node_id) = self.nodes.get(id) {
            if let Ok(layout) = self.taffy.layout(node_id) {
                return Bounds::new(
                    layout.location.x,
                    layout.location.y,
                    layout.size.width,
                    layout.size.height,
                );
            }
        }
        Bounds::ZERO
    }

    /// Get the size of a layout node.
    pub fn size(&self, id: LayoutId) -> Size {
        let bounds = self.layout(id);
        bounds.size
    }
}

// Helper functions for creating Taffy dimensions

/// Create a length in pixels.
pub fn px(value: f32) -> Dimension {
    Dimension::length(value)
}

/// Create a percentage value.
pub fn pct(value: f32) -> Dimension {
    Dimension::percent(value / 100.0)
}

/// Auto dimension.
pub fn auto() -> Dimension {
    Dimension::auto()
}

/// Create a length percentage in pixels.
pub fn length(value: f32) -> LengthPercentage {
    LengthPercentage::length(value)
}

/// Create a length percentage auto in pixels.
pub fn length_auto(value: f32) -> LengthPercentageAuto {
    LengthPercentageAuto::length(value)
}

/// Zero length percentage.
pub fn zero() -> LengthPercentage {
    LengthPercentage::length(0.0)
}

/// Relative size (1.0 = 100%).
pub fn relative(factor: f32) -> Dimension {
    Dimension::percent(factor)
}
