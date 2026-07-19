import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"

import {
  CONFIG_SCHEMA_VERSION,
  EVIDENCE_SCHEMA_VERSION,
  compareEvidence,
  globToRegularExpression,
  validateConfig,
  validateEvidence,
} from "./ui-gap.mjs"

test("the repository configuration is valid", async () => {
  const config = JSON.parse(await readFile(new URL("./openagents-zed.config.json", import.meta.url), "utf8"))
  assert.equal(config.schemaVersion, CONFIG_SCHEMA_VERSION)
  assert.deepEqual(validateConfig(config), [])
})

test("the glob adapter keeps matches inside the selected path", () => {
  const source = globToRegularExpression("apps/openagents-desktop/src/**")
  const images = globToRegularExpression("**/*.png")
  assert.equal(source.test("apps/openagents-desktop/src/renderer/shell.ts"), true)
  assert.equal(source.test("apps/openagents-mobile/src/shell.ts"), false)
  assert.equal(images.test("apps/openagents-desktop/a.png"), true)
  assert.equal(images.test("apps/openagents-desktop/a.jpg"), false)
})

test("comparison output keeps source, accessibility, and visual axes separate", () => {
  const leftSource = {
    targetId: "left",
    source: { probes: [{ id: "editor", axis: "editor", matchCount: 2 }] },
  }
  const rightSource = {
    targetId: "right",
    source: { probes: [{ id: "editor", axis: "editor", matchCount: 5 }] },
  }
  const leftRuntime = {
    runtime: {
      accessibility: { nodes: [{ role: "AXButton" }] },
      visual: { meanLuma: 0.2, edgeDensity: 0.1 },
    },
  }
  const rightRuntime = {
    runtime: {
      accessibility: { nodes: [{ role: "AXButton" }, { role: "AXTextField" }] },
      visual: { meanLuma: 0.3, edgeDensity: 0.15 },
    },
  }
  const result = compareEvidence({
    analysisId: "fixture",
    leftSource,
    rightSource,
    leftRuntime,
    rightRuntime,
  })
  assert.equal(result.schemaVersion, EVIDENCE_SCHEMA_VERSION)
  assert.equal(result.comparison.sourceProbeDeltas[0].delta, 3)
  assert.equal(result.comparison.accessibilityRoleDeltas.AXTextField.delta, 1)
  assert.equal(result.comparison.visualDeltas.meanLuma, 0.1)
  assert.deepEqual(validateEvidence(result), [])
})

test("evidence validation rejects an incomplete record", () => {
  assert.ok(validateEvidence({ schemaVersion: EVIDENCE_SCHEMA_VERSION, kind: "source-scan" }).length > 0)
})

test("evidence validation rejects local private paths", () => {
  const evidence = {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "command-receipt",
    generatedAt: new Date(0).toISOString(),
    command: { stdoutTail: "/Users/operator/private-project" },
  }
  assert.match(validateEvidence(evidence).join("\n"), /local home path/)
})

test("the reviewed assessment has unique findings and consistent scores", async () => {
  const assessment = JSON.parse(
    await readFile(new URL("./evidence/gap-assessment.json", import.meta.url), "utf8"),
  )
  assert.deepEqual(validateEvidence(assessment), [])
  assert.equal(assessment.assessment.findings.length, 20)
  assert.equal(new Set(assessment.assessment.findings.map((finding) => finding.id)).size, 20)
  for (const score of assessment.assessment.scores) {
    assert.equal(
      score.total,
      score.accessibility + score.performance + score.responsive + score.theming + score.antiPatterns,
    )
  }
})
