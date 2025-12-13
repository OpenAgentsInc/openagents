//! Tailwind-like styling API

use crate::color::Hsla;
use crate::layout::Length;

/// Display mode
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Display {
    #[default]
    Flex,
    Block,
    None,
}

/// Flex direction
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Flex wrap
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Alignment for justify-content and align-items
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Align {
    #[default]
    Start,
    End,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Overflow behavior
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
}

/// Style for an element
#[derive(Clone, Debug, Default)]
pub struct Style {
    // Display & Layout
    pub display: Display,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: Align,
    pub align_items: Align,
    pub align_self: Option<Align>,
    pub flex_grow: f32,
    pub flex_shrink: f32,

    // Sizing
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,

    // Spacing
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub gap: f32,

    // Position
    pub position_type: PositionType,
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,

    // Visual
    pub background: Hsla,
    pub border_color: Hsla,
    pub border_width: f32,
    pub border_radius: f32,

    // Overflow
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
}

/// Position type
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PositionType {
    #[default]
    Relative,
    Absolute,
}

impl Style {
    /// Convert to Taffy style
    pub fn to_taffy_style(&self) -> taffy::Style {
        taffy::Style {
            display: match self.display {
                Display::Flex => taffy::Display::Flex,
                Display::Block => taffy::Display::Block,
                Display::None => taffy::Display::None,
            },
            flex_direction: match self.flex_direction {
                FlexDirection::Row => taffy::FlexDirection::Row,
                FlexDirection::Column => taffy::FlexDirection::Column,
                FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
                FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
            },
            flex_wrap: match self.flex_wrap {
                FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
                FlexWrap::Wrap => taffy::FlexWrap::Wrap,
                FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
            },
            justify_content: Some(align_to_justify(self.justify_content)),
            align_items: Some(align_to_items(self.align_items)),
            align_self: self.align_self.map(align_to_items),
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            size: taffy::Size {
                width: self.width.to_taffy_dimension(),
                height: self.height.to_taffy_dimension(),
            },
            min_size: taffy::Size {
                width: self.min_width.to_taffy_dimension(),
                height: self.min_height.to_taffy_dimension(),
            },
            max_size: taffy::Size {
                width: self.max_width.to_taffy_dimension(),
                height: self.max_height.to_taffy_dimension(),
            },
            padding: taffy::Rect {
                top: taffy::LengthPercentage::length(self.padding_top),
                right: taffy::LengthPercentage::length(self.padding_right),
                bottom: taffy::LengthPercentage::length(self.padding_bottom),
                left: taffy::LengthPercentage::length(self.padding_left),
            },
            margin: taffy::Rect {
                top: taffy::LengthPercentageAuto::length(self.margin_top),
                right: taffy::LengthPercentageAuto::length(self.margin_right),
                bottom: taffy::LengthPercentageAuto::length(self.margin_bottom),
                left: taffy::LengthPercentageAuto::length(self.margin_left),
            },
            gap: taffy::Size {
                width: taffy::LengthPercentage::length(self.gap),
                height: taffy::LengthPercentage::length(self.gap),
            },
            position: match self.position_type {
                PositionType::Relative => taffy::Position::Relative,
                PositionType::Absolute => taffy::Position::Absolute,
            },
            inset: taffy::Rect {
                top: self.top.to_taffy_length_percentage_auto(),
                right: self.right.to_taffy_length_percentage_auto(),
                bottom: self.bottom.to_taffy_length_percentage_auto(),
                left: self.left.to_taffy_length_percentage_auto(),
            },
            ..Default::default()
        }
    }
}

fn align_to_justify(align: Align) -> taffy::JustifyContent {
    match align {
        Align::Start => taffy::JustifyContent::Start,
        Align::End => taffy::JustifyContent::End,
        Align::Center => taffy::JustifyContent::Center,
        Align::Stretch => taffy::JustifyContent::Stretch,
        Align::SpaceBetween => taffy::JustifyContent::SpaceBetween,
        Align::SpaceAround => taffy::JustifyContent::SpaceAround,
        Align::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
    }
}

fn align_to_items(align: Align) -> taffy::AlignItems {
    match align {
        Align::Start => taffy::AlignItems::Start,
        Align::End => taffy::AlignItems::End,
        Align::Center => taffy::AlignItems::Center,
        Align::Stretch => taffy::AlignItems::Stretch,
        Align::SpaceBetween => taffy::AlignItems::Start, // No direct equivalent
        Align::SpaceAround => taffy::AlignItems::Start,
        Align::SpaceEvenly => taffy::AlignItems::Start,
    }
}

/// Trait for elements that can be styled with fluent API
pub trait Styled: Sized {
    /// Get mutable reference to style
    fn style(&mut self) -> &mut Style;

    // Display
    fn flex(mut self) -> Self {
        self.style().display = Display::Flex;
        self
    }

    fn block(mut self) -> Self {
        self.style().display = Display::Block;
        self
    }

    fn hidden(mut self) -> Self {
        self.style().display = Display::None;
        self
    }

    // Flex direction
    fn flex_row(mut self) -> Self {
        self.style().flex_direction = FlexDirection::Row;
        self
    }

    fn flex_col(mut self) -> Self {
        self.style().flex_direction = FlexDirection::Column;
        self
    }

    // Flex properties
    fn flex_grow(mut self, value: f32) -> Self {
        self.style().flex_grow = value;
        self
    }

    fn flex_shrink(mut self, value: f32) -> Self {
        self.style().flex_shrink = value;
        self
    }

    fn flex_1(mut self) -> Self {
        self.style().flex_grow = 1.0;
        self.style().flex_shrink = 1.0;
        self
    }

