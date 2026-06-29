import { describe, expect, test } from "bun:test";

import {
  FORGE_PROTOCOL_SCHEMA_VERSION,
  decodeForgeControlPlaneScope,
  decodeForgeCoordinationIssueRow,
  decodeForgeCoordinationPrRow,
  decodeForgeCoordinationStatusRow,
  decodeForgeDispatchLeaseRow,
  decodeForgeGitAccessTokenRow,
  decodeForgeGitAccessTokenScopeRow,
  decodeForgeGitPackfileArchiveRow,
  decodeForgeDispatchCloseout,
  decodeForgeDispatchDecision,
  decodeForgeDispatchMessage,
  decodeForgeDispatchWorkItem,
  decodeForgePromotionDecisionReceipt,
  decodeForgeTenantRow,
  decodeForgeVerificationReceipt,
  decodeForgeMergeQueueLedgerRow,
  forgeControlPlaneScopes,
  forgeCoordinationStatusStateForNip34Kind,
  forgeCoordinationStatusStates,
  forgeNip34StatusKindForState,
} from "./index.js";

const at = "2026-06-28T16:00:00.000Z";

describe("@openagentsinc/forge-protocol", () => {
  test("exports the phase 0 schema version", () => {
    expect(FORGE_PROTOCOL_SCHEMA_VERSION).toBe(
      "openagents.forge.protocol.v0.1",
    );
  });

  test("round-trips NIP-34 status states and kinds", () => {
    for (const state of forgeCoordinationStatusStates) {
      const kind = forgeNip34StatusKindForState(state);
      expect(forgeCoordinationStatusStateForNip34Kind(kind)).toBe(state);
    }
  });

  test("keeps control-plane scopes separate from smart Git token scopes", () => {
    expect(forgeControlPlaneScopes).toContain("forge:promotion:decide");
    expect(decodeForgeControlPlaneScope("forge:work:write")).toBe(
      "forge:work:write",
    );
    expect(() => decodeForgeControlPlaneScope("git:receive-pack")).toThrow();
    expect(() => decodeForgeControlPlaneScope("git:admin")).toThrow();
  });

  test("decodes the D1 coordination source-of-truth row shapes", () => {
    expect(
      decodeForgeCoordinationIssueRow({
        tenant_ref: "tenant.openagents",
        issue_ref: "issue.forge.6746",
        github_issue_number: 6746,
        title: "D1 coordination schema",
        state: "open",
        priority_ref: "prio:0-pr-burndown",
        source_refs_json: JSON.stringify([
          "github:OpenAgentsInc/openagents#6746",
        ]),
        created_at: at,
        updated_at: at,
      }).issue_ref,
    ).toBe("issue.forge.6746");

    expect(
      decodeForgeCoordinationPrRow({
        tenant_ref: "tenant.openagents",
        pr_ref: "change.forge.test",
        issue_ref: "issue.forge.6746",
        change_ref: "change.forge.test",
        state: "ready",
        base_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
        patch_head: "9e0c9b2eaf84c821caf555cae233a0d27e94d4ac",
        verification_ref: "verification.forge.test",
        blocker_refs_json: "[]",
        source_refs_json: "[]",
        created_at: at,
        updated_at: at,
      }).state,
    ).toBe("ready");

    expect(
      decodeForgeCoordinationStatusRow({
        tenant_ref: "tenant.openagents",
        status_ref: "status.forge.test",
        subject_ref: "change.forge.test",
        nip34_kind: 1630,
        state: "open",
        actor_ref: "agent.public.test",
        source_refs_json: "[]",
        created_at: at,
      }).nip34_kind,
    ).toBe(1630);

    expect(
      decodeForgeDispatchLeaseRow({
        tenant_ref: "tenant.openagents",
        lease_ref: "lease.forge.test",
        work_ref: "issue.forge.6746",
        owner_agent_ref: "agent.public.test",
        state: "active",
        idempotency_key_hash: null,
        acquired_at: at,
        heartbeat_at: at,
        expires_at: "2026-06-28T16:10:00.000Z",
        released_at: null,
        source_refs_json: "[]",
      }).state,
    ).toBe("active");

    expect(
      decodeForgeMergeQueueLedgerRow({
        tenant_ref: "tenant.openagents",
        queue_ref: "queue.forge.test",
        base_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
        actual_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
        virtual_head: "9e0c9b2eaf84c821caf555cae233a0d27e94d4ac",
        state: "projected",
        next_promotion_ref: null,
        ready_json: "[]",
        blocked_json: "[]",
        source_refs_json: "[]",
        created_at: at,
        updated_at: at,
      }).state,
    ).toBe("projected");

    expect(
      decodeForgeGitPackfileArchiveRow({
        tenant_ref: "tenant.openagents",
        packfile_ref: "packfile.forge.test",
        repository_ref: "repo.openagents.openagents",
        change_ref: "change.forge.test",
        receive_pack_ref: "receive-pack.forge.test",
        artifact_r2_key:
          "private/forge/git-packfiles/tenant.openagents/repo.openagents.openagents/packfile.forge.test.pack",
        packfile_sha256: "a".repeat(64),
        packfile_bytes: 128,
        object_format: "sha1",
        command_count: 1,
        capabilities_json: '["report-status"]',
        ref_updates_json: "[]",
        source_refs_json: "[]",
        content_type: "application/x-git-packed-objects",
        visibility: "operator_only",
        created_at: at,
        updated_at: at,
      }).object_format,
    ).toBe("sha1");

    expect(
      decodeForgeTenantRow({
        tenant_ref: "tenant.openagents",
        display_name: "OpenAgents",
        state: "active",
        confidential_workspace_mode: "attested",
        attestation_ref: "attestation.forge.openagents.sgx.public",
        encrypted_knowledge_pack_ref:
          "knowledge-pack.forge.openagents.encrypted",
        refusal_reason: null,
        retention_policy_ref: "retention.forge.openagents.30d",
        created_at: at,
        updated_at: at,
      }).confidential_workspace_mode,
    ).toBe("attested");

    expect(
      decodeForgeGitAccessTokenRow({
        tenant_ref: "tenant.openagents",
        token_ref: "forge_git_token.test",
        subject_ref: "agent.public.test",
        repository_ref: "repo.openagents.openagents",
        token_hash: "b".repeat(64),
        token_prefix: "oa_forge_git_prefix",
        state: "active",
        created_at: at,
        expires_at: "2026-06-28T17:00:00.000Z",
        last_used_at: null,
        revoked_at: null,
        source_refs_json: "[]",
      }).repository_ref,
    ).toBe("repo.openagents.openagents");

    expect(
      decodeForgeGitAccessTokenScopeRow({
        tenant_ref: "tenant.openagents",
        token_ref: "forge_git_token.test",
        scope: "git:receive-pack",
        created_at: at,
      }).scope,
    ).toBe("git:receive-pack");
  });

  test("decodes Pylon-to-Forge dispatch messages", () => {
    const workItem = decodeForgeDispatchWorkItem({
      schema: "openagents.forge.dispatch.work_item.v0.1",
      tenant_ref: "tenant.openagents",
      dispatch_ref: "dispatch.forge.6751",
      work_ref: "work.forge.6751",
      issue_ref: "issue.forge.6751",
      objective_ref: "objective.forge.6751",
      objective_summary: "Implement the Pylon-to-Forge dispatch protocol",
      work_class: "codex_agent_task",
      payment_mode: "no-spend",
      capability_refs: ["capability.codex_cli"],
      git: {
        repository_ref: "repo.openagents.openagents",
        remote_url: "https://forge.openagents.com/openagents/openagents.git",
        base_ref: "refs/heads/main",
        base_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
        branch_ref: "refs/heads/forge/work/6751",
        receive_pack_ref: "receive-pack.forge.6751",
        git_access: {
          token_ref: "forge_git_token.6751",
          token_prefix: "oa_forge_git_dispatch",
          scopes: ["git:receive-pack"],
          expires_at: "2026-06-28T17:00:00.000Z",
          delivery: "out_of_band",
        },
      },
      verification_command: {
        command_ref: "verification-command.forge.6751",
        runner_ref: "forge.verification.runner.docker_bun.v0.1",
        working_directory: ".",
        args: ["bun", "run", "check:deploy"],
        timeout_seconds: 1800,
      },
      lease_ref: "lease.forge.6751",
      expires_at: "2026-06-28T17:00:00.000Z",
      created_at: at,
      source_refs: ["github:OpenAgentsInc/openagents#6751"],
    });
    expect(workItem.git.git_access.scopes).toEqual(["git:receive-pack"]);
    expect(decodeForgeDispatchMessage(workItem).schema).toBe(workItem.schema);

    const decision = decodeForgeDispatchDecision({
      schema: "openagents.forge.dispatch.decision.v0.1",
      tenant_ref: workItem.tenant_ref,
      dispatch_ref: workItem.dispatch_ref,
      work_ref: workItem.work_ref,
      lease_ref: workItem.lease_ref,
      pylon_ref: "pylon.local.codex.1",
      state: "accepted",
      accepted_at: at,
      rejected_at: null,
      blocker_refs: [],
      source_refs: workItem.source_refs,
    });
    expect(decision.accepted_at).toBe(at);

    const closeout = decodeForgeDispatchCloseout({
      schema: "openagents.forge.dispatch.closeout.v0.1",
      tenant_ref: workItem.tenant_ref,
      dispatch_ref: workItem.dispatch_ref,
      work_ref: workItem.work_ref,
      lease_ref: workItem.lease_ref,
      pylon_ref: "pylon.local.codex.1",
      status: "accepted",
      payment_mode: "no-spend",
      settlement_state: "not_applicable",
      payout_claim_allowed: false,
      change_ref: "change.forge.6751",
      packfile_ref: "packfile.forge.6751",
      verification_ref: "verification.forge.6751",
      artifact_refs: [],
      blocker_refs: [],
      build_refs: ["build.local.check-deploy"],
      closeout_refs: ["closeout.forge.6751"],
      preview_refs: [],
      proof_refs: ["proof.forge.6751"],
      receipt_refs: ["receipt.forge.6751"],
      result_refs: ["result.forge.6751"],
      summary_refs: ["summary.forge.6751"],
      test_refs: ["test.forge.6751"],
      source_refs: workItem.source_refs,
      redacted: true,
      completed_at: at,
    });
    expect(closeout.packfile_ref).toBe("packfile.forge.6751");
    expect(decodeForgeDispatchMessage(closeout).schema).toBe(closeout.schema);
  });

  test("decodes redacted verification and promotion decision receipts", () => {
    const verification = decodeForgeVerificationReceipt({
      schema: "openagents.forge.verification.receipt.v0.1",
      tenant_ref: "tenant.openagents",
      verification_ref: "verification.forge.6768",
      change_ref: "change.forge.6768",
      repository_ref: "repo.openagents.openagents",
      base_ref: "refs/heads/main",
      base_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
      head_ref: "refs/heads/forge/work/6768",
      head_head: "9e0c9b2eaf84c821caf555cae233a0d27e94d4ac",
      packfile_ref: "packfile.forge.6768",
      packfile_sha256: "c".repeat(64),
      executor_identity_ref: "pylon.local.codex.1",
      command_ref: "verification-command.forge.6768",
      command_args: ["bun", "run", "check:deploy"],
      exit_code: 0,
      verdict: "passed",
      started_at: at,
      completed_at: "2026-06-28T16:01:00.000Z",
      artifact_refs: ["artifact.forge.6768.logs"],
      log_sha256: "d".repeat(64),
      source_refs: ["github:OpenAgentsInc/openagents#6768"],
      redacted: true,
    });
    expect(verification.change_ref).toBe("change.forge.6768");
    expect(verification.packfile_sha256).toHaveLength(64);

    const promotion = decodeForgePromotionDecisionReceipt({
      schema: "openagents.forge.promotion.decision.v0.1",
      tenant_ref: "tenant.openagents",
      promotion_ref: "promotion.forge.6768",
      queue_ref: "queue.forge.main",
      change_ref: verification.change_ref,
      decision: "approved",
      base_head: verification.base_head,
      candidate_head: verification.head_head,
      promoted_head: verification.head_head,
      verification_ref: verification.verification_ref,
      gate_refs: ["gate.merge-deploy", "gate.issue-close-safe"],
      blocker_refs: [],
      decided_by_ref: "forge.service.promotion",
      decided_at: "2026-06-28T16:02:00.000Z",
      source_refs: verification.source_refs,
      redacted: true,
    });
    expect(promotion.decision).toBe("approved");
    expect(promotion.promoted_head).toBe(verification.head_head);
  });
});
