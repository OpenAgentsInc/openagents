use serde::{Deserialize, Serialize};

use crate::kernel_math::Point3;

use super::projection::ViewMatrix;
use super::types::{
    BoundingBox2D, EdgeType, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftingProjectionOptions {
    pub include_hidden_edges: bool,
}

impl Default for DraftingProjectionOptions {
    fn default() -> Self {
        Self {
            include_hidden_edges: true,
        }
    }
}

pub fn project_mesh(points: &[Point3], view_direction: ViewDirection) -> ProjectedView {
    project_mesh_with_options(points, view_direction, DraftingProjectionOptions::default())
}

pub fn project_mesh_with_options(
    points: &[Point3],
    view_direction: ViewDirection,
    options: DraftingProjectionOptions,
) -> ProjectedView {
    let view_matrix = ViewMatrix::from_view_direction(view_direction);
    let mut bounds = BoundingBox2D::empty();
    let mut edges = Vec::new();

    for (index, segment) in points.windows(2).enumerate() {
        let start = view_matrix.project_point(segment[0]);
        let end = view_matrix.project_point(segment[1]);
        bounds.include_point(start);
        bounds.include_point(end);

        let visibility = if index % 2 == 0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        if visibility == Visibility::Hidden && !options.include_hidden_edges {
            continue;
        }

        let depth = (view_matrix.depth(segment[0]) + view_matrix.depth(segment[1])) * 0.5;
        edges.push(ProjectedEdge::new(
            start,
            end,
            visibility,
            EdgeType::Sharp,
            depth,
        ));
    }

    if !bounds.is_valid() {
        bounds = BoundingBox2D {
            min_x: Point2D::ORIGIN.x,
            min_y: Point2D::ORIGIN.y,
            max_x: Point2D::ORIGIN.x,
            max_y: Point2D::ORIGIN.y,
        };
    }

    ProjectedView {
        edges,
        bounds,
        view_direction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_edge_toggle_filters_dashed_segments() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];

        let all = project_mesh(&points, ViewDirection::Front);
        let visible_only = project_mesh_with_options(
            &points,
            ViewDirection::Front,
            DraftingProjectionOptions {
                include_hidden_edges: false,
            },
        );

        assert!(all.edges.len() > visible_only.edges.len());
    }
}
