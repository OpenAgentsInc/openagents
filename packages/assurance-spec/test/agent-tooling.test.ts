import { describe, expect, test } from "vite-plus/test"
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  beginAssuranceSession,
  checkAssuranceSession,
  checkCompletionClaim,
  getAssuranceSpec,
  getCoverageLedgers,
  getEnvironments,
  getEvidenceChecklist,
  getGates,
  getObligation,
  getObligations,
  getRepositoryInventory,
  getSeams,
  getSubjectBinding,
  getTypedGaps,
  listAssuranceSpecs,
  proposeAssuranceSpec,
  runTool,
  serializeAssuranceSpec,
  validateAssuranceSpecFile,
  type ToolOutcome,
} from "../src/index.ts"
import { MVP_SPEC, MVP_SUBJECT, makeFixtureRoot, repoRoot } from "./fixture.ts"

const ok = <A>(outcome: ToolOutcome<A>): A => {
  if (!outcome.ok) throw new Error(`expected success, received ${outcome.code}: ${outcome.message}`)
  return outcome.value
}

const err = <A>(outcome: ToolOutcome<A>): { code: string; message: string } => {
  if (outcome.ok) throw new Error("expected failure, received success")
  return outcome
}

describe("sessions (stateless dual-digest pins)", () => {
  test("begin pins the MVP pair against current on-disk digests with no intent digest", () => {
    const pin = ok(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SPEC })))
    expect(pin.assurance_spec.path).toBe(MVP_SPEC)
    expect(pin.assurance_spec.revision).toBe(1)
    expect(pin.assurance_spec.document_digest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(pin.subject.path).toBe(MVP_SUBJECT)
    expect(pin.subject.document_digest).toBe("sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1")
    expect(pin.subject_binding).toBe("bound")
    expect(pin.criterion_refs).toHaveLength(18)
    expect(pin.session_id).toMatch(/^assurance-session-[a-f0-9]{24}$/)
    expect("intent_digest" in pin.subject).toBe(false)
    expect(pin.notes.join(" ")).toContain("intent_digest is not present")
  })

  test("begin is deterministic (byte-identical pins, no clock, no randomness)", () => {
    const first = ok(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SPEC })))
    const second = ok(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SPEC })))
    expect(JSON.stringify(first)).toBe(JSON.stringify(second))
  })

  test("begin fails typed on missing file, wrong extension, and missing subject", () => {
    expect(err(runTool(beginAssuranceSession({ root: repoRoot, path: "docs/mvp/does-not-exist.assurance-spec.md" }))).code)
      .toBe("file_not_found")
    expect(err(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SUBJECT }))).code)
      .toBe("invalid_assurance_spec_path")
    const root = makeFixtureRoot()
    rmSync(join(root, MVP_SUBJECT))
    expect(err(runTool(beginAssuranceSession({ root, path: MVP_SPEC }))).code).toBe("subject_missing")
  })

  test("begin surfaces a stale subject binding without hiding it", () => {
    const root = makeFixtureRoot()
    appendFileSync(join(root, MVP_SUBJECT), "\n")
    const pin = ok(runTool(beginAssuranceSession({ root, path: MVP_SPEC })))
    expect(pin.subject_binding).toBe("stale")
    expect(pin.subject.document_digest).not.toBe("sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1")
  })

  test("check classifies unchanged with continue_against_pinned", () => {
    const pin = ok(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SPEC })))
    const check = ok(runTool(checkAssuranceSession({ root: repoRoot, path: MVP_SPEC, pin })))
    expect(check.status).toBe("unchanged")
    expect(check.recommended_action).toBe("continue_against_pinned")
    expect(check.assurance_spec.changed).toBe(false)
    expect(check.subject.changed).toBe(false)
  })

  test("check classifies assurance_spec_changed, subject_changed, and both_changed", () => {
    const root = makeFixtureRoot()
    const pin = ok(runTool(beginAssuranceSession({ root, path: MVP_SPEC })))

    appendFileSync(join(root, MVP_SPEC), "\n")
    const specChanged = ok(runTool(checkAssuranceSession({ root, path: MVP_SPEC, pin })))
    expect(specChanged.status).toBe("assurance_spec_changed")
    expect(specChanged.recommended_action).toBe("replan_before_continuing")
    expect(specChanged.assurance_spec.changed).toBe(true)
    expect(specChanged.subject.changed).toBe(false)

    const rootB = makeFixtureRoot()
    const pinB = ok(runTool(beginAssuranceSession({ root: rootB, path: MVP_SPEC })))
    appendFileSync(join(rootB, MVP_SUBJECT), "\n")
    const subjectChanged = ok(runTool(checkAssuranceSession({ root: rootB, path: MVP_SPEC, pin: pinB })))
    expect(subjectChanged.status).toBe("subject_changed")
    expect(subjectChanged.subject.changed).toBe(true)

    appendFileSync(join(rootB, MVP_SPEC), "\n")
    const bothChanged = ok(runTool(checkAssuranceSession({ root: rootB, path: MVP_SPEC, pin: pinB })))
    expect(bothChanged.status).toBe("both_changed")
  })

  test("check classifies invalid_current when the current spec no longer validates or its subject is gone", () => {
    const root = makeFixtureRoot()
    const pin = ok(runTool(beginAssuranceSession({ root, path: MVP_SPEC })))
    writeFileSync(join(root, MVP_SPEC), "not an assurance spec\n")
    const invalid = ok(runTool(checkAssuranceSession({ root, path: MVP_SPEC, pin })))
    expect(invalid.status).toBe("invalid_current")
    expect(invalid.recommended_action).toBe("resolve_invalid_current")
    expect(invalid.errors.length).toBeGreaterThan(0)

    const rootB = makeFixtureRoot()
    const pinB = ok(runTool(beginAssuranceSession({ root: rootB, path: MVP_SPEC })))
    rmSync(join(rootB, MVP_SUBJECT))
    const missingSubject = ok(runTool(checkAssuranceSession({ root: rootB, path: MVP_SPEC, pin: pinB })))
    expect(missingSubject.status).toBe("invalid_current")
    expect(missingSubject.errors.map((error) => error.code)).toContain("subject_missing")
  })

  test("check accepts explicit digests, normalizes bare hex, and requires a full pin", () => {
    const pin = ok(runTool(beginAssuranceSession({ root: repoRoot, path: MVP_SPEC })))
    const bareHex = ok(runTool(checkAssuranceSession({
      root: repoRoot,
      path: MVP_SPEC,
      spec_digest: pin.assurance_spec.document_digest.slice("sha256:".length),
      subject_digest: pin.subject.document_digest.slice("sha256:".length),
    })))
    expect(bareHex.status).toBe("unchanged")

    expect(err(runTool(checkAssuranceSession({ root: repoRoot, path: MVP_SPEC, session_id: "assurance-session-abc" }))).code)
      .toBe("session_pin_required")
    expect(err(runTool(checkAssuranceSession({ root: repoRoot, path: MVP_SPEC, spec_digest: "zz", subject_digest: "zz" }))).code)
      .toBe("invalid_session_pin")
    expect(err(runTool(checkAssuranceSession({ root: repoRoot, path: MVP_SPEC, pin: { nope: true } }))).code)
      .toBe("invalid_session_pin")
  })
})

