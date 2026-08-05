import { spawnSync } from "node:child_process"
import { mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const repoRoot = resolve(appRoot, "../..")
const projectPath = join(appRoot, "tsconfig.tests.json")
const tscPath = join(repoRoot, "node_modules/typescript/bin/tsc")

const runTsc = args => spawnSync(process.execPath, [tscPath, ...args], {
  cwd: appRoot,
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
  walk(join(appRoot, "src"))
  return found.sort()
}

const listProjectRootFiles = () => {
  const run = runTsc(["-p", projectPath, "--showConfig"])
  if (run.status !== 0) throw new Error(`could not resolve OA Updates test-project roots:\n${run.stderr}`)
  const config = JSON.parse(run.stdout)
  if (!Array.isArray(config.files)) throw new Error("resolved OA Updates test project has no explicit root file list")
  return new Set(config.files.map(path => realpathSync(resolve(appRoot, path))))
}

const typecheckProject = () => {
  const run = runTsc(["-p", projectPath, "--noEmit", "--pretty", "false"])
  if (run.status !== 0) throw new Error(`OA Updates strict test project failed:\n${run.stdout}${run.stderr}`)
}

// Negative fixture: a Pylon release manifest missing its required artifact
// digest. `sha256` is the field the self-updater verifies before installing a
// downloaded binary, so TypeScript must reject a manifest that omits it long
// before such a release could reach the feed.
const fixtureSource = ({ includeSha256 }) => `import type { PylonReleaseManifest } from ${JSON.stringify(join(appRoot, "src/pylon-release.ts"))}

const candidate: PylonReleaseManifest = {
  version: "1.0.0-rc.1",
  channel: "rc",
  platform: "darwin-arm64",
  artifactUrl: "https://updates.openagents.test/assets/fixture-hash",
  ${includeSha256 ? `sha256: "${"0".repeat(64)}",` : ""}
  signature: "fixture-signature",
  kid: "2dbe811d19f67528"
}
void candidate
`

const proveBrokenFixtureFails = () => {
  const cacheRoot = join(repoRoot, "node_modules/.cache")
  mkdirSync(cacheRoot, { recursive: true })
  const scratch = mkdtempSync(join(cacheRoot, "oa-updates-typecheck-"))
  try {
    const fixturePath = join(scratch, "update-fixture.ts")
    const fixtureProjectPath = join(scratch, "tsconfig.json")
    writeFileSync(fixtureProjectPath, `${JSON.stringify({
      extends: projectPath,
      compilerOptions: { noEmit: true },
      files: [fixturePath],
      include: [],
    }, null, 2)}\n`)

    writeFileSync(fixturePath, fixtureSource({ includeSha256: true }))
    const valid = runTsc(["-p", fixtureProjectPath, "--pretty", "false"])
    if (valid.status !== 0) throw new Error(`valid OA Updates fixture did not compile:\n${valid.stdout}${valid.stderr}`)

    writeFileSync(fixturePath, fixtureSource({ includeSha256: false }))
    const broken = runTsc(["-p", fixtureProjectPath, "--pretty", "false"])
    const diagnostics = `${broken.stdout}\n${broken.stderr}`
    if (broken.status === 0 || !diagnostics.includes("Property 'sha256' is missing")) {
      throw new Error(`negative fixture did not fail on required PylonReleaseManifest.sha256:\n${diagnostics}`)
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
}

const tests = discoverTests()
const projectRoots = listProjectRootFiles()
const missing = tests.filter(path => !projectRoots.has(path))
if (missing.length > 0) {
  throw new Error(`OA Updates test project omits ${missing.length} test root(s):\n${missing.map(path => relative(appRoot, path)).join("\n")}`)
}

typecheckProject()
proveBrokenFixtureFails()
console.log(`oa-updates test typecheck coverage OK (${tests.length} test roots; strict project clean; negative required-property fixture rejected)`)
