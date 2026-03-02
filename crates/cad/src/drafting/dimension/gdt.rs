use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GdtSymbol {
    Straightness,
    Flatness,
    Circularity,
    Cylindricity,
    ProfileOfLine,
    ProfileOfSurface,
    Angularity,
    Perpendicularity,
    Parallelism,
    Position,
    Concentricity,
    Symmetry,
    CircularRunout,
    TotalRunout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialCondition {
    Mmc,
    Lmc,
    Rfs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatumRef {
    pub label: String,
    pub material_condition: Option<MaterialCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumFeatureSymbol {
    pub label: String,
    pub position: Point2D,
    pub leader_to: Option<GeometryRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureControlFrame {
    pub symbol: GdtSymbol,
    pub tolerance: f64,
    pub tolerance_is_diameter: bool,
    pub material_condition: Option<MaterialCondition>,
    pub datum_a: Option<DatumRef>,
    pub datum_b: Option<DatumRef>,
    pub datum_c: Option<DatumRef>,
    pub position: Point2D,
    pub leader_to: Option<GeometryRef>,
}

impl FeatureControlFrame {
    pub fn render_text(&self) -> String {
        let symbol = match self.symbol {
            GdtSymbol::Straightness => "STRAIGHT",
            GdtSymbol::Flatness => "FLAT",
            GdtSymbol::Circularity => "CIRC",
            GdtSymbol::Cylindricity => "CYL",
            GdtSymbol::ProfileOfLine => "PROF_LINE",
            GdtSymbol::ProfileOfSurface => "PROF_SURF",
            GdtSymbol::Angularity => "ANG",
            GdtSymbol::Perpendicularity => "PERP",
            GdtSymbol::Parallelism => "PAR",
            GdtSymbol::Position => "POS",
            GdtSymbol::Concentricity => "CONC",
            GdtSymbol::Symmetry => "SYM",
            GdtSymbol::CircularRunout => "RUNOUT",
            GdtSymbol::TotalRunout => "TOTAL_RUNOUT",
        };

        let tolerance_prefix = if self.tolerance_is_diameter { "D" } else { "" };
        let mut frame = format!("{symbol}|{tolerance_prefix}{:.3}", self.tolerance);
        for datum in [&self.datum_a, &self.datum_b, &self.datum_c] {
            if let Some(datum) = datum {
                frame.push('|');
                frame.push_str(&datum.label);
            }
        }
        frame
    }
}
