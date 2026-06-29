import { describe, expect, test } from "bun:test"

import { buildArtifactReadRequest } from "./bridge-client.js"
import {
  parseArtifactReadResponse,
  projectArtifactContentView,
} from "./artifact-content-view.js"

const baseRequest = {
  pairingRef: "pairing.fixture.0001",
  capabilityRef: "read_artifact",
  clientRequestId: "client.request.fixture.0001",
  idempotencyKey: "idem.fixture.0001",
}

// A representative projection-safe proof artifact, shaped like the node's
// writeRetainedArtifact output (apps/pylon control-sessions): schema + executor
// stats + a devCheck carrying the change summary + command results.
const proofArtifact = {
  schema: "openagents.pylon.control_session_artifact.v0.1",
  sessionRef: "session.fixture.0001",
  adapter: "codex",
  executor: {
    outcome: "completed",
    eventCount: 12,
    commandCount: 3,
    editedFileCount: 2,
    totalTokens: 1450,
  },
  devCheck: {
    state: "passed",
    changeSummary: {
      dirty: { state: "dirty", changedCount: 2, untrackedCount: 1 },
      changedFileRefs: [
        { fileRef: "src/foo.ts", status: "modified", area: "code", extension: "ts" },
        { fileRef: "src/bar.ts", status: "added", area: "code", extension: "ts" },
      ],
    },
    commandResults: [
      {
        commandRef: "cmd.0001",
        reasonRef: "verify.typecheck",
        status: "passed",
        exitCode: 0,
        durationMs: 4200,
        stdoutBytes: 120,
        stderrBytes: 0,
      },
    ],
  },
  deviations: [],
}

const failureArtifact = {
  schema: "openagents.pylon.control_session_failure.v0.1",
  sessionRef: "session.fixture.0002",
  errorClass: "executor.timeout",
  errorDigestRef: "digest.fixture.err.0001",
}

describe("buildArtifactReadRequest", () => {
  test("carries the artifact.read verb, sessionRef, and bridge refs", () => {
    expect(
      buildArtifactReadRequest({ ...baseRequest, sessionRef: "session.fixture.0001" }),
    ).toEqual({
      verb: "artifact.read",
      ...baseRequest,
      sessionRef: "session.fixture.0001",
    })
  })
})

describe("parseArtifactReadResponse", () => {
  test("parses a proof envelope", () => {
    const parsed = parseArtifactReadResponse({
      sessionRef: "session.fixture.0001",
      kind: "proof",
      artifact: proofArtifact,
    })
    expect(parsed.sessionRef).toBe("session.fixture.0001")
    expect(parsed.kind).toBe("proof")
    expect(parsed.artifact).toBe(proofArtifact)
  })

  test("degrades a garbled body to kind none rather than throwing", () => {
    expect(parseArtifactReadResponse(null)).toEqual({ sessionRef: "", kind: "none", artifact: null })
    expect(parseArtifactReadResponse({ kind: "weird" })).toEqual({
      sessionRef: "",
      kind: "none",
      artifact: null,
    })
  })
})

describe("projectArtifactContentView", () => {
  test("projects a proof artifact into changed files, commands, and a verbatim body", () => {
    const view = projectArtifactContentView({ kind: "proof", artifact: proofArtifact })

    expect(view.present).toBe(true)
    expect(view.kind).toBe("proof")
    expect(view.outcome).toBe("completed")
    expect(view.devCheckState).toBe("passed")
    expect(view.schemaRef).toBe("openagents.pylon.control_session_artifact.v0.1")

    expect(view.changedFiles).toEqual([
      { fileRef: "src/foo.ts", status: "modified", area: "code", extension: "ts" },
      { fileRef: "src/bar.ts", status: "added", area: "code", extension: "ts" },
    ])
    expect(view.dirtySummary).toBe("dirty · 2 changed · 1 untracked")

    expect(view.commandResults).toEqual([
      {
        commandRef: "cmd.0001",
        reasonRef: "verify.typecheck",
        status: "passed",
        exitCode: 0,
        durationMs: 4200,
        stdoutBytes: 120,
        stderrBytes: 0,
      },
    ])

    expect(view.review.editedFileCount).toBe(2)
    expect(view.review.commandCount).toBe(3)
    expect(view.review.totalTokens).toBe(1450)

    // The verbatim text fallback shows the whole projection-safe body.
    expect(view.body).toContain("\"sessionRef\": \"session.fixture.0001\"")
  })

  test("projects a failure artifact with error class + ref", () => {
    const view = projectArtifactContentView({ kind: "failure", artifact: failureArtifact })
    expect(view.kind).toBe("failure")
    expect(view.present).toBe(true)
    expect(view.errorClass).toBe("executor.timeout")
    expect(view.errorDigestRef).toBe("digest.fixture.err.0001")
    expect(view.changedFiles).toEqual([])
    expect(view.commandResults).toEqual([])
  })

  test("renders an empty, non-crashing view for a none/absent artifact", () => {
    const view = projectArtifactContentView({ kind: "none", artifact: null })
    expect(view.present).toBe(false)
    expect(view.kind).toBe("none")
    expect(view.changedFiles).toEqual([])
    expect(view.commandResults).toEqual([])
    expect(view.body).toBe("")
  })

  test("tolerates a top-level change summary (no devCheck nesting)", () => {
    const view = projectArtifactContentView({
      artifact: {
        changeSummary: {
          changedFileRefs: ["docs/readme.md"],
          dirty: { state: "clean", changedCount: 0 },
        },
      },
    })
    expect(view.changedFiles).toEqual([
      { fileRef: "docs/readme.md", status: "unknown", area: null, extension: null },
    ])
  })
})
