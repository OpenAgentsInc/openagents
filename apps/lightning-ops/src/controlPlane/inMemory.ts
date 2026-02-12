import { Effect, Layer } from "effect";

import type { CompileDiagnostic, ControlPlanePaywall, DeploymentIntentRecord, GatewayEventRecord } from "../contracts.js";

import { ControlPlaneService, type RecordDeploymentIntentInput } from "./service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];

export type InMemoryControlPlaneState = {
  paywalls: Array<ControlPlanePaywall>;
  deployments: Array<DeploymentIntentRecord>;
  events: Array<GatewayEventRecord>;
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
    events: [],
    writeCalls: [],
  };
  let nextDeployment = 1;
  let nextEvent = 1;

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

  const recordGatewayEvent: ControlPlaneApi["recordGatewayEvent"] = (args) =>
    Effect.sync(() => {
      const event: GatewayEventRecord = {
        eventId: `evt_mem_${nextEvent++}`,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        eventType: args.eventType,
        level: args.level,
        requestId: args.requestId,
        metadata: args.metadata,
        createdAtMs: Date.now(),
      };
      state.events.push(event);
      return { ...event };
    });

  const layer = Layer.succeed(
    ControlPlaneService,
    ControlPlaneService.of({
      listPaywallsForCompile,
      recordDeploymentIntent,
      recordGatewayEvent,
    }),
  );

  return {
    layer,
    state,
  };
};
