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
        self.choose_transition(state, values)
    }
    fn choose_transition<'a>(
        &'a self,
        state: usize,
        values: &[Value],
    ) -> Result<Option<&'a Transition>, String> {
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
/// Versioned semantic input bindings accompany the authored graph artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authored {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locomotion: Option<crate::locomotion::Definition>,
    pub version: u16,
    pub graph: Graph,
    #[serde(deserialize_with = "read_selectors")]
    pub selectors: std::collections::BTreeMap<State, usize>,
}
fn read_selectors<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<State, usize>, D::Error> {
    struct Selectors;
    impl<'de> serde::de::Visitor<'de> for Selectors {
        type Value = std::collections::BTreeMap<State, usize>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("unique semantic animation selectors")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = std::collections::BTreeMap::new();
            while let Some((state, index)) = map.next_entry()? {
                if values.insert(state, index).is_some() {
                    return Err(serde::de::Error::custom(
                        "Duplicate semantic animation selector",
                    ));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Selectors)
}
impl Authored {
    /// Builds a candidate from semantic bindings. Asset admission validates it.
    pub fn from_bindings(model: &Model) -> Self {
        let bindings: Vec<_> = model.states.iter().collect();
        let names: Vec<String> = bindings
            .iter()
            .map(|(state, _)| {
                serde_json::to_value(state)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        let parameters = names
            .iter()
            .map(|name| Parameter::Boolean {
                name: format!("select_{name}"),
                default: false,
            })
            .collect();
        let nodes = bindings
            .iter()
            .map(|(state, _)| Node::Clip {
                state: **state,
                rate: 1.,
            })
            .collect();
        let states = bindings
            .iter()
            .enumerate()
            .map(|(i, _)| GraphState {
                name: names[i].clone(),
                node: i,
                transitions: bindings
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(j, (_, binding))| Transition {
                        target: j,
                        seconds: binding.transition_seconds,
                        conditions: vec![Condition::Boolean {
                            parameter: j,
                            value: true,
                        }],
                    })
                    .collect(),
            })
            .collect();
        Self {
            locomotion: None,
            version: 1,
            graph: Graph {
                parameters,
                nodes,
                states,
                initial: bindings
                    .iter()
                    .position(|(state, _)| **state == State::Idle)
                    .unwrap_or(0),
            },
            selectors: bindings
                .iter()
                .enumerate()
                .map(|(i, (state, _))| (**state, i))
                .collect(),
        }
    }
    /// Extends the semantic graph with a phase-synchronized speed blend.
    pub fn from_locomotion(
        model: &Model,
        mut definition: crate::locomotion::Definition,
    ) -> Result<Self, String> {
        let mut authored = Self::from_bindings(model);
        definition.speed_parameter = authored.graph.parameters.len();
        authored.graph.parameters.push(Parameter::Scalar {
            name: "locomotion_speed".into(),
            min: 0.,
            max: definition.run_speed,
            default: 0.,
        });
        let mut samples = Vec::new();
        for (state, speed) in [
            (State::Idle, 0.),
            (State::Walk, definition.walk_speed),
            (State::Run, definition.run_speed),
        ] {
            let node = authored
                .graph
                .nodes
                .iter()
                .position(
                    |node| matches!(node, Node::Clip { state: selected, .. } if *selected == state),
                )
                .ok_or("Locomotion requires idle, walk, and run clips")?;
            let binding = animation::resolve(model, state.into())?;
            let duration = model
                .clips
                .iter()
                .find(|clip| clip.id == binding.clip)
                .unwrap()
                .duration;
            authored.graph.nodes[node] = Node::Clip {
                state,
                rate: duration,
            };
            samples.push(BlendSample { at: speed, node });
        }
        let blend = authored.graph.nodes.len();
        authored.graph.nodes.push(Node::Blend1d {
            parameter: definition.speed_parameter,
            samples,
        });
        for state in [State::Idle, State::Walk, State::Run] {
            let selector = authored.selectors[&state];
            authored.graph.states[selector].node = blend;
        }
        authored.locomotion = Some(definition);
        authored.validate(model)?;
        Ok(authored)
    }
    pub fn validate(&self, model: &Model) -> Result<(), String> {
        if let Some(definition) = &self.locomotion {
            crate::locomotion::Rig::admit(model, definition)?;
            if !matches!(self.graph.parameters.get(definition.speed_parameter),
                Some(Parameter::Scalar { min, max, default, .. }) if *min == 0. && *max >= definition.run_speed && *default == 0.)
            {
                return Err("Locomotion needs an admitted speed parameter".into());
            }
        }
        if self.version != 1 || self.selectors.is_empty() || self.selectors.len() > State::ALL.len()
        {
            return Err("Invalid semantic animation graph version or selector count".into());
        }
        self.graph.validate(model)?;
        let mut slots = BTreeSet::new();
        for (state, parameter) in &self.selectors {
            if !model.states.contains_key(state)
                || !slots.insert(*parameter)
                || !matches!(
                    self.graph.parameters.get(*parameter),
                    Some(Parameter::Boolean { default: false, .. })
                )
            {
                return Err("Invalid semantic animation graph selector".into());
            }
        }
        Ok(())
    }
    pub fn values(&self, state: State) -> Result<Vec<Value>, String> {
        let mut values = self.graph.defaults()?;
        let parameter = *self
            .selectors
            .get(&state)
            .ok_or("Missing semantic animation graph selector")?;
        let value = values
            .get_mut(parameter)
            .ok_or("Invalid semantic animation graph selector index")?;
        if !matches!(value, Value::Boolean(false)) {
            return Err("Invalid semantic animation graph selector type or default".into());
        }
        *value = Value::Boolean(true);
        Ok(values)
    }
}
/// Compiled semantic selectors share one immutable motion admission.
pub struct Semantic {
    rig: Option<crate::locomotion::Rig>,
    admitted: Admitted,
    defaults: Vec<Value>,
    selectors: std::collections::BTreeMap<State, usize>,
}
impl Semantic {
    pub fn new(authored: &Authored, model: &Model) -> Result<Self, String> {
        authored.validate(model)?;
        Ok(Self {
            rig: authored
                .locomotion
                .as_ref()
                .map(|definition| crate::locomotion::Rig::admit(model, definition))
                .transpose()?,
            defaults: authored.graph.defaults()?,
            selectors: authored.selectors.clone(),
            admitted: Admitted::new(authored.graph.clone(), model)?,
        })
    }
    pub fn rig(&self) -> Option<&crate::locomotion::Rig> {
        self.rig.as_ref()
    }
    pub fn motion_values(
        &self,
        state: State,
        input: crate::locomotion::Inputs,
    ) -> Result<Vec<Value>, String> {
        let mut values = self.values(state)?;
        if let Some(rig) = &self.rig {
            let speed = if state == State::Idle {
                0.
            } else if input.reset {
                match state {
                    State::Walk => rig.definition.walk_speed,
                    State::Run => rig.definition.run_speed,
                    _ => 0.,
                }
            } else {
                input.speed
            };
            values[rig.definition.speed_parameter] =
                Value::Scalar(speed.clamp(0., rig.definition.run_speed));
        }
        Ok(values)
    }
    pub fn phase(&self, state: State, seconds: f32) -> Result<f32, String> {
        if self.rig.is_some() && matches!(state, State::Idle | State::Walk | State::Run) {
            let binding = animation::resolve(&self.admitted.model, state.into())?;
            let clip = self
                .admitted
                .model
                .clips
                .iter()
                .find(|clip| clip.id == binding.clip)
                .unwrap();
            Ok(seconds / clip.duration)
        } else {
            Ok(seconds)
        }
    }
    pub fn admitted(&self) -> &Admitted {
        &self.admitted
    }
    pub fn values(&self, state: State) -> Result<Vec<Value>, String> {
        let index = *self
            .selectors
            .get(&state)
            .ok_or("Missing semantic animation graph selector")?;
        let mut values = self.defaults.clone();
        values[index] = Value::Boolean(true);
        Ok(values)
    }
}
/// Immutable admission owns only skeletal motion data, excluding mesh geometry.
pub struct Admitted {
    id: u64,
    graph: Graph,
    model: Model,
}
static NEXT_GRAPH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
impl Admitted {
    pub(crate) fn identity(&self) -> u64 {
        self.id
    }
    pub fn new(graph: Graph, model: &Model) -> Result<Self, String> {
        graph.validate(model)?;
        let id = NEXT_GRAPH
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |id| id.checked_add(1),
            )
            .map_err(|_| "Animation graph identity exhausted")?;
        Ok(Self {
            id,
            graph,
            model: Model {
                graph: None,
                source: String::new(),
                source_sha256: String::new(),
                height: model.height,
                bones: model.bones.clone(),
                clips: model.clips.clone(),
                states: model.states.clone(),
                skin: model.skin.clone(),
                markers: model.markers.clone(),
                surfaces: Vec::new(),
                attachments: Vec::new(),
            },
        })
    }
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    /// Normal layers distribute marker ownership by mean mask weight; additive
    /// layers retain base ownership. Blend weights accumulate through the DAG.
    /// The largest contribution wins, with the lowest node index breaking ties.
    fn marker_source(&self, root: usize, values: &[Value]) -> usize {
        let mut weights = vec![0.; root + 1];
        weights[root] = 1.;
        for i in (0..=root).rev() {
            let mass = weights[i];
            if mass == 0. {
                continue;
            }
            match &self.graph.nodes[i] {
                Node::Clip { .. } => {}
                Node::Blend1d { parameter, samples } => {
                    let Value::Scalar(value) = values[*parameter] else {
                        unreachable!()
                    };
                    let (a, b, weight) = blend_pair(samples, value);
                    weights[a] += mass * (1. - weight);
                    weights[b] += mass * weight;
                }
                Node::Layer {
                    base,
                    layer,
                    mode,
                    weight,
                    mask,
                } => {
                    let effective = if matches!(mode, LayerMode::Additive { .. }) {
                        0.
                    } else {
                        let weight = match weight {
                            Weight::Constant { value } => *value,
                            Weight::Parameter { parameter } => {
                                let Value::Scalar(value) = values[*parameter] else {
                                    unreachable!()
                                };
                                value
                            }
                        };
                        weight * mask.iter().sum::<f32>() / mask.len() as f32
                    };
                    weights[*base] += mass * (1. - effective);
                    weights[*layer] += mass * effective;
                }
            }
        }
        let mut best = None;
        for (i, weight) in weights.into_iter().enumerate() {
            if matches!(self.graph.nodes[i], Node::Clip { .. })
                && best.is_none_or(|(_, old)| weight > old)
            {
                best = Some((i, weight));
            }
        }
        best.unwrap().0
    }
}
#[derive(Clone, Default)]
pub struct Playback {
    matrices: std::sync::Arc<[Mat4]>,
    graph: Option<u64>,
    life: Option<crate::core::LifeId>,
    state: usize,
    clock: f64,
    entered: f64,
    sample_time: f64,
    sampled: Option<bool>,
    phase_epoch: Option<u64>,
    changed: f64,
    duration: f32,
    from: std::sync::Arc<[Local]>,
    current: std::sync::Arc<[Local]>,
    source: Option<usize>,
    epoch: u64,
    cursor: crate::markers::Cursor,
}
#[derive(Debug)]
pub struct Frame {
    pub evaluated: bool,
    pub matrices: Vec<Mat4>,
    pub markers: Vec<crate::markers::Event>,
    pub state: usize,
    pub selection_epoch: u64,
}
impl Playback {
    /// Refusals preserve the prior state and marker cursor. Clock seeks, admitted
    /// graph replacements, and actor-life changes establish a fresh pose baseline.
    /// During crossfades the outgoing pose is frozen; target-state clips own markers.
    pub fn update(
        &mut self,
        admitted: &Admitted,
        life: crate::core::LifeId,
        values: &[Value],
        clock: f64,
    ) -> Result<Frame, String> {
        self.update_time(admitted, life, values, clock, None, None, true)
    }
    /// Uses an admitted presentation phase, such as distance-driven locomotion,
    /// while the independent clock governs transitions. Backward phases seek.
    pub fn update_sampled(
        &mut self,
        admitted: &Admitted,
        life: crate::core::LifeId,
        values: &[Value],
        sample_time: f64,
        clock: f64,
    ) -> Result<Frame, String> {
        self.update_sampled_phase(admitted, life, values, sample_time, clock, None)
    }
    /// Changes to the explicit phase owner reset markers without replaying another
    /// source's elapsed time. Graph state and transition policy remain intact.
    pub fn update_sampled_phase(
        &mut self,
        admitted: &Admitted,
        life: crate::core::LifeId,
        values: &[Value],
        sample_time: f64,
        clock: f64,
        phase_epoch: Option<u64>,
    ) -> Result<Frame, String> {
        if !sample_time.is_finite() || !(0. ..=1_000_000.).contains(&sample_time) {
            return Err("Invalid animation graph presentation phase".into());
        }
        self.update_time(
            admitted,
            life,
            values,
            clock,
            Some(sample_time),
            phase_epoch,
            true,
        )
    }
    /// Advances marker ownership every frame while reusing bounded crowd poses.
    pub fn update_quality(
        &mut self,
        admitted: &Admitted,
        life: crate::core::LifeId,
        values: &[Value],
        sample_time: f64,
        clock: f64,
        phase_epoch: Option<u64>,
        sample_pose: bool,
    ) -> Result<Frame, String> {
        if !sample_time.is_finite() || !(0. ..=1_000_000.).contains(&sample_time) {
            return Err("Invalid animation graph presentation phase".into());
        }
        self.update_time(
            admitted,
            life,
            values,
            clock,
            Some(sample_time),
            phase_epoch,
            sample_pose,
        )
    }
    fn update_time(
        &mut self,
        admitted: &Admitted,
        life: crate::core::LifeId,
        values: &[Value],
        clock: f64,
        sample_time: Option<f64>,
        phase_epoch: Option<u64>,
        sample_pose: bool,
    ) -> Result<Frame, String> {
        if !clock.is_finite() || !(0. ..=1_000_000.).contains(&clock) {
            return Err("Invalid animation graph playback clock".into());
        }
        admitted.graph.validate_values(values)?;
        let mut next = self.clone();
        let reset = next.graph != Some(admitted.id)
            || next.life != Some(life)
            || clock < next.clock
            || next.sampled != Some(sample_time.is_some());
        if reset {
            next.graph = Some(admitted.id);
            next.sampled = Some(sample_time.is_some());
            next.life = Some(life);
            next.state = admitted.graph.initial;
            next.entered = clock;
            next.current = Default::default();
            next.from = Default::default();
            next.source = None;
            next.cursor = crate::markers::Cursor::default();
        }
        let edge = admitted.graph.choose_transition(next.state, values)?;
        let transitioned = edge.is_some();
        if let Some(edge) = edge {
            next.state = edge.target;
            next.entered = clock;
            next.changed = clock;
            next.duration = edge.seconds;
            next.from = next.current.clone();
        }
        let root = admitted.graph.states[next.state].node;
        let time = sample_time.unwrap_or(clock - next.entered);
        let seeked = !reset
            && (next.phase_epoch != phase_epoch || (!transitioned && time < next.sample_time));
        let source = admitted.marker_source(root, values);
        let evaluated = sample_pose
            || reset
            || seeked
            || transitioned
            || next.current.is_empty()
            || next.source != Some(source);
        let target = if evaluated {
            admitted
                .graph
                .locals(&admitted.model, root, values, time as f32)?
        } else {
            Vec::new()
        };
        if evaluated {
            if reset || seeked || next.current.is_empty() {
                next.current = target.into();
                next.from = next.current.clone();
                next.changed = clock;
                next.duration = 0.;
            } else {
                let weight = if next.duration == 0. {
                    1.
                } else {
                    ((clock - next.changed) / f64::from(next.duration)).clamp(0., 1.) as f32
                };
                let weight = weight * weight * (3. - 2. * weight);
                next.current = if weight == 0. {
                    next.from.clone()
                } else if weight == 1. {
                    target.into()
                } else {
                    next.from
                        .iter()
                        .zip(target)
                        .map(|(a, b)| blend(*a, b, weight))
                        .collect::<Vec<_>>()
                        .into()
                };
            }
        }
        if reset || seeked || transitioned || next.source != Some(source) {
            next.epoch = next
                .epoch
                .checked_add(1)
                .ok_or("Animation graph selection epoch exhausted")?;
            next.cursor = crate::markers::Cursor::default();
        }
        next.source = Some(source);
        let Node::Clip { state, rate } = admitted.graph.nodes[source] else {
            unreachable!()
        };
        let binding = animation::resolve(&admitted.model, state.into())?;
        let markers = if let Some(track) = admitted
            .model
            .markers
            .iter()
            .find(|track| track.clip == binding.clip)
        {
            next.cursor.advance(
                &track.track,
                life,
                next.epoch,
                time * f64::from(rate),
                binding.mode == crate::motion::Mode::Loop,
            )?
        } else {
            next.cursor = crate::markers::Cursor::default();
            Vec::new()
        };
        if evaluated {
            next.matrices = animation::matrices(&admitted.model, &next.current).into();
        }
        let matrices = next.matrices.to_vec();
        if matrices.iter().any(|matrix| !matrix.is_finite()) {
            return Err("Animation graph playback produced a nonfinite hierarchy".into());
        }
        next.clock = clock;
        next.sample_time = time;
        next.phase_epoch = phase_epoch;
        let frame = Frame {
            evaluated,
            matrices,
            markers,
            state: next.state,
            selection_epoch: next.epoch,
        };
        *self = next;
        Ok(frame)
    }
}
fn blend(a: Local, b: Local, weight: f32) -> Local {
    if weight == 0. {
        return a;
    }
    if weight == 1. {
        return b;
    }
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
            graph: None,
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
    fn life(generation: u64) -> crate::core::LifeId {
        crate::core::LifeId {
            instance: 1,
            actor: 7,
            generation,
        }
    }
    fn playback_fixture() -> (Model, Graph) {
        let (mut model, _) = fixture();
        model.states.insert(
            State::Cast,
            Binding {
                clip: 2,
                mode: Mode::Hold,
                transition_seconds: 1.,
            },
        );
        model.clips.push(Clip {
            id: 2,
            duration: 1.,
            bones: vec![BoneKeys {
                bone: 1,
                translation: vec![(0., [-2., 0., 0.])],
                rotation: vec![],
                scale: vec![],
            }],
        });
        let edge = |target, value| Transition {
            target,
            seconds: 1.,
            conditions: vec![Condition::Boolean {
                parameter: 0,
                value,
            }],
        };
        let graph = Graph {
            parameters: vec![Parameter::Boolean {
                name: "moving".into(),
                default: false,
            }],
            nodes: [State::Idle, State::Walk, State::Cast]
                .into_iter()
                .map(|state| Node::Clip { state, rate: 1. })
                .collect(),
            states: vec![
                GraphState {
                    name: "idle".into(),
                    node: 0,
                    transitions: vec![edge(1, true)],
                },
                GraphState {
                    name: "walk".into(),
                    node: 1,
                    transitions: vec![edge(2, false)],
                },
                GraphState {
                    name: "cast".into(),
                    node: 2,
                    transitions: vec![edge(1, true)],
                },
            ],
            initial: 0,
        };
        (model, graph)
    }
    #[test]
    fn graph_crossfade_interrupts_from_current_pose_and_resets_exact_lives() {
        let (model, graph) = playback_fixture();
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 0.)
            .unwrap();
        playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 0.1)
            .unwrap();
        let middle = playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 0.6)
            .unwrap();
        assert!(
            middle.matrices[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X, 1e-5)
        );
        let interrupted = playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 0.6)
            .unwrap();
        assert_eq!(middle.matrices, interrupted.matrices);
        let next = playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 1.1)
            .unwrap();
        assert!(
            next.matrices[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X * -0.5, 1e-5)
        );
        let respawn = playback
            .update(&admitted, life(1), &[Value::Boolean(false)], 1.1)
            .unwrap();
        assert!(respawn.matrices[1].abs_diff_eq(Mat4::IDENTITY, 1e-5));
        assert!(respawn.selection_epoch > next.selection_epoch);
        assert!(respawn.markers.is_empty());
    }
    #[test]
    fn weighted_markers_change_source_without_replay_and_refusals_are_atomic() {
        let (mut model, mut graph) = fixture();
        for state in &mut graph.states {
            state.transitions.clear();
        }
        model.markers = [0, 1]
            .into_iter()
            .map(|clip| crate::markers::ClipTrack {
                clip,
                track: crate::markers::Track {
                    duration: 1.,
                    markers: vec![
                        crate::markers::Marker {
                            id: 1,
                            seconds: 0.25,
                        },
                        crate::markers::Marker {
                            id: 2,
                            seconds: 0.75,
                        },
                    ],
                },
            })
            .collect();
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        let values = |speed| [Value::Scalar(speed), Value::Boolean(false)];
        playback
            .update(&admitted, life(0), &values(0.4), 0.)
            .unwrap();
        let first = playback
            .update(&admitted, life(0), &values(0.4), 0.3)
            .unwrap();
        assert_eq!(first.markers.len(), 1);
        let switched = playback
            .update(&admitted, life(0), &values(0.6), 0.4)
            .unwrap();
        assert!(switched.markers.is_empty());
        assert!(switched.selection_epoch > first.selection_epoch);
        let right = playback
            .update(&admitted, life(0), &values(0.6), 0.8)
            .unwrap();
        assert_eq!(right.markers.len(), 1);
        assert_eq!(right.markers[0].marker, 2);
        let clock = playback.clock;
        let epoch = playback.epoch;
        assert!(
            playback
                .update(&admitted, life(0), &values(0.6), 1000.)
                .is_err()
        );
        assert_eq!(playback.clock, clock);
        assert_eq!(playback.epoch, epoch);
        let retry = playback
            .update(&admitted, life(0), &values(0.6), 0.9)
            .unwrap();
        assert!(retry.markers.is_empty());
        let tie = playback
            .update(&admitted, life(0), &values(0.5), 0.9)
            .unwrap();
        assert!(tie.markers.is_empty());
        assert_eq!(playback.source, Some(0));
    }
    #[test]
    fn admitted_motion_is_immutable_and_replacement_and_seek_reset_baselines() {
        let (mut model, graph) = playback_fixture();
        let admitted = Admitted::new(graph.clone(), &model).unwrap();
        model.clips[1].bones[0].translation[0].1 = [90., 0., 0.];
        assert!(admitted.model.surfaces.is_empty());
        let mut playback = Playback::default();
        let first = playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 10.)
            .unwrap();
        assert!(
            first.matrices[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X * 2., 1e-5)
        );
        let seek = playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 5.)
            .unwrap();
        assert!(seek.selection_epoch > first.selection_epoch);
        assert!(seek.markers.is_empty());
        let replacement = Admitted::new(graph, &model).unwrap();
        let replaced = playback
            .update(&replacement, life(0), &[Value::Boolean(true)], 5.)
            .unwrap();
        assert!(replaced.selection_epoch > seek.selection_epoch);
        assert!(
            replaced.matrices[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X * 90., 1e-5)
        );
    }
    #[test]
    fn distance_driven_phase_can_pause_and_seek_without_advancing_authority() {
        let (mut model, mut graph) = fixture();
        for state in &mut graph.states {
            state.transitions.clear();
        }
        model.markers.push(crate::markers::ClipTrack {
            clip: 1,
            track: crate::markers::Track {
                duration: 1.,
                markers: vec![crate::markers::Marker {
                    id: 1,
                    seconds: 0.25,
                }],
            },
        });
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        let values = [Value::Scalar(1.), Value::Boolean(false)];
        playback
            .update_sampled(&admitted, life(0), &values, 0., 100.)
            .unwrap();
        let moving = playback
            .update_sampled(&admitted, life(0), &values, 0.3, 101.)
            .unwrap();
        assert_eq!(moving.markers.len(), 1);
        let paused = playback
            .update_sampled(&admitted, life(0), &values, 0.3, 110.)
            .unwrap();
        assert!(paused.markers.is_empty());
        assert_eq!(moving.matrices, paused.matrices);
        let seek = playback
            .update_sampled(&admitted, life(0), &values, 0.1, 111.)
            .unwrap();
        assert!(seek.markers.is_empty());
        assert!(seek.selection_epoch > paused.selection_epoch);
        assert!(
            playback
                .update_sampled(&admitted, life(0), &values, f64::NAN, 112.)
                .is_err()
        );
        assert_eq!(playback.clock, 111.);
    }
    #[test]
    fn changing_phase_policy_establishes_a_marker_baseline() {
        let (mut model, mut graph) = fixture();
        for state in &mut graph.states {
            state.transitions.clear();
        }
        model.markers.push(crate::markers::ClipTrack {
            clip: 1,
            track: crate::markers::Track {
                duration: 1.,
                markers: vec![crate::markers::Marker {
                    id: 1,
                    seconds: 0.25,
                }],
            },
        });
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        let values = [Value::Scalar(1.), Value::Boolean(false)];
        playback.update(&admitted, life(0), &values, 0.).unwrap();
        let timed = playback.update(&admitted, life(0), &values, 0.3).unwrap();
        assert_eq!(timed.markers.len(), 1);
        let sampled = playback
            .update_sampled(&admitted, life(0), &values, 10.3, 0.4)
            .unwrap();
        assert!(sampled.markers.is_empty());
        assert!(sampled.selection_epoch > timed.selection_epoch);
        let timed_again = playback.update(&admitted, life(0), &values, 0.5).unwrap();
        assert!(timed_again.markers.is_empty());
        assert!(timed_again.selection_epoch > sampled.selection_epoch);
    }
    #[test]
    fn phase_owner_handoff_preserves_state_and_bounded_marker_delivery() {
        let (mut model, mut graph) = fixture();
        for state in &mut graph.states {
            state.transitions.clear();
        }
        model.markers.push(crate::markers::ClipTrack {
            clip: 1,
            track: crate::markers::Track {
                duration: 1.,
                markers: vec![crate::markers::Marker {
                    id: 1,
                    seconds: 0.25,
                }],
            },
        });
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        let values = [Value::Scalar(1.), Value::Boolean(false)];
        let local = playback
            .update_sampled_phase(&admitted, life(0), &values, 0.3, 400., Some(12))
            .unwrap();
        let authority = playback
            .update_sampled_phase(&admitted, life(0), &values, 400., 400.016, None)
            .unwrap();
        assert_eq!(authority.state, local.state);
        assert!(authority.markers.is_empty());
        assert!(authority.selection_epoch > local.selection_epoch);
        let next = playback
            .update_sampled_phase(&admitted, life(0), &values, 400.3, 400.3, None)
            .unwrap();
        assert_eq!(next.markers.len(), 1);
        assert!(
            playback
                .update_sampled_phase(&admitted, life(0), &values, 900., 401., None)
                .is_err()
        );
        assert_eq!(playback.sample_time, 400.3);
        assert_eq!(playback.clock, 400.3);
        let restored = playback
            .update_sampled_phase(&admitted, life(0), &values, 0.3, 402., Some(13))
            .unwrap();
        assert!(restored.markers.is_empty());
        assert_eq!(restored.state, next.state);
    }
    #[test]
    fn invalid_parameters_and_epoch_exhaustion_preserve_playback() {
        let (model, graph) = playback_fixture();
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 0.)
            .unwrap();
        assert!(
            playback
                .update(&admitted, life(0), &[Value::Scalar(1.)], 1.)
                .is_err()
        );
        assert_eq!(playback.clock, 0.);
        assert_eq!(playback.state, 0);
        playback.epoch = u64::MAX;
        assert!(
            playback
                .update(&admitted, life(1), &[Value::Boolean(true)], 1.)
                .is_err()
        );
        assert_eq!(playback.life, Some(life(0)));
        assert_eq!(playback.epoch, u64::MAX);
    }
    #[test]
    fn semantic_graphs_select_named_states_without_numeric_clip_ids() {
        let (mut model, _) = fixture();
        let authored = Authored::from_bindings(&model);
        authored.validate(&model).unwrap();
        let serialized = serde_json::to_vec(&authored).unwrap();
        let restored: Authored = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(restored, authored);
        model.clips[1].id = 999;
        model.states.get_mut(&State::Walk).unwrap().clip = 999;
        let semantic = Semantic::new(&restored, &model).unwrap();
        let mut playback = Playback::default();
        let frame = playback
            .update_sampled(
                semantic.admitted(),
                life(0),
                &semantic.values(State::Walk).unwrap(),
                0.3,
                100.,
            )
            .unwrap();
        assert!(
            frame.matrices[1]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::X * 2., 1e-5)
        );
        assert!(semantic.values(State::Death).is_err());
    }
    #[test]
    fn graph_pose_endpoints_preserve_exact_rotations_across_interruptions() {
        let (mut model, graph) = playback_fixture();
        model.clips[1].bones[0].rotation = vec![(0., Quat::from_rotation_y(0.731).to_array())];
        model.clips[2].bones[0].rotation = vec![(0., Quat::from_rotation_y(-0.319).to_array())];
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        let first = playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 0.)
            .unwrap();
        let held = playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 0.1)
            .unwrap();
        assert_eq!(first.matrices, held.matrices);
        let interrupted = playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 0.1)
            .unwrap();
        assert_eq!(held.matrices, interrupted.matrices);
    }
    #[test]
    fn duplicate_serialized_selector_names_are_refused() {
        let (model, _) = fixture();
        let json = serde_json::to_string(&Authored::from_bindings(&model)).unwrap();
        let duplicate = json.replace("\"idle\":0", "\"idle\":0,\"idle\":0");
        assert_ne!(json, duplicate);
        assert!(serde_json::from_str::<Authored>(&duplicate).is_err());
    }
    #[test]
    fn transactional_candidates_share_immutable_pose_buffers() {
        let (model, graph) = playback_fixture();
        let admitted = Admitted::new(graph, &model).unwrap();
        let mut playback = Playback::default();
        playback
            .update(&admitted, life(0), &[Value::Boolean(true)], 0.)
            .unwrap();
        let retained = playback.clone();
        assert!(std::sync::Arc::ptr_eq(&retained.current, &playback.current));
        assert!(std::sync::Arc::ptr_eq(&retained.from, &playback.from));
        assert!(
            playback
                .update(&admitted, life(0), &[Value::Scalar(1.)], 1.)
                .is_err()
        );
        assert!(std::sync::Arc::ptr_eq(&retained.current, &playback.current));
        playback
            .update(&admitted, life(0), &[Value::Boolean(false)], 0.1)
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(&retained.current, &playback.from));
    }
    #[test]
    fn semantic_graph_version_and_selector_aliases_are_refused() {
        let (model, _) = fixture();
        let mut authored = Authored::from_bindings(&model);
        authored.version = 2;
        assert!(authored.validate(&model).is_err());
        authored.version = 1;
        let idle = authored.selectors[&State::Idle];
        authored.selectors.insert(State::Walk, idle);
        assert!(authored.validate(&model).is_err());
        authored.selectors.insert(State::Walk, usize::MAX);
        assert!(authored.validate(&model).is_err());
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
