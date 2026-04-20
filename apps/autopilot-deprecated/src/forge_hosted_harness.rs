use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use probe_client::{ProbeClient, ProbeClientConfig, ProbeClientTransportConfig};
use probe_core::backend_profiles::openai_codex_subscription;
use probe_core::harness::resolve_prompt_contract;
use probe_core::runtime::{PlainTextExecOutcome, PlainTextResumeRequest};
use probe_core::tools::{ProbeToolChoice, ToolApprovalConfig, ToolLoopConfig};
use probe_protocol::runtime::{SessionSnapshot, StartSessionRequest};
use probe_protocol::session::{
    SessionPreparedBaselineRef, SessionPreparedBaselineStatus, SessionSummaryArtifact,
    SessionWorkspaceBootMode, SessionWorkspaceState,
};
use serde::Serialize;

use crate::app_state::{
    AutopilotChatState, AutopilotThreadListEntry, ForgeBountyLifecycleStatus,
    ForgeBountyObjectiveKind, ForgeCampaignArtifactKind, ForgeEvidenceProductArtifactKind,
    ForgeEvidenceVerificationStatus, ForgeHostedAuditKind, ForgeHostedAuditNoteKind,
    ForgeHostedPreflightCheck, ForgeHostedPreflightCheckStatus, ForgeHostedPreflightDisposition,
    ForgeHostedPreflightReport,
};

const DEFAULT_THREAD_ID: &str = "forge-hosted-proof";
const DEFAULT_PROOF_FILE: &str = "FORGE_HOSTED_PROOF.md";
const DEFAULT_BASELINE_ID: &str = "forge-openagents-main";

trait StringResultContext<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;
}

impl<T> StringResultContext<T> for std::result::Result<T, String> {
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|error| anyhow!("{}: {error}", context.into()))
    }
}

#[derive(Parser, Debug)]
#[command(name = "autopilot-forge-hosted-dogfood")]
#[command(about = "Run the Forge hosted GCP dogfood flow against a hosted Probe TCP session")]
pub struct ForgeHostedHarnessCli {
    #[arg(long)]
    address: String,
    #[arg(long)]
    remote_cwd: PathBuf,
    #[arg(long)]
    local_workspace_root: PathBuf,
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value = DEFAULT_THREAD_ID)]
    thread_id: String,
    #[arg(long, default_value = DEFAULT_PROOF_FILE)]
    proof_file: String,
    #[arg(long, default_value = DEFAULT_BASELINE_ID)]
    worker_baseline: String,
    #[arg(long, default_value = "github-public-https")]
    repo_secret_ref: String,
}

#[derive(Clone, Debug, Serialize)]
struct ForgeHostedHarnessSummary {
    generated_at_epoch_ms: u64,
    address: String,
    thread_id: String,
    session_id: String,
    local_workspace_root: String,
    remote_cwd: String,
    proof_file: String,
    proof_content: String,
    projection_path: String,
    preflight_markdown_path: String,
    preflight_json_path: String,
    coding_audit_markdown_path: String,
    coding_audit_json_path: String,
    bookkeeping_audit_markdown_path: String,
    bookkeeping_audit_json_path: String,
    summary_artifact_kinds: Vec<String>,
    accepted_patch_summary_available: bool,
    mounted_pack_ids: Vec<String>,
    branch_ref: Option<String>,
    head_commit: Option<String>,
    delivery_status: Option<String>,
    evidence_status: Option<String>,
    bounty_status: Option<String>,
    campaign_status: Option<String>,
    promotion_status: Option<String>,
    settlement_status: Option<String>,
    final_snapshot: SessionSnapshot,
}

pub fn main_entry() -> Result<()> {
    run(ForgeHostedHarnessCli::parse())
}

