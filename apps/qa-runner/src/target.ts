// The Target: a deployment seen from outside (executor-style).
//
// The same scenario can point at dev or prod by swapping the Target's baseUrl;
// the brain drives black-box against it. Capabilities are a small declared set
// so a brain step can be skipped if a target lacks a surface.

export type TargetCapability = "browser" | "terminal";

export interface Target {
  /** Stable name, e.g. "openagents.com-prod". */
  readonly name: string;
  /** Base URL the browser resolves relative navigations against. */
  readonly baseUrl: string;
  /** Declared surfaces this target exposes to the run. */
  readonly capabilities: ReadonlySet<TargetCapability>;
}

export function makeTarget(input: {
  readonly name: string;
  readonly baseUrl: string;
  readonly capabilities?: ReadonlyArray<TargetCapability>;
}): Target {
  return {
    name: input.name,
    baseUrl: input.baseUrl.replace(/\/$/, ""),
    capabilities: new Set(input.capabilities ?? ["browser"]),
  };
}

/** The live openagents.com production target. */
export const openagentsComProd = (): Target =>
  makeTarget({ name: "openagents.com-prod", baseUrl: "https://openagents.com", capabilities: ["browser"] });

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
