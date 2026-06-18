import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import {
  TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
  TassadarCompiledWeightModuleListingUnsafe,
  projectTassadarCompiledWeightModuleListing,
  tassadarLinkedDenseComposedTraceDigest,
  tassadarLinkedDenseModuleDigest,
  tassadarLinkedDenseProgramFixture,
  verifyTassadarLinkedDenseComposition,
  type TassadarLinkedDenseProgramFixture,
} from "./linked-dense-module.js"

const fixtureFile = JSON.parse(
  readFileSync(
    new URL(
      "../fixtures/tassadar-linked-dense-module-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as TassadarLinkedDenseProgramFixture

const cloneFixture = (): TassadarLinkedDenseProgramFixture =>
  JSON.parse(JSON.stringify(tassadarLinkedDenseProgramFixture))

describe("Tassadar linked dense module", () => {
  test("matches the psionic-generated linked fixture metadata", () => {
    expect(tassadarLinkedDenseProgramFixture).toEqual(fixtureFile)
    expect(tassadarLinkedDenseProgramFixture.linkedModule.moduleKind).toBe(
      TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
    )
    expect(tassadarLinkedDenseModuleDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
    )
    expect(tassadarLinkedDenseComposedTraceDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
    )
    expect(tassadarLinkedDenseProgramFixture.linkedModule.banks).toHaveLength(2)
    expect(
      tassadarLinkedDenseProgramFixture.linkedModule.linkResolution.dependency_graph
        .edges,
    ).toHaveLength(1)
    expect(
      tassadarLinkedDenseProgramFixture.conformanceCases.map(
        (item) => item.programId,
      ),
    ).toEqual([
      "tassadar_corpus.mul_add_v1",
      "tassadar_corpus.memory_roundtrip_v1",
    ])
  })

  test("replay-verifies the composed module against both source banks", async () => {
    const verification = await verifyTassadarLinkedDenseComposition(
      tassadarLinkedDenseProgramFixture,
    )

    expect(verification.replayVerificationCleared).toBe(true)
    expect(verification.blockerRefs).toEqual([])
    expect(verification.composedTraceDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
    )
    expect(verification.conformanceCases).toHaveLength(2)
    for (const verdict of verification.conformanceCases) {
      expect(verdict.verified).toBe(true)
      expect(verdict.projectedRowsMatchSource).toBe(true)
      expect(verdict.projectedTraceDigest).toBe(verdict.sourceTraceDigest)
    }
  })

  test("projects a digest-pinned listing without settlement authority by default", async () => {
    const listing = await projectTassadarCompiledWeightModuleListing({
      fixture: tassadarLinkedDenseProgramFixture,
    })

    expect(listing).toMatchObject({
      composedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
      dependencyEdgeCount: 1,
      linkedModuleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
      listingRef: TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
      purchaseSettlementAllowed: false,
      replayVerificationCleared: true,
      settlementClaimAllowed: false,
      sourceBankCount: 2,
      state: "replay_verified_listed",
    })
    expect(listing.blockerRefs).toEqual([
      "blocker.public.tassadar_compiled_module.purchase_receipt_missing",
      "blocker.public.tassadar_compiled_module.settlement_receipt_missing",
    ])
    expect(listing.replayReceiptRefs).toEqual([
      "receipt.openagents.tassadar_linked_dense_replay.cc1403674fc0d388",
    ])
  })

  test("does not allow settlement when replay conformance is tampered", async () => {
    const tampered = cloneFixture()
    ;(tampered.conformanceCases[0] as { projectedRowsMatchSource: boolean })
      .projectedRowsMatchSource = false
    const listing = await projectTassadarCompiledWeightModuleListing({
      fixture: tampered,
      purchaseReceiptRefs: ["purchase.public.tassadar_module.test"],
      settlementReceiptRefs: ["settlement.public.tassadar_module.test"],
    })

    expect(listing.state).toBe("blocked")
    expect(listing.replayVerificationCleared).toBe(false)
    expect(listing.purchaseSettlementAllowed).toBe(false)
    expect(listing.settlementClaimAllowed).toBe(false)
    expect(listing.blockerRefs).toContain(
      "blocker.public.tassadar_compiled_module.replay_verification_missing",
    )
    expect(listing.blockerRefs).toContain(
      "blocker.public.tassadar_compiled_module.fixture_projection_not_verified",
    )
  })

  test("rejects raw private refs before listing projection", async () => {
    await expect(
      projectTassadarCompiledWeightModuleListing({
        fixture: tassadarLinkedDenseProgramFixture,
        purchaseReceiptRefs: ["customer_email.alice@example.com"],
      }),
    ).rejects.toThrow(TassadarCompiledWeightModuleListingUnsafe)
  })
})