describe("obligations", () => {
  test("lists the 18 MVP obligations with filters", () => {
    const all = ok(runTool(getObligations({ root: repoRoot, path: MVP_SPEC })))
    expect(all).toHaveLength(18)
    expect(all.every((entry) => entry.design_status === "needs_design")).toBe(true)

    const byCriterion = ok(runTool(getObligations({ root: repoRoot, path: MVP_SPEC, criterion_ref: "CW-AC-04" })))
    expect(byCriterion.map((entry) => entry.id)).toEqual(["AO-CW-AC-04-01"])

    expect(ok(runTool(getObligations({ root: repoRoot, path: MVP_SPEC, status: "ready" })))).toHaveLength(0)
    expect(ok(runTool(getObligations({ root: repoRoot, path: MVP_SPEC, status: "needs_design" })))).toHaveLength(18)
    expect(ok(runTool(getObligations({ root: repoRoot, path: MVP_SPEC, technique: "deterministic_test" })))).toHaveLength(0)
    expect(err(runTool(getObligations({ root: repoRoot, path: MVP_SPEC, status: "green" }))).code).toBe("invalid_argument")
  })

  test("returns AO-CW-AC-04-01 detail with its exact unresolved fields", () => {
    const detail = ok(runTool(getObligation({ root: repoRoot, path: MVP_SPEC, obligation_id: "AO-CW-AC-04-01" })))
    expect(detail.obligation.id).toBe("AO-CW-AC-04-01")
    expect(detail.design_status).toBe("needs_design")
    expect(detail.unresolved_fields).toEqual([
      "domains",
      "technique",
      "environment_refs",
      "oracle",
      "falsifier",
      "evidence",
      "independence",
      "activation_gate",
    ])
    expect(err(runTool(getObligation({ root: repoRoot, path: MVP_SPEC, obligation_id: "AO-NOPE-01" }))).code)
      .toBe("obligation_not_found")
  })
})

