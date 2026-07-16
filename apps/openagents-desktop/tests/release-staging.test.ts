/**
 * DIST-03 (#8916) — target-aware staging, native ledgers, and provenance
 * receipts. Fixture builds prove target selection for all six targets
 * WITHOUT six production runners; the negative oracles each fail on exactly
 * one injected defect (foreign binary, missing runtime, source-checkout
 * dependency, development file, unexpected ASAR entry); repeat staging from
 * the same inputs reproduces the same ledger identity.
 */
import { describe, expect, test } from "vite-plus/test";
import { Exit, Schema } from "effect";
import { readFileSync } from "node:fs";
import path from "node:path";

import {
  DesktopBuildReceiptSchema,
  DesktopTargetBuildDescriptorSchema,
  type DesktopTargetBuildDescriptor,
  type DesktopTargetKey,
  decodeDesktopTargetBuildDescriptor,
  desktopBuildReceiptRef,
  desktopReleaseSetArtifactName,
  desktopTargetKeys,
  desktopTargets,
  nativeComponentLedgerDigest,
  nativeComponentLedgerRef,
  NATIVE_COMPONENT_LEDGER_SCHEMA_ID,
  TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
} from "../src/release-staging-contract.ts";
import {
  buildNativeComponentLedger,
  detectExecutableArchitecture,
  finalizeDesktopBuildReceipt,
  readDesktopManifestPins,
  requiredRuntimePackages,
  stageTarget,
  stagedTreeViolations,
  stagingPlanForDescriptor,
  type StagedFile,
  type StageTargetIo,
} from "../scripts/stage-target.ts";

const root = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(root, "../..");
const sha = "a".repeat(40);
const lockfileSha256 = "b".repeat(64);
const digest = (seed: string): string => seed.repeat(64).slice(0, 64);

const descriptorFor = (
  targetKey: DesktopTargetKey,
  overrides: Partial<Record<string, unknown>> = {},
): DesktopTargetBuildDescriptor =>
  decodeDesktopTargetBuildDescriptor({
    schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
    product: "OpenAgents",
    targetKey,
    channel: "stable",
    version: "1.2.3",
    sourceRevision: sha,
    lockfileSha256,
    formats: [...desktopTargets[targetKey].requiredFormats],
    signingPolicy: "production",
    ...overrides,
  });

const pins = readDesktopManifestPins(readFileSync(path.join(root, "package.json"), "utf8"));
const decodeDescriptorExit = Schema.decodeUnknownExit(DesktopTargetBuildDescriptorSchema);
const decodeReceiptExit = Schema.decodeUnknownExit(DesktopBuildReceiptSchema);

// Native-header fixtures — real magic bytes, no real binaries required.
const machO = (arch: "arm64" | "x64"): Uint8Array => {
  const bytes = new Uint8Array(16);
  bytes.set([0xcf, 0xfa, 0xed, 0xfe]);
  bytes.set(arch === "arm64" ? [0x0c, 0x00, 0x00, 0x01] : [0x07, 0x00, 0x00, 0x01], 4);
  return bytes;
};
const elf = (arch: "arm64" | "x64"): Uint8Array => {
  const bytes = new Uint8Array(24);
  bytes.set([0x7f, 0x45, 0x4c, 0x46]);
  bytes[18] = arch === "arm64" ? 0xb7 : 0x3e;
  return bytes;
};
const pe = (arch: "arm64" | "x64"): Uint8Array => {
  const bytes = new Uint8Array(0x50);
  bytes.set([0x4d, 0x5a]);
  bytes[0x3c] = 0x40;
  bytes.set([0x50, 0x45, 0x00, 0x00], 0x40);
  const machine = arch === "arm64" ? [0x64, 0xaa] : [0x64, 0x86];
  bytes.set(machine, 0x44);
  return bytes;
};
const headerFor = (targetKey: DesktopTargetKey): Uint8Array => {
  const { platform, arch } = desktopTargets[targetKey];
  return platform === "darwin" ? machO(arch) : platform === "linux" ? elf(arch) : pe(arch);
};

const exeSuffix = (targetKey: DesktopTargetKey): string =>
  desktopTargets[targetKey].platform === "win32" ? ".exe" : "";

