use super::types::{
    BoundingBox2D, DetailView, DetailViewParams, Point2D, ProjectedEdge, ProjectedView,
};

const INSIDE: u8 = 0;
const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const BOTTOM: u8 = 4;
const TOP: u8 = 8;

pub fn create_detail_view(parent: &ProjectedView, params: &DetailViewParams) -> DetailView {
    let (min_x, max_x, min_y, max_y) = detail_bounds(params);

    let mut edges = Vec::new();
    let mut bounds = BoundingBox2D::empty();

    for edge in &parent.edges {
        if let Some(clipped) = clip_edge_to_rect(edge, min_x, max_x, min_y, max_y) {
            let transformed = transform_edge(&clipped, params);
            bounds.include_point(transformed.start);
            bounds.include_point(transformed.end);
            edges.push(transformed);
        }
    }

    if !bounds.is_valid() {
        bounds = BoundingBox2D {
            min_x: Point2D::ORIGIN.x,
            min_y: Point2D::ORIGIN.y,
            max_x: Point2D::ORIGIN.x,
            max_y: Point2D::ORIGIN.y,
        };
    }

    DetailView {
        edges,
        bounds,
        scale: params.scale,
        label: params.label.clone(),
    }
}

fn detail_bounds(params: &DetailViewParams) -> (f64, f64, f64, f64) {
    let half_width = params.width * 0.5;
    let half_height = params.height * 0.5;
    (
        params.center.x - half_width,
        params.center.x + half_width,
        params.center.y - half_height,
        params.center.y + half_height,
    )
}

fn compute_outcode(x: f64, y: f64, min_x: f64, max_x: f64, min_y: f64, max_y: f64) -> u8 {
    let mut code = INSIDE;
    if x < min_x {
        code |= LEFT;
    } else if x > max_x {
        code |= RIGHT;
    }
    if y < min_y {
        code |= BOTTOM;
    } else if y > max_y {
        code |= TOP;
    }
    code
}

fn clip_edge_to_rect(
    edge: &ProjectedEdge,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
) -> Option<ProjectedEdge> {
    let mut x0 = edge.start.x;
    let mut y0 = edge.start.y;
    let mut x1 = edge.end.x;
    let mut y1 = edge.end.y;

    let mut outcode0 = compute_outcode(x0, y0, min_x, max_x, min_y, max_y);
    let mut outcode1 = compute_outcode(x1, y1, min_x, max_x, min_y, max_y);

    loop {
        if (outcode0 | outcode1) == 0 {
            return Some(ProjectedEdge::new(
                Point2D::new(x0, y0),
                Point2D::new(x1, y1),
                edge.visibility,
                edge.edge_type,
                edge.depth,
            ));
        }

        if (outcode0 & outcode1) != 0 {
            return None;
        }

        let outcode_out = if outcode0 != 0 { outcode0 } else { outcode1 };

        let (x, y) = if (outcode_out & TOP) != 0 {
            let dy = y1 - y0;
            if dy.abs() <= 1e-12 {
                return None;
            }
            let x = x0 + (x1 - x0) * (max_y - y0) / dy;
            (x, max_y)
        } else if (outcode_out & BOTTOM) != 0 {
            let dy = y1 - y0;
            if dy.abs() <= 1e-12 {
                return None;
            }
            let x = x0 + (x1 - x0) * (min_y - y0) / dy;
            (x, min_y)
        } else if (outcode_out & RIGHT) != 0 {
            let dx = x1 - x0;
            if dx.abs() <= 1e-12 {
                return None;
            }
            let y = y0 + (y1 - y0) * (max_x - x0) / dx;
            (max_x, y)
        } else {
            let dx = x1 - x0;
            if dx.abs() <= 1e-12 {
                return None;
            }
            let y = y0 + (y1 - y0) * (min_x - x0) / dx;
            (min_x, y)
        };

        if outcode_out == outcode0 {
            x0 = x;
            y0 = y;
            outcode0 = compute_outcode(x0, y0, min_x, max_x, min_y, max_y);
        } else {
            x1 = x;
            y1 = y;
            outcode1 = compute_outcode(x1, y1, min_x, max_x, min_y, max_y);
        }
    }
}

fn transform_edge(edge: &ProjectedEdge, params: &DetailViewParams) -> ProjectedEdge {
    let transform_point = |point: Point2D| {
        Point2D::new(
            (point.x - params.center.x) * params.scale,
            (point.y - params.center.y) * params.scale,
        )
    };

    ProjectedEdge::new(
        transform_point(edge.start),
        transform_point(edge.end),
        edge.visibility,
        edge.edge_type,
        edge.depth,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drafting::types::{EdgeType, ViewDirection, Visibility};

    fn make_test_view() -> ProjectedView {
        let mut view = ProjectedView::new(ViewDirection::Front);
        view.add_edge(ProjectedEdge::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(100.0, 0.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(100.0, 0.0),
            Point2D::new(100.0, 100.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(100.0, 100.0),
            Point2D::new(0.0, 100.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(0.0, 100.0),
            Point2D::new(0.0, 0.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(100.0, 100.0),
            Visibility::Hidden,
            EdgeType::Sharp,
            0.0,
        ));
        view
    }

    #[test]
    fn detail_view_clips_and_scales_edges() {
        let view = make_test_view();
        let params = DetailViewParams {
            center: Point2D::new(25.0, 25.0),
            width: 50.0,
            height: 50.0,
            scale: 2.0,
            label: "A".to_string(),
        };

        let detail = create_detail_view(&view, &params);
        assert!(!detail.edges.is_empty());
        assert_eq!(detail.label, "A");
        assert!(detail.bounds.is_valid());
    }

    #[test]
    fn clipping_inside_edge_preserves_points() {
        let edge = ProjectedEdge::new(
            Point2D::new(10.0, 10.0),
            Point2D::new(20.0, 20.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        );

        let clipped = clip_edge_to_rect(&edge, 0.0, 100.0, 0.0, 100.0).expect("edge clipped");
        assert!((clipped.start.x - 10.0).abs() < 1e-9);
        assert!((clipped.end.x - 20.0).abs() < 1e-9);
    }

    #[test]
    fn clipping_outside_edge_returns_none() {
        let edge = ProjectedEdge::new(
            Point2D::new(-10.0, -10.0),
            Point2D::new(-5.0, -5.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        );

        assert!(clip_edge_to_rect(&edge, 0.0, 100.0, 0.0, 100.0).is_none());
    }

    #[test]
    fn clipping_crossing_edge_limits_to_bounds() {
        let edge = ProjectedEdge::new(
            Point2D::new(-50.0, 50.0),
            Point2D::new(150.0, 50.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        );

        let clipped = clip_edge_to_rect(&edge, 0.0, 100.0, 0.0, 100.0).expect("edge clipped");
        assert!((clipped.start.x - 0.0).abs() < 1e-9);
        assert!((clipped.end.x - 100.0).abs() < 1e-9);
    }

    #[test]
    fn transform_centers_and_scales_edge() {
        let params = DetailViewParams {
            center: Point2D::new(50.0, 50.0),
            width: 100.0,
            height: 100.0,
            scale: 2.0,
            label: "A".to_string(),
        };

        let edge = ProjectedEdge::new(
            Point2D::new(50.0, 50.0),
            Point2D::new(60.0, 50.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        );

        let transformed = transform_edge(&edge, &params);
        assert!((transformed.start.x - 0.0).abs() < 1e-9);
        assert!((transformed.start.y - 0.0).abs() < 1e-9);
        assert!((transformed.end.x - 20.0).abs() < 1e-9);
    }
}
