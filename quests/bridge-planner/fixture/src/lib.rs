//! Plans a bridge deck across a gap in the arena floor.

/// What one deck block is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// Load-bearing oak planks — what a bridge must be.
    Planks,
    /// Loose fill — cheap, and it gives way underfoot.
    Dirt,
}

/// What a bridge across `gap` costs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Deck blocks to lay.
    pub blocks: u32,
    /// What to lay them out of.
    pub material: Material,
}

/// Plans a deck `gap` blocks wide.
#[must_use]
pub fn plan_bridge(gap: u32) -> Plan {
    Plan {
        blocks: gap.saturating_sub(1),
        material: Material::Dirt,
    }
}
