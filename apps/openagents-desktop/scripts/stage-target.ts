/**
 * Target-aware Desktop staging builder (DIST-03, #8916).
 *
 * Replaces host-incidental packaging assumptions with an explicit,
 * descriptor-driven staging flow (audit §10.2, ProductSpec §9):
 *
 *   1. Require an explicit `DesktopTargetBuildDescriptor` — host platform or
 *      architecture inference never selects a target.
 *   2. Create a CLEAN per-target temporary staging workspace; the developer
 *      checkout and another architecture's dependency tree are never
 *      packaged.
 *   3. Resolve the exact provider/native runtime packages for the target
 *      FIRST and fail with a typed `missing_runtime_package` before any
 *      native build, maker, or signing work begins.
 *   4. Build `oa-desktop-audio` with the target's EXPLICIT Rust triple.
 *   5. Copy only allowlisted application resources.
 *   6. Run the staged-tree oracle (architecture, allowlist, dev-file,
 *      source-checkout, ASAR boundary) before any maker may run.
 *   7. Emit the public-safe native-component ledger; production staging also
 *      returns a receipt draft the maker/worker finalizes into a
 *      `DesktopBuildReceipt`. Unsigned-dev staging NEVER yields a receipt.
 *
 * Every decision is a pure function over injected inputs so fixture tests
 * prove target selection for all six targets without six production runners.
 */
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { cp, mkdir, readdir, readFile, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  type DesktopBuildArtifactInput,
  type DesktopBuildReceipt,
  type DesktopTargetBuildDescriptor,
  type DesktopTargetKey,
  type NativeComponentLedger,
  type NativeComponentLedgerEntry,
  decodeDesktopBuildReceipt,
  decodeDesktopTargetBuildDescriptor,
  decodeNativeComponentLedger,
  desktopTargets,
  nativeComponentLedgerDigest,
  nativeComponentLedgerRef,
  NATIVE_COMPONENT_LEDGER_SCHEMA_ID,
  BUILD_RECEIPT_SCHEMA_ID,
  TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
} from "../src/release-staging-contract.ts";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

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
   * Locked production install for the TARGET OS/architecture only. The env
   * overrides pin npm/pnpm platform resolution to the descriptor, so a host
   * of a different architecture can never satisfy the closure implicitly.
   */
  readonly install: {
    readonly args: ReadonlyArray<string>;
    readonly env: Readonly<Record<string, string>>;
  };
  /** Explicit-triple native build for owned components. */
  readonly cargo: {
    readonly args: ReadonlyArray<string>;
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
      args: ["install", "--prod", "--frozen-lockfile", "--ignore-scripts"],
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
        "target",
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
// Executable architecture detection — header truth, never file names
// ---------------------------------------------------------------------------

export interface DetectedExecutable {
  readonly platform: "darwin" | "linux" | "win32";
  readonly arch: "arm64" | "x64" | "universal" | "other";
}

const readUInt16LE = (bytes: Uint8Array, offset: number): number | null =>
  offset + 1 < bytes.length ? bytes[offset]! | (bytes[offset + 1]! << 8) : null;

const readUInt32LE = (bytes: Uint8Array, offset: number): number | null =>
  offset + 3 < bytes.length
    ? (bytes[offset]! | (bytes[offset + 1]! << 8) | (bytes[offset + 2]! << 16)) +
      bytes[offset + 3]! * 0x1000000
    : null;

/**
 * Classifies native executable headers (Mach-O, ELF, PE). Returns null for
 * non-native files (scripts, JS shims, data) — those are governed by the
 * allowlist instead. Universal (fat) Mach-O binaries are reported as
 * `universal`: a single-target closure never admits multi-architecture
 * payloads.
 */
export const detectExecutableArchitecture = (header: Uint8Array): DetectedExecutable | null => {
  if (header.length >= 4) {
    // Mach-O 64-bit little-endian file: cf fa ed fe
    if (header[0] === 0xcf && header[1] === 0xfa && header[2] === 0xed && header[3] === 0xfe) {
      const cputype = readUInt32LE(header, 4);
      const arch = cputype === 0x0100000c ? "arm64" : cputype === 0x01000007 ? "x64" : "other";
      return { platform: "darwin", arch };
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
      const arch = machine === 0xb7 ? "arm64" : machine === 0x3e ? "x64" : "other";
      return { platform: "linux", arch };
    }
    // PE
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
        const arch = machine === 0xaa64 ? "arm64" : machine === 0x8664 ? "x64" : "other";
        return { platform: "win32", arch };
      }
    }
  }
  return null;
};

