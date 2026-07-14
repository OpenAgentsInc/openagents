import { spawnSync } from "node:child_process"
import { existsSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const runtimeRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const projectPath = join(runtimeRoot, "tsconfig.tests.json")
const baselinePath = join(runtimeRoot, "test-typecheck-baseline.json")
const tscPath = resolve(runtimeRoot, "../../../../node_modules/typescript/bin/tsc")

const runTsc = args => spawnSync(process.execPath, [tscPath, ...args], {
  cwd: runtimeRoot,
  encoding: "utf8",
})

const discoverTests = () => {
  const found = []
  const walk = directory => {
    for (const entry of readdirSync(directory)) {
      const path = join(directory, entry)
      if (statSync(path).isDirectory()) walk(path)
      else if (/\.(?:test|spec)\.tsx?$/.test(entry)) found.push(realpathSync(path))
    }
  }
  walk(join(runtimeRoot, "src"))
  walk(join(runtimeRoot, "tests"))
  return found.sort()
}

const listProjectRootFiles = () => {
  const run = runTsc(["-p", projectPath, "--showConfig"])
  if (run.status !== 0) throw new Error(`could not resolve Probe test project roots:\n${run.stderr}`)
  const config = JSON.parse(run.stdout)
  if (!Array.isArray(config.files)) throw new Error("resolved Probe test project has no explicit root file list")
  return new Set(config.files.map(path => realpathSync(resolve(runtimeRoot, path))))
}

const currentDiagnostics = () => {
  const run = runTsc(["-p", projectPath, "--noEmit", "--pretty", "false"])
  const output = `${run.stdout}\n${run.stderr}`
  const diagnostics = [...output.matchAll(/^(.+?)\((\d+),(\d+)\): error (TS\d+): (.+)$/gm)].map(match => {
    const path = resolve(runtimeRoot, match[1])
    return `${relative(runtimeRoot, path)}:${match[2]}:${match[3]}:${match[4]}:${match[5]}`
  }).sort()
  if (run.status !== 0 && diagnostics.length === 0) throw new Error(`Probe TypeScript failed without parseable diagnostics:\n${output}`)
  return diagnostics
}

const enforceShrinkOnlyBaseline = diagnostics => {
  const initialize = process.argv.includes("--initialize-baseline")
  const update = process.argv.includes("--update-baseline")
  if (!existsSync(baselinePath)) {
    if (!initialize) throw new Error("Probe test diagnostic baseline is missing; initialization is an explicit one-time repository action")
    writeFileSync(baselinePath, `${JSON.stringify({ schemaVersion: 1, diagnostics }, null, 2)}\n`)
    console.log(`initialized Probe test diagnostic baseline (${diagnostics.length} diagnostics)`)
    return
  }
  const baseline = JSON.parse(readFileSync(baselinePath, "utf8"))
  const expected = new Set(baseline.diagnostics)
  const actual = new Set(diagnostics)
  const added = diagnostics.filter(diagnostic => !expected.has(diagnostic))
  const stale = baseline.diagnostics.filter(diagnostic => !actual.has(diagnostic))
  if (added.length > 0) throw new Error(`Probe test typecheck added ${added.length} diagnostic(s):\n${added.join("\n")}`)
  if (update) {
    writeFileSync(baselinePath, `${JSON.stringify({ schemaVersion: 1, diagnostics }, null, 2)}\n`)
    console.log(`shrunk Probe test diagnostic baseline ${baseline.diagnostics.length} -> ${diagnostics.length}`)
    return
  }
  if (stale.length > 0) throw new Error(`Probe test typecheck baseline has ${stale.length} stale resolved diagnostic(s); run the shrink-only update and commit the reduction:\n${stale.join("\n")}`)
}

const proveBrokenFixtureFails = () => {
  const scratch = mkdtempSync(join(runtimeRoot, ".typecheck-negative-"))
  try {
    const fixturePath = join(scratch, "missing-required-assignment-property.ts")
    const assignmentPath = join(runtimeRoot, "src/contracts/assignment.ts")
    writeFileSync(fixturePath, `import type { ProbeRunAssignment } from ${JSON.stringify(assignmentPath)}\n\nconst broken: ProbeRunAssignment = {\n  assignmentId: "assignment_negative",\n  runnerSessionId: "runner_negative"\n}\nvoid broken\n`)
    writeFileSync(join(scratch, "tsconfig.json"), JSON.stringify({
      extends: projectPath,
      compilerOptions: { noEmit: true },
      files: [fixturePath],
      include: [],
    }, null, 2))
    const run = runTsc(["-p", join(scratch, "tsconfig.json"), "--pretty", "false"])
    const diagnostics = `${run.stdout}\n${run.stderr}`
    if (run.status === 0 || !diagnostics.includes("Property 'goal' is missing")) {
      throw new Error(`negative fixture did not fail on the required ProbeRunAssignment.goal field:\n${diagnostics}`)
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

const tests = discoverTests()
const projectFiles = listProjectRootFiles()
const missing = tests.filter(path => !projectFiles.has(path))
if (missing.length > 0) {
  throw new Error(`Probe test project omits ${missing.length} test root(s):\n${missing.map(path => relative(runtimeRoot, path)).join("\n")}`)
}
enforceShrinkOnlyBaseline(currentDiagnostics())
proveBrokenFixtureFails()
console.log(`probe test typecheck coverage OK (${tests.length} test roots; shrink-only diagnostics enforced; negative required-property fixture rejected)`)
