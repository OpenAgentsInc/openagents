import { describe, expect, test } from "vite-plus/test";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const read = (relative: string): string => readFileSync(path.join(root, relative), "utf8");
const normalized = (text: string): string => text.replace(/\s+/gu, " ");

const index = read("docs/deploy/README.md");
const spec = read("docs/deploy/openagents-desktop-cross-platform-release.md");
const runbook = read("docs/deploy/openagents-desktop-production-release.md");
const invariants = read("INVARIANTS.md");

const expectLocalLinksExist = (sourcePath: string, markdown: string): void => {
  for (const match of markdown.matchAll(/\[[^\]]+\]\((?!https?:|#)([^)]+)\)/gu)) {
    const target = match[1]!.split("#", 1)[0]!;
    expect(existsSync(path.resolve(root, path.dirname(sourcePath), target))).toBe(true);
  }
};

describe("Desktop cross-platform release documentation contract", () => {
  test("routes deployment surfaces without merging their authorities", () => {
    for (const target of [
      "./openagents-desktop-cross-platform-release.md",
      "./openagents-desktop-production-release.md",
      "./openagents-mobile-production-release.md",
      "./agent-computer-production.md",
      "./openagents-audio-retention.md",
      "../../apps/openagents.com/AGENTS.md",
    ]) {
      expect(index).toContain(`(${target})`);
      expect(existsSync(path.resolve(root, "docs/deploy", target))).toBe(true);
    }
    expect(index).toContain("Never Desktop signing or update authority");
  });

  test("keeps every relative deployment-document link dereferenceable", () => {
    expectLocalLinksExist("docs/deploy/README.md", index);
    expectLocalLinksExist("docs/deploy/openagents-desktop-cross-platform-release.md", spec);
    expectLocalLinksExist("docs/deploy/openagents-desktop-production-release.md", runbook);
  });

  test("freezes the complete identity, target, package, trust, and support boundary", () => {
    expect(spec).toContain("ProductSpec version: 1.2.0");
    expect(spec).toContain("Date: 2026-07-18");
    expect(normalized(spec)).toContain(
      "none of the five target keys is admitted as cross-platform supported",
    );

    for (const target of [
      "darwin-arm64",
      "darwin-x64",
      "win32-x64",
      "linux-arm64",
      "linux-x64",
    ])
      expect(spec).toContain(`\`${target}\``);

    for (const format of ["DMG", "ZIP", "NSIS", "AppImage", "DEB", "RPM"]) {
      expect(spec).toContain(format);
    }

    for (const identity of [
      "com.openagents.desktop.rc",
      "OpenAgents.Desktop.RC",
      "com.openagents.desktop.rc.desktop",
      "openagents-desktop-rc",
      "openagents-rc",
      "OpenAgents, Inc.",
      "HQWSG26L43",
    ])
      expect(spec).toContain(identity);

    expect(spec).toContain("Per-user, one-click NSIS installer");
    expect(spec).toContain("No APT, YUM, DNF, or other package repository is in scope");
    expect(spec).toContain("DEB/RPM have **no in-app rollback claim**");
    expect(spec).toContain("GitHub Actions and GitHub-hosted CI are prohibited");
  });

  test("records minimum systems, immutable names, runner admission, retention, and telemetry semantics", () => {
    for (const policy of [
      "macOS 13.5 Ventura",
      "Windows 10 22H2 x64",
      "Windows is x64-only",
      "glibc 2.35, Linux kernel 5.15",
      "OpenAgents-<version>-<channel>-darwin-<arch>.dmg",
      "OpenAgents-<version>-<channel>-win32-<arch>-setup.exe",
      "OpenAgents-<version>-<channel>-linux-<arch>.AppImage",
      "not admitted by this spec",
      "seven years and never less than current plus N-1",
      "180 days after supersession and never while current",
      "Telemetry counts successful resolver/download responses, not installs",
    ])
      expect(normalized(spec)).toContain(policy);
  });

  test("links the source audit, parent, and every delivery issue", () => {
    expect(spec).toContain(
      "../teardowns/2026-07-16-t3-code-opencode-electron-build-update-analysis.md",
    );
    for (const issue of [
      8913, 8914, 8915, 8916, 8917, 8918, 8919, 8920, 8921, 8922, 8923, 8924, 8925, 8926, 8927,
    ]) {
      expect(spec).toContain(`https://github.com/OpenAgentsInc/openagents/issues/${issue}`);
    }
  });

  test("keeps the runbook subordinate to the spec and current support truth", () => {
    expect(normalized(runbook)).toContain(
      "ProductSpec owns identities, targets, formats, trust, support, and rollback claims",
    );
    expect(runbook).toContain("Current macOS arm64 v1 compatibility procedure");
    expect(runbook).toContain("MUST NOT synthesize missing native receipts");
    expect(runbook).toContain("complete known-good mobile export");
    expect(runbook).toContain("strictly newer fixed release");
  });

  test("freezes the one-command entrypoint and dual-changelog names before their implementations", () => {
    for (const contract of [
      "pnpm run release -- --channel <stable|rc> --version <semver>",
      "node --import tsx scripts/release.ts",
      "--resume <transaction-ref>",
      "docs/changelog/UNRELEASED.md",
      "docs/changelog/human/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md",
      "docs/changelog/agent/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md",
      "docs/deploy/receipts/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md",
      "indefinite",
    ])
      expect(spec).toContain(contract);
    expect(runbook).toContain("Converge, generate changelogs, and sign ReleaseSet v2");
    expect(runbook).toContain("homepage download CTAs and `/changelog`");
  });

  test("pairs material invariants with automated and release evidence boundaries", () => {
    expect(invariants).toContain("(DIST-01, #8914)");
    for (const boundary of [
      "intended automated boundary",
      "release boundary",
      "GitHub Actions and GitHub-hosted CI remain prohibited",
      "pre- and post-promotion mobile manifest/asset receipts",
      "`/download` resolves only the verified promoted Desktop ReleaseSet",
    ])
      expect(normalized(invariants)).toContain(boundary);
  });
});
