import { describe, expect, test } from "bun:test"

import {
  LBR_BOND_FORFEIT_FEEDBACK_TYPE,
  LBR_BOND_RELEASE_FEEDBACK_TYPE,
  LBR_FEEDBACK_KIND,
  LBR_PROVIDER_BOND_FEEDBACK_TYPE,
  LbrProtocolError,
  decodeLbrBondForfeitEvent,
  decodeLbrBondOutcomeEvent,
  decodeLbrBondReleaseEvent,
  decodeLbrProviderBondEvent,
  lbrBondForfeitToDraft,
  lbrBondReleaseToDraft,
  lbrProviderBondToDraft,
  makeLbrBondForfeit,
  makeLbrBondOutcome,
  makeLbrBondRelease,
  makeLbrProviderBond,
} from "./index.js"

const requesterPubkey = "11".repeat(32)
const requestId = "aa".repeat(32)
const eventId = "bb".repeat(32)
const sig = "33".repeat(64)

const eventFromDraft = (
  draft: Readonly<{
    kind: number
    tags: ReadonlyArray<readonly string[]>
    content: string
  }>,
  overrides: Partial<{
    id: string
    pubkey: string
    content: string
    tags: ReadonlyArray<readonly string[]>
  }> = {},
) => ({
  id: overrides.id ?? eventId,
  pubkey: overrides.pubkey ?? "22".repeat(32),
  created_at: 1_781_107_200,
  kind: draft.kind,
  tags: overrides.tags ?? draft.tags,
  content: overrides.content ?? draft.content,
  sig,
})

