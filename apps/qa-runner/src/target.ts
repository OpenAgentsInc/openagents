// The Target: a deployment seen from outside (executor-style).
//
// The same scenario can point at dev or prod by swapping the Target's baseUrl;
// the brain drives black-box against it. Capabilities are a small declared set
// so a brain step can be skipped if a target lacks a surface.
//
// RESTRICTIONS (#6190, mirroring droid-control `config.yaml` environments):
// a target can carry RESTRICTIONS that bound what a run is allowed to DO to it.
// The headline one is `read-only` — "never create data" — for a prod target.
// Restrictions are enforced HONESTLY by the runner: a scenario that tries to
// mutate a read-only target fails with an explicit, recorded reason (no silent
// skip, no fabricated pass). This is policy, not a security boundary: the live
// site already protects itself; the restriction stops the harness from being the
// thing that mutates prod.

export type TargetCapability = "browser" | "terminal";

// A declared restriction that bounds what a run may do to a target.
//   read-only — the run MUST NOT mutate the target. Mutating browser steps
//               (click / type) are refused with an honest recorded failure.
//               A prod target carries this by default.
export type TargetRestriction = "read-only";

export interface Target {
  /** Stable name, e.g. "openagents.com-prod". */
  readonly name: string;
  /** Base URL the browser resolves relative navigations against. */
  readonly baseUrl: string;
  /** Declared surfaces this target exposes to the run. */
  readonly capabilities: ReadonlySet<TargetCapability>;
  /** Declared restrictions bounding what a run may do to this target (#6190). */
  readonly restrictions: ReadonlySet<TargetRestriction>;
}

export function makeTarget(input: {
  readonly name: string;
  readonly baseUrl: string;
  readonly capabilities?: ReadonlyArray<TargetCapability>;
  readonly restrictions?: ReadonlyArray<TargetRestriction>;
}): Target {
  return {
    name: input.name,
    baseUrl: input.baseUrl.replace(/\/$/, ""),
    capabilities: new Set(input.capabilities ?? ["browser"]),
    restrictions: new Set(input.restrictions ?? []),
  };
}

/** True when this target declares the `read-only` restriction (#6190). */
export const isReadOnly = (target: Target): boolean =>
  target.restrictions.has("read-only");

// ---------------------------------------------------------------------------
// Restriction enforcement policy (#6190) — pure, testable on its own.
// ---------------------------------------------------------------------------
//
// A read-only target must not be MUTATED by a run. We classify brain step kinds
// into mutating vs read-only and refuse a mutating step against a read-only
// target with an HONEST recorded reason (no silent skip, no fabricated pass).
//
//   read-only kinds : navigate, wait-for, screenshot, assert (observe only)
//   mutating  kinds : click, type (interactions that can create/change data)
//
// This is the policy droid-control encodes as `production: read-only`: prod is
// observed, never written. The runner consults `checkStepAllowed` before
// executing each step; a refusal is recorded as a failed step + a run failure.

/** Brain step kinds that can mutate the target (create/change data). */
export const MUTATING_STEP_KINDS: ReadonlySet<string> = new Set(["click", "type"]);

/** True when a step of this kind could mutate the target. */
export const isMutatingStepKind = (kind: string): boolean =>
  MUTATING_STEP_KINDS.has(kind);

/**
 * Decide whether a step of the given kind is allowed against the target. A
 * mutating step against a read-only target is REFUSED with an honest reason;
 * everything else is allowed. Pure: the runner uses this to record the refusal
 * and fail the run, rather than letting the harness mutate a read-only target.
 */
export function checkStepAllowed(
  target: Target,
  kind: string,
): { readonly allowed: true } | { readonly allowed: false; readonly reason: string } {
  if (isReadOnly(target) && isMutatingStepKind(kind)) {
    return {
      allowed: false,
      reason:
        `restriction violation: target "${target.name}" is read-only ` +
        `(never create data) but step kind "${kind}" mutates it`,
    };
  }
  return { allowed: true };
}

/** The live openagents.com production target. Read-only by default: a prod run
 *  must never create data (#6190), so the harness refuses mutating steps. */
export const openagentsComProd = (): Target =>
  makeTarget({
    name: "openagents.com-prod",
    baseUrl: "https://openagents.com",
    capabilities: ["browser"],
    restrictions: ["read-only"],
  });

/** Resolve a target by name or explicit base URL (env-overridable). */
export function resolveTarget(
  env: Readonly<Record<string, string | undefined>> = process.env,
): Target {
  const explicit = env.QA_TARGET_URL;
  if (explicit) {
    return makeTarget({ name: env.QA_TARGET_NAME ?? explicit, baseUrl: explicit, capabilities: ["browser"] });
  }
  return openagentsComProd();
}
