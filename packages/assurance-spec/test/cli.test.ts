import { describe, expect, test } from "bun:test"
import { appendFileSync, readFileSync, writeFileSync } from "node:fs"
import { join, resolve } from "node:path"

import { MVP_SPEC, MVP_SUBJECT, makeFixtureRoot, repoRoot } from "./fixture.ts"

const cli = resolve(import.meta.dirname, "../src/cli.ts")

type CliResult = Readonly<{ exitCode: number; stdout: string; stderr: string }>

const run = (args: ReadonlyArray<string>, cwd: string = repoRoot): CliResult => {
  const result = Bun.spawnSync([process.execPath, cli, ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  return {
    exitCode: result.exitCode,
    stdout: result.stdout.toString("utf8"),
    stderr: result.stderr.toString("utf8"),
  }
}

describe("CLI exit-code discipline (0 success / 1 failure / 2 usage / 3 stale session)", () => {
  test("usage errors exit 2", () => {
    expect(run([]).exitCode).toBe(2)
    expect(run(["frobnicate"]).exitCode).toBe(2)
    expect(run(["session"]).exitCode).toBe(2)
    expect(run(["session", "begin"]).exitCode).toBe(2)
    expect(run(["session", "check", MVP_SPEC]).exitCode).toBe(2)
    expect(run(["obligation", MVP_SPEC]).exitCode).toBe(2)
    expect(run(["obligations", MVP_SPEC, "--status", "green"]).exitCode).toBe(2)
  })

  test("operation failures exit 1 with the stable code on stderr", () => {
    const missing = run(["session", "begin", "docs/mvp/nope.assurance-spec.md"])
    expect(missing.exitCode).toBe(1)
    expect(missing.stderr).toContain("file_not_found")
    const notFound = run(["obligation", MVP_SPEC, "AO-NOPE-01"])
    expect(notFound.exitCode).toBe(1)
    expect(notFound.stderr).toContain("obligation_not_found")
  })

  test("session begin exits 0 and returns the full stateless pin as JSON", () => {
    const result = run(["session", "begin", MVP_SPEC, "--json"])
    expect(result.exitCode).toBe(0)
    const pin = JSON.parse(result.stdout)
    expect(pin.assurance_spec.path).toBe(MVP_SPEC)
    expect(pin.subject.path).toBe(MVP_SUBJECT)
    expect(pin.subject_binding).toBe("bound")
    expect(pin.criterion_refs).toHaveLength(18)
    expect(pin.subject.intent_digest).toBeUndefined()
  })

  test("session check exits 0 when unchanged and 3 when stale or invalid", () => {
    const root = makeFixtureRoot()
    const begin = run(["session", "begin", MVP_SPEC, "--root", root, "--json"])
    expect(begin.exitCode).toBe(0)
    const pin = JSON.parse(begin.stdout)
    const pinFile = join(root, "session.json")
    writeFileSync(pinFile, begin.stdout)

    const unchanged = run(["session", "check", MVP_SPEC, "--root", root, "--against", pinFile, "--json"])
    expect(unchanged.exitCode).toBe(0)
    expect(JSON.parse(unchanged.stdout).status).toBe("unchanged")

    appendFileSync(join(root, MVP_SUBJECT), "\n")
    const subjectChanged = run(["session", "check", MVP_SPEC, "--root", root, "--against", pinFile, "--json"])
    expect(subjectChanged.exitCode).toBe(3)
    expect(JSON.parse(subjectChanged.stdout).status).toBe("subject_changed")
    expect(JSON.parse(subjectChanged.stdout).recommended_action).toBe("replan_before_continuing")

    appendFileSync(join(root, MVP_SPEC), "\n")
    const bothChanged = run([
      "session", "check", MVP_SPEC, "--root", root,
      "--spec-digest", pin.assurance_spec.document_digest,
      "--subject-digest", pin.subject.document_digest,
    ])
    expect(bothChanged.exitCode).toBe(3)
    expect(bothChanged.stdout).toContain("both_changed")

    writeFileSync(join(root, MVP_SPEC), "garbage\n")
    const invalid = run(["session", "check", MVP_SPEC, "--root", root, "--against", pinFile, "--json"])
    expect(invalid.exitCode).toBe(3)
    expect(JSON.parse(invalid.stdout).status).toBe("invalid_current")
    expect(JSON.parse(invalid.stdout).recommended_action).toBe("resolve_invalid_current")
  })
})

describe("CLI read commands", () => {
  test("observer fixture planner requires and checks an explicit accepted-subject pin", () => {
    const root = makeFixtureRoot()
    const pinPath = join(root, "accepted-subject.json")
    const outPath = join(root, "observer.assurance-spec.md")
    writeFileSync(pinPath, `${JSON.stringify({
      profile: "openagents_executable_v0.1_exact_document",
      path: MVP_SUBJECT,
      spec_format_version: "0.1",
      spec_revision: 6,
      document_digest: "sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1",
      criterion_refs: Array.from({ length: 18 }, (_, index) => `CW-AC-${String(index + 1).padStart(2, "0")}`),
    }, null, 2)}\n`)
    const result = run([
      "observer", "propose", MVP_SUBJECT,
      "--accepted-subject", pinPath,
      "--planner", "fixture",
      "--out", outPath,
      "--json",
    ], root)
    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: true,
      lifecycle_state: "proposed",
      execution_authorized: false,
      adequacy: { coverage: { criteria: 18, ready: 0, needs_design: 18 } },
    })
    const repeated = run([
      "observer", "propose", MVP_SUBJECT,
      "--accepted-subject", pinPath,
      "--planner", "fixture",
      "--out", outPath,
      "--force",
      "--json",
    ], root)
    expect(repeated.exitCode).toBe(0)

    const pin = JSON.parse(readFileSync(pinPath, "utf8"))
    writeFileSync(pinPath, `${JSON.stringify({ ...pin, spec_revision: 7 }, null, 2)}\n`)
    const stale = run([
      "observer", "propose", MVP_SUBJECT,
      "--accepted-subject", pinPath,
      "--planner", "fixture",
      "--out", outPath,
      "--force",
    ], root)
    expect(stale.exitCode).toBe(1)
    expect(stale.stderr).toContain("semantic_planner_subject_drift")
  })

  test("obligations lists and filters; obligation prints unresolved fields", () => {
    const all = run(["obligations", MVP_SPEC, "--json"])
    expect(all.exitCode).toBe(0)
    expect(JSON.parse(all.stdout)).toHaveLength(18)
    const filtered = run(["obligations", MVP_SPEC, "--criterion", "CW-AC-04", "--json"])
    expect(JSON.parse(filtered.stdout).map((entry: { id: string }) => entry.id)).toEqual(["AO-CW-AC-04-01"])
    const detail = run(["obligation", MVP_SPEC, "AO-CW-AC-04-01"])
    expect(detail.exitCode).toBe(0)
    expect(detail.stdout).toContain("unresolved: domains, technique, environment_refs, oracle, falsifier, evidence, independence, activation_gate")
  })

  test("ledgers reports the three ledgers separately and never a blended score", () => {
    const result = run(["ledgers", MVP_SPEC, "--json"])
    expect(result.exitCode).toBe(0)
    const ledgers = JSON.parse(result.stdout)
    expect(ledgers.criterion_traceability.traceable_criteria).toBe(18)
    expect(ledgers.execution.executed_obligations).toBe(0)
    expect(ledgers.reachable_frontier.status).toBe("not_computed")
    const human = run(["ledgers", MVP_SPEC])
    expect(human.stdout).toContain("traceability 18/18")
    expect(human.stdout).toContain("execution 0/18")
    expect(human.stdout).toContain("frontier not_computed")
  })

  test("checklist and claim answer honestly (all eight axes, nothing rounded up)", () => {
    const checklist = run(["checklist", MVP_SPEC, "--criterion", "CW-AC-04", "--json"])
    expect(checklist.exitCode).toBe(0)
    expect(JSON.parse(checklist.stdout).criteria[0].obligations[0].evidence_state).toBe("needs_design")

    const claim = run(["claim", MVP_SPEC, "--claim", "shipped it", "--json"])
    expect(claim.exitCode).toBe(0)
    const audit = JSON.parse(claim.stdout)
    expect(audit.claim).toBe("shipped it")
    expect(audit.obligations).toHaveLength(18)
    expect(audit.obligations.every((entry: { axes: { observation: string } }) => entry.axes.observation === "not_run")).toBe(true)

    const human = run(["claim", MVP_SPEC])
    expect(human.exitCode).toBe(0)
    expect(human.stdout).toContain("observation=not_run")
    expect(human.stdout).toContain("admission=proposed")
  })

  test("inventory labels candidates as not proof", () => {
    const root = makeFixtureRoot()
    const result = run(["inventory", root, "--json"])
    expect(result.exitCode).toBe(0)
    const report = JSON.parse(result.stdout)
    expect(report.candidates_not_proof).toBe(true)
    expect(report.state).toBe("not_git")
  })

  test("validate keeps working and supports --json", () => {
    const okRun = run(["validate", MVP_SPEC])
    expect(okRun.exitCode).toBe(0)
    expect(okRun.stdout).toContain(`ok ${MVP_SPEC}`)
    const json = run(["validate", MVP_SPEC, "--json"])
    expect(json.exitCode).toBe(0)
    expect(JSON.parse(json.stdout)[0].valid).toBe(true)
  })

  test("machine output is deterministic (byte-identical across runs)", () => {
    for (const command of [
      ["session", "begin", MVP_SPEC, "--json"],
      ["ledgers", MVP_SPEC, "--json"],
      ["claim", MVP_SPEC, "--json"],
      ["obligations", MVP_SPEC, "--json"],
    ]) {
      const first = run(command)
      const second = run(command)
      expect(first.exitCode).toBe(0)
      expect(first.stdout).toBe(second.stdout)
    }
  })
})
