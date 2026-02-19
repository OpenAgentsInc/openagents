#[derive(Clone, Copy, Debug)]
pub struct StyleStop<'a> {
    pub color: &'a str,
    pub position: &'a str,
}

pub fn style_strip(stops: &[StyleStop<'_>], direction: Option<&str>) -> String {
    let direction = direction.unwrap_or("to right");
    let mut series = Vec::new();

    for (index, stop) in stops.iter().enumerate() {
        if index == 0 {
            series.push(stop.color.to_string());
            series.push(format!("{} {}", stop.color, stop.position));
            continue;
        }

        let prev_position = stops[index - 1].position;
        series.push(format!("{} {}", stop.color, prev_position));
        series.push(format!("{} {}", stop.color, stop.position));
    }

    format!(
        "repeating-linear-gradient({}, {})",
        direction,
        series.join(", ")
    )
}

pub fn style_steps(length: usize, direction: Option<&str>, color: Option<&str>) -> String {
    let direction = direction.unwrap_or("to right");
    let color = color.unwrap_or("currentcolor");

    if length < 2 {
        return color.to_string();
    }

    let total = length + length - 1;
    let mut steps = Vec::with_capacity(total);

    for index in 0..total {
        let start = (index as f32 / total as f32) * 100.0;
        let end = ((index + 1) as f32 / total as f32) * 100.0;
        if index % 2 == 0 {
            steps.push(format!("{color} {start}%, {color} {end}%"));
        } else {
            steps.push(format!("transparent {start}%, transparent {end}%"));
        }
    }

    format!("linear-gradient({}, {})", direction, steps.join(", "))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeparatorDirection {
    Left,
    Right,
    Both,
}

#[derive(Clone, Debug)]
pub struct StyleSeparatorProps {
    pub color_active: String,
    pub color_static: String,
    pub is_vertical: bool,
    pub direction: SeparatorDirection,
    pub width: String,
    pub space: String,
}

impl StyleSeparatorProps {
    pub fn new(color_active: impl Into<String>, color_static: impl Into<String>) -> Self {
        Self {
            color_active: color_active.into(),
            color_static: color_static.into(),
            is_vertical: false,
            direction: SeparatorDirection::Right,
            width: "0.5rem".to_string(),
            space: "0.25rem".to_string(),
        }
    }

    pub fn vertical(mut self, is_vertical: bool) -> Self {
        self.is_vertical = is_vertical;
        self
    }

    pub fn direction(mut self, direction: SeparatorDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn width(mut self, width: impl Into<String>) -> Self {
        self.width = width.into();
        self
    }

    pub fn space(mut self, space: impl Into<String>) -> Self {
        self.space = space.into();
        self
    }
}

pub fn style_separator(props: StyleSeparatorProps) -> String {
    let StyleSeparatorProps {
        color_active,
        color_static,
        is_vertical,
        direction,
        width,
        space,
    } = props;

    let mut parts = Vec::new();

    if matches!(
        direction,
        SeparatorDirection::Left | SeparatorDirection::Both
    ) {
        parts.extend([
            format!("{color_active} 0px"),
            format!("{color_active} {width}"),
            format!("transparent {width}"),
            format!("transparent calc({width} + {space})"),
            format!("{color_active} calc({width} + {space})"),
            format!("{color_active} calc({width} * 2 + {space})"),
            format!("transparent calc({width} * 2 + {space})"),
            format!("transparent calc({width} * 2 + {space} * 2)"),
            format!("{color_static} calc({width} * 2 + {space} * 2)"),
        ]);
    } else {
        parts.push(format!("{color_static} 0%"));
    }

    if matches!(
        direction,
        SeparatorDirection::Right | SeparatorDirection::Both
    ) {
        parts.extend([
            format!("{color_static} calc(100% - {width} * 2 - {space} * 2)"),
            format!("transparent calc(100% - {width} * 2 - {space} * 2)"),
            format!("transparent calc(100% - {width} * 2 - {space})"),
            format!("{color_active} calc(100% - {width} * 2 - {space})"),
            format!("{color_active} calc(100% - {width} - {space})"),
            format!("transparent calc(100% - {width} - {space})"),
            format!("transparent calc(100% - {width})"),
            format!("{color_active} calc(100% - {width})"),
            format!("{color_active} 100%"),
        ]);
    } else {
        parts.push(format!("{color_static} 100%"));
    }

    format!(
        "linear-gradient(to {}, {})",
        if is_vertical { "bottom" } else { "right" },
        parts.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_strip() {
        let stops = [
            StyleStop {
                color: "red",
                position: "10%",
            },
            StyleStop {
                color: "blue",
                position: "20%",
            },
        ];
        let css = style_strip(&stops, None);
        assert!(css.contains("repeating-linear-gradient"));
        assert!(css.contains("red 10%"));
        assert!(css.contains("blue 20%"));
    }

    #[test]
    fn test_style_steps() {
        let css = style_steps(3, None, Some("cyan"));
        assert!(css.contains("linear-gradient"));
        assert!(css.contains("cyan"));
    }

    #[test]
    fn test_style_separator() {
        let css = style_separator(
            StyleSeparatorProps::new("red", "gray").direction(SeparatorDirection::Both),
        );
        assert!(css.contains("linear-gradient"));
        assert!(css.contains("red"));
        assert!(css.contains("gray"));
    }
}
