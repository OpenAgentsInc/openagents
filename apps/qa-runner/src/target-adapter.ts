// Third-party Target adapter contract (#8069).
//
// A Target adapter is the minimal public-safe description QA Swarm needs to run
// an arbitrary customer app: where the app lives, how auth/fresh identity are
// prepared, whether the app can be restarted, and which production policy must
// be enforced. The adapter is data, not bespoke integration code.

import { Schema as S } from "effect";
import {
  makeTarget,
  type Target,
  type TargetCapability,
  type TargetRestriction,
} from "./target";

export const TARGET_ADAPTER_SCHEMA_VERSION =
  "openagents.qa_runner.target_adapter.v1" as const;

export const TargetAdapterEnvironment = S.Literals([
  "local",
  "preview",
  "staging",
  "prod",
  "fixture",
]);
export type TargetAdapterEnvironment = typeof TargetAdapterEnvironment.Type;

export const TargetAdapterOwner = S.Literals(["first-party", "external"]);
export type TargetAdapterOwner = typeof TargetAdapterOwner.Type;

export const TargetAdapterAuthKind = S.Literals([
  "none",
  "env",
  "device",
  "seeded-test-account",
]);
export type TargetAdapterAuthKind = typeof TargetAdapterAuthKind.Type;

export const TargetAdapterRestartKind = S.Literals([
  "none",
  "command",
  "http",
]);
export type TargetAdapterRestartKind = typeof TargetAdapterRestartKind.Type;

export const TargetAdapterProdPolicy = S.Literals(["read-only", "blocked"]);
export type TargetAdapterProdPolicy = typeof TargetAdapterProdPolicy.Type;

export const TargetAdapterTarget = S.Struct({
  name: S.String,
  baseUrl: S.String,
  environment: TargetAdapterEnvironment,
  owner: TargetAdapterOwner,
  capabilities: S.Array(S.Literals(["browser", "terminal"])),
  restrictions: S.optional(S.Array(S.Literal("read-only"))),
});
export type TargetAdapterTarget = typeof TargetAdapterTarget.Type;

export const TargetAdapterAuth = S.Struct({
  kind: TargetAdapterAuthKind,
  loginUrl: S.optional(S.String),
  envVars: S.optional(S.Array(S.String)),
  freshIdentity: S.Struct({
    required: S.Boolean,
    strategy: S.String,
  }),
});
export type TargetAdapterAuth = typeof TargetAdapterAuth.Type;

export const TargetAdapterRestart = S.Struct({
  kind: TargetAdapterRestartKind,
  command: S.optional(S.String),
  url: S.optional(S.String),
});
export type TargetAdapterRestart = typeof TargetAdapterRestart.Type;

export const TargetAdapterProdReadOnly = S.Struct({
  policy: TargetAdapterProdPolicy,
  allowedStepKinds: S.Array(S.String),
  blockedStepKinds: S.Array(S.String),
  notes: S.optional(S.String),
});
export type TargetAdapterProdReadOnly = typeof TargetAdapterProdReadOnly.Type;

export const TargetAdapterScenarioSeed = S.Struct({
  id: S.String,
  title: S.String,
  startPath: S.String,
  commitment: S.String,
});
export type TargetAdapterScenarioSeed = typeof TargetAdapterScenarioSeed.Type;

export const TargetAdapterContract = S.Struct({
  schemaVersion: S.Literal(TARGET_ADAPTER_SCHEMA_VERSION),
  id: S.String,
  displayName: S.String,
  target: TargetAdapterTarget,
  auth: TargetAdapterAuth,
  restart: TargetAdapterRestart,
  prodReadOnly: TargetAdapterProdReadOnly,
  scenarioSeeds: S.Array(TargetAdapterScenarioSeed),
  checklist: S.Array(S.String),
});
export type TargetAdapterContract = typeof TargetAdapterContract.Type;

export const decodeTargetAdapterContract =
  S.decodeUnknownSync(TargetAdapterContract);

const unique = <T>(values: ReadonlyArray<T>): ReadonlyArray<T> => [...new Set(values)];

const isExternalProd = (adapter: TargetAdapterContract): boolean =>
  adapter.target.owner === "external" && adapter.target.environment === "prod";

/**
 * Convert an adapter into the runner's existing Target model. Enforcement rule:
 * an external production adapter is always read-only. If the adapter declares
 * `blocked`, it is rejected before any run can start.
 */
export function targetFromAdapter(adapter: TargetAdapterContract): Target {
  if (isExternalProd(adapter) && adapter.prodReadOnly.policy === "blocked") {
    throw new Error(
      `target adapter "${adapter.id}" points at external prod and is blocked by policy`,
    );
  }

  const restrictions = unique<TargetRestriction>([
    ...(adapter.target.restrictions ?? []),
    ...(isExternalProd(adapter) ? ["read-only" as const] : []),
  ]);

  return makeTarget({
    name: adapter.target.name,
    baseUrl: adapter.target.baseUrl,
    capabilities: adapter.target.capabilities as ReadonlyArray<TargetCapability>,
    restrictions,
  });
}

