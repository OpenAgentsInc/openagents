use serde::{Deserialize, Serialize};

use crate::kernel_math::{Point3, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    pub fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewDirection {
    #[default]
    Front,
    Back,
    Top,
    Bottom,
    Right,
    Left,
    Isometric {
        azimuth: f64,
        elevation: f64,
    },
}

impl ViewDirection {
    pub const ISOMETRIC_STANDARD: Self = Self::Isometric {
        azimuth: std::f64::consts::FRAC_PI_6,
        elevation: std::f64::consts::FRAC_PI_6,
    };

    pub const DIMETRIC: Self = Self::Isometric {
        azimuth: 0.4636476090008061,
        elevation: 0.4636476090008061,
    };

    pub fn view_vector(self) -> Vec3 {
        match self {
            Self::Front => Vec3::new(0.0, 1.0, 0.0),
            Self::Back => Vec3::new(0.0, -1.0, 0.0),
            Self::Top => Vec3::new(0.0, 0.0, -1.0),
            Self::Bottom => Vec3::new(0.0, 0.0, 1.0),
            Self::Right => Vec3::new(1.0, 0.0, 0.0),
            Self::Left => Vec3::new(-1.0, 0.0, 0.0),
            Self::Isometric { azimuth, elevation } => {
                let cos_elev = elevation.cos();
                let sin_elev = elevation.sin();
                let cos_az = azimuth.cos();
                let sin_az = azimuth.sin();
                Vec3::new(cos_elev * sin_az, cos_elev * cos_az, -sin_elev)
            }
        }
    }

    pub fn up_vector(self) -> Vec3 {
        match self {
            Self::Front | Self::Back | Self::Right | Self::Left | Self::Isometric { .. } => {
                Vec3::z()
            }
            Self::Top => Vec3::y(),
            Self::Bottom => Vec3::new(0.0, -1.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    Sharp,
    Silhouette,
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshEdge {
    pub v0: u32,
    pub v1: u32,
    pub tri0: u32,
    pub tri1: Option<u32>,
    pub edge_type: EdgeType,
}

impl MeshEdge {
    pub fn new(v0: u32, v1: u32, tri0: u32, tri1: Option<u32>, edge_type: EdgeType) -> Self {
        let (v0, v1) = if v0 < v1 { (v0, v1) } else { (v1, v0) };
        Self {
            v0,
            v1,
            tri0,
            tri1,
            edge_type,
        }
    }

    pub fn is_boundary(self) -> bool {
        self.tri1.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedEdge {
    pub start: Point2D,
    pub end: Point2D,
    pub visibility: Visibility,
    pub edge_type: EdgeType,
    pub depth: f64,
}

impl ProjectedEdge {
    pub fn new(
        start: Point2D,
        end: Point2D,
        visibility: Visibility,
        edge_type: EdgeType,
        depth: f64,
    ) -> Self {
        Self {
            start,
            end,
            visibility,
            edge_type,
            depth,
        }
    }

    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox2D {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl BoundingBox2D {
    pub fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    pub fn include_point(&mut self, point: Point2D) {
        self.min_x = self.min_x.min(point.x);
        self.min_y = self.min_y.min(point.y);
        self.max_x = self.max_x.max(point.x);
        self.max_y = self.max_y.max(point.y);
    }

    pub fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(self) -> f64 {
        self.max_y - self.min_y
    }

    pub fn is_valid(self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
            && self.min_x <= self.max_x
            && self.min_y <= self.max_y
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedView {
    pub edges: Vec<ProjectedEdge>,
    pub bounds: BoundingBox2D,
    pub view_direction: ViewDirection,
}

impl ProjectedView {
    pub fn new(view_direction: ViewDirection) -> Self {
        Self {
            edges: Vec::new(),
            bounds: BoundingBox2D::empty(),
            view_direction,
        }
    }

    pub fn add_edge(&mut self, edge: ProjectedEdge) {
        self.bounds.include_point(edge.start);
        self.bounds.include_point(edge.end);
        self.edges.push(edge);
    }

    pub fn visible_edges(&self) -> impl Iterator<Item = &ProjectedEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.visibility == Visibility::Visible)
    }

    pub fn hidden_edges(&self) -> impl Iterator<Item = &ProjectedEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.visibility == Visibility::Hidden)
    }

    pub fn num_visible(&self) -> usize {
        self.visible_edges().count()
    }

    pub fn num_hidden(&self) -> usize {
        self.hidden_edges().count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Triangle3D {
    pub a: Point3,
    pub b: Point3,
    pub c: Point3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SectionPlane {
    pub origin: Point3,
    pub normal: Vec3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionCurve {
    pub points: Vec<Point2D>,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HatchPattern {
    pub spacing: f64,
    pub angle_radians: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HatchRegion {
    pub bounds: BoundingBox2D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionView {
    pub curves: Vec<SectionCurve>,
    pub hatch_lines: Vec<(Point2D, Point2D)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailViewParams {
    pub center: Point2D,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailView {
    pub edges: Vec<ProjectedEdge>,
    pub bounds: BoundingBox2D,
    pub scale: f64,
    pub label: String,
}
