//! Bounded authored animation graphs and local-space layered pose evaluation.
use crate::{
    animation::{self, Local},
    assets::Model,
    motion::State,
};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Parameter {
    Boolean {
        name: String,
        default: bool,
    },
    Scalar {
        name: String,
        min: f32,
        max: f32,
        default: f32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Value {
    Boolean(bool),
    Scalar(f32),
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Weight {
    Constant { value: f32 },
    Parameter { parameter: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Greater,
    Less,
    AtLeast,
    AtMost,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    Boolean {
        parameter: usize,
        value: bool,
    },
    Scalar {
        parameter: usize,
        comparison: Comparison,
        value: f32,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendSample {
    pub at: f32,
    pub node: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerMode {
    Normal,
    Additive { reference: usize },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Clip {
        state: State,
        rate: f32,
    },
    Blend1d {
        parameter: usize,
        samples: Vec<BlendSample>,
    },
    Layer {
        base: usize,
        layer: usize,
        mode: LayerMode,
        weight: Weight,
        mask: Vec<f32>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub target: usize,
    pub seconds: f32,
    pub conditions: Vec<Condition>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphState {
    pub name: String,
    pub node: usize,
    pub transitions: Vec<Transition>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub parameters: Vec<Parameter>,
    pub nodes: Vec<Node>,
    pub states: Vec<GraphState>,
    pub initial: usize,
}
fn name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn range(min: f32, max: f32, value: f32) -> bool {
    min.is_finite()
        && max.is_finite()
        && min >= -1_000_000.
        && max <= 1_000_000.
        && min <= max
        && value.is_finite()
        && (min..=max).contains(&value)
}
fn blend_pair(samples: &[BlendSample], value: f32) -> (usize, usize, f32) {
    let low = samples
        .partition_point(|sample| sample.at <= value)
        .saturating_sub(1);
    let a = &samples[low];
    let b = samples.get(low + 1).unwrap_or(a);
    let weight = if a.at == b.at {
        0.
    } else {
        ((value - a.at) / (b.at - a.at)).clamp(0., 1.)
    };
    (a.node, b.node, weight)
}
impl Graph {
    /// Dependencies must precede their consumers. This bounds evaluation and
    /// rejects cycles without recursively traversing user-supplied nodes.
    pub fn validate(&self, model: &Model) -> Result<(), String> {
        if self.parameters.len() > 64
            || self.nodes.is_empty()
            || self.nodes.len() > 256
            || self.states.is_empty()
            || self.states.len() > 64
            || self.initial >= self.states.len()
            || model.bones.is_empty()
            || model.bones.len() > 256
        {
            return Err("Animation graph capacity or initial state is invalid".into());
        }
        self.validate_controls()?;
        model.validate_animation()?;
        for (i, node) in self.nodes.iter().enumerate() {
            match node {
                Node::Clip { state, rate } => {
                    animation::resolve(model, (*state).into())?;
                    if !rate.is_finite() || !(0.01..=4.).contains(rate) {
                        return Err("Invalid animation clip rate".into());
                    }
                }
                Node::Blend1d { parameter, samples } => {
                    self.scalar(*parameter)?;
                    if !(2..=32).contains(&samples.len())
                        || samples
                            .iter()
                            .any(|s| !s.at.is_finite() || s.at.abs() > 1_000_000. || s.node >= i)
                        || samples.windows(2).any(|s| s[0].at >= s[1].at)
                    {
                        return Err("Invalid animation blend samples or dependency".into());
                    }
                }
                Node::Layer {
                    base,
                    layer,
                    mode,
                    weight,
                    mask,
                } => {
                    if *base >= i
                        || *layer >= i
                        || matches!(mode, LayerMode::Additive { reference } if *reference >= i)
                        || mask.len() != model.bones.len()
                        || mask
                            .iter()
                            .any(|w| !w.is_finite() || !(0. ..=1.).contains(w))
                    {
                        return Err("Invalid animation layer dependencies or skeleton mask".into());
                    }
                    match weight {
                        Weight::Constant { value }
                            if value.is_finite() && (0. ..=1.).contains(value) => {}
                        Weight::Parameter { parameter } => {
                            let (min, max) = self.scalar(*parameter)?;
                            if min < 0. || max > 1. {
                                return Err(
                                    "Animation layer parameter must stay between zero and one"
                                        .into(),
                                );
                            }
                        }
                        _ => return Err("Invalid animation layer weight".into()),
                    }
                }
            }
        }
        Ok(())
    }
    fn validate_controls(&self) -> Result<(), String> {
        if self.parameters.len() > 64
            || self.nodes.is_empty()
            || self.nodes.len() > 256
            || self.states.is_empty()
            || self.states.len() > 64
            || self.initial >= self.states.len()
        {
            return Err("Animation graph capacity or initial state is invalid".into());
        }
        let mut names = BTreeSet::new();
        for parameter in &self.parameters {
            let name = match parameter {
                Parameter::Boolean { name, .. } => name,
                Parameter::Scalar {
                    name,
                    min,
                    max,
                    default,
                } => {
                    if !range(*min, *max, *default) {
                        return Err("Invalid animation parameter range".into());
                    }
                    name
                }
            };
            if !name_valid(name) || !names.insert(name) {
                return Err("Invalid or duplicate animation parameter name".into());
            }
        }
        names.clear();
        for state in &self.states {
            if !name_valid(&state.name)
                || !names.insert(&state.name)
                || state.node >= self.nodes.len()
                || state.transitions.len() > 32
            {
                return Err("Invalid animation graph state".into());
            }
            for edge in &state.transitions {
                if edge.target >= self.states.len()
                    || !edge.seconds.is_finite()
                    || !(0. ..=2.).contains(&edge.seconds)
                    || edge.conditions.is_empty()
                    || edge.conditions.len() > 16
                {
                    return Err("Invalid animation graph transition".into());
                }
                for condition in &edge.conditions {
                    match condition {
                        Condition::Boolean { parameter, .. } => {
                            if !matches!(
                                self.parameters.get(*parameter),
                                Some(Parameter::Boolean { .. })
                            ) {
                                return Err(
                                    "Animation condition requires a boolean parameter".into()
                                );
                            }
                        }
                        Condition::Scalar {
                            parameter, value, ..
                        } => {
                            let (min, max) = self.scalar(*parameter)?;
                            if !range(min, max, *value) {
                                return Err("Invalid animation condition threshold".into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn scalar(&self, index: usize) -> Result<(f32, f32), String> {
        match self.parameters.get(index) {
            Some(Parameter::Scalar { min, max, .. }) => Ok((*min, *max)),
            _ => Err("Animation node requires a scalar parameter".into()),
        }
    }
    pub fn defaults(&self) -> Result<Vec<Value>, String> {
        self.validate_controls()?;
        Ok(self
            .parameters
            .iter()
            .map(|p| match p {
                Parameter::Boolean { default, .. } => Value::Boolean(*default),
                Parameter::Scalar { default, .. } => Value::Scalar(*default),
            })
            .collect())
    }
    pub fn validate_values(&self, values: &[Value]) -> Result<(), String> {
        if self.parameters.len() > 64 || values.len() != self.parameters.len() {
            return Err("Animation parameter count differs from the graph".into());
        }
        for (parameter, value) in self.parameters.iter().zip(values) {
            match (parameter, value) {
                (Parameter::Boolean { .. }, Value::Boolean(_)) => {}
                (Parameter::Scalar { min, max, .. }, Value::Scalar(value))
                    if range(*min, *max, *value) => {}
                _ => return Err("Animation parameter type or value is invalid".into()),
            }
        }
        Ok(())
    }
    /// Chooses the first matching authored edge. At most one transition is
    /// selected per call, including zero-duration edges and cyclic state graphs.
    pub fn transition<'a>(
        &'a self,
        state: usize,
        values: &[Value],
    ) -> Result<Option<&'a Transition>, String> {
        self.validate_controls()?;
        self.validate_values(values)?;
        let state = self
            .states
            .get(state)
            .ok_or("Missing animation graph state")?;
        for edge in &state.transitions {
            let mut matched = true;
            for condition in &edge.conditions {
                matched &= match condition {
                    Condition::Boolean { parameter, value } => {
                        values.get(*parameter) == Some(&Value::Boolean(*value))
                    }
                    Condition::Scalar {
                        parameter,
                        comparison,
                        value,
                    } => {
                        let Some(Value::Scalar(actual)) = values.get(*parameter) else {
                            return Err("Invalid scalar animation condition".into());
                        };
                        match comparison {
                            Comparison::Greater => actual > value,
                            Comparison::Less => actual < value,
                            Comparison::AtLeast => actual >= value,
                            Comparison::AtMost => actual <= value,
                        }
                    }
                };
            }
            if matched {
                return Ok(Some(edge));
            }
        }
        Ok(None)
    }
    /// Samples one authored state. State-machine time and life ownership are
    /// supplied by the playback adapter; this function advances no game state.
    pub fn sample(
        &self,
        model: &Model,
        state: usize,
        values: &[Value],
        seconds: f32,
    ) -> Result<Vec<Mat4>, String> {
        self.validate(model)?;
        self.validate_values(values)?;
        if !seconds.is_finite() || !(0. ..=1_000_000.).contains(&seconds) {
            return Err("Invalid animation graph sample time".into());
        }
        let root = self
            .states
            .get(state)
            .ok_or("Missing animation graph state")?
            .node;
        let locals = self.locals(model, root, values, seconds)?;
        let result = animation::matrices(model, &locals);
        if result.iter().any(|matrix| !matrix.is_finite()) {
            return Err("Animation hierarchy produced a nonfinite matrix".into());
        }
        Ok(result)
    }
    fn locals(
        &self,
        model: &Model,
        root: usize,
        values: &[Value],
        seconds: f32,
    ) -> Result<Vec<Local>, String> {
        let scalar = |index| match values[index] {
            Value::Scalar(v) => v,
            _ => unreachable!(),
        };
        let mut needed = vec![false; root + 1];
        needed[root] = true;
        for i in (0..=root).rev() {
            if !needed[i] {
                continue;
            }
            match &self.nodes[i] {
                Node::Clip { .. } => {}
                Node::Blend1d { parameter, samples } => {
                    let (a, b, weight) = blend_pair(samples, scalar(*parameter));
                    if weight < 1. {
                        needed[a] = true;
                    }
                    if weight > 0. {
                        needed[b] = true;
                    }
                }
                Node::Layer {
                    base,
                    layer,
                    mode,
                    weight,
                    mask,
                } => {
                    needed[*base] = true;
                    let weight = match weight {
                        Weight::Constant { value } => *value,
                        Weight::Parameter { parameter } => scalar(*parameter),
                    };
                    if weight > 0. && mask.iter().any(|weight| *weight > 0.) {
                        needed[*layer] = true;
                        if let LayerMode::Additive { reference } = mode {
                            needed[*reference] = true;
                        }
                    }
                }
            }
        }
        let mut poses: Vec<Vec<Local>> = Vec::with_capacity(root + 1);
        for (i, node) in self.nodes[..=root].iter().enumerate() {
            if !needed[i] {
                poses.push(Vec::new());
                continue;
            }
            let mut pose = match node {
                Node::Clip { state, rate } => animation::sample(
                    model,
                    animation::resolve(model, (*state).into())?,
                    seconds * rate,
                ),
                Node::Blend1d { parameter, samples } => {
                    let (a, b, weight) = blend_pair(samples, scalar(*parameter));
                    if weight == 0. {
                        poses[a].clone()
                    } else if weight == 1. {
                        poses[b].clone()
                    } else {
                        poses[a]
                            .iter()
                            .zip(&poses[b])
                            .map(|(a, b)| blend(*a, *b, weight))
                            .collect()
                    }
                }
                Node::Layer {
                    base,
                    layer,
                    mode,
                    weight,
                    mask,
                } => {
                    let weight = match weight {
                        Weight::Constant { value } => *value,
                        Weight::Parameter { parameter } => scalar(*parameter),
                    };
                    if weight == 0. || mask.iter().all(|weight| *weight == 0.) {
                        poses[*base].clone()
                    } else {
                        poses[*base]
                            .iter()
                            .zip(&poses[*layer])
                            .enumerate()
                            .map(|(i, (base, layer))| {
                                let weight = weight * mask[i];
                                if weight == 0. {
                                    return Ok(*base);
                                }
                                match mode {
                                    LayerMode::Normal => Ok(blend(*base, *layer, weight)),
                                    LayerMode::Additive { reference } => {
                                        additive(*base, *layer, poses[*reference][i], weight)
                                    }
                                }
                            })
                            .collect::<Result<Vec<_>, String>>()?
                    }
                }
            };
            if pose.len() != model.bones.len()
                || pose.iter().any(|p| {
                    !p.translation.is_finite() || !p.rotation.is_finite() || !p.scale.is_finite()
                })
            {
                return Err("Animation graph produced an invalid local pose".into());
            }
            for local in &mut pose {
                if !local.rotation.length_squared().is_finite()
                    || local.rotation.length_squared() < 1e-8
                {
                    return Err("Animation graph produced an invalid quaternion".into());
                }
                local.rotation = local.rotation.normalize();
            }
            poses.push(pose);
        }
        Ok(poses.pop().unwrap())
    }
}
fn blend(a: Local, b: Local, weight: f32) -> Local {
    Local {
        translation: a.translation.lerp(b.translation, weight),
        rotation: a.rotation.slerp(b.rotation, weight).normalize(),
        scale: a.scale.lerp(b.scale, weight),
    }
}
fn additive(base: Local, layer: Local, reference: Local, weight: f32) -> Result<Local, String> {
    if reference.scale.abs().min_element() < 1e-8 {
        return Err("Additive animation reference scale is singular".into());
    }
    let delta = reference.rotation.inverse() * layer.rotation;
    Ok(Local {
        translation: base.translation + (layer.translation - reference.translation) * weight,
        rotation: (base.rotation * Quat::IDENTITY.slerp(delta, weight)).normalize(),
        scale: base.scale * Vec3::ONE.lerp(layer.scale / reference.scale, weight),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assets::{Bone, BoneKeys, Clip},
        motion::{Binding, Mode},
    };
    fn fixture() -> (Model, Graph) {
        let states = [(State::Idle, 0), (State::Walk, 1)]
            .into_iter()
            .map(|(state, clip)| {
                (
                    state,
                    Binding {
                        clip,
                        mode: Mode::Loop,
                        transition_seconds: 0.2,
                    },
                )
            })
            .collect();
        let model = Model {
            source: "owned-test".into(),
            source_sha256: String::new(),
            height: 2.,
            surfaces: vec![],
            attachments: vec![],
            skin: None,
            markers: vec![],
            states,
            bones: vec![
                Bone {
                    parent: -1,
                    pivot: [0.; 3],
                },
                Bone {
                    parent: 0,
                    pivot: [0.; 3],
                },
            ],
            clips: [0, 1]
                .into_iter()
                .map(|id| Clip {
                    id,
                    duration: 1.,
                    bones: vec![BoneKeys {
                        bone: 1,
                        translation: vec![(0., [id as f32 * 2., 0., 0.])],
                        rotation: vec![],
                        scale: vec![],
                    }],
                })
                .collect(),
        };
        let graph = Graph {
            parameters: vec![
                Parameter::Scalar {
                    name: "speed".into(),
                    min: 0.,
                    max: 1.,
                    default: 0.,
                },
                Parameter::Boolean {
                    name: "moving".into(),
                    default: false,
                },
            ],
            nodes: vec![
                Node::Clip {
                    state: State::Idle,
                    rate: 1.,
                },
                Node::Clip {
                    state: State::Walk,
                    rate: 1.,
                },
                Node::Blend1d {
                    parameter: 0,
                    samples: vec![
                        BlendSample { at: 0., node: 0 },
                        BlendSample { at: 1., node: 1 },
                    ],
                },
            ],
            states: vec![
                GraphState {
                    name: "locomotion".into(),
                    node: 2,
                    transitions: vec![
                        Transition {
                            target: 1,
                            seconds: 0.2,
                            conditions: vec![Condition::Boolean {
                                parameter: 1,
                                value: true,
                            }],
                        },
                        Transition {
                            target: 0,
                            seconds: 0.1,
                            conditions: vec![Condition::Scalar {
                                parameter: 0,
                                comparison: Comparison::Greater,
                                value: 0.2,
                            }],
                        },
                    ],
                },
                GraphState {
                    name: "walking".into(),
                    node: 1,
                    transitions: vec![],
                },
            ],
            initial: 0,
        };
        (model, graph)
    }
    #[test]
    fn blend_space_and_masked_layers_evaluate_before_hierarchy() {
        let (model, mut graph) = fixture();
        let values = [Value::Scalar(0.5), Value::Boolean(false)];
        let pose = graph.sample(&model, 0, &values, 0.).unwrap();
        assert!(
            pose[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X, 1e-6)
        );
        graph.nodes.push(Node::Layer {
            base: 0,
            layer: 1,
            mode: LayerMode::Normal,
            weight: Weight::Parameter { parameter: 0 },
            mask: vec![0., 1.],
        });
        graph.states[0].node = 3;
        assert!(graph.sample(&model, 0, &values, 0.).unwrap()[1].abs_diff_eq(pose[1], 1e-6));
        graph.nodes[3] = Node::Layer {
            base: 2,
            layer: 1,
            mode: LayerMode::Additive { reference: 0 },
            weight: Weight::Constant { value: 0.5 },
            mask: vec![0., 1.],
        };
        let layered = graph.sample(&model, 0, &values, 0.).unwrap();
        assert!(layered[0].abs_diff_eq(Mat4::IDENTITY, 1e-6));
        assert!(
            layered[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X * 2., 1e-6)
        );
    }
    #[test]
    fn transition_priority_is_authored_and_types_are_checked() {
        let (model, graph) = fixture();
        graph.validate(&model).unwrap();
        assert!(
            graph
                .transition(0, &graph.defaults().unwrap())
                .unwrap()
                .is_none()
        );
        let edge = graph
            .transition(0, &[Value::Scalar(0.8), Value::Boolean(true)])
            .unwrap()
            .unwrap();
        assert_eq!(edge.target, 1);
        assert!(
            graph
                .transition(0, &[Value::Boolean(true), Value::Scalar(0.8)])
                .is_err()
        );
        assert!(
            graph
                .sample(
                    &model,
                    0,
                    &[Value::Scalar(f32::NAN), Value::Boolean(false)],
                    0.
                )
                .is_err()
        );
    }
    #[test]
    fn refuses_cycles_bad_masks_missing_states_and_invalid_skeletons() {
        let (mut model, mut graph) = fixture();
        graph.nodes[2] = Node::Blend1d {
            parameter: 0,
            samples: vec![
                BlendSample { at: 0., node: 0 },
                BlendSample { at: 1., node: 2 },
            ],
        };
        assert!(graph.validate(&model).is_err());
        graph.nodes[2] = Node::Layer {
            base: 0,
            layer: 1,
            mode: LayerMode::Normal,
            weight: Weight::Constant { value: 1. },
            mask: vec![1.],
        };
        assert!(graph.validate(&model).is_err());
        graph.nodes[2] = Node::Clip {
            state: State::Death,
            rate: 1.,
        };
        assert!(graph.validate(&model).is_err());
        graph.nodes[2] = Node::Clip {
            state: State::Idle,
            rate: 1.,
        };
        model.bones[1].parent = 1;
        assert!(
            graph
                .sample(&model, 0, &graph.defaults().unwrap(), 0.)
                .is_err()
        );
    }
    #[test]
    fn additive_rotation_scale_and_singular_reference_are_explicit() {
        let reference = Local {
            translation: Vec3::ONE,
            rotation: Quat::from_rotation_z(0.3),
            scale: Vec3::splat(2.),
        };
        let layer = Local {
            translation: Vec3::splat(3.),
            rotation: reference.rotation * Quat::from_rotation_z(1.),
            scale: Vec3::splat(4.),
        };
        let base = Local {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let result = additive(base, layer, reference, 0.5).unwrap();
        assert!(result.translation.abs_diff_eq(Vec3::ONE, 1e-6));
        assert!(result.scale.abs_diff_eq(Vec3::splat(1.5), 1e-6));
        assert!(
            result
                .rotation
                .abs_diff_eq(Quat::from_rotation_z(0.5), 1e-6)
        );
        assert!(
            additive(
                base,
                layer,
                Local {
                    scale: Vec3::ZERO,
                    ..reference
                },
                1.
            )
            .is_err()
        );
    }
    #[test]
    fn inactive_layers_do_not_sample_singular_additive_references() {
        let (mut model, mut graph) = fixture();
        model.clips[0].bones[0].scale = vec![(0., [0.; 3])];
        graph.nodes.push(Node::Layer {
            base: 1,
            layer: 1,
            mode: LayerMode::Additive { reference: 0 },
            weight: Weight::Constant { value: 1. },
            mask: vec![1.; 2],
        });
        graph.nodes.push(Node::Clip {
            state: State::Walk,
            rate: 1.,
        });
        graph.states[0].node = 4;
        assert!(
            graph
                .sample(&model, 0, &graph.defaults().unwrap(), 0.)
                .is_ok()
        );
        graph.states[0].node = 3;
        assert!(
            graph
                .sample(&model, 0, &graph.defaults().unwrap(), 0.)
                .is_err()
        );
    }
    #[test]
    fn zero_weight_and_zero_mask_are_identity_without_reference_sampling() {
        let (mut model, mut graph) = fixture();
        model.clips[0].bones[0].scale = vec![(0., [0.; 3])];
        graph.nodes.push(Node::Layer {
            base: 1,
            layer: 1,
            mode: LayerMode::Additive { reference: 0 },
            weight: Weight::Constant { value: 0. },
            mask: vec![1.; 2],
        });
        graph.states[0].node = 3;
        let values = graph.defaults().unwrap();
        let base = graph.sample(&model, 1, &values, 0.).unwrap();
        assert_eq!(graph.sample(&model, 0, &values, 0.).unwrap(), base);
        graph.nodes[3] = Node::Layer {
            base: 1,
            layer: 1,
            mode: LayerMode::Additive { reference: 0 },
            weight: Weight::Constant { value: 1. },
            mask: vec![0.; 2],
        };
        assert_eq!(graph.sample(&model, 0, &values, 0.).unwrap(), base);
    }
    #[test]
    fn blend_endpoints_sample_only_the_selected_branch() {
        let (mut model, mut graph) = fixture();
        model.clips[0].bones[0].scale = vec![(0., [0.; 3])];
        graph.nodes.push(Node::Layer {
            base: 1,
            layer: 1,
            mode: LayerMode::Additive { reference: 0 },
            weight: Weight::Constant { value: 1. },
            mask: vec![1.; 2],
        });
        graph.nodes.push(Node::Blend1d {
            parameter: 0,
            samples: vec![
                BlendSample { at: 0., node: 1 },
                BlendSample { at: 1., node: 3 },
            ],
        });
        graph.states[0].node = 4;
        assert!(
            graph
                .sample(&model, 0, &graph.defaults().unwrap(), 0.)
                .is_ok()
        );
        assert!(
            graph
                .sample(&model, 0, &[Value::Scalar(1.), Value::Boolean(false)], 0.)
                .is_err()
        );
    }
    #[test]
    fn transition_refuses_invalid_targets_and_boolean_conditions() {
        let (_, mut graph) = fixture();
        graph.states[0].transitions[0].target = usize::MAX;
        assert!(
            graph
                .transition(0, &[Value::Scalar(0.5), Value::Boolean(true)])
                .is_err()
        );
        graph.states[0].transitions[0].target = 1;
        graph.states[0].transitions[0].conditions = vec![Condition::Boolean {
            parameter: 0,
            value: true,
        }];
        assert!(
            graph
                .transition(0, &[Value::Scalar(0.5), Value::Boolean(true)])
                .is_err()
        );
    }
    #[test]
    fn serialization_retains_graph_data_and_refuses_unknown_fields() {
        let (model, graph) = fixture();
        let bytes = serde_json::to_vec(&graph).unwrap();
        let restored: Graph = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored, graph);
        restored.validate(&model).unwrap();
        let mut value = serde_json::to_value(graph).unwrap();
        value["script"] = serde_json::json!("untrusted");
        assert!(serde_json::from_value::<Graph>(value).is_err());
    }
}
