import { createHash } from "node:crypto"
import { mkdtemp, readFile, rm, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { SQL } from "bun"
import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"

import type {
  PortableCapabilityLease,
  SecretMaterial,
} from "@openagentsinc/portable-session-contract"

import {
  ManagedPortableCapabilityInstallationPort,
  OwnerLocalPortableCapabilityInstallationPort,
  PortableCapabilityInstallationError,
  createPostgresManagedPortableCapabilityResourceResolver,
} from "./portable-capability-installation-ports.js"
import { managedCapabilityMarkerPath } from "./portable-capability-runtime-adapters.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import {
  hasLocalPostgres,
  startLocalPostgres,
  type LocalPostgres,
} from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const temporaryRoots: string[] = []

afterEach(async () => {
  await Promise.all(
    temporaryRoots.splice(0).map(root => rm(root, { recursive: true, force: true })),
  )
})

const lease = (
  targetRef = "target.port03.local",
): PortableCapabilityLease => ({
  leaseRef: "lease.port03.provider.3",
  ownerRef: "owner.port03",
  sessionRef: "session.port03",
  attachmentRef: "attachment.port03.3",
  attachmentGeneration: 3,
  targetRef,
  capability: "provider",
  accountRef: "account.codex.port03",
  expiresAt: "2026-07-13T20:00:00.000Z",
  state: "issued",
})

const fixtureMaterial = (): SecretMaterial =>
  new TextEncoder().encode("fixture-port03-capability-bytes") as SecretMaterial

describe("owner-local portable capability installation", () => {
  test("uses isolated Pylon custody, exact marker, replay, callback zeroization, and wipe", async () => {
    const pylonHome = await mkdtemp(join(tmpdir(), "port03-local-custody-"))
    temporaryRoots.push(pylonHome)
    const port = new OwnerLocalPortableCapabilityInstallationPort({
      pylonHome,
      ownerRef: "owner.port03",
      targetRef: "target.port03.local",
    })
    const material = fixtureMaterial()
    const installed = await port.install({
      lease: lease(),
      permissions: ["provider.turn.execute"],
      material,
    })
    const replayed = await port.install({
      lease: lease(),
      permissions: ["provider.turn.execute"],
      material,
    })

    expect(replayed).toEqual(installed)
    expect([...material].some(byte => byte !== 0)).toBe(true)
    const directory = port.custodyDirectory(lease().leaseRef)
    const materialPath = join(directory, "material.bin")
    const markerPath = join(directory, "installed.json")
    expect((await stat(directory)).mode & 0o777).toBe(0o700)
    expect((await stat(materialPath)).mode & 0o777).toBe(0o600)
    expect((await stat(markerPath)).mode & 0o777).toBe(0o600)
    expect(JSON.parse(await readFile(markerPath, "utf8"))).toEqual({
      leaseRef: lease().leaseRef,
      evidenceRef: installed.evidenceRef,
    })

    let callbackBytes: SecretMaterial | undefined
    const observedLength = await port.withInstalledMaterial({
      leaseRef: lease().leaseRef,
      installationRef: installed.installationRef,
      use: async bytes => {
        callbackBytes = bytes
        expect(new TextDecoder().decode(bytes)).toBe(
          "fixture-port03-capability-bytes",
        )
        return bytes.length
      },
    })
    expect(observedLength).toBe(material.length)
    expect([...callbackBytes!].every(byte => byte === 0)).toBe(true)

    const receipt = await port.wipe({
      leaseRef: lease().leaseRef,
      targetRef: lease().targetRef,
      attachmentRef: lease().attachmentRef,
      attachmentGeneration: lease().attachmentGeneration,
      installationRef: installed.installationRef,
    })
    expect(receipt.wipeReceiptRef).toMatch(/^receipt\.capability-wiped\./u)
    await expect(stat(directory)).rejects.toMatchObject({ code: "ENOENT" })
    expect(JSON.stringify([installed, receipt])).not.toContain(
      "fixture-port03-capability-bytes",
    )
  })

  test("fails closed on conflicting bytes, binding mismatch, and forged installation", async () => {
    const pylonHome = await mkdtemp(join(tmpdir(), "port03-local-conflict-"))
    temporaryRoots.push(pylonHome)
    const port = new OwnerLocalPortableCapabilityInstallationPort({
      pylonHome,
      ownerRef: "owner.port03",
      targetRef: "target.port03.local",
    })
    const installed = await port.install({
      lease: lease(),
      permissions: ["provider.turn.execute"],
      material: fixtureMaterial(),
    })
    await expect(
      port.install({
        lease: lease(),
        permissions: ["provider.turn.execute"],
        material: new TextEncoder().encode("conflicting-fixture") as SecretMaterial,
      }),
    ).rejects.toMatchObject({ code: "installation_conflict" })
    await expect(
      port.install({
        lease: lease("target.port03.wrong"),
        permissions: ["provider.turn.execute"],
        material: fixtureMaterial(),
      }),
    ).rejects.toMatchObject({ code: "invalid_scope" })
    await expect(
      port.withInstalledMaterial({
        leaseRef: lease().leaseRef,
        installationRef: "installation.capability.forged00000000000000000000000000",
        use: async () => undefined,
      }),
    ).rejects.toMatchObject({ code: "installation_conflict" })
    expect(installed.installationRef).not.toContain("fixture")
  })
})

