use openagents_provider_substrate::{
    ProviderComputeProduct, ProviderInventoryRow, ProviderSandboxExecutionClass,
    ProviderSandboxProfile,
};
use serde::{Deserialize, Serialize};

use crate::app_state::RenderState;

pub(crate) const CLUSTER_NOT_INTEGRATED_REASON: &str =
    "cluster transport is not integrated into the desktop control plane yet";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlInventoryProjectionStatus {
    pub source: String,
    pub latest_snapshot_id: Option<String>,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_validator_challenges_open: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlInventoryProductStatus {
    pub product_id: String,
    pub display_label: String,
    pub inventory_scope: String,
    pub compute_family: String,
    pub backend_family: String,
    pub topology_label: String,
    pub proof_posture: String,
    pub environment_binding: Option<String>,
    pub availability_state: String,
    pub blocker_reason: Option<String>,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub source_badge: String,
    pub capability_summary: String,
    pub capacity_lot_id: Option<String>,
    pub total_quantity: u64,
    pub reserved_quantity: u64,
    pub available_quantity: u64,
    pub delivery_state: String,
    pub price_floor_sats: u64,
    pub terms_label: String,
    pub forward_capacity_lot_id: Option<String>,
    pub forward_delivery_window_label: Option<String>,
    pub forward_total_quantity: u64,
    pub forward_reserved_quantity: u64,
    pub forward_available_quantity: u64,
    pub forward_terms_label: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlInventorySectionStatus {
    pub section_id: String,
    pub label: String,
    pub available: bool,
    pub blocker_reason: Option<String>,
    pub summary: String,
    pub product_count: usize,
    pub ready_product_count: usize,
    pub eligible_product_count: usize,
    pub open_quantity: u64,
    pub products: Vec<DesktopControlInventoryProductStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlInventoryStatus {
    pub authority: String,
    pub projection: DesktopControlInventoryProjectionStatus,
    pub sections: Vec<DesktopControlInventorySectionStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InventoryProjectionCounts {
    pub latest_snapshot_id: Option<String>,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_validator_challenges_open: u64,
}

pub(crate) struct InventoryStatusInput<'a> {
    pub uses_remote_kernel_projection: bool,
    pub projection: Option<InventoryProjectionCounts>,
    pub inventory_rows: &'a [ProviderInventoryRow],
    pub sandbox_profiles: &'a [ProviderSandboxProfile],
    pub gpt_oss_ready_model: Option<&'a str>,
    pub gpt_oss_configured_model: Option<&'a str>,
    pub apple_fm_ready_model: Option<&'a str>,
    pub cluster_available: bool,
    pub cluster_topology_label: &'a str,
    pub cluster_member_count: usize,
    pub cluster_last_error: Option<&'a str>,
}

pub(crate) fn inventory_status_for_state(state: &RenderState) -> DesktopControlInventoryStatus {
    let projection = state
        .economy_snapshot
        .latest_snapshot
        .as_ref()
        .map(|snapshot| InventoryProjectionCounts {
            latest_snapshot_id: Some(snapshot.snapshot_id.clone()),
            compute_products_active: snapshot.compute_products_active,
            compute_capacity_lots_open: snapshot.compute_capacity_lots_open,
            compute_capacity_lots_delivering: snapshot.compute_capacity_lots_delivering,
            compute_inventory_quantity_open: snapshot.compute_inventory_quantity_open,
            compute_inventory_quantity_reserved: snapshot.compute_inventory_quantity_reserved,
            compute_inventory_quantity_delivering: snapshot.compute_inventory_quantity_delivering,
            compute_delivery_proofs_24h: snapshot.compute_delivery_proofs_24h,
            compute_validator_challenges_open: snapshot.compute_validator_challenges_open,
        });
    build_inventory_status(InventoryStatusInput {
        uses_remote_kernel_projection: state.kernel_projection_worker.uses_remote_authority(),
        projection,
        inventory_rows: state.provider_runtime.inventory_rows.as_slice(),
        sandbox_profiles: state.provider_runtime.sandbox.profiles.as_slice(),
        gpt_oss_ready_model: state.provider_runtime.gpt_oss.ready_model.as_deref(),
        gpt_oss_configured_model: state.provider_runtime.gpt_oss.configured_model.as_deref(),
        apple_fm_ready_model: state.provider_runtime.apple_fm.ready_model.as_deref(),
        cluster_available: false,
        cluster_topology_label: "not_integrated",
        cluster_member_count: 0,
        cluster_last_error: Some(CLUSTER_NOT_INTEGRATED_REASON),
    })
}

pub(crate) fn inventory_status_summary(status: &DesktopControlInventoryStatus) -> String {
    let local_products = status
        .sections
        .iter()
        .find(|section| section.section_id == "local")
        .map(|section| section.product_count)
        .unwrap_or(0);
    let sandbox_products = status
        .sections
        .iter()
        .find(|section| section.section_id == "sandbox")
        .map(|section| section.product_count)
        .unwrap_or(0);
    let cluster_available = status
        .sections
        .iter()
        .find(|section| section.section_id == "cluster")
        .is_some_and(|section| section.available);
    format!(
        "inventory authority={} projection={} local_products={} sandbox_products={} cluster_available={}",
        status.authority,
        status.projection.source,
        local_products,
        sandbox_products,
        cluster_available
    )
}

pub(crate) fn inventory_detail_lines(status: &DesktopControlInventoryStatus) -> Vec<String> {
    let mut lines = vec![
        format!("Inventory authority: {}", status.authority),
        format!(
            "Projection: source={} snapshot={} products={} lots_open={} inventory_open={} reserved={} delivering={} proofs_24h={} challenges_open={}",
            status.projection.source,
            status
                .projection
                .latest_snapshot_id
                .as_deref()
                .unwrap_or("n/a"),
            status.projection.compute_products_active,
            status.projection.compute_capacity_lots_open,
            status.projection.compute_inventory_quantity_open,
            status.projection.compute_inventory_quantity_reserved,
            status.projection.compute_inventory_quantity_delivering,
            status.projection.compute_delivery_proofs_24h,
            status.projection.compute_validator_challenges_open
        ),
    ];
    for section in &status.sections {
        lines.push(format!(
            "{} inventory: available={} products={} ready={} eligible={} open_quantity={} summary={}",
            section.label,
            section.available,
            section.product_count,
            section.ready_product_count,
            section.eligible_product_count,
            section.open_quantity,
            section.summary
        ));
        if let Some(blocker_reason) = section.blocker_reason.as_deref() {
            lines.push(format!("{} blocker: {}", section.label, blocker_reason));
        }
        for product in &section.products {
            lines.push(format!(
                "{} [{}] state={} enabled={} backend_ready={} eligible={} topology={} proof={} source={}",
                product.display_label,
                product.product_id,
                product.availability_state,
                product.enabled,
                product.backend_ready,
                product.eligible,
                product.topology_label,
                product.proof_posture,
                product.source_badge
            ));
            if let Some(environment_binding) = product.environment_binding.as_deref() {
                lines.push(format!("binding: {}", environment_binding));
            }
            if let Some(blocker_reason) = product.blocker_reason.as_deref() {
                lines.push(format!("blocker: {}", blocker_reason));
            }
            lines.push(format!(
                "lots: spot={} total={} reserved={} available={} delivery={} floor={} terms={}",
                product.capacity_lot_id.as_deref().unwrap_or("n/a"),
                product.total_quantity,
                product.reserved_quantity,
                product.available_quantity,
                product.delivery_state,
                product.price_floor_sats,
                product.terms_label
            ));
            if product.forward_capacity_lot_id.is_some() || product.forward_terms_label.is_some() {
                lines.push(format!(
                    "forward: lot={} window={} total={} reserved={} available={} terms={}",
                    product.forward_capacity_lot_id.as_deref().unwrap_or("n/a"),
                    product
                        .forward_delivery_window_label
                        .as_deref()
                        .unwrap_or("n/a"),
                    product.forward_total_quantity,
                    product.forward_reserved_quantity,
                    product.forward_available_quantity,
                    product.forward_terms_label.as_deref().unwrap_or("n/a")
                ));
            }
            lines.push(format!("capability: {}", product.capability_summary));
        }
    }
    lines
}

pub(crate) fn build_inventory_status(
    input: InventoryStatusInput<'_>,
) -> DesktopControlInventoryStatus {
    let projection = projection_status(
        input.uses_remote_kernel_projection,
        input.projection.as_ref(),
    );
    let authority = match (
        input.uses_remote_kernel_projection,
        projection.latest_snapshot_id.is_some(),
    ) {
        (true, true) => "kernel_projected",
        (true, false) => "kernel_projection_pending",
        (false, _) => "local_only",
    };
    let local_products = build_product_rows("local", &input);
    let sandbox_products = build_product_rows("sandbox", &input);
    DesktopControlInventoryStatus {
        authority: authority.to_string(),
        projection,
        sections: vec![
            build_section("local", "Local", local_products, None),
            build_cluster_section(&input),
            build_sandbox_section(&input, sandbox_products),
        ],
    }
}

fn projection_status(
    uses_remote_kernel_projection: bool,
    projection: Option<&InventoryProjectionCounts>,
) -> DesktopControlInventoryProjectionStatus {
    let source = match (uses_remote_kernel_projection, projection.is_some()) {
        (true, true) => "kernel_projection",
        (true, false) => "kernel_projection_pending",
        (false, _) => "local_only",
    };
    let mut status = DesktopControlInventoryProjectionStatus {
        source: source.to_string(),
        ..DesktopControlInventoryProjectionStatus::default()
    };
    if let Some(projection) = projection {
        status.latest_snapshot_id = projection.latest_snapshot_id.clone();
        status.compute_products_active = projection.compute_products_active;
        status.compute_capacity_lots_open = projection.compute_capacity_lots_open;
        status.compute_capacity_lots_delivering = projection.compute_capacity_lots_delivering;
        status.compute_inventory_quantity_open = projection.compute_inventory_quantity_open;
        status.compute_inventory_quantity_reserved = projection.compute_inventory_quantity_reserved;
        status.compute_inventory_quantity_delivering =
            projection.compute_inventory_quantity_delivering;
        status.compute_delivery_proofs_24h = projection.compute_delivery_proofs_24h;
        status.compute_validator_challenges_open = projection.compute_validator_challenges_open;
    }
    status
}

fn build_product_rows(
    section_id: &str,
    input: &InventoryStatusInput<'_>,
) -> Vec<DesktopControlInventoryProductStatus> {
    input
        .inventory_rows
        .iter()
        .filter(|row| scope_for_product(row.target).0 == section_id)
        .map(|row| build_product_row(row, input))
        .collect()
}

fn build_product_row(
    row: &ProviderInventoryRow,
    input: &InventoryStatusInput<'_>,
) -> DesktopControlInventoryProductStatus {
    let descriptor = row.target.descriptor();
    let (_, inventory_scope, topology_label) = scope_for_product(row.target);
    DesktopControlInventoryProductStatus {
        product_id: descriptor.product_id,
        display_label: row.target.display_label().to_string(),
        inventory_scope: inventory_scope.to_string(),
        compute_family: descriptor.compute_family,
        backend_family: descriptor.backend_family,
        topology_label: topology_label.to_string(),
        proof_posture: proof_posture_for_product(row.target, input.uses_remote_kernel_projection),
        environment_binding: environment_binding_for_product(row.target, input),
        availability_state: availability_state_for_row(row),
        blocker_reason: blocker_reason_for_row(row, input),
        enabled: row.enabled,
        backend_ready: row.backend_ready,
        eligible: row.eligible,
        source_badge: row.source_badge.clone(),
        capability_summary: row.capability_summary.clone(),
        capacity_lot_id: row.capacity_lot_id.clone(),
        total_quantity: row.total_quantity,
        reserved_quantity: row.reserved_quantity,
        available_quantity: row.available_quantity,
        delivery_state: row.delivery_state.clone(),
        price_floor_sats: row.price_floor_sats,
        terms_label: row.terms_label.clone(),
        forward_capacity_lot_id: row.forward_capacity_lot_id.clone(),
        forward_delivery_window_label: row.forward_delivery_window_label.clone(),
        forward_total_quantity: row.forward_total_quantity,
        forward_reserved_quantity: row.forward_reserved_quantity,
        forward_available_quantity: row.forward_available_quantity,
        forward_terms_label: row.forward_terms_label.clone(),
    }
}

fn build_section(
    section_id: &str,
    label: &str,
    products: Vec<DesktopControlInventoryProductStatus>,
    blocker_reason: Option<String>,
) -> DesktopControlInventorySectionStatus {
    let product_count = products.len();
    let ready_product_count = products
        .iter()
        .filter(|product| product.backend_ready)
        .count();
    let eligible_product_count = products.iter().filter(|product| product.eligible).count();
    let open_quantity = products
        .iter()
        .map(|product| product.available_quantity)
        .sum::<u64>();
    let available = product_count > 0;
    let summary = format!(
        "products={} ready={} eligible={} open_quantity={}",
        product_count, ready_product_count, eligible_product_count, open_quantity
    );
    DesktopControlInventorySectionStatus {
        section_id: section_id.to_string(),
        label: label.to_string(),
        available,
        blocker_reason,
        summary,
        product_count,
        ready_product_count,
        eligible_product_count,
        open_quantity,
        products,
    }
}

fn build_cluster_section(input: &InventoryStatusInput<'_>) -> DesktopControlInventorySectionStatus {
    let available = input.cluster_available;
    let blocker_reason = (!available).then(|| {
        input
            .cluster_last_error
            .unwrap_or(CLUSTER_NOT_INTEGRATED_REASON)
            .to_string()
    });
    DesktopControlInventorySectionStatus {
        section_id: "cluster".to_string(),
        label: "Cluster".to_string(),
        available,
        blocker_reason,
        summary: format!(
            "topology={} members={}",
            input.cluster_topology_label, input.cluster_member_count
        ),
        product_count: 0,
        ready_product_count: 0,
        eligible_product_count: 0,
        open_quantity: 0,
        products: Vec::new(),
    }
}

fn build_sandbox_section(
    input: &InventoryStatusInput<'_>,
    products: Vec<DesktopControlInventoryProductStatus>,
) -> DesktopControlInventorySectionStatus {
    let profile_count = input.sandbox_profiles.len();
    let ready_profile_count = input
        .sandbox_profiles
        .iter()
        .filter(|profile| profile.runtime_ready)
        .count();
    let blocker_reason = if profile_count == 0 {
        Some(
            "no declared sandbox profiles are available in the current desktop runtime".to_string(),
        )
    } else if ready_profile_count == 0 {
        Some("sandbox profiles are declared but no runtime-ready profile is available".to_string())
    } else {
        None
    };
    let mut section = build_section("sandbox", "Sandbox", products, blocker_reason);
    section.available = profile_count > 0;
    section.summary = format!(
        "profiles={} ready_profiles={} {}",
        profile_count, ready_profile_count, section.summary
    );
    section
}

fn scope_for_product(
    product: ProviderComputeProduct,
) -> (&'static str, &'static str, &'static str) {
    match product {
        ProviderComputeProduct::GptOssInference
        | ProviderComputeProduct::GptOssEmbeddings
        | ProviderComputeProduct::AppleFoundationModelsInference
        | ProviderComputeProduct::AppleFoundationModelsAdapterHosting => {
            ("local", "local", "single_node")
        }
        ProviderComputeProduct::AdapterTrainingContributor => {
            ("local", "training", "cluster_attached")
        }
        ProviderComputeProduct::SandboxContainerExec
        | ProviderComputeProduct::SandboxPythonExec
        | ProviderComputeProduct::SandboxNodeExec
        | ProviderComputeProduct::SandboxPosixExec => ("sandbox", "sandbox", "sandbox_isolated"),
    }
}

fn proof_posture_for_product(
    product: ProviderComputeProduct,
    uses_remote_kernel_projection: bool,
) -> String {
    match (
        product.sandbox_execution_class(),
        uses_remote_kernel_projection,
    ) {
        (Some(_), true) => "sandbox_execution_evidence + kernel_delivery_proof".to_string(),
        (Some(_), false) => "sandbox_execution_evidence_only".to_string(),
        (None, true) => "execution_receipt + kernel_delivery_proof".to_string(),
        (None, false) => "execution_receipt_only".to_string(),
    }
}

fn environment_binding_for_product(
    product: ProviderComputeProduct,
    input: &InventoryStatusInput<'_>,
) -> Option<String> {
    match product {
        ProviderComputeProduct::GptOssInference | ProviderComputeProduct::GptOssEmbeddings => input
            .gpt_oss_ready_model
            .or(input.gpt_oss_configured_model)
            .map(|model| format!("model:{model}")),
        ProviderComputeProduct::AppleFoundationModelsInference
        | ProviderComputeProduct::AppleFoundationModelsAdapterHosting => input
            .apple_fm_ready_model
            .map(|model| format!("model:{model}")),
        ProviderComputeProduct::AdapterTrainingContributor => input
            .apple_fm_ready_model
            .map(|model| format!("training:{model}")),
        ProviderComputeProduct::SandboxContainerExec
        | ProviderComputeProduct::SandboxPythonExec
        | ProviderComputeProduct::SandboxNodeExec
        | ProviderComputeProduct::SandboxPosixExec => {
            let Some(execution_class) = product.sandbox_execution_class() else {
                return None;
            };
            let profile_ids = matching_sandbox_profile_ids(input.sandbox_profiles, execution_class);
            (!profile_ids.is_empty()).then(|| format!("profiles:{}", profile_ids.join(",")))
        }
    }
}

fn blocker_reason_for_row(
    row: &ProviderInventoryRow,
    input: &InventoryStatusInput<'_>,
) -> Option<String> {
    if !row.enabled {
        return Some("advertising disabled in Mission Control".to_string());
    }
    if row.backend_ready {
        return None;
    }
    match row.target {
        ProviderComputeProduct::GptOssInference | ProviderComputeProduct::GptOssEmbeddings => {
            Some("local GPT-OSS runtime is not ready to advertise this product".to_string())
        }
        ProviderComputeProduct::AppleFoundationModelsInference => {
            Some("Apple Foundation Models is not ready to advertise this product".to_string())
        }
        ProviderComputeProduct::AppleFoundationModelsAdapterHosting => Some(
            "Apple adapter hosting requires a ready Apple FM runtime plus at least one compatible loaded adapter".to_string(),
        ),
        ProviderComputeProduct::AdapterTrainingContributor => Some(
            "Psionic training contributor requires the Apple adapter lane to be ready and advertising enabled".to_string(),
        ),
        ProviderComputeProduct::SandboxContainerExec
        | ProviderComputeProduct::SandboxPythonExec
        | ProviderComputeProduct::SandboxNodeExec
        | ProviderComputeProduct::SandboxPosixExec => {
            let execution_class = row
                .target
                .sandbox_execution_class()
                .unwrap_or(ProviderSandboxExecutionClass::PosixExec);
            let profile_ids = matching_sandbox_profile_ids(input.sandbox_profiles, execution_class);
            if profile_ids.is_empty() {
                Some("no declared sandbox profile matches this execution class".to_string())
            } else {
                Some("declared sandbox profile exists but the runtime is not ready".to_string())
            }
        }
    }
}

fn matching_sandbox_profile_ids(
    profiles: &[ProviderSandboxProfile],
    execution_class: ProviderSandboxExecutionClass,
) -> Vec<String> {
    profiles
        .iter()
        .filter(|profile| profile.execution_class == execution_class)
        .map(|profile| profile.profile_id.clone())
        .collect()
}

fn availability_state_for_row(row: &ProviderInventoryRow) -> String {
    if !row.enabled {
        return "disabled".to_string();
    }
    if !row.backend_ready {
        return "blocked".to_string();
    }
    if row.delivery_state != "idle" {
        return row.delivery_state.clone();
    }
    if row.reserved_quantity > 0 {
        return "reserved".to_string();
    }
    if row.available_quantity > 0 {
        return "open".to_string();
    }
    if row.total_quantity > 0 {
        return "published".to_string();
    }
    if row.eligible {
        return "ready".to_string();
    }
    "blocked".to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        CLUSTER_NOT_INTEGRATED_REASON, DesktopControlInventorySectionStatus,
        InventoryProjectionCounts, InventoryStatusInput, build_inventory_status,
        inventory_detail_lines,
    };
    use openagents_provider_substrate::{
        ProviderComputeProduct, ProviderInventoryRow, ProviderSandboxExecutionClass,
        ProviderSandboxProfile, ProviderSandboxRuntimeKind,
    };

    fn local_row() -> ProviderInventoryRow {
        ProviderInventoryRow {
            target: ProviderComputeProduct::GptOssInference,
            enabled: true,
            backend_ready: true,
            eligible: true,
            capability_summary:
                "backend=gpt_oss execution=local_inference family=inference model=gpt-oss"
                    .to_string(),
            source_badge: "local_runtime".to_string(),
            capacity_lot_id: Some("lot.local.gpt-oss".to_string()),
            total_quantity: 1024,
            reserved_quantity: 0,
            available_quantity: 1024,
            delivery_state: "idle".to_string(),
            price_floor_sats: 21,
            terms_label: "spot session / single request".to_string(),
            forward_capacity_lot_id: Some("lot.forward.local.gpt-oss".to_string()),
            forward_delivery_window_label: Some("100..200".to_string()),
            forward_total_quantity: 256,
            forward_reserved_quantity: 0,
            forward_available_quantity: 256,
            forward_terms_label: Some("forward physical / single request window".to_string()),
        }
    }

    fn sandbox_row() -> ProviderInventoryRow {
        ProviderInventoryRow {
            target: ProviderComputeProduct::SandboxPythonExec,
            enabled: true,
            backend_ready: true,
            eligible: true,
            capability_summary:
                "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch"
                    .to_string(),
            source_badge: "sandbox_profiles".to_string(),
            capacity_lot_id: Some("lot.sandbox.python".to_string()),
            total_quantity: 64,
            reserved_quantity: 1,
            available_quantity: 63,
            delivery_state: "delivering".to_string(),
            price_floor_sats: 34,
            terms_label: "spot session / declared sandbox profile".to_string(),
            forward_capacity_lot_id: Some("lot.forward.sandbox.python".to_string()),
            forward_delivery_window_label: Some("300..400".to_string()),
            forward_total_quantity: 16,
            forward_reserved_quantity: 0,
            forward_available_quantity: 16,
            forward_terms_label: Some(
                "forward physical / declared sandbox profile window".to_string(),
            ),
        }
    }

    fn sandbox_profile() -> ProviderSandboxProfile {
        ProviderSandboxProfile {
            profile_id: "python-batch".to_string(),
            profile_digest: "sha256:python-batch".to_string(),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            runtime_family: "python3".to_string(),
            runtime_version: "Python 3.11.8".to_string(),
            sandbox_engine: "local_subprocess".to_string(),
            os_family: "linux".to_string(),
            arch: "x86_64".to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 120,
            network_mode: "none".to_string(),
            filesystem_mode: "workspace_only".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["python3".to_string()],
            toolchain_inventory: vec!["python3".to_string()],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
            runtime_kind: ProviderSandboxRuntimeKind::Python,
            runtime_ready: true,
            runtime_binary_path: Some("/usr/bin/python3".to_string()),
            capability_summary:
                "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch"
                    .to_string(),
        }
    }

    #[test]
    fn build_inventory_status_groups_local_cluster_and_sandbox_surfaces() {
        let status = build_inventory_status(InventoryStatusInput {
            uses_remote_kernel_projection: true,
            projection: Some(InventoryProjectionCounts {
                latest_snapshot_id: Some("snapshot.compute.1".to_string()),
                compute_products_active: 3,
                compute_capacity_lots_open: 2,
                compute_capacity_lots_delivering: 1,
                compute_inventory_quantity_open: 1088,
                compute_inventory_quantity_reserved: 1,
                compute_inventory_quantity_delivering: 1,
                compute_delivery_proofs_24h: 8,
                compute_validator_challenges_open: 2,
            }),
            inventory_rows: &[local_row(), sandbox_row()],
            sandbox_profiles: &[sandbox_profile()],
            gpt_oss_ready_model: Some("gpt-oss-20b"),
            gpt_oss_configured_model: Some("gpt-oss-20b"),
            apple_fm_ready_model: None,
            cluster_available: false,
            cluster_topology_label: "not_integrated",
            cluster_member_count: 0,
            cluster_last_error: Some(CLUSTER_NOT_INTEGRATED_REASON),
        });

        assert_eq!(status.authority, "kernel_projected");
        assert_eq!(status.sections.len(), 3);
        assert_eq!(status.projection.source, "kernel_projection");
        assert_eq!(
            status
                .sections
                .iter()
                .find(|section| section.section_id == "local")
                .map(|section| section.product_count),
            Some(1)
        );
        assert_eq!(
            status
                .sections
                .iter()
                .find(|section| section.section_id == "sandbox")
                .map(|section| section.summary.as_str()),
            Some("profiles=1 ready_profiles=1 products=1 ready=1 eligible=1 open_quantity=63")
        );
        assert_eq!(
            status
                .sections
                .iter()
                .find(|section| section.section_id == "cluster")
                .and_then(|section| section.blocker_reason.as_deref()),
            Some(CLUSTER_NOT_INTEGRATED_REASON)
        );
        assert_eq!(
            status.sections[0].products[0]
                .environment_binding
                .as_deref(),
            Some("model:gpt-oss-20b")
        );
        assert_eq!(
            status.sections[2].products[0]
                .environment_binding
                .as_deref(),
            Some("profiles:python-batch")
        );
    }

    #[test]
    fn inventory_detail_lines_surface_projection_and_blockers() {
        let status = super::DesktopControlInventoryStatus {
            authority: "local_only".to_string(),
            projection: super::DesktopControlInventoryProjectionStatus {
                source: "local_only".to_string(),
                latest_snapshot_id: None,
                ..super::DesktopControlInventoryProjectionStatus::default()
            },
            sections: vec![DesktopControlInventorySectionStatus {
                section_id: "cluster".to_string(),
                label: "Cluster".to_string(),
                available: false,
                blocker_reason: Some(CLUSTER_NOT_INTEGRATED_REASON.to_string()),
                summary: "topology=not_integrated members=0".to_string(),
                product_count: 0,
                ready_product_count: 0,
                eligible_product_count: 0,
                open_quantity: 0,
                products: Vec::new(),
            }],
        };

        let lines = inventory_detail_lines(&status);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("Inventory authority: local_only"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Projection: source=local_only"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Cluster blocker: cluster transport is not integrated"))
        );
    }
}
