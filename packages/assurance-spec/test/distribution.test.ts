import { describe, expect, test } from "bun:test"
import { existsSync, mkdtempSync, readFileSync, readdirSync } from "node:fs"
import { tmpdir } from "node:os"
import { resolve } from "node:path"

import {
  parseOwnedRunnerConfig,
  readOwnedRunnerConfig,
  runOwnedRunnerVerification,
} from "../src/index.ts"
import { buildPublicTarballs } from "../scripts/pack-public.ts"
import { verifyDistribution } from "../scripts/verify-distribution.ts"

const repositoryRoot = resolve(import.meta.dir, "../../..")
const starterRoot = resolve(import.meta.dir, "../starter-kit")

describe("AT-6 starter kit and owned-runner contract", () => {
  test("the one-commit kit validates, pins its subject, and reports ledgers without a threshold", () => {
    const config = readOwnedRunnerConfig(starterRoot, "assurance/owned-runner.json")
    const receipt = runOwnedRunnerVerification(starterRoot, config)
    expect(receipt.blocking_verdict).toBe("pass")
    expect(receipt.github_hosted_ci).toBe(false)
    expect(receipt.ledger_policy).toBe("informational_never_threshold")
    expect(receipt.specs).toHaveLength(1)
    expect(receipt.specs[0]).toMatchObject({
      structurally_valid: true,
      subject_binding: "bound",
      traceability: { total_criteria: 1, traceable_criteria: 1 },
      execution: { total_obligations: 1, executed_obligations: 0, receipt_source: "none" },
      errors: [],
    })
    expect(JSON.stringify(receipt)).not.toMatch(/coverage_percentage|ready_percentage|score/i)
  })

  test("the typed config cannot enable GitHub-hosted CI or make ledgers a gate", () => {
    const base = JSON.parse(readFileSync(resolve(starterRoot, "assurance/owned-runner.json"), "utf8"))
    expect(() => parseOwnedRunnerConfig({ ...base, github_hosted_ci: true })).toThrow()
    expect(() => parseOwnedRunnerConfig({ ...base, ledgers_are_informational: false })).toThrow()
    expect(() => parseOwnedRunnerConfig({ ...base, spec_paths: [] })).toThrow()
    expect(existsSync(resolve(starterRoot, ".github/workflows"))).toBe(false)
  })

  test("the OpenAgents monorepo adopts the kit with a current pinned MVP session", () => {
    const config = readOwnedRunnerConfig(repositoryRoot, "assurance/owned-runner.json")
    const receipt = runOwnedRunnerVerification(repositoryRoot, config)
    expect(receipt).toMatchObject({
      blocking_verdict: "pass",
      github_hosted_ci: false,
      ledger_policy: "informational_never_threshold",
      specs: [{
        path: "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
        structurally_valid: true,
        subject_binding: "bound",
        traceability: { total_criteria: 18, traceable_criteria: 18 },
        execution: { total_obligations: 18 },
        errors: [],
      }],
      session_checks: [{
        spec_path: "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
        pin_path: "assurance/openagents-desktop-mvp.session.json",
        status: "unchanged",
        blocking: false,
      }],
    })
  })
})

describe("AT-6 public package readiness", () => {
  test("packs concrete public manifests with no workspace or catalog protocols", async () => {
    const out = mkdtempSync(resolve(tmpdir(), "assurance-pack-test-"))
    const receipt = buildPublicTarballs(repositoryRoot, out)
    const committed = JSON.parse(readFileSync(resolve(
      repositoryRoot,
      "assurance/assurance-spec-public-distribution-receipt.json",
    ), "utf8"))
    expect(receipt.packages.map((entry) => entry.name)).toEqual([
      "@openagentsinc/product-spec",
      "@openagentsinc/assurance-spec",
    ])
    expect(receipt.packages).toEqual(committed.package_tarballs)
    expect(receipt.publish_order).toEqual(committed.publish_order)
    expect(receipt.packages.every((entry) => existsSync(resolve(out, entry.filename)))).toBe(true)
    expect(receipt.npm_publication).toBe("owner_authentication_required")
    for (const file of readdirSync(out).filter((path) => path.endsWith(".tgz"))) {
      const process = Bun.spawnSync(["tar", "-xOf", resolve(out, file), "package/package.json"])
      expect(process.exitCode).toBe(0)
      const manifest = new TextDecoder().decode(process.stdout)
      expect(manifest).not.toContain("workspace:")
      expect(manifest).not.toContain("catalog:")
    }
  })

  test("installs exact tarballs offline in a clean checkout and runs the owned gate", () => {
    const receipt = verifyDistribution(repositoryRoot)
    expect(receipt).toMatchObject({
      clean_checkout: "pass",
      owned_runner_verdict: "pass",
      starter_kit: "one_commit_copy",
      github_hosted_ci: false,
      npm_publication: "owner_authentication_required",
    })
  }, 30_000)
})
