import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

import { decodeAssuranceReceipt, sha256Digest } from "@openagentsinc/assurance-spec"

import { bridgeAssuranceReceipt } from "./assurance-receipt-bridge.ts"
import { makeProductSpecWorkroom } from "./product-spec-workroom.ts"

const repositoryRoot = resolve(import.meta.dir, "../../..")
const productSpecPath = join(repositoryRoot, "docs/mvp/openagents-codex-workroom-mvp.product-spec.md")
const assuranceSpecPath = join(repositoryRoot, "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md")
const manifestPath = join(repositoryRoot, "assurance/openagents-desktop-mvp.assurance-manifest.json")
const admissionPath = join(repositoryRoot, "assurance/openagents-desktop-mvp.assurance-admission.json")
const receiptPath = join(
  repositoryRoot,
  "assurance/receipts/openagents-desktop-mvp/AO-CW-AC-04-01.candidate.assurance-receipt.json",
)

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

describe("installed MVP Assurance receipt journey", () => {
  test("bridges exact accepted evidence and records the owner's read-only journey acceptance", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-mvp-assurance-bridge-"))
    roots.push(root)
    const workspaceRoot = join(root, "workspace")
    const stateRoot = join(root, "state")
    const resolverRoot = join(root, "resolver")
    mkdirSync(join(workspaceRoot, "specs"), { recursive: true })

    const productSpecBytes = readFileSync(productSpecPath, "utf8")
    const relativeSpecPath = "specs/openagents-codex-workroom-mvp.product-spec.md"
    writeFileSync(join(workspaceRoot, relativeSpecPath), productSpecBytes)

    const workroom = makeProductSpecWorkroom({
      workspaceRoot,
      stateRoot,
      now: () => "2026-07-13T19:00:00.000Z",
    })
    const opened = workroom.open({ workContextRef: "work.context.mvp-assurance", relativePath: relativeSpecPath })
    expect(opened.ok).toBe(true)
    if (!opened.ok || opened.value.state !== "ready") return

    const proposed = workroom.proposePlan({
      workContextRef: "work.context.mvp-assurance",
      spec: opened.value.identity,
      packets: [
        {
          packetRef: "packet.productspec-journey",
          title: "ProductSpec-native Codex workroom journey",
          criterionIds: ["CW-AC-04"],
          dependencyRefs: [],
          allocation: "root",
        },
        {
          packetRef: "packet.read-only-boundary",
          title: "Read-only review boundary",
          criterionIds: ["CW-AC-08"],
          dependencyRefs: ["packet.productspec-journey"],
          allocation: "child",
        },
      ],
      deferredCriterionIds: [
        "CW-AC-01", "CW-AC-02", "CW-AC-03", "CW-AC-05", "CW-AC-06", "CW-AC-07", "CW-AC-09",
        "CW-AC-10", "CW-AC-11", "CW-AC-12", "CW-AC-13", "CW-AC-14", "CW-AC-15", "CW-AC-16",
        "CW-AC-17", "CW-AC-18",
      ],
    })
    expect(proposed.ok).toBe(true)
    if (!proposed.ok) return
    const acceptedPlan = workroom.acceptPlan({ planRef: proposed.value.planRef, expectedSpec: opened.value.identity })
    expect(acceptedPlan.ok).toBe(true)
    if (!acceptedPlan.ok) return
    const admitted = workroom.admitPacket({
      runRef: acceptedPlan.value.runRef,
      packetRef: "packet.productspec-journey",
      leaseRef: "lease.mvp-assurance.1",
      executorRef: "runner.openagents.local.20260713",
      executionMode: "owner-present",
      expectedSpec: opened.value.identity,
    })
    expect(admitted.ok).toBe(true)
    if (!admitted.ok) return

    const receiptBytes = readFileSync(receiptPath, "utf8")
    const receipt = decodeAssuranceReceipt(JSON.parse(receiptBytes))
    expect(receipt.product_spec_digest).toBe(sha256Digest(productSpecBytes))
    expect(receipt.assurance_spec_digest).toBe(sha256Digest(readFileSync(assuranceSpecPath, "utf8")))
    expect(receipt.manifest_digest).toBe(sha256Digest(readFileSync(manifestPath, "utf8")))
    expect(receipt.admission_digest).toBe(sha256Digest(readFileSync(admissionPath, "utf8")))

    const bridged = bridgeAssuranceReceipt({
      workroom,
      resolverRoot,
      receiptBytes,
      expectedReceiptDigest: sha256Digest(receiptBytes),
      expectedManifestDigest: receipt.manifest_digest,
      expectedAssuranceSpecDigest: receipt.assurance_spec_digest,
      expectedAdmissionDigest: receipt.admission_digest,
      expectedObligationId: receipt.obligation_id,
      expectedSpec: opened.value.identity,
      expectedCriterionIds: ["CW-AC-04"],
      authorizedReviewerRefs: [receipt.reviewer_ref],
      runRef: acceptedPlan.value.runRef,
      packetRef: "packet.productspec-journey",
      leaseRef: "lease.mvp-assurance.1",
      verifierRef: receipt.reviewer_ref,
    })
    expect(bridged.ok).toBe(true)
    if (!bridged.ok) return
    expect(readFileSync(bridged.resolverPath, "utf8")).toBe(receiptBytes)
    expect(bridged.run.plan.packets[0]).toMatchObject({
      state: "verified",
      evidenceRefs: [bridged.handle],
      ownerDisposition: null,
    })

    const ownerAccepted = workroom.setOwnerDisposition({
      runRef: acceptedPlan.value.runRef,
      packetRef: "packet.productspec-journey",
      disposition: "accepted",
      ownerRef: "owner.christopherdavid",
      reason: "Owner accepted the installed ProductSpec-native Codex workroom journey and its read-only review boundary.",
      expectedSpec: opened.value.identity,
    })
    expect(ownerAccepted.ok).toBe(true)
    if (!ownerAccepted.ok) return
    expect(ownerAccepted.value.plan.packets[0]?.ownerDisposition).toEqual({
      disposition: "accepted",
      ownerRef: "owner.christopherdavid",
      reason: "Owner accepted the installed ProductSpec-native Codex workroom journey and its read-only review boundary.",
      decidedAt: "2026-07-13T19:00:00.000Z",
    })
  })
})
