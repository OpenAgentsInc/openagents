#!/usr/bin/env bun
import { resolve, relative } from "node:path"

import {
  ASSURANCE_SPEC_EXTENSION,
  assessAssuranceSpec,
  inventoryRepository,
  parseAssuranceSpec,
  proposeAssuranceSpec,
  validateAssuranceSpec,
} from "./index.ts"

const usage = (): never => {
  console.error("usage:")
  console.error("  assurance-spec propose <file.product-spec.md> [--repo <dir>] [--out <file.assurance-spec.md>] [--inventory-out <file.json>] [--id <id>] [--title <title>] [--author <author>] [--force]")
  console.error("  assurance-spec validate <file.assurance-spec.md> [...]")
  console.error("  assurance-spec coverage <file.assurance-spec.md> [--json]")
  process.exit(2)
}

const flagValue = (args: ReadonlyArray<string>, flag: string): string | undefined => {
  const index = args.indexOf(flag)
  return index === -1 ? undefined : args[index + 1]
}

const defaultOutput = (input: string): string => input.endsWith(".product-spec.md")
  ? `${input.slice(0, -".product-spec.md".length)}${ASSURANCE_SPEC_EXTENSION}`
  : `${input}${ASSURANCE_SPEC_EXTENSION}`

const propose = async (args: ReadonlyArray<string>): Promise<void> => {
  const input = args[0]
  if (input === undefined || input.startsWith("--")) usage()
  const inputAbsolute = resolve(input)
  if (!(await Bun.file(inputAbsolute).exists())) {
    console.error(`ProductSpec does not exist: ${input}`)
    process.exit(1)
  }
  const repoFlag = flagValue(args, "--repo")
  const repositoryRoot = repoFlag === undefined ? undefined : resolve(repoFlag)
  const base = repositoryRoot ?? process.cwd()
  const productSpecPath = relative(base, inputAbsolute).replaceAll("\\", "/")
  const output = resolve(flagValue(args, "--out") ?? defaultOutput(input))
  if (await Bun.file(output).exists() && !args.includes("--force")) {
    console.error(`refusing to overwrite existing file: ${output}`)
    process.exit(1)
  }
  const repositoryInventory = repositoryRoot === undefined ? undefined : inventoryRepository(repositoryRoot)
  const result = proposeAssuranceSpec({
    productSpecPath,
    productSpecMarkdown: await Bun.file(inputAbsolute).text(),
    ...(repositoryInventory === undefined ? {} : { repositoryInventory }),
    ...(flagValue(args, "--id") === undefined ? {} : { assuranceSpecId: flagValue(args, "--id")! }),
    ...(flagValue(args, "--title") === undefined ? {} : { title: flagValue(args, "--title")! }),
    ...(flagValue(args, "--author") === undefined ? {} : { author: flagValue(args, "--author")! }),
  })
  if (!result.ok) {
    console.error("AssuranceSpec proposal failed.")
    for (const diagnostic of result.diagnostics) console.error(`  ${diagnostic.code}: ${diagnostic.message}`)
    process.exit(1)
  }
  await Bun.write(output, result.markdown)
  const inventoryOut = flagValue(args, "--inventory-out")
  if (inventoryOut !== undefined) {
    await Bun.write(resolve(inventoryOut), `${JSON.stringify(result.document.environments.repository_inventory, null, 2)}\n`)
  }
  console.log(`proposed ${output}`)
  console.log(`  ${result.adequacy.coverage.obligations} obligations · ${result.adequacy.coverage.needs_design} need design · ${result.adequacy.coverage.ready} ready`)
  console.log(`  repository ${result.document.environments.repository_inventory.state} · structural valid · design ready ${result.adequacy.design_ready ? "yes" : "no"} · execution authorized no`)
}

const validate = async (paths: ReadonlyArray<string>): Promise<void> => {
  if (paths.length === 0) usage()
  let failures = 0
  for (const path of paths) {
    const result = validateAssuranceSpec(await Bun.file(path).text())
    if (result.valid) {
      console.log(`ok ${path}`)
    } else {
      failures += 1
      console.error(`FAIL ${path}`)
      for (const error of result.errors) console.error(`  ${error.code}: ${error.message}`)
    }
  }
  if (failures > 0) process.exit(1)
}

const coverage = async (args: ReadonlyArray<string>): Promise<void> => {
  const path = args[0]
  if (path === undefined || path.startsWith("--")) usage()
  const validation = validateAssuranceSpec(await Bun.file(path).text())
  if (!validation.valid || validation.document === undefined) {
    for (const error of validation.errors) console.error(`${error.code}: ${error.message}`)
    process.exit(1)
  }
  const assessment = assessAssuranceSpec(parseAssuranceSpec(await Bun.file(path).text()))
  if (args.includes("--json")) {
    console.log(JSON.stringify(assessment, null, 2))
    return
  }
  console.log(`${path}: ${assessment.coverage.ready}/${assessment.coverage.obligations} obligations ready; ${assessment.coverage.needs_design} need design`)
  for (const diagnostic of assessment.diagnostics) {
    console.log(`  ${diagnostic.severity} ${diagnostic.code}: ${diagnostic.message}`)
  }
}

const [command, ...args] = process.argv.slice(2)
if (command === "propose") await propose(args)
else if (command === "validate") await validate(args)
else if (command === "coverage") await coverage(args)
else usage()
