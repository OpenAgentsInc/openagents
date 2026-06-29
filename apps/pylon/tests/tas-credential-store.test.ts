import { describe, expect, test } from "bun:test"

import {
  invalidateStale,
  leaseState,
  revoke,
  type CredentialRef,
  type ProviderCredentialLease,
} from "../src/tas/credential-store"

const nowMs = Date.UTC(2026, 5, 13, 12, 0, 0)

function lease(
  overrides: Partial<ProviderCredentialLease> = {},
): ProviderCredentialLease {
  return {
    providerAccountRef: "provider-account.fixture.codex-primary",
    credentialRef: "ref://secure-store/provider/codex-primary" as CredentialRef,
    issuedAt: nowMs,
    expiresAt: nowMs + 60_000,
    state: "active",
    ...overrides,
  }
}

function assertRefsOnly(candidate: ProviderCredentialLease): void {
  const keys = Object.keys(candidate).sort()

  expect(keys).toEqual([
    "credentialRef",
    "expiresAt",
    "issuedAt",
    "providerAccountRef",
    "state",
  ])
  expect(candidate.credentialRef.startsWith("ref://") || candidate.credentialRef.startsWith("sha256:")).toBe(true)
  expect(JSON.stringify(candidate)).not.toMatch(
    /(access[_-]?token|refresh[_-]?token|api[_-]?key|bearer\s+[a-z0-9._-]+|sk-[a-z0-9_-]+|raw[_-]?secret|secret[_-]?key|password)/i,
  )
}

describe("tas credential store", () => {
  test("moves active leases to expired by clock", () => {
    const activeLease = lease()

    expect(leaseState(activeLease, nowMs + 59_999)).toBe("active")
    expect(leaseState(activeLease, nowMs + 60_000)).toBe("expired")
  })

  test("revokes a lease without changing credential refs", () => {
    const activeLease = lease()
    const revokedLease = revoke(activeLease)

    expect(revokedLease).toEqual({
      ...activeLease,
      state: "revoked",
    })
    expect(leaseState(revokedLease, nowMs)).toBe("revoked")
  })

  test("stale invalidation marks expired leases and preserves non-stale leases", () => {
    const activeLease = lease({
      providerAccountRef: "provider-account.fixture.active",
      expiresAt: nowMs + 1,
    })
    const staleLease = lease({
      providerAccountRef: "provider-account.fixture.stale",
      credentialRef: `sha256:${"a".repeat(64)}`,
      expiresAt: nowMs,
    })
    const revokedLease = lease({
      providerAccountRef: "provider-account.fixture.revoked",
      expiresAt: nowMs - 1,
      state: "revoked",
    })

    expect(invalidateStale([activeLease, staleLease, revokedLease], nowMs)).toEqual([
      activeLease,
      {
        ...staleLease,
        state: "expired",
      },
      revokedLease,
    ])
  })

  test("credential leases are refs-only and do not expose raw-secret-looking fields", () => {
    const leases = [
      lease(),
      lease({
        credentialRef: `sha256:${"b".repeat(64)}`,
      }),
      revoke(lease()),
      ...invalidateStale(
        [
          lease({
            providerAccountRef: "provider-account.fixture.expiring",
            expiresAt: nowMs - 1,
          }),
        ],
        nowMs,
      ),
    ]

    for (const candidate of leases) {
      assertRefsOnly(candidate)
    }
  })
})
