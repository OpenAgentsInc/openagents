pub mod detail;
pub mod dimension;
pub mod edge_extract;
pub mod hidden_line;
pub mod projection;
pub mod section;
pub mod types;

pub use detail::create_detail_view;
pub use dimension::{
    AngleDefinition, AngularDimension, AnnotationLayer, ArrowType, DatumFeatureSymbol, DatumRef,
    DimensionStyle, FeatureControlFrame, GdtSymbol, GeometryRef, LinearDimension,
    LinearDimensionType, MaterialCondition, OrdinateDimension, RadialDimension, RenderedArc,
    RenderedArrow, RenderedDimension, RenderedText, TextAlignment, TextPlacement, ToleranceMode,
};
pub use edge_extract::{
    DEFAULT_SHARP_ANGLE, DraftingEdgeSummary, extract_drawing_edges, extract_edges,
    extract_sharp_edges, extract_silhouette_edges,
};
pub use hidden_line::{DraftingProjectionOptions, project_mesh, project_mesh_with_options};
pub use projection::{ViewMatrix, project_point, project_point_with_depth};
pub use section::{
    SectionOptions, chain_segments, generate_hatch_lines, intersect_mesh_with_plane,
    project_to_section_plane, section_mesh,
};
pub use types::{
    BoundingBox2D, DetailView, DetailViewParams, EdgeType, HatchPattern, HatchRegion, MeshEdge,
    Point2D, ProjectedEdge, ProjectedView, SectionCurve, SectionPlane, SectionView, Triangle3D,
    ViewDirection, Visibility,
};

pub const DRAFTING_TOP_LEVEL_MODULES: [&str; 7] = [
    "detail",
    "dimension",
    "edge_extract",
    "hidden_line",
    "projection",
    "section",
    "types",
];

pub const DRAFTING_PUBLIC_EXPORTS: [&str; 57] = [
    "AngleDefinition",
    "AngularDimension",
    "AnnotationLayer",
    "ArrowType",
    "BoundingBox2D",
    "DEFAULT_SHARP_ANGLE",
    "DatumFeatureSymbol",
    "DatumRef",
    "DetailView",
    "DetailViewParams",
    "DimensionStyle",
    "DraftingEdgeSummary",
    "DraftingProjectionOptions",
    "EdgeType",
    "FeatureControlFrame",
    "GdtSymbol",
    "GeometryRef",
    "HatchPattern",
    "HatchRegion",
    "LinearDimension",
    "LinearDimensionType",
    "MaterialCondition",
    "MeshEdge",
    "OrdinateDimension",
    "Point2D",
    "ProjectedEdge",
    "ProjectedView",
    "RadialDimension",
    "RenderedArc",
    "RenderedArrow",
    "RenderedDimension",
    "RenderedText",
    "SectionCurve",
    "SectionOptions",
    "SectionPlane",
    "SectionView",
    "TextAlignment",
    "TextPlacement",
    "ToleranceMode",
    "Triangle3D",
    "ViewDirection",
    "ViewMatrix",
    "Visibility",
    "chain_segments",
    "create_detail_view",
    "extract_drawing_edges",
    "extract_edges",
    "extract_sharp_edges",
    "extract_silhouette_edges",
    "generate_hatch_lines",
    "intersect_mesh_with_plane",
    "project_mesh",
    "project_mesh_with_options",
    "project_point",
    "project_point_with_depth",
    "project_to_section_plane",
    "section_mesh",
];
