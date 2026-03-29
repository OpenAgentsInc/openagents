use openagents_compiled_agent::ShadowMode;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::app_state::{
    ContributorBetaPaneState, ContributorBetaSubmissionRow, ContributorBetaTailnetNodeStatus,
    ContributorSubmissionOutcome, ContributorSubmissionSource, ContributorTrustTier,
    ContributorWorkerRole, ProviderMode, RenderState,
};
use crate::compiled_agent_slice::{
    CompiledAgentFeedbackSignal, CompiledAgentSliceState, run_compiled_agent_slice,
    run_compiled_agent_slice_with_feedback,
};
use crate::desktop_control::{DesktopControlTailnetDeviceStatus, desktop_control_tailnet_status};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;

const PSIONIC_CANDIDATE_LABEL: &str = "psionic_candidate";
const TAILNET_GOVERNED_RUN_DIGEST: &str =
    "dc9ab99b00fa05ae990693b5e758cc728d7d06dcef36bb51b86bf769c7f18b37";
const TAILNET_XTRAIN_RECEIPT_DIGEST: &str =
    "4f7655b1b65931c538c3fbea643452a8a16e1ad7738ae4a9e12896ef722cef45";

pub fn refresh_contributor_beta_state(state: &mut RenderState) {
    let contributor_id = contributor_id(state);
    let environment_class = environment_class();
    let capability_summary = capability_summary(state);
    let trust_tier = trust_tier(&state.contributor_beta);
    let pending_credit_sats = pending_credit_sats(&state.contributor_beta);
    let confirmed_credit_sats = confirmed_credit_sats(&state.contributor_beta);
    let payment_link_state = if confirmed_credit_sats > 0 {
        "credit_confirmed"
    } else if pending_credit_sats > 0 {
        "credit_provisional"
    } else {
        "not_earned"
    };
    let tailnet = desktop_control_tailnet_status();
    let tailnet_nodes = contributor_tailnet_nodes(&tailnet);

    let pane = &mut state.contributor_beta;
    pane.load_state = crate::app_state::PaneLoadState::Ready;
    if pane.identity_connected {
        pane.contributor_id = contributor_id;
        pane.contributor_credit_account_id = Some(format!(
            "credit://{}",
            pane.contributor_id.replace(':', "_")
        ));
    }
    pane.environment_class = environment_class;
    pane.capability_summary = capability_summary;
    pane.trust_tier = trust_tier;
    pane.pending_credit_sats = pending_credit_sats;
    pane.confirmed_credit_sats = confirmed_credit_sats;
    pane.payment_link_state = payment_link_state.to_string();
    pane.review_queue_depth = pane.review_submission_count + pane.quarantined_submission_count;
    pane.review_sla_label = "manual_triage_lt_24h".to_string();
    pane.provisional_credit_rulebook =
        "accepted=earned review=provisional rejected_or_quarantined=0".to_string();
    pane.tailnet_current_tailnet = tailnet.current_tailnet.clone();
    pane.tailnet_pilot_label = "Tailnet-first M5 + RTX 4080 governed beta".to_string();
    pane.tailnet_last_governed_run_digest = TAILNET_GOVERNED_RUN_DIGEST.to_string();
    pane.tailnet_last_xtrain_receipt_digest = TAILNET_XTRAIN_RECEIPT_DIGEST.to_string();
    pane.tailnet_nodes = tailnet_nodes;
}

pub fn connect_identity(state: &mut RenderState) {
    state.contributor_beta.identity_connected = true;
    refresh_contributor_beta_state(state);
    state.contributor_beta.last_action = Some(format!(
        "Connected Tailnet-first contributor identity {}",
        state.contributor_beta.contributor_id
    ));
    state.contributor_beta.last_error = None;
}

pub fn accept_contract(state: &mut RenderState) {
    if !state.contributor_beta.identity_connected {
        state.contributor_beta.last_error =
            Some("Connect contributor identity before accepting the beta contract.".to_string());
        return;
    }
    state.contributor_beta.contract_accepted = true;
    refresh_contributor_beta_state(state);
    state.contributor_beta.last_action = Some(format!(
        "Accepted Tailnet-first governed contributor contract {} for {}",
        state.contributor_beta.contract_version, state.contributor_beta.admitted_family
    ));
    state.contributor_beta.last_error = None;
}

