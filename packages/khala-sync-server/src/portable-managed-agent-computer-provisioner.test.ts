import { describe, expect, test } from "vite-plus/test"

import {
  createOaCodexControlPortableProvisioner,
  OaCodexControlPortableProvisionerError,
} from "./portable-managed-agent-computer-provisioner.js"
import type { PortableCheckpointBundle } from "./portable-session-move.js"

const sha = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`
const bundle: PortableCheckpointBundle = {
  checkpoint: {
    schema: "openagents.portable_checkpoint.v1",
    checkpointRef: "checkpoint.port03.binding.source",
    sessionRef: "session.port03.binding",
    sourceAttachmentRef: "attachment.port03.binding.source",
    sourceGeneration: 1,
    repositoryRef: "repository.OpenAgentsInc.openagents",
    repositoryRevisionRef: "revision.port03.binding.source",
    repositoryPostImageDigest: sha("a"),
    diffDigest: sha("b"),
    eventLogCursor: 9,
    catalogGenerationRef: "catalog.port03.binding.1",
    graphDigest: sha("c"),
    digest: sha("d"),
    approvalRefs: [],
    artifactRefs: [],
    receiptRefs: [],
    secretMaterial: "excluded",
    processState: "excluded",
  },
  executionBinding: {
    schema: "openagents.portable_session_execution_binding.v1",
    sessionRef: "session.port03.binding",
    ownerRef: "owner.port03.binding",
    runRef: "run.port03.binding",
    repositoryRef: "repository.OpenAgentsInc.openagents",
    pinnedBaseRef: "revision.port03.binding.base",
  },
  graph: {
    rootAgentRef: "agent.port03.binding.root",
    nodes: [{
      agentRef: "agent.port03.binding.root",
      threadRef: "thread.port03.binding.root",
      transcriptRef: "transcript.port03.binding.root",
      activityCursor: 3,
      lifecycle: "waiting",
      attachmentGeneration: 1,
    }],
  },
  threadCursors: [{
    threadRef: "thread.port03.binding.root",
    transcriptRef: "transcript.port03.binding.root",
    activityCursor: 3,
    eventCursor: 9,
  }],
}

describe("oa-codex-control retained Agent Computer provisioner", () => {
  test("maps the exact PORT-03 lifecycle to authenticated refs-only operations", async () => {
    const requests: Array<{ path: string; authorization: string | null; body?: Record<string, unknown> }> = []
    const artifactSource = new TextEncoder().encode("fixture-private-checkpoint-artifact")
    const exportedArtifactSource = new TextEncoder().encode("fixture-managed-checkpoint-artifact")
    let issuedArtifact: Uint8Array | undefined
    let registeredArtifact: Uint8Array | undefined
    const fetch = async (request: string | URL | Request, init?: RequestInit): Promise<Response> => {
      const path = new URL(request instanceof Request ? request.url : request.toString()).pathname
      if (path.endsWith("/checkpoints/materialize")) {
        requests.push({ path, authorization: new Headers(init?.headers).get("authorization") })
        expect(new Headers(init?.headers).get("X-OA-Artifact-Digest")).toBe(
          `sha256:${createHash("sha256").update(artifactSource).digest("hex")}`,
        )
        return Response.json({
          resourceRef: "resource.agent-computer.binding",
          destinationRunnerSessionReservationRef:
            "runner-session-reservation.port03.binding",
          checkpointDigest: bundle.checkpoint.digest,
          repositoryPostImageDigest: bundle.checkpoint.repositoryPostImageDigest,
          diffDigest: bundle.checkpoint.diffDigest,
          graphDigest: bundle.checkpoint.graphDigest,
          threadCursors: bundle.threadCursors,
          acceptingWork: false,
          evidenceRefs: ["evidence.port03.binding.stage"],
        })
      }
      if (path.endsWith("/checkpoints/export")) {
        requests.push({ path, authorization: new Headers(init?.headers).get("authorization") })
        expect(new Headers(init?.headers).get("X-OA-Checkpoint-Ref")).toBe("checkpoint.port03.binding.managed")
        return new Response(exportedArtifactSource, {
          headers: {
            "content-type": "application/octet-stream",
            "X-OA-Artifact-Ref": "artifact.port03.binding.managed",
            "X-OA-Artifact-Digest": `sha256:${createHash("sha256").update(exportedArtifactSource).digest("hex")}`,
          },
        })
      }
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>
      requests.push({
        path,
        authorization: new Headers(init?.headers).get("authorization"),
        body,
      })
      const action = body.action
      const payload = body.payload as Record<string, unknown>
      const responses: Record<string, unknown> = {
        stage: {
          resourceRef: "resource.agent-computer.binding",
          materializationRequired: true,
        },
        activate: {
          schema: "openagents.ide_portable_destination_activation.v1",
          receiptRef: "receipt.port03.binding.activate",
          operationRef: body.operationRef,
          sessionRef: body.sessionRef,
          checkpointRef: payload.checkpointRef,
          destinationTargetRef: body.targetRef,
          destinationAttachmentRef: body.attachmentRef,
          destinationRunnerSessionReservationRef:
            payload.destinationRunnerSessionReservationRef,
          destinationGeneration: body.generation,
          authentication: {
            state: "reauthenticated",
            policyRef: payload.authenticationPolicyRef,
            evidenceRef: payload.authorityEvidenceRef,
            observedAt: payload.helpersObservedAt,
            expiresAt: null,
          },
          helpersObservedAt: payload.helpersObservedAt,
          helpers: ["pty", "lsp", "dap", "watcher", "native"].map(kind => ({
            kind,
            readiness: "unsupported",
            instanceRef: null,
            versionRef: null,
            omissionRef: `omission.port03.binding.${kind}`,
            evidenceRefs: [],
          })),
          activatedAgentRefs: ["agent.port03.binding.root"],
          acceptedWorkRefs: [{ agentRef: "agent.port03.binding.root", turnRef: "turn.port03.binding.1" }],
          evidenceRefs: ["evidence.port03.binding.activate"],
        },
        quiesce: {
          quiescedAgentRefs: ["agent.port03.binding.root"],
          evidenceRefs: ["evidence.port03.binding.quiesce"],
        },
        checkpoint: {
          ...bundle,
          checkpoint: {
            ...bundle.checkpoint,
            checkpointRef: "checkpoint.port03.binding.managed",
            sourceAttachmentRef: "attachment.port03.binding.managed",
            sourceGeneration: 2,
          },
        },
        reclaim: {
          cleanedAgentRefs: ["agent.port03.binding.root"],
          processes: "released",
          scratch: "released",
          ports: "released",
          evidenceRefs: ["evidence.port03.binding.reclaim"],
        },
      }
      return Response.json(responses[String(action)], { status: 200 })
    }
    const provisioner = createOaCodexControlPortableProvisioner({
      baseUrl: "http://127.0.0.1:8787",
      bearerToken: "fixture-control-token",
      fetch,
      checkpointArtifacts: {
        resolve: async input => {
          expect(input.bundle).toEqual(bundle)
          issuedArtifact = artifactSource.slice()
          return {
            artifactRef: "artifact.port03.binding",
            digest: `sha256:${createHash("sha256").update(issuedArtifact).digest("hex")}`,
            bytes: issuedArtifact,
          }
        },
        registerArtifact: async input => {
          expect(input.bundle.checkpoint.checkpointRef).toBe("checkpoint.port03.binding.managed")
          registeredArtifact = input.artifact.bytes
        },
      },
    })
    const staged = await provisioner.stage({
      operationRef: "operation.port03.binding.stage",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      bundle,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      capabilityLeaseRefs: ["lease.port03.binding.provider"],
    })
    expect(staged.resourceRef).toBe("resource.agent-computer.binding")
    await provisioner.activate({
      operationRef: "operation.port03.binding.activate",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      resourceRef: staged.resourceRef,
      checkpointRef: bundle.checkpoint.checkpointRef,
      sessionRef: bundle.checkpoint.sessionRef,
      executionBinding: bundle.executionBinding,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      destinationRunnerSessionReservationRef:
        staged.destinationRunnerSessionReservationRef,
      capabilityLeaseRefs: ["lease.port03.binding.provider"],
      authorityEvidenceRef: "evidence.port03.binding.authority",
    })
    await provisioner.quiesce({
      operationRef: "operation.port03.binding.quiesce",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      resourceRef: staged.resourceRef,
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      graph: bundle.graph,
      threadCursors: bundle.threadCursors,
    })
    await provisioner.checkpoint({
      operationRef: "operation.port03.binding.checkpoint",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      resourceRef: staged.resourceRef,
      checkpointRef: "checkpoint.port03.binding.managed",
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      eventLogCursor: 10,
      executionBinding: bundle.executionBinding,
      graph: bundle.graph,
      threadCursors: bundle.threadCursors,
    })
    await provisioner.reclaim({
      operationRef: "operation.port03.binding.reclaim",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      resourceRef: staged.resourceRef,
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      agentRefs: ["agent.port03.binding.root"],
    })

    expect(requests.map(item => item.body?.action ?? (item.path.endsWith("/export") ? "export" : "materialize"))).toEqual([
      "stage", "materialize", "activate", "quiesce", "checkpoint", "export", "reclaim",
    ])
    expect(requests.every(item => item.authorization === "Bearer fixture-control-token")).toBe(true)
    expect(issuedArtifact?.every(byte => byte === 0)).toBe(true)
    expect(registeredArtifact?.every(byte => byte === 0)).toBe(true)
    expect(JSON.stringify(requests.map(item => item.body))).not.toContain("fixture-control-token")
  })

  test("rejects non-TLS remote control and private-shaped responses", async () => {
    expect(() => createOaCodexControlPortableProvisioner({
      baseUrl: "http://agent-computer.example.test",
      bearerToken: "fixture-control-token",
      checkpointArtifacts: { resolve: async () => { throw new Error("unused") }, registerArtifact: async () => undefined },
    })).toThrow(OaCodexControlPortableProvisionerError)

    const provisioner = createOaCodexControlPortableProvisioner({
      baseUrl: "https://agent-computer.example.test",
      bearerToken: "fixture-control-token",
      fetch: async () => Response.json({ hostname: "private-host" }),
      checkpointArtifacts: { resolve: async () => { throw new Error("unused") }, registerArtifact: async () => undefined },
    })
    await expect(provisioner.stage({
      operationRef: "operation.port03.binding.unsafe",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      bundle,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      capabilityLeaseRefs: [],
    })).rejects.toMatchObject({ code: "unsafe_response" })
  })

  test("aborts the exact prepared resource scope when artifact resolution fails", async () => {
    const actions: string[] = []
    const provisioner = createOaCodexControlPortableProvisioner({
      baseUrl: "http://127.0.0.1:8787",
      bearerToken: "fixture-control-token",
      fetch: async (_request, init) => {
        const body = JSON.parse(String(init?.body)) as Record<string, unknown>
        actions.push(String(body.action))
        return Response.json(body.action === "stage"
          ? { resourceRef: "resource.agent-computer.prepared", materializationRequired: true }
          : { evidenceRefs: ["evidence.agent-computer.prepared-aborted"] })
      },
      checkpointArtifacts: { resolve: async () => { throw new Error("source unavailable") }, registerArtifact: async () => undefined },
    })
    await expect(provisioner.stage({
      operationRef: "operation.port03.binding.prepare-failure",
      ownerRef: "owner.port03.binding",
      targetRef: "target.port03.binding.managed",
      bundle,
      attachmentRef: "attachment.port03.binding.managed",
      generation: 2,
      capabilityLeaseRefs: [],
    })).rejects.toThrow("source unavailable")
    expect(actions).toEqual(["stage", "abortPrepared"])
  })
})
import { createHash } from "node:crypto"
