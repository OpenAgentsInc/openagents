import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { Effect } from "effect"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { makePylonNodeRuntime } from "../src/node/runtime.js"
import { startControlServer } from "../src/node/control-server.js"
import {
  isPylonOwnerLocalCapabilityIngressEnabled,
  makePylonOwnerLocalCapabilityIngress,
} from "../src/portable-owner-local-capability-ingress.js"
import {
  openPylonPortablePhaseContextAdmissionStore,
} from "../src/portable-phase-context-admission.js"
import {
  pylonOwnerLocalCapabilityOperationRef,
} from "../src/portable-owner-local-capability-transport.js"
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js"

const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

const authority = {
  commandExecutionClaimRef: "claim.ide13.capability-ingress",
  ownerRef: "owner.ide13.capability-ingress",
  pylonRef: "pylon.ide13.capability-ingress",
  sessionRef: "session.ide13.capability-ingress",
  attachmentRef: "attachment.ide13.capability-ingress.2",
  attachmentGeneration: 2,
  targetRef: "target.ide13.capability-ingress",
} as const

const leaseRef = "lease.ide13.capability-ingress"
const permissions = ["provider.turn.execute"]
const bearerToken = "private-control-token-capability-ingress"

const admission = {
  schema: "openagents.pylon.portable_phase_context_admission.v1" as const,
  request: {
    schema: "openagents.portable_phase_operation.v1" as const,
    operationRef: "operation.ide13.capability-ingress.stage",
    commandRef: "command.ide13.capability-ingress",
    commandExecutionClaimRef: authority.commandExecutionClaimRef,
    ownerRef: authority.ownerRef,
    sessionRef: authority.sessionRef,
    attachmentRef: authority.attachmentRef,
    attachmentGeneration: authority.attachmentGeneration,
    targetRef: authority.targetRef,
    pylonRef: authority.pylonRef,
    kind: "quiesce" as const,
    checkpointRef: null,
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: [],
    expiresAt: "2096-07-20T13:00:00.000Z",
  },
  payload: {
    kind: "quiesce" as const,
    input: {
      operationRef: "operation.ide13.capability-ingress.stage",
      sessionRef: authority.sessionRef,
      attachmentRef: authority.attachmentRef,
      generation: authority.attachmentGeneration,
      graph: {
        rootAgentRef: "agent.ide13.capability-ingress",
        nodes: [{
          agentRef: "agent.ide13.capability-ingress",
          threadRef: "thread.ide13.capability-ingress",
          transcriptRef: "transcript.ide13.capability-ingress",
          activityCursor: 0,
          lifecycle: "running" as const,
          attachmentGeneration: authority.attachmentGeneration,
        }],
      },
      threadCursors: [],
    },
  },
  recoverySemantics: "operation_ref_idempotent" as const,
}

const request = (url: string, token = bearerToken, ownerRef = authority.ownerRef): Request => {
  const scopedAuthority = { ...authority, ownerRef }
  const operationRef = pylonOwnerLocalCapabilityOperationRef({
    action: "install",
    authority: scopedAuthority,
    leaseRef,
    permissions,
  })
  const material = Buffer.from("private owner-local fixture")
  return new Request(`${url}/v1/portable-owner-local-capabilities/install`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/octet-stream",
      "content-length": String(material.byteLength),
      "idempotency-key": operationRef,
      "x-openagents-operation-ref": operationRef,
      "x-openagents-command-claim-ref": scopedAuthority.commandExecutionClaimRef,
      "x-openagents-owner-ref": scopedAuthority.ownerRef,
      "x-openagents-pylon-ref": scopedAuthority.pylonRef,
      "x-openagents-session-ref": scopedAuthority.sessionRef,
      "x-openagents-attachment-ref": scopedAuthority.attachmentRef,
      "x-openagents-attachment-generation": String(scopedAuthority.attachmentGeneration),
      "x-openagents-target-ref": scopedAuthority.targetRef,
      "x-openagents-lease-ref": leaseRef,
      "x-openagents-capability": "provider",
      "x-openagents-expires-at": "2096-07-20T13:00:00.000Z",
      "x-openagents-permissions": permissions.join(","),
    },
    body: material,
  })
}

const retiredActions = {
  walletSend: async () => undefined,
  walletReceive: async () => undefined,
  walletAdmitPayoutTarget: async () => undefined,
}