describe("retained managed portable capability installation", () => {
  test("uses only the raw one-shot channel and refs-only wipe operation", async () => {
    const seen: Array<{
      path: string
      headers: Headers
      transmittedBody?: Uint8Array
      jsonBody?: Record<string, unknown>
    }> = []
    let callbackOwnedBody: Uint8Array | undefined
    const resourceRef = "resource.agent-computer.port03"
    const fetch = async (input: string | URL | Request, init?: RequestInit) => {
      const url = new URL(input instanceof Request ? input.url : input.toString())
      const headers = new Headers(init?.headers)
      if (url.pathname.endsWith("/capabilities/install")) {
        callbackOwnedBody = init?.body as Uint8Array
        const transmittedBody = callbackOwnedBody.slice()
        seen.push({ path: url.pathname, headers, transmittedBody })
        const evidenceRef = headers.get("X-OA-Evidence-Ref")!
        const leaseRef = headers.get("X-OA-Lease-Ref")!
        return Response.json({
          installationRef: `installation.agent-computer.capability.${createHash("sha256")
            .update(`${resourceRef}|${leaseRef}`)
            .digest("hex")
            .slice(0, 16)}`,
          evidenceRef,
          resourceRef,
          marker: { leaseRef, evidenceRef },
          material: "excluded",
        })
      }
      const jsonBody = JSON.parse(String(init?.body)) as Record<string, unknown>
      seen.push({ path: url.pathname, headers, jsonBody })
      return Response.json({
        wipeReceiptRef: "receipt.agent-computer.capability-wiped.port03",
        material: "excluded",
      })
    }
    const port = new ManagedPortableCapabilityInstallationPort({
      baseUrl: "https://agent-computer.example",
      bearerToken: "fixture-service-bearer-value",
      ownerRef: "owner.port03",
      targetRef: "target.port03.managed",
      sessionRef: "session.port03",
      resolveResource: async binding => ({
        ...binding,
        resourceRef,
        state: "staged",
      }),
      fetch,
    })
    const managedLease = lease("target.port03.managed")
    const material = fixtureMaterial()
    const installed = await port.install({
      lease: managedLease,
      permissions: ["provider.turn.execute"],
      material,
      managedMarkerPath: managedCapabilityMarkerPath(
        managedLease.sessionRef,
        managedLease.leaseRef,
      ),
    })
    expect(seen[0]?.path).toBe(
      "/v1/portable-agent-computers/capabilities/install",
    )
    expect(seen[0]?.headers.get("content-type")).toBe(
      "application/octet-stream",
    )
    expect(seen[0]?.headers.get("X-OA-Owner-Ref")).toBe(managedLease.ownerRef)
    expect(seen[0]?.headers.has("X-OA-Resource-Ref")).toBeFalse()
    expect(seen[0]?.headers.get("X-OA-Capability")).toBe("provider")
    expect(new TextDecoder().decode(seen[0]?.transmittedBody)).toBe(
      "fixture-port03-capability-bytes",
    )
    expect([...callbackOwnedBody!].every(byte => byte === 0)).toBe(true)
    expect([...material].some(byte => byte !== 0)).toBe(true)

    const wiped = await port.wipe({
      leaseRef: managedLease.leaseRef,
      targetRef: managedLease.targetRef,
      attachmentRef: managedLease.attachmentRef,
      attachmentGeneration: managedLease.attachmentGeneration,
      installationRef: installed.installationRef,
    })
    expect(seen[1]?.path).toBe("/v1/portable-agent-computers/operations")
    expect(seen[1]?.jsonBody).toMatchObject({
      action: "wipeCapability",
      ownerRef: managedLease.ownerRef,
      targetRef: managedLease.targetRef,
      resourceRef,
      sessionRef: managedLease.sessionRef,
      payload: {
        leaseRef: managedLease.leaseRef,
        installationRef: installed.installationRef,
      },
    })
    expect(JSON.stringify(seen[1]?.jsonBody)).not.toContain(
      "fixture-port03-capability-bytes",
    )
    expect(wiped).toEqual({
      wipeReceiptRef: "receipt.agent-computer.capability-wiped.port03",
    })
  })

  test("zeroizes on transport failure and refuses wrong markers or unsafe responses", async () => {
    const managedLease = lease("target.port03.managed")
    let failedBody: Uint8Array | undefined
    const unavailable = new ManagedPortableCapabilityInstallationPort({
      baseUrl: "https://agent-computer.example",
      bearerToken: "fixture-service-bearer-value",
      ownerRef: managedLease.ownerRef,
      targetRef: managedLease.targetRef,
      sessionRef: managedLease.sessionRef,
      resolveResource: async binding => ({
        ...binding,
        resourceRef: "resource.agent-computer.port03",
        state: "staged",
      }),
      fetch: async (_input, init) => {
        failedBody = init?.body as Uint8Array
        throw new Error("offline")
      },
    })
    await expect(
      unavailable.install({
        lease: managedLease,
        permissions: ["provider.turn.execute"],
        material: fixtureMaterial(),
        managedMarkerPath: managedCapabilityMarkerPath(
          managedLease.sessionRef,
          managedLease.leaseRef,
        ),
      }),
    ).rejects.toMatchObject({ code: "target_unavailable" })
    expect([...failedBody!].every(byte => byte === 0)).toBe(true)

    const refused = new ManagedPortableCapabilityInstallationPort({
      baseUrl: "https://agent-computer.example",
      bearerToken: "fixture-service-bearer-value",
      ownerRef: managedLease.ownerRef,
      targetRef: managedLease.targetRef,
      sessionRef: managedLease.sessionRef,
      resolveResource: async binding => ({
        ...binding,
        resourceRef: "resource.agent-computer.port03",
        state: "staged",
      }),
      fetch: async (_input, init) => {
        const headers = new Headers(init?.headers)
        return Response.json({
          installationRef: "installation.agent-computer.capability.wrong",
          evidenceRef: headers.get("X-OA-Evidence-Ref"),
          resourceRef: "resource.agent-computer.port03",
          marker: {
            leaseRef: "lease.port03.wrong",
            evidenceRef: headers.get("X-OA-Evidence-Ref"),
          },
          material: "excluded",
        })
      },
    })
    await expect(
      refused.install({
        lease: managedLease,
        permissions: ["provider.turn.execute"],
        material: fixtureMaterial(),
        managedMarkerPath: managedCapabilityMarkerPath(
          managedLease.sessionRef,
          managedLease.leaseRef,
        ),
      }),
    ).rejects.toBeInstanceOf(PortableCapabilityInstallationError)

    const wrongResource = new ManagedPortableCapabilityInstallationPort({
      baseUrl: "https://agent-computer.example",
      bearerToken: "fixture-service-bearer-value",
      ownerRef: managedLease.ownerRef,
      targetRef: managedLease.targetRef,
      sessionRef: managedLease.sessionRef,
      resolveResource: async binding => ({
        ...binding,
        attachmentGeneration: binding.attachmentGeneration + 1,
        resourceRef: "resource.agent-computer.port03",
        state: "staged",
      }),
      fetch: async () => {
        throw new Error("must not reach transport")
      },
    })
    await expect(
      wrongResource.install({
        lease: managedLease,
        permissions: ["provider.turn.execute"],
        material: fixtureMaterial(),
        managedMarkerPath: managedCapabilityMarkerPath(
          managedLease.sessionRef,
          managedLease.leaseRef,
        ),
      }),
    ).rejects.toMatchObject({ code: "invalid_scope" })
  })
})

