import { describe, expect, test } from "vite-plus/test"

import type {
  PortableCapabilityLease,
  SecretMaterial,
} from "@openagentsinc/portable-session-contract"

import {
  HttpPortableCapabilityGrantVault,
  PortableCapabilityRuntimeAdapterError,
  makePortableCapabilityTargetAdapter,
  managedCapabilityMarkerPath,
} from "./portable-capability-runtime-adapters.js"

const lease = (targetRef = "target.managed.port03"): PortableCapabilityLease => ({
  leaseRef: "lease.port03.provider.2",
  ownerRef: "owner.port03",
  sessionRef: "session.port03",
  attachmentRef: "attachment.port03.managed.2",
  attachmentGeneration: 2,
  targetRef,
  capability: "provider",
  accountRef: "account.codex.port03",
  expiresAt: "2026-07-13T14:00:00.000Z",
  state: "issued",
})

type SeenRequest = Readonly<{ path: string; body: Record<string, unknown> }>

const fixture = () => {
  const seen: SeenRequest[] = []
  const fetch = async (input: string | URL | Request, init?: RequestInit) => {
    const url = new URL(input instanceof Request ? input.url : input.toString())
    const body = JSON.parse(String(init?.body)) as Record<string, unknown>
    seen.push({ path: url.pathname, body })
    if (url.pathname.endsWith("/revoke")) {
      return Response.json({
        grant: { grantRef: body.grantRef, status: "revoked" },
        material: "excluded",
      })
    }
    if (url.pathname.endsWith("/reissue")) {
      return Response.json({
        grant: { grantRef: body.destinationGrantRef, status: "issued" },
        material: "excluded",
      })
    }
    if (url.pathname.includes("provider-accounts")) {
      return Response.json({
        authMaterial: { authContentJson: "fixture-provider-material" },
        grant: { grantRef: body.grantRef, status: "resolved" },
      })
    }
    return Response.json({
      grant: {
        credential: { accessToken: "fixture-github-material" },
        grantRef: body.grantRef,
      },
    })
  }
  const vault = new HttpPortableCapabilityGrantVault({
    baseUrl: "https://openagents.example",
    serviceBearer: "service.fixture.port03",
    fetch,
    bindings: [
      {
        grantRef: "codex-auth-grant_source",
        ownerUserId: "owner.port03",
        kind: "provider",
        providerAccountRef: "provider-account.port03",
        runnerSessionId: "session.port03.source",
      },
      {
        grantRef: "github-write-grant_source",
        ownerUserId: "owner.port03",
        kind: "github",
        runnerSessionId: "session.port03.source",
      },
    ],
  })
  return { seen, vault }
}