// ---------------------------------------------------------------------------
// Staged-tree oracle
// ---------------------------------------------------------------------------

export interface StagedFile {
  /** Bundle-relative POSIX path. */
  readonly path: string;
  readonly byteLength: number;
  readonly executable: boolean;
  /** Leading file bytes (enough for header classification). */
  readonly header: Uint8Array;
  /** Optional text content for source-checkout leak scanning. */
  readonly content?: string;
  /** Resolved symlink target when the entry is a symlink. */
  readonly symlinkTarget?: string;
}

export const stagingViolationKinds = [
  "foreign_architecture_binary",
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
 * native header outside this set fails the target, regardless of digest.
 */
export const executableDestinationAllowlist: ReadonlyArray<RegExp> = [
  /^node_modules\/@openai\/codex-(?:darwin|win32|linux)-(?:arm64|x64)\/vendor\/[^/]+\/(?:bin\/(?:codex|codex-code-mode-host)(?:\.exe)?|codex-path\/rg(?:\.exe)?|codex-resources\/zsh\/bin\/zsh)$/,
  /^node_modules\/@anthropic-ai\/claude-agent-sdk-(?:darwin|win32|linux)-(?:arm64|x64)(?:-musl)?\/claude(?:\.exe)?$/,
  /^native\/(?:arm64|x64)\/oa-desktop-audio(?:\.exe)?$/,
];

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
  /** Optional app.asar entry listing (bundle-relative POSIX paths). */
  readonly asarEntries?: ReadonlyArray<string>;
}

/**
 * The package-content oracle over a staged tree. Deterministic, pure, and
 * fail-closed: ONE injected foreign binary, missing runtime package,
 * source-checkout dependency, development file, or unexpected ASAR entry
 * yields a violation, and any violation blocks maker/signing work.
 */
