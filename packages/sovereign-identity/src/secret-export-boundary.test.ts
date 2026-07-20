import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import * as packageBarrel from "./index.ts";

/**
 * IDR-06 static secret-export boundary proof.
 *
 * The whole point of IDR-06 is that normal Pylon and Desktop code cannot reach
 * the raw private material — only the signer operations. This test proves two
 * things statically, with no live secret and no Keychain probe:
 *
 * 1. The `@openagentsinc/sovereign-identity` package barrel exposes the SIGNER
 *    surface but NOT the secret-export custody implementation. The custody
 *    key-export functions live in `machinery/custody.ts`, which is imported by
 *    NO barrel.
 * 2. Normal caller source (`packages/pylon-core`, `apps/openagents-desktop`)
 *    never imports the custody module and never references the key-export escape
 *    hatches (`exportPrivateKeyBytes` / `exportNsec` / `makeCustodyKeyExport` /
 *    `custodyKeyExportLayer` / `asLocalKeySigner`).
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */

const srcRoot = import.meta.dirname;
const packagesRoot = path.resolve(srcRoot, "../..");
const repoRoot = path.resolve(packagesRoot, "..");

const sourceFiles = (root: string): ReadonlyArray<string> => {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(candidate);
    return /\.tsx?$/u.test(entry.name) ? [candidate] : [];
  });
};

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

/** The two normal-caller roots that must live on signer operations only. */
const NORMAL_CALLER_ROOTS = [
  path.join(repoRoot, "packages", "pylon-core", "src"),
  path.join(repoRoot, "apps", "openagents-desktop", "src"),
] as const;

/** Secret-export symbols a normal caller must never name. */
const FORBIDDEN_SECRET_SYMBOLS = [
  "exportPrivateKeyBytes",
  "exportNsec",
  "makeCustodyKeyExport",
  "custodyKeyExportLayer",
  "asLocalKeySigner",
] as const;

/** Import specifiers that reach the isolated custody module. */
const CUSTODY_MODULE_NEEDLES = ["machinery/custody", "/custody.ts", "/custody.js"] as const;

describe("IDR-06 the package barrel exposes the signer, not the secret-export custody", () => {
  test("the signer surface is exported", () => {
    expect("deriveLocalNostrIdentity" in packageBarrel).toBe(true);
    expect("makeSovereignSignerFromMnemonic" in packageBarrel).toBe(true);
    expect("sovereignSignerFromMnemonicLayer" in packageBarrel).toBe(true);
    expect("SovereignSigner" in packageBarrel).toBe(true);
  });

  test("the secret-export custody IMPLEMENTATION is NOT reachable through the barrel", () => {
    expect("makeCustodyKeyExport" in packageBarrel).toBe(false);
    expect("custodyKeyExportLayer" in packageBarrel).toBe(false);
  });

  test("no barrel re-exports the custody module", () => {
    const violations: string[] = [];
    for (const barrel of ["index.ts", "machinery/index.ts", "contract/index.ts"]) {
      // Inspect real `export ... from` / `import` specifiers only, so a prose
      // comment that names the custody path never trips the check.
      for (const specifier of importSpecifiers(readFileSync(path.join(srcRoot, barrel), "utf8"))) {
        if (CUSTODY_MODULE_NEEDLES.some((needle) => specifier.includes(needle))) {
          violations.push(`${barrel} -> ${specifier}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });
});

describe("IDR-06 normal Pylon/Desktop code cannot import secret-export methods", () => {
  const normalFiles = NORMAL_CALLER_ROOTS.flatMap(sourceFiles).filter(
    (file) => !/secret-export-boundary\.test\.tsx?$/u.test(file),
  );

  test("there are normal-caller files to check", () => {
    expect(normalFiles.length).toBeGreaterThan(0);
  });

  test("no normal-caller file imports the isolated custody module", () => {
    const violations: string[] = [];
    for (const file of normalFiles) {
      for (const specifier of importSpecifiers(readFileSync(file, "utf8"))) {
        if (CUSTODY_MODULE_NEEDLES.some((needle) => specifier.includes(needle))) {
          violations.push(`${path.relative(repoRoot, file)} -> ${specifier}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });

  test("no normal-caller file references a key-export escape-hatch symbol", () => {
    const violations: string[] = [];
    for (const file of normalFiles) {
      const source = readFileSync(file, "utf8");
      for (const symbol of FORBIDDEN_SECRET_SYMBOLS) {
        if (source.includes(symbol)) {
          violations.push(`${path.relative(repoRoot, file)} -> ${symbol}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });
});
