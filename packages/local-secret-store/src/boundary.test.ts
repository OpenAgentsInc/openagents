import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

/**
 * In-package import-boundary, subpath-export, and no-cycle check.
 *
 * `local-secret-store` is a neutral leaf package. It stores opaque encrypted
 * bytes by locator. It must import no app, no Pylon or Desktop code, no React or
 * Electron platform API, no wallet SDK, and no cloud client. Above all it must
 * know NOTHING about Nostr or Spark derivation, so it may not import
 * `@openagentsinc/sovereign-identity` (that would also be an import cycle) nor a
 * BIP-32/BIP-39/Nostr crypto primitive. It may import Effect only.
 */

const srcRoot = import.meta.dirname;
const packageRoot = path.resolve(srcRoot, "..");

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
  // Identity meaning must not leak into the neutral secret store. Importing the
  // sovereign-identity package would also create an import cycle.
  "@openagentsinc/sovereign-identity",
  // Nostr and Spark derivation primitives. This package must not know them.
  "@noble/",
  "@scure/",
  "nostr-tools",
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

const ALLOWED_PREFIXES = ["effect"] as const;

describe("local-secret-store package boundary", () => {
  const files = sourceFiles(srcRoot);

  test("the package has source files to check", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  test("no source file imports an app, platform, identity, wallet, or cloud module", () => {
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

  test("every non-relative import is Effect", () => {
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

  test("every declared subpath export points at a file that exists", () => {
    const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8")) as {
      exports?: Record<string, string>;
    };
    const missing: string[] = [];
    for (const [subpath, target] of Object.entries(manifest.exports ?? {})) {
      if (!existsSync(path.join(packageRoot, target))) missing.push(`${subpath} -> ${target}`);
    }
    expect(missing).toEqual([]);
  });
});