describe("NIP-LBR forfeitable provider bonds", () => {
  test("round-trips a provider bond as ref-only kind-7000 feedback", () => {
    const bond = makeLbrProviderBond({
      requestId,
      requesterPubkey,
      providerRef: "provider.public.pylon.codex_2",
      bondMsats: 250_000,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      forfeitDestination: "counterparty",
      forfeitConditionRef: "condition.public.validator.nonperformance",
      expiresAt: "expiry.public.bond.20300101",
      requestRelay: "wss://relay.openagents.com",
    })
    const draft = lbrProviderBondToDraft(bond)
    const parsed = decodeLbrProviderBondEvent(eventFromDraft(draft))

    expect(draft.kind).toBe(LBR_FEEDBACK_KIND)
    expect(draft.content).toBe("")
    expect(draft.tags).toContainEqual([
      "lbr_feedback_type",
      LBR_PROVIDER_BOND_FEEDBACK_TYPE,
    ])
    expect(parsed.requestId).toBe(requestId)
    expect(parsed.requesterPubkey).toBe(requesterPubkey)
    expect(parsed.providerRef).toBe("provider.public.pylon.codex_2")
    expect(parsed.bondMsats).toBe(250_000)
    expect(parsed.bondReceiptRef).toBe("receipt.public.bond.codex_2_1")
    expect(parsed.forfeitDestination).toBe("counterparty")
    expect(parsed.forfeitConditionRef).toBe(
      "condition.public.validator.nonperformance",
    )
    expect(parsed.expiresAt).toBe("expiry.public.bond.20300101")
  })

  test("round-trips release and forfeit terminal outcomes", () => {
    const release = makeLbrBondRelease({
      requestId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      releaseReceiptRef: "receipt.public.bond_release.codex_2_1",
      authorityRef: "authority.public.validator.1",
    })
    const parsedRelease = decodeLbrBondReleaseEvent(
      eventFromDraft(lbrBondReleaseToDraft(release)),
    )
    expect(parsedRelease.releaseReceiptRef).toBe(
      "receipt.public.bond_release.codex_2_1",
    )
    expect(decodeLbrBondOutcomeEvent(
      eventFromDraft(lbrBondReleaseToDraft(release)),
    )).toMatchObject({
      kind: "released",
      bondReceiptRef: "receipt.public.bond.codex_2_1",
    })

    const forfeit = makeLbrBondForfeit({
      requestId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      forfeitReceiptRef: "receipt.public.bond_forfeit.codex_2_1",
      forfeitDestination: "burn",
      forfeitConditionRef: "condition.public.validator.nonperformance",
      authorityRef: "authority.public.validator.1",
    })
    const parsedForfeit = decodeLbrBondForfeitEvent(
      eventFromDraft(lbrBondForfeitToDraft(forfeit)),
    )
    expect(parsedForfeit.forfeitReceiptRef).toBe(
      "receipt.public.bond_forfeit.codex_2_1",
    )
    expect(parsedForfeit.forfeitDestination).toBe("burn")
    expect(decodeLbrBondOutcomeEvent(
      eventFromDraft(lbrBondForfeitToDraft(forfeit)),
    )).toMatchObject({
      kind: "forfeited",
      forfeitDestination: "burn",
    })
  })

  test("rejects invoices, preimages, private paths, and raw payment material", () => {
    expect(() =>
      makeLbrProviderBond({
        requestId,
        requesterPubkey,
        providerRef: "provider.public.pylon.codex_2",
        bondMsats: 250_000,
        bondReceiptRef: "receipt.public.payment_hash.leak",
        forfeitDestination: "counterparty",
        forfeitConditionRef: "condition.public.validator.nonperformance",
      }),
    ).toThrow(LbrProtocolError)

    const bond = makeLbrProviderBond({
      requestId,
      requesterPubkey,
      providerRef: "provider.public.pylon.codex_2",
      bondMsats: 250_000,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      forfeitDestination: "counterparty",
      forfeitConditionRef: "condition.public.validator.nonperformance",
    })
    const draft = lbrProviderBondToDraft(bond)

    expect(() =>
      decodeLbrProviderBondEvent(
        eventFromDraft(draft, { content: "payment_preimage=deadbeef" }),
      ),
    ).toThrow(LbrProtocolError)

    expect(() =>
      decodeLbrProviderBondEvent(
        eventFromDraft(draft, {
          tags: draft.tags.map((tag) =>
            tag[0] === "amount" ? ["amount", "250000", "lnbc1unsafe"] : tag,
          ),
        }),
      ),
    ).toThrow(LbrProtocolError)

    const release = makeLbrBondRelease({
      requestId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      releaseReceiptRef: "receipt.public.bond_release.codex_2_1",
      authorityRef: "authority.public.validator.1",
    })
    const releaseDraft = lbrBondReleaseToDraft(release)
    expect(() =>
      decodeLbrBondReleaseEvent(
        eventFromDraft(releaseDraft, {
          tags: [...releaseDraft.tags, ["amount", "1"]],
        }),
      ),
    ).toThrow(LbrProtocolError)
  })

  test("enforces exactly one terminal outcome", () => {
    const release = makeLbrBondRelease({
      requestId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      releaseReceiptRef: "receipt.public.bond_release.codex_2_1",
      authorityRef: "authority.public.validator.1",
    })
    const forfeit = makeLbrBondForfeit({
      requestId,
      requesterPubkey,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      forfeitReceiptRef: "receipt.public.bond_forfeit.codex_2_1",
      forfeitDestination: "refund_payer",
      forfeitConditionRef: "condition.public.validator.nonperformance",
      authorityRef: "authority.public.validator.1",
    })

    expect(makeLbrBondOutcome({ release })).toMatchObject({
      kind: "released",
      releaseReceiptRef: "receipt.public.bond_release.codex_2_1",
    })
    expect(makeLbrBondOutcome({ forfeit })).toMatchObject({
      kind: "forfeited",
      forfeitReceiptRef: "receipt.public.bond_forfeit.codex_2_1",
    })
    expect(() => makeLbrBondOutcome({ release, forfeit })).toThrow(
      LbrProtocolError,
    )
    expect(() => makeLbrBondOutcome({})).toThrow(LbrProtocolError)
  })

  test("rejects feedback variants that are not bond outcomes", () => {
    const bond = makeLbrProviderBond({
      requestId,
      requesterPubkey,
      providerRef: "provider.public.pylon.codex_2",
      bondMsats: 250_000,
      bondReceiptRef: "receipt.public.bond.codex_2_1",
      forfeitDestination: "counterparty",
      forfeitConditionRef: "condition.public.validator.nonperformance",
    })
    const draft = lbrProviderBondToDraft(bond)
    expect(draft.tags).toContainEqual([
      "lbr_feedback_type",
      LBR_PROVIDER_BOND_FEEDBACK_TYPE,
    ])
    expect(draft.tags).not.toContainEqual([
      "lbr_feedback_type",
      LBR_BOND_RELEASE_FEEDBACK_TYPE,
    ])
    expect(draft.tags).not.toContainEqual([
      "lbr_feedback_type",
      LBR_BOND_FORFEIT_FEEDBACK_TYPE,
    ])
    expect(() => decodeLbrBondOutcomeEvent(eventFromDraft(draft))).toThrow(
      LbrProtocolError,
    )
  })
})