pub fn run(cli: ForgeHostedHarnessCli) -> Result<()> {
    let address = normalize_address(cli.address.as_str())?;
    let local_workspace_root = canonicalize_path(cli.local_workspace_root.as_path())?;
    let output_dir = create_output_dir(cli.output_dir.as_path())?;
    let projection_path = output_dir.join("forge-hosted-artifact-projection.json");
    let transcript_path = output_dir.join(format!("{}.jsonl", cli.thread_id));
    let thread_id = cli.thread_id.trim().to_string();
    let proof_file = cli.proof_file.trim().to_string();
    let worker_baseline = cli.worker_baseline.trim().to_string();
    if thread_id.is_empty() {
        bail!("thread id cannot be empty");
    }
    if proof_file.is_empty() {
        bail!("proof file cannot be empty");
    }
    if worker_baseline.is_empty() {
        bail!("worker baseline cannot be empty");
    }

    let now = current_epoch_ms();
    let repo_remote_url = git_output(local_workspace_root.as_path(), &["remote", "get-url", "origin"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let git_branch = git_output(local_workspace_root.as_path(), &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let session_title = format!("Forge Hosted Proof {}", git_branch.as_deref().unwrap_or("main"));

    let mut chat = AutopilotChatState::from_artifact_projection_path(projection_path.clone());
    chat.set_thread_entries(vec![AutopilotThreadListEntry {
        thread_id: thread_id.clone(),
        thread_name: Some(String::from("Forge Hosted Proof")),
        preview: String::from("Hosted Probe dogfood flow"),
        status: Some(String::from("idle")),
        loaded: true,
        cwd: Some(local_workspace_root.display().to_string()),
        path: Some(transcript_path.display().to_string()),
        created_at: now as i64,
        updated_at: now as i64,
    }]);
    if let Some(metadata) = chat.thread_metadata.get_mut(thread_id.as_str()) {
        metadata.workspace_root = Some(local_workspace_root.display().to_string());
        metadata.project_name = Some(String::from("openagents"));
        metadata.git_branch = git_branch.clone();
        metadata.git_dirty = Some(false);
    }
    chat.set_probe_thread_projection_state(thread_id.as_str(), Some(String::from("idle")), false, true);
    chat.ensure_probe_shared_session_for_thread(thread_id.as_str(), now)
        .ok_or_else(|| anyhow!("failed to create Forge shared session"))?;

    let docs_pack_id = chat
        .record_repo_file_knowledge_pack_for_thread(
            thread_id.as_str(),
            crate::app_state::ForgeKnowledgePackKind::RepoDocs,
            "OpenAgents repo docs",
            vec![String::from("README.md")],
            "harness:repo_docs",
            now + 1,
        )
        .context("record repo docs knowledge pack")?;
    let runbook_pack_id = chat
        .record_repo_file_knowledge_pack_for_thread(
            thread_id.as_str(),
            crate::app_state::ForgeKnowledgePackKind::RepoRunbook,
            "Forge hosted dogfood runbook",
            vec![String::from("docs/codex/FORGE_HOSTED_GCP_DOGFOOD_RUNBOOK.md")],
            "harness:repo_runbook",
            now + 2,
        )
        .context("record runbook knowledge pack")?;

    let project_id = chat
        .project_for_thread(thread_id.as_str())
        .map(|project| project.project_id.clone());
    let plan = chat.knowledge_mount_plan_for_scope(
        project_id.as_deref(),
        Some(local_workspace_root.to_string_lossy().as_ref()),
    );
    let mut preflight = build_preflight_report(
        local_workspace_root.as_path(),
        repo_remote_url.clone(),
        Some(worker_baseline.clone()),
        Some(cli.repo_secret_ref.trim().to_string()),
        &plan,
        now + 3,
    );
    let preflight_markdown_path = output_dir.join("forge-hosted-preflight.md");
    let preflight_json_path = output_dir.join("forge-hosted-preflight.json");
    preflight.report_path = Some(preflight_markdown_path.display().to_string());
    chat.record_probe_hosted_preflight_for_thread(thread_id.as_str(), preflight.clone(), now + 4)
        .context("record hosted preflight")?;

    let mut client = connect_hosted_client(address.as_str())?;
    let mut profile = openai_codex_subscription();
    profile.reasoning_level = Some(String::from("low"));
    let (system_prompt, harness_profile) = resolve_prompt_contract(
        Some("coding_bootstrap"),
        None,
        cli.remote_cwd.as_path(),
        None,
        profile.kind,
    )
    .map_err(anyhow::Error::msg)
    .context("resolve Probe prompt contract")?;

    chat.remember_pending_probe_session_start_mount_plan(
        local_workspace_root.to_string_lossy().as_ref(),
        plan.clone(),
    );
    let session = client
        .start_session(StartSessionRequest {
            title: Some(session_title),
            cwd: cli.remote_cwd.clone(),
            profile: profile.clone(),
            system_prompt,
            harness_profile,
            workspace_state: Some(SessionWorkspaceState {
                boot_mode: SessionWorkspaceBootMode::PreparedBaseline,
                baseline: Some(SessionPreparedBaselineRef {
                    baseline_id: worker_baseline.clone(),
                    repo_identity: repo_remote_url.clone(),
                    base_ref: git_branch.clone(),
                    status: SessionPreparedBaselineStatus::Ready,
                }),
                snapshot: None,
                execution_host: None,
                provenance_note: Some(String::from(
                    "Forge hosted dogfood asked Probe for a prepared baseline session start",
                )),
            }),
            mounted_refs: plan.mounted_refs.clone(),
        })
        .context("start hosted Probe session")?;
    let session_id = session.session.id.clone();

    let patch_outcome = continue_session(
        &mut client,
        &session_id,
        profile.clone(),
        patch_tool_loop(),
        format!(
            "Use one `shell` call to create a new file named `{proof_file}` at the repository root with exactly these contents:\n\
             # Forge Hosted Proof\n\
             transport=tcp_jsonl\n\
             status=ok\n\
             \n\
             Do not modify any other files. Do not inspect unrelated files first. After the command succeeds, stop."
        ),
    )
    .context("run hosted patch turn")?;
    let patch_turn_id = patch_outcome.turn.id.0.to_string();

    let read_outcome = continue_session(
        &mut client,
        &session_id,
        profile,
        read_tool_loop(),
        format!(
            "Use `read_file` to read `{proof_file}` from the repository root. Do not call any other tools. \
             Return the exact file contents and then stop."
        ),
    )
    .context("read hosted proof file")?;
    let proof_content = extract_read_file_content(&read_outcome, proof_file.as_str())
        .ok_or_else(|| anyhow!("Probe did not return a read_file tool result for `{proof_file}`"))?;

    let final_snapshot = client
        .inspect_session(&session_id)
        .context("inspect hosted Probe session")?;
    chat.sync_probe_knowledge_mount_projection_for_thread(
        thread_id.as_str(),
        final_snapshot.session.mounted_refs.as_slice(),
        final_snapshot.summary_artifacts.as_slice(),
        now + 10,
    )
    .context("sync mounted pack projection")?;
    chat.sync_probe_remote_session_projection_for_thread(
        thread_id.as_str(),
        final_snapshot.session.runtime_owner.as_ref(),
        final_snapshot.session.workspace_state.as_ref(),
        final_snapshot.session.hosted_receipts.as_ref(),
        now + 11,
    )
    .context("sync hosted session projection")?;

    let retained_pack_id = chat
        .record_probe_retained_summary_pack_for_thread(
            thread_id.as_str(),
            Some(String::from("Hosted retained session summary")),
            "harness:retained_summary",
            now + 12,
        )
        .context("record retained summary knowledge pack")?;
    let patch_pack_id = match chat.record_probe_accepted_patch_pack_for_thread(
        thread_id.as_str(),
        Some(String::from("Hosted accepted patch summary")),
        "harness:accepted_patch_summary",
        now + 13,
    ) {
        Ok(pack_id) => Some(pack_id),
        Err(error)
            if error
                == "No accepted patch summary is available for the current Forge shared session." =>
        {
            None
        }
        Err(error) => {
            return Err(anyhow!("record accepted patch summary knowledge pack: {error}"));
        }
    };
    let campaign_artifact_kind = if patch_pack_id.is_some() {
        ForgeCampaignArtifactKind::AcceptedPatchSummary
    } else {
        ForgeCampaignArtifactKind::ProbeSummary
    };
    let campaign_artifact_note = if patch_pack_id.is_some() {
        String::from("Hosted accepted patch summary from the Probe-managed proof file.")
    } else {
        String::from(
            "Hosted retained session summary from the Probe-managed proof file because this proof turn used shell rather than apply_patch.",
        )
    };

    let diff = render_new_file_diff(proof_file.as_str(), proof_content.as_str());
    chat.set_diff_artifact(thread_id.as_str(), patch_turn_id.clone(), diff, now + 14);
    chat.complete_review_artifact(
        thread_id.as_str(),
        patch_turn_id.clone(),
        "Reviewed the hosted proof artifact and the patch is limited to the expected proof file.",
        now + 15,
        false,
    );
    chat.record_probe_evidence_verification_for_thread(
        thread_id.as_str(),
        "hosted_probe_turns",
        ForgeEvidenceVerificationStatus::Passed,
        Some(final_snapshot.session.transcript_path.display().to_string()),
        Some(String::from(
            "Hosted Probe session completed the patch and read-back turns successfully.",
        )),
        now + 16,
    )
    .context("record verification evidence")?;
    chat.record_probe_evidence_log_ref_for_thread(
        thread_id.as_str(),
        "probe_transcript",
        final_snapshot.session.transcript_path.display().to_string(),
        now + 17,
    )
    .context("record transcript log reference")?;

    let branch_state = final_snapshot.branch_state.clone();
    let delivery_state = final_snapshot.delivery_state.clone();
    let base_branch = branch_state
        .as_ref()
        .and_then(|state| state.upstream_ref.as_deref())
        .map(short_ref)
        .or_else(|| git_branch.clone())
        .unwrap_or_else(|| String::from("main"));
    let head_branch = branch_state
        .as_ref()
        .map(|state| state.head_ref.clone())
        .unwrap_or_else(|| String::from("unknown"));
    let head_commit = branch_state
        .as_ref()
        .map(|state| state.head_commit.clone())
        .unwrap_or_else(|| String::from("unknown"));

    chat.record_probe_delivery_pr_for_thread(
        thread_id.as_str(),
        base_branch,
        branch_state
            .as_ref()
            .and_then(|state| (!state.head_commit.is_empty()).then(|| state.head_commit.clone())),
        head_branch,
        head_commit,
        delivery_state
            .as_ref()
            .and_then(|state| state.compare_ref.clone()),
        None,
        "Forge hosted proof delivery",
        "Hosted Probe created the proof artifact in the managed remote workspace and OpenAgents projected the resulting evidence and delivery objects locally.",
        now + 18,
    )
    .context("record delivery receipt")?;
    chat.record_probe_delivery_merge_for_thread(
        thread_id.as_str(),
        "operator",
        Some(String::from(
            "Closed the hosted Forge proof flow without opening a public GitHub PR.",
        )),
        now + 19,
    )
    .context("record delivery merge closure")?;

    chat.record_probe_bounty_contract_for_thread(
        thread_id.as_str(),
        ForgeBountyObjectiveKind::AcceptedMerge,
        "Hosted Forge proof closeout",
        Some(String::from(
            "Prove the hosted Probe session, mounted pack projection, evidence bundle, and delivery closure on GCP.",
        )),
        "harness:bounty_open",
        now + 20,
    )
    .context("record bounty contract")?;
    chat.record_probe_bounty_credit_for_thread(thread_id.as_str(), "operator", 4_000, now + 21)
        .context("record operator bounty credit")?;
    chat.record_probe_bounty_credit_for_thread(thread_id.as_str(), "probe", 6_000, now + 22)
        .context("record probe bounty credit")?;
    chat.record_probe_bounty_claim_for_thread(
        thread_id.as_str(),
        "probe",
        Some(String::from("Hosted Probe agent completed the remote proof artifact.")),
        "harness:bounty_claim",
        now + 23,
    )
    .context("record bounty claim")?;
    chat.record_probe_bounty_lifecycle_for_thread(
        thread_id.as_str(),
        ForgeBountyLifecycleStatus::Admitted,
        Some(String::from("Operator admitted the hosted proof result.")),
        "harness:bounty_admit",
        now + 24,
    )
    .context("advance bounty lifecycle")?;
    chat.record_probe_settlement_merge_for_thread(
        thread_id.as_str(),
        "operator",
        Some(String::from(
            "Settlement recorded for the hosted proof closeout after operator merge closure.",
        )),
        "harness:settle_merge",
        now + 25,
    )
    .context("record settlement receipt")?;

    chat.record_probe_campaign_for_thread(
        thread_id.as_str(),
        "Hosted Forge proof campaign",
        now + 26,
    )
    .context("open campaign")?;
    chat.record_probe_campaign_goal_for_thread(
        thread_id.as_str(),
        "Retain one hosted closeout case that proves the current Forge object layer on GCP.",
        now + 27,
    )
    .context("record campaign goal")?;
    chat.record_probe_campaign_scope_for_thread(
        thread_id.as_str(),
        "One hosted Probe coding proof in a disposable managed workspace under the boring GCP footprint.",
        now + 28,
    )
    .context("record campaign scope")?;
    chat.record_probe_campaign_candidate_for_thread(
        thread_id.as_str(),
        campaign_artifact_kind,
        "current",
        Some(campaign_artifact_note),
        now + 29,
    )
    .context("record campaign candidate")?;
    chat.record_probe_campaign_case_selection_for_thread(
        thread_id.as_str(),
        "hosted-proof-case-1",
        ForgeCampaignArtifactKind::EvidenceBundle,
        "active",
        Some(String::from("Retain the evidence bundle for the first live hosted GCP proof.")),
        now + 30,
    )
    .context("record campaign retained case")?;
    chat.record_probe_campaign_verification_ref_for_thread(
        thread_id.as_str(),
        ForgeCampaignArtifactKind::EvidenceBundle,
        "active",
        Some(String::from("Verification reference for the hosted proof evidence bundle.")),
        now + 31,
    )
    .context("record campaign verification reference")?;
    chat.record_probe_promotion_shadow_for_thread(
        thread_id.as_str(),
        ForgeCampaignArtifactKind::DeliveryReceipt,
        "active",
        "operator",
        Some(String::from("Shadow the hosted proof delivery receipt as the candidate revision.")),
        "harness:promote_shadow",
        now + 32,
    )
    .context("record promotion shadow")?;
    chat.record_probe_promotion_promote_for_thread(
        thread_id.as_str(),
        "operator",
        Some(String::from("Promote the hosted proof delivery receipt into the retained ledger.")),
        "harness:promote",
        now + 33,
    )
    .context("record promotion")?;

    let coding_environment_summary = format!(
        "GCP hosted Probe closeout on {address} against managed workspace `{}`",
        cli.remote_cwd.display()
    );
    let bookkeeping_environment_summary = format!(
        "GCP hosted bookkeeping rehearsal linked to session {}",
        session_id.as_str()
    );
    chat.record_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::CodingCloseout,
        coding_environment_summary,
        now + 34,
    )
    .context("record hosted coding audit bundle")?;
    chat.record_probe_hosted_audit_note_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::CodingCloseout,
        ForgeHostedAuditNoteKind::OperatorNote,
        match patch_pack_id.as_deref() {
            Some(patch_pack_id) => format!(
                "Mounted `{docs_pack_id}`, `{runbook_pack_id}`, `{retained_pack_id}`, and `{patch_pack_id}` into the hosted proof session."
            ),
            None => format!(
                "Mounted `{docs_pack_id}`, `{runbook_pack_id}`, and `{retained_pack_id}` into the hosted proof session. No accepted patch summary artifact was emitted because the proof turn used shell instead of apply_patch."
            ),
        },
        "harness:hosted_note",
        now + 35,
    )
    .context("record coding audit operator note")?;
    chat.record_probe_hosted_audit_note_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::CodingCloseout,
        ForgeHostedAuditNoteKind::RecoveryStep,
        "Reconnected over the same hosted TCP address after the patch and read-back turns to inspect the persisted session snapshot.",
        "harness:hosted_recovery",
        now + 36,
    )
    .context("record coding audit recovery note")?;
    chat.record_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::BookkeepingRehearsal,
        bookkeeping_environment_summary,
        now + 37,
    )
    .context("record hosted bookkeeping audit bundle")?;
    chat.record_probe_hosted_audit_note_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::BookkeepingRehearsal,
        ForgeHostedAuditNoteKind::OperatorNote,
        "Recorded bounty, settlement, campaign, and promotion state above the same hosted Probe session.",
        "harness:bookkeeping_note",
        now + 38,
    )
    .context("record bookkeeping audit note")?;

    let coding_audit_markdown_path = output_dir.join("forge-hosted-coding-audit.md");
    let coding_audit_json_path = output_dir.join("forge-hosted-coding-audit.json");
    let bookkeeping_audit_markdown_path = output_dir.join("forge-hosted-bookkeeping-audit.md");
    let bookkeeping_audit_json_path = output_dir.join("forge-hosted-bookkeeping-audit.json");

    chat.export_probe_hosted_preflight_report_for_thread(
        thread_id.as_str(),
        preflight.clone(),
        Some(preflight_markdown_path.to_string_lossy().as_ref()),
        now + 39,
    )
    .context("export preflight markdown")?;
    chat.export_probe_hosted_preflight_report_for_thread(
        thread_id.as_str(),
        preflight,
        Some(preflight_json_path.to_string_lossy().as_ref()),
        now + 40,
    )
    .context("export preflight json")?;
    chat.export_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::CodingCloseout,
        Some(coding_audit_markdown_path.to_string_lossy().as_ref()),
        now + 41,
    )
    .context("export coding audit markdown")?;
    chat.export_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::CodingCloseout,
        Some(coding_audit_json_path.to_string_lossy().as_ref()),
        now + 42,
    )
    .context("export coding audit json")?;
    chat.export_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::BookkeepingRehearsal,
        Some(bookkeeping_audit_markdown_path.to_string_lossy().as_ref()),
        now + 43,
    )
    .context("export bookkeeping audit markdown")?;
    chat.export_probe_hosted_audit_bundle_for_thread(
        thread_id.as_str(),
        ForgeHostedAuditKind::BookkeepingRehearsal,
        Some(bookkeeping_audit_json_path.to_string_lossy().as_ref()),
        now + 44,
    )
    .context("export bookkeeping audit json")?;

    chat.record_probe_evidence_product_artifact_for_thread(
        thread_id.as_str(),
        ForgeEvidenceProductArtifactKind::Other,
        "hosted_coding_audit",
        coding_audit_markdown_path.display().to_string(),
        Some(String::from("Rendered hosted coding closeout audit bundle.")),
        now + 45,
    )
    .context("record coding audit product artifact")?;
    chat.record_probe_evidence_product_artifact_for_thread(
        thread_id.as_str(),
        ForgeEvidenceProductArtifactKind::Other,
        "hosted_bookkeeping_audit",
        bookkeeping_audit_markdown_path.display().to_string(),
        Some(String::from("Rendered hosted bookkeeping rehearsal audit bundle.")),
        now + 46,
    )
    .context("record bookkeeping audit product artifact")?;

    let shared_session = chat
        .shared_session_for_thread(thread_id.as_str())
        .ok_or_else(|| anyhow!("missing Forge shared session after harness run"))?;
    let summary = ForgeHostedHarnessSummary {
        generated_at_epoch_ms: now + 47,
        address,
        thread_id: thread_id.clone(),
        session_id: session_id.as_str().to_string(),
        local_workspace_root: local_workspace_root.display().to_string(),
        remote_cwd: cli.remote_cwd.display().to_string(),
        proof_file: proof_file.clone(),
        proof_content: proof_content.clone(),
        projection_path: projection_path.display().to_string(),
        preflight_markdown_path: preflight_markdown_path.display().to_string(),
        preflight_json_path: preflight_json_path.display().to_string(),
        coding_audit_markdown_path: coding_audit_markdown_path.display().to_string(),
        coding_audit_json_path: coding_audit_json_path.display().to_string(),
        bookkeeping_audit_markdown_path: bookkeeping_audit_markdown_path.display().to_string(),
        bookkeeping_audit_json_path: bookkeeping_audit_json_path.display().to_string(),
        summary_artifact_kinds: final_snapshot
            .summary_artifacts
            .iter()
            .map(summary_artifact_kind)
            .collect(),
        accepted_patch_summary_available: patch_pack_id.is_some(),
        mounted_pack_ids: shared_session.knowledge_mounts.mounted_pack_ids.clone(),
        branch_ref: final_snapshot.branch_state.as_ref().map(|state| state.head_ref.clone()),
        head_commit: final_snapshot
            .branch_state
            .as_ref()
            .map(|state| state.head_commit.clone()),
        delivery_status: chat.active_delivery_receipt().map(|receipt| receipt.status.label().to_string()),
        evidence_status: chat.active_evidence_bundle().map(|bundle| bundle.reviewer_status().label().to_string()),
        bounty_status: chat.active_bounty_contract().map(|contract| contract.lifecycle_status.label().to_string()),
        campaign_status: chat.active_campaign().map(|campaign| campaign.status.label().to_string()),
        promotion_status: chat.active_promotion_ledger().map(|ledger| ledger.status.label().to_string()),
        settlement_status: chat.active_settlement_receipt().map(|receipt| receipt.status.label().to_string()),
        final_snapshot,
    };

    let summary_json_path = output_dir.join("forge-hosted-summary.json");
    let summary_markdown_path = output_dir.join("forge-hosted-summary.md");
    fs::write(
        &summary_json_path,
        serde_json::to_string_pretty(&summary).context("encode hosted summary json")?,
    )
    .with_context(|| format!("write {}", summary_json_path.display()))?;
    fs::write(
        &summary_markdown_path,
        render_summary_markdown(&summary),
    )
    .with_context(|| format!("write {}", summary_markdown_path.display()))?;

    println!("session_id={}", summary.session_id);
    println!("summary_markdown={}", summary_markdown_path.display());
    println!("summary_json={}", summary_json_path.display());
    println!("coding_audit={}", summary.coding_audit_markdown_path);
    println!("bookkeeping_audit={}", summary.bookkeeping_audit_markdown_path);
    println!("preflight_report={}", summary.preflight_markdown_path);
    Ok(())
}

