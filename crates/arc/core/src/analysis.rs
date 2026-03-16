use serde::{Deserialize, Serialize};

use crate::schema::ArcGrid;

/// What belongs in analysis and what must stay out of it.
pub const ANALYSIS_BOUNDARY_SUMMARY: &str = "Own deterministic ARC grid analysis helpers such as canonical palette views and object-like summaries. Do not absorb solver search policy, benchmark scoring, or client/runtime transport behavior.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcBoundingBox {
    pub min_x: u8,
    pub min_y: u8,
    pub max_x: u8,
    pub max_y: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridAnalysisSummary {
    pub palette: Vec<u8>,
    pub non_background_cell_count: usize,
    pub bounding_box: Option<ArcBoundingBox>,
}

#[must_use]
pub fn canonical_palette(grid: &ArcGrid) -> Vec<u8> {
    let mut palette = grid.cells().to_vec();
    palette.sort_unstable();
    palette.dedup();
    palette
}

#[must_use]
pub fn summarize_grid(grid: &ArcGrid) -> GridAnalysisSummary {
    let palette = canonical_palette(grid);
    let non_background_positions = grid
        .cells()
        .iter()
        .enumerate()
        .filter(|(_, color)| **color != 0)
        .map(|(index, _)| {
            let width = usize::from(grid.width());
            let x = (index % width) as u8;
            let y = (index / width) as u8;
            (x, y)
        })
        .collect::<Vec<_>>();

    let bounding_box = non_background_positions
        .iter()
        .copied()
        .fold(None, |current, (x, y)| match current {
            None => Some(ArcBoundingBox {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            }),
            Some(bounds) => Some(ArcBoundingBox {
                min_x: bounds.min_x.min(x),
                min_y: bounds.min_y.min(y),
                max_x: bounds.max_x.max(x),
                max_y: bounds.max_y.max(y),
            }),
        });

    GridAnalysisSummary {
        palette,
        non_background_cell_count: non_background_positions.len(),
        bounding_box,
    }
}
