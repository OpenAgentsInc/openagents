/**
 * Installed-artifact oracle for the package-owned Codex runtime (#8827).
 * It invokes the exact unpacked binary with a minimal GUI PATH; global
 * Codex/NVM state cannot satisfy this proof. The returned receipt is safe to
 * publish: it contains no filesystem path, environment, or process output.
 */
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { accessSync, constants, readFileSync, statSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const packageRoot = path.dirname(fileURLToPath(new URL("../package.json", import.meta.url)))
const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
  version: string
  dependencies: Record<string, string>
}

export type CodexArtifactRuntimeReceipt = Readonly<{
  schema: "openagents.desktop.codex_runtime_artifact.v1"
  appVersion: string
  platform: string
  arch: string
  source: "desktop-bundle"
  expectedVersion: string
  observedVersion: string
  state: "ready"
  identitySha256: string
  minimalPath: true
  signatureVerified: boolean
}>

type Exec = (file: string, args: ReadonlyArray<string>, options?: Readonly<{
  env?: NodeJS.ProcessEnv
  encoding?: BufferEncoding
  timeout?: number
}>) => string

const defaultExec: Exec = (file, args, options) => execFileSync(file, [...args], {
  ...options,
  stdio: ["ignore", "pipe", "pipe"],
}).toString().trim()

export const packagedCodexPath = (appPath: string, platform: string, arch: string): string => {
  if (platform !== "darwin" || (arch !== "arm64" && arch !== "x64")) {
    throw new Error(`unsupported packaged Codex target: ${platform}-${arch}`)
  }
  const triple = arch === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin"
  return path.join(
    appPath, "Contents", "Resources", "app.asar.unpacked", "node_modules",
    `@openai/codex-darwin-${arch}`, "vendor", triple, "bin", "codex",
  )
}

export const verifyPackagedCodexRuntime = (input: Readonly<{
  appPath: string
  platform: string
  arch: string
  requireSignature?: boolean
  exec?: Exec
}>): CodexArtifactRuntimeReceipt => {
  const executable = packagedCodexPath(input.appPath, input.platform, input.arch)
  if (!path.isAbsolute(executable) || !statSync(executable).isFile()) throw new Error("packaged Codex is not a regular file")
  accessSync(executable, constants.X_OK)
  const exec = input.exec ?? defaultExec
  const architecture = exec("/usr/bin/file", ["-b", executable], { encoding: "utf8", timeout: 5_000 })
  const fileArchitecture = input.arch === "x64" ? "x86_64" : "arm64"
  if (!architecture.includes(fileArchitecture)) throw new Error(`packaged Codex has wrong architecture: expected ${input.arch}`)
  if (input.requireSignature === true) {
    exec("/usr/bin/codesign", ["--verify", "--strict", "--verbose=2", executable], { encoding: "utf8", timeout: 10_000 })
  }
  const expectedVersion = manifest.dependencies["@openai/codex"]
  if (expectedVersion === undefined) throw new Error("desktop Codex version pin is missing")
  const versionOutput = exec(executable, ["--version"], {
    encoding: "utf8",
    timeout: 5_000,
    env: { PATH: "/usr/bin:/bin:/usr/sbin:/sbin" },
  })
  const observedVersion = /^(?:codex-cli|codex)\s+(\d+\.\d+\.\d+)$/u.exec(versionOutput)?.[1]
  if (observedVersion !== expectedVersion) {
    throw new Error(`packaged Codex version mismatch: expected ${expectedVersion}, observed ${observedVersion ?? "malformed"}`)
  }
  return {
    schema: "openagents.desktop.codex_runtime_artifact.v1",
    appVersion: manifest.version,
    platform: input.platform,
    arch: input.arch,
    source: "desktop-bundle",
    expectedVersion,
    observedVersion,
    state: "ready",
    identitySha256: createHash("sha256").update(readFileSync(executable)).digest("hex"),
    minimalPath: true,
    signatureVerified: input.requireSignature === true,
  }
}

const direct = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
if (direct) {
  const appIndex = process.argv.indexOf("--app")
  if (appIndex < 0 || process.argv[appIndex + 1] === undefined) throw new Error("usage: codex-runtime-artifact-smoke.ts --app <OpenAgents.app> [--signed]")
  const receipt = verifyPackagedCodexRuntime({
    appPath: path.resolve(process.argv[appIndex + 1]!),
    platform: "darwin",
    arch: process.arch,
    requireSignature: process.argv.includes("--signed"),
  })
  process.stdout.write(`${JSON.stringify(receipt)}\n`)
}
