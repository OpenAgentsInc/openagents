import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

/**
 * AFS-00 package-boundary, subpath-export, and import-cycle check.
 *
 * It mirrors `apps/openagents-desktop/scripts/check-ide-boundaries.ts` for the
 * new AFS root packages. A root-core package must not import an app, a platform
 * API (Electron, Node, React, or React Native), a provider SDK, a SQL driver,
 * or a cloud client. The Apple FM package keeps Node host authority in its
 * `./node` subpath only. The turn kernel must not import the Apple FM adapter or
 * a concrete store.
 */
const repositoryRoot = path.resolve(import.meta.dirname, "..");
const packagesRoot = path.join(repositoryRoot, "packages");

type BoundaryViolation = Readonly<{
  file: string;
  rule: string;
  detail: string;
}>;

const relative = (file: string): string => path.relative(repositoryRoot, file);

const sourceFiles = (root: string): ReadonlyArray<string> => {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(candidate);
    return /\.tsx?$/u.test(entry.name) ? [candidate] : [];
  });
};

/** Extract every module specifier from `import`/`export ... from`/dynamic-import lines. */
export const importSpecifiers = (source: string): ReadonlyArray<string> => {
  const specifiers: string[] = [];
  const pattern = /(?:import|export)[^;]*?from\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/gu;
  for (const match of source.matchAll(pattern)) {
    const specifier = match[1] ?? match[2];
    if (specifier) specifiers.push(specifier);
  }
  return specifiers;
};

const APP_IMPORT_NEEDLES = [
  "apps/",
  "@openagentsinc/pylon",
  "@openagentsinc/probe",
  "pylon-runtime",
  "pylon-core",
  "blueprint-contracts",
  "/blueprint",
  "-desktop",
] as const;

const PLATFORM_IMPORT_NEEDLES = [
  "electron",
  "react",
  "react-dom",
  "react-native",
  "expo",
  "monaco-editor",
  "@pierre/",
] as const;

const PROVIDER_SDK_NEEDLES = [
  "@anthropic-ai",
  "@anthropic",
  "openai",
  "@openai",
  "@google/generative-ai",
  "grok-sdk",
  "@ai-sdk/",
] as const;

const SQL_DRIVER_NEEDLES = [
  "better-sqlite3",
  "node:sqlite",
  "postgres",
  "pg",
  "drizzle",
  "@openagentsinc/sqlite-runtime",
  "@openagentsinc/postgres-runtime",
] as const;

const CLOUD_CLIENT_NEEDLES = [
  "@google-cloud",
  "googleapis",
  "gcloud",
  "@aws-sdk",
  "aws-sdk",
  "@openagentsinc/cloud-contract",
] as const;

/** The AFS root packages this check governs, with their allowed intra-graph edges. */
const AFS_PACKAGES = [
  "ide-runtime",
  "agent-turn-runtime",
  "agent-turn-store",
  "apple-fm-runtime",
  "agent-surface",
] as const;

const AFS_PACKAGE_NAMES: Readonly<Record<(typeof AFS_PACKAGES)[number], string>> = {
  "ide-runtime": "@openagentsinc/ide-runtime",
  "agent-turn-runtime": "@openagentsinc/agent-turn-runtime",
  "agent-turn-store": "@openagentsinc/agent-turn-store",
  "apple-fm-runtime": "@openagentsinc/apple-fm-runtime",
  "agent-surface": "@openagentsinc/agent-surface",
};

const nodeImportAllowed = (pkg: string, file: string): boolean =>
  pkg === "apple-fm-runtime" && /(?:^|[/\\])node\.ts$/u.test(file);

const importsFromPackage = (source: string, packageName: string): boolean =>
  importSpecifiers(source).some(
    (specifier) => specifier === packageName || specifier.startsWith(`${packageName}/`),
  );

