use std::collections::VecDeque;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitGrid {
    pub width: u8,
    pub height: u8,
    pub bits: Vec<u64>,
}

impl BitGrid {
    fn from_points(width: u8, height: u8, points: &[(u8, u8)]) -> Self {
        let total_bits = usize::from(width) * usize::from(height);
        let word_count = total_bits.div_ceil(64);
        let mut bits = vec![0_u64; word_count];

        for (x, y) in points {
            let bit_index = usize::from(*y) * usize::from(width) + usize::from(*x);
            let word_index = bit_index / 64;
            let bit_offset = bit_index % 64;
            bits[word_index] |= 1_u64 << bit_offset;
        }

        Self {
            width,
            height,
            bits,
        }
    }

    #[must_use]
    pub fn is_set(&self, x: u8, y: u8) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        let bit_index = usize::from(y) * usize::from(self.width) + usize::from(x);
        let word_index = bit_index / 64;
        let bit_offset = bit_index % 64;
        (self.bits[word_index] & (1_u64 << bit_offset)) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectivityKind {
    FourConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeSignature {
    pub horizontal: bool,
    pub vertical: bool,
    pub rotational_180: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcObject {
    pub object_id: ObjectId,
    pub color_histogram: [u8; 10],
    pub bbox: ArcBoundingBox,
    pub mask: BitGrid,
    pub holes: u8,
    pub connectivity: ConnectivityKind,
    pub centroid: (f32, f32),
    pub shape_signature: ShapeSignature,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectRelationKind {
    Touch,
    RowAligned,
    ColumnAligned,
    LeftOf { gap: u8 },
    Above { gap: u8 },
    SymmetryPeer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRelation {
    pub source: ObjectId,
    pub target: ObjectId,
    pub kind: ObjectRelationKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationGraph {
    pub objects: Vec<ArcObject>,
    pub edges: Vec<ObjectRelation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrespondenceFeatures {
    pub same_bbox_size: bool,
    pub same_cell_count: bool,
    pub same_hole_count: bool,
    pub same_shape_signature: bool,
    pub same_relation_degree: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrespondenceCandidate {
    pub input_object: ObjectId,
    pub output_object: ObjectId,
    pub score: u8,
    pub features: CorrespondenceFeatures,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainPairCorrespondence {
    pub pair_index: u16,
    pub input_graph: RelationGraph,
    pub output_graph: RelationGraph,
    pub candidates: Vec<CorrespondenceCandidate>,
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

#[must_use]
pub fn extract_relation_graph(grid: &ArcGrid) -> RelationGraph {
    let mut objects = extract_objects(grid);
    objects.sort_by_key(object_sort_key);
    for (index, object) in objects.iter_mut().enumerate() {
        object.object_id = ObjectId(index as u32);
    }

    let mut edges = build_relation_edges(&objects);
    edges.sort_by_key(relation_sort_key);

    RelationGraph { objects, edges }
}

pub fn extract_train_correspondence_candidates(
    task: &ArcTask,
) -> Result<Vec<TrainPairCorrespondence>, ArcCanonicalizationError> {
    let canonical = canonicalize_task(task)?;

    canonical
        .normalized_train
        .iter()
        .enumerate()
        .map(|(pair_index, pair)| {
            let input_graph = extract_relation_graph(&pair.input.grid);
            let output_graph = extract_relation_graph(&pair.output.grid);
            let mut candidates = build_correspondence_candidates(&input_graph, &output_graph);
            candidates.sort_by_key(correspondence_sort_key);

            Ok(TrainPairCorrespondence {
                pair_index: pair_index as u16,
                input_graph,
                output_graph,
                candidates,
            })
        })
        .collect()
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

fn extract_objects(grid: &ArcGrid) -> Vec<ArcObject> {
    let width = usize::from(grid.width());
    let mut visited = vec![false; width * usize::from(grid.height())];
    let mut objects = Vec::new();

    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let index = usize::from(y) * width + usize::from(x);
            if visited[index] {
                continue;
            }

            let Some(color) = grid.cell(x, y) else {
                continue;
            };
            visited[index] = true;
            if color == 0 {
                continue;
            }

            let points = flood_fill_component(grid, x, y, color, &mut visited);
            objects.push(build_arc_object(color, &points));
        }
    }

    objects
}

fn flood_fill_component(
    grid: &ArcGrid,
    start_x: u8,
    start_y: u8,
    color: u8,
    visited: &mut [bool],
) -> Vec<(u8, u8)> {
    let width = usize::from(grid.width());
    let mut queue = VecDeque::from([(start_x, start_y)]);
    let mut points = Vec::new();

    while let Some((x, y)) = queue.pop_front() {
        points.push((x, y));

        for (next_x, next_y) in four_neighbors_grid(grid, x, y) {
            let index = usize::from(next_y) * width + usize::from(next_x);
            if visited[index] {
                continue;
            }

            visited[index] = true;
            if grid.cell(next_x, next_y) == Some(color) {
                queue.push_back((next_x, next_y));
            }
        }
    }

    points
}

fn build_arc_object(color: u8, points: &[(u8, u8)]) -> ArcObject {
    let bbox = bounding_box_for_points(points);
    let relative_points = points
        .iter()
        .map(|(x, y)| (x - bbox.min_x, y - bbox.min_y))
        .collect::<Vec<_>>();
    let mask = BitGrid::from_points(
        bbox.max_x - bbox.min_x + 1,
        bbox.max_y - bbox.min_y + 1,
        &relative_points,
    );
    let mut color_histogram = [0_u8; 10];
    color_histogram[usize::from(color)] = points.len() as u8;

    let x_sum = points.iter().map(|(x, _)| f32::from(*x)).sum::<f32>();
    let y_sum = points.iter().map(|(_, y)| f32::from(*y)).sum::<f32>();
    let count = points.len() as f32;

    ArcObject {
        object_id: ObjectId(0),
        color_histogram,
        bbox,
        mask: mask.clone(),
        holes: count_holes(&mask),
        connectivity: ConnectivityKind::FourConnected,
        centroid: (x_sum / count, y_sum / count),
        shape_signature: shape_signature(&mask),
    }
}

fn bounding_box_for_points(points: &[(u8, u8)]) -> ArcBoundingBox {
    points.iter().copied().fold(
        ArcBoundingBox {
            min_x: u8::MAX,
            min_y: u8::MAX,
            max_x: 0,
            max_y: 0,
        },
        |bbox, (x, y)| ArcBoundingBox {
            min_x: bbox.min_x.min(x),
            min_y: bbox.min_y.min(y),
            max_x: bbox.max_x.max(x),
            max_y: bbox.max_y.max(y),
        },
    )
}

fn count_holes(mask: &BitGrid) -> u8 {
    let width = usize::from(mask.width);
    let mut visited = vec![false; width * usize::from(mask.height)];
    let mut holes = 0_u8;

    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.is_set(x, y) {
                continue;
            }

            let index = usize::from(y) * width + usize::from(x);
            if visited[index] {
                continue;
            }

            let mut queue = VecDeque::from([(x, y)]);
            visited[index] = true;
            let mut touches_boundary =
                x == 0 || y == 0 || x + 1 == mask.width || y + 1 == mask.height;

            while let Some((current_x, current_y)) = queue.pop_front() {
                for (next_x, next_y) in four_neighbors_mask(mask, current_x, current_y) {
                    if mask.is_set(next_x, next_y) {
                        continue;
                    }

                    let next_index = usize::from(next_y) * width + usize::from(next_x);
                    if visited[next_index] {
                        continue;
                    }
                    visited[next_index] = true;
                    if next_x == 0
                        || next_y == 0
                        || next_x + 1 == mask.width
                        || next_y + 1 == mask.height
                    {
                        touches_boundary = true;
                    }
                    queue.push_back((next_x, next_y));
                }
            }

            if !touches_boundary {
                holes = holes.saturating_add(1);
            }
        }
    }

    holes
}

fn shape_signature(mask: &BitGrid) -> ShapeSignature {
    let horizontal = (0..mask.height)
        .all(|y| (0..mask.width).all(|x| mask.is_set(x, y) == mask.is_set(mask.width - 1 - x, y)));
    let vertical = (0..mask.height)
        .all(|y| (0..mask.width).all(|x| mask.is_set(x, y) == mask.is_set(x, mask.height - 1 - y)));
    let rotational_180 = (0..mask.height).all(|y| {
        (0..mask.width)
            .all(|x| mask.is_set(x, y) == mask.is_set(mask.width - 1 - x, mask.height - 1 - y))
    });

    ShapeSignature {
        horizontal,
        vertical,
        rotational_180,
    }
}

fn build_relation_edges(objects: &[ArcObject]) -> Vec<ObjectRelation> {
    let mut edges = Vec::new();

    for (index, source) in objects.iter().enumerate() {
        for target in objects.iter().skip(index + 1) {
            if objects_touch(source, target) {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::Touch,
                });
            }
            if source.bbox.min_y == target.bbox.min_y {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::RowAligned,
                });
            }
            if source.bbox.min_x == target.bbox.min_x {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::ColumnAligned,
                });
            }
            if source.bbox.max_x < target.bbox.min_x {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::LeftOf {
                        gap: target.bbox.min_x - source.bbox.max_x - 1,
                    },
                });
            }
            if source.bbox.max_y < target.bbox.min_y {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::Above {
                        gap: target.bbox.min_y - source.bbox.max_y - 1,
                    },
                });
            }
            if source.shape_signature == target.shape_signature
                && source.mask.width == target.mask.width
                && source.mask.height == target.mask.height
            {
                edges.push(ObjectRelation {
                    source: source.object_id,
                    target: target.object_id,
                    kind: ObjectRelationKind::SymmetryPeer,
                });
            }
        }
    }

    edges
}

