import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"
import {
  AGENT_COMPUTER_ISOLATION_POLICY_VERSION,
  CLOUD_VM_PROVISIONER_VERSION,
  CODEX_PLACEMENT_ASSIGNMENT_VERSION,
  DEFAULT_AGENT_COMPUTER_ISOLATION_POLICY,
  GCE_CAPACITY_CLASS_VERSION,
  RESOURCE_USAGE_RECEIPT_VERSION,
  AgentComputerIsolationPolicySchema,
  CloudVmSessionHandleSchema,
  CodexPlacementAssignmentSchema,
  GceCapacityClassSchema,
  ResourceUsageReceiptSchema,
} from "./index.ts"

describe("@openagentsinc/cloud-contract", () => {
  test("placement assignment schema accepts public-safe refs", () => {
    const value = S.decodeUnknownSync(CodexPlacementAssignmentSchema)({
      schema: CODEX_PLACEMENT_ASSIGNMENT_VERSION,
      assignmentRef: "assignment.cloud.place.fixture",
      workContextRef: "work_context.user.thread.repo",
      requestedLane: "cloud_gcp",
    })
    expect(value.assignmentRef).toContain("assignment.")
  })

  test("cloud-vm handle and gce capacity schemas are ref-only", () => {
    const session = S.decodeUnknownSync(CloudVmSessionHandleSchema)({
      schema: CLOUD_VM_PROVISIONER_VERSION,
      sessionRef: "cloud_vm.session.fixture",
      limits: { vcpus: 2, memoryMb: 2048, timeoutMs: 60_000 },
    })
    expect(session.sessionRef).toBe("cloud_vm.session.fixture")

    const capacity = S.decodeUnknownSync(GceCapacityClassSchema)({
      schema: GCE_CAPACITY_CLASS_VERSION,
      capacityClassId: "gce.ephemeral.default",
      costBasis: "catalog_plus_10pct",
    })
    expect(capacity.capacityClassId).toContain("gce.")
  })

  test("resource usage receipt requires schema + receiptRef", () => {
    const receipt = S.decodeUnknownSync(ResourceUsageReceiptSchema)({
      schema: RESOURCE_USAGE_RECEIPT_VERSION,
      receiptRef: "receipt.resource.fixture",
      workContextRef: "work_context.fixture",
      cleanup: {
        scratchWipeReceiptRef: "receipt.scratch.wipe",
        microvmDestroyReceiptRef: "receipt.microvm.destroy",
      },
    })
    expect(receipt.cleanup?.scratchWipeReceiptRef).toBeDefined()
  })

  test("default agent computer isolation policy has no wallet authority", () => {
    const policy = S.decodeUnknownSync(AgentComputerIsolationPolicySchema)(
      DEFAULT_AGENT_COMPUTER_ISOLATION_POLICY,
    )
    expect(policy.schema).toBe(AGENT_COMPUTER_ISOLATION_POLICY_VERSION)
    expect(policy.walletAuthority).toBe("none")
    expect(policy.oneWorkContextPerComputer).toBe(true)
  })
})
