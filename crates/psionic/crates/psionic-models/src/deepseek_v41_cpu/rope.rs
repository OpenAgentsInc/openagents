//! Adjacent-pair tail RoPE and YaRN, `model.py:369-407`.

use super::{CpuError, finite, positive};

#[derive(Clone, Copy, Debug)]
pub struct RopeConfig {
    pub dim: usize,
    pub base: f32,
    pub original_context: usize,
    pub factor: f32,
    pub beta_fast: f32,
    pub beta_slow: f32,
}

impl RopeConfig {
    pub fn window(dim: usize) -> Self {
        Self {
            dim,
            base: 10_000.0,
            original_context: 0,
            factor: 16.0,
            beta_fast: 32.0,
            beta_slow: 1.0,
        }
    }
    pub fn compressed(dim: usize) -> Self {
        Self {
            base: 160_000.0,
            original_context: 65_536,
            ..Self::window(dim)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rope {
    frequencies: Vec<f32>,
}

impl Rope {
    pub fn new(config: RopeConfig) -> Result<Self, CpuError> {
        let c = config;
        if c.dim == 0
            || !c.dim.is_multiple_of(2)
            || !c.base.is_finite()
            || c.base <= 1.0
            || !positive(c.factor)
            || !positive(c.beta_slow)
            || !positive(c.beta_fast)
            || c.beta_fast < c.beta_slow
        {
            return Err(CpuError::Invalid("RoPE/YaRN configuration"));
        }
        let corrected_dim = |rotations: f32| {
            c.dim as f64
                * (c.original_context as f64 / (rotations as f64 * 2.0 * std::f64::consts::PI)).ln()
                / (2.0 * (c.base as f64).ln())
        };
        let low = if c.original_context > 0 {
            corrected_dim(c.beta_fast).floor().max(0.0) as f32
        } else {
            0.0
        };
        let high = if c.original_context > 0 {
            corrected_dim(c.beta_slow).ceil().min((c.dim - 1) as f64) as f32
        } else {
            0.0
        };
        let mut frequencies = Vec::with_capacity(c.dim / 2);
        for i in 0..c.dim / 2 {
            let exponent = (2 * i) as f32 / c.dim as f32;
            // Round the scalar power to f32 before the reciprocal, matching the
            // reference tensor's f32 frequency construction.
            let mut f = ((c.base as f64).powf(exponent as f64) as f32).recip();
            if c.original_context > 0 {
                let ramp = ((i as f32 - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
                let smooth = 1.0 - ramp;
                f = f / c.factor * (1.0 - smooth) + f * smooth;
            }
            frequencies.push(f);
        }
        finite("RoPE frequencies", &frequencies)?;
        Ok(Self { frequencies })
    }
    pub fn dim(&self) -> usize {
        self.frequencies.len() * 2
    }
    pub fn frequencies(&self) -> &[f32] {
        &self.frequencies
    }
    /// Flattened `[heads, head_width]`, with an unchanged non-RoPE prefix.
    /// Frequencies/angles/rotation products use f32; output casting is separate.
    pub fn apply_tail(
        &self,
        values: &mut [f32],
        head_width: usize,
        position: usize,
        inverse: bool,
    ) -> Result<(), CpuError> {
        if head_width < self.dim()
            || values.is_empty()
            || !values.len().is_multiple_of(head_width)
            || position >= 1_048_576
        {
            return Err(CpuError::Invalid("RoPE shape or position"));
        }
        finite("RoPE input", values)?;
        let mut output = values.to_vec();
        for head in output.chunks_exact_mut(head_width) {
            for (pair, f) in head[head_width - self.dim()..]
                .chunks_exact_mut(2)
                .zip(&self.frequencies)
            {
                let angle = position as f32 * f;
                let cos = angle.cos();
                let sin = angle.sin() * if inverse { -1.0 } else { 1.0 };
                let (a, b) = (pair[0], pair[1]);
                pair[0] = a * cos - b * sin;
                pair[1] = a * sin + b * cos;
            }
        }
        finite("RoPE output", &output)?;
        values.copy_from_slice(&output);
        Ok(())
    }
}

/// A completed compression group is positioned at its first token.
pub fn group_position(last_position: usize, ratio: usize) -> Result<usize, CpuError> {
    let end = last_position
        .checked_add(1)
        .ok_or(CpuError::Invalid("compression position overflow"))?;
    if ratio == 0 || !end.is_multiple_of(ratio) {
        return Err(CpuError::Invalid("compression group is incomplete"));
    }
    Ok(end - ratio)
}

impl super::projection::AttentionProjections<'_> {
    pub fn rotated_query(
        &self,
        input: &[f32],
        position: usize,
        rope: &Rope,
    ) -> Result<super::projection::Query, CpuError> {
        if rope.dim() != self.geometry.rope_dim {
            return Err(CpuError::Invalid("projection RoPE dimensions"));
        }
        let mut q = self.query(input)?;
        rope.apply_tail(&mut q.heads, self.geometry.head_dim, position, false)?;
        Ok(q)
    }
    pub fn rotated_window_kv(
        &self,
        input: &[f32],
        position: usize,
        rope: &Rope,
    ) -> Result<Vec<f32>, CpuError> {
        if rope.dim() != self.geometry.rope_dim {
            return Err(CpuError::Invalid("projection RoPE dimensions"));
        }
        let mut kv = self.window_kv(input)?;
        rope.apply_tail(&mut kv, self.geometry.head_dim, position, false)?;
        Ok(kv)
    }
    pub fn rotated_output(
        &self,
        heads: &[f32],
        position: usize,
        rope: &Rope,
    ) -> Result<Vec<f32>, CpuError> {
        if rope.dim() != self.geometry.rope_dim {
            return Err(CpuError::Invalid("projection RoPE dimensions"));
        }
        let mut heads = heads.to_vec();
        rope.apply_tail(&mut heads, self.geometry.head_dim, position, true)?;
        self.output(&heads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Case {
        compressed: bool,
        frequencies: Vec<f32>,
        positions: Vec<usize>,
        outputs: Vec<Vec<f32>>,
    }
    #[derive(Deserialize)]
    struct Fixture {
        input: Vec<f32>,
        cases: Vec<Case>,
    }
    #[test]
    fn frequencies_and_long_position_rotations_match_numpy()
    -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/rope.json"))?;
        for c in f.cases {
            let rope = Rope::new(if c.compressed {
                RopeConfig::compressed(64)
            } else {
                RopeConfig::window(64)
            })?;
            for (a, b) in rope.frequencies().iter().zip(&c.frequencies) {
                assert!((a - b).abs() < 1e-8, "{a} != {b}");
            }
            for (pos, expected) in c.positions.into_iter().zip(c.outputs) {
                let mut x = f.input.clone();
                rope.apply_tail(&mut x, 68, pos, false)?;
                for (a, b) in x.iter().zip(expected) {
                    assert!((a - b).abs() < 2e-6, "position {pos}: {a} != {b}");
                }
                rope.apply_tail(&mut x, 68, pos, true)?;
                for (a, b) in x.iter().zip(&f.input) {
                    assert!((a - b).abs() < 2e-6);
                }
                assert_eq!(&x[..4], &f.input[..4]);
                assert_eq!(&x[68..72], &f.input[68..72]);
            }
        }
        Ok(())
    }
    #[test]
    fn adjacent_pairs_and_inverse_are_distinct_from_split_halves() -> Result<(), CpuError> {
        let rope = Rope::new(RopeConfig::window(4))?;
        let mut x = vec![9.0, 8.0, 1.0, 0.0, 1.0, 0.0];
        rope.apply_tail(&mut x, 6, 1, false)?;
        assert_eq!(&x[..2], &[9.0, 8.0]);
        assert!((x[2] - 1.0_f32.cos()).abs() < 1e-7);
        assert!((x[3] - 1.0_f32.sin()).abs() < 1e-7);
        assert!((x[4] - 0.01_f32.cos()).abs() < 1e-7);
        Ok(())
    }
    #[test]
    fn projection_rotation_and_group_position_are_wired() -> Result<(), Box<dyn std::error::Error>>
    {
        use super::super::projection::{
            AttentionProjections,
            tests::{load, tiny},
        };
        let (s, x, _) = load()?;
        let p = AttentionProjections::new(&s, 2, tiny())?;
        let rope = Rope::new(RopeConfig::compressed(2))?;
        let q = p.rotated_query(&x, 129, &rope)?;
        let mut expected = p.query(&x)?.heads;
        rope.apply_tail(&mut expected, 4, 129, false)?;
        assert_eq!(q.heads, expected);
        assert_eq!(group_position(1, 2)?, 0);
        assert_eq!(group_position(129, 2)?, 128);
        assert_eq!(group_position(128, 1)?, 128);
        assert!(group_position(128, 2).is_err());
        Ok(())
    }
    #[test]
    fn invalid_inputs_do_not_mutate_vectors() -> Result<(), CpuError> {
        assert!(Rope::new(RopeConfig::window(3)).is_err());
        assert!(
            Rope::new(RopeConfig {
                factor: 0.0,
                ..RopeConfig::compressed(64)
            })
            .is_err()
        );
        let rope = Rope::new(RopeConfig::window(4))?;
        let mut x = vec![1.0; 6];
        let original = x.clone();
        assert!(rope.apply_tail(&mut x, 3, 1, false).is_err());
        assert_eq!(x, original);
        assert!(rope.apply_tail(&mut x, 6, 1_048_576, false).is_err());
        assert_eq!(x, original);
        Ok(())
    }
}