    // Alignment
    fn justify_start(mut self) -> Self {
        self.style().justify_content = Align::Start;
        self
    }

    fn justify_center(mut self) -> Self {
        self.style().justify_content = Align::Center;
        self
    }

    fn justify_end(mut self) -> Self {
        self.style().justify_content = Align::End;
        self
    }

    fn justify_between(mut self) -> Self {
        self.style().justify_content = Align::SpaceBetween;
        self
    }

    fn items_start(mut self) -> Self {
        self.style().align_items = Align::Start;
        self
    }

    fn items_center(mut self) -> Self {
        self.style().align_items = Align::Center;
        self
    }

    fn items_end(mut self) -> Self {
        self.style().align_items = Align::End;
        self
    }

    fn items_stretch(mut self) -> Self {
        self.style().align_items = Align::Stretch;
        self
    }

    // Sizing
    fn w(mut self, width: impl Into<Length>) -> Self {
        self.style().width = width.into();
        self
    }

    fn h(mut self, height: impl Into<Length>) -> Self {
        self.style().height = height.into();
        self
    }

    fn size(mut self, size: impl Into<Length> + Copy) -> Self {
        self.style().width = size.into();
        self.style().height = size.into();
        self
    }

    fn w_full(mut self) -> Self {
        self.style().width = Length::Percent(100.0);
        self
    }

    fn h_full(mut self) -> Self {
        self.style().height = Length::Percent(100.0);
        self
    }

    fn min_w(mut self, width: impl Into<Length>) -> Self {
        self.style().min_width = width.into();
        self
    }

    fn min_h(mut self, height: impl Into<Length>) -> Self {
        self.style().min_height = height.into();
        self
    }

    fn max_w(mut self, width: impl Into<Length>) -> Self {
        self.style().max_width = width.into();
        self
    }

    fn max_h(mut self, height: impl Into<Length>) -> Self {
        self.style().max_height = height.into();
        self
    }

    // Padding
    fn p(mut self, padding: f32) -> Self {
        self.style().padding_top = padding;
        self.style().padding_right = padding;
        self.style().padding_bottom = padding;
        self.style().padding_left = padding;
        self
    }

    fn px(mut self, padding: f32) -> Self {
        self.style().padding_left = padding;
        self.style().padding_right = padding;
        self
    }

    fn py(mut self, padding: f32) -> Self {
        self.style().padding_top = padding;
        self.style().padding_bottom = padding;
        self
    }

    fn pt(mut self, padding: f32) -> Self {
        self.style().padding_top = padding;
        self
    }

    fn pr(mut self, padding: f32) -> Self {
        self.style().padding_right = padding;
        self
    }

    fn pb(mut self, padding: f32) -> Self {
        self.style().padding_bottom = padding;
        self
    }

    fn pl(mut self, padding: f32) -> Self {
        self.style().padding_left = padding;
        self
    }

    // Margin
    fn m(mut self, margin: f32) -> Self {
        self.style().margin_top = margin;
        self.style().margin_right = margin;
        self.style().margin_bottom = margin;
        self.style().margin_left = margin;
        self
    }

    fn mx(mut self, margin: f32) -> Self {
        self.style().margin_left = margin;
        self.style().margin_right = margin;
        self
    }

    fn my(mut self, margin: f32) -> Self {
        self.style().margin_top = margin;
        self.style().margin_bottom = margin;
        self
    }

    fn mt(mut self, margin: f32) -> Self {
        self.style().margin_top = margin;
        self
    }

    fn mr(mut self, margin: f32) -> Self {
        self.style().margin_right = margin;
        self
    }

    fn mb(mut self, margin: f32) -> Self {
        self.style().margin_bottom = margin;
        self
    }

    fn ml(mut self, margin: f32) -> Self {
        self.style().margin_left = margin;
        self
    }

    // Gap
    fn gap(mut self, gap: f32) -> Self {
        self.style().gap = gap;
        self
    }

    // Position
    fn relative(mut self) -> Self {
        self.style().position_type = PositionType::Relative;
        self
    }

    fn absolute(mut self) -> Self {
        self.style().position_type = PositionType::Absolute;
        self
    }

    fn top(mut self, value: impl Into<Length>) -> Self {
        self.style().top = value.into();
        self
    }

    fn right(mut self, value: impl Into<Length>) -> Self {
        self.style().right = value.into();
        self
    }

    fn bottom(mut self, value: impl Into<Length>) -> Self {
        self.style().bottom = value.into();
        self
    }

    fn left(mut self, value: impl Into<Length>) -> Self {
        self.style().left = value.into();
        self
    }

    // Visual
    fn bg(mut self, color: impl Into<Hsla>) -> Self {
        self.style().background = color.into();
        self
    }

    fn border(mut self, width: f32) -> Self {
        self.style().border_width = width;
        self
    }

    fn border_color(mut self, color: impl Into<Hsla>) -> Self {
        self.style().border_color = color.into();
        self
    }

    fn rounded(mut self, radius: f32) -> Self {
        self.style().border_radius = radius;
        self
    }

    // Overflow
    fn overflow_hidden(mut self) -> Self {
        self.style().overflow_x = Overflow::Hidden;
        self.style().overflow_y = Overflow::Hidden;
        self
    }

    fn overflow_scroll(mut self) -> Self {
        self.style().overflow_x = Overflow::Scroll;
        self.style().overflow_y = Overflow::Scroll;
        self
    }
}

// Implement Into<Length> for f32 (pixels)
impl From<f32> for Length {
    fn from(px: f32) -> Self {
        Length::Px(px)
    }
}
