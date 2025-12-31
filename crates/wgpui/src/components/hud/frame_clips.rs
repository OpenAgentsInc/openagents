#[derive(Clone, Debug)]
pub enum CssSize {
    Px(f32),
    Raw(String),
}

impl CssSize {
    fn to_css(&self) -> String {
        match self {
            CssSize::Px(value) => format!("{value}px"),
            CssSize::Raw(value) => value.clone(),
        }
    }
}

impl From<f32> for CssSize {
    fn from(value: f32) -> Self {
        CssSize::Px(value)
    }
}

impl From<i32> for CssSize {
    fn from(value: i32) -> Self {
        CssSize::Px(value as f32)
    }
}

impl From<&str> for CssSize {
    fn from(value: &str) -> Self {
        CssSize::Raw(value.to_string())
    }
}

impl From<String> for CssSize {
    fn from(value: String) -> Self {
        CssSize::Raw(value)
    }
}

#[derive(Clone, Debug)]
pub struct StyleFrameClipOctagonProps {
    pub square_size: CssSize,
    pub left_top: bool,
    pub left_bottom: bool,
    pub right_top: bool,
    pub right_bottom: bool,
}

impl Default for StyleFrameClipOctagonProps {
    fn default() -> Self {
        Self {
            square_size: CssSize::Px(16.0),
            left_top: true,
            left_bottom: true,
            right_top: true,
            right_bottom: true,
        }
    }
}

impl StyleFrameClipOctagonProps {
    pub fn square_size(mut self, size: impl Into<CssSize>) -> Self {
        self.square_size = size.into();
        self
    }

    pub fn left_top(mut self, enabled: bool) -> Self {
        self.left_top = enabled;
        self
    }

    pub fn left_bottom(mut self, enabled: bool) -> Self {
        self.left_bottom = enabled;
        self
    }

    pub fn right_top(mut self, enabled: bool) -> Self {
        self.right_top = enabled;
        self
    }

    pub fn right_bottom(mut self, enabled: bool) -> Self {
        self.right_bottom = enabled;
        self
    }
}

pub fn style_frame_clip_octagon(props: StyleFrameClipOctagonProps) -> String {
    let ss = props.square_size.to_css();

    let left_top_points = if props.left_top {
        format!("0 {ss},\n{ss} 0,")
    } else {
        "0 0,".to_string()
    };

    let right_top_points = if props.right_top {
        format!("calc(100% - {ss}) 0,\n100% {ss},")
    } else {
        "100% 0,".to_string()
    };

    let right_bottom_points = if props.right_bottom {
        format!("100% calc(100% - {ss}),\ncalc(100% - {ss}) 100%,")
    } else {
        "100% 100%,".to_string()
    };

    let left_bottom_points = if props.left_bottom {
        format!("{ss} 100%,\n0 calc(100% - {ss})")
    } else {
        "0 100%".to_string()
    };

    format!(
        "polygon(\n{left_top_points}\n{right_top_points}\n{right_bottom_points}\n{left_bottom_points}\n)"
    )
}

#[derive(Clone, Debug)]
pub struct StyleFrameClipKranoxProps {
    pub square_size: CssSize,
    pub padding: CssSize,
    pub stroke_width: CssSize,
    pub small_line_length: CssSize,
    pub large_line_length: CssSize,
}

impl Default for StyleFrameClipKranoxProps {
    fn default() -> Self {
        Self {
            square_size: CssSize::Px(16.0),
            stroke_width: CssSize::Px(1.0),
            small_line_length: CssSize::Px(16.0),
            large_line_length: CssSize::Px(64.0),
            padding: CssSize::Px(0.0),
        }
    }
}

impl StyleFrameClipKranoxProps {
    pub fn square_size(mut self, size: impl Into<CssSize>) -> Self {
        self.square_size = size.into();
        self
    }

    pub fn padding(mut self, padding: impl Into<CssSize>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn stroke_width(mut self, width: impl Into<CssSize>) -> Self {
        self.stroke_width = width.into();
        self
    }

    pub fn small_line_length(mut self, length: impl Into<CssSize>) -> Self {
        self.small_line_length = length.into();
        self
    }

    pub fn large_line_length(mut self, length: impl Into<CssSize>) -> Self {
        self.large_line_length = length.into();
        self
    }
}

pub fn style_frame_clip_kranox(props: StyleFrameClipKranoxProps) -> String {
    let p = props.padding.to_css();
    let ss = props.square_size.to_css();
    let so = format!("calc({} / 2)", props.stroke_width.to_css());
    let sll = props.small_line_length.to_css();
    let lll = props.large_line_length.to_css();

    let points = vec![
        // Left-bottom.
        (
            format!("{so} + {p} + calc({ss} * 2)"),
            format!("100% - calc({so} + {p})"),
        ),
        (
            format!("{so} + {p} + {ss}"),
            format!("100% - calc({so} + {p} + {ss})"),
        ),
        // Left.
        (
            format!("{so} + {p} + {ss}"),
            format!("{so} + {p} + {lll} + calc({ss} * 3) + {sll}"),
        ),
        (
            format!("{so} + {p}"),
            format!("{so} + {p} + {lll} + calc({ss} * 2) + {sll}"),
        ),
        (
            format!("{so} + {p}"),
            format!("{so} + {p} + calc({ss} * 2) + {sll}"),
        ),
        (
            format!("{so} + {p} + {ss}"),
            format!("{so} + {p} + {sll} + {ss}"),
        ),
        // Left-top.
        (format!("{so} + {p} + {ss}"), format!("{so} + {p} + {ss}")),
        (
            format!("{so} + {p} + calc({ss} * 2)"),
            format!("{so} + {p}"),
        ),
        // Right-top.
        (
            format!("100% - calc({so} + {p} + calc({ss} * 2))"),
            format!("{so} + {p}"),
        ),
        (
            format!("100% - calc({so} + {p} + {ss})"),
            format!("{so} + {p} + {ss}"),
        ),
        // Right.
        (
            format!("100% - calc({so} + {p} + {ss})"),
            format!("100% - calc({so} + {p} + calc({ss} * 3) + {sll} + {lll})"),
        ),
        (
            format!("100% - calc({so} + {p})"),
            format!("100% - calc({so} + {p} + calc({ss} * 2) + {sll} + {lll})"),
        ),
        (
            format!("100% - calc({so} + {p})"),
            format!("100% - calc({so} + {p} + calc({ss} * 2) + {sll})"),
        ),
        (
            format!("100% - calc({so} + {p} + {ss})"),
            format!("100% - calc({so} + {p} + {ss} + {sll})"),
        ),
        // Right-bottom.
        (
            format!("100% - calc({so} + {p} + {ss})"),
            format!("100% - calc({so} + {p} + {ss})"),
        ),
        (
            format!("100% - calc({so} + {p} + calc({ss} * 2))"),
            format!("100% - calc({so} + {p})"),
        ),
    ];

    let series = points
        .into_iter()
        .map(|(x, y)| format!("calc({x}) calc({y})"))
        .collect::<Vec<String>>()
        .join(",\n  ");

    format!("polygon(\n  {series}\n)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_octagon_defaults() {
        let css = style_frame_clip_octagon(StyleFrameClipOctagonProps::default());
        assert!(css.contains("polygon("));
        assert!(css.contains("16px"));
    }

    #[test]
    fn test_clip_kranox_defaults() {
        let css = style_frame_clip_kranox(StyleFrameClipKranoxProps::default());
        assert!(css.contains("polygon("));
        assert!(css.contains("calc("));
    }
}
