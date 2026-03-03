use serde::{Deserialize, Serialize};

use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeometryRef {
    Point(Point2D),
    Edge { start: Point2D, end: Point2D },
    Circle { center: Point2D, radius: f64 },
    VertexIndex(usize),
    EdgeIndex(usize),
}
