use serde::{Deserialize, Serialize};

use super::angular::{AngleDefinition, AngularDimension};
use super::gdt::{DatumFeatureSymbol, FeatureControlFrame};
use super::linear::{LinearDimension, LinearDimensionType};
use super::ordinate::OrdinateDimension;
use super::radial::RadialDimension;
use super::render::RenderedDimension;
use super::style::DimensionStyle;
use crate::drafting::types::Point2D;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationLayer {
    pub linear_dimensions: Vec<LinearDimension>,
    pub angular_dimensions: Vec<AngularDimension>,
    pub radial_dimensions: Vec<RadialDimension>,
    pub ordinate_dimensions: Vec<OrdinateDimension>,
    pub feature_control_frames: Vec<FeatureControlFrame>,
    pub datum_symbols: Vec<DatumFeatureSymbol>,
    pub style: DimensionStyle,
}

impl AnnotationLayer {
    pub fn new() -> Self {
        Self {
            linear_dimensions: Vec::new(),
            angular_dimensions: Vec::new(),
            radial_dimensions: Vec::new(),
            ordinate_dimensions: Vec::new(),
            feature_control_frames: Vec::new(),
            datum_symbols: Vec::new(),
            style: DimensionStyle::default(),
        }
    }

    pub fn add_horizontal_dimension(
        &mut self,
        start: Point2D,
        end: Point2D,
        offset: f64,
    ) -> &mut Self {
        self.linear_dimensions.push(LinearDimension {
            start,
            end,
            dimension_type: LinearDimensionType::Horizontal,
            offset,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_vertical_dimension(
        &mut self,
        start: Point2D,
        end: Point2D,
        offset: f64,
    ) -> &mut Self {
        self.linear_dimensions.push(LinearDimension {
            start,
            end,
            dimension_type: LinearDimensionType::Vertical,
            offset,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_angle_dimension(
        &mut self,
        start: Point2D,
        vertex: Point2D,
        end: Point2D,
        radius: f64,
    ) -> &mut Self {
        self.angular_dimensions.push(AngularDimension {
            definition: AngleDefinition::FromPoints { start, vertex, end },
            radius,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_radius_dimension(&mut self, center: Point2D, rim_point: Point2D) -> &mut Self {
        self.radial_dimensions.push(RadialDimension {
            center,
            rim_point,
            is_diameter: false,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_diameter_dimension(&mut self, center: Point2D, rim_point: Point2D) -> &mut Self {
        self.radial_dimensions.push(RadialDimension {
            center,
            rim_point,
            is_diameter: true,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_ordinate_dimension(
        &mut self,
        datum: Point2D,
        target: Point2D,
        is_x: bool,
    ) -> &mut Self {
        self.ordinate_dimensions.push(OrdinateDimension {
            datum,
            target,
            is_x,
            style: self.style,
            override_text: None,
            geometry_ref: None,
        });
        self
    }

    pub fn add_feature_control_frame(&mut self, frame: FeatureControlFrame) -> &mut Self {
        self.feature_control_frames.push(frame);
        self
    }

    pub fn add_datum_symbol(&mut self, datum: DatumFeatureSymbol) -> &mut Self {
        self.datum_symbols.push(datum);
        self
    }

    pub fn render_all(&self) -> Vec<RenderedDimension> {
        let mut rendered = Vec::new();
        rendered.extend(self.linear_dimensions.iter().map(LinearDimension::render));
        rendered.extend(self.angular_dimensions.iter().map(AngularDimension::render));
        rendered.extend(self.radial_dimensions.iter().map(RadialDimension::render));
        rendered.extend(
            self.ordinate_dimensions
                .iter()
                .map(OrdinateDimension::render),
        );
        rendered
    }

    pub fn num_annotations(&self) -> usize {
        self.linear_dimensions.len()
            + self.angular_dimensions.len()
            + self.radial_dimensions.len()
            + self.ordinate_dimensions.len()
            + self.feature_control_frames.len()
            + self.datum_symbols.len()
    }
}

impl Default for AnnotationLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_collects_dimensions_and_renders() {
        let mut layer = AnnotationLayer::new();
        layer
            .add_horizontal_dimension(Point2D::new(0.0, 0.0), Point2D::new(10.0, 0.0), 2.0)
            .add_vertical_dimension(Point2D::new(0.0, 0.0), Point2D::new(0.0, 5.0), 2.0)
            .add_angle_dimension(
                Point2D::new(1.0, 0.0),
                Point2D::new(0.0, 0.0),
                Point2D::new(0.0, 1.0),
                3.0,
            );

        assert_eq!(layer.num_annotations(), 3);
        assert_eq!(layer.render_all().len(), 3);
    }
}
