import { describe, expect, test } from "bun:test"

import {
  FORGE_PROTOCOL_SCHEMA_VERSION,
  decodeForgeCoordinationIssueRow,
  decodeForgeCoordinationPrRow,
  decodeForgeCoordinationStatusRow,
  decodeForgeDispatchLeaseRow,
  decodeForgeMergeQueueLedgerRow,
  forgeCoordinationStatusStateForNip34Kind,
  forgeCoordinationStatusStates,
  forgeNip34StatusKindForState,
} from "./index.js"

const at = "2026-06-28T16:00:00.000Z"

describe("@openagentsinc/forge-protocol", () => {
  test("exports the phase 0 schema version", () => {
    expect(FORGE_PROTOCOL_SCHEMA_VERSION).toBe("openagents.forge.protocol.v0.1")
  })

  test("round-trips NIP-34 status states and kinds", () => {
    for (const state of forgeCoordinationStatusStates) {
      const kind = forgeNip34StatusKindForState(state)
      expect(forgeCoordinationStatusStateForNip34Kind(kind)).toBe(state)
    }
  })

  test("decodes the D1 coordination source-of-truth row shapes", () => {
    expect(
      decodeForgeCoordinationIssueRow({
        tenant_ref: "tenant.openagents",
        issue_ref: "issue.forge.6746",
        github_issue_number: 6746,
        title: "D1 coordination schema",
        state: "open",
        priority_ref: "prio:0-pr-burndown",
        source_refs_json: JSON.stringify(["github:OpenAgentsInc/openagents#6746"]),
        created_at: at,
        updated_at: at,
      }).issue_ref,
    ).toBe("issue.forge.6746")

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
    ).toBe("ready")

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
    ).toBe(1630)

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
    ).toBe("active")

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
    ).toBe("projected")
  })
})
