use taffy::prelude::{
    AlignItems, Dimension, Display, FlexDirection, JustifyContent, LengthPercentage,
    LengthPercentageAuto, Rect,
};
use taffy::Overflow;

use crate::Hsla;

use super::StyleRefinement;

pub trait Styled: Sized {
    fn style(&mut self) -> &mut StyleRefinement;

    fn bg(mut self, color: Hsla) -> Self {
        self.style().background = Some(color);
        self
    }

    fn border(mut self, color: Hsla, width: f32) -> Self {
        self.style().border_color = Some(color);
        self.style().border_width = Some(width);
        self
    }

    fn border_color(mut self, color: Hsla) -> Self {
        self.style().border_color = Some(color);
        self
    }

    fn border_width(mut self, width: f32) -> Self {
        self.style().border_width = Some(width);
        self
    }

    fn text_color(mut self, color: Hsla) -> Self {
        self.style().text_color = Some(color);
        self
    }

    fn text_size(mut self, size: f32) -> Self {
        self.style().font_size = Some(size);
        self
    }

    fn flex(mut self) -> Self {
        self.style().layout.display = Display::Flex;
        self
    }

    fn flex_row(mut self) -> Self {
        self.style().layout.flex_direction = FlexDirection::Row;
        self
    }

    fn flex_col(mut self) -> Self {
        self.style().layout.flex_direction = FlexDirection::Column;
        self
    }

    fn flex_grow(mut self, value: f32) -> Self {
        self.style().layout.flex_grow = value;
        self
    }

    fn flex_shrink(mut self, value: f32) -> Self {
        self.style().layout.flex_shrink = value;
        self
    }

    fn flex_1(mut self) -> Self {
        self.style().layout.flex_grow = 1.0;
        self.style().layout.flex_shrink = 1.0;
        self
    }

    fn flex_none(mut self) -> Self {
        self.style().layout.flex_grow = 0.0;
        self.style().layout.flex_shrink = 0.0;
        self
    }

    fn items_start(mut self) -> Self {
        self.style().layout.align_items = Some(AlignItems::FlexStart);
        self
    }

    fn items_center(mut self) -> Self {
        self.style().layout.align_items = Some(AlignItems::Center);
        self
    }

    fn items_end(mut self) -> Self {
        self.style().layout.align_items = Some(AlignItems::FlexEnd);
        self
    }

    fn justify_start(mut self) -> Self {
        self.style().layout.justify_content = Some(JustifyContent::FlexStart);
        self
    }

    fn justify_center(mut self) -> Self {
        self.style().layout.justify_content = Some(JustifyContent::Center);
        self
    }

    fn justify_end(mut self) -> Self {
        self.style().layout.justify_content = Some(JustifyContent::FlexEnd);
        self
    }

    fn justify_between(mut self) -> Self {
        self.style().layout.justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    fn gap(mut self, value: f32) -> Self {
        let gap = LengthPercentage::length(value);
        self.style().layout.gap = taffy::Size {
            width: gap,
            height: gap,
        };
        self
    }

    fn w(mut self, value: f32) -> Self {
        self.style().layout.width = Dimension::length(value);
        self
    }

    fn h(mut self, value: f32) -> Self {
        self.style().layout.height = Dimension::length(value);
        self
    }

    fn w_full(mut self) -> Self {
        self.style().layout.width = Dimension::percent(1.0);
        self
    }

    fn h_full(mut self) -> Self {
        self.style().layout.height = Dimension::percent(1.0);
        self
    }

    fn min_w_0(mut self) -> Self {
        self.style().layout.min_width = Dimension::length(0.0);
        self
    }

    fn min_h_0(mut self) -> Self {
        self.style().layout.min_height = Dimension::length(0.0);
        self
    }

    fn p(mut self, value: f32) -> Self {
        let pad = LengthPercentage::length(value);
        self.style().layout.padding = Rect {
            left: pad,
            right: pad,
            top: pad,
            bottom: pad,
        };
        self
    }

    fn px(mut self, value: f32) -> Self {
        let pad = LengthPercentage::length(value);
        let padding = &mut self.style().layout.padding;
        padding.left = pad;
        padding.right = pad;
        self
    }

    fn py(mut self, value: f32) -> Self {
        let pad = LengthPercentage::length(value);
        let padding = &mut self.style().layout.padding;
        padding.top = pad;
        padding.bottom = pad;
        self
    }

    fn m(mut self, value: f32) -> Self {
        let margin = LengthPercentageAuto::length(value);
        self.style().layout.margin = Rect {
            left: margin,
            right: margin,
            top: margin,
            bottom: margin,
        };
        self
    }

    fn mx(mut self, value: f32) -> Self {
        let margin = LengthPercentageAuto::length(value);
        let margin_ref = &mut self.style().layout.margin;
        margin_ref.left = margin;
        margin_ref.right = margin;
        self
    }

    fn my(mut self, value: f32) -> Self {
        let margin = LengthPercentageAuto::length(value);
        let margin_ref = &mut self.style().layout.margin;
        margin_ref.top = margin;
        margin_ref.bottom = margin;
        self
    }

    fn overflow_hidden(mut self) -> Self {
        self.style().layout.overflow = taffy::Point {
            x: Overflow::Hidden,
            y: Overflow::Hidden,
        };
        self
    }

    fn overflow_x_hidden(mut self) -> Self {
        self.style().layout.overflow.x = Overflow::Hidden;
        self
    }

    fn overflow_y_hidden(mut self) -> Self {
        self.style().layout.overflow.y = Overflow::Hidden;
        self
    }

    fn overflow_x_auto(mut self) -> Self {
        self.style().layout.overflow.x = Overflow::Scroll;
        self
    }

    fn overflow_y_auto(mut self) -> Self {
        self.style().layout.overflow.y = Overflow::Scroll;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styled::{Styled, div};
    use crate::theme;

    #[test]
    fn test_styled_builder_sets_fields() {
        let div = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(theme::bg::SURFACE)
            .border(theme::border::DEFAULT, 2.0)
            .w(120.0)
            .h(48.0)
            .p(6.0);

        assert_eq!(div.style.layout.display, Display::Flex);
        assert_eq!(div.style.layout.flex_direction, FlexDirection::Row);
        assert_eq!(div.style.layout.align_items, Some(AlignItems::Center));
        assert_eq!(
            div.style.layout.justify_content,
            Some(JustifyContent::SpaceBetween)
        );
        assert_eq!(div.style.background, Some(theme::bg::SURFACE));
        assert_eq!(div.style.border_color, Some(theme::border::DEFAULT));
        assert_eq!(div.style.border_width, Some(2.0));
        assert_eq!(div.style.layout.width, Dimension::length(120.0));
        assert_eq!(div.style.layout.height, Dimension::length(48.0));
        assert_eq!(
            div.style.layout.padding,
            Rect {
                left: LengthPercentage::length(6.0),
                right: LengthPercentage::length(6.0),
                top: LengthPercentage::length(6.0),
                bottom: LengthPercentage::length(6.0),
            }
        );
    }
}
