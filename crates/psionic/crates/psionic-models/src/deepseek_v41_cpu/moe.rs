//! Text MoE equations from `model.py:794-906`, with explicit arithmetic modes.

use super::{CheckpointMatrix, CpuError, MatrixRows, finite, matvec, positive, shape, sigmoid};

#[derive(Clone, Copy, Debug)]
pub struct Router {
    pub top_k: usize,
    pub temperature: f32,
    pub normalize: bool,
    pub route_scale: f32,
}

impl Router {
    pub fn flash() -> Self {
        Self {
            top_k: 6,
            temperature: 1.0,
            normalize: true,
            route_scale: 1.5,
        }
    }

    /// Bias selects experts; only unbiased sqrt(softplus(logit)) sets weights.
    /// Exact ties choose the lower expert ID. Official GPU tie order is not
    /// specified by torch.topk and must be checked separately with the oracle.
    pub fn select(&self, logits: &[f32], bias: &[f32]) -> Result<Routing, CpuError> {
        if self.top_k == 0
            || self.top_k > logits.len()
            || !positive(self.temperature)
            || !positive(self.route_scale)
        {
            return Err(CpuError::Invalid("router configuration"));
        }
        finite("router logits", logits)?;
        shape("router bias", bias, logits.len())?;
        let mut scores = Vec::with_capacity(logits.len());
        for x in logits {
            let x = x / self.temperature;
            if !x.is_finite() {
                return Err(CpuError::NonFinite("router scaled logits"));
            }
            // PyTorch softplus uses the identity above its default threshold 20.
            scores.push(if x > 20.0 {
                x.sqrt()
            } else {
                x.exp().ln_1p().sqrt()
            });
        }
        let ranked: Vec<_> = scores.iter().zip(bias).map(|(s, b)| s + b).collect();
        finite("biased router scores", &ranked)?;
        let mut indices: Vec<_> = (0..scores.len()).collect();
        indices.sort_unstable_by(|a, b| ranked[*b].total_cmp(&ranked[*a]).then_with(|| a.cmp(b)));
        indices.truncate(self.top_k);
        let mut weights: Vec<_> = indices.iter().map(|i| scores[*i]).collect();
        if self.normalize && self.top_k > 1 {
            let sum = weights.iter().sum::<f32>() + 1e-20;
            if !sum.is_finite() {
                return Err(CpuError::NonFinite("router score sum"));
            }
            for w in &mut weights {
                *w /= sum;
            }
        }
        for w in &mut weights {
            *w *= self.route_scale;
        }
        finite("routing weights", &weights)?;
        Ok(Routing { indices, weights })
    }
}

#[derive(Debug, Clone)]
pub struct Routing {
    pub indices: Vec<usize>,
    pub weights: Vec<f32>,
}

/// Evaluate one expert. The route weight multiplies the intermediate activation
/// before w2, preserving the location of the reference's later BF16 boundary.
pub fn expert(
    input: &[f32],
    w1: &dyn MatrixRows,
    w2: &dyn MatrixRows,
    w3: &dyn MatrixRows,
    route_weight: f32,
    limit: f32,
) -> Result<Vec<f32>, CpuError> {
    expert_with_arithmetic(
        input,
        w1,
        w2,
        w3,
        route_weight,
        limit,
        super::quantization::Arithmetic::Float32,
    )
}

pub fn expert_with_arithmetic(
    input: &[f32],
    w1: &dyn MatrixRows,
    w2: &dyn MatrixRows,
    w3: &dyn MatrixRows,
    route_weight: f32,
    limit: f32,
    arithmetic: super::quantization::Arithmetic,
) -> Result<Vec<f32>, CpuError> {
    use super::quantization;
    let (intermediate, width) = w1.dimensions();
    if w3.dimensions() != (intermediate, width)
        || w2.dimensions() != (width, intermediate)
        || !route_weight.is_finite()
        || route_weight < 0.0
        || !limit.is_finite()
        || limit < 0.0
    {
        return Err(CpuError::Invalid("expert dimensions, weight, or clamp"));
    }
    shape("expert input", input, width)?;
    let gate = quantization::linear(w1, input, arithmetic)?;
    let up = quantization::linear(w3, input, arithmetic)?;
    let hidden: Vec<_> = gate
        .into_iter()
        .zip(up)
        .map(|(mut g, mut u)| {
            if limit > 0.0 {
                g = g.min(limit);
                u = u.clamp(-limit, limit);
            }
            (g * sigmoid(g) * u) * route_weight
        })
        .collect();
    let hidden = if arithmetic.is_bf16() {
        quantization::round_bf16(&hidden)?
    } else {
        hidden
    };
    quantization::linear(w2, &hidden, arithmetic)
}

