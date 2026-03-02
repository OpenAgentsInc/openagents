mod angular;
mod gdt;
mod geometry_ref;
mod layer;
mod linear;
mod ordinate;
mod radial;
mod render;
mod style;

pub use angular::{AngleDefinition, AngularDimension};
pub use gdt::{DatumFeatureSymbol, DatumRef, FeatureControlFrame, GdtSymbol, MaterialCondition};
pub use geometry_ref::GeometryRef;
pub use layer::AnnotationLayer;
pub use linear::{LinearDimension, LinearDimensionType};
pub use ordinate::OrdinateDimension;
pub use radial::RadialDimension;
pub use render::{RenderedArc, RenderedArrow, RenderedDimension, RenderedText, TextAlignment};
pub use style::{ArrowType, DimensionStyle, TextPlacement, ToleranceMode};

pub const DRAFTING_DIMENSION_MODULES: [&str; 9] = [
    "angular",
    "gdt",
    "geometry_ref",
    "layer",
    "linear",
    "ordinate",
    "radial",
    "render",
    "style",
];
