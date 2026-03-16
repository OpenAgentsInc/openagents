use std::collections::BTreeMap;

use arc_core::{ArcBoundingBox, ArcGrid, ArcGridError, ArcObject, extract_relation_graph};
use thiserror::Error;

use crate::dsl::{ArcGridExpr, ArcObjectSelector, ArcObjectTransform, ArcProgram, ArcSymbol};

#[derive(Default)]
pub struct ArcInterpreter;

impl ArcInterpreter {
    pub fn execute(program: &ArcProgram, input: &ArcGrid) -> Result<ArcGrid, ArcInterpreterError> {
        let mut environment = BTreeMap::new();
        environment.insert(program.input_symbol.as_str().to_owned(), input.clone());

        for binding in &program.bindings {
            let value = Self::eval_grid(&binding.value, input, &mut environment)?;
            environment.insert(binding.name.as_str().to_owned(), value);
        }

        Self::eval_grid(&program.body, input, &mut environment)
    }

    fn eval_grid(
        expr: &ArcGridExpr,
        current_input: &ArcGrid,
        environment: &mut BTreeMap<String, ArcGrid>,
    ) -> Result<ArcGrid, ArcInterpreterError> {
        match expr {
            ArcGridExpr::Input => Ok(current_input.clone()),
            ArcGridExpr::Var { name } => environment
                .get(name.as_str())
                .cloned()
                .ok_or_else(|| ArcInterpreterError::UnknownSymbol(name.clone())),
            ArcGridExpr::Empty {
                width,
                height,
                fill,
            } => Ok(fill_grid(*width, *height, *fill)?),
            ArcGridExpr::Sequence { steps } => {
                let Some(first) = steps.first() else {
                    return Err(ArcInterpreterError::EmptySequence);
                };
                let mut current = Self::eval_grid(first, current_input, environment)?;
                for step in &steps[1..] {
                    current = Self::eval_grid(step, &current, environment)?;
                }
                Ok(current)
            }
            ArcGridExpr::CropToSelector { source, selector } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                crop_to_selector(&source, selector)
            }
            ArcGridExpr::PaintSelector {
                base,
                source,
                selector,
                recolor,
                transform,
            } => {
                let base = Self::eval_grid(base, current_input, environment)?;
                let source = Self::eval_grid(source, current_input, environment)?;
                paint_selector(&base, &source, selector, *recolor, *transform)
            }
            ArcGridExpr::RotateQuarterTurns {
                source,
                quarter_turns,
            } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                rotate_grid(&source, *quarter_turns)
            }
            ArcGridExpr::ReflectHorizontal { source } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                reflect_horizontal(&source)
            }
            ArcGridExpr::ReflectVertical { source } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                reflect_vertical(&source)
            }
            ArcGridExpr::Recolor { source, from, to } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                recolor_grid(&source, *from, *to)
            }
            ArcGridExpr::IfAnyObjects {
                source,
                selector,
                then_branch,
                else_branch,
            } => {
                let source = Self::eval_grid(source, current_input, environment)?;
                let selected = select_objects(&source, selector);
                if selected.is_empty() {
                    Self::eval_grid(else_branch, current_input, environment)
                } else {
                    Self::eval_grid(then_branch, current_input, environment)
                }
            }
            ArcGridExpr::Let { name, value, body } => {
                let value = Self::eval_grid(value, current_input, environment)?;
                let previous = environment.insert(name.as_str().to_owned(), value);
                let result = Self::eval_grid(body, current_input, environment);
                if let Some(previous) = previous {
                    environment.insert(name.as_str().to_owned(), previous);
                } else {
                    environment.remove(name.as_str());
                }
                result
            }
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcInterpreterError {
    #[error("ARC interpreter encountered an empty sequence expression")]
    EmptySequence,
    #[error("ARC interpreter could not resolve symbol `{0}`")]
    UnknownSymbol(ArcSymbol),
    #[error("ARC interpreter selector matched no objects on the current grid")]
    NoObjectsMatched,
    #[error(
        "ARC interpreter translation or repaint would place a cell outside the grid at ({x}, {y})"
    )]
    PaintOutOfBounds { x: i16, y: i16 },
    #[error("ARC interpreter rotate-quarter-turns requires a value in the 0..=3 range, got {0}")]
    InvalidQuarterTurns(u8),
    #[error("ARC interpreter failed to build a valid grid: {0}")]
    Grid(#[from] ArcGridError),
}

#[derive(Clone)]
struct SelectedObject {
    object: ArcObject,
    color: u8,
    points: Vec<(u8, u8)>,
}

fn fill_grid(width: u8, height: u8, fill: u8) -> Result<ArcGrid, ArcInterpreterError> {
    Ok(ArcGrid::new(
        width,
        height,
        vec![fill; usize::from(width) * usize::from(height)],
    )?)
}

