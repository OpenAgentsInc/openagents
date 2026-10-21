//! Current-layer CPU weight cache for the original-weight qualification run.
//! Engram tables and token embeddings retain sparse row reads. Other matrices
//! are decoded once per layer, then use the library's same per-row reductions.
use psionic_models::{
    deepseek_v41_cpu::{
        CheckpointMatrix, CpuError, MatrixRows, multiply_row,
        projection::WeightSource,
        quantization::{MatrixPrecision, round_bf16},
    },
    deepseek_v41_weights::DeepSeekV41Checkpoint,
};
use rayon::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    sync::Arc,
};
#[derive(Clone)]
struct Dense {
    rows: usize,
    cols: usize,
    precision: MatrixPrecision,
    values: Arc<Vec<f32>>,
    bf16: Option<Arc<Vec<f32>>>,
}
impl MatrixRows for Dense {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn precision(&self) -> MatrixPrecision {
        self.precision
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        if index >= self.rows {
            return Err(CpuError::Invalid("resident row index"));
        }
        Ok(self.values[index * self.cols..(index + 1) * self.cols].to_vec())
    }
    fn multiply_partitioned(
        &self,
        input: &[f32],
        bf16_weights: bool,
        block: Option<usize>,
        shards: usize,
    ) -> Result<Vec<f32>, CpuError> {
        if input.len() != self.cols {
            return Err(CpuError::Invalid("resident matrix input width"));
        }
        let values = if bf16_weights {
            self.bf16.as_ref().unwrap_or(&self.values)
        } else {
            &self.values
        };
        let cast = bf16_weights && self.precision != MatrixPrecision::Bf16 && self.bf16.is_none();
        values
            .par_chunks_exact(self.cols)
            .map(|row| multiply_row(row, input, cast, block, shards))
            .collect()
    }
    fn multiply_grouped(
        &self,
        input: &[f32],
        groups: usize,
        bf16_weights: bool,
    ) -> Result<Vec<f32>, CpuError> {
        if groups == 0
            || !self.rows.is_multiple_of(groups)
            || self.cols.checked_mul(groups) != Some(input.len())
        {
            return Err(CpuError::Invalid("resident grouped input"));
        }
        let values = if bf16_weights {
            self.bf16.as_ref().unwrap_or(&self.values)
        } else {
            &self.values
        };
        let cast = bf16_weights && self.precision != MatrixPrecision::Bf16 && self.bf16.is_none();
        values
            .par_chunks_exact(self.cols)
            .enumerate()
            .map(|(i, row)| {
                let g = i / (self.rows / groups);
                multiply_row(
                    row,
                    &input[g * self.cols..(g + 1) * self.cols],
                    cast,
                    None,
                    1,
                )
            })
            .collect()
    }
}
pub struct ResidentCheckpoint<'a> {
    checkpoint: &'a DeepSeekV41Checkpoint,
    layer: Cell<Option<usize>>,
    matrices: RefCell<BTreeMap<String, Dense>>,
    vectors: RefCell<BTreeMap<String, Vec<f32>>>,
    peak_bytes: Cell<usize>,
}
impl<'a> ResidentCheckpoint<'a> {
    pub fn new(checkpoint: &'a DeepSeekV41Checkpoint) -> Self {
        Self {
            checkpoint,
            layer: Cell::new(None),
            matrices: RefCell::new(BTreeMap::new()),
            vectors: RefCell::new(BTreeMap::new()),
            peak_bytes: Cell::new(0),
        }
    }
    fn enter(&self, name: &str) {
        let layer = name
            .strip_prefix("layers.")
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(layer) = layer {
            if self.layer.get() != Some(layer) {
                self.matrices
                    .borrow_mut()
                    .retain(|name, _| !name.starts_with("layers."));
                self.vectors
                    .borrow_mut()
                    .retain(|name, _| !name.starts_with("layers."));
                self.layer.set(Some(layer));
            }
        }
    }
    pub fn peak_decoded_matrix_bytes(&self) -> usize {
        self.peak_bytes.get()
    }
}
impl WeightSource for ResidentCheckpoint<'_> {
    fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
        self.enter(name);
        if name == "embed.weight" || name.ends_with("engram.embed.weight") {
            return Ok(Box::new(CheckpointMatrix::new(self.checkpoint, name)?));
        }
        if let Some(matrix) = self.matrices.borrow().get(name) {
            return Ok(Box::new(matrix.clone()));
        }
        let source = CheckpointMatrix::new(self.checkpoint, name)?;
        let (rows, cols) = source.dimensions();
        let elements = rows
            .checked_mul(cols)
            .ok_or(CpuError::Invalid("resident matrix size overflow"))?;
        let mut values = vec![0.0; elements];
        values.par_chunks_exact_mut(cols).enumerate().try_for_each(
            |(i, out)| -> Result<(), CpuError> {
                out.copy_from_slice(&source.row(i)?);
                Ok(())
            },
        )?;
        let bf16 = if name.ends_with("attn.wo_a.weight") {
            Some(Arc::new(round_bf16(&values)?))
        } else {
            None
        };
        let matrix = Dense {
            rows,
            cols,
            precision: source.precision(),
            values: Arc::new(values),
            bf16,
        };
        self.matrices
            .borrow_mut()
            .insert(name.to_string(), matrix.clone());
        let bytes = self
            .matrices
            .borrow()
            .values()
            .map(|m| (m.values.len() + m.bf16.as_ref().map_or(0, |v| v.len())) * 4)
            .sum();
        self.peak_bytes.set(self.peak_bytes.get().max(bytes));
        Ok(Box::new(matrix))
    }
    fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
        self.enter(name);
        if let Some(values) = self.vectors.borrow().get(name) {
            return Ok(values.clone());
        }
        let values = self.checkpoint.vector(name)?;
        self.vectors
            .borrow_mut()
            .insert(name.to_string(), values.clone());
        Ok(values)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use psionic_models::deepseek_v41_cpu::MatrixRef;
    #[test]
    fn resident_reductions_keep_order_precision_and_group_boundaries() -> Result<(), CpuError> {
        let values: Vec<_> = (0..512)
            .map(|i| ((i % 17) as f32 - 8.0) * 0.03125)
            .collect();
        let dense = Dense {
            rows: 2,
            cols: 256,
            precision: MatrixPrecision::Fp8,
            values: Arc::new(values.clone()),
            bf16: None,
        };
        let original = MatrixRef::new(2, 256, &values)?;
        let input = vec![1.0; 256];
        assert_eq!(
            dense.multiply_partitioned(&input, false, Some(32), 8)?,
            original.multiply_partitioned(&input, false, Some(32), 8)?
        );
        let input: Vec<_> = (0..512).map(|i| (i / 256 + 1) as f32).collect();
        assert_eq!(
            dense.multiply_grouped(&input, 2, true)?,
            original.multiply_grouped(&input, 2, true)?
        );
        assert!(dense.multiply_grouped(&input, 0, false).is_err());
        Ok(())
    }
}
