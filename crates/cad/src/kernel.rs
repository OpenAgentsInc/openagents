use crate::CadResult;
use crate::primitives::{BoxPrimitive, CylinderPrimitive};

/// Product-agnostic kernel adapter boundary.
///
/// CAD domain modules must not depend directly on a specific geometry engine.
/// All primitive/eval calls route through this trait.
pub trait CadKernelAdapter {
    /// Opaque kernel-managed solid handle.
    type Solid;

    /// Create a box solid in kernel space.
    fn create_box(&mut self, primitive: &BoxPrimitive) -> CadResult<Self::Solid>;

    /// Create a cylinder solid in kernel space.
    fn create_cylinder(&mut self, primitive: &CylinderPrimitive) -> CadResult<Self::Solid>;
}
