import { decodeFleetAccountEntity, type FleetAccountEntity } from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import { selectDispatchAccount } from "./fleet-account-selection.js"

const account = (
  overrides: Partial<{
    accountRefHash: string
    readiness: "ready" | "cooldown" | "unavailable" | "unknown"
    provider: string
    capacityAvailable: number
    capacityBusy: number
    capacityQueued: number
  }>,
): FleetAccountEntity =>
  decodeFleetAccountEntity({
    accountRefHash: "account.pylon.codex.aaaaaaaaaaaaaaaa",
    readiness: "ready",
    updatedAt: "2026-07-04T00:00:00Z",
    ...overrides,
  })

describe("selectDispatchAccount", () => {
  test("returns undefined for an empty list", () => {
    expect(selectDispatchAccount([])).toBeUndefined()
  })

  test("returns the single eligible account", () => {
    const only = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 2,
    })
    expect(selectDispatchAccount([only])).toBe(only)
  })

  test("picks the account with the highest capacityAvailable", () => {
    const low = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 1,
    })
    const high = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 5,
    })
    const mid = account({
      accountRefHash: "account.pylon.codex.3333333333333333",
      capacityAvailable: 3,
    })
    expect(selectDispatchAccount([low, high, mid])).toBe(high)
  })

  test("tie-breaks equal capacityAvailable by lowest busy+queued load", () => {
    const busier = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 2,
      capacityBusy: 3,
      capacityQueued: 1,
    })
    const lessLoaded = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 2,
      capacityBusy: 1,
      capacityQueued: 0,
    })
    expect(selectDispatchAccount([busier, lessLoaded])).toBe(lessLoaded)
  })

  test("missing busy/queued counts as zero load in the tie-break", () => {
    const noLoadReported = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 2,
    })
    const someLoad = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 2,
      capacityBusy: 1,
    })
    expect(selectDispatchAccount([noLoadReported, someLoad])).toBe(noLoadReported)
  })

  test("tie-breaks a full tie by ascending accountRefHash", () => {
    const higherHash = account({
      accountRefHash: "account.pylon.codex.ffffffffffffffff",
      capacityAvailable: 2,
      capacityBusy: 0,
      capacityQueued: 0,
    })
    const lowerHash = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 2,
      capacityBusy: 0,
      capacityQueued: 0,
    })
    expect(selectDispatchAccount([higherHash, lowerHash])).toBe(lowerHash)
  })

  test("returns undefined when every account is at zero capacity", () => {
    const zeroA = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 0,
    })
    const zeroB = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 0,
    })
    expect(selectDispatchAccount([zeroA, zeroB])).toBeUndefined()
  })

  test("returns undefined when capacityAvailable is missing on every account", () => {
    const unknownA = account({ accountRefHash: "account.pylon.codex.1111111111111111" })
    const unknownB = account({ accountRefHash: "account.pylon.codex.2222222222222222" })
    expect(selectDispatchAccount([unknownA, unknownB])).toBeUndefined()
  })

  test("does not treat missing capacityAvailable as infinite or as available", () => {
    const unknownCapacity = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
    })
    const knownSmallCapacity = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 1,
    })
    expect(selectDispatchAccount([unknownCapacity, knownSmallCapacity])).toBe(
      knownSmallCapacity,
    )
  })

  test("excludes a non-ready account even if it reports available capacity", () => {
    const cooldown = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      readiness: "cooldown",
      capacityAvailable: 5,
    })
    const ready = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      readiness: "ready",
      capacityAvailable: 1,
    })
    expect(selectDispatchAccount([cooldown, ready])).toBe(ready)
  })

  test("excludes unavailable and unknown readiness accounts", () => {
    const unavailable = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      readiness: "unavailable",
      capacityAvailable: 9,
    })
    const unknown = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      readiness: "unknown",
      capacityAvailable: 9,
    })
    expect(selectDispatchAccount([unavailable, unknown])).toBeUndefined()
  })

  test("filters to the matching provider when options.provider is set", () => {
    const codexAccount = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 1,
      provider: "codex",
    })
    const claudeAccount = account({
      accountRefHash: "account.pylon.claude.2222222222222222",
      capacityAvailable: 9,
      provider: "claude",
    })
    expect(
      selectDispatchAccount([codexAccount, claudeAccount], { provider: "codex" }),
    ).toBe(codexAccount)
  })

  test("excludes an account with no reported provider when options.provider is set", () => {
    const noProvider = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 9,
    })
    expect(selectDispatchAccount([noProvider], { provider: "codex" })).toBeUndefined()
  })

  test("returns undefined when the only ready accounts have zero/missing capacity", () => {
    const readyZero = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      readiness: "ready",
      capacityAvailable: 0,
    })
    const cooldownWithCapacity = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      readiness: "cooldown",
      capacityAvailable: 5,
    })
    expect(selectDispatchAccount([readyZero, cooldownWithCapacity])).toBeUndefined()
  })

  describe("round-robin within a full tie", () => {
    const tiedA = account({
      accountRefHash: "account.pylon.codex.1111111111111111",
      capacityAvailable: 2,
      capacityBusy: 0,
      capacityQueued: 0,
    })
    const tiedB = account({
      accountRefHash: "account.pylon.codex.2222222222222222",
      capacityAvailable: 2,
      capacityBusy: 0,
      capacityQueued: 0,
    })
    const tiedC = account({
      accountRefHash: "account.pylon.codex.3333333333333333",
      capacityAvailable: 2,
      capacityBusy: 0,
      capacityQueued: 0,
    })

    test("with no lastUsedAccountRefHash, picks the deterministic lowest hash", () => {
      expect(selectDispatchAccount([tiedC, tiedA, tiedB])).toBe(tiedA)
    })

    test("cycles to the next tied account after the last-used one", () => {
      expect(
        selectDispatchAccount([tiedC, tiedA, tiedB], {
          lastUsedAccountRefHash: tiedA.accountRefHash,
        }),
      ).toBe(tiedB)
    })

    test("wraps back to the first tied account after the last one", () => {
      expect(
        selectDispatchAccount([tiedC, tiedA, tiedB], {
          lastUsedAccountRefHash: tiedC.accountRefHash,
        }),
      ).toBe(tiedA)
    })

    test("ignores lastUsedAccountRefHash when it isn't in the tied group", () => {
      expect(
        selectDispatchAccount([tiedC, tiedA, tiedB], {
          lastUsedAccountRefHash: "account.pylon.codex.9999999999999999",
        }),
      ).toBe(tiedA)
    })

    test("ignores lastUsedAccountRefHash when there is no tie (single winner)", () => {
      const clearWinner = account({
        accountRefHash: "account.pylon.codex.9999999999999999",
        capacityAvailable: 10,
      })
      const loser = account({
        accountRefHash: "account.pylon.codex.1111111111111111",
        capacityAvailable: 1,
      })
      expect(
        selectDispatchAccount([clearWinner, loser], {
          lastUsedAccountRefHash: clearWinner.accountRefHash,
        }),
      ).toBe(clearWinner)
    })
  })
})