export const stagedTreeViolations = (
  input: StagedTreeAuditInput,
): ReadonlyArray<StagingViolation> => {
  const violations: Array<StagingViolation> = [];
  const definition = desktopTargets[input.descriptor.targetKey];

  for (const file of input.files) {
    if (developmentFilePatterns.some((pattern) => pattern.test(file.path))) {
      violations.push({
        kind: "development_file",
        path: file.path,
        detail: "development/source file staged into the bundle",
      });
      continue;
    }
    const detected = detectExecutableArchitecture(file.header);
    if (detected !== null) {
      if (detected.platform !== definition.platform || detected.arch !== definition.arch) {
        violations.push({
          kind: "foreign_architecture_binary",
          path: file.path,
          detail: `expected ${definition.platform}-${definition.arch}, found ${detected.platform}-${detected.arch}`,
        });
      }
      if (!executableDestinationAllowlist.some((pattern) => pattern.test(file.path))) {
        violations.push({
          kind: "unallowlisted_binary",
          path: file.path,
          detail: "native executable outside the staged-binary allowlist",
        });
      }
    } else if (
      file.executable &&
      !executableDestinationAllowlist.some((p) => p.test(file.path)) &&
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
    if (file.content !== undefined && file.content.includes(input.repoRoot)) {
      violations.push({
        kind: "source_checkout_dependency",
        path: file.path,
        detail: "absolute source-checkout path baked into staged file",
      });
    }
    if (
      file.symlinkTarget !== undefined &&
      (path.isAbsolute(file.symlinkTarget) || file.symlinkTarget.split("/").includes(".."))
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
// Ledger + receipt assembly
// ---------------------------------------------------------------------------

export interface StagedComponentInput {
  readonly name: string;
  readonly version: string;
  readonly sha256: string;
  readonly byteLength: number;
  readonly provenance: NativeComponentLedgerEntry["provenance"];
  readonly destination: string;
}

export const buildNativeComponentLedger = (
  descriptor: DesktopTargetBuildDescriptor,
  components: ReadonlyArray<StagedComponentInput>,
): NativeComponentLedger =>
  decodeNativeComponentLedger({
    schema: NATIVE_COMPONENT_LEDGER_SCHEMA_ID,
    targetKey: descriptor.targetKey,
    channel: descriptor.channel,
    version: descriptor.version,
    sourceRevision: descriptor.sourceRevision,
    components: [...components]
      .sort((a, b) => a.destination.localeCompare(b.destination))
      .map((component) => ({ ...component, targetKey: descriptor.targetKey })),
  });

export interface DesktopBuildReceiptDraft {
  readonly descriptor: DesktopTargetBuildDescriptor;
  readonly componentLedger: { readonly sha256: string; readonly componentCount: number };
  readonly toolchain: { readonly electron: string; readonly node: string; readonly pnpm: string };
  readonly worker: { readonly workerRef: string; readonly hostClass: string };
}

/**
 * Post-make receipt finalization: the maker/worker supplies the produced
 * artifact identities; the schema enforces canonical version-first names,
 * complete format coverage, and structural refusal of unsigned-dev output.
 */
export const finalizeDesktopBuildReceipt = (
  draft: DesktopBuildReceiptDraft,
  artifacts: ReadonlyArray<DesktopBuildArtifactInput>,
  completedAt: string,
): DesktopBuildReceipt =>
  decodeDesktopBuildReceipt({
    schema: BUILD_RECEIPT_SCHEMA_ID,
    descriptor: draft.descriptor,
    componentLedger: draft.componentLedger,
    toolchain: draft.toolchain,
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
  readonly sha256?: string;
  readonly byteLength?: number;
}

export interface StageTargetIo {
  /** Creates a fresh, empty staging workspace and returns its absolute path. */
  readonly createWorkspace: (prefix: string) => Promise<string>;
  /**
   * Copies the exact locked runtime package for the target into the staged
   * workspace at `node_modules/<name>` and returns its identity, or reports
   * it unavailable. MUST NOT fall back to host-global installs.
   */
  readonly materializeRuntimePackage: (
    workspace: string,
    pkg: RequiredRuntimePackage,
  ) => Promise<MaterializedRuntimePackage>;
  /** Builds the application bundle (dist/) into the staged workspace. */
  readonly buildApplication: (workspace: string) => Promise<void>;
  /**
   * Builds the owned native helper with the plan's EXPLICIT target triple
   * and stages it at the plan destination; returns its identity.
   */
  readonly buildNativeHelper: (
    workspace: string,
    plan: TargetStagingPlan,
  ) => Promise<{ readonly sha256: string; readonly byteLength: number; readonly version: string }>;
  /** Lists the staged tree for the oracle. */
  readonly collectStagedFiles: (workspace: string) => Promise<ReadonlyArray<StagedFile>>;
  readonly repoRoot: string;
  readonly toolchain: { readonly electron: string; readonly node: string; readonly pnpm: string };
  readonly worker: { readonly workerRef: string; readonly hostClass: string };
}

export type StageTargetResult =
  | {
      readonly ok: true;
      readonly workspace: string;
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
      readonly failure: "missing_runtime_package" | "staged_tree_violations";
      readonly missingPackages?: ReadonlyArray<string>;
      readonly violations?: ReadonlyArray<StagingViolation>;
    };

export const stageTarget = async (
  descriptor: DesktopTargetBuildDescriptor,
  pins: DesktopManifestPins,
  io: StageTargetIo,
): Promise<StageTargetResult> => {
  const plan = stagingPlanForDescriptor(descriptor, pins);
  const workspace = await io.createWorkspace(plan.workspacePrefix);

  // Runtime availability gates EVERYTHING: fail before native builds and
  // before any maker/signing work can start.
  const materialized: Array<{ pkg: RequiredRuntimePackage; result: MaterializedRuntimePackage }> =
    [];
  for (const pkg of plan.runtimePackages) {
    materialized.push({ pkg, result: await io.materializeRuntimePackage(workspace, pkg) });
  }
  const missing = materialized.filter((entry) => !entry.result.available);
  if (missing.length > 0) {
    return {
      ok: false,
      failure: "missing_runtime_package",
      missingPackages: missing.map((entry) => `${entry.pkg.name}@${entry.pkg.version}`),
    };
  }

  await io.buildApplication(workspace);
  const helper = await io.buildNativeHelper(workspace, plan);

  const files = await io.collectStagedFiles(workspace);
  const violations = stagedTreeViolations({
    descriptor,
    files,
    runtimePackages: plan.runtimePackages,
    repoRoot: io.repoRoot,
  });
  if (violations.length > 0) return { ok: false, failure: "staged_tree_violations", violations };

  const ledger = buildNativeComponentLedger(descriptor, [
    {
      name: "oa-desktop-audio",
      version: helper.version,
      sha256: helper.sha256,
      byteLength: helper.byteLength,
      provenance: "workspace-crate",
      destination: plan.nativeHelperDestination,
    },
    ...materialized.map(({ pkg, result }) => ({
      name: pkg.name,
      version: result.version ?? pkg.version,
      sha256: result.sha256!,
      byteLength: result.byteLength!,
      provenance: "locked-dependency" as const,
      destination: `node_modules/${pkg.name}`,
    })),
  ]);
  const ledgerDigest = nativeComponentLedgerDigest(ledger);
  const unsignedDev = descriptor.signingPolicy === "unsigned-dev";
  return {
    ok: true,
    workspace,
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
            toolchain: io.toolchain,
            worker: io.worker,
          },
        }),
    unsignedDev,
  };
};

// ---------------------------------------------------------------------------
// Real IO (host implementation) + CLI
// ---------------------------------------------------------------------------

const sha256File = async (filePath: string): Promise<{ sha256: string; byteLength: number }> => {
  const bytes = await readFile(filePath);
  return {
    sha256: createHash("sha256").update(bytes).digest("hex"),
    byteLength: bytes.byteLength,
  };
};

const listFilesRecursive = async (root: string, relative = ""): Promise<Array<string>> => {
  const entries = await readdir(path.join(root, relative), { withFileTypes: true });
  const files: Array<string> = [];
  for (const entry of entries) {
    const entryPath = relative === "" ? entry.name : `${relative}/${entry.name}`;
    if (entry.isDirectory()) files.push(...(await listFilesRecursive(root, entryPath)));
    else files.push(entryPath);
  }
  return files;
};

export const hostStageTargetIo = (workerRef: string): StageTargetIo => ({
  createWorkspace: async (prefix) => {
    const workspace = mkdtempSync(path.join(tmpdir(), prefix));
    await mkdir(path.join(workspace, "node_modules"), { recursive: true });
    return workspace;
  },
  materializeRuntimePackage: async (workspace, pkg) => {
    // Resolution is bounded to the exact locked application dependency tree;
    // a host-global or NVM install can never satisfy this seam.
    const { createRequire } = await import("node:module");
    const resolveFromApp = createRequire(path.join(appRoot, "package.json"));
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
        // the installed package directory (mirrors forge packageAfterCopy).
        packageRoot = path.dirname(resolveFromApp.resolve(pkg.name));
      } else {
        packageRoot = path.dirname(resolveFromApp.resolve(`${pkg.name}/package.json`));
      }
    } catch {
      return { available: false };
    }
    const destination = path.join(workspace, "node_modules", ...pkg.name.split("/"));
    await mkdir(path.dirname(destination), { recursive: true });
    await cp(packageRoot, destination, {
      recursive: true,
      dereference: true,
      // `.bin` shim directories are host-install conveniences, not runtime
      // payload; excluding them keeps the closure minimal and store-path-free.
      filter: (source) => path.basename(source) !== ".bin",
    });
    const manifest = JSON.parse(await readFile(path.join(destination, "package.json"), "utf8")) as {
      version: string;
    };
    // Deterministic tree digest over the staged package: sorted relative
    // paths bound to their content hashes, so any byte change is visible.
    const relativePaths = (await listFilesRecursive(destination)).toSorted();
    const tree = createHash("sha256");
    let byteLength = 0;
    for (const relativePath of relativePaths) {
      const bytes = await readFile(path.join(destination, ...relativePath.split("/")));
      byteLength += bytes.byteLength;
      tree.update(relativePath, "utf8");
      tree.update(new Uint8Array([0]));
      tree.update(createHash("sha256").update(bytes).digest());
    }
    return { available: true, version: manifest.version, sha256: tree.digest("hex"), byteLength };
  },
  buildApplication: async (workspace) => {
    execFileSync("node", ["--import", "tsx", "scripts/build.ts"], {
      cwd: appRoot,
      stdio: "inherit",
    });
    await cp(path.join(appRoot, "dist"), path.join(workspace, "dist"), {
      recursive: true,
      dereference: true,
      // The dev build stages a host-arch DEBUG helper under dist/native; the
      // staged closure carries only the explicit-triple release helper at
      // the plan's native/<arch> destination.
      filter: (source) => !source.startsWith(path.join(appRoot, "dist", "native")),
    });
    await cp(path.join(appRoot, "package.json"), path.join(workspace, "package.json"));
  },
  buildNativeHelper: async (workspace, plan) => {
    const workspaceRoot = path.resolve(appRoot, "../..");
    execFileSync("cargo", [...plan.cargo.args], { cwd: workspaceRoot, stdio: "inherit" });
    const built = path.join(workspaceRoot, ...plan.cargo.outputRelativePath.split("/"));
    const destination = path.join(workspace, ...plan.nativeHelperDestination.split("/"));
    await mkdir(path.dirname(destination), { recursive: true });
    await cp(built, destination);
    return { version: "0.1.0", ...(await sha256File(destination)) };
  },
  collectStagedFiles: async (workspace) => {
    const paths = await listFilesRecursive(workspace);
    const files: Array<StagedFile> = [];
    for (const relativePath of paths) {
      const absolute = path.join(workspace, ...relativePath.split("/"));
      const info = await stat(absolute);
      const bytes = await readFile(absolute);
      files.push({
        path: relativePath,
        byteLength: info.size,
        executable: (info.mode & 0o111) !== 0,
        header: bytes.subarray(0, 512),
      });
    }
    return files;
  },
  repoRoot: path.resolve(appRoot, "../.."),
  toolchain: {
    electron:
      (
        JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8")) as {
          devDependencies?: Record<string, string>;
        }
      ).devDependencies?.["electron"]?.replace(/^[~^]/, "") ?? "unknown",
    node: process.version.replace(/^v/, ""),
    pnpm: process.env.npm_config_user_agent?.match(/pnpm\/(\S+)/)?.[1] ?? "unknown",
  },
  worker: { workerRef, hostClass: `local-${process.platform}-${process.arch}` },
});