describe("Pylon owner-local capability ingress", () => {
  test("is explicitly disabled unless the exact startup flag is set", () => {
    expect(isPylonOwnerLocalCapabilityIngressEnabled({})).toBe(false)
    expect(isPylonOwnerLocalCapabilityIngressEnabled({
      PYLON_OWNER_LOCAL_CAPABILITY_INGRESS: "true",
    })).toBe(false)
    expect(isPylonOwnerLocalCapabilityIngressEnabled({
      PYLON_OWNER_LOCAL_CAPABILITY_INGRESS: "1",
    })).toBe(true)
  })

  test("mounts only on loopback and leaves the route absent by default", async () => {
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const runtime = yield* makePylonNodeRuntime
      const disabled = yield* startControlServer(runtime, {
        token: bearerToken,
        actions: retiredActions,
        hostname: "127.0.0.1",
        port: 0,
      })
      expect((yield* Effect.promise(() => fetch(request(disabled.url)))).status).toBe(404)
      let routed = 0
      const enabled = yield* startControlServer(runtime, {
        token: bearerToken,
        actions: retiredActions,
        hostname: "127.0.0.1",
        port: 0,
        ownerLocalCapabilityHandler: async incoming => {
          routed += 1
          expect(await incoming.bytes()).toEqual(new TextEncoder().encode("private owner-local fixture"))
          return new Response(null, { status: 204 })
        },
      })
      expect((yield* Effect.promise(() => fetch(request(enabled.url)))).status).toBe(204)
      expect(routed).toBe(1)
    })))

    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const runtime = yield* makePylonNodeRuntime
      yield* startControlServer(runtime, {
        token: bearerToken,
        actions: retiredActions,
        hostname: "0.0.0.0",
        port: 0,
        ownerLocalCapabilityHandler: async () => new Response(null, { status: 204 }),
      }).pipe(Effect.flip, Effect.map(error => {
        expect(error.message).toContain("non-loopback")
      }))
    })))
  })

  test("authorizes before isolated custody and replays after process restart", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-capability-ingress-"))
    roots.push(root)
    const pylonHome = join(root, "isolated-pylon-home")
    const defaultCodexHome = join(root, ".codex")
    const ledgerPath = join(root, "portable-ledger.sqlite")
    const authorityPath = join(root, "private", "phase-context.sqlite")
    const firstDatabase = new NodeTestDatabase(ledgerPath, { create: true })
    const firstLedger = new PylonPortableSessionOperationLedger(firstDatabase)
    await Effect.runPromise(firstLedger.registerSession({
      sessionRef: authority.sessionRef,
      attachmentRef: authority.attachmentRef,
      generation: authority.attachmentGeneration,
      acceptingWork: true,
    }))
    await Effect.runPromise(firstLedger.persistControlBinding({
      sessionRef: authority.sessionRef,
      attachmentRef: authority.attachmentRef,
      generation: authority.attachmentGeneration,
      runtimeInstanceRef: "runtime.ide13.capability-ingress.first",
      agents: [{
        agentRef: "agent.ide13.capability-ingress",
        controlSessionRef: "control.ide13.capability-ingress",
        workspaceRef: "workspace.ide13.capability-ingress",
      }],
    }))
    const firstAuthority = await openPylonPortablePhaseContextAdmissionStore({
      databasePath: authorityPath,
      now: () => new Date("2026-07-20T12:00:00.000Z"),
      pylonRef: authority.pylonRef,
      targetRef: authority.targetRef,
    })
    firstAuthority.store.admit(admission)
    firstAuthority.store.acknowledgeTerminal(admission.request.operationRef)
    expect(firstAuthority.store.purge()).toBe(1)
    let bindingCurrent = false
    const firstHandler = makePylonOwnerLocalCapabilityIngress({
      bearerToken,
      pylonHome,
      pylonRef: authority.pylonRef,
      targetRef: authority.targetRef,
      sessionRef: authority.sessionRef,
      ledger: firstLedger,
      authorityStore: firstAuthority.store,
      targetBindingIsCurrent: () => bindingCurrent,
    })

    expect((await firstHandler(request("http://127.0.0.1", "wrong-token-value-000"))).status).toBe(401)
    expect((await firstHandler(request("http://127.0.0.1"))).status).toBe(403)
    bindingCurrent = true
    expect((await firstHandler(request("http://127.0.0.1", bearerToken, "owner.ide13.forged"))).status).toBe(403)
    expect(existsSync(join(pylonHome, "runtime", "portable-capabilities"))).toBe(false)

    const installed = await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const runtime = yield* makePylonNodeRuntime
      const server = yield* startControlServer(runtime, {
        token: bearerToken,
        actions: retiredActions,
        hostname: "127.0.0.1",
        port: 0,
        ownerLocalCapabilityHandler: firstHandler,
      })
      return yield* Effect.promise(() => fetch(request(server.url)))
    })))
    expect(installed.status).toBe(200)
    expect(existsSync(join(pylonHome, "runtime", "portable-capabilities"))).toBe(true)
    expect(existsSync(defaultCodexHome)).toBe(false)
    const leaseDirectory = createHash("sha256").update(leaseRef).digest("hex")
    expect(existsSync(join(
      pylonHome,
      "runtime",
      "portable-capabilities",
      leaseDirectory,
      "material.bin",
    ))).toBe(true)
    firstAuthority.close()
    firstDatabase.close()

    const restartedDatabase = new NodeTestDatabase(ledgerPath)
    const restartedLedger = new PylonPortableSessionOperationLedger(restartedDatabase)
    const restartedAuthority = await openPylonPortablePhaseContextAdmissionStore({
      databasePath: authorityPath,
      now: () => new Date("2026-07-20T12:00:01.000Z"),
      pylonRef: authority.pylonRef,
      targetRef: authority.targetRef,
    })
    const restartedHandler = makePylonOwnerLocalCapabilityIngress({
      bearerToken,
      pylonHome,
      pylonRef: authority.pylonRef,
      targetRef: authority.targetRef,
      sessionRef: authority.sessionRef,
      ledger: restartedLedger,
      authorityStore: restartedAuthority.store,
      targetBindingIsCurrent: () => true,
    })
    expect((await restartedHandler(request("http://127.0.0.1"))).status).toBe(200)
    expect(existsSync(defaultCodexHome)).toBe(false)
    restartedAuthority.close()
    restartedDatabase.close()
  })
})
