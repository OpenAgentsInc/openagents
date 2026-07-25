import { describe, expect, test } from "vite-plus/test"

import {
  FULL_AUTO_COMPLETION_AUTHORITY_REF,
  asHostExecutedVerification,
  buildFullAutoRunEvidence,
  isFullAutoEvidenceRef,
  isFullAutoEvidenceText,
  measureFullAutoWorkspaceBaseline,
  projectFullAutoEvidenceReceiptBlock,
  type BuildFullAutoRunEvidenceInput,
  type FullAutoWorkspaceMeasurement,
} from "./full-auto-evidence.ts"
import {
  FULL_AUTO_VERIFICATION_RESULT_SCHEMA,
  type FullAutoVerificationResult,
} from "./full-auto-verification.ts"

// OMEGA-MOB-31-03 (omega#47) / OMEGA-FA-10 (omega#43): the host-stamped
// evidence chain for one finished Full Auto work unit.
//
// Every test here is a REFUSAL watched happening. The reader on the phone
// already refuses an incomplete, contradicted, private, or self-reported chain;
// these prove the producer never writes one in the first place, so the two
// refusals are not the same refusal counted twice.

const HEAD = "4f2b8c1d9e0a7b6c5d4e3f2a1b0c9d8e7f6a5b4c"
const OTHER_HEAD = "0123456789abcdef0123456789abcdef01234567"

const passed = (
  overrides: Partial<FullAutoVerificationResult> = {},
): FullAutoVerificationResult => ({
  schema: FULL_AUTO_VERIFICATION_RESULT_SCHEMA,
  spec: { kind: "command", command: "cargo test -p full_auto_ui" },
  status: "passed",
  exitCode: 0,
  detail: "test result: ok. 37 passed",
  at: "2026-07-25T18:31:15.000Z",
  ...overrides,
})

const measurement = (
  overrides: Partial<FullAutoWorkspaceMeasurement> = {},
): FullAutoWorkspaceMeasurement => ({
  headRef: HEAD,
  generation: 2,
  diffShortstat: "2 files changed, 3 insertions(+)",
  ...overrides,
})

const input = (
  overrides: Partial<BuildFullAutoRunEvidenceInput> = {},
): BuildFullAutoRunEvidenceInput => ({
  runRef: "run.full-auto.abcd1234.efgh5678",
  objective: "Land one finished Full Auto work unit and prove it end to end.",
  turnRef: "turn.full-auto.1",
  measurement: measurement(),
  baseline: { headRef: OTHER_HEAD, generation: 1, recordedAtMs: 1_785_004_274_000 },
  verification: asHostExecutedVerification(passed())!,
  recordedAtMs: 1_785_004_331_622,
  ...overrides,
})

describe("host-executed narrowing", () => {
  test("only a command the host itself ran, and that passed, can carry a chain", () => {
    expect(asHostExecutedVerification(passed())).not.toBeNull()

    // A provider self-report never reaches the builder, because the host never
    // produces a `passed` verdict from one.
    expect(asHostExecutedVerification(passed({ status: "failed", exitCode: 2 }))).toBeNull()
    expect(asHostExecutedVerification(passed({ status: "absent", exitCode: null }))).toBeNull()
    expect(asHostExecutedVerification(passed({ status: "error", exitCode: null }))).toBeNull()

    // An `evidence_ref` check is host-CONSULTED, not host-EXECUTED. It has no
    // command and no exit code, so it cannot claim `hostExecuted`.
    expect(
      asHostExecutedVerification(
        passed({ spec: { kind: "evidence_ref", ref: "evidence.some.ref" }, exitCode: null }),
      ),
    ).toBeNull()
    expect(asHostExecutedVerification(passed({ spec: { kind: "none" }, exitCode: null }))).toBeNull()

    // A `passed` command verdict with no exit code never happened: exitCode is
    // non-null exactly when a child process ran.
    expect(asHostExecutedVerification(passed({ exitCode: null }))).toBeNull()
  })
})