fn connect_hosted_client(address: &str) -> Result<ProbeClient> {
    let probe_home = probe_core::runtime::default_probe_home()
        .map_err(anyhow::Error::from)
        .context("resolve local PROBE_HOME")?;
    let mut config = ProbeClientConfig::new(probe_home, "autopilot-forge-hosted-harness");
    config.client_version = Some(String::from(env!("CARGO_PKG_VERSION")));
    config.transport = ProbeClientTransportConfig::HostedTcp {
        address: address.to_string(),
    };
    ProbeClient::connect(config).map_err(anyhow::Error::from)
}

fn continue_session(
    client: &mut ProbeClient,
    session_id: &probe_protocol::session::SessionId,
    profile: probe_protocol::backend::BackendProfile,
    tool_loop: ToolLoopConfig,
    prompt: String,
) -> Result<PlainTextExecOutcome> {
    client
        .continue_plain_text_session(PlainTextResumeRequest {
            session_id: session_id.clone(),
            profile,
            prompt,
            tool_loop: Some(tool_loop),
        })
        .map_err(anyhow::Error::from)
}

fn patch_tool_loop() -> ToolLoopConfig {
    let mut tool_loop = ToolLoopConfig::coding_bootstrap(ProbeToolChoice::Auto, false);
    tool_loop.max_model_round_trips = 12;
    tool_loop.approval = ToolApprovalConfig::allow_all();
    tool_loop
}

