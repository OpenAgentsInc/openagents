import { readFileSync } from "node:fs"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"
import {
  decodeDesktopTargetBuildDescriptor,
  desktopTargets,
  TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
} from "../src/release-staging-contract.ts"
import {
  closureOwnerForDestination,
  executableDestinationAllowlist,
  nativeClosureEntries,
  plannedAsarPlacement,
  readDesktopManifestPins,
  requiredRuntimePackages,
  stagedTreeViolations,
  type StagedFile,
} from "../scripts/stage-target.ts"

const root = path.resolve(import.meta.dirname, "..")
const repoRoot = path.resolve(root, "../..")
const pins = readDesktopManifestPins(readFileSync(path.join(root, "package.json"), "utf8"))
const digest = (seed: string): string => seed.repeat(64).slice(0, 64)

const machO = (arch: "arm64" | "x64"): Uint8Array => {
  const bytes = new Uint8Array(16)
  bytes.set([0xcf, 0xfa, 0xed, 0xfe])
  bytes.set(arch === "arm64" ? [0x0c, 0x00, 0x00, 0x01] : [0x07, 0x00, 0x00, 0x01], 4)
  return bytes
}

const descriptor = decodeDesktopTargetBuildDescriptor({
  schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
  product: "OpenAgents",
  targetKey: "darwin-arm64",
  channel: "stable",
  version: "1.2.3",
  sourceRevision: "a".repeat(40),
  lockfileSha256: "b".repeat(64),
  formats: [...desktopTargets["darwin-arm64"].requiredFormats],
  signingPolicy: "production",
})

const darwinArm64Tree = (bridge?: StagedFile): Array<StagedFile> => [
  { path: "dist/main.js", byteLength: 10, executable: false, header: new Uint8Array([0x2f, 0x2f]), sha256: digest("a") },
  { path: "package.json", byteLength: 10, executable: false, header: new Uint8Array([0x7b]), sha256: digest("b") },
  { path: "native/arm64/oa-desktop-audio", byteLength: 100, executable: true, header: machO("arm64"), sha256: digest("c") },
  { path: "node_modules/@anthropic-ai/claude-agent-sdk/package.json", byteLength: 10, executable: false, header: new Uint8Array([0x7b]), sha256: digest("d") },
  { path: "node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude", byteLength: 100, executable: true, header: machO("arm64"), sha256: digest("e") },
  ...(bridge === undefined ? [] : [bridge]),
]

const bridgeFile = (arch: "arm64" | "x64", pathOverride?: string): StagedFile => ({
  path: pathOverride ?? "native/arm64/foundation-bridge",
  byteLength: 200,
  executable: true,
  header: machO(arch),
  sha256: digest("f"),
})

const audit = (files: ReadonlyArray<StagedFile>) => ({
  descriptor,
  files,
  runtimePackages: requiredRuntimePackages(descriptor, pins),
  repoRoot,
})

describe("Apple FM Swift sidecar staging admission (AFM-7)", () => {
  test("the executable allowlist admits the arm64 bridge path only", () => {
    expect(executableDestinationAllowlist.some((pattern) => pattern.test("native/arm64/foundation-bridge"))).toBe(true)
    expect(executableDestinationAllowlist.some((pattern) => pattern.test("native/x64/foundation-bridge"))).toBe(false)
  })

  test("a darwin-arm64 tree with the arm64 bridge Mach-O has no violation", () => {
    expect(stagedTreeViolations(audit(darwinArm64Tree(bridgeFile("arm64"))))).toEqual([])
  })

  test("a foreign-arch bridge at the arm64 path fails closed", () => {
    const violations = stagedTreeViolations(audit(darwinArm64Tree(bridgeFile("x64"))))
    expect(violations.map((v) => v.kind)).toContain("foreign_architecture_binary")
  })

  test("the bridge at an unallowlisted native path is refused", () => {
    const violations = stagedTreeViolations(audit(darwinArm64Tree(bridgeFile("arm64", "native/x64/foundation-bridge"))))
    // native/x64 is a foreign arch for darwin-arm64 AND off the bridge allowlist.
    expect(violations.map((v) => v.kind)).toContain("unallowlisted_binary")
  })

  test("the bridge ships as an extra-resource with an honest owned-source ledger owner", () => {
    expect(plannedAsarPlacement("native/arm64/foundation-bridge")).toBe("extra-resource")
    expect(
      closureOwnerForDestination("native/arm64/foundation-bridge", descriptor, new Map(), "0.1.0"),
    ).toEqual({ name: "foundation-bridge", version: "0.1.3", provenance: "workspace-crate" })
  })

  test("the per-file closure ledgers the bridge beside the voice helper", () => {
    const entries = nativeClosureEntries(descriptor, darwinArm64Tree(bridgeFile("arm64")), new Map(), "0.1.0")
    const bridge = entries.find((entry) => entry.destination === "native/arm64/foundation-bridge")
    expect(bridge).toMatchObject({
      name: "foundation-bridge",
      version: "0.1.3",
      architecture: "arm64",
      fileKind: "executable",
      asarPlacement: "extra-resource",
      provenance: "workspace-crate",
    })
  })
})