describe("ledgers, checklist, claim, gaps", () => {
  test("reports the three ledgers separately: 18/18 traceable, 0 executed, frontier not computed", () => {
    const ledgers = ok(runTool(getCoverageLedgers({ root: repoRoot, path: MVP_SPEC })))
    expect(ledgers.criterion_traceability.total_criteria).toBe(18)
    expect(ledgers.criterion_traceability.traceable_criteria).toBe(18)
    expect(ledgers.execution.total_obligations).toBe(18)
    expect(ledgers.execution.executed_obligations).toBe(0)
    expect(ledgers.execution.receipt_source).toBe("none")
    expect(ledgers.execution.entries.every((entry) => entry.observation === "not_run")).toBe(true)
    expect(ledgers.reachable_frontier.status).toBe("not_computed")
    // Never a blended score: the report has exactly the three ledgers plus a
    // message, and no key carries a percentage or combined score.
    expect(Object.keys(ledgers)).toEqual([
      "criterion_traceability",
      "execution",
      "reachable_frontier",
      "message",
    ])
    expect(JSON.stringify(ledgers)).not.toMatch(/"(?:[a-z_]*(?:percent|score|blended)[a-z_]*)"\s*:/)
  })

  test("evidence checklist reports undesigned evidence as typed gaps, filterable per criterion", () => {
    const full = ok(runTool(getEvidenceChecklist({ root: repoRoot, path: MVP_SPEC })))
    expect(full.criteria).toHaveLength(18)
    const single = ok(runTool(getEvidenceChecklist({ root: repoRoot, path: MVP_SPEC, criterion_ref: "CW-AC-04" })))
    expect(single.criteria).toHaveLength(1)
    const bound = single.criteria[0]!.obligations[0]!
    expect(bound.obligation_id).toBe("AO-CW-AC-04-01")
    expect(bound.evidence_state).toBe("needs_design")
    expect(bound.gaps.map((gap) => gap.code)).toContain("evidence_requirements_undesigned")
    expect(bound.present).toHaveLength(0)
    expect(err(runTool(getEvidenceChecklist({ root: repoRoot, path: MVP_SPEC, criterion_ref: "CW-AC-99" }))).code)
      .toBe("criterion_not_found")
  })

  test("completion-claim audit reports all eight axes and refuses to round anything up", () => {
    const audit = ok(runTool(checkCompletionClaim({
      root: repoRoot,
      path: MVP_SPEC,
      claim: "The Codex Workroom MVP is done.",
    })))
    expect(audit.claim).toBe("The Codex Workroom MVP is done.")
    expect(audit.claim_evaluated).toBe(false)
    expect(audit.admission_state).toBe("proposed")
    expect(audit.subject_binding).toBe("bound")
    expect(audit.obligations).toHaveLength(18)
    for (const entry of audit.obligations) {
      expect(Object.keys(entry.axes)).toEqual([
        "admission",
        "readiness",
        "observation",
        "infrastructure",
        "stability",
        "freshness",
        "disposition",
        "exception",
      ])
      expect(entry.axes.admission).toBe("proposed")
      expect(entry.axes.readiness).toBe("needs_design")
      expect(entry.axes.observation).toBe("not_run")
      expect(entry.axes.infrastructure).toBe("not_computed")
      expect(entry.axes.stability).toBe("unknown")
      expect(entry.axes.freshness).toBe("current")
      expect(entry.axes.disposition).toBe("pending_review")
      expect(entry.axes.exception).toBe("none")
    }
    expect(audit.message).toContain("human/policy decision")
  })

  test("completion-claim freshness goes stale when the subject drifts", () => {
    const root = makeFixtureRoot()
    appendFileSync(join(root, MVP_SUBJECT), "\n")
    const audit = ok(runTool(checkCompletionClaim({ root, path: MVP_SPEC })))
    expect(audit.subject_binding).toBe("stale")
    expect(audit.obligations.every((entry) => entry.axes.freshness === "stale")).toBe(true)
    expect(audit.obligations.every((entry) => entry.axes.observation === "not_run")).toBe(true)
  })

  test("typed-gap report consolidates design gaps with stable codes", () => {
    const report = ok(runTool(getTypedGaps({ root: repoRoot, path: MVP_SPEC })))
    const codes = report.gaps.map((gap) => gap.code)
    expect(report.count).toBe(report.gaps.length)
    expect(codes.filter((code) => code === "obligation_needs_design")).toHaveLength(18)
    expect(codes.filter((code) => code === "missing_oracle")).toHaveLength(18)
    expect(codes.filter((code) => code === "missing_falsifier")).toHaveLength(18)
    expect(codes).toContain("risk_model_needs_design")
    expect(codes).toContain("environment_profiles_need_design")
    expect(codes).toContain("gates_need_design")
    expect(codes).toContain("evidence_policy_needs_design")
    expect(codes).toContain("authority_policy_needs_design")
    const gap = report.gaps.find((candidate) => candidate.obligation_id === "AO-CW-AC-04-01" && candidate.code === "obligation_needs_design")
    expect(gap?.missing_fields).toContain("oracle")
  })

  test("typed-gap report names a stale subject binding", () => {
    const root = makeFixtureRoot()
    appendFileSync(join(root, MVP_SUBJECT), "\n")
    const report = ok(runTool(getTypedGaps({ root, path: MVP_SPEC })))
    expect(report.gaps.map((gap) => gap.code)).toContain("subject_document_digest_mismatch")
  })
})

