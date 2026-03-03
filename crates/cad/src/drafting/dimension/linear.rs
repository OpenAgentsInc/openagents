use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use super::render::{RenderedDimension, RenderedText, TextAlignment};
use super::style::DimensionStyle;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LinearDimensionType {
    Horizontal,
    Vertical,
    Aligned,
    Rotated { angle_radians: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearDimension {
    pub start: Point2D,
    pub end: Point2D,
    pub dimension_type: LinearDimensionType,
    pub offset: f64,
    pub style: DimensionStyle,
    pub override_text: Option<String>,
    pub geometry_ref: Option<GeometryRef>,
}

impl LinearDimension {
    pub fn measurement_value(&self) -> f64 {
        match self.dimension_type {
            LinearDimensionType::Horizontal => (self.end.x - self.start.x).abs(),
            LinearDimensionType::Vertical => (self.end.y - self.start.y).abs(),
            LinearDimensionType::Aligned => self.start.distance(self.end),
            LinearDimensionType::Rotated { angle_radians } => {
                let unit = Point2D::new(angle_radians.cos(), angle_radians.sin());
                let dx = self.end.x - self.start.x;
                let dy = self.end.y - self.start.y;
                (dx * unit.x + dy * unit.y).abs()
            }
        }
    }

    pub fn render(&self) -> RenderedDimension {
        let value = self.measurement_value();
        let label = self.override_text.clone().unwrap_or_else(|| {
            format!("{value:.precision$}", precision = self.style.decimal_places)
        });
        let text_position = Point2D::new(
            (self.start.x + self.end.x) * 0.5,
            (self.start.y + self.end.y) * 0.5 + self.offset,
        );

        RenderedDimension {
            lines: vec![(self.start, self.end)],
            texts: vec![RenderedText {
                text: label,
                position: text_position,
                alignment: TextAlignment::Center,
            }],
            ..RenderedDimension::default()
        }
    }
}
