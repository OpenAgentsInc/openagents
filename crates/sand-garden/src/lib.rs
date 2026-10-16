//! The Blue Rush sand garden: portable drawings and the browser editor.

use serde::{Deserialize, Serialize};
pub mod sand;
pub mod visitors;

pub const WIDTH: f64 = 960.0;
pub const HEIGHT: f64 = 600.0;
pub const MAX_POINTS: usize = 3000;
pub const MAX_ACTIONS: usize = 160;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    #[default]
    Rake,
    Finger,
    Smooth,
    Water,
    Select,
    Conch,
    Stone,
    Glass,
    Tooth,
    Anemone,
}
impl Tool {
    pub fn draws(self) -> bool {
        matches!(self, Self::Rake | Self::Finger | Self::Smooth | Self::Water)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Rake => "Rake",
            Self::Finger => "Finger",
            Self::Smooth => "Smooth",
            Self::Water => "Water",
            Self::Select => "Select item",
            Self::Anemone => "Anemone",
            Self::Conch => "Conch",
            Self::Stone => "Stone",
            Self::Glass => "Sea glass",
            Self::Tooth => "Shark tooth",
        }
    }
    pub fn placeable(self) -> bool {
        matches!(
            self,
            Self::Conch | Self::Stone | Self::Glass | Self::Tooth | Self::Anemone
        )
    }
    pub fn color(self) -> &'static str {
        match self {
            Self::Conch => "#dbad94",
            Self::Stone => "#78868a",
            Self::Glass => "#7bac9f",
            Self::Tooth => "#e9debc",
            Self::Anemone => "#bc7f97",
            Self::Water => "#549caa",
            _ => "#e2cca9",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.into())).ok()
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x: (x.clamp(0., WIDTH) * 10.).round() / 10.,
            y: (y.clamp(0., HEIGHT) * 10.).round() / 10.,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub tool: Tool,
    pub size: f64,
    pub angle: f64,
    pub points: Vec<Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    #[default]
    Sun,
    Shade,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Drawing {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub light: Light,
    #[serde(default = "visitors_on")]
    pub visitors: bool,
}
fn visitors_on() -> bool {
    true
}
impl Default for Drawing {
    fn default() -> Self {
        Self {
            actions: vec![],
            light: Light::Sun,
            visitors: true,
        }
    }
}
pub fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl Drawing {
    pub fn valid(&self) -> bool {
        !self.actions.is_empty() && self.within_limits()
    }
    pub fn within_limits(&self) -> bool {
        self.actions.len() <= MAX_ACTIONS
            && self.actions.iter().map(|a| a.points.len()).sum::<usize>() <= MAX_POINTS
            && self.actions.iter().all(|a| {
                a.tool != Tool::Select
                    && a.color.as_deref().is_none_or(valid_color)
                    && a.size.is_finite()
                    && (0.6..=1.8).contains(&a.size)
                    && a.angle.is_finite()
                    && a.angle.abs() <= 7.
                    && !a.points.is_empty()
                    && (a.tool.draws() || a.points.len() == 1)
                    && a.points.iter().all(|p| {
                        p.x.is_finite()
                            && p.y.is_finite()
                            && (0.0..=WIDTH).contains(&p.x)
                            && (0.0..=HEIGHT).contains(&p.y)
                    })
            })
    }
    pub fn points(&self) -> usize {
        self.actions.iter().map(|a| a.points.len()).sum()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Creation {
    pub id: String,
    pub title: String,
    pub drawing: Drawing,
    pub votes: usize,
    pub voted: bool,
    pub can_delete: bool,
}
#[derive(Default, Deserialize, Serialize)]
pub struct Gallery {
    pub items: Vec<Creation>,
    pub total: usize,
    pub offset: usize,
    pub admin: bool,
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod creatures;
#[cfg(target_arch = "wasm32")]
mod paint;
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    browser::start()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawings_bound_work_and_reject_invalid_geometry() {
        let mut drawing = Drawing {
            actions: vec![Action {
                tool: Tool::Rake,
                size: 1.,
                angle: 0.,
                points: vec![Point::new(10., 20.)],
                color: None,
            }],
            ..Drawing::default()
        };
        assert!(drawing.valid());
        drawing.actions[0].points[0].x = f64::NAN;
        assert!(!drawing.valid());
        drawing.actions[0].points = vec![Point::new(10., 20.); MAX_POINTS + 1];
        assert!(!drawing.valid());
        drawing.actions[0].points.truncate(2);
        drawing.actions[0].tool = Tool::Stone;
        assert!(!drawing.valid());
    }
    #[test]
    fn old_drawings_still_open_and_custom_colors_are_checked() {
        let old =
            r##"{"actions":[{"tool":"conch","size":1,"angle":0,"points":[{"x":100,"y":100}]}]}"##;
        let mut d: Drawing = serde_json::from_str(old).unwrap();
        assert!(d.valid());
        assert_eq!(d.light, Light::Sun);
        assert!(d.visitors);
        d.actions[0].color = Some("#52d6ca".into());
        d.light = Light::Shade;
        assert!(d.valid());
        d.actions[0].color = Some("url(evil)".into());
        assert!(!d.valid());
        d.actions[0].color = None;
        d.actions[0].tool = Tool::Select;
        assert!(!d.valid());
    }
    #[test]
    fn tools_have_plain_labels() {
        for t in [
            Tool::Rake,
            Tool::Finger,
            Tool::Smooth,
            Tool::Conch,
            Tool::Stone,
            Tool::Glass,
            Tool::Tooth,
            Tool::Water,
            Tool::Anemone,
            Tool::Select,
        ] {
            assert!(oa_copy::violations(t.label(), &[]).is_empty());
        }
    }
}