describe("seams, environments, gates, subject binding, documents", () => {
  test("absent seam coverage is a queryable fact", () => {
    const report = ok(runTool(getSeams({ root: repoRoot, path: MVP_SPEC })))
    expect(report.count).toBe(0)
    expect(report.seams).toHaveLength(0)
    expect(report.message).toContain("queryable fact")
  })

  test("environments report typed gaps, never empty successes", () => {
    const report = ok(runTool(getEnvironments({ root: repoRoot, path: MVP_SPEC })))
    expect(report.profile_support).toBe("not_implemented")
    expect(report.referenced_environments).toHaveLength(0)
    expect(report.gaps.map((gap) => gap.code)).toContain("environment_profiles_need_design")
  })

  test("designed fixtures report per-environment gaps, seams, and gate arms", () => {
    const source = readFileSync(join(repoRoot, MVP_SUBJECT), "utf8")
    const proposal = proposeAssuranceSpec({ productSpecPath: MVP_SUBJECT, productSpecMarkdown: source })
    if (!proposal.ok) throw new Error("fixture proposal failed")
    const designed = {
      ...proposal.document,
      environments: {
        ...proposal.document.environments,
        profiles: [{ id: "ENV-LOCAL", status: "proposed" as const }],
      },
      obligations: proposal.document.obligations.map((obligation, index) => ({
        ...obligation,
        domains: index === 0 ? ["seam"] : ["contract"],
        environment_refs: ["ENV-LOCAL"],
        activation_gate: "GATE-LOCAL",
      })),
      gates: [{ id: "GATE-LOCAL", expression: "all required obligations are ready" }],
    }
    const root = makeFixtureRoot()
    writeFileSync(join(root, MVP_SPEC), serializeAssuranceSpec(designed))

    const environments = ok(runTool(getEnvironments({ root, path: MVP_SPEC })))
    expect(environments.referenced_environments).toHaveLength(1)
    expect(environments.referenced_environments[0]!.id).toBe("ENV-LOCAL")
    expect(environments.referenced_environments[0]!.referenced_by_obligations).toHaveLength(18)
    expect(environments.gaps.map((gap) => gap.code)).toContain("environment_profile_missing")

    const seams = ok(runTool(getSeams({ root, path: MVP_SPEC })))
    expect(seams.count).toBe(1)
    expect(seams.seams[0]!.id).toBe("AO-CW-AC-01-01")

    const gates = ok(runTool(getGates({ root, path: MVP_SPEC })))
    expect(gates.count).toBe(1)
    expect(gates.gates[0]!.arms).toHaveLength(18)
  })

  test("subject binding reports bound with live digests, stale on drift", () => {
    const bound = ok(runTool(getSubjectBinding({ root: repoRoot, path: MVP_SPEC })))
    expect(bound.subject_status).toBe("bound")
    expect(bound.current_digest).toBe(bound.declared_digest)
    expect(bound.current_revision).toBe(6)

    const root = makeFixtureRoot()
    appendFileSync(join(root, MVP_SUBJECT), "\n")
    const stale = ok(runTool(getSubjectBinding({ root, path: MVP_SPEC })))
    expect(stale.subject_status).toBe("stale")
    expect(stale.current_digest).not.toBe(stale.declared_digest)
  })

  test("get/validate return the parsed document and full validation result", () => {
    const document = ok(runTool(getAssuranceSpec({ root: repoRoot, path: MVP_SPEC })))
    expect(document.frontmatter.assurance_spec_id).toBe("assurance.openagents.desktop.codex.workroom.mvp")
    expect(document.obligations).toHaveLength(18)

    const validation = ok(runTool(validateAssuranceSpecFile({ root: repoRoot, path: MVP_SPEC })))
    expect(validation.valid).toBe(true)
    expect(validation.errors).toHaveLength(0)

    const root = makeFixtureRoot()
    writeFileSync(join(root, MVP_SPEC), "garbage\n")
    const invalid = ok(runTool(validateAssuranceSpecFile({ root, path: MVP_SPEC })))
    expect(invalid.valid).toBe(false)
    expect(invalid.errors[0]?.code).toBe("missing_frontmatter")
    expect(err(runTool(getAssuranceSpec({ root, path: MVP_SPEC }))).code).toBe("missing_frontmatter")
  })

  test("list_assurance_specs walks the root deterministically and reports validity", () => {
    const root = makeFixtureRoot()
    writeFileSync(join(root, "broken.assurance-spec.md"), "garbage\n")
    const first = ok(runTool(listAssuranceSpecs({ root })))
    const second = ok(runTool(listAssuranceSpecs({ root })))
    expect(JSON.stringify(first)).toBe(JSON.stringify(second))
    expect(first.map((entry) => entry.path)).toEqual(["broken.assurance-spec.md", MVP_SPEC])
    expect(first[0]!.valid).toBe(false)
    expect(first[0]!.error_count).toBeGreaterThan(0)
    expect(first[1]!.valid).toBe(true)
    expect(first[1]!.assurance_spec_id).toBe("assurance.openagents.desktop.codex.workroom.mvp")
    expect(first[1]!.subject_path).toBe(MVP_SUBJECT)
  })

  test("repository inventory labels candidates as not proof", () => {
    const root = mkdtempSync(join(tmpdir(), "assurance-at1-notgit-"))
    const report = ok(runTool(getRepositoryInventory({ root })))
    expect(report.candidates_not_proof).toBe(true)
    expect(report.state).toBe("not_git")
  })
})

