#!/usr/bin/env node
import { cpSync, mkdirSync, mkdtempSync, realpathSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { resolve } from "node:path"
import { spawnSync } from "node:child_process"

import { buildPublicTarballs } from "./pack-public.ts"

const run = (command: string, args: string[], cwd: string): string => {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" })
  if (result.status !== 0) {
    throw new Error(`${command}_failed:${result.stderr.trim()}\n${result.stdout.trim()}`)
  }
  return result.stdout
}

export const verifyDistribution = (repositoryRoot: string): Readonly<Record<string, unknown>> => {
  const proofRoot = mkdtempSync(resolve(tmpdir(), "openagents-assurance-clean-checkout-"))
  const tarballs = resolve(proofRoot, "tarballs")
  const consumer = resolve(proofRoot, "consumer")
  mkdirSync(consumer, { recursive: true })
  const distribution = buildPublicTarballs(repositoryRoot, tarballs)
  const product = distribution.packages.find((entry) => entry.name === "@openagentsinc/product-spec")!
  const assurance = distribution.packages.find((entry) => entry.name === "@openagentsinc/assurance-spec")!
  const effectSource = realpathSync(resolve(repositoryRoot, "node_modules/effect"))
  const effectPack = JSON.parse(run("npm", ["pack", "--json", "--pack-destination", tarballs], effectSource)) as Array<{ filename: string }>
  const effectFilename = effectPack[0]?.filename
  if (effectFilename === undefined) throw new Error("effect_pack_missing_filename")
  writeFileSync(resolve(consumer, "package.json"), `${JSON.stringify({
    name: "assurance-starter-clean-checkout",
    private: true,
    type: "module",
    dependencies: {
      effect: `file:${resolve(tarballs, effectFilename)}`,
      "@openagentsinc/product-spec": `file:${resolve(tarballs, product.filename)}`,
      "@openagentsinc/assurance-spec": `file:${resolve(tarballs, assurance.filename)}`,
    },
    overrides: {
      effect: `file:${resolve(tarballs, effectFilename)}`,
      "@openagentsinc/product-spec": `file:${resolve(tarballs, product.filename)}`,
    },
  }, null, 2)}\n`)
  cpSync(resolve(repositoryRoot, "packages/assurance-spec/starter-kit"), consumer, { recursive: true })
  // The pinned Node runtime's bundled npm currently carries an `ini` release
  // whose advisory engine range starts at Node 24.15. The package payload is
  // compatible with 24.13; keep this offline consumer proof scoped to the
  // tarballs rather than npm's own transitive engine metadata.
  run("npm", ["install", "--engine-strict=false", "--ignore-scripts", "--offline"], consumer)
  const tsxLoader = resolve(repositoryRoot, "node_modules/tsx/dist/loader.mjs")
  const cli = resolve(consumer, "node_modules/@openagentsinc/assurance-spec/src/cli.ts")
  const ownedRunner = JSON.parse(run("node", [
    "--import", tsxLoader, cli,
    "owned-runner", "assurance/owned-runner.json", "--root", ".", "--json",
  ], consumer)) as { blocking_verdict: string }
  run("node", [
    "--import", tsxLoader, cli,
    "validate", "assurance/example.assurance-spec.md", "--json",
  ], consumer)
  const receipt = {
    distribution_proof_format_version: "0.1",
    clean_checkout: "pass",
    owned_runner_verdict: ownedRunner.blocking_verdict,
    package_tarballs: distribution.packages,
    starter_kit: "one_commit_copy",
    github_hosted_ci: false,
    npm_publication: "owner_authentication_required",
  }
  writeFileSync(resolve(tarballs, "clean-checkout-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`)
  return receipt
}

if (import.meta.main) {
  const root = resolve(import.meta.dirname, "../../..")
  console.log(JSON.stringify(verifyDistribution(root), null, 2))
}
