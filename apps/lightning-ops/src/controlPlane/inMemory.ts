import { Effect, Layer } from "effect";

import type { CompileDiagnostic, ControlPlanePaywall, DeploymentIntentRecord } from "../contracts.js";

import { ControlPlaneService, type RecordDeploymentIntentInput } from "./service.js";

export type InMemoryControlPlaneState = {
  paywalls: Array<ControlPlanePaywall>;
  deployments: Array<DeploymentIntentRecord>;
  writeCalls: Array<RecordDeploymentIntentInput>;
};

const cloneDiagnostics = (diagnostics: ReadonlyArray<CompileDiagnostic>) =>
  diagnostics.map((diag) => ({ ...diag }));

const clonePaywall = (paywall: ControlPlanePaywall): ControlPlanePaywall => ({
  ...paywall,
  policy: {
    ...paywall.policy,
    allowedHosts: paywall.policy.allowedHosts ? [...paywall.policy.allowedHosts] : undefined,
    blockedHosts: paywall.policy.blockedHosts ? [...paywall.policy.blockedHosts] : undefined,
  },
  routes: paywall.routes.map((route) => ({ ...route })),
});

const cloneDeployment = (deployment: DeploymentIntentRecord): DeploymentIntentRecord => ({
  ...deployment,
  diagnostics: deployment.diagnostics,
});

export const makeInMemoryControlPlaneHarness = (input?: {
  readonly paywalls?: ReadonlyArray<ControlPlanePaywall>;
}) => {
  const state: InMemoryControlPlaneState = {
    paywalls: [...(input?.paywalls ?? [])].map(clonePaywall),
    deployments: [],
    writeCalls: [],
  };
  let nextDeployment = 1;

  const listPaywallsForCompile = () =>
    Effect.sync(() => state.paywalls.map(clonePaywall));

  const recordDeploymentIntent = (args: RecordDeploymentIntentInput) =>
    Effect.sync(() => {
      state.writeCalls.push({ ...args, diagnostics: cloneDiagnostics(args.diagnostics) });
      const deploymentId = args.deploymentId ?? `dep_mem_${nextDeployment++}`;
      const now = Date.now();
      const existingIndex = state.deployments.findIndex((deployment) => deployment.deploymentId === deploymentId);
      const next: DeploymentIntentRecord = {
        deploymentId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        configHash: args.configHash,
        imageDigest: args.imageDigest,
        status: args.status,
        diagnostics: {
          diagnostics: cloneDiagnostics(args.diagnostics),
          metadata: args.metadata,
          requestId: args.requestId,
        },
        appliedAtMs: args.appliedAtMs,
        rolledBackFrom: args.rolledBackFrom,
        createdAtMs: existingIndex >= 0 ? state.deployments[existingIndex]!.createdAtMs : now,
        updatedAtMs: now,
      };

      if (existingIndex >= 0) {
        state.deployments[existingIndex] = next;
      } else {
        state.deployments.push(next);
      }

      return cloneDeployment(next);
    });

  const layer = Layer.succeed(
    ControlPlaneService,
    ControlPlaneService.of({
      listPaywallsForCompile,
      recordDeploymentIntent,
    }),
  );

  return {
    layer,
    state,
  };
};
