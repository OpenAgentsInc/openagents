// The multi-target REGISTRY (#6190, Rhys req #3).
//
// "The same (or near-same) test runs against multiple targets — 'test the dev
// server,' 'test production' — without rewriting it." A scenario is written ONCE
// (a brain step list / commitments) and pointed at a NAMED target from this
// registry. No scenario rewrite, no per-target code: the registry is config.
//
// This mirrors droid-control's `config.yaml` ENVIRONMENTS model: named
// environments (dev / staging / prod / selfhost), each with a base URL,
// declared capabilities, and RESTRICTIONS (the headline being `prod: read-only —
// never create data`). The runner enforces those restrictions honestly so a
// scenario that tries to mutate a read-only target fails with a recorded reason
// rather than silently creating data on prod.
//
// HONEST DEFAULTS:
//   - `prod` is read-only by default. A prod run must never create data, so the
//     harness refuses mutating steps (click / type) against it.
//   - `selfhost` has NO baked-in URL: a self-hosted deployment lives wherever the
//     operator runs it, so its base URL MUST come from env (`QA_SELFHOST_URL`).
//     Resolving `selfhost` without that env is an explicit, honest error — never
//     a guessed/placeholder URL.
//   - Every target's base URL is env-overridable (`QA_<NAME>_URL`) so the same
//     registry attaches to a live local/preview instance without an edit.

import { makeTarget, type Target, type TargetCapability, type TargetRestriction } from "./target";

// A NAMED target in the registry. The names mirror droid-control environments.
export type TargetName = "dev" | "staging" | "prod" | "selfhost";

export const TARGET_NAMES: ReadonlyArray<TargetName> = ["dev", "staging", "prod", "selfhost"];

export const isTargetName = (value: string): value is TargetName =>
  (TARGET_NAMES as ReadonlyArray<string>).includes(value);

// The declarative spec for one registry entry (config, not code). `baseUrl` is
// optional: a target with no default URL (selfhost) MUST be supplied via env.
export interface TargetSpec {
  readonly name: TargetName;
  /** Human description (appears in the registry listing / honest errors). */
  readonly description: string;
  /** Default base URL. Omitted for targets that have no canonical home. */
  readonly baseUrl?: string;
  readonly capabilities: ReadonlyArray<TargetCapability>;
  readonly restrictions: ReadonlyArray<TargetRestriction>;
}

// The registry: dev / staging / prod / selfhost. PROD is read-only (never create
// data). dev + staging are writable (safe to mutate). selfhost has no default
// URL (operator-supplied) and is writable.
export const TARGET_REGISTRY: { readonly [K in TargetName]: TargetSpec } = {
  dev: {
    name: "dev",
    description: "Local dev server — writable; safe to create/mutate test data.",
    baseUrl: "http://localhost:5173",
    capabilities: ["browser"],
    restrictions: [],
  },
  staging: {
    name: "staging",
    description: "Staging deployment — writable; safe to create/mutate test data.",
    baseUrl: "https://staging.openagents.com",
    capabilities: ["browser"],
    restrictions: [],
  },
  prod: {
    name: "prod",
    description: "Live production (openagents.com) — READ-ONLY: never create data.",
    baseUrl: "https://openagents.com",
    capabilities: ["browser"],
    restrictions: ["read-only"],
  },
  selfhost: {
    name: "selfhost",
    description:
      "A self-hosted deployment — writable; base URL is operator-supplied (QA_SELFHOST_URL).",
    capabilities: ["browser"],
    restrictions: [],
  },
};

// The default target when none is selected. Read from env so the registry's
// default is operator-configurable without code; falls back to `dev` (the safest
// writable target). An unknown `QA_DEFAULT_TARGET` is an honest error, not a
// silent fallback.
export function defaultTargetName(
  env: Readonly<Record<string, string | undefined>> = process.env,
): TargetName {
  const requested = env.QA_DEFAULT_TARGET;
  if (requested === undefined) return "dev";
  if (!isTargetName(requested)) {
    throw new Error(
      `QA_DEFAULT_TARGET="${requested}" is not a known target. Known: ${TARGET_NAMES.join(", ")}.`,
    );
  }
  return requested;
}

// The env var that overrides a target's base URL, e.g. QA_DEV_URL / QA_PROD_URL.
const urlEnvKey = (name: TargetName): string => `QA_${name.toUpperCase()}_URL`;

/**
 * Resolve a NAMED registry target into a concrete `Target`. The base URL is
 * taken from env (`QA_<NAME>_URL`) when set, else the spec default. A target
 * with no default URL and no env override (selfhost) is an HONEST error — never
 * a guessed URL. Capabilities + restrictions come from the spec (prod stays
 * read-only).
 */
export function resolveRegistryTarget(
  name: TargetName,
  env: Readonly<Record<string, string | undefined>> = process.env,
): Target {
  const spec = TARGET_REGISTRY[name];
  const fromEnv = env[urlEnvKey(name)];
  const baseUrl = fromEnv ?? spec.baseUrl;
  if (baseUrl === undefined || baseUrl.length === 0) {
    throw new Error(
      `target "${name}" has no base URL: set ${urlEnvKey(name)} (${spec.description}).`,
    );
  }
  return makeTarget({
    name,
    baseUrl,
    capabilities: spec.capabilities,
    restrictions: spec.restrictions,
  });
}

/**
 * Resolve a SELECTION of registry targets (the multi-target axis). Used to run
 * one scenario across N targets from a single definition. Order + de-duplication
 * are preserved/applied so a selection like `["dev","prod","dev"]` resolves to
 * `[dev, prod]`. An unknown name is an honest error.
 */
export function resolveSelectedTargets(
  names: ReadonlyArray<string>,
  env: Readonly<Record<string, string | undefined>> = process.env,
): ReadonlyArray<Target> {
  const seen = new Set<string>();
  const out: Target[] = [];
  for (const raw of names) {
    if (!isTargetName(raw)) {
      throw new Error(
        `unknown target "${raw}". Known: ${TARGET_NAMES.join(", ")}.`,
      );
    }
    if (seen.has(raw)) continue;
    seen.add(raw);
    out.push(resolveRegistryTarget(raw, env));
  }
  return out;
}

/**
 * Parse a comma/space-separated target selection string (e.g. "dev,prod" or
 * "dev prod"). Empty/whitespace-only -> the default target. Honest errors on
 * unknown names (via `resolveSelectedTargets`). Returns the resolved targets.
 */
export function parseTargetSelection(
  selection: string | undefined,
  env: Readonly<Record<string, string | undefined>> = process.env,
): ReadonlyArray<Target> {
  const names = (selection ?? "")
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  if (names.length === 0) return [resolveRegistryTarget(defaultTargetName(env), env)];
  return resolveSelectedTargets(names, env);
}
