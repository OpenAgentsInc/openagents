import { describe, expect, test } from "vite-plus/test"

import {
  admitFullAutoCompletion,
  deriveFullAutoVerificationSpec,
  fullAutoCompletionBlockReason,
  runFullAutoVerification,
  type FullAutoVerificationExec,
} from "./full-auto-verification.ts"

const at = () => new Date("2026-07-22T00:00:00.000Z")

describe("HANDS-2 host verification", () => {
  test("derives a command spec from a fenced verify block", () => {
    const spec = deriveFullAutoVerificationSpec(
      "Merged and green on main.\n```verify\npnpm --dir apps/x test\n```",
    )
    expect(spec).toEqual({ kind: "command", command: "pnpm --dir apps/x test" })
  })

  test("derives a command spec from a verify: line prefix (incl. list bullet)", () => {
    expect(deriveFullAutoVerificationSpec("verify: npm test")).toEqual({ kind: "command", command: "npm test" })
    expect(deriveFullAutoVerificationSpec("- verify: pnpm run check")).toEqual({
      kind: "command",
      command: "pnpm run check",
    })
  })

  test("returns none when no structured marker exists (no keyword guessing)", () => {
    expect(deriveFullAutoVerificationSpec("The tests should pass and it should be merged.")).toEqual({ kind: "none" })
  })

  test("a passing command admits completion", async () => {
    const exec: FullAutoVerificationExec = async () => ({ exitCode: 0, stdout: "ok" })
    const result = await runFullAutoVerification({ spec: { kind: "command", command: "true" }, exec, now: at })
    expect(result.status).toBe("passed")
    expect(admitFullAutoCompletion(result)).toBe(true)
    expect(fullAutoCompletionBlockReason(result)).toBeNull()
  })

  test("a failing command keeps the run active with a typed reason", async () => {
    const exec: FullAutoVerificationExec = async () => ({ exitCode: 2, stderr: "1 failing" })
    const result = await runFullAutoVerification({ spec: { kind: "command", command: "false" }, exec, now: at })
    expect(result.status).toBe("failed")
    expect(result.exitCode).toBe(2)
    expect(admitFullAutoCompletion(result)).toBe(false)
    expect(fullAutoCompletionBlockReason(result)).toBe("host_verification_failed:exit_2")
  })

  test("an absent spec never auto-admits completion", async () => {
    const result = await runFullAutoVerification({ spec: { kind: "none" }, now: at })
    expect(result.status).toBe("absent")
    expect(admitFullAutoCompletion(result)).toBe(false)
    expect(fullAutoCompletionBlockReason(result)).toBe("host_verification_absent")
  })

  test("a command spec with no executor is an error, never a silent pass", async () => {
    const result = await runFullAutoVerification({ spec: { kind: "command", command: "true" }, now: at })
    expect(result.status).toBe("error")
    expect(admitFullAutoCompletion(result)).toBe(false)
    expect(fullAutoCompletionBlockReason(result)).toBe("host_verification_error")
  })

  test("an executor that throws becomes an error verdict, not a crash", async () => {
    const exec: FullAutoVerificationExec = async () => {
      throw new Error("spawn ENOENT")
    }
    const result = await runFullAutoVerification({ spec: { kind: "command", command: "nope" }, exec, now: at })
    expect(result.status).toBe("error")
    expect(result.detail).toContain("spawn ENOENT")
    expect(admitFullAutoCompletion(result)).toBe(false)
  })

  test("evidence_ref: present passes, absent stays unadmitted", async () => {
    const present = await runFullAutoVerification({
      spec: { kind: "evidence_ref", ref: "receipt.abc" },
      evidencePresent: (ref) => ref === "receipt.abc",
      now: at,
    })
    expect(present.status).toBe("passed")
    expect(admitFullAutoCompletion(present)).toBe(true)

    const missing = await runFullAutoVerification({
      spec: { kind: "evidence_ref", ref: "receipt.missing" },
      evidencePresent: () => false,
      now: at,
    })
    expect(missing.status).toBe("absent")
    expect(admitFullAutoCompletion(missing)).toBe(false)
  })

  test("workspaceRef overrides the spec cwd for a command", async () => {
    let seenCwd: string | undefined
    const exec: FullAutoVerificationExec = async ({ cwd }) => {
      seenCwd = cwd
      return { exitCode: 0 }
    }
    await runFullAutoVerification({
      spec: { kind: "command", command: "true", cwd: "/spec/cwd" },
      exec,
      workspaceRef: "/granted/workspace",
      now: at,
    })
    expect(seenCwd).toBe("/granted/workspace")
  })
})