pub fn cycle_worker_role(state: &mut RenderState) {
    let current_index = ContributorWorkerRole::ALL
        .iter()
        .position(|candidate| *candidate == state.contributor_beta.worker_role)
        .unwrap_or(0);
    state.contributor_beta.worker_role =
        ContributorWorkerRole::ALL[(current_index + 1) % ContributorWorkerRole::ALL.len()];
    refresh_contributor_beta_state(state);
    state.contributor_beta.last_action = Some(format!(
        "Selected external worker role {}",
        state.contributor_beta.worker_role.label()
    ));
    state.contributor_beta.last_error = None;
}

pub fn run_benchmark_pack(state: &mut RenderState) {
    if !assert_submission_prereqs(state, "benchmark pack") {
        return;
    }

    let slice_state = slice_state(state);
    let receipts = vec![
        run_compiled_agent_slice(
            "Can I go online right now?",
            &slice_state,
            ShadowMode::Disabled,
        ),
        run_compiled_agent_slice(
            "How many sats are in the wallet right now?",
            &slice_state,
            ShadowMode::Disabled,
        ),
        run_compiled_agent_slice(
            "Can you answer an unsupported Berlin readiness request?",
            &slice_state,
            ShadowMode::EvaluateCandidate {
                label: PSIONIC_CANDIDATE_LABEL.to_string(),
            },
        ),
    ];
    let digest = canonical_digest(&receipts);
    let summary = format!(
        "Ran Tailnet-first bounded benchmark pack with {} retained rows for {}",
        receipts.len(),
        state.contributor_beta.admitted_family
    );
    record_submission(
        state,
        ContributorSubmissionSource::BenchmarkPack,
        ContributorSubmissionOutcome::Accepted,
        summary,
        digest,
        None,
        None,
        None,
        None,
    );
    state.contributor_beta.last_action = Some(
        "Tailnet-first benchmark pack completed and submitted into the bounded contributor beta"
            .to_string(),
    );
    state.contributor_beta.last_error = None;
}

pub fn submit_runtime_receipt(state: &mut RenderState) {
    if !assert_submission_prereqs(state, "runtime receipt") {
        return;
    }

    let receipt = run_compiled_agent_slice_with_feedback(
        "How many sats are in the wallet right now?",
        &slice_state(state),
        ShadowMode::EvaluateCandidate {
            label: PSIONIC_CANDIDATE_LABEL.to_string(),
        },
        Some(CompiledAgentFeedbackSignal {
            disagreed: true,
            correction_text: Some(
                "Keep the answer grounded in the retained wallet facts and preserve the narrow contract."
                    .to_string(),
            ),
            reason_code: Some("grounded_synthesis_drift".to_string()),
            operator_note: Some(
                "Retain as a governed runtime disagreement receipt for the external beta.".to_string(),
            ),
        }),
    );
    let digest = canonical_digest(&receipt);
    let authority_path = Some(format!("{:?}", receipt.telemetry.authority_path).to_lowercase());
    let confidence_band =
        Some(format!("{:?}", receipt.telemetry.primary_confidence_band).to_lowercase());
    let source_receipt_id = Some(receipt.telemetry.lineage.receipt_id.clone());
    record_submission(
        state,
        ContributorSubmissionSource::RuntimeReceipt,
        ContributorSubmissionOutcome::Review,
        "Submitted governed Tailnet runtime disagreement receipt for review and replay."
            .to_string(),
        digest,
        authority_path.clone(),
        confidence_band.clone(),
        source_receipt_id.clone(),
        Some("grounded_synthesis_drift".to_string()),
    );
    state.contributor_beta.latest_runtime_receipt_id = source_receipt_id;
    state.contributor_beta.latest_runtime_authority_path = authority_path;
    state.contributor_beta.latest_runtime_confidence_band = confidence_band;
    state.contributor_beta.last_action =
        Some("Tailnet runtime disagreement receipt captured and routed into review.".to_string());
    state.contributor_beta.last_error = None;
}

