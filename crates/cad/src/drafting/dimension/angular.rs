use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use super::render::{RenderedArc, RenderedDimension, RenderedText, TextAlignment};
use super::style::DimensionStyle;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AngleDefinition {
    FromEdges {
        vertex: Point2D,
        edge_a: Point2D,
        edge_b: Point2D,
    },
    FromPoints {
        start: Point2D,
        vertex: Point2D,
        end: Point2D,
    },
    FromHorizontal {
        vertex: Point2D,
        edge_end: Point2D,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AngularDimension {
    pub definition: AngleDefinition,
    pub radius: f64,
    pub style: DimensionStyle,
    pub override_text: Option<String>,
    pub geometry_ref: Option<GeometryRef>,
}

impl AngularDimension {
    pub fn measurement_degrees(&self) -> f64 {
        let (vertex, vector_a, vector_b) = match self.definition {
            AngleDefinition::FromEdges {
                vertex,
                edge_a,
                edge_b,
            }
            | AngleDefinition::FromPoints {
                start: edge_a,
                vertex,
                end: edge_b,
            } => (
                vertex,
                Point2D::new(edge_a.x - vertex.x, edge_a.y - vertex.y),
                Point2D::new(edge_b.x - vertex.x, edge_b.y - vertex.y),
            ),
            AngleDefinition::FromHorizontal { vertex, edge_end } => (
                vertex,
                Point2D::new(1.0, 0.0),
                Point2D::new(edge_end.x - vertex.x, edge_end.y - vertex.y),
            ),
        };

        let _ = vertex;
        angle_between(vector_a, vector_b).to_degrees().abs()
    }

    pub fn render(&self) -> RenderedDimension {
        let (vertex, vector_a, vector_b) = match self.definition {
            AngleDefinition::FromEdges {
                vertex,
                edge_a,
                edge_b,
            }
            | AngleDefinition::FromPoints {
                start: edge_a,
                vertex,
                end: edge_b,
            } => (
                vertex,
                Point2D::new(edge_a.x - vertex.x, edge_a.y - vertex.y),
                Point2D::new(edge_b.x - vertex.x, edge_b.y - vertex.y),
            ),
            AngleDefinition::FromHorizontal { vertex, edge_end } => (
                vertex,
                Point2D::new(1.0, 0.0),
                Point2D::new(edge_end.x - vertex.x, edge_end.y - vertex.y),
            ),
        };

        let start_angle = vector_a.y.atan2(vector_a.x);
        let end_angle = vector_b.y.atan2(vector_b.x);
        let label = self.override_text.clone().unwrap_or_else(|| {
            format!(
                "{:.precision$}deg",
                self.measurement_degrees(),
                precision = self.style.decimal_places
            )
        });

        let text_position = Point2D::new(
            vertex.x + self.radius * ((start_angle + end_angle) * 0.5).cos(),
            vertex.y + self.radius * ((start_angle + end_angle) * 0.5).sin(),
        );

        RenderedDimension {
            arcs: vec![RenderedArc {
                center: vertex,
                radius: self.radius,
                start_angle,
                end_angle,
            }],
            texts: vec![RenderedText {
                text: label,
                position: text_position,
                alignment: TextAlignment::Center,
            }],
            ..RenderedDimension::default()
        }
    }
}

fn angle_between(a: Point2D, b: Point2D) -> f64 {
    let dot = a.x * b.x + a.y * b.y;
    let mag = (a.x * a.x + a.y * a.y).sqrt() * (b.x * b.x + b.y * b.y).sqrt();
    if mag <= 1e-12 {
        0.0
    } else {
        (dot / mag).clamp(-1.0, 1.0).acos()
    }
}
