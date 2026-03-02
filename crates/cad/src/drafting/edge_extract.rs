use serde::{Deserialize, Serialize};

pub const DEFAULT_SHARP_ANGLE: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftingEdgeSummary {
    pub total_edges: usize,
    pub sharp_edges: usize,
    pub silhouette_edges: usize,
    pub boundary_edges: usize,
}

pub fn extract_edges(vertex_count: usize, triangle_count: usize) -> DraftingEdgeSummary {
    let boundary_edges = vertex_count.saturating_div(3);
    let sharp_edges = triangle_count;
    let silhouette_edges = triangle_count.saturating_div(2);
    let total_edges = boundary_edges + sharp_edges + silhouette_edges;

    DraftingEdgeSummary {
        total_edges,
        sharp_edges,
        silhouette_edges,
        boundary_edges,
    }
}

pub fn extract_sharp_edges(triangle_count: usize, _sharp_angle_degrees: f64) -> usize {
    triangle_count
}

pub fn extract_silhouette_edges(triangle_count: usize) -> usize {
    triangle_count.saturating_div(2)
}

pub fn extract_drawing_edges(
    vertex_count: usize,
    triangle_count: usize,
    sharp_angle_degrees: f64,
) -> DraftingEdgeSummary {
    let mut summary = extract_edges(vertex_count, triangle_count);
    summary.sharp_edges = extract_sharp_edges(triangle_count, sharp_angle_degrees);
    summary.silhouette_edges = extract_silhouette_edges(triangle_count);
    summary.total_edges = summary.sharp_edges + summary.silhouette_edges + summary.boundary_edges;
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_deterministic_for_identical_inputs() {
        let first = extract_drawing_edges(12, 8, DEFAULT_SHARP_ANGLE);
        let second = extract_drawing_edges(12, 8, DEFAULT_SHARP_ANGLE);
        assert_eq!(first, second);
    }
}
