/**
 * DIST-03 (#8916) — target-aware staging, native ledgers, and provenance
 * receipts (amended per the two independent review comments). Fixture builds
 * prove target selection for all six targets WITHOUT six production runners;
 * the negative oracles each fail on exactly one injected defect (foreign
 * binary, unknown/truncated executable identity, escaping symlink, missing
 * runtime, source-checkout dependency, development file, unexpected ASAR
 * entry); repeat staging from the same inputs reproduces the same ledger
 * identity; descriptors require EXACT per-target format coverage.
 */
import { describe, expect, test } from "vite-plus/test";
import { Exit, Schema } from "effect";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { createPackage } from "@electron/asar";

import {
  DesktopBuildReceiptSchema,
  DesktopTargetBuildDescriptorSchema,
  type DesktopBuildToolchain,
  type DesktopTargetBuildDescriptor,
  type DesktopTargetKey,
  type NativeComponentLedgerEntry,
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
  asarPlacementViolations,
  buildNativeComponentLedger,
  closureOwnerForDestination,
  detectExecutableArchitecture,
  detectSigningState,
  extraResourceDestination,
  finalizeDesktopBuildReceipt,
  makerIdentityRefs,
  nativeClosureEntries,
  plannedAsarPlacement,
  readDesktopManifestPins,
  requiredRuntimePackages,
  stageTarget,
  stagedTreePath,
  stagedTreeViolations,
  stagingPlanForDescriptor,
  verifyStagedTreeAgainstLedger,
  verifyPackagedClosureBytes,
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

const fixtureToolchain: DesktopBuildToolchain = {
  electron: "43.1.0",
  node: "24.6.0",
  pnpm: "11.10.0",
  forge: "7.11.2",
  rust: "rustc 1.88.0 (fixture)",
  compiler: "Apple clang version 17.0.0 (fixture)",
};
const fixtureMetadata = {
  lockfileSha256,
  osImage: "darwin-arm64-25.4.0",
  toolchain: fixtureToolchain,
};

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
      sha256: digest("a"),
    },
    {
      path: "package.json",
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x7b]),
      sha256: digest("b"),
    },
    {
      path: `native/${arch}/oa-desktop-audio${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
      sha256: digest("c"),
    },
    {
      path: `node_modules/@anthropic-ai/claude-agent-sdk/package.json`,
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x7b]),
      sha256: digest("d"),
    },
    {
      path: `node_modules/@anthropic-ai/claude-agent-sdk-${platform}-${arch}/claude${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
      sha256: digest("e"),
    },
    {
      path: `node_modules/@openai/codex/bin/codex.js`,
      byteLength: 10,
      executable: false,
      header: new Uint8Array([0x23, 0x21]),
      sha256: digest("f"),
    },
    {
      path: `node_modules/@openai/codex-${platform}-${arch}/vendor/${codexTriple}/bin/codex${exeSuffix(targetKey)}`,
      byteLength: 100,
      executable: true,
      header,
      sha256: digest("0"),
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

  test("REQUIRES exact per-target format coverage — subsets are refused", () => {
    const decode = decodeDescriptorExit;
    const base = {
      schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
      product: "OpenAgents",
      channel: "stable",
      version: "1.2.3",
      sourceRevision: sha,
      lockfileSha256,
      signingPolicy: "production",
    };
    // darwin cannot omit zip; linux cannot omit deb/rpm; win32 needs nsis.
    expect(Exit.isFailure(decode({ ...base, targetKey: "darwin-arm64", formats: ["dmg"] }))).toBe(
      true,
    );
    expect(Exit.isFailure(decode({ ...base, targetKey: "darwin-x64", formats: ["zip"] }))).toBe(
      true,
    );
    expect(
      Exit.isFailure(decode({ ...base, targetKey: "linux-x64", formats: ["appimage", "deb"] })),
    ).toBe(true);
    expect(
      Exit.isSuccess(
        decode({ ...base, targetKey: "linux-x64", formats: ["rpm", "appimage", "deb"] }),
      ),
    ).toBe(true);
    expect(Exit.isSuccess(decode({ ...base, targetKey: "win32-x64", formats: ["nsis"] }))).toBe(
      true,
    );
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
  test("every target maps to its explicit Rust triple and target-only locked install", () => {
    for (const targetKey of desktopTargetKeys) {
      const { platform, arch, rustTargetTriple } = desktopTargets[targetKey];
      const plan = stagingPlanForDescriptor(descriptorFor(targetKey), pins);
      expect(plan.cargo.args).toContain("--target");
      expect(plan.cargo.args).toContain(rustTargetTriple);
      expect(plan.cargo.outputRelativePath).toContain(rustTargetTriple);
      expect(plan.install.command).toBe("pnpm");
      expect(plan.install.args).toContain("--prod");
      expect(plan.install.args).toContain("--frozen-lockfile");
      expect(plan.install.args).toContain("--ignore-scripts");
      expect(plan.install.supportedArchitectures.os).toEqual([platform]);
      expect(plan.install.supportedArchitectures.cpu).toEqual([arch]);
      expect(plan.install.env.npm_config_platform).toBe(platform);
      expect(plan.install.env.npm_config_arch).toBe(arch);
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
    expect(forgeSource).toContain("OA_DESKTOP_STAGING_WORKSPACE");
    expect(forgeSource).toContain("requireStagedBuildInputs");
    expect(forgeSource).not.toContain("process.arch");
  });

  test("forge packages the STAGED tree with the descriptor decoded once — never process.cwd() content", () => {
    const forgeSource = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    // The checkout copy is fully ignored; the staged tree is the sole source.
    expect(forgeSource).toContain("ignore: () => true");
    expect(forgeSource).toContain("decodeDesktopTargetBuildDescriptor");
    expect(forgeSource).toContain("cachedStagedBuildInputs");
    expect(forgeSource).toContain('for (const entry of ["dist", "package.json", "node_modules"])');
    // Packaging never builds: staging owns app + native builds.
    expect(forgeSource).not.toContain('execFileSync("cargo"');
    expect(forgeSource).not.toContain("scripts/build.ts");
    // The live post-package asar gate is wired.
    expect(forgeSource).toContain("postPackage");
    expect(forgeSource).toContain("assertPackagedAsarAdmissible");
    // The packaging entrypoints route through the descriptor-first wrapper.
    const manifest = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(manifest.scripts["package:mac"]).toContain("stage-and-package.ts");
    expect(manifest.scripts["make:mac"]).toContain("stage-and-package.ts");
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

  test("truncated or unprovable native identity reports unknown — never null", () => {
    // Mach-O magic with the cputype outside the sample.
    expect(detectExecutableArchitecture(new Uint8Array([0xcf, 0xfa, 0xed, 0xfe]))).toEqual({
      platform: "darwin",
      arch: "unknown",
    });
    // ELF magic with the machine field outside the sample.
    expect(detectExecutableArchitecture(new Uint8Array([0x7f, 0x45, 0x4c, 0x46]))).toEqual({
      platform: "linux",
      arch: "unknown",
    });
    // MZ stub whose PE header offset points outside the sample.
    const truncatedPe = new Uint8Array(0x40);
    truncatedPe.set([0x4d, 0x5a]);
    truncatedPe[0x3c] = 0xf0;
    expect(detectExecutableArchitecture(truncatedPe)).toEqual({
      platform: "unknown",
      arch: "unknown",
    });
    // MZ stub without a verifiable PE signature.
    const dosOnly = new Uint8Array(0x60);
    dosOnly.set([0x4d, 0x5a]);
    expect(detectExecutableArchitecture(dosOnly)).toEqual({
      platform: "unknown",
      arch: "unknown",
    });
  });

  test("reports embedded signing state from header truth", () => {
    // Mach-O with one LC_CODE_SIGNATURE load command.
    const signed = new Uint8Array(64);
    signed.set([0xcf, 0xfa, 0xed, 0xfe]);
    signed.set([0x0c, 0x00, 0x00, 0x01], 4); // arm64
    signed[16] = 1; // ncmds = 1
    signed[32] = 0x1d; // LC_CODE_SIGNATURE
    signed[36] = 16; // cmdsize
    expect(detectSigningState(signed, { platform: "darwin", arch: "arm64" })).toBe("signed");
    // Mach-O with one non-signature load command.
    const unsigned = new Uint8Array(64);
    unsigned.set(signed);
    unsigned[32] = 0x19; // LC_SEGMENT_64
    expect(detectSigningState(unsigned, { platform: "darwin", arch: "arm64" })).toBe("unsigned");
    // Truncated Mach-O cannot prove either way.
    expect(detectSigningState(machO("arm64"), { platform: "darwin", arch: "arm64" })).toBe(
      "undetermined",
    );
    // ELF has no embedded-signature concept.
    expect(detectSigningState(elf("x64"), { platform: "linux", arch: "x64" })).toBe(
      "not-applicable",
    );
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

  test("unknown/truncated executable identity FAILS CLOSED — even at allowlisted destinations", () => {
    // A truncated Mach-O at the allowlisted native helper destination.
    const truncated = cleanStagedTree("darwin-arm64").map((file) =>
      file.path.endsWith("oa-desktop-audio")
        ? { ...file, header: new Uint8Array([0xcf, 0xfa, 0xed, 0xfe]) }
        : file,
    );
    expect(stagedTreeViolations(auditInput("darwin-arm64", truncated))).toContainEqual(
      expect.objectContaining({
        kind: "unknown_executable_identity",
        path: "native/arm64/oa-desktop-audio",
      }),
    );
    // An opaque non-native payload spoofing the allowlisted claude runtime.
    const spoofed = cleanStagedTree("darwin-arm64").map((file) =>
      file.path.endsWith("/claude") ? { ...file, header: new Uint8Array([0x23, 0x21]) } : file,
    );
    expect(stagedTreeViolations(auditInput("darwin-arm64", spoofed))).toContainEqual(
      expect.objectContaining({
        kind: "unknown_executable_identity",
        detail: expect.stringContaining("provable native header"),
      }),
    );
    // An MZ stub with an unverifiable PE header inside a runtime package.
    const dosStub = new Uint8Array(0x60);
    dosStub.set([0x4d, 0x5a]);
    const injected = [
      ...cleanStagedTree("win32-x64"),
      {
        path: "node_modules/@openai/codex/vendor/helper.exe",
        byteLength: 96,
        executable: true,
        header: dosStub,
        sha256: digest("9"),
      },
    ];
    expect(stagedTreeViolations(auditInput("win32-x64", injected))).toContainEqual(
      expect.objectContaining({
        kind: "unknown_executable_identity",
        path: "node_modules/@openai/codex/vendor/helper.exe",
      }),
    );
    // A native-module extension without a provable native header.
    const fakeNode = [
      ...cleanStagedTree("darwin-arm64"),
      {
        path: "node_modules/@openai/codex/build/fake.node",
        byteLength: 32,
        executable: false,
        header: new Uint8Array([0x00, 0x01, 0x02]),
        sha256: digest("8"),
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", fakeNode))).toContainEqual(
      expect.objectContaining({
        kind: "unknown_executable_identity",
        path: "node_modules/@openai/codex/build/fake.node",
      }),
    );
  });

  test("an escaping symlink fails; a bounded relative symlink does not", () => {
    const base = cleanStagedTree("darwin-arm64");
    for (const target of ["/etc/passwd", "../../outside", "vendor/../../escape"]) {
      const files = [
        ...base,
        {
          path: "node_modules/@openai/codex/vendor/link",
          byteLength: 1,
          executable: false,
          header: new Uint8Array(0),
          symlinkTarget: target,
        },
      ];
      expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
        expect.objectContaining({
          kind: "source_checkout_dependency",
          path: "node_modules/@openai/codex/vendor/link",
          detail: "symlink escapes the staging workspace",
        }),
      );
    }
    const bounded = [
      ...base,
      {
        path: "node_modules/@openai/codex/vendor/link",
        byteLength: 1,
        executable: false,
        header: new Uint8Array(0),
        symlinkTarget: "sibling/file",
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", bounded))).toEqual([]);
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
        sha256: digest("7"),
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
    const script = {
      byteLength: 32,
      executable: true,
      header: new Uint8Array([0x23, 0x21]),
      sha256: digest("6"),
    };
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

  test("a staged file carrying the absolute source checkout or staging path fails", () => {
    const files = [
      ...cleanStagedTree("darwin-arm64"),
      {
        path: "dist/config.json",
        byteLength: 64,
        executable: false,
        header: new Uint8Array([0x7b]),
        sha256: digest("5"),
        content: `{"rendererRoot":"${repoRoot}/apps/openagents-desktop/dist"}`,
      },
    ];
    expect(stagedTreeViolations(auditInput("darwin-arm64", files))).toContainEqual(
      expect.objectContaining({ kind: "source_checkout_dependency", path: "dist/config.json" }),
    );
    const stagingLeak = [
      ...cleanStagedTree("darwin-arm64"),
      {
        path: "dist/paths.json",
        byteLength: 64,
        executable: false,
        header: new Uint8Array([0x7b]),
        sha256: digest("4"),
        content: '{"source":"/tmp/oa-desktop-stage-darwin-arm64-xyz/source"}',
      },
    ];
    expect(
      stagedTreeViolations({
        ...auditInput("darwin-arm64", stagingLeak),
        forbiddenPathPrefixes: ["/tmp/oa-desktop-stage-darwin-arm64-xyz"],
      }),
    ).toContainEqual(
      expect.objectContaining({ kind: "source_checkout_dependency", path: "dist/paths.json" }),
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
          sha256: digest("3"),
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

describe("DIST-03 planned ASAR placement (mirror of the packaging boundary)", () => {
  test("provider runtimes and renderer/worker entries unpack; resources ship beside the asar", () => {
    expect(plannedAsarPlacement("native/arm64/oa-desktop-audio")).toBe("extra-resource");
    expect(plannedAsarPlacement("dist/builtin-skills/manifest.json")).toBe("extra-resource");
    expect(plannedAsarPlacement("dist/renderer/boot.js")).toBe("unpacked");
    expect(plannedAsarPlacement("dist/workers/codex-history-worker.js")).toBe("unpacked");
    expect(plannedAsarPlacement("node_modules/@openai/codex-darwin-arm64/vendor/t/bin/codex")).toBe(
      "unpacked",
    );
    expect(plannedAsarPlacement("node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs")).toBe(
      "unpacked",
    );
    expect(plannedAsarPlacement("dist/main.js")).toBe("asar");
    expect(plannedAsarPlacement("package.json")).toBe("asar");
  });
});

describe("DIST-03 native component ledger (§9)", () => {
  const closureFiles = (targetKey: DesktopTargetKey): ReadonlyArray<StagedFile> =>
    cleanStagedTree(targetKey);
  const versionsFor = (targetKey: DesktopTargetKey): ReadonlyMap<string, string> =>
    new Map(
      requiredRuntimePackages(descriptorFor(targetKey), pins).map((pkg) => [pkg.name, pkg.version]),
    );

  test("enumerates the per-FILE native dependency closure — never aggregate package trees", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const entries = nativeClosureEntries(
      descriptor,
      closureFiles("darwin-arm64"),
      versionsFor("darwin-arm64"),
      "0.1.0",
    );
    const byDestination = new Map(entries.map((entry) => [entry.destination, entry]));
    // Exact executable files with architecture, signing, and placement state.
    expect(byDestination.get("native/arm64/oa-desktop-audio")).toMatchObject({
      name: "oa-desktop-audio",
      fileKind: "executable",
      architecture: "arm64",
      provenance: "workspace-crate",
      asarPlacement: "extra-resource",
    });
    expect(
      byDestination.get("node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"),
    ).toMatchObject({
      name: "@anthropic-ai/claude-agent-sdk-darwin-arm64",
      fileKind: "executable",
      architecture: "arm64",
      provenance: "locked-dependency",
      asarPlacement: "unpacked",
    });
    expect(
      byDestination.get("node_modules/@openai/codex-darwin-arm64/vendor/fixture-triple/bin/codex"),
    ).toMatchObject({ name: "@openai/codex-darwin-arm64", fileKind: "executable" });
    // Aggregate package entries (destination = the package directory) do not exist.
    expect(byDestination.has("node_modules/@openai/codex-darwin-arm64")).toBe(false);
    // Non-executable data files are not closure entries.
    expect(byDestination.has("dist/main.js")).toBe(false);
  });

  test("carries the complete §9 metadata: lockfile digest, OS image, toolchain, makers, artifacts", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const ledger = buildNativeComponentLedger(
      descriptor,
      nativeClosureEntries(
        descriptor,
        closureFiles("darwin-arm64"),
        versionsFor("darwin-arm64"),
        "0.1.0",
      ),
      fixtureMetadata,
    );
    expect(ledger.schema).toBe(NATIVE_COMPONENT_LEDGER_SCHEMA_ID);
    // Pre-maker staging evidence is EXPLICITLY typed as such; the receipt is
    // the only document carrying final artifact/maker identities.
    expect(ledger.phase).toBe("pre-maker-staging");
    expect(ledger.lockfileSha256).toBe(lockfileSha256);
    expect(ledger.osImage).toBe("darwin-arm64-25.4.0");
    expect(ledger.toolchain).toMatchObject({
      electron: "43.1.0",
      node: "24.6.0",
      pnpm: "11.10.0",
      forge: "7.11.2",
      rust: expect.stringContaining("rustc"),
      compiler: expect.stringContaining("clang"),
    });
    expect(ledger.plannedMakerIdentities).toEqual([
      { format: "dmg", ref: "maker:forge-dmg-7.11.2" },
      { format: "zip", ref: "maker:forge-zip-7.11.2" },
    ]);
    expect(ledger.packageContentAllowlist).toBe("pass");
    expect(ledger.plannedArtifacts).toEqual([
      { name: "OpenAgents-1.2.3-stable-darwin-arm64.dmg", format: "dmg" },
      { name: "OpenAgents-1.2.3-stable-darwin-arm64.zip", format: "zip" },
    ]);
    expect(makerIdentityRefs("win32-x64", "7.11.2")).toEqual([
      { format: "nsis", ref: "maker:pending-nsis" },
    ]);
  });

  test("is public-safe, single-target, and deterministic across repeat staging", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const entries = nativeClosureEntries(
      descriptor,
      closureFiles("darwin-arm64"),
      versionsFor("darwin-arm64"),
      "0.1.0",
    );
    const first = buildNativeComponentLedger(descriptor, entries, fixtureMetadata);
    const second = buildNativeComponentLedger(descriptor, [...entries].reverse(), fixtureMetadata);
    // Repeat staging from the same inputs — identical ledger identity, even
    // with a different component discovery order.
    expect(nativeComponentLedgerDigest(first)).toBe(nativeComponentLedgerDigest(second));
    expect(nativeComponentLedgerRef(first)).toBe(`sha256:${nativeComponentLedgerDigest(first)}`);
    // Any input change changes the identity.
    const mutated = buildNativeComponentLedger(
      descriptor,
      entries.map((entry, index) => (index === 0 ? { ...entry, sha256: digest("2") } : entry)),
      fixtureMetadata,
    );
    expect(nativeComponentLedgerDigest(mutated)).not.toBe(nativeComponentLedgerDigest(first));
    expect(JSON.stringify(first)).not.toContain(repoRoot);
  });

  test("rejects absolute destinations, traversal, duplicates, and cross-arch entries", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const entries = nativeClosureEntries(
      descriptor,
      closureFiles("darwin-arm64"),
      versionsFor("darwin-arm64"),
      "0.1.0",
    );
    const helper = entries.find((entry) => entry.name === "oa-desktop-audio")!;
    expect(() =>
      buildNativeComponentLedger(
        descriptor,
        [{ ...helper, destination: "/usr/local/bin/oa-desktop-audio" }],
        fixtureMetadata,
      ),
    ).toThrow();
    expect(() =>
      buildNativeComponentLedger(
        descriptor,
        [{ ...helper, destination: "../escape" }],
        fixtureMetadata,
      ),
    ).toThrow();
    expect(() =>
      buildNativeComponentLedger(descriptor, [helper, helper], fixtureMetadata),
    ).toThrow();
    // A native executable claiming the wrong architecture is inadmissible.
    expect(() =>
      buildNativeComponentLedger(
        descriptor,
        [{ ...helper, architecture: "x64" as NativeComponentLedgerEntry["architecture"] }],
        fixtureMetadata,
      ),
    ).toThrow();
    // An executable placed inside app.asar is inadmissible.
    expect(() =>
      buildNativeComponentLedger(
        descriptor,
        [{ ...helper, destination: "dist/main.js", asarPlacement: "asar" }],
        fixtureMetadata,
      ),
    ).toThrow();
  });

  test("closure ownership resolves package, workspace crate, and app resources", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const versions = versionsFor("darwin-arm64");
    expect(
      closureOwnerForDestination(
        "node_modules/@openai/codex-darwin-arm64/vendor/t/bin/codex",
        descriptor,
        versions,
        "0.1.0",
      ),
    ).toMatchObject({ name: "@openai/codex-darwin-arm64", provenance: "locked-dependency" });
    expect(
      closureOwnerForDestination("native/arm64/oa-desktop-audio", descriptor, versions, "0.1.0"),
    ).toEqual({ name: "oa-desktop-audio", version: "0.1.0", provenance: "workspace-crate" });
    expect(
      closureOwnerForDestination("dist/builtin-skills/run.sh", descriptor, versions, "0.1.0"),
    ).toMatchObject({ name: "openagents-desktop-app", provenance: "application-resource" });
  });
});

describe("DIST-03 build receipt", () => {
  const draftFor = (descriptor: DesktopTargetBuildDescriptor) => ({
    descriptor,
    componentLedger: { sha256: digest("4"), componentCount: 5 },
    toolchain: fixtureToolchain,
    gates: { stagedTree: "pass" as const },
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
      makerRef: `maker:forge-${format}-7.11.2`,
    }));

  test("binds descriptor, lockfile identity, toolchain, gates, ledger ref, artifacts, and worker", () => {
    for (const targetKey of desktopTargetKeys) {
      const descriptor = descriptorFor(targetKey);
      const receipt = finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        artifactsFor(descriptor),
        "2026-07-16T12:00:00Z",
        "pass",
      );
      expect(receipt.descriptor.lockfileSha256).toBe(lockfileSha256);
      expect(receipt.componentLedger.sha256).toBe(digest("4"));
      expect(receipt.gates).toEqual({ stagedTree: "pass", asarAllowlist: "pass" });
      expect(receipt.toolchain.forge).toBe("7.11.2");
      // Final artifact evidence carries the ACTUAL maker identity per output.
      for (const artifact of receipt.artifacts) {
        expect(artifact.makerRef).toMatch(/^maker:forge-/);
      }
      expect(desktopBuildReceiptRef(receipt)).toMatch(/^sha256:[0-9a-f]{64}$/);
    }
  });

  test("refuses planned/pending maker identities — pre-maker staging evidence can never masquerade as final artifact evidence", () => {
    const descriptor = descriptorFor("darwin-arm64");
    const pending = artifactsFor(descriptor).map((artifact, index) =>
      index === 0 ? { ...artifact, makerRef: "maker:pending-dmg" } : artifact,
    );
    expect(() =>
      finalizeDesktopBuildReceipt(draftFor(descriptor), pending, "2026-07-16T12:00:00Z", "pass"),
    ).toThrow(/ACTUAL maker identity/);
    const nonMaker = artifactsFor(descriptor).map((artifact, index) =>
      index === 0 ? { ...artifact, makerRef: "forge-dmg-7.11.2" } : artifact,
    );
    expect(() =>
      finalizeDesktopBuildReceipt(draftFor(descriptor), nonMaker, "2026-07-16T12:00:00Z", "pass"),
    ).toThrow(/ACTUAL maker identity/);
  });

  test("is structurally inadmissible for unsigned-dev output", () => {
    const descriptor = descriptorFor("darwin-arm64", { signingPolicy: "unsigned-dev" });
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        artifactsFor(descriptor),
        "2026-07-16T12:00:00Z",
        "pass",
      ),
    ).toThrow(/unsigned-dev builds are ineligible/);
    const production = descriptorFor("darwin-arm64");
    const marked = artifactsFor(production).map((artifact, index) =>
      index === 0
        ? { ...artifact, name: "OpenAgents-1.2.3-UNSIGNED-DEV-stable-darwin-arm64.dmg" }
        : artifact,
    );
    expect(() =>
      finalizeDesktopBuildReceipt(draftFor(production), marked, "2026-07-16T12:00:00Z", "pass"),
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
        "pass",
      ),
    ).toThrow(/canonical/);
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        [artifacts[0]!],
        "2026-07-16T12:00:00Z",
        "pass",
      ),
    ).toThrow(/cover every descriptor format/);
    expect(() =>
      finalizeDesktopBuildReceipt(
        draftFor(descriptor),
        [artifacts[0]!, artifacts[0]!],
        "2026-07-16T12:00:00Z",
        "pass",
      ),
    ).toThrow(/duplicate artifact format/);
    expect(Exit.isFailure(decodeReceiptExit({}))).toBe(true);
  });
});

describe("DIST-03 stageTarget orchestration (fixture io)", () => {
  const fixtureIo = (input: {
    targetKey: DesktopTargetKey;
    unavailable?: ReadonlyArray<string>;
    wrongVersion?: ReadonlyArray<string>;
    injectViolation?: StagedFile;
    lockfileSha256?: string;
  }): StageTargetIo & { readonly log: Array<string> } => {
    const log: Array<string> = [];
    return {
      log,
      createWorkspace: async (prefix) => {
        log.push(`workspace:${prefix}`);
        return `/tmp/fixture/${prefix}x`;
      },
      exportSource: async (workspace, sourceRevision) => {
        log.push(`export:${sourceRevision}`);
        return `${workspace}/source`;
      },
      // Runtime pins come from the EXPORTED source manifest, never the live
      // checkout — the fixture serves the pins AS the archived manifest.
      readDesktopSourceManifest: async () => {
        log.push("readSourceManifest");
        return JSON.stringify({
          dependencies: {
            "@anthropic-ai/claude-agent-sdk": pins.claudeAgentSdk,
            "@openai/codex": pins.codex,
          },
        });
      },
      lockfileSha256: async () => input.lockfileSha256 ?? lockfileSha256,
      runTargetProductionInstall: async (_sourceRoot, plan) => {
        log.push(
          `install:${plan.install.command} ${plan.install.args.join(" ")} os=${plan.install.supportedArchitectures.os.join(",")} cpu=${plan.install.supportedArchitectures.cpu.join(",")}`,
        );
      },
      materializeRuntimePackage: async (_workspace, _sourceRoot, pkg) => {
        log.push(`materialize:${pkg.name}`);
        if (input.unavailable?.includes(pkg.name)) return { available: false };
        if (input.wrongVersion?.includes(pkg.name)) {
          return { available: true, version: "9.9.9-wrong" };
        }
        return { available: true, version: pkg.version };
      },
      buildApplication: async () => {
        log.push("buildApplication");
      },
      buildNativeHelper: async (_workspace, _sourceRoot, plan) => {
        log.push(`cargo:${plan.cargo.args.join(" ")}`);
        return { sha256: digest("7"), byteLength: 64, version: "0.1.0" };
      },
      collectStagedFiles: async () => {
        const files = cleanStagedTree(input.targetKey);
        return input.injectViolation ? [...files, input.injectViolation] : files;
      },
      toolchainIdentity: async () => {
        log.push("toolchainIdentity");
        return fixtureToolchain;
      },
      repoRoot,
      osImage: fixtureMetadata.osImage,
      worker: { workerRef: "runner:fixture-01", hostClass: "owned-fixture" },
    };
  };

  test("stages any of the six targets from one host and emits a deterministic §9 ledger", async () => {
    for (const targetKey of desktopTargetKeys) {
      const descriptor = descriptorFor(targetKey);
      const io = fixtureIo({ targetKey });
      const first = await stageTarget(descriptor, io);
      const second = await stageTarget(descriptor, fixtureIo({ targetKey }));
      expect(first.ok).toBe(true);
      expect(second.ok).toBe(true);
      if (!first.ok || !second.ok) return;
      expect(first.ledgerDigest).toBe(second.ledgerDigest);
      expect(first.unsignedDev).toBe(false);
      expect(first.stagedTree).toBe(stagedTreePath(first.workspace));
      expect(first.ledger.lockfileSha256).toBe(lockfileSha256);
      expect(first.receiptDraft?.componentLedger.componentCount).toBe(
        first.ledger.components.length,
      );
      expect(first.receiptDraft?.gates).toEqual({ stagedTree: "pass" });
      // The plan's locked target-only install EXECUTED, before materialization.
      const installIndex = io.log.findIndex((line) => line.startsWith("install:pnpm install"));
      const materializeIndex = io.log.findIndex((line) => line.startsWith("materialize:"));
      expect(installIndex).toBeGreaterThanOrEqual(0);
      expect(io.log[installIndex]).toContain("--prod");
      expect(io.log[installIndex]).toContain("--frozen-lockfile");
      expect(io.log[installIndex]).toContain(`os=${desktopTargets[targetKey].platform}`);
      expect(io.log[installIndex]).toContain(`cpu=${desktopTargets[targetKey].arch}`);
      expect(materializeIndex).toBeGreaterThan(installIndex);
      expect(io.log).toContainEqual(
        `cargo:build --release -p oa-desktop-audio --target ${desktopTargets[targetKey].rustTargetTriple}`,
      );
    }
  });

  test("a lockfile identity mismatch fails typed BEFORE any install or build", async () => {
    const io = fixtureIo({ targetKey: "darwin-arm64", lockfileSha256: "c".repeat(64) });
    const result = await stageTarget(descriptorFor("darwin-arm64"), io);
    expect(result).toMatchObject({ ok: false, failure: "lockfile_mismatch" });
    expect(io.log.some((line) => line.startsWith("install:"))).toBe(false);
    expect(io.log).not.toContainEqual("buildApplication");
  });

  test("fails typed and BEFORE native build/maker work when a runtime package is unavailable", async () => {
    const io = fixtureIo({ targetKey: "win32-arm64", unavailable: ["@openai/codex-win32-arm64"] });
    const result = await stageTarget(descriptorFor("win32-arm64"), io);
    expect(result).toMatchObject({
      ok: false,
      failure: "missing_runtime_package",
      missingPackages: [`@openai/codex-win32-arm64@${pins.codex}-win32-arm64`],
    });
    expect(io.log).not.toContainEqual("buildApplication");
    expect(io.log.some((line) => line.startsWith("cargo:"))).toBe(false);
  });

  test("fails typed when a staged runtime version differs from the exact locked version", async () => {
    const io = fixtureIo({
      targetKey: "darwin-arm64",
      wrongVersion: ["@anthropic-ai/claude-agent-sdk-darwin-arm64"],
    });
    const result = await stageTarget(descriptorFor("darwin-arm64"), io);
    expect(result).toMatchObject({
      ok: false,
      failure: "runtime_version_mismatch",
      versionMismatches: [
        `@anthropic-ai/claude-agent-sdk-darwin-arm64: staged 9.9.9-wrong, locked ${pins.claudeAgentSdk}`,
      ],
    });
    expect(io.log).not.toContainEqual("buildApplication");
    expect(io.log.some((line) => line.startsWith("cargo:"))).toBe(false);
  });

  test("derives runtime pins from the EXPORTED source manifest before planning the install", async () => {
    const io = fixtureIo({ targetKey: "darwin-arm64" });
    const result = await stageTarget(descriptorFor("darwin-arm64"), io);
    expect(result.ok).toBe(true);
    const manifestIndex = io.log.indexOf("readSourceManifest");
    const exportIndex = io.log.findIndex((line) => line.startsWith("export:"));
    const installIndex = io.log.findIndex((line) => line.startsWith("install:"));
    expect(exportIndex).toBeGreaterThanOrEqual(0);
    expect(manifestIndex).toBeGreaterThan(exportIndex);
    expect(installIndex).toBeGreaterThan(manifestIndex);
  });

  test("fails closed on a staged-tree violation before any maker may run", async () => {
    const io = fixtureIo({
      targetKey: "darwin-arm64",
      injectViolation: {
        path: "node_modules/evil/bin/payload",
        byteLength: 16,
        executable: true,
        header: elf("x64"),
        sha256: digest("1"),
      },
    });
    const result = await stageTarget(descriptorFor("darwin-arm64"), io);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.failure).toBe("staged_tree_violations");
    expect(result.violations).toContainEqual(
      expect.objectContaining({ kind: "foreign_architecture_binary" }),
    );
  });

  test("unsigned-dev staging is conspicuous and never yields a receipt draft", async () => {
    const descriptor = descriptorFor("darwin-arm64", { signingPolicy: "unsigned-dev" });
    const result = await stageTarget(descriptor, fixtureIo({ targetKey: "darwin-arm64" }));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.unsignedDev).toBe(true);
    expect(result.receiptDraft).toBeUndefined();
  });
});

describe("DIST-03 packaged ASAR placement fidelity", () => {
  const descriptor = descriptorFor("darwin-arm64");
  const ledgerFor = (): ReturnType<typeof buildNativeComponentLedger> =>
    buildNativeComponentLedger(
      descriptor,
      nativeClosureEntries(
        descriptor,
        cleanStagedTree("darwin-arm64"),
        new Map(requiredRuntimePackages(descriptor, pins).map((pkg) => [pkg.name, pkg.version])),
        "0.1.0",
      ),
      fixtureMetadata,
    );
  const packagedEntriesFor = (
    ledger: ReturnType<typeof buildNativeComponentLedger>,
  ): Array<{ path: string; unpacked: boolean }> =>
    ledger.components
      .filter((component) => component.asarPlacement !== "extra-resource")
      .map((component) => ({
        path: component.destination,
        unpacked: component.asarPlacement === "unpacked",
      }));
  const extraResourcesFor = (
    ledger: ReturnType<typeof buildNativeComponentLedger>,
  ): ReadonlySet<string> =>
    new Set(
      ledger.components
        .filter((component) => component.asarPlacement === "extra-resource")
        .map((component) => extraResourceDestination(component.destination)),
    );

  test("a faithful package (packed/unpacked/extraResource all as planned) passes", () => {
    const ledger = ledgerFor();
    expect(
      asarPlacementViolations(ledger, packagedEntriesFor(ledger), extraResourcesFor(ledger)),
    ).toEqual([]);
  });

  test("a required-unpacked runtime executable packed inside app.asar FAILS", () => {
    const ledger = ledgerFor();
    const entries = packagedEntriesFor(ledger).map((entry) =>
      entry.path.endsWith("/claude") ? { ...entry, unpacked: false } : entry,
    );
    expect(asarPlacementViolations(ledger, entries, extraResourcesFor(ledger))).toContainEqual(
      expect.objectContaining({
        kind: "unexpected_asar_entry",
        detail: "planned-unpacked component was packed inside app.asar",
      }),
    );
  });

  test("a missing planned component and a leaked extra-resource both fail", () => {
    const ledger = ledgerFor();
    // Missing planned-unpacked component.
    const missing = packagedEntriesFor(ledger).filter((entry) => !entry.path.endsWith("/claude"));
    expect(asarPlacementViolations(ledger, missing, extraResourcesFor(ledger))).toContainEqual(
      expect.objectContaining({
        detail: "planned unpacked component is absent from the packaged app",
      }),
    );
    // Extra-resource helper leaked into app.asar AND absent from Resources.
    const leaked = [
      ...packagedEntriesFor(ledger),
      { path: "native/arm64/oa-desktop-audio", unpacked: false },
    ];
    const violations = asarPlacementViolations(ledger, leaked, new Set());
    expect(violations).toContainEqual(
      expect.objectContaining({
        detail: "planned extra-resource component leaked into app.asar",
      }),
    );
    expect(violations).toContainEqual(
      expect.objectContaining({
        detail: "planned extra-resource component is absent from Resources",
      }),
    );
  });

  test("extra-resource destinations map builtin-skills beside the asar", () => {
    expect(extraResourceDestination("native/arm64/oa-desktop-audio")).toBe(
      "native/arm64/oa-desktop-audio",
    );
    expect(extraResourceDestination("dist/builtin-skills/manifest.json")).toBe(
      "builtin-skills/manifest.json",
    );
  });
});

describe("DIST-03 Forge<->ledger binding (verifyStagedTreeAgainstLedger)", () => {
  const writeFixtureTree = (): {
    stagedTree: string;
    ledger: ReturnType<typeof buildNativeComponentLedger>;
    cleanup: () => void;
  } => {
    const stagedTree = mkdtempSync(path.join(tmpdir(), "oa-ledger-bind-"));
    const helperBytes = Buffer.concat([Buffer.from(machO("arm64")), Buffer.alloc(84)]);
    const helperPath = path.join(stagedTree, "native", "arm64", "oa-desktop-audio");
    mkdirSync(path.dirname(helperPath), { recursive: true });
    writeFileSync(helperPath, helperBytes);
    const descriptor = descriptorFor("darwin-arm64");
    const ledger = buildNativeComponentLedger(
      descriptor,
      [
        {
          name: "oa-desktop-audio",
          version: "0.1.0",
          targetKey: "darwin-arm64",
          sha256: createHash("sha256").update(helperBytes).digest("hex"),
          byteLength: helperBytes.byteLength,
          provenance: "workspace-crate",
          destination: "native/arm64/oa-desktop-audio",
          fileKind: "executable",
          architecture: "arm64",
          signingState: "undetermined",
          asarPlacement: "extra-resource",
        },
      ],
      fixtureMetadata,
    );
    return {
      stagedTree,
      ledger,
      cleanup: () => rmSync(stagedTree, { recursive: true, force: true }),
    };
  };

  test("binds descriptor fields, recomputes the ledger ref, and verifies CURRENT staged bytes", async () => {
    const { stagedTree, ledger, cleanup } = writeFixtureTree();
    try {
      const verified = await verifyStagedTreeAgainstLedger(
        stagedTree,
        descriptorFor("darwin-arm64"),
        ledger,
        nativeComponentLedgerRef(ledger),
      );
      expect(verified.ledgerRef).toMatch(/^sha256:[0-9a-f]{64}$/);
      expect(verified.ledger.components).toHaveLength(1);
      // Descriptor drift refuses.
      await expect(
        verifyStagedTreeAgainstLedger(
          stagedTree,
          descriptorFor("darwin-arm64", { version: "9.9.9" }),
          ledger,
          nativeComponentLedgerRef(ledger),
        ),
      ).rejects.toThrow(/version/);
    } finally {
      cleanup();
    }
  });

  test("a staging workspace mutated AFTER staging is refused — stale proof never reaches a maker", async () => {
    const { stagedTree, ledger, cleanup } = writeFixtureTree();
    try {
      const helperPath = path.join(stagedTree, "native", "arm64", "oa-desktop-audio");
      const mutated = Buffer.concat([Buffer.from(machO("arm64")), Buffer.alloc(84, 1)]);
      writeFileSync(helperPath, mutated);
      await expect(
        verifyStagedTreeAgainstLedger(
          stagedTree,
          descriptorFor("darwin-arm64"),
          ledger,
          nativeComponentLedgerRef(ledger),
        ),
      ).rejects.toThrow(/bytes changed since staging/);
      rmSync(helperPath);
      await expect(
        verifyStagedTreeAgainstLedger(
          stagedTree,
          descriptorFor("darwin-arm64"),
          ledger,
          nativeComponentLedgerRef(ledger),
        ),
      ).rejects.toThrow(/missing from the staged tree/);
    } finally {
      cleanup();
    }
  });

  test("a coherent staged-byte plus ledger mutation cannot replace the independently pinned trust root", async () => {
    const { stagedTree, ledger, cleanup } = writeFixtureTree();
    try {
      const expectedLedgerRef = nativeComponentLedgerRef(ledger);
      const helperPath = path.join(stagedTree, "native", "arm64", "oa-desktop-audio");
      const mutated = Buffer.concat([Buffer.from(machO("arm64")), Buffer.alloc(84, 7)]);
      writeFileSync(helperPath, mutated);
      const coherentlyMutatedLedger = {
        ...ledger,
        components: ledger.components.map((component) => ({
          ...component,
          sha256: createHash("sha256").update(mutated).digest("hex"),
          byteLength: mutated.byteLength,
        })),
      };
      await expect(
        verifyStagedTreeAgainstLedger(
          stagedTree,
          descriptorFor("darwin-arm64"),
          coherentlyMutatedLedger,
          expectedLedgerRef,
        ),
      ).rejects.toThrow(/does not match independently supplied expected/);
    } finally {
      cleanup();
    }
  });
});

describe("DIST-03 shipped packed-ASAR byte fidelity", () => {
  test("reads packed WASM from the actual archive and refuses byte drift", async () => {
    const fixtureRoot = mkdtempSync(path.join(tmpdir(), "oa-packed-byte-gate-"));
    try {
      const source = path.join(fixtureRoot, "source");
      const asarPath = path.join(fixtureRoot, "app.asar");
      const destination = "dist/runtime.wasm";
      const expectedBytes = Buffer.from("expected-wasm-bytes");
      mkdirSync(path.join(source, "dist"), { recursive: true });
      writeFileSync(path.join(source, destination), expectedBytes);
      await createPackage(source, asarPath);
      const descriptor = descriptorFor("darwin-arm64");
      const ledger = buildNativeComponentLedger(
        descriptor,
        [
          {
            name: "runtime-wasm",
            version: "1.0.0",
            targetKey: "darwin-arm64",
            sha256: createHash("sha256").update(expectedBytes).digest("hex"),
            byteLength: expectedBytes.byteLength,
            provenance: "workspace-crate",
            destination,
            fileKind: "wasm-module",
            architecture: "none",
            signingState: "not-applicable",
            asarPlacement: "asar",
          },
        ],
        fixtureMetadata,
      );
      await expect(
        verifyPackagedClosureBytes({ ledger, asarPath, resourcesPath: fixtureRoot }),
      ).resolves.toBe(1);

      rmSync(asarPath);
      writeFileSync(path.join(source, destination), Buffer.from("drifted-wasm-bytes!"));
      await createPackage(source, asarPath);
      await expect(
        verifyPackagedClosureBytes({ ledger, asarPath, resourcesPath: fixtureRoot }),
      ).rejects.toThrow(/does not match the staged ledger bytes/);
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
});
