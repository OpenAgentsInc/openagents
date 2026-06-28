import { describe, expect, test } from "bun:test"
import {
  decodeForgeDispatchCloseout,
  decodeForgeDispatchDecision,
  decodeForgeDispatchWorkItem,
  type ForgeDispatchWorkItem,
} from "@openagentsinc/forge-protocol"
import type { AssignmentCloseout } from "./assignment.js"
import {
  FORGE_PYLON_DISPATCH_BACKEND_REF,
  forgeDispatchDecisionForPylon,
  forgeDispatchWorkItemToPylonLease,
  pylonCloseoutToForgeDispatchCloseout,
} from "./forge-dispatch-protocol.js"

const at = "2026-06-28T18:00:00.000Z"

const sampleWorkItem = (): ForgeDispatchWorkItem =>
  decodeForgeDispatchWorkItem({
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
        token_prefix: "oa_forge_git_visible",
        scopes: ["git:receive-pack"],
        expires_at: "2026-06-28T19:00:00.000Z",
        delivery: "out_of_band",
      },
    },
    verification_command: {
      command_ref: "verification-command.forge.6751",
      runner_ref: "forge.verification.runner.docker_bun.v0.1",
      working_directory: ".",
      args: ["bun", "test", "apps/pylon/src/forge-dispatch-protocol.test.ts"],
      timeout_seconds: 900,
    },
    lease_ref: "lease.forge.6751",
    expires_at: "2026-06-28T19:00:00.000Z",
    created_at: at,
    source_refs: ["github:OpenAgentsInc/openagents#6751"],
  })

const sampleCloseout = (): AssignmentCloseout => ({
  schema: "openagents.pylon.assignment_closeout.v0.3",
  assignmentRef: "work.forge.6751",
  leaseRef: "lease.forge.6751",
  status: "accepted",
  paymentMode: "no-spend",
  settlementState: "not_applicable",
  payoutClaimAllowed: false,
  artifactRefs: ["artifact.forge.6751"],
  blockerRefs: [],
  buildRefs: ["build.check-deploy"],
  closeoutRefs: ["closeout.forge.6751"],
  previewRefs: [],
  proofRefs: ["proof.forge.6751"],
  receiptRefs: ["receipt.forge.6751"],
  resultRefs: ["result.forge.6751"],
  summaryRefs: ["summary.forge.6751"],
  testRefs: ["test.forge.6751"],
  redacted: true,
  completedAt: at,
})

describe("forge dispatch protocol adapter", () => {
  test("projects Forge work items into existing Pylon assignment leases", () => {
    const lease = forgeDispatchWorkItemToPylonLease(sampleWorkItem())

    expect(lease).toMatchObject({
      schema: "openagents.pylon.assignment_lease.v0.3",
      assignmentRef: "work.forge.6751",
      leaseRef: "lease.forge.6751",
      goal: "Implement the Pylon-to-Forge dispatch protocol",
      paymentMode: "no-spend",
      capabilityRefs: ["capability.codex_cli"],
      backendRef: FORGE_PYLON_DISPATCH_BACKEND_REF,
      expiresAt: "2026-06-28T19:00:00.000Z",
      createdAt: at,
    })
    expect(lease.codingAssignment).toMatchObject({
      schema: "openagents.forge.pylon_dispatch.coding_assignment.v0.1",
      tenantRef: "tenant.openagents",
      dispatchRef: "dispatch.forge.6751",
      issueRef: "issue.forge.6751",
      git: {
        repositoryRef: "repo.openagents.openagents",
        branchRef: "refs/heads/forge/work/6751",
        gitAccess: {
          tokenRef: "forge_git_token.6751",
          tokenPrefix: "oa_forge_git_visible",
          scopes: ["git:receive-pack"],
          delivery: "out_of_band",
        },
      },
      verificationCommand: {
        runnerRef: "forge.verification.runner.docker_bun.v0.1",
        args: ["bun", "test", "apps/pylon/src/forge-dispatch-protocol.test.ts"],
      },
    })

    const serialized = JSON.stringify(lease)
    expect(serialized).toContain("tokenPrefix")
    expect(serialized).not.toContain("\"token\":")
    expect(serialized).not.toContain("oa_forge_git_secret")
  })

  test("emits Forge decision and closeout messages from Pylon state", () => {
    const item = sampleWorkItem()
    const decision = decodeForgeDispatchDecision(
      forgeDispatchDecisionForPylon({
        item,
        pylonRef: "pylon.local.codex.1",
        state: "accepted",
        observedAt: at,
      }),
    )

    expect(decision.accepted_at).toBe(at)
    expect(decision.rejected_at).toBeNull()

    const closeout = decodeForgeDispatchCloseout(
      pylonCloseoutToForgeDispatchCloseout({
        item,
        pylonRef: "pylon.local.codex.1",
        closeout: sampleCloseout(),
        changeRef: "change.forge.6751",
        packfileRef: "packfile.forge.6751",
        verificationRef: "verification.forge.6751",
        sourceRefs: ["forge:dispatch"],
      }),
    )

    expect(closeout.status).toBe("accepted")
    expect(closeout.change_ref).toBe("change.forge.6751")
    expect(closeout.packfile_ref).toBe("packfile.forge.6751")
    expect(closeout.verification_ref).toBe("verification.forge.6751")
    expect(closeout.source_refs).toEqual([
      "github:OpenAgentsInc/openagents#6751",
      "forge:dispatch",
    ])
    expect(closeout.redacted).toBe(true)
  })

  test("rejects closeouts for the wrong Forge lease", () => {
    expect(() =>
      pylonCloseoutToForgeDispatchCloseout({
        item: sampleWorkItem(),
        pylonRef: "pylon.local.codex.1",
        closeout: {
          ...sampleCloseout(),
          leaseRef: "lease.forge.someone_else",
        },
      }),
    ).toThrow("Forge dispatch closeout lease mismatch")
  })
})
