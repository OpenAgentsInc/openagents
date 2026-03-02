use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::policy;
use crate::primitives::{BoxPrimitive, CylinderPrimitive};
use crate::{CadError, CadResult};

/// Product-agnostic kernel adapter boundary (v1).
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

/// Engine family used by a pluggable v2 adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum KernelEngineFamily {
    VcadSubset,
    OpenCascade,
    Custom,
}

/// Stable capability keys exposed by kernel adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum KernelCapability {
    PrimitiveBox,
    PrimitiveCylinder,
    BooleanUnion,
    BooleanDifference,
    Tessellation,
    StepImport,
    StepExport,
}

/// Declarative adapter metadata used for engine discovery and hot-swap checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelAdapterV2Descriptor {
    pub schema_version: u64,
    pub issue_id: String,
    pub adapter_version: String,
    pub engine_id: String,
    pub engine_family: KernelEngineFamily,
    pub supports_hot_swap: bool,
    pub capabilities: Vec<KernelCapability>,
    pub diagnostics_contract: String,
}

impl KernelAdapterV2Descriptor {
    /// Return a normalized descriptor with sorted, deduplicated capabilities.
    pub fn normalized(mut self) -> Self {
        self.capabilities.sort();
        self.capabilities.dedup();
        self
    }
}

/// Default OpenAgents kernel adapter descriptor for v2 contract checks.
pub fn openagents_kernel_adapter_v2_descriptor() -> KernelAdapterV2Descriptor {
    KernelAdapterV2Descriptor {
        schema_version: 1,
        issue_id: "VCAD-PARITY-011".to_string(),
        adapter_version: "2.0.0".to_string(),
        engine_id: "openagents-kernel-v2".to_string(),
        engine_family: KernelEngineFamily::VcadSubset,
        supports_hot_swap: true,
        capabilities: vec![
            KernelCapability::PrimitiveBox,
            KernelCapability::PrimitiveCylinder,
        ],
        diagnostics_contract: "cad.error.v1 + kernel.receipt.v2".to_string(),
    }
    .normalized()
}

/// Operation context passed to v2 adapters for deterministic receipts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelOperationContext {
    pub request_id: String,
    pub operation: String,
    pub tolerance_mm: f64,
}

impl KernelOperationContext {
    pub fn new(
        request_id: impl Into<String>,
        operation: impl Into<String>,
        tolerance_mm: f64,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            operation: operation.into(),
            tolerance_mm,
        }
    }
}

impl Default for KernelOperationContext {
    fn default() -> Self {
        Self {
            request_id: "kernel-op-default".to_string(),
            operation: "primitive.create".to_string(),
            tolerance_mm: policy::BASE_TOLERANCE_MM,
        }
    }
}

/// Deterministic operation receipt emitted by v2 adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelOperationReceipt {
    pub request_id: String,
    pub operation: String,
    pub engine_id: String,
    pub adapter_version: String,
    pub deterministic: bool,
    pub diagnostics: Vec<String>,
}

/// V2 operation result: kernel solid handle plus deterministic receipt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelOperationResult<S> {
    pub solid: S,
    pub receipt: KernelOperationReceipt,
}

/// Product-agnostic kernel adapter boundary (v2) with pluggability metadata.
pub trait CadKernelAdapterV2 {
    type Solid;

    fn descriptor(&self) -> KernelAdapterV2Descriptor;

    fn create_box_v2(
        &mut self,
        primitive: &BoxPrimitive,
        context: &KernelOperationContext,
    ) -> CadResult<KernelOperationResult<Self::Solid>>;

    fn create_cylinder_v2(
        &mut self,
        primitive: &CylinderPrimitive,
        context: &KernelOperationContext,
    ) -> CadResult<KernelOperationResult<Self::Solid>>;
}

/// Bridge legacy v1 adapters into v2 adapter contract without breaking callers.
pub struct KernelAdapterV2Bridge<K: CadKernelAdapter> {
    inner: K,
    descriptor: KernelAdapterV2Descriptor,
}

