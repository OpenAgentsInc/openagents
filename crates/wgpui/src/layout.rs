use taffy::prelude::*;

pub struct LayoutEngine {
    taffy: TaffyTree,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutId(pub taffy::NodeId);

#[derive(Clone, Debug, Default)]
pub struct LayoutStyle {
    pub style: Style,
}

pub fn px(val: f32) -> LengthPercentage {
    LengthPercentage::length(val)
}

pub fn pct(val: f32) -> LengthPercentage {
    LengthPercentage::percent(val / 100.0)
}

pub fn length(val: f32) -> Dimension {
    Dimension::length(val)
}

pub fn length_auto() -> Dimension {
    Dimension::auto()
}

pub fn auto() -> Dimension {
    Dimension::auto()
}

pub fn zero() -> LengthPercentage {
    LengthPercentage::length(0.0)
}

pub fn relative(val: f32) -> LengthPercentageAuto {
    LengthPercentageAuto::percent(val / 100.0)
}
