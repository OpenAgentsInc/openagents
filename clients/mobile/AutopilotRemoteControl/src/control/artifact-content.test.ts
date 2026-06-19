// G3 (#5495) artifact/diff viewer reads: the mobile client fetches a session's
// retained artifact over the bridge `artifact.read` verb (read_artifact) and
// projects it into the render-ready ArtifactContentView, with a dev-token
// `session.artifact` fallback. These tests mock fetch so they run without a node.

import { afterEach, describe, expect, test } from "bun:test"

import { createBridgeTransport } from "@openagentsinc/autopilot-control-protocol"

import {
  fetchSessionArtifactContent,
  fetchSessionArtifactContentViaBridge,
  type BridgeSession,
} from "./control-client"

const realFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = realFetch
})

const proofArtifact = {
  schema: "openagents.pylon.control_session_artifact.v0.1",
  sessionRef: "sess.1",
  executor: { outcome: "completed", editedFileCount: 1, commandCount: 2 },
  devCheck: {
    state: "passed",
    changeSummary: {
      dirty: { state: "dirty", changedCount: 1 },
      changedFileRefs: [{ fileRef: "src/x.ts", status: "modified", area: "code", extension: "ts" }],
    },
    commandResults: [{ commandRef: "cmd.1", reasonRef: "verify.typecheck", status: "passed", exitCode: 0 }],
  },
}

function bridgeFor(impl: typeof fetch): BridgeSession {
  const transport = createBridgeTransport({
    baseUrl: "https://node.example",
    credential: { pairingRef: "p1", jti: "j1", capabilityRef: "read_artifact" },
    fetchImpl: impl,
  })
  return {
    transport,
    credential: { pairingRef: "p1", jti: "j1", capabilityRef: "read_artifact" },
    baseUrl: "https://node.example",
  }
}

describe("fetchSessionArtifactContentViaBridge", () => {
  test("reads over artifact.read and projects the proof artifact", async () => {
    let body: any = null
    const impl = (async (_url: string, init?: RequestInit) => {
      body = JSON.parse(String(init!.body))
      return new Response(
        JSON.stringify({ ok: true, result: { sessionRef: "sess.1", kind: "proof", artifact: proofArtifact } }),
        { status: 200 },
      )
    }) as unknown as typeof fetch

    const view = await fetchSessionArtifactContentViaBridge(bridgeFor(impl), "sess.1")
    expect(body.verb).toBe("artifact.read")
    expect(body.sessionRef).toBe("sess.1")
    expect(view).not.toBeNull()
    expect(view?.kind).toBe("proof")
    expect(view?.outcome).toBe("completed")
    expect(view?.changedFiles).toEqual([
      { fileRef: "src/x.ts", status: "modified", area: "code", extension: "ts" },
    ])
    expect(view?.commandResults.length).toBe(1)
  })

  test("returns null when the node has no artifact (kind none)", async () => {
    const impl = (async () =>
      new Response(JSON.stringify({ ok: true, result: { sessionRef: "sess.2", kind: "none", artifact: null } }), {
        status: 200,
      })) as unknown as typeof fetch
    const view = await fetchSessionArtifactContentViaBridge(bridgeFor(impl), "sess.2")
    expect(view).toBeNull()
  })
})

describe("fetchSessionArtifactContent (dev-token fallback)", () => {
  test("reads session.artifact and projects the body", async () => {
    let body: any = null
    globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
      body = JSON.parse(String(init!.body))
      return new Response(
        JSON.stringify({ ok: true, result: { kind: "proof", artifact: proofArtifact } }),
        { status: 200 },
      )
    }) as typeof fetch

    const view = await fetchSessionArtifactContent({ baseUrl: "https://node.example", token: "tok" }, "sess.1")
    expect(body.type).toBe("session.artifact")
    expect(body.sessionRef).toBe("sess.1")
    expect(view?.kind).toBe("proof")
    expect(view?.changedFiles.length).toBe(1)
  })

  test("returns null on a none artifact or transport failure", async () => {
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ ok: true, result: { kind: "none", artifact: null } }), {
        status: 200,
      })) as unknown as typeof fetch
    expect(await fetchSessionArtifactContent({ baseUrl: "https://node.example", token: "tok" }, "sess.3")).toBeNull()

    globalThis.fetch = (async () => new Response("nope", { status: 500 })) as unknown as typeof fetch
    expect(await fetchSessionArtifactContent({ baseUrl: "https://node.example", token: "tok" }, "sess.4")).toBeNull()
  })
})