impl<K: CadKernelAdapter> KernelAdapterV2Bridge<K> {
    pub fn new(inner: K, descriptor: KernelAdapterV2Descriptor) -> Self {
        Self {
            inner,
            descriptor: descriptor.normalized(),
        }
    }

    pub fn inner(&self) -> &K {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut K {
        &mut self.inner
    }
}

impl<K: CadKernelAdapter> CadKernelAdapterV2 for KernelAdapterV2Bridge<K> {
    type Solid = K::Solid;

    fn descriptor(&self) -> KernelAdapterV2Descriptor {
        self.descriptor.clone()
    }

    fn create_box_v2(
        &mut self,
        primitive: &BoxPrimitive,
        context: &KernelOperationContext,
    ) -> CadResult<KernelOperationResult<Self::Solid>> {
        let solid = self.inner.create_box(primitive)?;
        Ok(KernelOperationResult {
            solid,
            receipt: operation_receipt(&self.descriptor, context),
        })
    }

    fn create_cylinder_v2(
        &mut self,
        primitive: &CylinderPrimitive,
        context: &KernelOperationContext,
    ) -> CadResult<KernelOperationResult<Self::Solid>> {
        let solid = self.inner.create_cylinder(primitive)?;
        Ok(KernelOperationResult {
            solid,
            receipt: operation_receipt(&self.descriptor, context),
        })
    }
}

fn operation_receipt(
    descriptor: &KernelAdapterV2Descriptor,
    context: &KernelOperationContext,
) -> KernelOperationReceipt {
    KernelOperationReceipt {
        request_id: context.request_id.clone(),
        operation: context.operation.clone(),
        engine_id: descriptor.engine_id.clone(),
        adapter_version: descriptor.adapter_version.clone(),
        deterministic: true,
        diagnostics: vec![
            format!("tolerance_mm={:.6}", context.tolerance_mm),
            format!("capability_count={}", descriptor.capabilities.len()),
        ],
    }
}

/// Registry used to track and switch active kernel engines for pluggability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelAdapterRegistry {
    active_engine_id: String,
    descriptors: BTreeMap<String, KernelAdapterV2Descriptor>,
}

impl KernelAdapterRegistry {
    pub fn new(active: KernelAdapterV2Descriptor) -> Self {
        let active = active.normalized();
        let active_engine_id = active.engine_id.clone();
        let mut descriptors = BTreeMap::new();
        descriptors.insert(active.engine_id.clone(), active);
        Self {
            active_engine_id,
            descriptors,
        }
    }

    pub fn register(&mut self, descriptor: KernelAdapterV2Descriptor) -> CadResult<()> {
        let descriptor = descriptor.normalized();
        if self.descriptors.contains_key(&descriptor.engine_id) {
            return Err(CadError::InvalidPolicy {
                reason: format!("duplicate kernel engine id: {}", descriptor.engine_id),
            });
        }
        self.descriptors
            .insert(descriptor.engine_id.clone(), descriptor);
        Ok(())
    }

    pub fn set_active_engine(&mut self, engine_id: &str) -> CadResult<()> {
        if !self.descriptors.contains_key(engine_id) {
            return Err(CadError::InvalidPolicy {
                reason: format!("unknown kernel engine id: {engine_id}"),
            });
        }
        self.active_engine_id = engine_id.to_string();
        Ok(())
    }

    pub fn active_descriptor(&self) -> &KernelAdapterV2Descriptor {
        // Registry invariants guarantee this key exists.
        self.descriptors
            .get(&self.active_engine_id)
            .expect("active kernel engine must exist in registry")
    }

    pub fn active_engine_id(&self) -> &str {
        &self.active_engine_id
    }

    pub fn descriptors(&self) -> Vec<&KernelAdapterV2Descriptor> {
        self.descriptors.values().collect()
    }

