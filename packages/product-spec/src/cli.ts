#!/usr/bin/env bun
// product-spec CLI: validate `.product-spec.md` files, scaffold new ones.
//
//   bun packages/product-spec/src/cli.ts validate <file...>
//   bun packages/product-spec/src/cli.ts validate --specs-root <dir>
//   bun packages/product-spec/src/cli.ts init <file> [--title "..."] [--type prd|hypothesis]
import { readdirSync, statSync } from "node:fs"
import { basename, join } from "node:path"

import {
  PRODUCT_SPEC_EXTENSION,
  starterProductSpec,
  validateProductSpec,
} from "./index.ts"
import type { ArtifactType } from "./index.ts"

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

const validateFiles = async (paths: string[]): Promise<number> => {
  let failures = 0
  for (const path of paths) {
    const markdown = await Bun.file(path).text()
    const result = validateProductSpec(markdown)
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

  if (command === "validate") {
    const specsRootIndex = rest.indexOf("--specs-root")
    const paths =
      specsRootIndex === -1
        ? rest
        : collectSpecFiles(rest[specsRootIndex + 1] ?? "specs")
    if (paths.length === 0) {
      console.error("usage: product-spec validate <file...> | --specs-root <dir>")
      process.exit(2)
    }
    const failures = await validateFiles(paths)
    if (failures > 0) {
      console.error(`${failures} invalid Product Spec file(s).`)
      process.exit(1)
    }
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
    if (await Bun.file(file).exists()) {
      console.error(`refusing to overwrite existing file: ${file}`)
      process.exit(1)
    }
    await Bun.write(file, starterProductSpec({ title, artifactType }))
    console.log(`created ${file}`)
    return
  }

  console.error("usage: product-spec <validate|init> ...")
  process.exit(2)
}

await main()