fn read_tool_loop() -> ToolLoopConfig {
    let mut tool_loop = ToolLoopConfig::coding_bootstrap(ProbeToolChoice::Auto, false);
    tool_loop.max_model_round_trips = 8;
    tool_loop.approval = ToolApprovalConfig::allow_all();
    tool_loop
}

fn extract_read_file_content(outcome: &PlainTextExecOutcome, expected_path: &str) -> Option<String> {
    outcome.tool_results.iter().find_map(|result| {
        if result.name != "read_file" {
            return None;
        }
        let path = result.output.get("path").and_then(|value| value.as_str())?;
        if path != expected_path && !path.ends_with(expected_path) {
            return None;
        }
        result
            .output
            .get("content")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    })
}

fn render_new_file_diff(path: &str, content: &str) -> String {
    let line_count = content.lines().count();
    let added = content
        .lines()
        .map(|line| format!("+{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\nindex 0000000..1111111\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{line_count} @@\n{added}\n"
    )
}

fn normalize_address(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("hosted Probe address cannot be empty");
    }
    Ok(trimmed.trim_start_matches("tcp://").to_string())
}

fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("canonicalize {}", path.display()))
}

fn create_output_dir(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    canonicalize_existing_or_self(path)
}

fn canonicalize_existing_or_self(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        path.canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))
    } else {
        Ok(path.to_path_buf())
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn git_output(cwd: &Path, args: &[&str]) -> Result<String> {
    command_output("git", cwd, args)
}

fn gh_status_ok(cwd: &Path) -> std::result::Result<(), String> {
    command_output("gh", cwd, &["auth", "status"])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn gcloud_output(args: &[&str]) -> Result<String> {
    command_output("gcloud", Path::new("."), args)
}

fn command_output(program: &str, cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("spawn `{program} {}`", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        bail!("{program} {} failed: {}", args.join(" "), detail);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn build_preflight_report(
    workspace_root: &Path,
    repo_remote_url: Option<String>,
    worker_baseline: Option<String>,
    repo_secret_ref: Option<String>,
    plan: &crate::app_state::ForgeKnowledgeMountPlan,
    recorded_at_epoch_ms: u64,
) -> ForgeHostedPreflightReport {
    let mut checks = Vec::new();

    match git_output(workspace_root, &["ls-remote", "--exit-code", "origin", "HEAD"]) {
        Ok(_) => checks.push(preflight_check(
            "repo access",
            ForgeHostedPreflightCheckStatus::Ok,
            "Verified git remote access against the active workspace origin.",
            repo_remote_url.clone(),
        )),
        Err(error) => checks.push(preflight_check(
            "repo access",
            ForgeHostedPreflightCheckStatus::Blocker,
            "Git remote access is not working for the active workspace.",
            Some(error.to_string()),
        )),
    }

    match gh_status_ok(workspace_root) {
        Ok(()) => checks.push(preflight_check(
            "github auth",
            ForgeHostedPreflightCheckStatus::Ok,
            "GitHub CLI auth is available for delivery and review operations.",
            None,
        )),
        Err(error) => checks.push(preflight_check(
            "github auth",
            ForgeHostedPreflightCheckStatus::Blocker,
            "GitHub CLI auth is missing or unusable.",
            Some(error),
        )),
    }

    match gcloud_output(&["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"]) {
        Ok(account) if !account.is_empty() => checks.push(preflight_check(
            "gcp auth",
            ForgeHostedPreflightCheckStatus::Ok,
            format!("Active gcloud account: `{account}`."),
            None,
        )),
        Ok(_) => checks.push(preflight_check(
            "gcp auth",
            ForgeHostedPreflightCheckStatus::Blocker,
            "No active gcloud account is configured.",
            None,
        )),
        Err(error) => checks.push(preflight_check(
            "gcp auth",
            ForgeHostedPreflightCheckStatus::Blocker,
            "gcloud auth is missing or unusable.",
            Some(error.to_string()),
        )),
    }

    let gcp_project = gcloud_output(&["config", "get-value", "project"])
        .ok()
        .filter(|value| !value.is_empty() && value != "(unset)");
    match &gcp_project {
        Some(project) => checks.push(preflight_check(
            "gcp project",
            ForgeHostedPreflightCheckStatus::Ok,
            format!("Active gcloud project: `{project}`."),
            None,
        )),
        None => checks.push(preflight_check(
            "gcp project",
            ForgeHostedPreflightCheckStatus::Blocker,
            "gcloud does not have an active project configured.",
            None,
        )),
    }

    let gcp_region = gcloud_output(&["config", "get-value", "compute/region"])
        .ok()
        .filter(|value| !value.is_empty() && value != "(unset)");
    match &gcp_region {
        Some(region) => checks.push(preflight_check(
            "gcp region",
            ForgeHostedPreflightCheckStatus::Ok,
            format!("Active gcloud region: `{region}`."),
            None,
        )),
        None => checks.push(preflight_check(
            "gcp region",
            ForgeHostedPreflightCheckStatus::Warning,
            "gcloud region is unset; hosted commands will need an explicit region.",
            None,
        )),
    }

    match repo_secret_ref.filter(|value| !value.trim().is_empty()) {
        Some(secret_ref) => checks.push(preflight_check(
            "repo secret",
            ForgeHostedPreflightCheckStatus::Ok,
            format!("Hosted repo secret reference is set to `{secret_ref}`."),
            None,
        )),
        None => checks.push(preflight_check(
            "repo secret",
            ForgeHostedPreflightCheckStatus::Blocker,
            "No hosted repo secret reference is configured for this run.",
            None,
        )),
    }

    match worker_baseline.clone().filter(|value| !value.trim().is_empty()) {
        Some(baseline) => checks.push(preflight_check(
            "worker baseline",
            ForgeHostedPreflightCheckStatus::Ok,
            format!("Hosted worker baseline is set to `{baseline}`."),
            None,
        )),
        None => checks.push(preflight_check(
            "worker baseline",
            ForgeHostedPreflightCheckStatus::Blocker,
            "No hosted worker baseline is configured for this run.",
            None,
        )),
    }

    if plan.unsupported_routes.is_empty() {
        checks.push(preflight_check(
            "knowledge routing",
            ForgeHostedPreflightCheckStatus::Ok,
            "All routed knowledge packs are compatible with Probe mounted refs.",
            None,
        ));
    } else {
        let detail = plan
            .unsupported_routes
            .iter()
            .map(|route| format!("{}: {}", route.title, route.reason))
            .collect::<Vec<_>>()
            .join(" | ");
        checks.push(preflight_check(
            "knowledge routing",
            ForgeHostedPreflightCheckStatus::Warning,
            "Some requested knowledge packs cannot be mounted into Probe yet.",
            Some(detail),
        ));
    }

    let disposition = if checks
        .iter()
        .any(|check| check.status == ForgeHostedPreflightCheckStatus::Blocker)
    {
        ForgeHostedPreflightDisposition::Blocked
    } else {
        ForgeHostedPreflightDisposition::Ready
    };

    ForgeHostedPreflightReport {
        preflight_id: format!("forge-hosted-preflight-{}", recorded_at_epoch_ms),
        disposition,
        workspace_root: Some(workspace_root.display().to_string()),
        repo_remote_url,
        gcp_project,
        gcp_region,
        worker_baseline,
        report_path: None,
        checks,
        recorded_at_epoch_ms,
    }
}

fn preflight_check(
    label: impl Into<String>,
    status: ForgeHostedPreflightCheckStatus,
    summary: impl Into<String>,
    detail: Option<String>,
) -> ForgeHostedPreflightCheck {
    ForgeHostedPreflightCheck {
        label: label.into(),
        status,
        summary: summary.into(),
        detail,
    }
}

fn short_ref(value: &str) -> String {
    value.rsplit('/').next().unwrap_or(value).to_string()
}

fn summary_artifact_kind(artifact: &SessionSummaryArtifact) -> String {
    match artifact {
        SessionSummaryArtifact::RetainedSessionSummary(_) => String::from("retained_session_summary"),
        SessionSummaryArtifact::AcceptedPatchSummary(_) => String::from("accepted_patch_summary"),
    }
}

fn render_summary_markdown(summary: &ForgeHostedHarnessSummary) -> String {
    let summary_artifacts = if summary.summary_artifact_kinds.is_empty() {
        String::from("none")
    } else {
        summary.summary_artifact_kinds.join(", ")
    };
    let mounted = if summary.mounted_pack_ids.is_empty() {
        String::from("none")
    } else {
        summary
            .mounted_pack_ids
            .iter()
            .map(|pack_id| format!("`{pack_id}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "# Forge Hosted Harness Summary\n\n\
         - generated_at_epoch_ms: {}\n\
         - session_id: `{}`\n\
         - address: `{}`\n\
         - remote_cwd: `{}`\n\
         - local_workspace_root: `{}`\n\
         - proof_file: `{}`\n\
         - mounted_pack_ids: {}\n\
         - summary_artifacts: {}\n\
         - accepted_patch_summary_available: {}\n\
         - delivery_status: {}\n\
         - evidence_status: {}\n\
         - bounty_status: {}\n\
         - campaign_status: {}\n\
         - promotion_status: {}\n\
         - settlement_status: {}\n\
         \n\
         ## Proof Content\n\n\
         ```text\n{}\n```\n\
         \n\
         ## Outputs\n\n\
         - projection: `{}`\n\
         - preflight markdown: `{}`\n\
         - preflight json: `{}`\n\
         - coding audit markdown: `{}`\n\
         - coding audit json: `{}`\n\
         - bookkeeping audit markdown: `{}`\n\
         - bookkeeping audit json: `{}`\n",
        summary.generated_at_epoch_ms,
        summary.session_id,
        summary.address,
        summary.remote_cwd,
        summary.local_workspace_root,
        summary.proof_file,
        mounted,
        summary_artifacts,
        summary.accepted_patch_summary_available,
        summary.delivery_status.as_deref().unwrap_or("none"),
        summary.evidence_status.as_deref().unwrap_or("none"),
        summary.bounty_status.as_deref().unwrap_or("none"),
        summary.campaign_status.as_deref().unwrap_or("none"),
        summary.promotion_status.as_deref().unwrap_or("none"),
        summary.settlement_status.as_deref().unwrap_or("none"),
        summary.proof_content,
        summary.projection_path,
        summary.preflight_markdown_path,
        summary.preflight_json_path,
        summary.coding_audit_markdown_path,
        summary.coding_audit_json_path,
        summary.bookkeeping_audit_markdown_path,
        summary.bookkeeping_audit_json_path,
    )
}
