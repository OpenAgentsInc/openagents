use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::schema::{ArcGrid, ArcGridError, ArcTask};

/// What belongs in analysis and what must stay out of it.
pub const ANALYSIS_BOUNDARY_SUMMARY: &str = "Own deterministic ARC grid analysis helpers such as canonicalization, color normalization, dimension summaries, and object-like summaries. Do not absorb solver search policy, benchmark scoring, or client/runtime transport behavior.";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcGridDimensions {
    pub width: u8,
    pub height: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcGridPadding {
    pub right: u8,
    pub bottom: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcColorNormalization {
    pub original: u8,
    pub normalized: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalGrid {
    pub grid: ArcGrid,
    pub original_dimensions: ArcGridDimensions,
    pub padding_to_task_max: ArcGridPadding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPair {
    pub input: CanonicalGrid,
    pub output: CanonicalGrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDimensionSummary {
    pub max_width: u8,
    pub max_height: u8,
    pub train_inputs: Vec<ArcGridDimensions>,
    pub train_outputs: Vec<ArcGridDimensions>,
    pub test_inputs: Vec<ArcGridDimensions>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalTask {
    pub raw: ArcTask,
    pub color_normalization: Vec<ArcColorNormalization>,
    pub normalized_train: Vec<CanonicalPair>,
    pub normalized_test_inputs: Vec<CanonicalGrid>,
    pub dimension_summary: TaskDimensionSummary,
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

pub fn summarize_task_dimensions(
    task: &ArcTask,
) -> Result<TaskDimensionSummary, ArcCanonicalizationError> {
    if task.train.is_empty() {
        return Err(ArcCanonicalizationError::MissingTrainExamples);
    }
    if task.test.is_empty() {
        return Err(ArcCanonicalizationError::MissingTestInputs);
    }

    let train_inputs = task
        .train
        .iter()
        .map(|example| dimensions_for(&example.input))
        .collect::<Vec<_>>();
    let train_outputs = task
        .train
        .iter()
        .map(|example| dimensions_for(&example.output))
        .collect::<Vec<_>>();
    let test_inputs = task.test.iter().map(dimensions_for).collect::<Vec<_>>();

    let max_width = train_inputs
        .iter()
        .chain(train_outputs.iter())
        .chain(test_inputs.iter())
        .map(|dimensions| dimensions.width)
        .max()
        .ok_or(ArcCanonicalizationError::MissingTaskGrids)?;
    let max_height = train_inputs
        .iter()
        .chain(train_outputs.iter())
        .chain(test_inputs.iter())
        .map(|dimensions| dimensions.height)
        .max()
        .ok_or(ArcCanonicalizationError::MissingTaskGrids)?;

    Ok(TaskDimensionSummary {
        max_width,
        max_height,
        train_inputs,
        train_outputs,
        test_inputs,
    })
}

pub fn canonicalize_task(task: &ArcTask) -> Result<CanonicalTask, ArcCanonicalizationError> {
    let dimension_summary = summarize_task_dimensions(task)?;
    let color_normalization = build_task_color_normalization(task);
    let normalization_table = build_color_table(&color_normalization);

    let normalized_train = task
        .train
        .iter()
        .map(|example| {
            Ok(CanonicalPair {
                input: canonicalize_grid(
                    &example.input,
                    &normalization_table,
                    dimension_summary.max_width,
                    dimension_summary.max_height,
                )?,
                output: canonicalize_grid(
                    &example.output,
                    &normalization_table,
                    dimension_summary.max_width,
                    dimension_summary.max_height,
                )?,
            })
        })
        .collect::<Result<Vec<_>, ArcCanonicalizationError>>()?;

    let normalized_test_inputs = task
        .test
        .iter()
        .map(|grid| {
            canonicalize_grid(
                grid,
                &normalization_table,
                dimension_summary.max_width,
                dimension_summary.max_height,
            )
        })
        .collect::<Result<Vec<_>, ArcCanonicalizationError>>()?;

    Ok(CanonicalTask {
        raw: task.clone(),
        color_normalization,
        normalized_train,
        normalized_test_inputs,
        dimension_summary,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcCanonicalizationError {
    #[error("ARC canonicalization requires at least one train example")]
    MissingTrainExamples,
    #[error("ARC canonicalization requires at least one test input")]
    MissingTestInputs,
    #[error("ARC canonicalization requires at least one grid")]
    MissingTaskGrids,
    #[error("failed to build normalized ARC grid: {0}")]
    InvalidNormalizedGrid(#[from] ArcGridError),
}

fn dimensions_for(grid: &ArcGrid) -> ArcGridDimensions {
    ArcGridDimensions {
        width: grid.width(),
        height: grid.height(),
    }
}

fn build_task_color_normalization(task: &ArcTask) -> Vec<ArcColorNormalization> {
    let mut seen = [false; 10];
    let mut normalization = vec![ArcColorNormalization {
        original: 0,
        normalized: 0,
    }];
    seen[0] = true;
    let mut next_normalized = 1_u8;

    for grid in task
        .train
        .iter()
        .flat_map(|example| [&example.input, &example.output])
        .chain(task.test.iter())
    {
        for color in grid.cells() {
            let index = usize::from(*color);
            if seen[index] {
                continue;
            }
            seen[index] = true;
            normalization.push(ArcColorNormalization {
                original: *color,
                normalized: next_normalized,
            });
            next_normalized += 1;
        }
    }

    normalization
}

fn build_color_table(normalization: &[ArcColorNormalization]) -> [u8; 10] {
    let mut table = [0_u8; 10];
    for mapping in normalization {
        table[usize::from(mapping.original)] = mapping.normalized;
    }
    table
}

fn canonicalize_grid(
    grid: &ArcGrid,
    normalization_table: &[u8; 10],
    max_width: u8,
    max_height: u8,
) -> Result<CanonicalGrid, ArcCanonicalizationError> {
    let normalized_cells = grid
        .cells()
        .iter()
        .map(|color| normalization_table[usize::from(*color)])
        .collect::<Vec<_>>();
    let normalized_grid = ArcGrid::new(grid.width(), grid.height(), normalized_cells)?;

    Ok(CanonicalGrid {
        grid: normalized_grid,
        original_dimensions: dimensions_for(grid),
        padding_to_task_max: ArcGridPadding {
            right: max_width - grid.width(),
            bottom: max_height - grid.height(),
        },
    })
}