export const inspectAfsBoundaries = (): ReadonlyArray<BoundaryViolation> => {
  const violations: BoundaryViolation[] = [];

  for (const pkg of AFS_PACKAGES) {
    const src = path.join(packagesRoot, pkg, "src");
    for (const file of sourceFiles(src)) {
      const source = readFileSync(file, "utf8");
      const specifiers = importSpecifiers(source);

      for (const specifier of specifiers) {
        const isNode = specifier === "node" || specifier.startsWith("node:");
        if (isNode && !nodeImportAllowed(pkg, file)) {
          violations.push({
            file: relative(file),
            rule: "no-node-in-root-core",
            detail: `Root-core package ${pkg} imported a Node API (${specifier}). Node host authority lives only in the Apple FM ./node subpath.`,
          });
        }
        for (const needle of APP_IMPORT_NEEDLES) {
          if (specifier.includes(needle)) {
            violations.push({
              file: relative(file),
              rule: "no-app-import",
              detail: `Root-core package ${pkg} imported app or nested-app code (${specifier}).`,
            });
          }
        }
        for (const needle of PLATFORM_IMPORT_NEEDLES) {
          if (specifier === needle || specifier.startsWith(`${needle}/`) || specifier.startsWith(`${needle}`)) {
            violations.push({
              file: relative(file),
              rule: "no-platform-api-import",
              detail: `Root-core package ${pkg} imported a platform API (${specifier}).`,
            });
          }
        }
        for (const needle of PROVIDER_SDK_NEEDLES) {
          if (specifier === needle || specifier.startsWith(needle)) {
            violations.push({
              file: relative(file),
              rule: "no-provider-sdk-import",
              detail: `Root-core package ${pkg} imported a provider SDK (${specifier}).`,
            });
          }
        }
        for (const needle of SQL_DRIVER_NEEDLES) {
          if (specifier === needle || specifier.startsWith(needle)) {
            violations.push({
              file: relative(file),
              rule: "no-sql-driver-import",
              detail: `Root-core package ${pkg} imported a SQL driver (${specifier}).`,
            });
          }
        }
        for (const needle of CLOUD_CLIENT_NEEDLES) {
          if (specifier === needle || specifier.startsWith(needle)) {
            violations.push({
              file: relative(file),
              rule: "no-cloud-client-import",
              detail: `Root-core package ${pkg} imported a cloud client (${specifier}).`,
            });
          }
        }
      }

      if (/\bas\s+(?:any|unknown)\b/u.test(source)) {
        violations.push({
          file: relative(file),
          rule: "no-unsafe-casts",
          detail: `AFS authority code in ${pkg} may not recover type safety through unchecked casts.`,
        });
      }
    }
  }

  // The turn kernel must not import the Apple FM adapter or a concrete store.
  const turnSrc = path.join(packagesRoot, "agent-turn-runtime", "src");
  for (const file of sourceFiles(turnSrc)) {
    const source = readFileSync(file, "utf8");
    if (importsFromPackage(source, AFS_PACKAGE_NAMES["apple-fm-runtime"])) {
      violations.push({
        file: relative(file),
        rule: "turn-kernel-provider-free",
        detail: "agent-turn-runtime must not import @openagentsinc/apple-fm-runtime; Apple FM implements the provider port instead.",
      });
    }
    if (importsFromPackage(source, AFS_PACKAGE_NAMES["agent-turn-store"])) {
      violations.push({
        file: relative(file),
        rule: "turn-kernel-store-free",
        detail: "agent-turn-runtime must not import @openagentsinc/agent-turn-store; the store depends on the kernel, not the reverse.",
      });
    }
  }

  // Subpath-export validation: every declared export file must exist.
  for (const pkg of AFS_PACKAGES) {
    const manifestPath = path.join(packagesRoot, pkg, "package.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
      exports?: Record<string, string>;
    };
    const exportsMap = manifest.exports ?? {};
    for (const [subpath, target] of Object.entries(exportsMap)) {
      const resolved = path.join(packagesRoot, pkg, target);
      if (!existsSync(resolved)) {
        violations.push({
          file: relative(manifestPath),
          rule: "subpath-export-exists",
          detail: `Declared export "${subpath}" points at a missing file (${target}).`,
        });
      }
    }
    // A root export must not resolve to a Node subpath file.
    const rootTarget = exportsMap["."];
    if (rootTarget && /node\.ts$/u.test(rootTarget)) {
      violations.push({
        file: relative(manifestPath),
        rule: "root-export-portable",
        detail: "The root export must be portable and must not resolve to a Node host file.",
      });
    }
  }

  // Import-cycle check across the AFS packages (the graph must be a DAG).
  const edges = new Map<string, Set<string>>();
  const nameToDir = new Map<string, string>();
  for (const pkg of AFS_PACKAGES) nameToDir.set(AFS_PACKAGE_NAMES[pkg], pkg);
  for (const pkg of AFS_PACKAGES) {
    const src = path.join(packagesRoot, pkg, "src");
    const out = new Set<string>();
    for (const file of sourceFiles(src)) {
      for (const specifier of importSpecifiers(readFileSync(file, "utf8"))) {
        for (const [name, dir] of nameToDir) {
          if ((specifier === name || specifier.startsWith(`${name}/`)) && dir !== pkg) out.add(dir);
        }
      }
    }
    edges.set(pkg, out);
  }
  const visitState = new Map<string, "visiting" | "done">();
  const detectCycle = (node: string, stack: ReadonlyArray<string>): void => {
    if (visitState.get(node) === "done") return;
    if (visitState.get(node) === "visiting") {
      violations.push({
        file: `packages/${node}`,
        rule: "no-import-cycle",
        detail: `Import cycle across AFS packages: ${[...stack, node].join(" -> ")}.`,
      });
      return;
    }
    visitState.set(node, "visiting");
    for (const next of edges.get(node) ?? new Set()) detectCycle(next, [...stack, node]);
    visitState.set(node, "done");
  };
  for (const pkg of AFS_PACKAGES) detectCycle(pkg, []);

  return violations;
};

const main = (): void => {
  const violations = inspectAfsBoundaries();
  if (violations.length === 0) {
    console.log("[afs-boundaries] PASS — AFS root-package boundaries and exports are intact");
    return;
  }
  console.error(`[afs-boundaries] FAIL — ${violations.length} violation(s)`);
  for (const violation of violations) {
    console.error(`${violation.file} [${violation.rule}] ${violation.detail}`);
  }
  process.exitCode = 1;
};

if (import.meta.url === `file://${process.argv[1]}`) main();