    pub fn engine_ids(&self) -> Vec<String> {
        self.descriptors.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadKernelAdapter, CadKernelAdapterV2, KernelAdapterRegistry, KernelAdapterV2Bridge,
        KernelAdapterV2Descriptor, KernelCapability, KernelEngineFamily, KernelOperationContext,
        openagents_kernel_adapter_v2_descriptor,
    };
    use crate::primitives::{BoxPrimitive, CylinderPrimitive};
    use crate::{CadError, CadResult};

    #[derive(Default)]
    struct MockKernel {
        box_calls: usize,
        cylinder_calls: usize,
    }

    impl CadKernelAdapter for MockKernel {
        type Solid = String;

        fn create_box(&mut self, _primitive: &BoxPrimitive) -> CadResult<Self::Solid> {
            self.box_calls += 1;
            Ok("box-solid".to_string())
        }

        fn create_cylinder(&mut self, _primitive: &CylinderPrimitive) -> CadResult<Self::Solid> {
            self.cylinder_calls += 1;
            Ok("cylinder-solid".to_string())
        }
    }

    fn fallback_descriptor() -> KernelAdapterV2Descriptor {
        KernelAdapterV2Descriptor {
            schema_version: 1,
            issue_id: "VCAD-PARITY-011".to_string(),
            adapter_version: "2.0.0".to_string(),
            engine_id: "opencascade-kernel-v2".to_string(),
            engine_family: KernelEngineFamily::OpenCascade,
            supports_hot_swap: true,
            capabilities: vec![
                KernelCapability::PrimitiveBox,
                KernelCapability::PrimitiveCylinder,
            ],
            diagnostics_contract: "cad.error.v1 + kernel.receipt.v2".to_string(),
        }
    }

    #[test]
    fn openagents_kernel_descriptor_is_stable() {
        let descriptor = openagents_kernel_adapter_v2_descriptor();
        assert_eq!(descriptor.issue_id, "VCAD-PARITY-011");
        assert_eq!(descriptor.adapter_version, "2.0.0");
        assert_eq!(descriptor.engine_id, "openagents-kernel-v2");
        assert_eq!(descriptor.engine_family, KernelEngineFamily::VcadSubset);
    }

    #[test]
    fn bridge_emits_receipts_and_routes_to_legacy_adapter() {
        let context =
            KernelOperationContext::new("req-1", "primitive.box", crate::policy::BASE_TOLERANCE_MM);
        let mut bridge = KernelAdapterV2Bridge::new(
            MockKernel::default(),
            openagents_kernel_adapter_v2_descriptor(),
        );
        let primitive = BoxPrimitive {
            width_mm: 10.0,
            depth_mm: 10.0,
            height_mm: 10.0,
        };
        let result = bridge
            .create_box_v2(&primitive, &context)
            .expect("box operation should pass");
        assert_eq!(result.solid, "box-solid");
        assert_eq!(result.receipt.request_id, "req-1");
        assert_eq!(result.receipt.operation, "primitive.box");
        assert_eq!(result.receipt.engine_id, "openagents-kernel-v2");
        assert!(result.receipt.deterministic);
        assert_eq!(bridge.inner().box_calls, 1);
        assert_eq!(bridge.inner().cylinder_calls, 0);
    }

    #[test]
    fn registry_supports_engine_registration_and_switching() {
        let mut registry = KernelAdapterRegistry::new(openagents_kernel_adapter_v2_descriptor());
        registry
            .register(fallback_descriptor())
            .expect("register fallback descriptor");
        assert_eq!(registry.engine_ids().len(), 2);
        registry
            .set_active_engine("opencascade-kernel-v2")
            .expect("switch active engine");
        assert_eq!(registry.active_engine_id(), "opencascade-kernel-v2");
    }

    #[test]
    fn registry_rejects_duplicate_engine_ids() {
        let mut registry = KernelAdapterRegistry::new(openagents_kernel_adapter_v2_descriptor());
        let duplicate = openagents_kernel_adapter_v2_descriptor();
        let error = registry
            .register(duplicate)
            .expect_err("duplicate register should fail");
        assert_eq!(
            error,
            CadError::InvalidPolicy {
                reason: "duplicate kernel engine id: openagents-kernel-v2".to_string()
            }
        );
    }
}