describe.skipIf(!hasLocalPostgres())(
  "Postgres managed portable capability resource resolver",
  () => {
    let pg: LocalPostgres
    let sql: SQL
    const ownerRef = "owner.port03.resolver"
    const targetRef = "target.port03.resolver"
    const sessionRef = "session.port03.resolver"
    const attachmentRef = "attachment.port03.resolver.4"

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE port03_capability_resolver")
      await admin.end()
      const url = pg.urlFor("port03_capability_resolver")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 4 })
      await sql`
        INSERT INTO khala_sync_portable_sessions
          (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
           event_log_ref, current_projection_ref, command_scope_ref,
           root_agent_ref, state, latest_event_cursor,
           current_attachment_ref, current_attachment_generation)
        VALUES
          (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`},
           'work.port03.resolver', 'eventlog.port03.resolver',
           'projection.port03.resolver', 'commands.port03.resolver',
           'agent.port03.resolver.root', 'detached', 4, NULL, 3)
      `
      await sql`
        INSERT INTO khala_sync_portable_managed_targets
          (owner_user_id, session_ref, target_ref, attachment_ref, generation,
           checkpoint_ref, resource_ref, state, accepting_work, bundle_json,
           stage_receipt_json)
        VALUES
          (${ownerRef}, ${sessionRef}, ${targetRef}, ${attachmentRef}, 4,
           'checkpoint.port03.resolver', 'resource.port03.resolver', 'staged',
           FALSE, '{}'::jsonb, '{}'::jsonb)
      `
    })

    afterAll(async () => {
      if (sql !== undefined) await sql.end()
      if (pg !== undefined) await pg.stop()
    })

    test("returns only the exact retained staged row and rejects drift", async () => {
      const resolveResource =
        createPostgresManagedPortableCapabilityResourceResolver({
          sql: sql as unknown as SyncSql,
          ownerRef,
          targetRef,
          sessionRef,
        })
      const exact = {
        ownerRef,
        targetRef,
        sessionRef,
        attachmentRef,
        attachmentGeneration: 4,
      }
      expect(await resolveResource(exact)).toEqual({
        ...exact,
        resourceRef: "resource.port03.resolver",
        state: "staged",
      })
      await expect(
        resolveResource({ ...exact, attachmentGeneration: 5 }),
      ).rejects.toBeInstanceOf(PortableCapabilityInstallationError)

      await sql`
        UPDATE khala_sync_portable_managed_targets
        SET accepting_work = TRUE
        WHERE owner_user_id = ${ownerRef}
          AND session_ref = ${sessionRef}
          AND target_ref = ${targetRef}
      `
      await expect(resolveResource(exact)).rejects.toMatchObject({
        code: "installation_conflict",
      })
    })
  },
)
