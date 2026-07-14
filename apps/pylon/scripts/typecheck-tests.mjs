#!/usr/bin/env node

import { readdir, readFile, writeFile } from "node:fs/promises"
import { dirname, relative, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"

import ts from "typescript"

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const defaultRoot = resolve(scriptDirectory, "..")

const normalizePath = (value) => value.replaceAll("\\", "/")

const diagnosticKey = (diagnostic) =>
  JSON.stringify([
    diagnostic.file,
    diagnostic.line,
    diagnostic.column,
    diagnostic.code,
    diagnostic.message,
  ])

const collectTestFiles = async (directory) => {
  const files = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name)
    if (entry.isDirectory() && entry.name !== "node_modules" && entry.name !== ".git") {
      files.push(...(await collectTestFiles(path)))
    } else if (/\.(?:test|spec)\.(?:cts|mts|tsx?)$/.test(entry.name)) {
      files.push(path)
    }
  }
  return files.sort()
}

const formatDiagnostic = (root, diagnostic) => {
  const file = diagnostic.file
  const location = file && diagnostic.start !== undefined
    ? file.getLineAndCharacterOfPosition(diagnostic.start)
    : undefined
  return {
    file: file ? normalizePath(relative(root, file.fileName)) : "<config>",
    line: location ? location.line + 1 : 0,
    column: location ? location.character + 1 : 0,
    code: diagnostic.code,
    message: ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
  }
}

const sortDiagnostics = (diagnostics) =>
  [...new Map(diagnostics.map((diagnostic) => [diagnosticKey(diagnostic), diagnostic])).values()]
    .sort((left, right) => diagnosticKey(left).localeCompare(diagnosticKey(right)))

const printDifference = (label, diagnostics) => {
  if (diagnostics.length === 0) return
  console.error(`\n${label} (${diagnostics.length}):`)
  for (const diagnostic of diagnostics.slice(0, 50)) {
    console.error(
      `  ${diagnostic.file}:${diagnostic.line}:${diagnostic.column} TS${diagnostic.code} ${diagnostic.message}`,
    )
  }
  if (diagnostics.length > 50) console.error(`  ... ${diagnostics.length - 50} more`)
}

export const typecheckTests = async ({
  root = defaultRoot,
  project = resolve(root, "tsconfig.tests.json"),
  baseline = resolve(root, "typecheck-tests-baseline.json"),
  updateBaseline = false,
} = {}) => {
  const configResult = ts.readConfigFile(project, ts.sys.readFile)
  const parsed = ts.parseJsonConfigFileContent(
    configResult.config ?? {},
    ts.sys,
    dirname(project),
    undefined,
    project,
  )
  const program = ts.createProgram({ rootNames: parsed.fileNames, options: parsed.options })
  const diagnostics = sortDiagnostics(
    [configResult.error, ...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
      .filter(Boolean)
      .map((diagnostic) => formatDiagnostic(root, diagnostic)),
  )

  const expectedTestFiles = await collectTestFiles(root)
  const programFiles = new Set(program.getRootFileNames().map((path) => resolve(path)))
  const omittedTestFiles = expectedTestFiles.filter((path) => !programFiles.has(path))
  if (omittedTestFiles.length > 0) {
    throw new Error(
      `test typecheck project omitted ${omittedTestFiles.length} test file(s):\n${omittedTestFiles
        .slice(0, 50)
        .map((path) => `  ${normalizePath(relative(root, path))}`)
        .join("\n")}`,
    )
  }

  const nextBaseline = {
    schemaVersion: 1,
    project: normalizePath(relative(root, project)),
    testFileCount: expectedTestFiles.length,
    diagnostics,
  }

  let previousBaseline
  try {
    previousBaseline = JSON.parse(await readFile(baseline, "utf8"))
  } catch (error) {
    if (error?.code !== "ENOENT") throw error
  }

  if (updateBaseline) {
    if (previousBaseline) {
      const previousKeys = new Set(previousBaseline.diagnostics.map(diagnosticKey))
      const added = diagnostics.filter((diagnostic) => !previousKeys.has(diagnosticKey(diagnostic)))
      if (added.length > 0) {
        printDifference("new diagnostics cannot be added to the baseline", added)
        throw new Error("Pylon test diagnostic baseline may only shrink")
      }
      if (expectedTestFiles.length < previousBaseline.testFileCount) {
        throw new Error(
          `test file count fell from ${previousBaseline.testFileCount} to ${expectedTestFiles.length}; ` +
            "deletion requires an explicit baseline review",
        )
      }
    }
    await writeFile(baseline, `${JSON.stringify(nextBaseline, null, 2)}\n`)
    console.log(
      `wrote ${normalizePath(relative(root, baseline))}: ${expectedTestFiles.length} tests, ` +
        `${diagnostics.length} allowed diagnostics`,
    )
    return nextBaseline
  }

  if (!previousBaseline) {
    throw new Error("Pylon test diagnostic baseline is missing; review and generate it explicitly")
  }
  if (previousBaseline.schemaVersion !== 1) {
    throw new Error(`unsupported Pylon test diagnostic baseline v${previousBaseline.schemaVersion}`)
  }
  if (previousBaseline.testFileCount !== expectedTestFiles.length) {
    throw new Error(
      `test file count changed: baseline ${previousBaseline.testFileCount}, current ${expectedTestFiles.length}`,
    )
  }

  const previousByKey = new Map(
    previousBaseline.diagnostics.map((diagnostic) => [diagnosticKey(diagnostic), diagnostic]),
  )
  const currentByKey = new Map(diagnostics.map((diagnostic) => [diagnosticKey(diagnostic), diagnostic]))
  const added = diagnostics.filter((diagnostic) => !previousByKey.has(diagnosticKey(diagnostic)))
  const resolved = previousBaseline.diagnostics.filter(
    (diagnostic) => !currentByKey.has(diagnosticKey(diagnostic)),
  )
  printDifference("new diagnostics", added)
  printDifference("resolved diagnostics must be removed from the baseline", resolved)
  if (added.length > 0 || resolved.length > 0) {
    throw new Error(
      "Pylon test typecheck baseline changed; fix new errors and run the shrink-only baseline update after resolved errors",
    )
  }

  console.log(
    `Pylon test typecheck green: ${expectedTestFiles.length} tests compiled; ` +
      `${diagnostics.length} known diagnostics; 0 new`,
  )
  return nextBaseline
}

const args = new Set(process.argv.slice(2))
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    await typecheckTests({ updateBaseline: args.has("--update-baseline") })
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  }
}
