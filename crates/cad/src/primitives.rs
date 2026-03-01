use crate::kernel::CadKernelAdapter;
use crate::policy;
use crate::{CadError, CadResult};

/// Box primitive dimensions in millimeters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxPrimitive {
    pub width_mm: f64,
    pub depth_mm: f64,
    pub height_mm: f64,
}

impl BoxPrimitive {
    /// Validate this primitive before sending to a kernel implementation.
    pub fn validate(&self) -> CadResult<()> {
        if !policy::is_dimension_positive(self.width_mm) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "box width must be greater than tolerance {} {}",
                    policy::BASE_TOLERANCE_MM,
                    policy::CANONICAL_UNIT
                ),
            });
        }
        if !policy::is_dimension_positive(self.depth_mm) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "box depth must be greater than tolerance {} {}",
                    policy::BASE_TOLERANCE_MM,
                    policy::CANONICAL_UNIT
                ),
            });
        }
        if !policy::is_dimension_positive(self.height_mm) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "box height must be greater than tolerance {} {}",
                    policy::BASE_TOLERANCE_MM,
                    policy::CANONICAL_UNIT
                ),
            });
        }
        Ok(())
    }
}

/// Cylinder primitive dimensions in millimeters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CylinderPrimitive {
    pub radius_mm: f64,
    pub height_mm: f64,
}

impl CylinderPrimitive {
    /// Validate this primitive before sending to a kernel implementation.
    pub fn validate(&self) -> CadResult<()> {
        if !policy::is_dimension_positive(self.radius_mm) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "cylinder radius must be greater than tolerance {} {}",
                    policy::BASE_TOLERANCE_MM,
                    policy::CANONICAL_UNIT
                ),
            });
        }
        if !policy::is_dimension_positive(self.height_mm) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "cylinder height must be greater than tolerance {} {}",
                    policy::BASE_TOLERANCE_MM,
                    policy::CANONICAL_UNIT
                ),
            });
        }
        Ok(())
    }
}

/// Primitive specification routed through the kernel adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrimitiveSpec {
    Box(BoxPrimitive),
    Cylinder(CylinderPrimitive),
}

/// Build one primitive solid through the active kernel adapter.
pub fn build_primitive<K: CadKernelAdapter>(
    kernel: &mut K,
    primitive: PrimitiveSpec,
) -> CadResult<K::Solid> {
    match primitive {
        PrimitiveSpec::Box(spec) => {
            spec.validate()?;
            kernel.create_box(&spec)
        }
        PrimitiveSpec::Cylinder(spec) => {
            spec.validate()?;
            kernel.create_cylinder(&spec)
        }
    }
}

/// Build multiple primitive solids through the active kernel adapter.
pub fn build_primitives<K: CadKernelAdapter>(
    kernel: &mut K,
    primitives: &[PrimitiveSpec],
) -> CadResult<Vec<K::Solid>> {
    primitives
        .iter()
        .copied()
        .map(|primitive| build_primitive(kernel, primitive))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{BoxPrimitive, PrimitiveSpec, build_primitive};
    use crate::CadResult;
    use crate::kernel::CadKernelAdapter;
    use crate::primitives::CylinderPrimitive;

    #[derive(Default)]
    struct CountingKernel {
        box_calls: usize,
        cylinder_calls: usize,
    }

    impl CadKernelAdapter for CountingKernel {
        type Solid = &'static str;

        fn create_box(&mut self, _primitive: &BoxPrimitive) -> CadResult<Self::Solid> {
            self.box_calls = self.box_calls.saturating_add(1);
            Ok("box")
        }

        fn create_cylinder(&mut self, _primitive: &CylinderPrimitive) -> CadResult<Self::Solid> {
            self.cylinder_calls = self.cylinder_calls.saturating_add(1);
            Ok("cylinder")
        }
    }

    #[test]
    fn invalid_box_is_rejected_before_kernel_call() {
        let mut kernel = CountingKernel::default();
        let result = build_primitive(
            &mut kernel,
            PrimitiveSpec::Box(BoxPrimitive {
                width_mm: 0.0,
                depth_mm: 10.0,
                height_mm: 10.0,
            }),
        );
        assert!(result.is_err(), "invalid primitive should return error");
        assert_eq!(kernel.box_calls, 0);
        assert_eq!(kernel.cylinder_calls, 0);
    }

    #[test]
    fn valid_cylinder_routes_through_kernel_call() {
        let mut kernel = CountingKernel::default();
        let result = build_primitive(
            &mut kernel,
            PrimitiveSpec::Cylinder(CylinderPrimitive {
                radius_mm: 4.0,
                height_mm: 8.0,
            }),
        );
        assert!(result.is_ok(), "valid primitive should succeed");
        assert_eq!(kernel.box_calls, 0);
        assert_eq!(kernel.cylinder_calls, 1);
    }
}
