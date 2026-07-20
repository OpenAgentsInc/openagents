import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

/**
 * In-package import-boundary, subpath-export, and cross-package cycle check.
 *
 * The repository `check:afs-boundaries` gate governs a fixed list of AFS
 * packages, so this package owns its own boundary proof. `sovereign-identity`
 * is a neutral root package: it must import no app, no Pylon or Desktop code, no
 * React or Electron platform API, and no wallet SDK. It may import Effect, the
 * audited crypto primitives (`@noble/*`, `@scure/*`), and the neutral
 * `@openagentsinc/local-secret-store` port only.
 */

const srcRoot = import.meta.dirname;
const packageRoot = path.resolve(srcRoot, "..");
const packagesRoot = path.resolve(packageRoot, "..");

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

const ALLOWED_PREFIXES = [
  "effect",
  "@noble/",
  "@scure/",
  "@openagentsinc/local-secret-store",
] as const;

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

/**
 * The two IDR packages must form a DAG. `sovereign-identity` may depend on
 * `local-secret-store`; `local-secret-store` must never depend back. This test
 * reads both packages' source directly, so the graph stays acyclic without a
 * repository-wide gate.
 */
describe("sovereign-identity and local-secret-store form a DAG", () => {
  const IDR_PACKAGE_NAMES: Readonly<Record<string, string>> = {
    "sovereign-identity": "@openagentsinc/sovereign-identity",
    "local-secret-store": "@openagentsinc/local-secret-store",
  };

  const packageImports = (pkgDir: string): ReadonlySet<string> => {
    const src = path.join(packagesRoot, pkgDir, "src");
    const out = new Set<string>();
    if (!existsSync(src)) return out;
    for (const file of sourceFiles(src)) {
      for (const specifier of importSpecifiers(readFileSync(file, "utf8"))) {
        for (const [dir, name] of Object.entries(IDR_PACKAGE_NAMES)) {
          if ((specifier === name || specifier.startsWith(`${name}/`)) && dir !== pkgDir) {
            out.add(dir);
          }
        }
      }
    }
    return out;
  };

  test("local-secret-store does not import sovereign-identity", () => {
    expect([...packageImports("local-secret-store")]).toEqual([]);
  });

  test("the import graph across the two IDR packages is acyclic", () => {
    const edges = new Map<string, ReadonlySet<string>>([
      ["sovereign-identity", packageImports("sovereign-identity")],
      ["local-secret-store", packageImports("local-secret-store")],
    ]);
    const state = new Map<string, "visiting" | "done">();
    const cycles: string[] = [];
    const visit = (node: string, stack: ReadonlyArray<string>): void => {
      if (state.get(node) === "done") return;
      if (state.get(node) === "visiting") {
        cycles.push([...stack, node].join(" -> "));
        return;
      }
      state.set(node, "visiting");
      for (const next of edges.get(node) ?? new Set<string>()) visit(next, [...stack, node]);
      state.set(node, "done");
    };
    for (const node of edges.keys()) visit(node, []);
    expect(cycles).toEqual([]);
  });
});
