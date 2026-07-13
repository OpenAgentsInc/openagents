import { describe, expect, test } from "bun:test"

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
    const requests: Array<{ authorization: string | null; body: Record<string, unknown> }> = []
    const fetch = async (_input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>
      requests.push({
        authorization: new Headers(init?.headers).get("authorization"),
        body,
      })
      const action = body.action
      const responses: Record<string, unknown> = {
        stage: {
          resourceRef: "resource.agent-computer.binding",
          checkpointDigest: bundle.checkpoint.digest,
          repositoryPostImageDigest: bundle.checkpoint.repositoryPostImageDigest,
          diffDigest: bundle.checkpoint.diffDigest,
          graphDigest: bundle.checkpoint.graphDigest,
          threadCursors: bundle.threadCursors,
          acceptingWork: false,
          evidenceRefs: ["evidence.port03.binding.stage"],
        },
        activate: {
          activatedAgentRefs: ["agent.port03.binding.root"],
          acceptedWorkRefs: [{ agentRef: "agent.port03.binding.root", turnRef: "turn.port03.binding.1" }],
          evidenceRefs: ["evidence.port03.binding.activate"],
        },
        quiesce: {
          quiescedAgentRefs: ["agent.port03.binding.root"],
          evidenceRefs: ["evidence.port03.binding.quiesce"],
        },
        checkpoint: bundle,
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

    expect(requests.map(item => item.body.action)).toEqual([
      "stage", "activate", "quiesce", "checkpoint", "reclaim",
    ])
    expect(requests.every(item => item.authorization === "Bearer fixture-control-token")).toBeTrue()
    expect(JSON.stringify(requests.map(item => item.body))).not.toContain("fixture-control-token")
  })

  test("rejects non-TLS remote control and private-shaped responses", async () => {
    expect(() => createOaCodexControlPortableProvisioner({
      baseUrl: "http://agent-computer.example.test",
      bearerToken: "fixture-control-token",
    })).toThrow(OaCodexControlPortableProvisionerError)

    const provisioner = createOaCodexControlPortableProvisioner({
      baseUrl: "https://agent-computer.example.test",
      bearerToken: "fixture-control-token",
      fetch: async () => Response.json({ hostname: "private-host" }),
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
})
