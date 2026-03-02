use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use super::render::{RenderedDimension, RenderedText, TextAlignment};
use super::style::DimensionStyle;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadialDimension {
    pub center: Point2D,
    pub rim_point: Point2D,
    pub is_diameter: bool,
    pub style: DimensionStyle,
    pub override_text: Option<String>,
    pub geometry_ref: Option<GeometryRef>,
}

impl RadialDimension {
    pub fn measurement_value(&self) -> f64 {
        let radius = self.center.distance(self.rim_point);
        if self.is_diameter {
            radius * 2.0
        } else {
            radius
        }
    }

    pub fn render(&self) -> RenderedDimension {
        let prefix = if self.is_diameter { "D" } else { "R" };
        let value = self.measurement_value();
        let label = self.override_text.clone().unwrap_or_else(|| {
            format!(
                "{prefix}{value:.precision$}",
                precision = self.style.decimal_places
            )
        });

        RenderedDimension {
            lines: vec![(self.center, self.rim_point)],
            texts: vec![RenderedText {
                text: label,
                position: self.rim_point,
                alignment: TextAlignment::Left,
            }],
            ..RenderedDimension::default()
        }
    }
}