fn crop_to_selector(
    source: &ArcGrid,
    selector: &ArcObjectSelector,
) -> Result<ArcGrid, ArcInterpreterError> {
    let selected = select_objects(source, selector);
    if selected.is_empty() {
        return Err(ArcInterpreterError::NoObjectsMatched);
    }

    let bounds = selected
        .iter()
        .map(|object| object.object.bbox)
        .fold(selected[0].object.bbox, union_bbox);
    crop_grid(source, bounds)
}

fn paint_selector(
    base: &ArcGrid,
    source: &ArcGrid,
    selector: &ArcObjectSelector,
    recolor: Option<u8>,
    transform: ArcObjectTransform,
) -> Result<ArcGrid, ArcInterpreterError> {
    let selected = select_objects(source, selector);
    if selected.is_empty() {
        return Err(ArcInterpreterError::NoObjectsMatched);
    }

    let mut cells = base.cells().to_vec();
    for object in &selected {
        for (x, y) in transformed_points(object, transform)? {
            if x < 0 || y < 0 || x >= i16::from(base.width()) || y >= i16::from(base.height()) {
                return Err(ArcInterpreterError::PaintOutOfBounds { x, y });
            }
            let x = usize::try_from(x).unwrap_or_default();
            let y = usize::try_from(y).unwrap_or_default();
            let index = y * usize::from(base.width()) + x;
            cells[index] = recolor.unwrap_or(object.color);
        }
    }

    Ok(ArcGrid::new(base.width(), base.height(), cells)?)
}

fn rotate_grid(source: &ArcGrid, quarter_turns: u8) -> Result<ArcGrid, ArcInterpreterError> {
    if quarter_turns > 3 {
        return Err(ArcInterpreterError::InvalidQuarterTurns(quarter_turns));
    }

    match quarter_turns {
        0 => Ok(source.clone()),
        1 => {
            let new_width = source.height();
            let new_height = source.width();
            let mut cells = vec![0; usize::from(new_width) * usize::from(new_height)];
            for y in 0..source.height() {
                for x in 0..source.width() {
                    let value = source.cell(x, y).unwrap_or_default();
                    let rotated_x = source.height() - 1 - y;
                    let rotated_y = x;
                    let index =
                        usize::from(rotated_y) * usize::from(new_width) + usize::from(rotated_x);
                    cells[index] = value;
                }
            }
            Ok(ArcGrid::new(new_width, new_height, cells)?)
        }
        2 => rotate_grid(&rotate_grid(source, 1)?, 1),
        3 => rotate_grid(&rotate_grid(source, 2)?, 1),
        other => Err(ArcInterpreterError::InvalidQuarterTurns(other)),
    }
}

fn reflect_horizontal(source: &ArcGrid) -> Result<ArcGrid, ArcInterpreterError> {
    let mut cells = vec![0; source.cell_count()];
    for y in 0..source.height() {
        for x in 0..source.width() {
            let value = source.cell(x, y).unwrap_or_default();
            let reflected_x = source.width() - 1 - x;
            let index = usize::from(y) * usize::from(source.width()) + usize::from(reflected_x);
            cells[index] = value;
        }
    }
    Ok(ArcGrid::new(source.width(), source.height(), cells)?)
}

fn reflect_vertical(source: &ArcGrid) -> Result<ArcGrid, ArcInterpreterError> {
    let mut cells = vec![0; source.cell_count()];
    for y in 0..source.height() {
        for x in 0..source.width() {
            let value = source.cell(x, y).unwrap_or_default();
            let reflected_y = source.height() - 1 - y;
            let index = usize::from(reflected_y) * usize::from(source.width()) + usize::from(x);
            cells[index] = value;
        }
    }
    Ok(ArcGrid::new(source.width(), source.height(), cells)?)
}

fn recolor_grid(source: &ArcGrid, from: u8, to: u8) -> Result<ArcGrid, ArcInterpreterError> {
    let cells = source
        .cells()
        .iter()
        .map(|value| if *value == from { to } else { *value })
        .collect::<Vec<_>>();
    Ok(ArcGrid::new(source.width(), source.height(), cells)?)
}

fn crop_grid(source: &ArcGrid, bounds: ArcBoundingBox) -> Result<ArcGrid, ArcInterpreterError> {
    let width = bounds.max_x - bounds.min_x + 1;
    let height = bounds.max_y - bounds.min_y + 1;
    let mut cells = Vec::with_capacity(usize::from(width) * usize::from(height));
    for y in bounds.min_y..=bounds.max_y {
        for x in bounds.min_x..=bounds.max_x {
            cells.push(source.cell(x, y).unwrap_or_default());
        }
    }
    Ok(ArcGrid::new(width, height, cells)?)
}

