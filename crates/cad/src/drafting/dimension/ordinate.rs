use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use super::render::{RenderedDimension, RenderedText, TextAlignment};
use super::style::DimensionStyle;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrdinateDimension {
    pub datum: Point2D,
    pub target: Point2D,
    pub is_x: bool,
    pub style: DimensionStyle,
    pub override_text: Option<String>,
    pub geometry_ref: Option<GeometryRef>,
}

impl OrdinateDimension {
    pub fn measurement_value(&self) -> f64 {
        if self.is_x {
            self.target.x - self.datum.x
        } else {
            self.target.y - self.datum.y
        }
    }

    pub fn render(&self) -> RenderedDimension {
        let axis = if self.is_x { "X" } else { "Y" };
        let value = self.measurement_value();
        let label = self.override_text.clone().unwrap_or_else(|| {
            format!(
                "{axis}{value:.precision$}",
                precision = self.style.decimal_places
            )
        });

        RenderedDimension {
            lines: vec![(self.datum, self.target)],
            texts: vec![RenderedText {
                text: label,
                position: self.target,
                alignment: TextAlignment::Left,
            }],
            ..RenderedDimension::default()
        }
    }
}
