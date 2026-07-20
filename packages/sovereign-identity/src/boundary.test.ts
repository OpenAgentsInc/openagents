import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

/**
 * In-package import-boundary check.
 *
 * The repository `check:afs-boundaries` gate governs a fixed list of AFS
 * packages, so this package owns its own boundary proof. `sovereign-identity`
 * is a neutral root package: it must import no app, no Pylon or Desktop code, no
 * React or Electron platform API, and no wallet SDK. It may import Effect and
 * the audited crypto primitives (`@noble/*`, `@scure/*`) only.
 */

const srcRoot = import.meta.dirname;

const sourceFiles = (root: string): ReadonlyArray<string> =>
  readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(candidate);
    if (!/\.tsx?$/u.test(entry.name)) return [];
    if (/\.test\.tsx?$/u.test(entry.name)) return [];
    return [candidate];
  });

const importSpecifiers = (source: string): ReadonlyArray<string> => {
  const specifiers: string[] = [];
  const pattern =
    /(?:import|export)[^;]*?from\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/gu;
  for (const match of source.matchAll(pattern)) {
    const specifier = match[1] ?? match[2];
    if (specifier) specifiers.push(specifier);
  }
  return specifiers;
};

const FORBIDDEN_NEEDLES = [
  "apps/",
  "@openagentsinc/pylon",
  "pylon-runtime",
  "pylon-core",
  "@openagentsinc/probe",
  "-desktop",
  "electron",
  "react",
  "react-dom",
  "react-native",
  "expo",
  "monaco-editor",
  // Wallet SDKs — the shared root never links a wallet SDK; that is a bounded
  // adapter callback in a later packet.
  "@breeztech",
  "breez-sdk",
  "@buildonspark",
  "spark-sdk",
  "@ldk",
  "lightningdevkit",
  // Provider SDKs and cloud clients.
  "@anthropic",
  "openai",
  "@google-cloud",
  "googleapis",
  "@aws-sdk",
] as const;

const ALLOWED_PREFIXES = ["effect", "@noble/", "@scure/"] as const;

describe("package import boundary", () => {
  const files = sourceFiles(srcRoot);

  test("the package has source files to check", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  test("no source file imports an app, platform, wallet SDK, or cloud client", () => {
    const violations: string[] = [];
    for (const file of files) {
      for (const specifier of importSpecifiers(readFileSync(file, "utf8"))) {
        for (const needle of FORBIDDEN_NEEDLES) {
          if (specifier === needle || specifier.startsWith(needle)) {
            violations.push(`${path.basename(file)} -> ${specifier}`);
          }
        }
      }
    }
    expect(violations).toEqual([]);
  });

  test("every non-relative import is Effect or an audited crypto primitive", () => {
    const unexpected: string[] = [];
    for (const file of files) {
      for (const specifier of importSpecifiers(readFileSync(file, "utf8"))) {
        if (specifier.startsWith(".") || specifier.startsWith("node:")) continue;
        if (
          !ALLOWED_PREFIXES.some((prefix) => specifier === prefix || specifier.startsWith(prefix))
        ) {
          unexpected.push(`${path.basename(file)} -> ${specifier}`);
        }
      }
    }
    expect(unexpected).toEqual([]);
  });
});
