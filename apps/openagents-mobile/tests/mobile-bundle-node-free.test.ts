/**
 * The mobile bundle must not reach a Node-only module.
 *
 * This exists because of a real failure that every other gate passed. The
 * owner-private read model imported the bare `@openagentsinc/sarah` barrel,
 * whose index re-exports `nostr-migration`, which imported `node:crypto`.
 * Typecheck passed, 3300+ mobile tests passed, and the app built and installed
 * — then died on launch with "Unable to resolve module node:crypto" and a red
 * screen instead of the workroom.
 *
 * Node-hosted tests can never catch this: they run where `node:crypto`
 * resolves. So the guard is static, walking the same import graph Metro walks.
 */
import { readFileSync } from "node:fs";
import { readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { describe, expect, test } from "vite-plus/test";

const REPO_ROOT = resolve(import.meta.dirname, "../../..");
const MOBILE_SRC = join(REPO_ROOT, "apps/openagents-mobile/src");
const WORKSPACE_PREFIX = "@openagentsinc/";

/** Modules React Native does not provide. A bundle reaching one is broken. */
const NODE_BUILTIN = /^node:/;

const sourceFiles = (root: string): Array<string> => {
  const out: Array<string> = [];
  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry);
      if (statSync(path).isDirectory()) {
        walk(path);
        continue;
      }
      if (/\.(ts|tsx)$/.test(entry) && !/\.test\.tsx?$/.test(entry)) out.push(path);
    }
  };
  walk(root);
  return out;
};

const IMPORT_RE = /(?:from|import)\s*["']([^"']+)["']/g;

const importsOf = (file: string): Array<string> => {
  const text = readFileSync(file, "utf8");
  const found: Array<string> = [];
  for (const match of text.matchAll(IMPORT_RE)) {
    const specifier = match[1];
    if (specifier !== undefined) found.push(specifier);
  }
  return found;
};

/** Resolve a workspace package subpath to a file, using its exports map. */
const resolveWorkspace = (specifier: string): string | null => {
  const withoutScope = specifier.slice(WORKSPACE_PREFIX.length);
  const [name, ...rest] = withoutScope.split("/");
  const packageJsonPath = join(REPO_ROOT, "packages", name ?? "", "package.json");
  let manifest: { exports?: Record<string, string> };
  try {
    manifest = JSON.parse(readFileSync(packageJsonPath, "utf8")) as typeof manifest;
  } catch {
    // Not a packages/* workspace member (apps/*, or an external dependency).
    return null;
  }
  const subpath = rest.length === 0 ? "." : `./${rest.join("/")}`;
  const target = manifest.exports?.[subpath];
  if (target === undefined) return null;
  return join(REPO_ROOT, "packages", name ?? "", target.replace(/^\.\//, ""));
};

const resolveRelative = (fromFile: string, specifier: string): string | null => {
  const base = resolve(dirname(fromFile), specifier);
  for (const candidate of [base, `${base}.ts`, `${base}.tsx`, join(base, "index.ts")]) {
    try {
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // Try the next shape.
    }
  }
  return null;
};

interface Violation {
  readonly builtin: string;
  readonly path: ReadonlyArray<string>;
}

const relative = (file: string): string => file.slice(REPO_ROOT.length + 1);

/** Walk the graph breadth-first so a reported path is the shortest one. */
const findNodeBuiltinReach = (entries: ReadonlyArray<string>): Array<Violation> => {
  const violations: Array<Violation> = [];
  const seen = new Set<string>(entries);
  const queue: Array<{ file: string; trail: ReadonlyArray<string> }> = entries.map((file) => ({
    file,
    trail: [file],
  }));

  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined) break;
    for (const specifier of importsOf(current.file)) {
      if (NODE_BUILTIN.test(specifier)) {
        violations.push({
          builtin: specifier,
          path: current.trail.map(relative),
        });
        continue;
      }
      const next = specifier.startsWith(".")
        ? resolveRelative(current.file, specifier)
        : specifier.startsWith(WORKSPACE_PREFIX)
          ? resolveWorkspace(specifier)
          : null;
      if (next === null || seen.has(next)) continue;
      seen.add(next);
      queue.push({ file: next, trail: [...current.trail, next] });
    }
  }
  return violations;
};

describe("the mobile bundle stays free of Node-only modules", () => {
  test("no module reachable from apps/openagents-mobile/src imports a node: builtin", () => {
    const violations = findNodeBuiltinReach(sourceFiles(MOBILE_SRC));

    // Report the whole import chain, because the offending file is usually
    // several hops away from the mobile module that pulled it in.
    const rendered = violations.map(
      (violation) => `${violation.builtin} via\n    ${violation.path.join("\n    -> ")}`,
    );
    expect(rendered).toEqual([]);
  });

  test("the guard actually detects a node: import", () => {
    // A guard that cannot fail proves nothing. `generate-receipt` is a
    // Node-only build helper inside the same package the mobile app consumes,
    // so it is a genuine positive control.
    const violations = findNodeBuiltinReach([
      join(REPO_ROOT, "packages/sarah/src/nostr-journey/generate-receipt.ts"),
    ]);
    expect(violations.length).toBeGreaterThan(0);
    expect(violations.some((violation) => violation.builtin.startsWith("node:"))).toBe(true);
  });
});
