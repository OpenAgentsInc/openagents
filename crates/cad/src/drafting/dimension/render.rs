use serde::{Deserialize, Serialize};

use crate::drafting::types::Point2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedText {
    pub text: String,
    pub position: Point2D,
    pub alignment: TextAlignment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedArrow {
    pub tip: Point2D,
    pub left_wing: Point2D,
    pub right_wing: Point2D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedArc {
    pub center: Point2D,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RenderedDimension {
    pub lines: Vec<(Point2D, Point2D)>,
    pub arrows: Vec<RenderedArrow>,
    pub arcs: Vec<RenderedArc>,
    pub texts: Vec<RenderedText>,
}