fn build_correspondence_candidates(
    input_graph: &RelationGraph,
    output_graph: &RelationGraph,
) -> Vec<CorrespondenceCandidate> {
    let mut candidates = Vec::new();

    for input_object in &input_graph.objects {
        for output_object in &output_graph.objects {
            let features = CorrespondenceFeatures {
                same_bbox_size: input_object.mask.width == output_object.mask.width
                    && input_object.mask.height == output_object.mask.height,
                same_cell_count: object_cell_count(input_object)
                    == object_cell_count(output_object),
                same_hole_count: input_object.holes == output_object.holes,
                same_shape_signature: input_object.shape_signature == output_object.shape_signature,
                same_relation_degree: relation_degree(input_graph, input_object.object_id)
                    == relation_degree(output_graph, output_object.object_id),
            };
            let score = correspondence_score(features);
            if score == 0 {
                continue;
            }

            candidates.push(CorrespondenceCandidate {
                input_object: input_object.object_id,
                output_object: output_object.object_id,
                score,
                features,
            });
        }
    }

    candidates
}

fn objects_touch(source: &ArcObject, target: &ArcObject) -> bool {
    let x_gap = bounding_gap(
        source.bbox.min_x,
        source.bbox.max_x,
        target.bbox.min_x,
        target.bbox.max_x,
    );
    let y_gap = bounding_gap(
        source.bbox.min_y,
        source.bbox.max_y,
        target.bbox.min_y,
        target.bbox.max_y,
    );
    x_gap <= 1 && y_gap <= 1
}