const parseCliDescriptor = (argv: ReadonlyArray<string>): DesktopTargetBuildDescriptor => {
  const flag = (name: string): string | undefined => {
    const index = argv.indexOf(`--${name}`);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  const targetKey = flag("target");
  if (targetKey === undefined) {
    throw new Error(
      "stage-target REQUIRES an explicit target descriptor: --target <darwin-arm64|darwin-x64|win32-arm64|win32-x64|linux-arm64|linux-x64> " +
        "--channel <stable|rc> --version <semver> --source-revision <sha> --lockfile-sha256 <hex> [--formats a,b] [--unsigned-dev] [--plan]",
    );
  }
  const formats = flag("formats");
  return decodeDesktopTargetBuildDescriptor({
    schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
    product: "OpenAgents",
    targetKey,
    channel: flag("channel") ?? "stable",
    version: flag("version"),
    sourceRevision: flag("source-revision"),
    lockfileSha256: flag("lockfile-sha256"),
    formats:
      formats !== undefined
        ? formats.split(",")
        : [...(desktopTargets[targetKey as DesktopTargetKey]?.requiredFormats ?? [])],
    signingPolicy: argv.includes("--unsigned-dev") ? "unsigned-dev" : "production",
  });
};

const direct =
  process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (direct) {
  const descriptor = parseCliDescriptor(process.argv.slice(2));
  const pins = readDesktopManifestPins(readFileSync(path.join(appRoot, "package.json"), "utf8"));
  if (process.argv.includes("--plan")) {
    process.stdout.write(
      `${JSON.stringify(stagingPlanForDescriptor(descriptor, pins), null, 2)}\n`,
    );
  } else {
    const result = await stageTarget(descriptor, pins, hostStageTargetIo("local-stage-cli"));
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    if (!result.ok) process.exitCode = 1;
  }
}