/// Supplies routed experts by ID and the shared expert as `None`.
pub trait ExpertProvider {
    fn forward(
        &self,
        expert_id: Option<usize>,
        input: &[f32],
        route_weight: f32,
        limit: f32,
    ) -> Result<Vec<f32>, CpuError>;
}

/// Direct connection to the native safetensors loader; no eager expert load.
pub struct CheckpointExperts<'a> {
    pub checkpoint: &'a crate::deepseek_v41_weights::DeepSeekV41Checkpoint,
    pub layer: u32,
}

impl ExpertProvider for CheckpointExperts<'_> {
    fn forward(
        &self,
        expert_id: Option<usize>,
        input: &[f32],
        route_weight: f32,
        limit: f32,
    ) -> Result<Vec<f32>, CpuError> {
        let stem = match expert_id {
            Some(i) => format!("layers.{}.ffn.experts.{i}", self.layer),
            None => format!("layers.{}.ffn.shared_experts", self.layer),
        };
        let w1 = CheckpointMatrix::new(self.checkpoint, format!("{stem}.w1.weight"))?;
        let w2 = CheckpointMatrix::new(self.checkpoint, format!("{stem}.w2.weight"))?;
        let w3 = CheckpointMatrix::new(self.checkpoint, format!("{stem}.w3.weight"))?;
        expert(input, &w1, &w2, &w3, route_weight, limit)
    }
}

