//! The Blue Rush sand garden: portable drawings and the browser editor.

use serde::{Deserialize, Serialize};
pub mod sand;

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
    Conch,
    Stone,
    Glass,
    Tooth,
}
impl Tool {
    pub fn draws(self) -> bool {
        matches!(self, Self::Rake | Self::Finger | Self::Smooth)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Rake => "Rake",
            Self::Finger => "Finger",
            Self::Smooth => "Smooth",
            Self::Conch => "Conch",
            Self::Stone => "Stone",
            Self::Glass => "Sea glass",
            Self::Tooth => "Shark tooth",
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
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Drawing {
    pub actions: Vec<Action>,
}
impl Drawing {
    pub fn valid(&self) -> bool {
        !self.actions.is_empty()
            && self.actions.len() <= MAX_ACTIONS
            && self.actions.iter().map(|a| a.points.len()).sum::<usize>() <= MAX_POINTS
            && self.actions.iter().all(|a| {
                a.size.is_finite()
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
            }],
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
    fn tools_have_plain_labels() {
        for t in [
            Tool::Rake,
            Tool::Finger,
            Tool::Smooth,
            Tool::Conch,
            Tool::Stone,
            Tool::Glass,
            Tool::Tooth,
        ] {
            assert!(oa_copy::violations(t.label(), &[]).is_empty());
        }
    }
}