pub fn run_worker_role(state: &mut RenderState) {
    if !assert_submission_prereqs(state, "worker role") {
        return;
    }

    let outcome = if state.contributor_beta.accepted_submission_count == 0 {
        ContributorSubmissionOutcome::Quarantined
    } else {
        ContributorSubmissionOutcome::Accepted
    };
    let review_reason = if outcome == ContributorSubmissionOutcome::Quarantined {
        Some("no_accepted_benchmark_lineage".to_string())
    } else {
        None
    };
    let role = state.contributor_beta.worker_role.label().to_string();
    let summary = format!(
        "Submitted Tailnet-first bounded external worker output for role {}",
        state.contributor_beta.worker_role.label()
    );
    let payload = format!(
        "{}:{}:{}:{}",
        state.contributor_beta.contributor_id,
        state.contributor_beta.contract_version,
        state.contributor_beta.environment_class,
        role
    );
    let digest = canonical_digest(&payload);
    record_submission(
        state,
        ContributorSubmissionSource::WorkerOutput,
        outcome,
        summary,
        digest,
        None,
        None,
        None,
        review_reason,
    );
    if let Some(last) = state.contributor_beta.submissions.first_mut() {
        last.worker_role = Some(role.clone());
    }
    state.contributor_beta.last_action = Some(format!(
        "Tailnet-first worker role {} submitted through the governed beta",
        role
    ));
    state.contributor_beta.last_error = None;
}

fn assert_submission_prereqs(state: &mut RenderState, label: &str) -> bool {
    if !state.contributor_beta.identity_connected {
        state.contributor_beta.last_error = Some(format!(
            "Connect contributor identity before submitting {}.",
            label
        ));
        return false;
    }
    if !state.contributor_beta.contract_accepted {
        state.contributor_beta.last_error = Some(format!(
            "Accept {} before submitting {}.",
            state.contributor_beta.contract_version, label
        ));
        return false;
    }
    true
}

fn record_submission(
    state: &mut RenderState,
    source: ContributorSubmissionSource,
    outcome: ContributorSubmissionOutcome,
    summary: String,
    digest: String,
    authority_path: Option<String>,
    confidence_band: Option<String>,
    source_receipt_id: Option<String>,
    review_reason: Option<String>,
) {
    let recorded_at_epoch_ms = current_epoch_millis();
    let submission_id = format!(
        "external.beta.submission.{:04}",
        state.contributor_beta.next_submission_seq
    );
    state.contributor_beta.next_submission_seq =
        state.contributor_beta.next_submission_seq.saturating_add(1);

    let row = ContributorBetaSubmissionRow {
        submission_id,
        source,
        outcome,
        summary,
        digest,
        recorded_at_epoch_ms,
        contract_version: state.contributor_beta.contract_version.clone(),
        contributor_id: state.contributor_beta.contributor_id.clone(),
        environment_class: state.contributor_beta.environment_class.clone(),
        capability_summary: state.contributor_beta.capability_summary.clone(),
        admitted_family: state.contributor_beta.admitted_family.clone(),
        authority_path,
        confidence_band,
        source_receipt_id,
        worker_role: None,
        review_reason,
    };
    state.contributor_beta.submissions.insert(0, row);
    if state.contributor_beta.submissions.len() > 8 {
        state.contributor_beta.submissions.truncate(8);
    }
    recompute_accounting(&mut state.contributor_beta);
    refresh_contributor_beta_state(state);
}

fn recompute_accounting(pane: &mut ContributorBetaPaneState) {
    pane.accepted_submission_count = pane
        .submissions
        .iter()
        .filter(|row| row.outcome == ContributorSubmissionOutcome::Accepted)
        .count() as u32;
    pane.rejected_submission_count = pane
        .submissions
        .iter()
        .filter(|row| row.outcome == ContributorSubmissionOutcome::Rejected)
        .count() as u32;
    pane.quarantined_submission_count = pane
        .submissions
        .iter()
        .filter(|row| row.outcome == ContributorSubmissionOutcome::Quarantined)
        .count() as u32;
    pane.review_submission_count = pane
        .submissions
        .iter()
        .filter(|row| row.outcome == ContributorSubmissionOutcome::Review)
        .count() as u32;
}

