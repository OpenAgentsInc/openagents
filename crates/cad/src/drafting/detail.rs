use super::types::{
    BoundingBox2D, DetailView, DetailViewParams, Point2D, ProjectedEdge, ProjectedView,
};

pub fn create_detail_view(view: &ProjectedView, params: &DetailViewParams) -> DetailView {
    let half_width = params.width * 0.5;
    let half_height = params.height * 0.5;
    let region = BoundingBox2D {
        min_x: params.center.x - half_width,
        min_y: params.center.y - half_height,
        max_x: params.center.x + half_width,
        max_y: params.center.y + half_height,
    };

    let mut edges = Vec::new();
    let mut bounds = BoundingBox2D::empty();

    for edge in &view.edges {
        let midpoint = Point2D::new(
            (edge.start.x + edge.end.x) * 0.5,
            (edge.start.y + edge.end.y) * 0.5,
        );
        if midpoint.x < region.min_x
            || midpoint.x > region.max_x
            || midpoint.y < region.min_y
            || midpoint.y > region.max_y
        {
            continue;
        }

        let scaled_edge = scale_edge(edge, params.center, params.scale);
        bounds.include_point(scaled_edge.start);
        bounds.include_point(scaled_edge.end);
        edges.push(scaled_edge);
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

fn scale_edge(edge: &ProjectedEdge, origin: Point2D, scale: f64) -> ProjectedEdge {
    let start = Point2D::new(
        origin.x + (edge.start.x - origin.x) * scale,
        origin.y + (edge.start.y - origin.y) * scale,
    );
    let end = Point2D::new(
        origin.x + (edge.end.x - origin.x) * scale,
        origin.y + (edge.end.y - origin.y) * scale,
    );
    ProjectedEdge::new(start, end, edge.visibility, edge.edge_type, edge.depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drafting::types::{EdgeType, ViewDirection, Visibility};

    #[test]
    fn detail_view_scales_edges_inside_region() {
        let edge = ProjectedEdge::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        );
        let view = ProjectedView {
            edges: vec![edge],
            bounds: BoundingBox2D {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 1.0,
                max_y: 1.0,
            },
            view_direction: ViewDirection::Front,
        };
        let params = DetailViewParams {
            center: Point2D::new(0.5, 0.0),
            width: 2.0,
            height: 2.0,
            scale: 2.0,
            label: "A".to_string(),
        };

        let detail = create_detail_view(&view, &params);
        assert_eq!(detail.edges.len(), 1);
        assert!((detail.edges[0].end.x - 1.5).abs() < 1e-9);
    }
}
