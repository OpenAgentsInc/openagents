//! Public admission rules for the bounded creator hook; the renderer stays private.
use aethyrforge_host::{ProjectDocument, PublishRuntime, validate_playable_values_v1};
use naga::{Block, Expression, MathFunction, ScalarKind, Statement, TypeInner, VectorSize};
fn bounded(block: &Block) -> bool {
    block.iter().all(|s| match s {
        Statement::Loop { .. } | Statement::Kill => false,
        Statement::Block(b) => bounded(b),
        Statement::If { accept, reject, .. } => bounded(accept) && bounded(reject),
        Statement::Switch { cases, .. } => cases.iter().all(|case| bounded(&case.body)),
        _ => true,
    })
}
pub(super) fn validate(d: &ProjectDocument, runtime: &PublishRuntime) -> bool {
    if d.validate().is_err()
        || !(runtime.accepts(d) || runtime.accepts_personal(d))
        || validate_playable_values_v1(d, &runtime.material_ids).is_err()
    {
        return false;
    }
    let Ok(module) = naga::front::wgsl::parse_str(&d.active_source) else {
        return false;
    };
    if naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .is_err()
        || module.functions.len() != 1
        || !module.entry_points.is_empty()
        || !module.global_variables.is_empty()
        || !module.overrides.is_empty()
        || !module.constants.is_empty()
    {
        return false;
    }
    let (_, function) = module.functions.iter().next().unwrap();
    let vector = |ty: naga::Handle<naga::Type>, size| matches!(module.types[ty].inner,TypeInner::Vector{size:s,scalar}if s==size&&scalar.kind==ScalarKind::Float&&scalar.width==4);
    if function.name.as_deref() != Some("oa_material")
        || function.arguments.len() != 5
        || function
            .arguments
            .iter()
            .zip(["base", "position", "normal", "uv", "time"])
            .any(|(a, name)| a.name.as_deref() != Some(name) || a.binding.is_some())
        || !function.arguments[..3]
            .iter()
            .all(|a| vector(a.ty, VectorSize::Tri))
        || !vector(function.arguments[3].ty, VectorSize::Bi)
        || !matches!(module.types[function.arguments[4].ty].inner,TypeInner::Scalar(s)if s.kind==ScalarKind::Float&&s.width==4)
        || function
            .result
            .as_ref()
            .is_none_or(|r| r.binding.is_some() || !vector(r.ty, VectorSize::Tri))
        || !bounded(&function.body)
        || module
            .types
            .iter()
            .any(|(_, t)| !matches!(t.inner, TypeInner::Scalar(_) | TypeInner::Vector { .. }))
    {
        return false;
    }
    function.expressions.iter().all(|(_, e)| match e {
        Expression::Math { fun, .. } => matches!(
            fun,
            MathFunction::Sin
                | MathFunction::Cos
                | MathFunction::Abs
                | MathFunction::Min
                | MathFunction::Max
                | MathFunction::Clamp
                | MathFunction::Mix
                | MathFunction::SmoothStep
                | MathFunction::Step
                | MathFunction::Fract
                | MathFunction::Floor
                | MathFunction::Ceil
                | MathFunction::Pow
                | MathFunction::Sqrt
                | MathFunction::Dot
                | MathFunction::Length
                | MathFunction::Normalize
        ),
        Expression::Derivative { .. } | Expression::Relational { .. } => false,
        _ => true,
    })
}
