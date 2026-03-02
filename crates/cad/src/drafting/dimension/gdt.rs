use serde::{Deserialize, Serialize};

use super::geometry_ref::GeometryRef;
use super::render::{RenderedDimension, RenderedText, TextAlignment};
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

impl GdtSymbol {
    pub fn render_token(self) -> &'static str {
        match self {
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
        }
    }

    pub fn dxf_text(self) -> &'static str {
        match self {
            GdtSymbol::Straightness => "%%c-",
            GdtSymbol::Flatness => "%%cF",
            GdtSymbol::Circularity => "%%c",
            GdtSymbol::Cylindricity => "%%cC",
            GdtSymbol::ProfileOfLine => "%%cL",
            GdtSymbol::ProfileOfSurface => "%%cS",
            GdtSymbol::Angularity => "%%cA",
            GdtSymbol::Perpendicularity => "%%cP",
            GdtSymbol::Parallelism => "//",
            GdtSymbol::Position => "%%cPOS",
            GdtSymbol::Concentricity => "(O)",
            GdtSymbol::Symmetry => "=",
            GdtSymbol::CircularRunout => "%%cR",
            GdtSymbol::TotalRunout => "%%cRR",
        }
    }

    pub fn requires_datum(self) -> bool {
        matches!(
            self,
            GdtSymbol::Angularity
                | GdtSymbol::Perpendicularity
                | GdtSymbol::Parallelism
                | GdtSymbol::Position
                | GdtSymbol::Concentricity
                | GdtSymbol::Symmetry
                | GdtSymbol::CircularRunout
                | GdtSymbol::TotalRunout
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialCondition {
    Mmc,
    Lmc,
    Rfs,
}

impl MaterialCondition {
    pub fn dxf_text(self) -> &'static str {
        match self {
            MaterialCondition::Mmc => "(M)",
            MaterialCondition::Lmc => "(L)",
            MaterialCondition::Rfs => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatumRef {
    pub label: String,
    pub material_condition: Option<MaterialCondition>,
}

impl DatumRef {
    pub fn render_text(&self) -> String {
        let mut text = self.label.clone();
        if let Some(condition) = self.material_condition {
            text.push_str(condition.dxf_text());
        }
        text
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumFeatureSymbol {
    pub label: String,
    pub position: Point2D,
    pub leader_to: Option<GeometryRef>,
}

impl DatumFeatureSymbol {
    pub fn render(&self) -> RenderedDimension {
        let mut rendered = RenderedDimension::default();
        let triangle_height = 4.0;
        let half_base = 2.4;
        let top = Point2D::new(self.position.x, self.position.y + triangle_height * 0.5);
        let left = Point2D::new(
            self.position.x - half_base,
            self.position.y - triangle_height * 0.5,
        );
        let right = Point2D::new(
            self.position.x + half_base,
            self.position.y - triangle_height * 0.5,
        );
        rendered
            .lines
            .extend([(top, left), (left, right), (right, top)]);
        rendered.texts.push(RenderedText {
            text: self.label.clone(),
            position: Point2D::new(self.position.x, self.position.y - 0.5),
            alignment: TextAlignment::Center,
        });

        if let Some(anchor) = self.leader_to.as_ref().and_then(geometry_anchor) {
            rendered.lines.push((self.position, anchor));
        }
        rendered
    }
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
    pub fn datum_requirement_satisfied(&self) -> bool {
        !self.symbol.requires_datum() || self.datum_a.is_some()
    }

    pub fn render_text(&self) -> String {
        let tolerance_prefix = if self.tolerance_is_diameter { "D" } else { "" };
        let mut frame = format!(
            "{}|{tolerance_prefix}{:.3}",
            self.symbol.render_token(),
            self.tolerance
        );
        if let Some(condition) = self.material_condition {
            frame.push_str(condition.dxf_text());
        }
        for datum in [&self.datum_a, &self.datum_b, &self.datum_c] {
            if let Some(datum) = datum {
                frame.push('|');
                frame.push_str(&datum.render_text());
            }
        }
        frame
    }

    pub fn render(&self) -> RenderedDimension {
        const CELL_WIDTH: f64 = 8.0;
        const CELL_HEIGHT: f64 = 4.0;

        let cells: Vec<String> = self
            .render_text()
            .split('|')
            .map(|cell| cell.to_string())
            .collect();
        let cell_count = cells.len().max(1);
        let width = CELL_WIDTH * cell_count as f64;
        let left = self.position.x - width * 0.5;
        let right = left + width;
        let bottom = self.position.y - CELL_HEIGHT * 0.5;
        let top = self.position.y + CELL_HEIGHT * 0.5;

        let mut rendered = RenderedDimension::default();
        rendered.lines.extend([
            (Point2D::new(left, bottom), Point2D::new(right, bottom)),
            (Point2D::new(right, bottom), Point2D::new(right, top)),
            (Point2D::new(right, top), Point2D::new(left, top)),
            (Point2D::new(left, top), Point2D::new(left, bottom)),
        ]);

        for divider in 1..cell_count {
            let x = left + CELL_WIDTH * divider as f64;
            rendered
                .lines
                .push((Point2D::new(x, bottom), Point2D::new(x, top)));
        }

        rendered
            .texts
            .extend(cells.iter().enumerate().map(|(index, text)| RenderedText {
                text: text.clone(),
                position: Point2D::new(left + CELL_WIDTH * (index as f64 + 0.5), self.position.y),
                alignment: TextAlignment::Center,
            }));

        if let Some(anchor) = self.leader_to.as_ref().and_then(geometry_anchor) {
            rendered
                .lines
                .push((Point2D::new(self.position.x, bottom), anchor));
        }

        rendered
    }
}

fn geometry_anchor(reference: &GeometryRef) -> Option<Point2D> {
    match reference {
        GeometryRef::Point(point) => Some(*point),
        GeometryRef::Edge { start, end } => Some(Point2D::new(
            (start.x + end.x) * 0.5,
            (start.y + end.y) * 0.5,
        )),
        GeometryRef::Circle { center, .. } => Some(*center),
        GeometryRef::VertexIndex(_) | GeometryRef::EdgeIndex(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DatumFeatureSymbol, DatumRef, FeatureControlFrame, GdtSymbol, MaterialCondition};
    use crate::drafting::{GeometryRef, Point2D};

    #[test]
    fn gdt_symbol_datum_requirement_matches_contract() {
        assert!(GdtSymbol::Position.requires_datum());
        assert!(GdtSymbol::Perpendicularity.requires_datum());
        assert!(!GdtSymbol::Flatness.requires_datum());
        assert!(!GdtSymbol::Circularity.requires_datum());
    }

    #[test]
    fn feature_control_frame_render_text_includes_modifiers() {
        let frame = FeatureControlFrame {
            symbol: GdtSymbol::Position,
            tolerance: 0.125,
            tolerance_is_diameter: true,
            material_condition: Some(MaterialCondition::Mmc),
            datum_a: Some(DatumRef {
                label: "A".to_string(),
                material_condition: None,
            }),
            datum_b: Some(DatumRef {
                label: "B".to_string(),
                material_condition: Some(MaterialCondition::Lmc),
            }),
            datum_c: None,
            position: Point2D::new(0.0, 0.0),
            leader_to: None,
        };

        assert_eq!(frame.render_text(), "POS|D0.125(M)|A|B(L)");
        assert!(frame.datum_requirement_satisfied());
    }

    #[test]
    fn feature_control_frame_render_outputs_lines_and_cells() {
        let frame = FeatureControlFrame {
            symbol: GdtSymbol::Flatness,
            tolerance: 0.05,
            tolerance_is_diameter: false,
            material_condition: None,
            datum_a: None,
            datum_b: None,
            datum_c: None,
            position: Point2D::new(0.0, 0.0),
            leader_to: Some(GeometryRef::Point(Point2D::new(4.0, -5.0))),
        };

        let rendered = frame.render();
        assert_eq!(rendered.texts.len(), 2);
        assert_eq!(rendered.lines.len(), 6);
    }

    #[test]
    fn datum_feature_symbol_render_outputs_triangle_text_and_leader() {
        let datum = DatumFeatureSymbol {
            label: "A".to_string(),
            position: Point2D::new(10.0, 10.0),
            leader_to: Some(GeometryRef::Point(Point2D::new(13.0, 6.0))),
        };
        let rendered = datum.render();
        assert_eq!(rendered.lines.len(), 4);
        assert_eq!(rendered.texts.len(), 1);
        assert_eq!(rendered.texts[0].text, "A");
    }
}