describe("root confinement (security)", () => {
  test("rejects .. traversal and absolute paths outside root", () => {
    const root = makeFixtureRoot()
    expect(err(runTool(getObligations({ root, path: "../outside.assurance-spec.md" }))).code).toBe("invalid_path")
    expect(err(runTool(getObligations({ root, path: "docs/../../outside.assurance-spec.md" }))).code).toBe("invalid_path")
    const outside = mkdtempSync(join(tmpdir(), "assurance-at1-outside-"))
    writeFileSync(join(outside, "escape.assurance-spec.md"), "garbage\n")
    expect(err(runTool(getObligations({ root, path: join(outside, "escape.assurance-spec.md") }))).code)
      .toBe("path_outside_root")
  })

  test("rejects symlinks that resolve outside root", () => {
    const root = makeFixtureRoot()
    const outside = mkdtempSync(join(tmpdir(), "assurance-at1-target-"))
    writeFileSync(join(outside, "target.assurance-spec.md"), "garbage\n")
    symlinkSync(join(outside, "target.assurance-spec.md"), join(root, "evil.assurance-spec.md"))
    expect(err(runTool(getObligations({ root, path: "evil.assurance-spec.md" }))).code).toBe("path_outside_root")
  })

  test("walks skip symlinks, .git, node_modules, and dist", () => {
    const root = makeFixtureRoot()
    for (const directory of [".git", "node_modules", "dist"]) {
      mkdirSync(join(root, directory), { recursive: true })
      writeFileSync(join(root, directory, "hidden.assurance-spec.md"), "garbage\n")
    }
    const outside = mkdtempSync(join(tmpdir(), "assurance-at1-linked-"))
    writeFileSync(join(outside, "linked.assurance-spec.md"), "garbage\n")
    symlinkSync(outside, join(root, "linked-dir"))
    symlinkSync(join(outside, "linked.assurance-spec.md"), join(root, "linked.assurance-spec.md"))
    const listed = ok(runTool(listAssuranceSpecs({ root })))
    expect(listed.map((entry) => entry.path)).toEqual([MVP_SPEC])
  })
})