fn select_objects(source: &ArcGrid, selector: &ArcObjectSelector) -> Vec<SelectedObject> {
    let graph = extract_relation_graph(source);
    let objects = graph
        .objects
        .into_iter()
        .map(selected_object_from_arc)
        .collect::<Vec<_>>();

    match selector {
        ArcObjectSelector::All => objects,
        ArcObjectSelector::ByColor { color } => objects
            .into_iter()
            .filter(|object| object.color == *color)
            .collect(),
        ArcObjectSelector::Largest => select_extreme(objects, true, cell_count_key),
        ArcObjectSelector::Smallest => select_extreme(objects, false, cell_count_key),
        ArcObjectSelector::TopLeft => select_extreme(objects, false, top_left_key),
        ArcObjectSelector::BottomRight => select_extreme(objects, true, bottom_right_key),
    }
}

fn select_extreme<T>(
    objects: Vec<SelectedObject>,
    largest: bool,
    key_fn: fn(&SelectedObject) -> T,
) -> Vec<SelectedObject>
where
    T: Ord + Clone,
{
    let Some(best_key) = objects.iter().map(key_fn).reduce(|left, right| {
        if largest {
            left.max(right)
        } else {
            left.min(right)
        }
    }) else {
        return Vec::new();
    };

    objects
        .into_iter()
        .filter(|object| key_fn(object) == best_key)
        .collect()
}

fn selected_object_from_arc(object: ArcObject) -> SelectedObject {
    let color = object
        .color_histogram
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(index, _)| index as u8)
        .unwrap_or_default();
    let points = object_points(&object);
    SelectedObject {
        object,
        color,
        points,
    }
}

fn object_points(object: &ArcObject) -> Vec<(u8, u8)> {
    let bbox_width = object.bbox.max_x - object.bbox.min_x + 1;
    let bbox_height = object.bbox.max_y - object.bbox.min_y + 1;
    let mut points = Vec::new();
    for y in 0..bbox_height {
        for x in 0..bbox_width {
            if object.mask.is_set(x, y) {
                points.push((object.bbox.min_x + x, object.bbox.min_y + y));
            }
        }
    }
    points
}

fn transformed_points(
    object: &SelectedObject,
    transform: ArcObjectTransform,
) -> Result<Vec<(i16, i16)>, ArcInterpreterError> {
    let bbox = object.object.bbox;
    let width = i16::from(bbox.max_x - bbox.min_x + 1);
    let height = i16::from(bbox.max_y - bbox.min_y + 1);

    object
        .points
        .iter()
        .map(|(x, y)| {
            let relative_x = i16::from(*x) - i16::from(bbox.min_x);
            let relative_y = i16::from(*y) - i16::from(bbox.min_y);
            Ok(match transform {
                ArcObjectTransform::Identity => (i16::from(*x), i16::from(*y)),
                ArcObjectTransform::Translate { dx, dy } => {
                    (i16::from(*x) + i16::from(dx), i16::from(*y) + i16::from(dy))
                }
                ArcObjectTransform::ReflectHorizontal => (
                    i16::from(bbox.min_x) + (width - 1 - relative_x),
                    i16::from(*y),
                ),
                ArcObjectTransform::ReflectVertical => (
                    i16::from(*x),
                    i16::from(bbox.min_y) + (height - 1 - relative_y),
                ),
                ArcObjectTransform::RotateQuarterTurns { quarter_turns } => {
                    if quarter_turns > 3 {
                        return Err(ArcInterpreterError::InvalidQuarterTurns(quarter_turns));
                    }

                    match quarter_turns {
                        0 => (i16::from(*x), i16::from(*y)),
                        1 => (
                            i16::from(bbox.min_x) + (height - 1 - relative_y),
                            i16::from(bbox.min_y) + relative_x,
                        ),
                        2 => (
                            i16::from(bbox.min_x) + (width - 1 - relative_x),
                            i16::from(bbox.min_y) + (height - 1 - relative_y),
                        ),
                        3 => (
                            i16::from(bbox.min_x) + relative_y,
                            i16::from(bbox.min_y) + (width - 1 - relative_x),
                        ),
                        other => return Err(ArcInterpreterError::InvalidQuarterTurns(other)),
                    }
                }
            })
        })
        .collect()
}

fn union_bbox(left: ArcBoundingBox, right: ArcBoundingBox) -> ArcBoundingBox {
    ArcBoundingBox {
        min_x: left.min_x.min(right.min_x),
        min_y: left.min_y.min(right.min_y),
        max_x: left.max_x.max(right.max_x),
        max_y: left.max_y.max(right.max_y),
    }
}

fn cell_count_key(object: &SelectedObject) -> (usize, (u8, u8)) {
    (
        object.points.len(),
        (object.object.bbox.min_y, object.object.bbox.min_x),
    )
}

fn top_left_key(object: &SelectedObject) -> (u8, u8, usize) {
    (
        object.object.bbox.min_y,
        object.object.bbox.min_x,
        object.points.len(),
    )
}

fn bottom_right_key(object: &SelectedObject) -> (u8, u8, usize) {
    (
        object.object.bbox.max_y,
        object.object.bbox.max_x,
        object.points.len(),
    )
}