describe("the built chain", () => {
  test("names the objective by the host's own digest of it", () => {
    const evidence = buildFullAutoRunEvidence(input())!
    expect(evidence.objectiveRef).toMatch(/^objective\.[0-9a-f]{64}$/)
    // Two runs with the same objective name it identically, and a revised
    // objective names itself, not the one the run started with.
    expect(buildFullAutoRunEvidence(input())!.objectiveRef).toBe(evidence.objectiveRef)
    expect(
      buildFullAutoRunEvidence(input({ objective: "a different mission" }))!.objectiveRef,
    ).not.toBe(evidence.objectiveRef)
  })

  test("binds the change to the exact tree and generation the host measured", () => {
    const evidence = buildFullAutoRunEvidence(input())!
    expect(evidence.changeRef).toBe(`change.${HEAD}`)
    expect(evidence.projectGeneration).toBe("generation.project.00002")
    // The diff names the baseline it was measured against, so the counts can
    // never be read as being about some other span.
    expect(evidence.diffSummary).toBe(
      `since ${OTHER_HEAD.slice(0, 7)}: 2 files changed, 3 insertions(+)`,
    )
  })

  test("reports a run that changed nothing as changing nothing", () => {
    const evidence = buildFullAutoRunEvidence(
      input({ measurement: measurement({ diffShortstat: "" }) }),
    )!
    expect(evidence.diffSummary).toBe(`no files changed since ${OTHER_HEAD.slice(0, 7)}`)
  })

  test("binds the verification ref to the verdict, so it cannot be re-pointed", () => {
    const base = buildFullAutoRunEvidence(input())!
    for (const changed of [
      input({ runRef: "run.full-auto.other.run" }),
      input({ verification: asHostExecutedVerification(passed({ exitCode: 1 }))! }),
      input({
        verification: asHostExecutedVerification(
          passed({ spec: { kind: "command", command: "cargo test -p workroom_receipts" } }),
        )!,
      }),
      input({ verification: asHostExecutedVerification(passed({ at: "2026-07-25T19:00:00.000Z" }))! }),
      input({ measurement: measurement({ headRef: OTHER_HEAD }) }),
    ]) {
      expect(buildFullAutoRunEvidence(changed)!.verificationRef).not.toBe(base.verificationRef)
    }
  })

  test("names WHICH authority allowed the completion", () => {
    const evidence = buildFullAutoRunEvidence(input())!
    expect(evidence.authorityRef).toBe(FULL_AUTO_COMPLETION_AUTHORITY_REF)
    expect(evidence.decisionRef).toMatch(/^decision\.full_auto\.completion\.[0-9a-f]{32}$/)
    expect(evidence.authorityReceiptRef).toMatch(
      /^receipt\.authority\.full_auto\.completion\.[0-9a-f]{32}$/,
    )
    // The decision and its receipt name the same decision.
    expect(evidence.authorityReceiptRef.endsWith(evidence.decisionRef.split(".").at(-1)!)).toBe(true)
  })

  test("the report's whole record and the receipt's share cannot disagree", () => {
    const evidence = buildFullAutoRunEvidence(input())!
    const receipt = projectFullAutoEvidenceReceiptBlock(evidence)
    for (const field of ["objectiveRef", "turnRef", "changeRef", "verificationRef"] as const) {
      expect(receipt[field]).toBe(evidence[field])
    }
  })
})

describe("refusals the producer makes so the phone never has to", () => {
  test("a verification command carrying a private path writes no chain at all", () => {
    expect(
      buildFullAutoRunEvidence(
        input({
          verification: asHostExecutedVerification(
            passed({ spec: { kind: "command", command: "cat /Users/owner/.codex/auth.json" } }),
          )!,
        }),
      ),
    ).toBeNull()
    expect(
      buildFullAutoRunEvidence(
        input({
          verification: asHostExecutedVerification(
            passed({ spec: { kind: "command", command: "curl -H 'Authorization: Bearer abc' x" } }),
          )!,
        }),
      ),
    ).toBeNull()
  })

  test("a verification command longer than the phone will show writes no chain", () => {
    expect(
      buildFullAutoRunEvidence(
        input({
          verification: asHostExecutedVerification(
            passed({ spec: { kind: "command", command: "x".repeat(257) } }),
          )!,
        }),
      ),
    ).toBeNull()
  })

  test("a turn ref that is not a bounded opaque reference writes no chain", () => {
    for (const turnRef of [
      "turn ref with spaces",
      "/Users/owner/turns/1",
      "",
      "a".repeat(257),
      "sk-turn-that-looks-like-a-key",
    ]) {
      expect(buildFullAutoRunEvidence(input({ turnRef }))).toBeNull()
    }
  })

  test("a diff summary carrying a private path writes no chain", () => {
    expect(
      buildFullAutoRunEvidence(
        input({
          measurement: measurement({ diffShortstat: "1 file changed in /Users/owner/work" }),
        }),
      ),
    ).toBeNull()
  })

  test("a head the host could not read as a commit writes no chain", () => {
    expect(
      buildFullAutoRunEvidence(input({ measurement: measurement({ headRef: "not a sha" }) })),
    ).toBeNull()
  })
})

describe("the public-safety gates mirror the phone's", () => {
  test("bounded opaque refs pass and credential or path shapes do not", () => {
    expect(isFullAutoEvidenceRef("objective.5123ec99")).toBe(true)
    expect(isFullAutoEvidenceRef("terminal.full_auto.completed.control_api")).toBe(true)
    expect(isFullAutoEvidenceRef("/Users/owner/.codex/auth.json")).toBe(false)
    expect(isFullAutoEvidenceRef("~/work/secrets")).toBe(false)
    expect(isFullAutoEvidenceRef("ghp_abcdefghijklmnopqrstuvwxyz0123456789")).toBe(false)
    expect(isFullAutoEvidenceRef("a ref with spaces")).toBe(false)
    expect(isFullAutoEvidenceRef("a".repeat(65))).toBe(false)
  })

  test("bounded owner-facing text passes and control characters do not", () => {
    expect(isFullAutoEvidenceText("2 files changed, 3 insertions(+)", 512)).toBe(true)
    expect(isFullAutoEvidenceText("line one\nline two", 512)).toBe(false)
    expect(isFullAutoEvidenceText(" leading space", 512)).toBe(false)
    expect(isFullAutoEvidenceText("", 512)).toBe(false)
    expect(isFullAutoEvidenceText("cat /home/owner/.ssh/id_rsa", 512)).toBe(false)
  })
})

describe("the workspace baseline", () => {
  test("is the host's own reading, and is absent when it cannot read one", async () => {
    const at = () => new Date("2026-07-25T18:30:00.000Z")
    expect(
      await measureFullAutoWorkspaceBaseline({
        probe: async () => measurement(),
        workspaceRef: "/tmp/ws",
        now: at,
      }),
    ).toEqual({ headRef: HEAD, generation: 2, recordedAtMs: at().getTime() })

    expect(
      await measureFullAutoWorkspaceBaseline({
        probe: async () => null,
        workspaceRef: "/tmp/not-a-repository",
        now: at,
      }),
    ).toBeNull()
  })
})
