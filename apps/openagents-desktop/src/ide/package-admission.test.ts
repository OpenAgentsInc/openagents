import { readFileSync } from "node:fs";
import path from "node:path";

import { Exit, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { IdeIndexBenchmarkReceiptSchema } from "./index-benchmark-contract.ts";
import { ide01PackageDecisions, IdePackageDecisionSchema } from "./package-admission.ts";
import { IdePackageAuditReceiptSchema } from "./package-audit-contract.ts";
import { IdePackageSpikeMatrixReceiptSchema } from "./package-spike-contract.ts";
import { PierreDiffProjectionSchema, decodePierreDiffProjection } from "./pierre-diffs-adapter.tsx";
import {
  DesktopThemeProjectionSchema,
  tokyoNightDesktopThemeProjection,
} from "./tokyo-night-theme.ts";
import { khalaEditorDesktopThemeProjection } from "./khala-editor-theme.ts";
import {
  defaultDesktopEditorThemeId,
  desktopEditorThemeRegistry,
  fallbackDesktopEditorThemeId,
} from "./desktop-editor-themes.ts";
import { IdeVimEngineDecisionSchema, ide01VimDecision } from "./vim-mode-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "../..");
const repositoryRoot = path.resolve(appRoot, "../..");
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide");

const json = (file: string): unknown => JSON.parse(readFileSync(file, "utf8"));

const relativeLuminance = (hex: string): number => {
  const channels = hex
    .slice(1, 7)
    .match(/../gu)
    ?.map((pair) => Number.parseInt(pair, 16) / 255);
  if (channels === undefined || channels.length !== 3) throw new Error(`invalid color ${hex}`);
  const linear = channels.map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
};
const contrast = (left: string, right: string): number => {
  const leftLuminance = relativeLuminance(left);
  const rightLuminance = relativeLuminance(right);
  return (
    (Math.max(leftLuminance, rightLuminance) + 0.05) /
    (Math.min(leftLuminance, rightLuminance) + 0.05)
  );
};

describe("IDE-01 package admission", () => {
  test("pins adopted artifacts and leaves projection packages with no authority", () => {
    expect(
      ide01PackageDecisions.every((decision) =>
        Exit.isSuccess(Schema.decodeUnknownExit(IdePackageDecisionSchema)(decision)),
      ),
    ).toBe(true);
    const adopted = ide01PackageDecisions.filter((decision) => decision._tag === "Adopt");
    expect(
      adopted.map((decision) => `${decision.artifact.packageName}@${decision.artifact.version}`),
    ).toEqual(["monaco-editor@0.55.1", "@pierre/diffs@1.2.12"]);
    expect(
      adopted.every((decision) =>
        Object.values(decision.authority).every((hasAuthority) => hasAuthority === false),
      ),
    ).toBe(true);

    const manifest = json(path.join(appRoot, "package.json")) as {
      readonly dependencies: Readonly<Record<string, string>>;
    };
    expect(manifest.dependencies["monaco-editor"]).toBe("0.55.1");
    expect(manifest.dependencies["@pierre/diffs"]).toBe("1.2.12");
    expect(manifest.dependencies["monaco-vim"]).toBeUndefined();
    expect(manifest.dependencies["@replit/codemirror-vim"]).toBeUndefined();
    const lock = readFileSync(path.join(repositoryRoot, "pnpm-lock.yaml"), "utf8");
    expect(lock).not.toContain("monaco-vim");
    expect(lock).not.toContain("codemirror-vim");
  });

  test("selects the complete app-owned public-Monaco Vim contract", () => {
    expect(
      Exit.isSuccess(Schema.decodeUnknownExit(IdeVimEngineDecisionSchema)(ide01VimDecision)),
    ).toBe(true);
    expect(ide01VimDecision.defaultEnabled).toBe(false);
    expect(ide01VimDecision.dependencyPackages).toEqual([]);
    expect(ide01VimDecision.capabilities).toHaveLength(32);
    expect(new Set(ide01VimDecision.capabilities.map((entry) => entry.capability)).size).toBe(32);
  });

  test("keeps the Pierre adapter projection-only", () => {
    const decoded = decodePierreDiffProjection({
      schemaVersion: "openagents.desktop.pierre-diff-projection.v1",
      reviewRef: "ide.review.package-test",
      fileRef: "ide.file.package-test",
      patch: "@@ -1 +1 @@\n-old\n+new",
      mode: "unified",
      contextLines: 3,
      selection: null,
      annotations: [],
      root: "/private/workspace",
      grant: "secret-grant",
      apply: () => undefined,
    });
    expect(Exit.isSuccess(Schema.decodeUnknownExit(PierreDiffProjectionSchema)(decoded))).toBe(
      true,
    );
    expect(Object.keys(decoded).sort()).toEqual([
      "annotations",
      "contextLines",
      "fileRef",
      "mode",
      "patch",
      "reviewRef",
      "schemaVersion",
      "selection",
    ]);
  });

  test("uses a safe Khala default and retains Tokyo Night as the owned fallback", () => {
    expect(defaultDesktopEditorThemeId).toBe("khala-editor");
    expect(fallbackDesktopEditorThemeId).toBe("tokyo-night");
    expect(Object.keys(desktopEditorThemeRegistry).sort()).toEqual(["khala-editor", "tokyo-night"]);
    for (const projection of Object.values(desktopEditorThemeRegistry)) {
      expect(
        Exit.isSuccess(Schema.decodeUnknownExit(DesktopThemeProjectionSchema)(projection)),
      ).toBe(true);
      expect(projection.pierre.allowUnsafeCss).toBe(false);
      expect(projection.pierre.allowRemoteTheme).toBe(false);
      expect(projection.monaco.colors.editorBackground).toBe(projection.effectNative.background);
      expect(projection.terminal.background).toBe(projection.effectNative.background);
      for (const foreground of [
        projection.palette.foreground,
        projection.palette.foregroundMuted,
        projection.palette.foregroundFaint,
        projection.palette.blue,
        projection.palette.red,
        projection.palette.yellow,
        projection.palette.green,
      ]) {
        expect(contrast(foreground, projection.palette.background)).toBeGreaterThanOrEqual(4.5);
      }
      expect(Object.keys(projection.surfaces).sort()).toEqual([
        "browser",
        "debug",
        "output",
        "problems",
        "proposal",
        "review",
        "status",
      ]);
    }
    expect(khalaEditorDesktopThemeProjection.palette.background).toBe("#05070d");
    expect(khalaEditorDesktopThemeProjection.monaco.rules.map((rule) => rule.token)).toEqual(
      tokyoNightDesktopThemeProjection.monaco.rules.map((rule) => rule.token),
    );
    expect(khalaEditorDesktopThemeProjection.monaco.rules.filter((rule) => rule.token !== "function"))
      .toEqual(tokyoNightDesktopThemeProjection.monaco.rules.filter((rule) => rule.token !== "function"));
    const provenance = readFileSync(
      path.join(appRoot, "resources", "third-party", "tokyo-night", "PROVENANCE.md"),
      "utf8",
    );
    expect(provenance).toContain("7c0f11eaef322f293621ca7befe462214b7ea468");
    expect(provenance).toContain("#8990ad");
  });

  test("decodes generated package, audit, and TypeScript placement receipts", () => {
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(IdePackageSpikeMatrixReceiptSchema)(
          json(path.join(benchmarkRoot, "2026-07-19-ide-01-package-spike.json")),
        ),
      ),
    ).toBe(true);
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(IdePackageAuditReceiptSchema)(
          json(path.join(benchmarkRoot, "2026-07-19-ide-01-package-audit.json")),
        ),
      ),
    ).toBe(true);
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(IdeIndexBenchmarkReceiptSchema)(
          json(path.join(benchmarkRoot, "2026-07-19-ide-01-typescript-index.json")),
        ),
      ),
    ).toBe(true);
  });
});