/// Run only selected routed experts, accumulating in expert-ID order as in the
/// reference, then add the unscaled shared expert once.
pub fn forward(
    input: &[f32],
    gate_weight: &dyn MatrixRows,
    bias: &[f32],
    router: Router,
    experts: &dyn ExpertProvider,
    limit: f32,
) -> Result<(Routing, Vec<f32>), CpuError> {
    if !limit.is_finite() || limit < 0.0 {
        return Err(CpuError::Invalid("expert clamp"));
    }
    let routing = router.select(&matvec(gate_weight, input)?, bias)?;
    let mut selected: Vec<_> = routing
        .indices
        .iter()
        .copied()
        .zip(routing.weights.iter().copied())
        .collect();
    selected.sort_unstable_by_key(|(id, _)| *id);
    let mut out = vec![0.0; input.len()];
    for (id, weight) in selected
        .into_iter()
        .map(|(id, w)| (Some(id), w))
        .chain(std::iter::once((None, 1.0)))
    {
        let y = experts.forward(id, input, weight, limit)?;
        shape("expert output", &y, out.len())?;
        for (a, b) in out.iter_mut().zip(y) {
            *a += b;
        }
    }
    finite("MoE output", &out)?;
    Ok((routing, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek_v41_cpu::MatrixRef;
    use serde::Deserialize;
    use std::cell::RefCell;

    #[derive(Deserialize)]
    struct Fixture {
        input: Vec<f32>,
        gate: Vec<f32>,
        bias: Vec<f32>,
        ids: Vec<usize>,
        weights: Vec<f32>,
        output: Vec<f32>,
        w1: Vec<Vec<f32>>,
        w2: Vec<Vec<f32>>,
        w3: Vec<Vec<f32>>,
    }
    struct Bank<'a> {
        f: &'a Fixture,
        calls: RefCell<Vec<Option<usize>>>,
    }
    impl ExpertProvider for Bank<'_> {
        fn forward(
            &self,
            id: Option<usize>,
            x: &[f32],
            w: f32,
            limit: f32,
        ) -> Result<Vec<f32>, CpuError> {
            self.calls.borrow_mut().push(id);
            let i = id.unwrap_or(8);
            expert(
                x,
                &MatrixRef::new(4, 3, &self.f.w1[i])?,
                &MatrixRef::new(3, 4, &self.f.w2[i])?,
                &MatrixRef::new(4, 3, &self.f.w3[i])?,
                w,
                limit,
            )
        }
    }

    #[test]
    fn matches_numpy_routing_and_selected_experts() -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/moe.json"))?;
        let bank = Bank {
            f: &f,
            calls: RefCell::new(Vec::new()),
        };
        let (r, out) = forward(
            &f.input,
            &MatrixRef::new(8, 3, &f.gate)?,
            &f.bias,
            Router {
                top_k: 3,
                ..Router::flash()
            },
            &bank,
            10.0,
        )?;
        assert_eq!(r.indices, f.ids);
        for (x, y) in r
            .weights
            .iter()
            .zip(&f.weights)
            .chain(out.iter().zip(&f.output))
        {
            assert!((x - y).abs() < 1e-5, "{x} != {y}");
        }
        let mut expected: Vec<_> = f.ids.iter().copied().map(Some).collect();
        expected.sort();
        expected.push(None);
        assert_eq!(*bank.calls.borrow(), expected);
        Ok(())
    }

    #[test]
    fn bias_changes_ids_but_never_combine_scores() -> Result<(), CpuError> {
        let r = Router {
            top_k: 2,
            ..Router::flash()
        }
        .select(&[4.0, 1.0, -4.0], &[-100.0, 0.0, 100.0])?;
        assert_eq!(r.indices, vec![2, 1]);
        assert!(r.weights[0] < r.weights[1]);
        assert!((r.weights.iter().sum::<f32>() - 1.5).abs() < 1e-6);
        let one = Router {
            top_k: 1,
            ..Router::flash()
        }
        .select(&[4.0, 1.0], &[0.0, 0.0])?;
        assert!((one.weights[0] - (4.0_f32.exp().ln_1p().sqrt() * 1.5)).abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn six_of_384_ties_and_extremes() -> Result<(), CpuError> {
        let r = Router::flash().select(&[0.0; 384], &[0.0; 384])?;
        assert_eq!(r.indices, vec![0, 1, 2, 3, 4, 5]);
        assert!(r.weights.iter().all(|w| (*w - 0.25).abs() < 1e-6));
        let r = Router {
            top_k: 2,
            ..Router::flash()
        }
        .select(&[-1000.0, 1000.0], &[0.0, 0.0])?;
        assert_eq!(r.weights, vec![1.5, 0.0]);
        Ok(())
    }

    #[test]
    fn gate_clamps_only_above_while_up_clamps_both_sides() -> Result<(), CpuError> {
        let w1 = MatrixRef::new(2, 1, &[-20.0, 20.0])?;
        let w3 = MatrixRef::new(2, 1, &[-30.0, 30.0])?;
        let w2 = MatrixRef::new(1, 2, &[1.0, 0.0])?;
        let y = expert(&[1.0], &w1, &w2, &w3, 0.5, 10.0)?;
        assert!((y[0] - (-20.0_f32 * sigmoid(-20.0) * -10.0 * 0.5)).abs() < 1e-10);
        let w2 = MatrixRef::new(1, 2, &[0.0, 1.0])?;
        let y = expert(&[1.0], &w1, &w2, &w3, 0.5, 10.0)?;
        assert!((y[0] - (10.0 * sigmoid(10.0) * 10.0 * 0.5)).abs() < 1e-5);
        Ok(())
    }

    #[test]
    fn invalid_router_and_matrices_are_refused() {
        assert!(Router::flash().select(&[1.0], &[0.0]).is_err());
        assert!(
            Router {
                top_k: 1,
                ..Router::flash()
            }
            .select(&[f32::NAN], &[0.0])
            .is_err()
        );
        assert!(MatrixRef::new(usize::MAX, 2, &[]).is_err());
        assert!(MatrixRef::new(1, 1, &[f32::INFINITY]).is_err());
    }
}
