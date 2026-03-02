mod pattern;
mod placeholder;
mod primitives;
mod transform;

#[cfg(test)]
mod tests;

pub use pattern::{
    CircularPatternFeatureOp, CircularPatternFeatureResult, CircularPatternInstance,
    LinearPatternFeatureOp, LinearPatternFeatureResult, LinearPatternInstance,
    evaluate_circular_pattern_feature, evaluate_linear_pattern_feature,
};
pub use placeholder::{
    FILLET_PLACEHOLDER_OPERATION_KEY, FilletPlaceholderFeatureOp, FilletPlaceholderFeatureResult,
    FilletPlaceholderKind, evaluate_fillet_placeholder_feature,
};
pub use primitives::{
    BoxFeatureOp, CutHoleFeatureOp, CylinderFeatureOp, FeatureOpResult, evaluate_box_feature,
    evaluate_cut_hole_feature, evaluate_cylinder_feature,
};
pub use transform::{
    TransformFeatureOp, TransformFeatureResult, compose_transform_sequence,
    evaluate_transform_feature,
};
