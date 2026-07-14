#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"
// product-spec CLI: validate `.product-spec.md` files, scaffold new ones,
// and compute document/intent digests.
//
//   bun packages/product-spec/src/cli.ts validate <file...> [--profile openagents|upstream]
//   bun packages/product-spec/src/cli.ts validate --specs-root <dir> [--profile ...]
//   bun packages/product-spec/src/cli.ts digest <file...>
//   bun packages/product-spec/src/cli.ts init <file> [--title "..."] [--type prd|hypothesis]
import { readFileSync, readdirSync, statSync } from "node:fs"
import { basename, join } from "node:path"

import {
  PRODUCT_SPEC_EXTENSION,
  PRODUCT_SPEC_PROFILES,
  computeProductSpecDocumentDigest,
  computeProductSpecIntentDigest,
  starterProductSpec,
  validateDecisionTrace,
  validateProductSpec,
} from "./index.ts"
import type { ArtifactType, ProductSpecProfile } from "./index.ts"

const collectSpecFiles = (root: string): string[] => {
  const results: string[] = []
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry)
      if (statSync(path).isDirectory()) walk(path)
      else if (path.endsWith(PRODUCT_SPEC_EXTENSION)) results.push(path)
    }
  }
  walk(root)
  return results.sort()
}

const validateFiles = async (
  paths: string[],
  profile: ProductSpecProfile,
): Promise<number> => {
  let failures = 0
  for (const path of paths) {
    const markdown = await Runtime.file(path).text()
    const result = validateProductSpec(markdown, { profile })
    if (result.valid) {
      const warningNote = result.warnings.length
        ? ` (${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"})`
        : ""
      console.log(`ok ${path}${warningNote}`)
      for (const warning of result.warnings) {
        console.log(`  warn ${warning.code}: ${warning.message}`)
      }
    } else {
      failures += 1
      console.error(`FAIL ${path}`)
      for (const error of result.errors) {
        console.error(`  error ${error.code}: ${error.message}`)
      }
    }
  }
  return failures
}

const main = async () => {
  const [command, ...rest] = process.argv.slice(2)

  if (command === "validate-trace") {
    if (rest.length === 0) {
      console.error("usage: product-spec validate-trace <file...>")
      process.exit(2)
    }
    let failures = 0
    for (const path of rest) {
      let input: string
      try {
        input = readFileSync(path, "utf8")
      } catch (error) {
        failures += 1
        console.error(`FAIL ${path}`)
        console.error(`  error unreadable_decision_trace: ${error instanceof Error ? error.message : String(error)}`)
        continue
      }
      const result = validateDecisionTrace(input)
      if (result.valid) {
        console.log(`ok ${path}`)
      } else {
        failures += 1
        console.error(`FAIL ${path}`)
        for (const issue of result.errors) {
          console.error(`  error ${issue.code}: ${issue.message}`)
        }
      }
    }
    if (failures > 0) {
      console.error(`${failures} invalid Decision Trace file(s).`)
      process.exit(1)
    }
    return
  }

  if (command === "validate") {
    const args = [...rest]
    let profile: ProductSpecProfile = "openagents"
    const profileIndex = args.indexOf("--profile")
    if (profileIndex !== -1) {
      const requested = args[profileIndex + 1] ?? ""
      if (!(PRODUCT_SPEC_PROFILES as ReadonlyArray<string>).includes(requested)) {
        console.error(`unknown --profile: ${requested} (use ${PRODUCT_SPEC_PROFILES.join("|")})`)
        process.exit(2)
      }
      profile = requested as ProductSpecProfile
      args.splice(profileIndex, 2)
    }
    const specsRootIndex = args.indexOf("--specs-root")
    const paths =
      specsRootIndex === -1
        ? args
        : collectSpecFiles(args[specsRootIndex + 1] ?? "specs")
    if (paths.length === 0) {
      console.error("usage: product-spec validate <file...> | --specs-root <dir> [--profile openagents|upstream]")
      process.exit(2)
    }
    const failures = await validateFiles(paths, profile)
    if (failures > 0) {
      console.error(`${failures} invalid Product Spec file(s).`)
      process.exit(1)
    }
    return
  }

  if (command === "digest") {
    if (rest.length === 0) {
      console.error("usage: product-spec digest <file...>")
      process.exit(2)
    }
    let failures = 0
    for (const path of rest) {
      const markdown = await Runtime.file(path).text()
      const documentDigest = computeProductSpecDocumentDigest(markdown)
      try {
        const intentDigest = computeProductSpecIntentDigest(markdown)
        console.log(`${path}\n  document ${documentDigest}\n  intent   ${intentDigest}`)
      } catch (error) {
        failures += 1
        console.error(`FAIL ${path}\n  document ${documentDigest}\n  intent   unavailable: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    if (failures > 0) process.exit(1)
    return
  }

  if (command === "init") {
    const [file, ...flags] = rest
    if (!file || !file.endsWith(PRODUCT_SPEC_EXTENSION)) {
      console.error(`usage: product-spec init <name>${PRODUCT_SPEC_EXTENSION}`)
      process.exit(2)
    }
    const titleIndex = flags.indexOf("--title")
    const typeIndex = flags.indexOf("--type")
    const title =
      titleIndex === -1
        ? basename(file, PRODUCT_SPEC_EXTENSION)
            .split(/[-_]/)
            .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
            .join(" ")
        : (flags[titleIndex + 1] ?? "Untitled")
    const artifactType = (typeIndex === -1 ? "prd" : flags[typeIndex + 1]) as ArtifactType
    if (await Runtime.file(file).exists()) {
      console.error(`refusing to overwrite existing file: ${file}`)
      process.exit(1)
    }
    await Runtime.write(file, starterProductSpec({ title, artifactType }))
    console.log(`created ${file}`)
    return
  }

  console.error("usage: product-spec <validate|validate-trace|digest|init> ...")
  process.exit(2)
}

await main()
