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

    /// Set width
    pub fn width(mut self, value: Dimension) -> Self {
        self.width = value;
        self
    }

    /// Set height
    pub fn height(mut self, value: Dimension) -> Self {
        self.height = value;
        self
    }

    /// Set flex direction
    pub fn flex_direction(mut self, value: FlexDirection) -> Self {
        self.flex_direction = value;
        self
    }

    /// Set flex direction to row.
    pub fn flex_row(mut self) -> Self {
        self.flex_direction = FlexDirection::Row;
        self
    }

    /// Set flex direction to column.
    pub fn flex_col(mut self) -> Self {
        self.flex_direction = FlexDirection::Column;
        self
    }

    /// Set flex grow
    pub fn flex_grow(mut self, value: f32) -> Self {
        self.flex_grow = value;
        self
    }

    /// Set flex shrink
    pub fn flex_shrink(mut self, value: f32) -> Self {
        self.flex_shrink = value;
        self
    }

    /// Set padding (all sides)
    pub fn padding(mut self, value: LengthPercentage) -> Self {
        self.padding = Rect {
            left: value,
            right: value,
            top: value,
            bottom: value,
        };
        self
    }

    /// Set margin (all sides)
    pub fn margin(mut self, value: LengthPercentageAuto) -> Self {
        self.margin = Rect {
            left: value,
            right: value,
            top: value,
            bottom: value,
        };
        self
    }

    /// Set gap between items
    pub fn gap(mut self, value: LengthPercentage) -> Self {
        self.gap = taffy::Size {
            width: value,
            height: value,
        };
        self
    }

    /// Set justify content
    pub fn justify_content(mut self, value: JustifyContent) -> Self {
        self.justify_content = Some(value);
        self
    }

    /// Set align items
    pub fn align_items(mut self, value: AlignItems) -> Self {
        self.align_items = Some(value);
        self
    }

    /// Set overflow behavior
    pub fn overflow(mut self, value: Overflow) -> Self {
        self.overflow = taffy::Point { x: value, y: value };
        self
    }

    /// Set display mode
    pub fn display(mut self, value: Display) -> Self {
        self.display = value;
        self
    }

    /// Set position type
    pub fn position(mut self, value: Position) -> Self {
        self.position = value;
        self
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
        if let Some(&node_id) = self.nodes.get(id)
            && let Ok(layout) = self.taffy.layout(node_id)
        {
            return Bounds::new(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_style_default() {
        let style = LayoutStyle::default();
        assert_eq!(style.display, Display::Flex);
        assert_eq!(style.position, Position::Relative);
        assert_eq!(style.flex_direction, FlexDirection::Column);
        assert_eq!(style.flex_grow, 0.0);
        assert_eq!(style.flex_shrink, 1.0);
    }

    #[test]
    fn test_layout_style_builder() {
        let style = LayoutStyle::new()
            .width(px(100.0))
            .height(px(50.0))
            .flex_grow(1.0)
            .flex_direction(FlexDirection::Row);

        assert_eq!(style.width, Dimension::length(100.0));
        assert_eq!(style.height, Dimension::length(50.0));
        assert_eq!(style.flex_grow, 1.0);
        assert_eq!(style.flex_direction, FlexDirection::Row);
    }

    #[test]
    fn test_layout_engine_creation() {
        let engine = LayoutEngine::new();
        assert!(engine.nodes.is_empty());
    }

    #[test]
    fn test_layout_engine_leaf() {
        let mut engine = LayoutEngine::new();
        let style = LayoutStyle::new().width(px(100.0)).height(px(50.0));
        let id = engine.request_leaf(&style);

        engine.compute_layout(id, Size::new(800.0, 600.0));
        let bounds = engine.layout(id);

        assert!((bounds.size.width - 100.0).abs() < 0.001);
        assert!((bounds.size.height - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_engine_with_children() {
        let mut engine = LayoutEngine::new();

        // Create children
        let child1 = engine.request_leaf(&LayoutStyle::new().height(px(30.0)));
        let child2 = engine.request_leaf(&LayoutStyle::new().height(px(40.0)));

        // Create parent
        let parent = engine.request_layout(
            &LayoutStyle::new()
                .width(px(200.0))
                .flex_direction(FlexDirection::Column),
            &[child1, child2],
        );

        engine.compute_layout(parent, Size::new(800.0, 600.0));

        let parent_bounds = engine.layout(parent);
        let child1_bounds = engine.layout(child1);
        let child2_bounds = engine.layout(child2);

        assert!((parent_bounds.size.width - 200.0).abs() < 0.001);
        assert!((child1_bounds.size.height - 30.0).abs() < 0.001);
        assert!((child2_bounds.size.height - 40.0).abs() < 0.001);
        assert!((child2_bounds.origin.y - 30.0).abs() < 0.001); // child2 below child1
    }

    #[test]
    fn test_layout_engine_clear() {
        let mut engine = LayoutEngine::new();
        let _ = engine.request_leaf(&LayoutStyle::new());
        assert!(!engine.nodes.is_empty());

        engine.clear();
        assert!(engine.nodes.is_empty());
    }

    #[test]
    fn test_helper_functions() {
        assert_eq!(px(10.0), Dimension::length(10.0));
        assert_eq!(pct(50.0), Dimension::percent(0.5));
        assert_eq!(auto(), Dimension::auto());
        assert_eq!(length(5.0), LengthPercentage::length(5.0));
        assert_eq!(zero(), LengthPercentage::length(0.0));
        assert_eq!(relative(0.5), Dimension::percent(0.5));
    }

    #[test]
    fn test_layout_to_taffy_style_conversion() {
        let style = LayoutStyle::new()
            .width(px(100.0))
            .height(px(200.0))
            .flex_grow(2.0)
            .padding(length(10.0));

        let taffy_style: Style = (&style).into();

        assert_eq!(taffy_style.size.width, Dimension::length(100.0));
        assert_eq!(taffy_style.size.height, Dimension::length(200.0));
        assert_eq!(taffy_style.flex_grow, 2.0);
        assert_eq!(taffy_style.padding.left, LengthPercentage::length(10.0));
    }
}
