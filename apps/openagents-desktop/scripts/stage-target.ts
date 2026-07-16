/**
 * Target-aware Desktop staging builder (DIST-03, #8916; repaired after the
 * independent review on #8916).
 *
 * Replaces host-incidental packaging assumptions with an explicit,
 * descriptor-driven staging flow (audit §10.2, ProductSpec §9):
 *
 *   1. Require an explicit `DesktopTargetBuildDescriptor` — host platform or
 *      architecture inference never selects a target.
 *   2. Create a CLEAN per-target temporary staging workspace and export the
 *      EXACT source revision into it (`git archive`). The developer checkout
 *      and shared node_modules are never the packaged source.
 *   3. Verify the immutable lockfile identity (descriptor.lockfileSha256)
 *      against the exported source before installing anything.
 *   4. EXECUTE the staging plan's locked, target-only production install
 *      (`pnpm install --prod --frozen-lockfile --ignore-scripts` with the
 *      target's `supportedArchitectures`) inside the staging workspace, then
 *      materialize the exact provider/native runtime packages for the target
 *      from THAT install. Unavailable runtimes fail with a typed
 *      `missing_runtime_package` before any native build, maker, or signing
 *      work begins.
 *   5. Build the application bundle from the exported source and build
 *      `oa-desktop-audio` with the target's EXPLICIT Rust triple into the
 *      staging workspace (`CARGO_TARGET_DIR` inside the workspace, path-prefix
 *      remapped for cross-run determinism) — never the shared checkout target
 *      directory.
 *   6. Run the staged-tree oracle (architecture, allowlist, dev-file,
 *      source-checkout, symlink-escape, unknown-executable, ASAR boundary)
 *      before any maker may run. Unknown or truncated executable identity
 *      fails CLOSED, including at allowlisted destinations.
 *   7. Emit the §9 public-safe native-component ledger: per-FILE native
 *      dependency closure (runtimes, CLIs, native modules, shared libraries,
 *      helpers, WASM, executables — each with architecture, signing state,
 *      and planned ASAR placement), lockfile digest, OS image identity, and
 *      the Electron/Node/pnpm/Forge/maker/Rust/compiler toolchain identity.
 *      Production staging also returns a receipt draft the maker/worker
 *      finalizes into a `DesktopBuildReceipt`. Unsigned-dev staging NEVER
 *      yields a receipt.
 *
 * Forge consumption (blockers 2 and 3): `forge.config.ts` requires
 * `OA_DESKTOP_STAGING_WORKSPACE`, decodes the staged descriptor once, packages
 * the STAGED tree (the checkout copy is fully discarded), and — after the
 * package step assembles the real app.asar — feeds the REAL asar entry list
 * back through `stagedTreeViolations` via `assertPackagedAsarAdmissible` as a
 * live gate before any maker/signing work.
 *
 * Every decision is a pure function over injected inputs so fixture tests
 * prove target selection for all six targets without six production runners.
 */
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { chmod, cp, mkdir, readdir, readFile, readlink, lstat, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

import {
  type DesktopArtifactFormat,
  type DesktopBuildArtifactInput,
  type DesktopBuildReceipt,
  type DesktopBuildToolchain,
  type DesktopTargetBuildDescriptor,
  type DesktopTargetKey,
  type NativeComponentAsarPlacement,
  type NativeComponentLedger,
  type NativeComponentLedgerEntry,
  type NativeComponentSigningState,
  decodeDesktopBuildReceipt,
  decodeDesktopTargetBuildDescriptor,
  decodeNativeComponentLedger,
  desktopReleaseSetArtifactName,
  desktopTargets,
  nativeComponentLedgerDigest,
  nativeComponentLedgerRef,
  NATIVE_COMPONENT_LEDGER_SCHEMA_ID,
  BUILD_RECEIPT_SCHEMA_ID,
  TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
} from "../src/release-staging-contract.ts";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkoutRoot = path.resolve(appRoot, "../..");
const DESKTOP_PACKAGE_FILTER = "@openagentsinc/openagents-desktop";

// ---------------------------------------------------------------------------
// Staging plan — pure projection of a descriptor into worker commands
// ---------------------------------------------------------------------------

export interface RequiredRuntimePackage {
  /** Installed package name inside the staged bundle's node_modules. */
  readonly name: string;
  /** Expected version identity (lockfile-pinned). */
  readonly version: string;
  readonly role: "provider-runtime";
}

export interface TargetStagingPlan {
  readonly targetKey: DesktopTargetKey;
  readonly rustTargetTriple: string;
  /** mkdtemp prefix for the clean per-target staging workspace. */
  readonly workspacePrefix: string;
  /**
   * Locked production install for the TARGET OS/architecture only, EXECUTED
   * inside the staging workspace's exported source. `supportedArchitectures`
   * pins pnpm's optional-dependency selection to the descriptor target, so a
   * host of a different architecture can never satisfy the closure
   * implicitly, and the frozen lockfile is the only resolution authority.
   */
  readonly install: {
    readonly command: "pnpm";
    readonly args: ReadonlyArray<string>;
    readonly supportedArchitectures: {
      readonly os: ReadonlyArray<string>;
      readonly cpu: ReadonlyArray<string>;
      readonly libc: ReadonlyArray<string>;
    };
    readonly env: Readonly<Record<string, string>>;
  };
  /** Explicit-triple native build for owned components. */
  readonly cargo: {
    readonly args: ReadonlyArray<string>;
    /** Output path relative to the staging cargo target directory. */
    readonly outputRelativePath: string;
  };
  /** Destination of the owned voice helper inside the staged bundle. */
  readonly nativeHelperDestination: string;
  readonly runtimePackages: ReadonlyArray<RequiredRuntimePackage>;
  /** Checkout-relative application resources admitted into staging. */
  readonly resourceAllowlist: ReadonlyArray<string>;
}

interface DesktopManifestPins {
  readonly claudeAgentSdk: string;
  readonly codex: string;
}

export const readDesktopManifestPins = (manifestSource: string): DesktopManifestPins => {
  const manifest = JSON.parse(manifestSource) as { dependencies?: Record<string, string> };
  const claudeAgentSdk = manifest.dependencies?.["@anthropic-ai/claude-agent-sdk"];
  const codex = manifest.dependencies?.["@openai/codex"];
  if (claudeAgentSdk === undefined || codex === undefined) {
    throw new Error("desktop provider runtime version pins are missing from package.json");
  }
  return { claudeAgentSdk, codex };
};

export const requiredRuntimePackages = (
  descriptor: DesktopTargetBuildDescriptor,
  pins: DesktopManifestPins,
): ReadonlyArray<RequiredRuntimePackage> => {
  const { platform, arch } = desktopTargets[descriptor.targetKey];
  return [
    {
      name: "@anthropic-ai/claude-agent-sdk",
      version: pins.claudeAgentSdk,
      role: "provider-runtime",
    },
    {
      name: `@anthropic-ai/claude-agent-sdk-${platform}-${arch}`,
      version: pins.claudeAgentSdk,
      role: "provider-runtime",
    },
    { name: "@openai/codex", version: pins.codex, role: "provider-runtime" },
    {
      name: `@openai/codex-${platform}-${arch}`,
      version: `${pins.codex}-${platform}-${arch}`,
      role: "provider-runtime",
    },
  ];
};

export const stagingPlanForDescriptor = (
  descriptor: DesktopTargetBuildDescriptor,
  pins: DesktopManifestPins,
): TargetStagingPlan => {
  const definition = desktopTargets[descriptor.targetKey];
  return {
    targetKey: descriptor.targetKey,
    rustTargetTriple: definition.rustTargetTriple,
    workspacePrefix: `oa-desktop-stage-${descriptor.targetKey}-`,
    install: {
      command: "pnpm",
      args: [
        "install",
        "--prod",
        "--frozen-lockfile",
        "--ignore-scripts",
        "--prefer-offline",
        "--filter",
        `${DESKTOP_PACKAGE_FILTER}...`,
      ],
      supportedArchitectures: {
        os: [definition.platform],
        cpu: [definition.arch],
        libc: definition.platform === "linux" ? ["glibc"] : ["current"],
      },
      env: {
        npm_config_platform: definition.platform,
        npm_config_arch: definition.arch,
      },
    },
    cargo: {
      args: [
        "build",
        "--release",
        "-p",
        "oa-desktop-audio",
        "--target",
        definition.rustTargetTriple,
      ],
      outputRelativePath: path.posix.join(
        definition.rustTargetTriple,
        "release",
        definition.platform === "win32" ? "oa-desktop-audio.exe" : "oa-desktop-audio",
      ),
    },
    nativeHelperDestination: path.posix.join(
      "native",
      definition.arch,
      definition.platform === "win32" ? "oa-desktop-audio.exe" : "oa-desktop-audio",
    ),
    runtimePackages: requiredRuntimePackages(descriptor, pins),
    resourceAllowlist: ["dist", "package.json", "resources", "build"],
  };
};

// ---------------------------------------------------------------------------
// Executable identity — header truth, never file names; unknown fails closed
// ---------------------------------------------------------------------------

export interface DetectedExecutable {
  readonly platform: "darwin" | "linux" | "win32" | "unknown";
  readonly arch: "arm64" | "x64" | "universal" | "other" | "unknown";
}

const readUInt16LE = (bytes: Uint8Array, offset: number): number | null =>
  offset + 1 < bytes.length ? bytes[offset]! | (bytes[offset + 1]! << 8) : null;

const readUInt32LE = (bytes: Uint8Array, offset: number): number | null =>
  offset + 3 < bytes.length
    ? (bytes[offset]! | (bytes[offset + 1]! << 8) | (bytes[offset + 2]! << 16)) +
      bytes[offset + 3]! * 0x1000000
    : null;

/**
 * Classifies native executable headers (Mach-O, ELF, PE). Returns null ONLY
 * for files carrying no native magic at all (scripts, JS shims, data) — those
 * are governed by the allowlist instead. Any native magic whose identity
 * cannot be proven from the header sample reports `unknown` and FAILS CLOSED
 * in the staged-tree oracle, even at allowlisted or runtime destinations.
 * Universal (fat) Mach-O binaries are reported as `universal`: a
 * single-target closure never admits multi-architecture payloads.
 */
export const detectExecutableArchitecture = (header: Uint8Array): DetectedExecutable | null => {
  if (header.length >= 4) {
    // Mach-O 64-bit little-endian file: cf fa ed fe
    if (header[0] === 0xcf && header[1] === 0xfa && header[2] === 0xed && header[3] === 0xfe) {
      const cputype = readUInt32LE(header, 4);
      const arch =
        cputype === null
          ? "unknown"
          : cputype === 0x0100000c
            ? "arm64"
            : cputype === 0x01000007
              ? "x64"
              : "other";
      return { platform: "darwin", arch };
    }
    // Mach-O 32-bit little-endian file: ce fa ed fe — never single-target valid.
    if (header[0] === 0xce && header[1] === 0xfa && header[2] === 0xed && header[3] === 0xfe) {
      return { platform: "darwin", arch: "other" };
    }
    // Universal (fat) Mach-O: ca fe ba be / ca fe ba bf (big-endian magic)
    if (
      header[0] === 0xca &&
      header[1] === 0xfe &&
      header[2] === 0xba &&
      (header[3] === 0xbe || header[3] === 0xbf)
    ) {
      return { platform: "darwin", arch: "universal" };
    }
    // ELF
    if (header[0] === 0x7f && header[1] === 0x45 && header[2] === 0x4c && header[3] === 0x46) {
      const machine = readUInt16LE(header, 18);
      const arch =
        machine === null ? "unknown" : machine === 0xb7 ? "arm64" : machine === 0x3e ? "x64" : "other";
      return { platform: "linux", arch };
    }
    // PE — an MZ stub whose PE header cannot be verified inside the sample is
    // an executable of UNPROVABLE identity, not a benign file (fail closed).
    if (header[0] === 0x4d && header[1] === 0x5a) {
      const peOffset = readUInt32LE(header, 0x3c);
      if (
        peOffset !== null &&
        peOffset + 5 < header.length &&
        header[peOffset] === 0x50 &&
        header[peOffset + 1] === 0x45 &&
        header[peOffset + 2] === 0x00 &&
        header[peOffset + 3] === 0x00
      ) {
        const machine = readUInt16LE(header, peOffset + 4);
        const arch =
          machine === null
            ? "unknown"
            : machine === 0xaa64
              ? "arm64"
              : machine === 0x8664
                ? "x64"
                : "other";
        return { platform: "win32", arch };
      }
      return { platform: "unknown", arch: "unknown" };
    }
  }
  return null;
};

const WASM_MAGIC = [0x00, 0x61, 0x73, 0x6d] as const;
export const isWasmModule = (header: Uint8Array): boolean =>
  header.length >= 4 && WASM_MAGIC.every((byte, index) => header[index] === byte);

/**
 * Embedded-signature presence from the header sample (pure, deterministic):
 * Mach-O LC_CODE_SIGNATURE load-command scan; PE Authenticode certificate
 * table directory; ELF/WASM/scripts have no embedded-signature concept.
 */
export const detectSigningState = (
  header: Uint8Array,
  detected: DetectedExecutable,
): NativeComponentSigningState => {
  if (detected.platform === "linux") return "not-applicable";
  if (detected.platform === "darwin" && detected.arch !== "universal") {
    const ncmds = readUInt32LE(header, 16);
    if (ncmds === null || ncmds > 4096) return "undetermined";
    let offset = 32;
    for (let index = 0; index < ncmds; index += 1) {
      const cmd = readUInt32LE(header, offset);
      const cmdsize = readUInt32LE(header, offset + 4);
      if (cmd === null || cmdsize === null || cmdsize < 8) return "undetermined";
      if (cmd === 0x1d) return "signed"; // LC_CODE_SIGNATURE
      offset += cmdsize;
    }
    return "unsigned";
  }
  if (detected.platform === "win32") {
    const peOffset = readUInt32LE(header, 0x3c);
    if (peOffset === null) return "undetermined";
    const optionalMagic = readUInt16LE(header, peOffset + 24);
    if (optionalMagic === null) return "undetermined";
    const certDirOffset =
      optionalMagic === 0x20b
        ? peOffset + 24 + 144
        : optionalMagic === 0x10b
          ? peOffset + 24 + 128
          : null;
    if (certDirOffset === null) return "undetermined";
    const size = readUInt32LE(header, certDirOffset + 4);
    if (size === null) return "undetermined";
    return size > 0 ? "signed" : "unsigned";
  }
  return "undetermined";
};

// ---------------------------------------------------------------------------
// Staged-tree oracle
// ---------------------------------------------------------------------------

export interface StagedFile {
  /** Bundle-relative POSIX path. */
  readonly path: string;
  readonly byteLength: number;
  readonly executable: boolean;
  /** Leading file bytes (enough for header + signature classification). */
  readonly header: Uint8Array;
  /** sha256 of the complete file bytes (per-file ledger identity). */
  readonly sha256?: string;
  /** Optional text content for source-checkout leak scanning. */
  readonly content?: string;
  /** Raw symlink target when the entry is a symlink (recorded via lstat). */
  readonly symlinkTarget?: string;
}

export const stagingViolationKinds = [
  "foreign_architecture_binary",
  "unknown_executable_identity",
  "unallowlisted_binary",
  "missing_runtime_package",
  "source_checkout_dependency",
  "development_file",
  "unexpected_asar_entry",
] as const;
export type StagingViolationKind = (typeof stagingViolationKinds)[number];

export interface StagingViolation {
  readonly kind: StagingViolationKind;
  readonly path: string;
  readonly detail: string;
}

/**
 * Executable destinations admitted into the staged bundle. Anything with a
 * native header outside this set fails the target, regardless of digest —
 * and anything AT one of these destinations whose native identity cannot be
 * proven fails closed as `unknown_executable_identity`.
 */
export const executableDestinationAllowlist: ReadonlyArray<RegExp> = [
  /^node_modules\/@openai\/codex-(?:darwin|win32|linux)-(?:arm64|x64)\/vendor\/[^/]+\/(?:bin\/(?:codex|codex-code-mode-host)(?:\.exe)?|codex-path\/rg(?:\.exe)?|codex-resources\/zsh\/bin\/zsh)$/,
  /^node_modules\/@anthropic-ai\/claude-agent-sdk-(?:darwin|win32|linux)-(?:arm64|x64)(?:-musl)?\/claude(?:\.exe)?$/,
  /^native\/(?:arm64|x64)\/oa-desktop-audio(?:\.exe)?$/,
];

const nativeArtifactExtension = /\.(?:node|dylib|so(?:\.\d+)*|dll)$/u;

const developmentFilePatterns: ReadonlyArray<RegExp> = [
  /^(?:src|scripts|tests|docs|receipts)(?:\/|$)/,
  /^(?:forge\.config\.ts|tsconfig(?:\..+)?\.json|vite\.config\.ts|README\.md|UPSTREAM\.md|GUARANTEES\.md)$/,
  /\.(?:test|spec)\.[cm]?[jt]sx?$/,
  /\.tsbuildinfo$/,
  /(?:^|\/)\.env(?:\..*)?$/,
  /(?:^|\/)\.git(?:\/|$)/,
];

const asarEntryAllowlist = (
  runtimePackages: ReadonlyArray<RequiredRuntimePackage>,
): ReadonlyArray<RegExp> => [
  /^dist(?:\/|$)/,
  /^package\.json$/,
  ...runtimePackages.map(
    (pkg) => new RegExp(`^node_modules/${pkg.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?:/|$)`),
  ),
];

export interface StagedTreeAuditInput {
  readonly descriptor: DesktopTargetBuildDescriptor;
  readonly files: ReadonlyArray<StagedFile>;
  readonly runtimePackages: ReadonlyArray<RequiredRuntimePackage>;
  /** Absolute source checkout root; its literal appearance is a leak. */
  readonly repoRoot: string;
  /** Additional forbidden absolute path prefixes (staging workspace roots). */
  readonly forbiddenPathPrefixes?: ReadonlyArray<string>;
  /** Optional app.asar entry listing (bundle-relative POSIX paths). */
  readonly asarEntries?: ReadonlyArray<string>;
}

/**
 * The package-content oracle over a staged tree. Deterministic, pure, and
 * fail-closed: ONE injected foreign binary, unknown/truncated executable
 * identity, missing runtime package, source-checkout dependency, escaping
 * symlink, development file, or unexpected ASAR entry yields a violation,
 * and any violation blocks maker/signing work.
 */
export const stagedTreeViolations = (
  input: StagedTreeAuditInput,
): ReadonlyArray<StagingViolation> => {
  const violations: Array<StagingViolation> = [];
  const definition = desktopTargets[input.descriptor.targetKey];
  const forbiddenPrefixes = [input.repoRoot, ...(input.forbiddenPathPrefixes ?? [])];

  for (const file of input.files) {
    if (developmentFilePatterns.some((pattern) => pattern.test(file.path))) {
      violations.push({
        kind: "development_file",
        path: file.path,
        detail: "development/source file staged into the bundle",
      });
      continue;
    }
    const expectsNativeExecutable = executableDestinationAllowlist.some((pattern) =>
      pattern.test(file.path),
    );
    // Symlink entries carry no content bytes; they are audited exclusively by
    // the symlink-escape rule below (a symlink at a native destination still
    // fails: it has no provable native header).
    const detected =
      file.symlinkTarget === undefined ? detectExecutableArchitecture(file.header) : null;
    if (detected !== null) {
      if (detected.platform === "unknown" || detected.arch === "unknown") {
        // Fail CLOSED: unprovable executable identity is never admissible,
        // including at allowlisted or runtime destinations.
        violations.push({
          kind: "unknown_executable_identity",
          path: file.path,
          detail: "executable identity cannot be proven from the file header",
        });
      } else if (detected.platform !== definition.platform || detected.arch !== definition.arch) {
        violations.push({
          kind: "foreign_architecture_binary",
          path: file.path,
          detail: `expected ${definition.platform}-${definition.arch}, found ${detected.platform}-${detected.arch}`,
        });
      }
      if (!expectsNativeExecutable) {
        violations.push({
          kind: "unallowlisted_binary",
          path: file.path,
          detail: "native executable outside the staged-binary allowlist",
        });
      }
    } else if (expectsNativeExecutable) {
      // Fail CLOSED: an allowlisted native destination must carry a provable
      // native header — a script, truncated, or opaque payload here is a
      // spoofed executable, not a pass.
      violations.push({
        kind: "unknown_executable_identity",
        path: file.path,
        detail: "allowlisted native destination does not carry a provable native header",
      });
    } else if (nativeArtifactExtension.test(file.path) && !isWasmModule(file.header)) {
      violations.push({
        kind: "unknown_executable_identity",
        path: file.path,
        detail: "native-artifact extension without a provable native header",
      });
    } else if (
      file.executable &&
      file.symlinkTarget === undefined &&
      !/^dist\//.test(file.path) &&
      !input.runtimePackages.some((pkg) => file.path.startsWith(`node_modules/${pkg.name}/`))
    ) {
      // Non-native script executables (JS launchers, shell wrappers) are
      // admitted only inside the application bundle or a required runtime
      // package; native-header binaries above stay on the strict allowlist.
      violations.push({
        kind: "unallowlisted_binary",
        path: file.path,
        detail: "executable staged outside the allowlist",
      });
    }
    if (
      file.content !== undefined &&
      forbiddenPrefixes.some((prefix) => file.content!.includes(prefix))
    ) {
      violations.push({
        kind: "source_checkout_dependency",
        path: file.path,
        detail: "absolute source-checkout or staging-workspace path baked into staged file",
      });
    }
    if (
      file.symlinkTarget !== undefined &&
      (path.isAbsolute(file.symlinkTarget) ||
        /^[A-Za-z]:/.test(file.symlinkTarget) ||
        file.symlinkTarget.split(/[\\/]/).includes(".."))
    ) {
      violations.push({
        kind: "source_checkout_dependency",
        path: file.path,
        detail: "symlink escapes the staging workspace",
      });
    }
  }

  for (const pkg of input.runtimePackages) {
    const prefix = `node_modules/${pkg.name}/`;
    if (!input.files.some((file) => file.path.startsWith(prefix))) {
      violations.push({
        kind: "missing_runtime_package",
        path: prefix,
        detail: `required ${pkg.role} package ${pkg.name}@${pkg.version} is absent for ${input.descriptor.targetKey}`,
      });
    }
  }

  if (input.asarEntries !== undefined) {
    const allowlist = asarEntryAllowlist(input.runtimePackages);
    for (const entry of input.asarEntries) {
      if (!allowlist.some((pattern) => pattern.test(entry))) {
        violations.push({
          kind: "unexpected_asar_entry",
          path: entry,
          detail: "ASAR entry outside the packaged-content allowlist",
        });
      }
    }
  }

  return violations;
};

// ---------------------------------------------------------------------------
// Planned ASAR placement — the pure mirror of forge.config packaging rules
// ---------------------------------------------------------------------------

/**
 * Deterministic placement plan for a staged file, mirroring forge.config.ts:
 * provider runtime packages and the renderer/worker entries are asar-unpacked
 * (child processes and worker_threads need real files); native/ and
 * dist/builtin-skills ship as extraResource; everything else packs into
 * app.asar. The post-package live gate verifies reality against this plan.
 */
export const plannedAsarPlacement = (filePath: string): NativeComponentAsarPlacement => {
  if (/^native\//.test(filePath) || /^dist\/builtin-skills\//.test(filePath)) {
    return "extra-resource";
  }
  if (/^dist\/(?:renderer|workers)\//.test(filePath)) return "unpacked";
  if (/^node_modules\/(?:@anthropic-ai\/claude-agent-sdk|@openai\/codex)/.test(filePath)) {
    return "unpacked";
  }
  return "asar";
};

// ---------------------------------------------------------------------------
// Ledger + receipt assembly
// ---------------------------------------------------------------------------

export interface StagedClosureMetadata {
  readonly lockfileSha256: string;
  readonly osImage: string;
  readonly toolchain: DesktopBuildToolchain;
}

/** Maker identity refs per required format (public-safe, no scoped names). */
export const makerIdentityRefs = (
  targetKey: DesktopTargetKey,
  forgeVersion: string,
): ReadonlyArray<{ readonly format: DesktopArtifactFormat; readonly ref: string }> =>
  desktopTargets[targetKey].requiredFormats.map((format) => ({
    format,
    ref:
      format === "dmg" || format === "zip"
        ? `maker:forge-${format}-${forgeVersion}`
        : `maker:pending-${format}`,
  }));

export interface NativeClosureOwner {
  readonly name: string;
  readonly version: string;
  readonly provenance: NativeComponentLedgerEntry["provenance"];
}

/** Resolves the owning component for a staged file destination. */
export const closureOwnerForDestination = (
  destination: string,
  descriptor: DesktopTargetBuildDescriptor,
  packageVersions: ReadonlyMap<string, string>,
  helperVersion: string,
): NativeClosureOwner => {
  if (destination.startsWith("node_modules/")) {
    const segments = destination.split("/");
    const name = segments[1]!.startsWith("@") ? `${segments[1]}/${segments[2]}` : segments[1]!;
    return {
      name,
      version: packageVersions.get(name) ?? "unknown",
      provenance: "locked-dependency",
    };
  }
  if (/^native\//.test(destination)) {
    return { name: "oa-desktop-audio", version: helperVersion, provenance: "workspace-crate" };
  }
  return {
    name: "openagents-desktop-app",
    version: descriptor.version,
    provenance: "application-resource",
  };
};

/**
 * Derives the §9 per-file native dependency closure from the staged tree:
 * every native executable, native Node module, shared library, WASM module,
 * and executable script — each with header-derived architecture, embedded
 * signing state, and planned ASAR placement. Aggregate package-tree entries
 * are NOT emitted.
 */
export const nativeClosureEntries = (
  descriptor: DesktopTargetBuildDescriptor,
  files: ReadonlyArray<StagedFile>,
  packageVersions: ReadonlyMap<string, string>,
  helperVersion: string,
): ReadonlyArray<NativeComponentLedgerEntry> => {
  const entries: Array<NativeComponentLedgerEntry> = [];
  for (const file of files) {
    if (file.symlinkTarget !== undefined) continue;
    const detected = detectExecutableArchitecture(file.header);
    let fileKind: NativeComponentLedgerEntry["fileKind"] | null = null;
    let architecture: NativeComponentLedgerEntry["architecture"] = "none";
    let signingState: NativeComponentSigningState = "not-applicable";
    if (detected !== null) {
      if (detected.arch !== "arm64" && detected.arch !== "x64") {
        throw new Error(
          `native closure cannot ledger unprovable executable ${file.path} (oracle must fail first)`,
        );
      }
      fileKind = /\.node$/u.test(file.path)
        ? "native-module"
        : /\.(?:dylib|so(?:\.\d+)*|dll)$/u.test(file.path)
          ? "shared-library"
          : "executable";
      architecture = detected.arch;
      signingState = detectSigningState(file.header, detected);
    } else if (isWasmModule(file.header) || /\.wasm$/u.test(file.path)) {
      fileKind = "wasm-module";
    } else if (file.executable) {
      fileKind = "script-launcher";
    }
    if (fileKind === null) continue;
    if (file.sha256 === undefined) {
      throw new Error(`staged closure file ${file.path} is missing its sha256 identity`);
    }
    const owner = closureOwnerForDestination(file.path, descriptor, packageVersions, helperVersion);
    entries.push({
      name: owner.name,
      version: owner.version,
      targetKey: descriptor.targetKey,
      sha256: file.sha256,
      byteLength: file.byteLength,
      provenance: owner.provenance,
      destination: file.path,
      fileKind,
      architecture,
      signingState,
      asarPlacement: plannedAsarPlacement(file.path),
    });
  }
  return entries;
};

export const buildNativeComponentLedger = (
  descriptor: DesktopTargetBuildDescriptor,
  components: ReadonlyArray<NativeComponentLedgerEntry>,
  metadata: StagedClosureMetadata,
): NativeComponentLedger =>
  decodeNativeComponentLedger({
    schema: NATIVE_COMPONENT_LEDGER_SCHEMA_ID,
    phase: "pre-maker-staging",
    targetKey: descriptor.targetKey,
    channel: descriptor.channel,
    version: descriptor.version,
    sourceRevision: descriptor.sourceRevision,
    lockfileSha256: metadata.lockfileSha256,
    osImage: metadata.osImage,
    toolchain: metadata.toolchain,
    plannedMakerIdentities: makerIdentityRefs(descriptor.targetKey, metadata.toolchain.forge),
    packageContentAllowlist: "pass",
    plannedArtifacts: descriptor.formats.map((format) => ({
      name: desktopReleaseSetArtifactName({
        version: descriptor.version,
        channel: descriptor.channel,
        targetKey: descriptor.targetKey,
        format,
      }),
      format,
    })),
    components: [...components].sort((a, b) => a.destination.localeCompare(b.destination)),
  });

export interface DesktopBuildReceiptDraft {
  readonly descriptor: DesktopTargetBuildDescriptor;
  readonly componentLedger: { readonly sha256: string; readonly componentCount: number };
  readonly toolchain: DesktopBuildToolchain;
  readonly gates: { readonly stagedTree: "pass" };
  readonly worker: { readonly workerRef: string; readonly hostClass: string };
}

/**
 * Post-make receipt finalization: the maker/worker supplies the produced
 * artifact identities and PROOF that the live post-package asar gate ran
 * green; the schema enforces canonical version-first names, complete format
 * coverage, and structural refusal of unsigned-dev output.
 */
export const finalizeDesktopBuildReceipt = (
  draft: DesktopBuildReceiptDraft,
  artifacts: ReadonlyArray<DesktopBuildArtifactInput>,
  completedAt: string,
  asarAllowlistGate: "pass",
): DesktopBuildReceipt =>
  decodeDesktopBuildReceipt({
    schema: BUILD_RECEIPT_SCHEMA_ID,
    descriptor: draft.descriptor,
    componentLedger: draft.componentLedger,
    toolchain: draft.toolchain,
    gates: { stagedTree: draft.gates.stagedTree, asarAllowlist: asarAllowlistGate },
    artifacts,
    worker: draft.worker,
    completedAt,
  });

// ---------------------------------------------------------------------------
// Staging orchestration — every effect is injected
// ---------------------------------------------------------------------------

export interface MaterializedRuntimePackage {
  readonly available: boolean;
  readonly version?: string;
}

export interface StageTargetIo {
  /** Creates a fresh, empty staging workspace and returns its absolute path. */
  readonly createWorkspace: (prefix: string) => Promise<string>;
  /**
   * Exports the EXACT source revision into the staging workspace (never a
   * live checkout copy) and returns the exported source root.
   */
  readonly exportSource: (workspace: string, sourceRevision: string) => Promise<string>;
  /**
   * The EXPORTED desktop manifest source (`apps/openagents-desktop/
   * package.json` at descriptor.sourceRevision). Runtime and toolchain pins
   * derive from THIS text — never the live checkout's manifest — so a
   * staging run can never mix revision A's source with revision B's pins.
   */
  readonly readDesktopSourceManifest: (sourceRoot: string) => Promise<string>;
  /** sha256 of the exported source's immutable pnpm lockfile. */
  readonly lockfileSha256: (sourceRoot: string) => Promise<string>;
  /**
   * EXECUTES the staging plan's locked, target-only production install inside
   * the exported source (pnpm, frozen lockfile, target supportedArchitectures).
   */
  readonly runTargetProductionInstall: (
    sourceRoot: string,
    plan: TargetStagingPlan,
  ) => Promise<void>;
  /**
   * Copies the exact locked runtime package for the target FROM THE STAGING
   * PRODUCTION INSTALL into the staged tree at `node_modules/<name>` and
   * returns its identity, or reports it unavailable. MUST NOT fall back to
   * the developer checkout, shared node_modules, or host-global installs.
   */
  readonly materializeRuntimePackage: (
    workspace: string,
    sourceRoot: string,
    pkg: RequiredRuntimePackage,
  ) => Promise<MaterializedRuntimePackage>;
  /**
   * Builds the application bundle from the exported source and stages it
   * (dist/, package.json, packaging resources) into the staged tree.
   */
  readonly buildApplication: (
    workspace: string,
    sourceRoot: string,
    descriptor: DesktopTargetBuildDescriptor,
  ) => Promise<void>;
  /**
   * Builds the owned native helper with the plan's EXPLICIT target triple,
   * with build output INSIDE the staging workspace, stages it at the plan
   * destination, and returns its identity.
   */
  readonly buildNativeHelper: (
    workspace: string,
    sourceRoot: string,
    plan: TargetStagingPlan,
  ) => Promise<{ readonly sha256: string; readonly byteLength: number; readonly version: string }>;
  /** Lists the staged tree for the oracle and the per-file ledger closure. */
  readonly collectStagedFiles: (workspace: string) => Promise<ReadonlyArray<StagedFile>>;
  /**
   * The §9 toolchain identity for this staging run: Electron/Forge pins from
   * the EXPORTED source manifest (revision-exact) plus the ACTUAL invoked
   * node/pnpm/rustc/compiler versions probed on this worker.
   */
  readonly toolchainIdentity: (sourceRoot: string) => Promise<DesktopBuildToolchain>;
  readonly repoRoot: string;
  readonly osImage: string;
  readonly worker: { readonly workerRef: string; readonly hostClass: string };
}

export type StageTargetResult =
  | {
      readonly ok: true;
      readonly workspace: string;
      readonly stagedTree: string;
      readonly ledger: NativeComponentLedger;
      readonly ledgerDigest: string;
      readonly ledgerRef: string;
      /** Present only for production signing policy — never for unsigned-dev. */
      readonly receiptDraft?: DesktopBuildReceiptDraft;
      /** Conspicuous marker mirrored from the descriptor for dev staging. */
      readonly unsignedDev: boolean;
    }
  | {
      readonly ok: false;
      readonly failure:
        | "missing_runtime_package"
        | "runtime_version_mismatch"
        | "staged_tree_violations"
        | "lockfile_mismatch";
      /** Present whenever a workspace was created, so callers can clean up. */
      readonly workspace?: string;
      readonly missingPackages?: ReadonlyArray<string>;
      readonly versionMismatches?: ReadonlyArray<string>;
      readonly violations?: ReadonlyArray<StagingViolation>;
      readonly detail?: string;
    };

/** The staged (packaged-source) tree inside a staging workspace. */
export const stagedTreePath = (workspace: string): string => path.join(workspace, "staged");

export const stageTarget = async (
  descriptor: DesktopTargetBuildDescriptor,
  io: StageTargetIo,
): Promise<StageTargetResult> => {
  const workspace = await io.createWorkspace(`oa-desktop-stage-${descriptor.targetKey}-`);
  const sourceRoot = await io.exportSource(workspace, descriptor.sourceRevision);

  // The immutable lockfile is the only dependency-resolution authority.
  const lockfileSha256 = await io.lockfileSha256(sourceRoot);
  if (lockfileSha256 !== descriptor.lockfileSha256) {
    return {
      ok: false,
      failure: "lockfile_mismatch",
      workspace,
      detail: `exported source lockfile ${lockfileSha256} does not match descriptor ${descriptor.lockfileSha256}`,
    };
  }

  // Runtime pins come from the EXPORTED source at descriptor.sourceRevision —
  // never the live checkout — so plan and source share ONE revision identity.
  const pins = readDesktopManifestPins(await io.readDesktopSourceManifest(sourceRoot));
  const plan = stagingPlanForDescriptor(descriptor, pins);

  // EXECUTE the locked, target-only production install, then materialize the
  // runtime closure from it. Runtime availability AND exact locked version
  // identity gate EVERYTHING: fail typed before the app build, native
  // builds, and any maker/signing work.
  await io.runTargetProductionInstall(sourceRoot, plan);
  const materialized: Array<{ pkg: RequiredRuntimePackage; result: MaterializedRuntimePackage }> =
    [];
  for (const pkg of plan.runtimePackages) {
    materialized.push({
      pkg,
      result: await io.materializeRuntimePackage(workspace, sourceRoot, pkg),
    });
  }
  const missing = materialized.filter((entry) => !entry.result.available);
  if (missing.length > 0) {
    return {
      ok: false,
      failure: "missing_runtime_package",
      workspace,
      missingPackages: missing.map((entry) => `${entry.pkg.name}@${entry.pkg.version}`),
    };
  }
  const mismatched = materialized.filter(
    (entry) => entry.result.version !== entry.pkg.version,
  );
  if (mismatched.length > 0) {
    return {
      ok: false,
      failure: "runtime_version_mismatch",
      workspace,
      versionMismatches: mismatched.map(
        (entry) =>
          `${entry.pkg.name}: staged ${entry.result.version ?? "unknown"}, locked ${entry.pkg.version}`,
      ),
    };
  }

  await io.buildApplication(workspace, sourceRoot, descriptor);
  const helper = await io.buildNativeHelper(workspace, sourceRoot, plan);

  const files = await io.collectStagedFiles(workspace);
  const violations = stagedTreeViolations({
    descriptor,
    files,
    runtimePackages: plan.runtimePackages,
    repoRoot: io.repoRoot,
    forbiddenPathPrefixes: [workspace],
  });
  if (violations.length > 0) {
    return { ok: false, failure: "staged_tree_violations", workspace, violations };
  }

  const packageVersions = new Map<string, string>(
    materialized.map(({ pkg, result }) => [pkg.name, result.version ?? pkg.version]),
  );
  const toolchain = await io.toolchainIdentity(sourceRoot);
  const ledger = buildNativeComponentLedger(
    descriptor,
    nativeClosureEntries(descriptor, files, packageVersions, helper.version),
    { lockfileSha256, osImage: io.osImage, toolchain },
  );
  const ledgerDigest = nativeComponentLedgerDigest(ledger);
  const unsignedDev = descriptor.signingPolicy === "unsigned-dev";
  return {
    ok: true,
    workspace,
    stagedTree: stagedTreePath(workspace),
    ledger,
    ledgerDigest,
    ledgerRef: nativeComponentLedgerRef(ledger),
    // Unsigned-dev output is structurally inadmissible to publication: it
    // never receives a receipt draft, so no DesktopBuildReceipt can exist.
    ...(unsignedDev
      ? {}
      : {
          receiptDraft: {
            descriptor,
            componentLedger: { sha256: ledgerDigest, componentCount: ledger.components.length },
            toolchain,
            gates: { stagedTree: "pass" as const },
            worker: io.worker,
          },
        }),
    unsignedDev,
  };
};

// ---------------------------------------------------------------------------
// Ledger/staged-tree binding + post-package live ASAR gate (forge.config.ts)
// ---------------------------------------------------------------------------

export interface VerifiedStagedLedger {
  readonly ledger: NativeComponentLedger;
  readonly ledgerDigest: string;
  readonly ledgerRef: string;
}

/**
 * Binds a staging workspace's ledger to the descriptor and to the CURRENT
 * staged bytes (review blocker 3): decodes/validates ledger.json, checks
 * every descriptor identity field, recomputes the canonical ledger digest,
 * and re-hashes every closure component on disk against its recorded
 * sha256/byteLength. A staging workspace mutated after staging can therefore
 * never reach a maker or produce a receipt referencing stale proof.
 */
export const verifyStagedTreeAgainstLedger = async (
  stagedTree: string,
  descriptor: DesktopTargetBuildDescriptor,
  ledgerJson: unknown,
): Promise<VerifiedStagedLedger> => {
  const ledger = decodeNativeComponentLedger(ledgerJson);
  const bindings: ReadonlyArray<[string, string, string]> = [
    ["targetKey", ledger.targetKey, descriptor.targetKey],
    ["channel", ledger.channel, descriptor.channel],
    ["version", ledger.version, descriptor.version],
    ["sourceRevision", ledger.sourceRevision, descriptor.sourceRevision],
    ["lockfileSha256", ledger.lockfileSha256, descriptor.lockfileSha256],
  ];
  for (const [field, ledgerValue, descriptorValue] of bindings) {
    if (ledgerValue !== descriptorValue) {
      throw new Error(
        `staged ledger REFUSED: ${field} ${ledgerValue} does not match descriptor ${descriptorValue}`,
      );
    }
  }
  for (const component of ledger.components) {
    let bytes: Uint8Array;
    try {
      bytes = await readFile(path.join(stagedTree, ...component.destination.split("/")));
    } catch {
      throw new Error(
        `staged ledger REFUSED: component ${component.destination} is missing from the staged tree`,
      );
    }
    if (bytes.byteLength !== component.byteLength) {
      throw new Error(
        `staged ledger REFUSED: component ${component.destination} byte length changed since staging`,
      );
    }
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (digest !== component.sha256) {
      throw new Error(
        `staged ledger REFUSED: component ${component.destination} bytes changed since staging`,
      );
    }
  }
  const ledgerDigest = nativeComponentLedgerDigest(ledger);
  return { ledger, ledgerDigest, ledgerRef: `sha256:${ledgerDigest}` };
};

interface AsarHeaderDirectory {
  readonly files: Record<string, AsarHeaderEntry>;
}
type AsarHeaderEntry =
  | AsarHeaderDirectory
  | { readonly size?: number; readonly link?: string; readonly unpacked?: boolean };

export interface RealAsarEntry {
  readonly path: string;
  readonly unpacked: boolean;
}

/**
 * Lists the REAL file entries inside a built app.asar via the asar raw
 * header, PRESERVING per-entry packed/unpacked placement state.
 */
export const realAsarEntries = (asarPath: string): ReadonlyArray<RealAsarEntry> => {
  const requireFromApp = createRequire(path.join(appRoot, "package.json"));
  const asar = requireFromApp("@electron/asar") as {
    getRawHeader: (archive: string) => { header: AsarHeaderDirectory };
  };
  const { header } = asar.getRawHeader(asarPath);
  const entries: Array<RealAsarEntry> = [];
  const walk = (directory: AsarHeaderDirectory, prefix: string): void => {
    for (const [name, entry] of Object.entries(directory.files)) {
      const entryPath = prefix === "" ? name : `${prefix}/${name}`;
      if ("files" in entry) walk(entry as AsarHeaderDirectory, entryPath);
      else entries.push({ path: entryPath, unpacked: entry.unpacked === true });
    }
  };
  walk(header, "");
  return entries.sort((a, b) => a.path.localeCompare(b.path));
};

/** Resources-relative destination of an extra-resource closure entry. */
export const extraResourceDestination = (componentDestination: string): string =>
  componentDestination.replace(/^dist\/builtin-skills\//, "builtin-skills/");

/**
 * PURE placement-fidelity comparison (review blocker 4): every ledger
 * closure entry's ACTUAL packed/unpacked/extraResource state must match its
 * planned placement. A required-unpacked runtime executable that ends up
 * packed inside app.asar fails, as does an extra-resource file that leaks
 * into the archive or a missing planned entry.
 */
export const asarPlacementViolations = (
  ledger: NativeComponentLedger,
  entries: ReadonlyArray<RealAsarEntry>,
  extraResourcePaths: ReadonlySet<string>,
): ReadonlyArray<StagingViolation> => {
  const violations: Array<StagingViolation> = [];
  const byPath = new Map(entries.map((entry) => [entry.path, entry]));
  for (const component of ledger.components) {
    const actual = byPath.get(component.destination);
    if (component.asarPlacement === "extra-resource") {
      if (actual !== undefined) {
        violations.push({
          kind: "unexpected_asar_entry",
          path: component.destination,
          detail: "planned extra-resource component leaked into app.asar",
        });
      }
      if (!extraResourcePaths.has(extraResourceDestination(component.destination))) {
        violations.push({
          kind: "unexpected_asar_entry",
          path: component.destination,
          detail: "planned extra-resource component is absent from Resources",
        });
      }
      continue;
    }
    if (actual === undefined) {
      violations.push({
        kind: "unexpected_asar_entry",
        path: component.destination,
        detail: `planned ${component.asarPlacement} component is absent from the packaged app`,
      });
      continue;
    }
    if (component.asarPlacement === "unpacked" && !actual.unpacked) {
      violations.push({
        kind: "unexpected_asar_entry",
        path: component.destination,
        detail: "planned-unpacked component was packed inside app.asar",
      });
    }
    if (component.asarPlacement === "asar" && actual.unpacked) {
      violations.push({
        kind: "unexpected_asar_entry",
        path: component.destination,
        detail: "planned-packed component was unpacked beside app.asar",
      });
    }
  }
  return violations;
};

export interface PackagedAsarGateInput {
  readonly descriptor: DesktopTargetBuildDescriptor;
  readonly ledger: NativeComponentLedger;
  readonly stagedTree: string;
  readonly asarPath: string;
  /** The packaged Resources directory (extraResource destination root). */
  readonly resourcesPath: string;
  readonly repoRoot: string;
}

/** Present extra-resource destinations for the ledger's closure entries. */
const presentExtraResourcePaths = async (
  resourcesPath: string,
  ledger: NativeComponentLedger,
): Promise<ReadonlySet<string>> => {
  const present = new Set<string>();
  for (const component of ledger.components) {
    if (component.asarPlacement !== "extra-resource") continue;
    const destination = extraResourceDestination(component.destination);
    try {
      await lstat(path.join(resourcesPath, ...destination.split("/")));
      present.add(destination);
    } catch {
      // absent — asarPlacementViolations reports it
    }
  }
  return present;
};

/**
 * The LIVE post-package gate (review blockers 3 and 4): re-audits the staged
 * tree together with the REAL entry list of the just-built app.asar through
 * `stagedTreeViolations`, verifies per-closure-entry placement fidelity
 * (packed vs unpacked vs extraResource), and re-hashes the PACKAGED bytes of
 * every unpacked/extra-resource closure component against the ledger. One
 * unexpected entry, placement drift, or byte drift fails the build before
 * any maker or signing work. Returns the gate receipt on success.
 */
export const assertPackagedAsarAdmissible = async (
  input: PackagedAsarGateInput,
): Promise<{
  readonly asarEntryCount: number;
  readonly unpackedEntryCount: number;
  readonly verifiedComponents: number;
  readonly result: "pass";
}> => {
  const stagedManifest = await readFile(path.join(input.stagedTree, "package.json"), "utf8");
  const pins = readDesktopManifestPins(stagedManifest);
  const runtimePackages = requiredRuntimePackages(input.descriptor, pins);
  const files = await collectStagedTreeFiles(input.stagedTree);
  const entries = realAsarEntries(input.asarPath);
  const violations = [
    ...stagedTreeViolations({
      descriptor: input.descriptor,
      files,
      runtimePackages,
      repoRoot: input.repoRoot,
      asarEntries: entries.map((entry) => entry.path),
    }),
    ...asarPlacementViolations(
      input.ledger,
      entries,
      await presentExtraResourcePaths(input.resourcesPath, input.ledger),
    ),
  ];
  if (violations.length > 0) {
    throw new Error(
      `packaged ASAR REFUSED (${violations.length} violation${violations.length === 1 ? "" : "s"}): ` +
        violations
          .slice(0, 10)
          .map((violation) => `${violation.kind}:${violation.path} (${violation.detail})`)
          .join(", "),
    );
  }
  // Byte fidelity of the PACKAGED closure: unpacked and extra-resource
  // components run as real files — their shipped bytes must equal the
  // ledgered staging bytes.
  let verifiedComponents = 0;
  for (const component of input.ledger.components) {
    const packagedPath =
      component.asarPlacement === "unpacked"
        ? path.join(`${input.asarPath}.unpacked`, ...component.destination.split("/"))
        : component.asarPlacement === "extra-resource"
          ? path.join(
              input.resourcesPath,
              ...extraResourceDestination(component.destination).split("/"),
            )
          : null;
    if (packagedPath === null) continue;
    const bytes = await readFile(packagedPath);
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (digest !== component.sha256 || bytes.byteLength !== component.byteLength) {
      throw new Error(
        `packaged ASAR REFUSED: shipped component ${component.destination} does not match the staged ledger bytes`,
      );
    }
    verifiedComponents += 1;
  }
  return {
    asarEntryCount: entries.length,
    unpackedEntryCount: entries.filter((entry) => entry.unpacked).length,
    verifiedComponents,
    result: "pass",
  };
};

// ---------------------------------------------------------------------------
// Real IO (host implementation) + CLI
// ---------------------------------------------------------------------------

const sha256Hex = (bytes: Uint8Array | string): string =>
  createHash("sha256").update(bytes).digest("hex");

/** Child stdout is routed to stderr so CLI stdout stays machine-readable. */
const run = (
  command: string,
  args: ReadonlyArray<string>,
  options: { cwd: string; env?: Record<string, string | undefined> },
): void => {
  execFileSync(command, [...args], {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: ["ignore", process.stderr, "inherit"],
  });
};

const capture = (command: string, args: ReadonlyArray<string>, cwd: string): string =>
  execFileSync(command, [...args], { cwd, encoding: "utf8" }).trim();

const HEADER_SAMPLE_BYTES = 65_536;
const TEXT_SCAN_MAX_BYTES = 32 * 1024 * 1024;

/** Host staged-tree collection: lstat-first so symlinks are RECORDED. */
export const collectStagedTreeFiles = async (
  stagedTree: string,
): Promise<ReadonlyArray<StagedFile>> => {
  const files: Array<StagedFile> = [];
  const walk = async (relative: string): Promise<void> => {
    const absoluteDir = relative === "" ? stagedTree : path.join(stagedTree, relative);
    for (const entry of await readdir(absoluteDir, { withFileTypes: true })) {
      const entryPath = relative === "" ? entry.name : `${relative}/${entry.name}`;
      const absolute = path.join(stagedTree, ...entryPath.split("/"));
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) {
        files.push({
          path: entryPath,
          byteLength: info.size,
          executable: false,
          header: new Uint8Array(0),
          symlinkTarget: await readlink(absolute),
        });
        continue;
      }
      if (info.isDirectory()) {
        await walk(entryPath);
        continue;
      }
      const bytes = await readFile(absolute);
      const header = new Uint8Array(bytes.subarray(0, HEADER_SAMPLE_BYTES));
      const looksBinary = header.subarray(0, 8_192).includes(0);
      files.push({
        path: entryPath,
        byteLength: info.size,
        executable: (info.mode & 0o111) !== 0,
        header,
        sha256: sha256Hex(bytes),
        ...(looksBinary || bytes.byteLength > TEXT_SCAN_MAX_BYTES
          ? {}
          : { content: bytes.toString("utf8") }),
      });
    }
  };
  await walk("");
  return files;
};

const patchSupportedArchitectures = async (
  sourceRoot: string,
  plan: TargetStagingPlan,
): Promise<string> => {
  const workspaceManifestPath = path.join(sourceRoot, "pnpm-workspace.yaml");
  const original = await readFile(workspaceManifestPath, "utf8");
  const block =
    `supportedArchitectures:\n` +
    `  cpu: [${plan.install.supportedArchitectures.cpu.join(", ")}]\n` +
    `  libc: [${plan.install.supportedArchitectures.libc.join(", ")}]\n` +
    `  os: [${plan.install.supportedArchitectures.os.join(", ")}]\n`;
  const pattern = /supportedArchitectures:\n(?: {2}.*\n)+/;
  const patched = pattern.test(original)
    ? original.replace(pattern, block)
    : `${original}\n${block}`;
  await writeFile(workspaceManifestPath, patched, "utf8");
  return original;
};

export const hostStageTargetIo = (workerRef: string): StageTargetIo => {
  const probe = (command: string, args: ReadonlyArray<string>): string => {
    try {
      return capture(command, args, checkoutRoot).split("\n")[0]!.trim();
    } catch {
      return "unavailable";
    }
  };
  let originalWorkspaceManifest: string | null = null;
  return {
    createWorkspace: async (prefix) => {
      const workspace = mkdtempSync(path.join(os.tmpdir(), prefix));
      await mkdir(path.join(stagedTreePath(workspace), "node_modules"), { recursive: true });
      return workspace;
    },
    exportSource: async (workspace, sourceRevision) => {
      const sourceRoot = path.join(workspace, "source");
      await mkdir(sourceRoot, { recursive: true });
      const tarPath = path.join(workspace, "source.tar");
      run("git", ["-C", checkoutRoot, "archive", "--format=tar", "-o", tarPath, sourceRevision], {
        cwd: checkoutRoot,
      });
      run("tar", ["-xf", tarPath, "-C", sourceRoot], { cwd: workspace });
      await rm(tarPath, { force: true });
      return sourceRoot;
    },
    readDesktopSourceManifest: async (sourceRoot) =>
      readFile(path.join(sourceRoot, "apps", "openagents-desktop", "package.json"), "utf8"),
    lockfileSha256: async (sourceRoot) =>
      sha256Hex(await readFile(path.join(sourceRoot, "pnpm-lock.yaml"))),
    runTargetProductionInstall: async (sourceRoot, plan) => {
      // Pin pnpm's optional-dependency selection to the TARGET before the
      // locked production install; the original manifest is restored before
      // the host-architecture build-environment install.
      originalWorkspaceManifest = await patchSupportedArchitectures(sourceRoot, plan);
      run(plan.install.command, ["--dir", sourceRoot, ...plan.install.args], {
        cwd: sourceRoot,
        env: { ...plan.install.env, CI: "true" },
      });
    },
    materializeRuntimePackage: async (workspace, sourceRoot, pkg) => {
      // Resolution is bounded to the staging workspace's fresh, locked,
      // target-only production install; the developer checkout, shared
      // node_modules, and host-global installs can never satisfy this seam.
      const stagedSourceApp = path.join(sourceRoot, "apps", "openagents-desktop");
      const resolveFromApp = createRequire(path.join(stagedSourceApp, "package.json"));
      let packageRoot: string;
      try {
        if (pkg.name === "@openai/codex") {
          packageRoot = path.dirname(
            path.dirname(resolveFromApp.resolve("@openai/codex/bin/codex.js")),
          );
        } else if (pkg.name.startsWith("@anthropic-ai/claude-agent-sdk-")) {
          const resolveFromSdk = createRequire(
            resolveFromApp.resolve("@anthropic-ai/claude-agent-sdk"),
          );
          packageRoot = path.dirname(resolveFromSdk.resolve(`${pkg.name}/package.json`));
        } else if (pkg.name === "@anthropic-ai/claude-agent-sdk") {
          // The SDK does not export ./package.json; resolve its entry and take
          // the installed package directory.
          packageRoot = path.dirname(resolveFromApp.resolve(pkg.name));
        } else if (pkg.name.startsWith("@openai/codex-")) {
          // Platform packages are optionalDependencies of @openai/codex;
          // resolve them from the codex package's own dependency context.
          const resolveFromCodex = createRequire(
            resolveFromApp.resolve("@openai/codex/bin/codex.js"),
          );
          packageRoot = path.dirname(resolveFromCodex.resolve(`${pkg.name}/package.json`));
        } else {
          packageRoot = path.dirname(resolveFromApp.resolve(`${pkg.name}/package.json`));
        }
      } catch {
        return { available: false };
      }
      const destination = path.join(
        stagedTreePath(workspace),
        "node_modules",
        ...pkg.name.split("/"),
      );
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(packageRoot, destination, {
        recursive: true,
        dereference: true,
        // `.bin` shim directories are host-install conveniences, not runtime
        // payload; excluding them keeps the closure minimal and store-path-free.
        filter: (source) => path.basename(source) !== ".bin",
      });
      const manifest = JSON.parse(
        await readFile(path.join(destination, "package.json"), "utf8"),
      ) as { version: string };
      return { available: true, version: manifest.version };
    },
    buildApplication: async (workspace, sourceRoot, descriptor) => {
      const stagedSourceApp = path.join(sourceRoot, "apps", "openagents-desktop");
      const archivedManifest = JSON.parse(
        await readFile(path.join(stagedSourceApp, "package.json"), "utf8"),
      ) as { version: string };
      if (archivedManifest.version !== descriptor.version) {
        throw new Error(
          `descriptor version ${descriptor.version} does not match source revision version ${archivedManifest.version}`,
        );
      }
      // Restore the original (host) architecture manifest for the build-
      // environment install: build tooling runs on the host; the PACKAGED
      // node_modules were already materialized from the target-only install.
      if (originalWorkspaceManifest !== null) {
        await writeFile(
          path.join(sourceRoot, "pnpm-workspace.yaml"),
          originalWorkspaceManifest,
          "utf8",
        );
      }
      run(
        "pnpm",
        [
          "--dir",
          sourceRoot,
          "install",
          "--frozen-lockfile",
          "--ignore-scripts",
          "--prefer-offline",
          "--filter",
          `${DESKTOP_PACKAGE_FILTER}...`,
        ],
        { cwd: sourceRoot, env: { CI: "true" } },
      );
      // The development build script may stage a host-arch DEBUG helper; the
      // skip env avoids that work on revisions that support it, and any
      // legacy cargo output lands under <source>/target — still inside the
      // staging workspace, never the developer checkout's shared target dir.
      run("node", ["--import", "tsx", "scripts/build.ts"], {
        cwd: stagedSourceApp,
        env: { OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1" },
      });
      const staged = stagedTreePath(workspace);
      await cp(path.join(stagedSourceApp, "dist"), path.join(staged, "dist"), {
        recursive: true,
        dereference: true,
        // The dev build may stage a host-arch helper under dist/native; the
        // staged closure carries only the explicit-triple release helper at
        // the plan's native/<arch> destination.
        filter: (source) => !source.startsWith(path.join(stagedSourceApp, "dist", "native")),
      });
      await cp(path.join(stagedSourceApp, "package.json"), path.join(staged, "package.json"));
      // Packaging inputs (icon, signing entitlements) come from the exported
      // source revision, not the developer checkout.
      await mkdir(path.join(staged, "resources"), { recursive: true });
      await cp(
        path.join(stagedSourceApp, "resources", "openagents-icon.icns"),
        path.join(staged, "resources", "openagents-icon.icns"),
      );
      await mkdir(path.join(staged, "build"), { recursive: true });
      for (const entitlements of ["entitlements.mac.plist", "entitlements.mac.inherit.plist"]) {
        await cp(
          path.join(stagedSourceApp, "build", entitlements),
          path.join(staged, "build", entitlements),
        );
      }
    },
    buildNativeHelper: async (workspace, sourceRoot, plan) => {
      const cargoTarget = path.join(workspace, "cargo-target");
      run("cargo", [...plan.cargo.args], {
        cwd: sourceRoot,
        env: {
          CARGO_TARGET_DIR: cargoTarget,
          // Cross-run ledger determinism: strip the per-run staging workspace
          // prefix from panic/debug paths embedded in the release binary.
          RUSTFLAGS: `--remap-path-prefix=${workspace}=/oa-staging`,
        },
      });
      const built = path.join(cargoTarget, ...plan.cargo.outputRelativePath.split("/"));
      const destination = path.join(
        stagedTreePath(workspace),
        ...plan.nativeHelperDestination.split("/"),
      );
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(built, destination);
      await chmod(destination, 0o755);
      const helperCargoManifest = await readFile(
        path.join(sourceRoot, "crates", "oa-desktop-audio", "Cargo.toml"),
        "utf8",
      );
      const version = /^version\s*=\s*"([^"]+)"/m.exec(helperCargoManifest)?.[1] ?? "0.0.0";
      const bytes = await readFile(destination);
      // The packaged runtime reads resources/native/<arch>/manifest.json.
      await writeFile(
        path.join(path.dirname(destination), "manifest.json"),
        `${JSON.stringify({
          protocolVersion: 1,
          helperVersion: version,
          architecture: desktopTargets[plan.targetKey].arch,
          sha256: sha256Hex(bytes),
        })}\n`,
        { mode: 0o644 },
      );
      return { version, sha256: sha256Hex(bytes), byteLength: bytes.byteLength };
    },
    collectStagedFiles: async (workspace) => collectStagedTreeFiles(stagedTreePath(workspace)),
    toolchainIdentity: async (sourceRoot) => {
      // Electron/Forge identities are the versions the staging workspace's
      // OWN frozen-lockfile install resolved at descriptor.sourceRevision —
      // never the live checkout's manifest or node_modules. The remaining
      // entries are the ACTUAL tool versions this staging run invoked
      // (node/pnpm for installs+build, rustc/cc for the native helper),
      // probed on this worker.
      const stagedInstalledVersion = async (packageName: string): Promise<string> => {
        const manifest = JSON.parse(
          await readFile(
            path.join(
              sourceRoot,
              "apps",
              "openagents-desktop",
              "node_modules",
              ...packageName.split("/"),
              "package.json",
            ),
            "utf8",
          ),
        ) as { version: string };
        return manifest.version;
      };
      return {
        electron: await stagedInstalledVersion("electron"),
        node: process.version.replace(/^v/, ""),
        pnpm: probe("pnpm", ["--version"]),
        forge: await stagedInstalledVersion("@electron-forge/cli"),
        rust: probe("rustc", ["--version"]),
        compiler: probe("cc", ["--version"]),
      };
    },
    repoRoot: checkoutRoot,
    osImage: `${process.platform}-${os.arch()}-${os.release()}`,
    worker: { workerRef, hostClass: `local-${process.platform}-${process.arch}` },
  };
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const parseCliDescriptor = (argv: ReadonlyArray<string>): DesktopTargetBuildDescriptor => {
  const flag = (name: string): string | undefined => {
    const index = argv.indexOf(`--${name}`);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  const targetKey = flag("target");
  if (targetKey === undefined || !(targetKey in desktopTargets)) {
    throw new Error(
      "stage-target REQUIRES an explicit target descriptor: --target <darwin-arm64|darwin-x64|win32-arm64|win32-x64|linux-arm64|linux-x64> " +
        "[--auto | --channel <stable|rc> --version <semver> --source-revision <sha> --lockfile-sha256 <hex>] [--unsigned-dev] [--plan]",
    );
  }
  const auto = argv.includes("--auto");
  const sourceRevision =
    flag("source-revision") ?? (auto ? capture("git", ["rev-parse", "HEAD"], checkoutRoot) : undefined);
  let version = flag("version");
  let lockfileSha256 = flag("lockfile-sha256");
  if (auto && sourceRevision !== undefined) {
    version ??= (
      JSON.parse(
        capture(
          "git",
          ["show", `${sourceRevision}:apps/openagents-desktop/package.json`],
          checkoutRoot,
        ),
      ) as { version: string }
    ).version;
    lockfileSha256 ??= sha256Hex(
      execFileSync("git", ["show", `${sourceRevision}:pnpm-lock.yaml`], {
        cwd: checkoutRoot,
        maxBuffer: 512 * 1024 * 1024,
      }),
    );
  }
  const channel = flag("channel") ?? (version?.includes("-rc.") ? "rc" : "stable");
  return decodeDesktopTargetBuildDescriptor({
    schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
    product: "OpenAgents",
    targetKey,
    channel,
    version,
    sourceRevision,
    lockfileSha256,
    formats: [...desktopTargets[targetKey as DesktopTargetKey].requiredFormats],
    signingPolicy: argv.includes("--unsigned-dev") ? "unsigned-dev" : "production",
  });
};

/** Persists descriptor.json + ledger.json into a successful workspace. */
export const persistStagingDocuments = async (
  descriptor: DesktopTargetBuildDescriptor,
  result: Extract<StageTargetResult, { ok: true }>,
): Promise<void> => {
  await writeFile(
    path.join(result.workspace, "descriptor.json"),
    `${JSON.stringify(descriptor, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    path.join(result.workspace, "ledger.json"),
    `${JSON.stringify(result.ledger, null, 2)}\n`,
    "utf8",
  );
};

/**
 * Removes an auto-created staging workspace. Callers keep one ONLY behind an
 * explicit `--retain` (debug/proof runs) — success and failure both clean up
 * by default so temporary workspaces never leak (review blocker 6).
 */
export const cleanupStagingWorkspace = async (workspace: string | undefined): Promise<void> => {
  if (workspace === undefined) return;
  await rm(workspace, { recursive: true, force: true });
};

const direct =
  process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (direct) {
  const descriptor = parseCliDescriptor(process.argv.slice(2));
  if (process.argv.includes("--plan")) {
    // Even the plan projection derives runtime pins from the descriptor's
    // EXACT source revision, never the live checkout manifest.
    const pins = readDesktopManifestPins(
      capture(
        "git",
        ["show", `${descriptor.sourceRevision}:apps/openagents-desktop/package.json`],
        checkoutRoot,
      ),
    );
    process.stdout.write(
      `${JSON.stringify(stagingPlanForDescriptor(descriptor, pins), null, 2)}\n`,
    );
  } else {
    const retain = process.argv.includes("--retain");
    // Track the auto-created workspace independently of the result so
    // cleanup covers typed failures AND thrown errors alike.
    let createdWorkspace: string | undefined;
    const hostIo = hostStageTargetIo("local-stage-cli");
    const io: StageTargetIo = {
      ...hostIo,
      createWorkspace: async (prefix) => {
        createdWorkspace = await hostIo.createWorkspace(prefix);
        return createdWorkspace;
      },
    };
    let retained = false;
    try {
      const result = await stageTarget(descriptor, io);
      if (result.ok) {
        await persistStagingDocuments(descriptor, result);
        retained = retain;
      }
      process.stdout.write(
        `${JSON.stringify(
          result.ok
            ? {
                ok: true,
                workspace: result.workspace,
                stagedTree: result.stagedTree,
                retained,
                ledgerRef: result.ledgerRef,
                componentCount: result.ledger.components.length,
                unsignedDev: result.unsignedDev,
                receiptDraft: result.receiptDraft ?? null,
              }
            : result,
          null,
          2,
        )}\n`,
      );
      if (!result.ok) process.exitCode = 1;
    } finally {
      if (!retained) await cleanupStagingWorkspace(createdWorkspace);
    }
  }
}