describe("portable capability runtime adapters", () => {
  test("accepts only exact verified command-scoped grant bindings", async () => {
    const { vault } = fixture()
    expect(() => vault.register({
      grantRef: "codex-auth-grant_source",
      ownerUserId: "owner.other",
      kind: "provider",
    })).toThrow(/conflicts with existing scope/)
    await expect(vault.revokeSourceGrant({
      sourceGrantRef: "grant.unverified",
      leaseRef: "lease.port03.provider.1",
    })).rejects.toMatchObject({ code: "invalid_scope" })
  })

  test("revokes and reissues exact provider and GitHub refs through refs-only authority routes", async () => {
    const { seen, vault } = fixture()
    await vault.revokeSourceGrant({
      sourceGrantRef: "codex-auth-grant_source",
      leaseRef: "lease.port03.provider.1",
    })
    await vault.reissue({
      sourceGrantRef: "codex-auth-grant_source",
      destinationGrantRef: "codex-auth-grant_destination",
      runnerSessionId: "session.port03.destination",
      requestedAction: "portable_session_resume",
    })
    await vault.revokeSourceGrant({
      sourceGrantRef: "github-write-grant_source",
      leaseRef: "lease.port03.github.1",
    })
    await vault.reissue({
      sourceGrantRef: "github-write-grant_source",
      destinationGrantRef: "github-write-grant_destination",
      runnerSessionId: "session.port03.destination",
    })

    expect(seen.map(item => item.path)).toEqual([
      "/api/portable-capability-grants/provider/revoke",
      "/api/portable-capability-grants/provider/reissue",
      "/api/portable-capability-grants/github/revoke",
      "/api/portable-capability-grants/github/reissue",
    ])
    expect(JSON.stringify(seen)).not.toMatch(
      /fixture-(?:provider|github)-material/u,
    )
  })

  test("keeps resolved material inside one callback and zeroizes after success and failure", async () => {
    const { vault } = fixture()
    let successBytes: SecretMaterial | undefined
    const length = await vault.withSourceGrantMaterial({
      sourceGrantRef: "codex-auth-grant_source",
      leaseRef: "lease.port03.provider.1",
      use: async material => {
        successBytes = material
        expect(new TextDecoder().decode(material)).toBe(
          "fixture-provider-material",
        )
        return material.length
      },
    })
    expect(length).toBeGreaterThan(0)
    expect([...successBytes!].every(byte => byte === 0)).toBe(true)

    let failedBytes: SecretMaterial | undefined
    await expect(
      vault.withSourceGrantMaterial({
        sourceGrantRef: "github-write-grant_source",
        leaseRef: "lease.port03.github.1",
        use: async material => {
          failedBytes = material
          throw new Error("installer unavailable")
        },
      }),
    ).rejects.toThrow("installer unavailable")
    expect([...failedBytes!].every(byte => byte === 0)).toBe(true)
  })

  test("requires exact managed install marker and returns refs-only receipts", async () => {
    const installed: Array<Readonly<{
      markerPath?: string | undefined
      materialLength: number
    }>> = []
    const adapter = makePortableCapabilityTargetAdapter({
      adapterRef: "adapter.port03.managed",
      targetClass: "openagents_managed",
      port: {
        install: async input => {
          installed.push({
            markerPath: input.managedMarkerPath,
            materialLength: input.material.length,
          })
          return {
            installationRef: "installation.port03.managed.2",
            evidenceRef: "evidence.port03.capability-installed.2",
            marker: {
              leaseRef: input.lease.leaseRef,
              evidenceRef: "evidence.port03.capability-installed.2",
            },
          }
        },
        wipe: async input => ({
          wipeReceiptRef: `receipt.wipe.${input.leaseRef}`,
        }),
      },
    })
    const material = new TextEncoder().encode(
      "fixture-target-material",
    ) as SecretMaterial
    const redeemed = await adapter.redeem({
      lease: lease(),
      permissions: ["provider.turn.execute"],
      material,
    })
    const wiped = await adapter.wipe({
      leaseRef: lease().leaseRef,
      targetRef: lease().targetRef,
      attachmentRef: lease().attachmentRef,
      attachmentGeneration: lease().attachmentGeneration,
      installationRef: redeemed.installationRef,
    })

    expect(installed).toEqual([
      {
        markerPath: managedCapabilityMarkerPath(
          lease().sessionRef,
          lease().leaseRef,
        ),
        materialLength: material.length,
      },
    ])
    expect(redeemed).toEqual({
      installationRef: "installation.port03.managed.2",
    })
    expect(wiped).toEqual({
      wipeReceiptRef: "receipt.wipe.lease.port03.provider.2",
    })
    expect(JSON.stringify([redeemed, wiped])).not.toContain("fixture-target")
  })

  test("fails closed on a mismatched managed marker and authority outage", async () => {
    const adapter = makePortableCapabilityTargetAdapter({
      adapterRef: "adapter.port03.managed",
      targetClass: "openagents_managed",
      port: {
        install: async () => ({
          installationRef: "installation.port03.managed.2",
          evidenceRef: "evidence.port03.capability-installed.2",
          marker: {
            leaseRef: "lease.port03.wrong",
            evidenceRef: "evidence.port03.capability-installed.2",
          },
        }),
        wipe: async () => ({ wipeReceiptRef: "receipt.port03.wipe" }),
      },
    })
    await expect(
      adapter.redeem({
        lease: lease(),
        permissions: ["provider.turn.execute"],
        material: new Uint8Array([1]) as SecretMaterial,
      }),
    ).rejects.toMatchObject({ code: "target_refused" })

    const vault = new HttpPortableCapabilityGrantVault({
      baseUrl: "https://openagents.example",
      serviceBearer: "service.fixture.port03",
      fetch: async () => {
        throw new Error("offline")
      },
      bindings: [
        {
          grantRef: "codex-auth-grant_source",
          ownerUserId: "owner.port03",
          kind: "provider",
        },
      ],
    })
    await expect(
      vault.revokeSourceGrant({
        sourceGrantRef: "codex-auth-grant_source",
        leaseRef: "lease.port03.provider.1",
      }),
    ).rejects.toBeInstanceOf(PortableCapabilityRuntimeAdapterError)
  })
})