const cleanStagedTree = (targetKey: DesktopTargetKey): Array<StagedFile> => {
  const { platform, arch } = desktopTargets[targetKey];
  const header = headerFor(targetKey);
  const codexTriple = "fixture-triple";
  return [
    {
      path: "dist/main.js",
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x2f, 0x2f]),
    },
    { path: "package.json", byteLength: 10, executable: false, header: new Uint8Array([0x7b]) },
    {
      path: `native/${arch}/oa-desktop-audio${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
    },
    {
      path: `node_modules/@anthropic-ai/claude-agent-sdk/package.json`,
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x7b]),
    },
    {
      path: `node_modules/@anthropic-ai/claude-agent-sdk-${platform}-${arch}/claude${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
    },
    {
      path: `node_modules/@openai/codex/bin/codex.js`,
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x23, 0x21]),
    },
    {
      path: `node_modules/@openai/codex-${platform}-${arch}/vendor/${codexTriple}/bin/codex${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
    },
  ];
};

const auditInput = (targetKey: DesktopTargetKey, files: ReadonlyArray<StagedFile>) => ({
  descriptor: descriptorFor(targetKey),
  files,
  runtimePackages: requiredRuntimePackages(descriptorFor(targetKey), pins),
  repoRoot,
});

describe("DIST-03 target build descriptor", () => {
  test("accepts all six closed target keys with their required formats", () => {
    for (const targetKey of desktopTargetKeys) {
      const descriptor = descriptorFor(targetKey);
      expect(descriptor.targetKey).toBe(targetKey);
      expect(descriptor.formats).toEqual(desktopTargets[targetKey].requiredFormats);
    }
  });

  test("rejects an unknown target, a foreign format, duplicates, and channel/version drift", () => {
    const decode = decodeDescriptorExit;
    const base = {
      schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
      product: "OpenAgents",
      targetKey: "darwin-arm64",
      channel: "stable",
      version: "1.2.3",
      sourceRevision: sha,
      lockfileSha256,
      formats: ["dmg", "zip"],
      signingPolicy: "production",
    };
    expect(Exit.isFailure(decode({ ...base, targetKey: "darwin-universal" }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, formats: ["nsis"] }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, formats: ["dmg", "dmg"] }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, formats: [] }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, version: "1.2.3-rc.1" }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, channel: "rc" }))).toBe(true);
    expect(Exit.isFailure(decode({ ...base, sourceRevision: "main" }))).toBe(true);
    expect(Exit.isSuccess(decode({ ...base, channel: "rc", version: "1.2.3-rc.1" }))).toBe(true);
  });
});

describe("DIST-03 version-first immutable artifact names", () => {
  test("produces exactly the ProductSpec §6 basenames for every target/format", () => {
    const name = (targetKey: DesktopTargetKey, format: string): string =>
      desktopReleaseSetArtifactName({
        version: "1.2.3",
        channel: "stable",
        targetKey,
        format: format as never,
      });
    expect(name("darwin-arm64", "dmg")).toBe("OpenAgents-1.2.3-stable-darwin-arm64.dmg");
    expect(name("darwin-x64", "zip")).toBe("OpenAgents-1.2.3-stable-darwin-x64.zip");
    expect(name("win32-arm64", "nsis")).toBe("OpenAgents-1.2.3-stable-win32-arm64-setup.exe");
    expect(name("win32-x64", "nsis")).toBe("OpenAgents-1.2.3-stable-win32-x64-setup.exe");
    expect(name("linux-arm64", "appimage")).toBe("OpenAgents-1.2.3-stable-linux-arm64.AppImage");
    expect(name("linux-x64", "deb")).toBe("OpenAgents-1.2.3-stable-linux-x64.deb");
    expect(name("linux-x64", "rpm")).toBe("OpenAgents-1.2.3-stable-linux-x64.rpm");
    expect(
      desktopReleaseSetArtifactName({
        version: "1.2.3-rc.1",
        channel: "rc",
        targetKey: "darwin-arm64",
        format: "dmg",
      }),
    ).toBe("OpenAgents-1.2.3-rc.1-rc-darwin-arm64.dmg");
  });

  test("refuses foreign formats, bad channels, and path injection", () => {
    expect(() =>
      desktopReleaseSetArtifactName({
        version: "1.2.3",
        channel: "stable",
        targetKey: "darwin-arm64",
        format: "nsis",
      }),
    ).toThrow("not defined for target");
    expect(() =>
      desktopReleaseSetArtifactName({
        version: "../1.2.3",
        channel: "stable",
        targetKey: "darwin-arm64",
        format: "dmg",
      }),
    ).toThrow("Invalid release artifact version");
    expect(() =>
      desktopReleaseSetArtifactName({
        version: "1.2.3",
        channel: "nightly",
        targetKey: "darwin-arm64",
        format: "dmg",
      }),
    ).toThrow("Invalid release artifact channel");
  });
});

describe("DIST-03 staging plan (fixture target selection, no production runners)", () => {
  test("every target maps to its explicit Rust triple and target-only install env", () => {
    for (const targetKey of desktopTargetKeys) {
      const { platform, arch, rustTargetTriple } = desktopTargets[targetKey];
      const plan = stagingPlanForDescriptor(descriptorFor(targetKey), pins);
      expect(plan.cargo.args).toContain("--target");
      expect(plan.cargo.args).toContain(rustTargetTriple);
      expect(plan.cargo.outputRelativePath).toContain(rustTargetTriple);
      expect(plan.install.env.npm_config_platform).toBe(platform);
      expect(plan.install.env.npm_config_arch).toBe(arch);
      expect(plan.install.args).toContain("--prod");
      expect(plan.install.args).toContain("--frozen-lockfile");
      expect(plan.workspacePrefix).toContain(targetKey);
      expect(plan.nativeHelperDestination.startsWith(`native/${arch}/`)).toBe(true);
    }
  });

  test("selects the exact provider runtime packages for the requested target", () => {
    const win = requiredRuntimePackages(descriptorFor("win32-arm64"), pins);
    expect(win.map((pkg) => pkg.name)).toEqual([
      "@anthropic-ai/claude-agent-sdk",
      "@anthropic-ai/claude-agent-sdk-win32-arm64",
      "@openai/codex",
      "@openai/codex-win32-arm64",
    ]);
    const codexAlias = win.find((pkg) => pkg.name === "@openai/codex-win32-arm64");
    expect(codexAlias?.version).toBe(`${pins.codex}-win32-arm64`);
    const linux = requiredRuntimePackages(descriptorFor("linux-x64"), pins);
    expect(linux.map((pkg) => pkg.name)).toContain("@anthropic-ai/claude-agent-sdk-linux-x64");
  });

  test("release staging never infers the target from the host", () => {
    const source = readFileSync(path.join(root, "scripts", "stage-target.ts"), "utf8");
    const planningSource = source.slice(0, source.indexOf("// Real IO"));
    expect(planningSource).not.toContain("process.arch");
    expect(planningSource).not.toContain("process.platform");
    const forgeSource = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    expect(forgeSource).toContain("OA_DESKTOP_TARGET");
    expect(forgeSource).toContain("requireExplicitDesktopTarget");
  });
});

describe("DIST-03 executable architecture detection", () => {
  test("classifies Mach-O, ELF, and PE headers for both architectures", () => {
    expect(detectExecutableArchitecture(machO("arm64"))).toEqual({
      platform: "darwin",
      arch: "arm64",
    });
    expect(detectExecutableArchitecture(machO("x64"))).toEqual({ platform: "darwin", arch: "x64" });
    expect(detectExecutableArchitecture(elf("arm64"))).toEqual({
      platform: "linux",
      arch: "arm64",
    });
    expect(detectExecutableArchitecture(elf("x64"))).toEqual({ platform: "linux", arch: "x64" });
    expect(detectExecutableArchitecture(pe("arm64"))).toEqual({ platform: "win32", arch: "arm64" });
    expect(detectExecutableArchitecture(pe("x64"))).toEqual({ platform: "win32", arch: "x64" });
    expect(detectExecutableArchitecture(new Uint8Array([0x23, 0x21, 0x2f]))).toBeNull();
    // Universal binaries are not single-target payloads.
    expect(detectExecutableArchitecture(new Uint8Array([0xca, 0xfe, 0xba, 0xbe]))).toEqual({
      platform: "darwin",
      arch: "universal",
    });
  });
});

describe("DIST-03 staged-tree oracle", () => {
  test("a clean single-target closure passes for every target", () => {
    for (const targetKey of desktopTargetKeys) {
      expect(stagedTreeViolations(auditInput(targetKey, cleanStagedTree(targetKey)))).toEqual([]);
    }
  });

  test("ONE injected foreign-architecture binary fails the target", () => {
    const files = cleanStagedTree("darwin-arm64").map((file) =>
      file.path.endsWith("oa-desktop-audio") ? { ...file, header: machO("x64") } : file,
    );
    const violations = stagedTreeViolations(auditInput("darwin-arm64", files));
    expect(violations).toContainEqual(
      expect.objectContaining({
        kind: "foreign_architecture_binary",
        detail: expect.stringContaining("expected darwin-arm64, found darwin-x64"),
      }),
    );
  });

  test("a foreign-platform binary fails even at an allowlisted destination", () => {
    const files = cleanStagedTree("linux-arm64").map((file) =>
      file.path.endsWith("/claude") ? { ...file, header: pe("arm64") } : file,
    );
    expect(stagedTreeViolations(auditInput("linux-arm64", files))).toContainEqual(
      expect.objectContaining({ kind: "foreign_architecture_binary" }),
    );
  });

  test("one missing provider runtime package fails before maker work", () => {
    const files = cleanStagedTree("darwin-arm64").filter(
      (file) => !file.path.startsWith("node_modules/@openai/codex-darwin-arm64/"),
    );
    expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
      expect.objectContaining({
        kind: "missing_runtime_package",
        detail: expect.stringContaining("@openai/codex-darwin-arm64"),
      }),
    );
  });

  test("an unallowlisted native executable fails the target", () => {
    const files = [
      ...cleanStagedTree("darwin-arm64"),
      {
        path: "node_modules/some-dep/bin/helper",
        byteLength: 64,
        executable: true,
        header: machO("arm64"),
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
      expect.objectContaining({
        kind: "unallowlisted_binary",
        path: "node_modules/some-dep/bin/helper",
      }),
    );
  });

  test("script executables are admitted only inside dist/ or a required runtime package", () => {
    const script = { byteLength: 32, executable: true, header: new Uint8Array([0x23, 0x21]) };
    const admitted = [
      ...cleanStagedTree("darwin-arm64"),
      {
        ...script,
        path: "node_modules/@openai/codex/bin/codex-launcher.sh",
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", admitted))).toEqual([]);
    const foreign = [
      ...cleanStagedTree("darwin-arm64"),
      {
        ...script,
        path: "node_modules/left-pad/cli.sh",
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", foreign))).toContainEqual(
      expect.objectContaining({
        kind: "unallowlisted_binary",
        path: "node_modules/left-pad/cli.sh",
      }),
    );
  });

  test("a staged file carrying the absolute source checkout path fails", () => {
    const files = [
      ...cleanStagedTree("darwin-arm64"),
      {
        path: "dist/config.json",
        byteLength: 64,
        executable: false,
        header: new Uint8Array([0x7b]),
        content: `{"rendererRoot":"${repoRoot}/apps/openagents-desktop/dist"}`,
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
      expect.objectContaining({ kind: "source_checkout_dependency", path: "dist/config.json" }),
    );
  });

  test("a development/source file staged into the bundle fails", () => {
    for (const injected of [
      "src/main.ts",
      "tsconfig.json",
      "forge.config.ts",
      "dist/chat.test.js",
    ]) {
      const files = [
        ...cleanStagedTree("darwin-arm64"),
        {
          path: injected,
          byteLength: 8,
          executable: false,
          header: new Uint8Array([0x2f]),
        },
      ];
      expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
        expect.objectContaining({ kind: "development_file", path: injected }),
      );
    }
  });

  test("an unexpected ASAR entry fails; allowlisted entries pass", () => {
    const base = auditInput("darwin-arm64", cleanStagedTree("darwin-arm64"));
    expect(
      stagedTreeViolations({
        ...base,
        asarEntries: [
          "dist/main.js",
          "package.json",
          "node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs",
          "node_modules/@openai/codex/bin/codex.js",
        ],
      }),
    ).toEqual([]);
    expect(
      stagedTreeViolations({
        ...base,
        asarEntries: ["dist/main.js", "node_modules/left-pad/index.js"],
      }),
    ).toContainEqual(
      expect.objectContaining({
        kind: "unexpected_asar_entry",
        path: "node_modules/left-pad/index.js",
      }),
    );
  });
});

describe("DIST-03 native component ledger", () => {
  const components = [
    {
      name: "oa-desktop-audio",
      version: "0.1.0",
      sha256: digest("1"),
      byteLength: 1024,
      provenance: "workspace-crate" as const,
      destination: "native/arm64/oa-desktop-audio",
    },
    {
      name: "@openai/codex-darwin-arm64",
      version: "0.144.1-darwin-arm64",
      sha256: digest("2"),
      byteLength: 2048,
      provenance: "locked-dependency" as const,
      destination: "node_modules/@openai/codex-darwin-arm64",
    },
  ];

  test("is public-safe, single-target, and deterministic across repeat staging", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const first = buildNativeComponentLedger(descriptor, components);
    const second = buildNativeComponentLedger(descriptor, components.toReversed());
    expect(first.schema).toBe(NATIVE_COMPONENT_LEDGER_SCHEMA_ID);
    // Repeat staging from the same inputs — identical ledger identity, even
    // with a different component discovery order.
    expect(nativeComponentLedgerDigest(first)).toBe(nativeComponentLedgerDigest(second));
    expect(nativeComponentLedgerRef(first)).toBe(`sha256:${nativeComponentLedgerDigest(first)}`);
    // Any input change changes the identity.
    const mutated = buildNativeComponentLedger(descriptor, [
      { ...components[0]!, sha256: digest("3") },
      components[1]!,
    ]);
    expect(nativeComponentLedgerDigest(mutated)).not.toBe(nativeComponentLedgerDigest(first));
    expect(JSON.stringify(first)).not.toContain(repoRoot);
  });

  test("rejects absolute destinations, traversal, and duplicate destinations", () => {
    const descriptor = descriptorFor("darwin-arm64");
    expect(() =>
      buildNativeComponentLedger(descriptor, [
        { ...components[0]!, destination: "/usr/local/bin/oa-desktop-audio" },
      ]),
    ).toThrow();
    expect(() =>
      buildNativeComponentLedger(descriptor, [{ ...components[0]!, destination: "../escape" }]),
    ).toThrow();
    expect(() =>
      buildNativeComponentLedger(descriptor, [components[0]!, components[0]!]),
    ).toThrow();
  });
});

describe("DIST-03 build receipt", () => {
  const draftFor = (descriptor: DesktopTargetBuildDescriptor) => ({
    descriptor,
    componentLedger: { sha256: digest("4"), componentCount: 5 },
    toolchain: { electron: "39.2.7", node: "24.6.0", pnpm: "11.10.0" },
    worker: { workerRef: "runner:fixture-01", hostClass: "owned-mac-arm64" },
  });
  const artifactsFor = (descriptor: DesktopTargetBuildDescriptor) =>
    descriptor.formats.map((format) => ({
      name: desktopReleaseSetArtifactName({
        version: descriptor.version,
        channel: descriptor.channel,
        targetKey: descriptor.targetKey,
        format,
      }),
      format,
      sha256: digest("5"),
      byteLength: 4096,
    }));

  test("binds descriptor, lockfile identity, toolchain, ledger ref, artifacts, and worker", () => {
    for (const targetKey of desktopTargetKeys) {
      const descriptor = descriptorFor(targetKey);
      const receipt = finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        artifactsFor(descriptor),
        "2026-07-16T12:00:00Z",
      );
      expect(receipt.descriptor.lockfileSha256).toBe(lockfileSha256);
      expect(receipt.componentLedger.sha256).toBe(digest("4"));
      expect(desktopBuildReceiptRef(receipt)).toMatch(/^sha256:[0-9a-f]{64}$/);
    }
  });

  test("is structurally inadmissible for unsigned-dev output", () => {
    const descriptor = descriptorFor("darwin-arm64", { signingPolicy: "unsigned-dev" });
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        artifactsFor(descriptor),
        "2026-07-16T12:00:00Z",
      ),
    ).toThrow(/unsigned-dev builds are ineligible/);
    const production = descriptorFor("darwin-arm64");
    const marked = artifactsFor(production).map((artifact, index) =>
      index === 0
        ? { ...artifact, name: "OpenAgents-1.2.3-UNSIGNED-DEV-stable-darwin-arm64.dmg" }
        : artifact,
    );
    expect(() =>
      finalizeDesktopBuildReceipt(draftFor(production), marked, "2026-07-16T12:00:00Z"),
    ).toThrow(/UNSIGNED-DEV/);
  });

  test("refuses non-canonical names, missing formats, and format duplication", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const artifacts = artifactsFor(descriptor);
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        [{ ...artifacts[0]!, name: "OpenAgents-darwin-arm64.dmg" }, artifacts[1]!],
        "2026-07-16T12:00:00Z",
      ),
    ).toThrow(/canonical/);
    expect(() =>
      finalizeDesktopBuildReceipt(draftFor(descriptor), [artifacts[0]!], "2026-07-16T12:00:00Z"),
    ).toThrow(/cover every descriptor format/);
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        [artifacts[0]!, artifacts[0]!],
        "2026-07-16T12:00:00Z",
      ),
    ).toThrow(/duplicate artifact format/);
    expect(Exit.isFailure(decodeReceiptExit({}))).toBe(true);
  });
});

describe("DIST-03 stageTarget orchestration (fixture io)", () => {
  const fixtureIo = (input: {
    targetKey: DesktopTargetKey;
    unavailable?: ReadonlyArray<string>;
    injectViolation?: StagedFile;
  }): StageTargetIo & { readonly log: Array<string> } => {
    const log: Array<string> = [];
    return {
      log,
      createWorkspace: async (prefix) => {
        log.push(`workspace:${prefix}`);
        return `/tmp/fixture/${prefix}x`;
      },
      materializeRuntimePackage: async (_workspace, pkg) => {
        log.push(`materialize:${pkg.name}`);
        if (input.unavailable?.includes(pkg.name)) return { available: false };
        return { available: true, version: pkg.version, sha256: digest("6"), byteLength: 32 };
      },
      buildApplication: async () => {
        log.push("buildApplication");
      },
      buildNativeHelper: async (_workspace, plan) => {
        log.push(`cargo:${plan.cargo.args.join(" ")}`);
        return { sha256: digest("7"), byteLength: 64, version: "0.1.0" };
      },
      collectStagedFiles: async () => {
        const files = cleanStagedTree(input.targetKey);
        return input.injectViolation ? [...files, input.injectViolation] : files;
      },
      repoRoot,
      toolchain: { electron: "39.2.7", node: "24.6.0", pnpm: "11.10.0" },
      worker: { workerRef: "runner:fixture-01", hostClass: "owned-fixture" },
    };
  };

  test("stages any of the six targets from one host and emits a deterministic ledger", async () => {
    for (const targetKey of desktopTargetKeys) {
      const descriptor = descriptorFor(targetKey);
      const io = fixtureIo({ targetKey });
      const first = await stageTarget(descriptor, pins, io);
      const second = await stageTarget(descriptor, pins, fixtureIo({ targetKey }));
      expect(first.ok).toBe(true);
      expect(second.ok).toBe(true);
      if (!first.ok || !second.ok) return;
      expect(first.ledgerDigest).toBe(second.ledgerDigest);
      expect(first.unsignedDev).toBe(false);
      expect(first.receiptDraft?.componentLedger.componentCount).toBe(
        first.ledger.components.length,
      );
      expect(io.log).toContainEqual(
        `cargo:build --release -p oa-desktop-audio --target ${desktopTargets[targetKey].rustTargetTriple}`,
      );
    }
  });

  test("fails typed and BEFORE native build/maker work when a runtime package is unavailable", async () => {
    const io = fixtureIo({ targetKey: "win32-arm64", unavailable: ["@openai/codex-win32-arm64"] });
    const result = await stageTarget(descriptorFor("win32-arm64"), pins, io);
    expect(result).toMatchObject({
      ok: false,
      failure: "missing_runtime_package",
      missingPackages: [`@openai/codex-win32-arm64@${pins.codex}-win32-arm64`],
    });
    expect(io.log).not.toContainEqual("buildApplication");
    expect(io.log.some((line) => line.startsWith("cargo:"))).toBe(false);
  });

  test("fails closed on a staged-tree violation before any maker may run", async () => {
    const io = fixtureIo({
      targetKey: "darwin-arm64",
      injectViolation: {
        path: "node_modules/evil/bin/payload",
        byteLength: 16,
        executable: true,
        header: elf("x64"),
      },
    });
    const result = await stageTarget(descriptorFor("darwin-arm64"), pins, io);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.failure).toBe("staged_tree_violations");
    expect(result.violations).toContainEqual(
      expect.objectContaining({ kind: "foreign_architecture_binary" }),
    );
  });

  test("unsigned-dev staging is conspicuous and never yields a receipt draft", async () => {
    const descriptor = descriptorFor("darwin-arm64", { signingPolicy: "unsigned-dev" });
    const result = await stageTarget(descriptor, pins, fixtureIo({ targetKey: "darwin-arm64" }));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.unsignedDev).toBe(true);
    expect(result.receiptDraft).toBeUndefined();
  });
});