fn bounding_gap(source_min: u8, source_max: u8, target_min: u8, target_max: u8) -> u8 {
    if source_max < target_min {
        target_min - source_max - 1
    } else if target_max < source_min {
        source_min - target_max - 1
    } else {
        0
    }
}

fn object_sort_key(object: &ArcObject) -> (u8, u8, u16, [u8; 10]) {
    let area = u16::from(object.bbox.max_x - object.bbox.min_x + 1)
        * u16::from(object.bbox.max_y - object.bbox.min_y + 1);
    (
        object.bbox.min_y,
        object.bbox.min_x,
        area,
        object.color_histogram,
    )
}

fn relation_sort_key(relation: &ObjectRelation) -> (u32, u32, String) {
    (
        relation.source.0,
        relation.target.0,
        format!("{:?}", relation.kind),
    )
}

fn correspondence_sort_key(candidate: &CorrespondenceCandidate) -> (u8, u32, u32) {
    (
        u8::MAX - candidate.score,
        candidate.input_object.0,
        candidate.output_object.0,
    )
}

fn object_cell_count(object: &ArcObject) -> u16 {
    object
        .color_histogram
        .iter()
        .map(|count| u16::from(*count))
        .sum()
}

fn relation_degree(graph: &RelationGraph, object_id: ObjectId) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| edge.source == object_id || edge.target == object_id)
        .count()
}

fn correspondence_score(features: CorrespondenceFeatures) -> u8 {
    let mut score = 0_u8;
    if features.same_shape_signature {
        score = score.saturating_add(4);
    }
    if features.same_bbox_size {
        score = score.saturating_add(2);
    }
    if features.same_cell_count {
        score = score.saturating_add(2);
    }
    if features.same_hole_count {
        score = score.saturating_add(1);
    }
    if features.same_relation_degree {
        score = score.saturating_add(1);
    }
    score
}

fn four_neighbors_grid(grid: &ArcGrid, x: u8, y: u8) -> Vec<(u8, u8)> {
    let mut neighbors = Vec::with_capacity(4);
    if x > 0 {
        neighbors.push((x - 1, y));
    }
    if x + 1 < grid.width() {
        neighbors.push((x + 1, y));
    }
    if y > 0 {
        neighbors.push((x, y - 1));
    }
    if y + 1 < grid.height() {
        neighbors.push((x, y + 1));
    }
    neighbors
}

fn four_neighbors_mask(mask: &BitGrid, x: u8, y: u8) -> Vec<(u8, u8)> {
    let mut neighbors = Vec::with_capacity(4);
    if x > 0 {
        neighbors.push((x - 1, y));
    }
    if x + 1 < mask.width {
        neighbors.push((x + 1, y));
    }
    if y > 0 {
        neighbors.push((x, y - 1));
    }
    if y + 1 < mask.height {
        neighbors.push((x, y + 1));
    }
    neighbors
}
