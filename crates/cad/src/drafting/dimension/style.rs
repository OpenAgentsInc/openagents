use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowType {
    Closed,
    Filled,
    Open,
    Dot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextPlacement {
    InLine,
    Above,
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToleranceMode {
    Symmetric,
    Bilateral,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimensionStyle {
    pub text_height: f64,
    pub arrow_size: f64,
    pub decimal_places: usize,
    pub arrow_type: ArrowType,
    pub text_placement: TextPlacement,
    pub tolerance_mode: ToleranceMode,
}

impl Default for DimensionStyle {
    fn default() -> Self {
        Self {
            text_height: 2.5,
            arrow_size: 2.0,
            decimal_places: 2,
            arrow_type: ArrowType::Closed,
            text_placement: TextPlacement::Above,
            tolerance_mode: ToleranceMode::Symmetric,
        }
    }
}
