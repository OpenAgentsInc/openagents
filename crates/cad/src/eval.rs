use crate::CadResult;
use crate::kernel::CadKernelAdapter;
use crate::primitives::{PrimitiveSpec, build_primitives};

/// Minimal eval plan for early adapter-boundary validation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvalPlan {
    pub primitives: Vec<PrimitiveSpec>,
}

/// Evaluate the plan by routing all primitive creation through the kernel adapter.
pub fn evaluate_plan<K: CadKernelAdapter>(
    kernel: &mut K,
    plan: &EvalPlan,
) -> CadResult<Vec<K::Solid>> {
    build_primitives(kernel, &plan.primitives)
}

#[cfg(test)]
mod tests {
    use super::{EvalPlan, evaluate_plan};
    use crate::CadResult;
    use crate::kernel::CadKernelAdapter;
    use crate::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec};

    #[derive(Default)]
    struct MockKernel {
        calls: Vec<String>,
    }

    impl CadKernelAdapter for MockKernel {
        type Solid = String;

        fn create_box(&mut self, primitive: &BoxPrimitive) -> CadResult<Self::Solid> {
            self.calls.push(format!(
                "box:{:.1}:{:.1}:{:.1}",
                primitive.width_mm, primitive.depth_mm, primitive.height_mm
            ));
            Ok("solid-box".to_string())
        }

        fn create_cylinder(&mut self, primitive: &CylinderPrimitive) -> CadResult<Self::Solid> {
            self.calls.push(format!(
                "cylinder:{:.1}:{:.1}",
                primitive.radius_mm, primitive.height_mm
            ));
            Ok("solid-cylinder".to_string())
        }
    }

    #[test]
    fn evaluate_plan_routes_all_primitives_through_kernel_adapter() {
        let mut kernel = MockKernel::default();
        let plan = EvalPlan {
            primitives: vec![
                PrimitiveSpec::Box(BoxPrimitive {
                    width_mm: 10.0,
                    depth_mm: 20.0,
                    height_mm: 30.0,
                }),
                PrimitiveSpec::Cylinder(CylinderPrimitive {
                    radius_mm: 5.0,
                    height_mm: 12.0,
                }),
            ],
        };

        let result = evaluate_plan(&mut kernel, &plan);
        assert!(result.is_ok(), "plan eval should succeed");

        if let Ok(solids) = result {
            assert_eq!(
                solids,
                vec!["solid-box".to_string(), "solid-cylinder".to_string()]
            );
            assert_eq!(
                kernel.calls,
                vec![
                    "box:10.0:20.0:30.0".to_string(),
                    "cylinder:5.0:12.0".to_string()
                ]
            );
        }
    }
}