fn trust_tier(pane: &ContributorBetaPaneState) -> ContributorTrustTier {
    if pane.rejected_submission_count > 0 || pane.quarantined_submission_count > 0 {
        ContributorTrustTier::Caution
    } else if pane.accepted_submission_count >= 2 {
        ContributorTrustTier::Governed
    } else {
        ContributorTrustTier::Pending
    }
}

fn pending_credit_sats(pane: &ContributorBetaPaneState) -> u64 {
    pane.submissions
        .iter()
        .map(|row| match row.outcome {
            ContributorSubmissionOutcome::Accepted => match row.source {
                ContributorSubmissionSource::BenchmarkPack => 180,
                ContributorSubmissionSource::WorkerOutput => 220,
                ContributorSubmissionSource::RuntimeReceipt => 140,
            },
            ContributorSubmissionOutcome::Review => 40,
            ContributorSubmissionOutcome::Rejected | ContributorSubmissionOutcome::Quarantined => 0,
        })
        .sum()
}

fn confirmed_credit_sats(pane: &ContributorBetaPaneState) -> u64 {
    pane.submissions
        .iter()
        .filter(|row| row.outcome == ContributorSubmissionOutcome::Accepted)
        .map(|row| match row.source {
            ContributorSubmissionSource::BenchmarkPack => 180,
            ContributorSubmissionSource::WorkerOutput => 220,
            ContributorSubmissionSource::RuntimeReceipt => 140,
        })
        .sum()
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn contributor_id(state: &RenderState) -> String {
    state
        .nostr_identity
        .as_ref()
        .map(|identity| format!("npub://{}", identity.public_key_hex))
        .unwrap_or_else(|| {
            format!(
                "contributor.local.{}.{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })
}

fn environment_class() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn capability_summary(state: &RenderState) -> String {
    let mut capabilities = vec![
        "compiled_agent_benchmark_kit".to_string(),
        "tailnet_governed_beta".to_string(),
    ];
    if state.apple_fm_execution.reachable {
        capabilities.push("apple_fm_runtime".to_string());
    }
    if state.gpt_oss_execution.reachable {
        capabilities.push("gpt_oss_runtime".to_string());
    }
    if matches!(state.provider_runtime.mode, ProviderMode::Online) {
        capabilities.push("runtime_receipt_collector".to_string());
    }
    capabilities.push(format!(
        "worker:{}",
        state.contributor_beta.worker_role.label()
    ));
    capabilities.join(" + ")
}

fn contributor_tailnet_nodes(
    tailnet: &crate::desktop_control::DesktopControlTailnetStatus,
) -> Vec<ContributorBetaTailnetNodeStatus> {
    let mut nodes = Vec::new();
    if let Some(local) = tailnet.self_device.as_ref() {
        nodes.push(build_tailnet_node(local, "coordinator"));
    }
    if let Some(remote) = tailnet
        .peers
        .iter()
        .find(|peer| peer.host_name == "archlinux" || peer.display_name == "archlinux")
    {
        nodes.push(build_tailnet_node(remote, "worker"));
    } else {
        nodes.push(ContributorBetaTailnetNodeStatus {
            device_name: "archlinux".to_string(),
            node_id: "archlinux".to_string(),
            machine_class: "consumer_gpu_cuda".to_string(),
            role: "worker".to_string(),
            status: "offline".to_string(),
            tailnet_ip: None,
        });
    }
    nodes
}

fn build_tailnet_node(
    device: &DesktopControlTailnetDeviceStatus,
    role: &str,
) -> ContributorBetaTailnetNodeStatus {
    ContributorBetaTailnetNodeStatus {
        device_name: device.display_name.clone(),
        node_id: device.node_id.clone(),
        machine_class: machine_class_for_device(device),
        role: role.to_string(),
        status: if device.online { "online" } else { "offline" }.to_string(),
        tailnet_ip: device.tailscale_ips.first().cloned(),
    }
}

fn machine_class_for_device(device: &DesktopControlTailnetDeviceStatus) -> String {
    let host = device.host_name.to_ascii_lowercase();
    let os = device.os.to_ascii_lowercase();
    if host.contains("archlinux") {
        "consumer_gpu_cuda".to_string()
    } else if os.contains("mac") || host.contains("macbook-pro-m5") {
        "apple_silicon_mlx".to_string()
    } else {
        format!("{}_{}", os, role_safe_host(host.as_str()))
    }
}

fn role_safe_host(host: &str) -> String {
    host.replace('.', "_").replace('-', "_")
}

fn slice_state(state: &RenderState) -> CompiledAgentSliceState {
    CompiledAgentSliceState {
        provider_ready: matches!(state.provider_runtime.mode, ProviderMode::Online),
        provider_blockers: if matches!(state.provider_runtime.mode, ProviderMode::Online) {
            Vec::new()
        } else {
            vec!["provider_not_ready".to_string()]
        },
        wallet_balance_sats: state
            .spark_wallet
            .balance
            .as_ref()
            .map(spark_total_balance_sats)
            .unwrap_or(0),
        recent_earnings_sats: state.earnings_scoreboard.sats_today,
    }
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize contributor beta digest payload");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_tier_advances_after_accepted_rows() {
        let mut pane = ContributorBetaPaneState::default();
        pane.submissions.push(ContributorBetaSubmissionRow {
            submission_id: "a".to_string(),
            source: ContributorSubmissionSource::BenchmarkPack,
            outcome: ContributorSubmissionOutcome::Accepted,
            summary: "accepted".to_string(),
            digest: "digest-a".to_string(),
            recorded_at_epoch_ms: 1,
            contract_version: pane.contract_version.clone(),
            contributor_id: pane.contributor_id.clone(),
            environment_class: pane.environment_class.clone(),
            capability_summary: pane.capability_summary.clone(),
            admitted_family: pane.admitted_family.clone(),
            authority_path: None,
            confidence_band: None,
            source_receipt_id: None,
            worker_role: None,
            review_reason: None,
        });
        pane.submissions.push(ContributorBetaSubmissionRow {
            submission_id: "b".to_string(),
            source: ContributorSubmissionSource::WorkerOutput,
            outcome: ContributorSubmissionOutcome::Accepted,
            summary: "accepted".to_string(),
            digest: "digest-b".to_string(),
            recorded_at_epoch_ms: 2,
            contract_version: pane.contract_version.clone(),
            contributor_id: pane.contributor_id.clone(),
            environment_class: pane.environment_class.clone(),
            capability_summary: pane.capability_summary.clone(),
            admitted_family: pane.admitted_family.clone(),
            authority_path: None,
            confidence_band: None,
            source_receipt_id: None,
            worker_role: Some(ContributorWorkerRole::ReplayGeneration.label().to_string()),
            review_reason: None,
        });
        recompute_accounting(&mut pane);
        assert_eq!(trust_tier(&pane), ContributorTrustTier::Governed);
        assert_eq!(pending_credit_sats(&pane), 400);
        assert_eq!(confirmed_credit_sats(&pane), 400);
    }

    #[test]
    fn rejected_rows_drop_trust_to_caution() {
        let pane = ContributorBetaPaneState {
            rejected_submission_count: 1,
            ..ContributorBetaPaneState::default()
        };
        assert_eq!(trust_tier(&pane), ContributorTrustTier::Caution);
    }

    #[test]
    fn contributor_tailnet_nodes_keeps_archlinux_worker_slot() {
        let tailnet = crate::desktop_control::DesktopControlTailnetStatus {
            self_device: Some(crate::desktop_control::DesktopControlTailnetDeviceStatus {
                node_id: "self".to_string(),
                display_name: "macbook-pro-m5".to_string(),
                dns_name: "macbook-pro-m5".to_string(),
                host_name: "macbook-pro-m5".to_string(),
                os: "macOS".to_string(),
                online: true,
                active: true,
                exit_node: false,
                relay: None,
                current_address: Some("100.127.107.31:41641".to_string()),
                tailscale_ips: vec!["100.127.107.31".to_string()],
                allowed_ips: Vec::new(),
                rx_bytes: 0,
                tx_bytes: 0,
                created_at: None,
                last_seen: None,
                last_write: None,
                last_handshake: None,
            }),
            peers: Vec::new(),
            ..crate::desktop_control::DesktopControlTailnetStatus::default()
        };
        let nodes = contributor_tailnet_nodes(&tailnet);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].role, "coordinator");
        assert_eq!(nodes[1].device_name, "archlinux");
        assert_eq!(nodes[1].status, "offline");
    }
}
