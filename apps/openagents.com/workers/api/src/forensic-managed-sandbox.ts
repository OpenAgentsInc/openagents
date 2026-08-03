import {
  ForensicEvaluatorAdjudicationSchema,
  ForensicProviderUsageReceiptSchema,
  ForensicReviewerBurdenReceiptSchema,
  ForensicRunEventSchema,
  FORENSIC_WORKER_PLACEMENT_VERSION,
  type ForensicBudget,
  ForensicBudgetSchema,
  type ForensicWorkerPlacement,
  ForensicWorkerPlacementSchema,
} from "@openagentsinc/forensic-contract";
import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  type ManagedSandboxArtifactReceipt,
  type ManagedSandboxCommand,
  ManagedSandboxCommandSchema,
  type ManagedSandboxResource,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import {
  BoxV1FacadeError,
  type BoxV1NativeStore,
  type BoxV1Policy,
  type BoxV1Principal,
  type BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import {
  type ManagedSandboxBroker,
  type ManagedSandboxBrokerResult,
  makeManagedSandboxBroker,
} from "./managed-sandbox-broker";
import type {
  ForensicMetricEvidenceAppend,
  ForensicMetricRunEvidence,
} from "./forensic-metric-evidence";

const FORENSIC_DRIVER_REF = "driver.openagents.forensic-worker.v1";
const FORENSIC_PROFILE_REF = "profile.sbx.gce.e2-small.v1";
const FORENSIC_NETWORK_POLICY_REF =
  "network-policy-ref://openagents/managed-sandbox/broker-only-v1";
const MANAGED_SANDBOX_MAX_HOURLY_COST_MICROS = 20_000;

export const FORENSIC_MANAGED_SANDBOX_PATH = "/api/forensics/workers" as const;
const INITIAL_FORENSIC_BUDGET_REF = "budget.forensic.worker.initial.v1";

export type ForensicWorkerBudgetReceipt = Readonly<{
  receiptRef: string;
  sandboxRef: string;
  resourceGeneration: number;
  observedAt: string;
  elapsedMillis: number;
  tokens: number;
  costMicros: number;
  sourceBytes: number;
  artifactBytes: number;
  networkBytes: number;
  activeTurns: number;
  exact: boolean;
}>;

export type ForensicWorkerAdmission = Readonly<{
  commandRef: string;
  idempotencyRef: string;
  requestedByRef: string;
  ownerRef: string;
  tenantRef: string;
  workUnitRef: string;
  attachmentRef: string;
  placementRef: string;
  regionRef: string;
  budgetRef: string;
  requestedAt: string;
  expiresAt: string;
  budget: ForensicBudget;
}>;

export type ForensicWorkerDispatch = Readonly<{
  commandRef: string;
  idempotencyRef: string;
  requestedByRef: string;
  turnRef: string;
  capabilityRef: string;
  requestedAt: string;
  prompt: string;
  sourceBinding: ForensicWorkerSourceBinding;
}>;

export type ForensicWorkerSourceBinding = Readonly<{
  runRef: string;
  authorityRef: string;
  bundleRef: string;
  coverageRef: string;
  coverageDigest: string;
  sourceDigest: string;
  materializationReceiptRef: string;
}>;

export type ForensicWorkerCancellation = Readonly<{
  commandRef: string;
  inspectCommandRef: string;
  idempotencyRef: string;
  inspectIdempotencyRef: string;
  requestedByRef: string;
  turnRef: string;
  reasonRef: string;
  requestedAt: string;
}>;

export type ForensicArtifactDeliveryReceipt = Readonly<{
  schema: "openagents.forensic_artifact_delivery_receipt.v1";
  receiptRef: string;
  sandboxRef: string;
  resourceGeneration: number;
  capabilityRef: string;
  artifact: ManagedSandboxArtifactReceipt;
  content: string;
  encoding: "base64";
  contentType: string;
}>;

export type ForensicManagedSandbox = Readonly<{
  admit: (
    admission: ForensicWorkerAdmission,
  ) => Effect.Effect<ForensicWorkerPlacement, BoxV1FacadeError>;
  dispatch: (
    placement: ForensicWorkerPlacement,
    request: ForensicWorkerDispatch,
  ) => Effect.Effect<ManagedSandboxBrokerResult, BoxV1FacadeError>;
  cancel: (
    placement: ForensicWorkerPlacement,
    request: ForensicWorkerCancellation,
  ) => Effect.Effect<ManagedSandboxBrokerResult, BoxV1FacadeError>;
  delete: (
    placement: ForensicWorkerPlacement,
    request: Readonly<{
      commandRef: string;
      idempotencyRef: string;
      requestedByRef: string;
      reasonRef: string;
      requestedAt: string;
    }>,
  ) => Effect.Effect<ForensicWorkerPlacement, BoxV1FacadeError>;
  collectArtifact: (
    placement: ForensicWorkerPlacement,
    requestedAt: string,
  ) => Effect.Effect<ForensicArtifactDeliveryReceipt, BoxV1FacadeError>;
}>;

const refuse = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "conflict",
    status: 409,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  });

const recoveryRequired = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "upstream_unavailable",
    status: 503,
    message,
    retryable: true,
    ...(details === undefined ? {} : { details }),
  });

const decodeCommand = (value: unknown) =>
  Effect.try({
    try: () => S.decodeUnknownSync(ManagedSandboxCommandSchema)(value),
    catch: (error) => refuse("forensic worker command failed strict validation", error),
  });

const decodePlacement = (value: unknown) =>
  Effect.try({
    try: () => S.decodeUnknownSync(ForensicWorkerPlacementSchema)(value),
    catch: (error) => recoveryRequired("forensic placement receipt failed validation", error),
  });

const allTrue = (proof: Readonly<Record<string, boolean>> | undefined): boolean =>
  proof !== undefined && Object.values(proof).every(Boolean);

const assertOrderedEvents = (result: ManagedSandboxBrokerResult) => {
  const ordered = result.events.every(
    (event, index) => index === 0 || event.sequence > result.events[index - 1]!.sequence,
  );
  return ordered
    ? Effect.void
    : Effect.fail(recoveryRequired("forensic runtime emitted unordered receipts"));
};

export const assertForensicBudgetBelowPrompt = (
  budget: ForensicBudget,
  receipt: ForensicWorkerBudgetReceipt,
  input: Readonly<{
    sandboxRef: string;
    resourceGeneration: number;
    now: Date;
  }>,
): Effect.Effect<void, BoxV1FacadeError> => {
  if (
    !receipt.exact ||
    receipt.receiptRef.trim().length === 0 ||
    receipt.sandboxRef !== input.sandboxRef ||
    receipt.resourceGeneration !== input.resourceGeneration ||
    !Number.isFinite(Date.parse(receipt.observedAt)) ||
    input.now.getTime() - Date.parse(receipt.observedAt) < 0 ||
    input.now.getTime() - Date.parse(receipt.observedAt) > 30_000
  ) {
    return Effect.fail(refuse("forensic dispatch requires a fresh exact budget receipt"));
  }
  if (budget.maxConcurrency !== 1 || receipt.activeTurns >= budget.maxConcurrency) {
    return Effect.fail(refuse("forensic dispatch exceeds its single-turn concurrency budget"));
  }
  if (
    receipt.elapsedMillis >= budget.maxTimeSeconds * 1_000 ||
    receipt.tokens >= budget.maxTokens ||
    receipt.costMicros >= budget.maxCostMicros ||
    receipt.sourceBytes + receipt.artifactBytes >= budget.maxArtifactBytes ||
    receipt.networkBytes >= budget.maxNetworkBytes
  ) {
    return Effect.fail(refuse("forensic dispatch has no remaining admitted budget"));
  }
  return Effect.void;
};

const sha256 = (value: string): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
      return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    },
    catch: (error) => recoveryRequired("forensic digest computation failed", error),
  });

const sameRefs = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean =>
  [...left].sort().join("\n") === [...right].sort().join("\n");

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 32_768) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 32_768));
  }
  return btoa(binary);
};

const exactPlacementResource = (
  placement: ForensicWorkerPlacement,
  resource: ManagedSandboxResource,
  policy: BoxV1Policy,
  profileDigest: string,
  budget: ForensicBudget,
  principal: BoxV1Principal,
): boolean =>
  resource.ownerRef === principal.ownerRef &&
  resource.tenantRef === principal.tenantRef &&
  placement.ownerRef === resource.ownerRef &&
  placement.tenantRef === resource.tenantRef &&
  placement.workUnitRef === resource.workUnitRef &&
  placement.sandboxRef === resource.sandboxRef &&
  placement.attachmentGeneration === resource.attachmentGeneration &&
  placement.resourceGeneration === resource.resourceGeneration &&
  placement.targetClass === resource.target.targetClass &&
  placement.provider === resource.target.provider &&
  placement.adapterRef === resource.target.adapterRef &&
  placement.isolation === resource.target.isolation &&
  placement.regionRef === `region.google-cloud.${resource.target.region}` &&
  placement.imageDigest === resource.imageDigest &&
  placement.profileDigest === profileDigest &&
  placement.networkPolicyRef === FORENSIC_NETWORK_POLICY_REF &&
  placement.leaseRef === resource.lease.leaseRef &&
  canonicalJson(resource.target) === canonicalJson(policy.target) &&
  resource.imageDigest === policy.imageDigest &&
  resource.profileRef === policy.profileRef &&
  resource.lease.ttlSeconds === budget.maxTimeSeconds &&
  resource.budget.maxLifetimeSeconds === budget.maxTimeSeconds &&
  resource.budget.maxCpuMillis === budget.maxTimeSeconds * 1_000 &&
  resource.budget.maxCostMicros === budget.maxCostMicros &&
  resource.budget.maxArtifactBytes === budget.maxArtifactBytes &&
  resource.budget.maxNetworkBytes === budget.maxNetworkBytes &&
  sameRefs(
    placement.capabilityRefs,
    resource.capabilities.map((capability) => capability.capabilityRef),
  );

export const makeForensicManagedSandbox = (
  input: Readonly<{
    broker: ManagedSandboxBroker;
    policy: BoxV1Policy;
    principal: BoxV1Principal;
    runtime: BoxV1Runtime;
    profileDigest: `sha256:${string}`;
    resolveBudget: (budgetRef: string) => Effect.Effect<ForensicBudget, BoxV1FacadeError>;
    now?: (() => Date) | undefined;
    assertSourceReady: (
      binding: ForensicWorkerSourceBinding &
        Readonly<{
          ownerRef: string;
          tenantRef: string;
          workUnitRef: string;
          sandboxRef: string;
          attachmentGeneration: number;
          resourceGeneration: number;
        }>,
    ) => Effect.Effect<
      Readonly<{ expiresAt: string; release: Effect.Effect<void, BoxV1FacadeError> }>,
      BoxV1FacadeError
    >;
  }>,
): ForensicManagedSandbox => {
  const now = input.now ?? (() => new Date());

  const exactResource = (placement: ForensicWorkerPlacement) =>
    Effect.gen(function* () {
      const resources = yield* input.broker.list();
      const budget = yield* input.resolveBudget(placement.budgetRef);
      const resource = resources.find((candidate) => candidate.sandboxRef === placement.sandboxRef);
      if (
        resource === undefined ||
        !exactPlacementResource(
          placement,
          resource,
          input.policy,
          input.profileDigest,
          budget,
          input.principal,
        )
      ) {
        return yield* refuse("forensic placement does not bind the durable sandbox generation");
      }
      return resource;
    });
  const assertForensicPolicy = () => {
    const target = input.policy.target;
    if (
      target.targetClass !== "openagents_managed" ||
      target.provider !== "google_cloud" ||
      target.adapterRef !== "adapter.oa-codex-control.gce.v1" ||
      target.isolation !== "gce_vm" ||
      target.dataPosture !== "openagents_managed_region" ||
      input.policy.profileRef !== FORENSIC_PROFILE_REF
    ) {
      return Effect.fail(
        refuse("forensic work requires the exact OpenAgents Cloud GCE target and profile"),
      );
    }
    return Effect.void;
  };

  const admit: ForensicManagedSandbox["admit"] = (admission) =>
    Effect.gen(function* () {
      yield* assertForensicPolicy();
      if (
        admission.ownerRef !== input.principal.ownerRef ||
        admission.tenantRef !== input.principal.tenantRef ||
        admission.requestedByRef !== input.principal.actorRef ||
        admission.regionRef !== `region.google-cloud.${input.policy.target.region}`
      ) {
        return yield* refuse("forensic admission does not bind the authenticated policy scope");
      }
      if (admission.budget.maxConcurrency !== 1) {
        return yield* refuse("initial forensic workers admit exactly one active turn");
      }
      const authoritativeBudget = yield* input.resolveBudget(admission.budgetRef);
      if (canonicalJson(authoritativeBudget) !== canonicalJson(admission.budget)) {
        return yield* refuse("forensic admission budget does not match budget authority");
      }
      const ttlSeconds = Math.floor(
        (Date.parse(admission.expiresAt) - Date.parse(admission.requestedAt)) / 1_000,
      );
      if (!Number.isSafeInteger(ttlSeconds) || ttlSeconds < 1) {
        return yield* refuse("forensic worker lease is invalid");
      }
      const capabilityRefs = {
        agentTurn: `capability.${admission.workUnitRef}.agent_turn`,
        sourceMaterializer: `capability.${admission.workUnitRef}.source_materializer`,
        artifactSink: `capability.${admission.workUnitRef}.artifact_sink`,
      } as const;
      const command = yield* decodeCommand({
        _tag: "Create",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: admission.commandRef,
        requestedByRef: admission.requestedByRef,
        ownerRef: admission.ownerRef,
        tenantRef: admission.tenantRef,
        idempotencyRef: admission.idempotencyRef,
        requestedAt: admission.requestedAt,
        workUnitRef: admission.workUnitRef,
        attachmentRef: admission.attachmentRef,
        target: input.policy.target,
        imageDigest: input.policy.imageDigest,
        profileRef: input.policy.profileRef,
        lease: {
          leaseRef: `lease.${admission.workUnitRef}`,
          state: "active",
          issuedAt: admission.requestedAt,
          expiresAt: admission.expiresAt,
          ttlSeconds,
          renewable: false,
        },
        budget: {
          currency: "USD",
          maxCostMicros: admission.budget.maxCostMicros,
          maxCpuMillis: admission.budget.maxTimeSeconds * 1_000,
          maxNetworkBytes: admission.budget.maxNetworkBytes,
          maxArtifactBytes: admission.budget.maxArtifactBytes,
          maxLifetimeSeconds: admission.budget.maxTimeSeconds,
        },
        requestedCapabilities: [
          {
            capabilityRef: capabilityRefs.agentTurn,
            kind: "agent_turn",
            state: "active",
            expiresAt: admission.expiresAt,
          },
          {
            capabilityRef: capabilityRefs.sourceMaterializer,
            kind: "forensic_source_delivery",
            state: "active",
            expiresAt: admission.expiresAt,
          },
          {
            capabilityRef: capabilityRefs.artifactSink,
            kind: "artifact_read",
            state: "active",
            expiresAt: admission.expiresAt,
          },
        ],
      });
      const result = yield* input.broker.execute(command, {
        attachmentGeneration: 1,
      });
      const outcome = result.lifecycleOutcome;
      if (
        result.receipt.outcome !== "succeeded" ||
        outcome?.phase !== "ready" ||
        outcome.forensicDriverRef !== FORENSIC_DRIVER_REF ||
        !allTrue(outcome.readinessProof) ||
        outcome.measuredCostMicros > admission.budget.maxCostMicros
      ) {
        return yield* recoveryRequired("forensic worker readiness or cost truth was incomplete");
      }
      yield* assertOrderedEvents(result);
      return yield* decodePlacement({
        schema: FORENSIC_WORKER_PLACEMENT_VERSION,
        placementRef: admission.placementRef,
        ownerRef: admission.ownerRef,
        tenantRef: admission.tenantRef,
        workUnitRef: admission.workUnitRef,
        sandboxRef: result.resource.sandboxRef,
        attachmentGeneration: result.resource.attachmentGeneration,
        resourceGeneration: result.resource.resourceGeneration,
        targetClass: "openagents_managed",
        provider: "google_cloud",
        adapterRef: "adapter.oa-codex-control.gce.v1",
        isolation: "gce_vm",
        regionRef: `region.google-cloud.${input.policy.target.region}`,
        imageDigest: input.policy.imageDigest,
        profileDigest: input.profileDigest,
        networkPolicyRef: FORENSIC_NETWORK_POLICY_REF,
        leaseRef: result.resource.lease.leaseRef,
        budgetRef: admission.budgetRef,
        capabilityRefs: Object.values(capabilityRefs),
        state: "worker_ready",
        admissionReceiptRef: result.receipt.receiptRef,
        readinessReceiptRef: outcome.receiptRef,
        updatedAt: outcome.observedAt,
      });
    });

  const dispatch: ForensicManagedSandbox["dispatch"] = (placement, request) =>
    Effect.gen(function* () {
      if (request.requestedByRef !== input.principal.actorRef) {
        return yield* refuse("forensic dispatch does not bind the authenticated actor");
      }
      const resource = yield* exactResource(placement);
      if (resource.facts.lifecycle !== "ready") {
        return yield* recoveryRequired("forensic worker is not ready for dispatch");
      }
      const dispatchNow = now();
      const currentTime = dispatchNow.getTime();
      const requestedAt = Date.parse(request.requestedAt);
      const capability = resource.capabilities.find(
        (candidate) => candidate.capabilityRef === request.capabilityRef,
      );
      if (
        !Number.isFinite(requestedAt) ||
        currentTime - requestedAt < 0 ||
        currentTime - requestedAt > 30_000 ||
        resource.lease.state !== "active" ||
        Date.parse(resource.lease.expiresAt) <= currentTime ||
        capability === undefined ||
        capability.kind !== "agent_turn" ||
        capability.state !== "active" ||
        Date.parse(capability.expiresAt) <= currentTime ||
        !placement.capabilityRefs.includes(capability.capabilityRef)
      ) {
        return yield* refuse(
          "forensic dispatch requires the exact active generation-bound agent-turn capability",
        );
      }
      const budget = yield* input.resolveBudget(placement.budgetRef);
      const probe = yield* input.runtime.probe === undefined
        ? Effect.fail(recoveryRequired("forensic budget authority is unavailable"))
        : input.runtime.probe({
            principal: input.principal,
            resource,
            operationRef: `${request.commandRef}.budget`,
            idempotencyRef: `${request.idempotencyRef}.budget`,
          });
      if (
        probe.action !== "probe" ||
        probe.phase !== "ready" ||
        probe.generation !== resource.resourceGeneration ||
        probe.usageProof === undefined
      ) {
        if (
          probe.action === "probe" &&
          probe.phase === "recovery_required" &&
          probe.generation === resource.resourceGeneration
        ) {
          const reconciliation = yield* decodeCommand({
            _tag: "Stop",
            schema: "openagents.managed_sandbox_command.v1",
            commandRef: `${request.commandRef}.probe-recovery`,
            requestedByRef: request.requestedByRef,
            ownerRef: placement.ownerRef,
            tenantRef: placement.tenantRef,
            idempotencyRef: `${request.idempotencyRef}.probe-recovery`,
            requestedAt: request.requestedAt,
            sandboxRef: placement.sandboxRef,
            expectedVersion: resource.version,
            reasonRef: `reason.${probe.errorCode ?? "forensic_probe_recovery"}`,
          });
          if (reconciliation._tag !== "Stop") {
            return yield* recoveryRequired("forensic probe reconciliation was invalid");
          }
          if (input.broker.reconcileProbeRecovery === undefined) {
            return yield* recoveryRequired("forensic probe recovery authority is unavailable");
          }
          yield* input.broker.reconcileProbeRecovery(reconciliation, probe);
        }
        return yield* recoveryRequired("forensic budget receipt did not bind the live worker");
      }
      yield* assertForensicBudgetBelowPrompt(
        budget,
        {
          receiptRef: probe.receiptRef,
          sandboxRef: resource.sandboxRef,
          resourceGeneration: probe.generation,
          observedAt: probe.observedAt,
          elapsedMillis: probe.measuredRunningMs,
          tokens: probe.usageProof.tokens,
          costMicros: probe.measuredCostMicros,
          sourceBytes: probe.usageProof.sourceBytes,
          artifactBytes: probe.usageProof.artifactBytes,
          networkBytes: probe.usageProof.networkBytes,
          activeTurns: probe.usageProof.activeTurns,
          exact: probe.usageProof.exact,
        },
        {
          sandboxRef: resource.sandboxRef,
          resourceGeneration: resource.resourceGeneration,
          now: dispatchNow,
        },
      );
      if (
        budget.maxTimeSeconds !== resource.budget.maxLifetimeSeconds ||
        budget.maxCostMicros !== resource.budget.maxCostMicros ||
        budget.maxArtifactBytes !== resource.budget.maxArtifactBytes ||
        budget.maxNetworkBytes !== resource.budget.maxNetworkBytes
      ) {
        return yield* refuse("forensic dispatch budget does not bind the admitted worker");
      }
      const promptDigest = yield* sha256(request.prompt);
      const observedAt = Date.parse(probe.observedAt);
      const remainingTimeMillis = budget.maxTimeSeconds * 1_000 - probe.measuredRunningMs;
      const remainingCostMicros = budget.maxCostMicros - probe.measuredCostMicros;
      const costBoundMillis = Math.floor(
        (remainingCostMicros * 3_600_000) / MANAGED_SANDBOX_MAX_HOURLY_COST_MICROS,
      );
      const deadlineMillis = Math.min(
        Date.parse(resource.lease.expiresAt),
        Date.parse(capability.expiresAt),
        currentTime + remainingTimeMillis,
        currentTime + costBoundMillis,
      );
      const guardrails = {
        receiptRef: probe.receiptRef,
        sandboxRef: resource.sandboxRef,
        resourceGeneration: resource.resourceGeneration,
        observedAt: probe.observedAt,
        deadlineAt: new Date(deadlineMillis).toISOString(),
        remainingTokens: budget.maxTokens - probe.usageProof.tokens,
        remainingCostMicros,
        networkBytesObserved: probe.usageProof.networkBytes,
        remainingNetworkBytes: budget.maxNetworkBytes - probe.usageProof.networkBytes,
        artifactBytesObserved: probe.usageProof.artifactBytes,
        remainingArtifactBytes:
          budget.maxArtifactBytes - probe.usageProof.sourceBytes - probe.usageProof.artifactBytes,
      };
      if (
        !Number.isFinite(observedAt) ||
        !Number.isFinite(deadlineMillis) ||
        deadlineMillis <= currentTime ||
        Object.values(guardrails).some((value) =>
          typeof value === "number" ? !Number.isSafeInteger(value) || value < 0 : false,
        ) ||
        guardrails.remainingTokens < 1 ||
        guardrails.remainingCostMicros < 1 ||
        guardrails.remainingNetworkBytes < 1 ||
        guardrails.remainingArtifactBytes < 1
      ) {
        return yield* recoveryRequired("forensic in-flight budget guardrails were exhausted");
      }
      const command = yield* decodeCommand({
        _tag: "Dispatch",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.commandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.idempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        turnRef: request.turnRef,
        capabilityRef: request.capabilityRef,
        promptDigest: `sha256:${promptDigest}`,
        runtime: {
          provider: "codex",
          modelRef: "model.openai.gpt-5",
          harnessRef: FORENSIC_DRIVER_REF,
        },
      });
      const sourceLease = yield* input.assertSourceReady({
        ...request.sourceBinding,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        workUnitRef: placement.workUnitRef,
        sandboxRef: placement.sandboxRef,
        attachmentGeneration: placement.attachmentGeneration,
        resourceGeneration: placement.resourceGeneration,
      });
      const sourceLeaseExpiresAt = Date.parse(sourceLease.expiresAt);
      if (!Number.isFinite(sourceLeaseExpiresAt) || sourceLeaseExpiresAt <= currentTime) {
        yield* sourceLease.release;
        return yield* recoveryRequired("forensic source dispatch lease is already expired");
      }
      const sourceGuardrails = {
        ...guardrails,
        deadlineAt: new Date(
          Math.min(Date.parse(guardrails.deadlineAt), sourceLeaseExpiresAt),
        ).toISOString(),
      };
      const result = yield* input.broker
        .execute(command, {
          prompt: request.prompt,
          guardrails: sourceGuardrails,
        })
        .pipe(
          Effect.onError(() =>
            sourceLease.release.pipe(
              Effect.catch(() =>
                Effect.logError(
                  "forensic source dispatch lease release failed after dispatch error",
                ),
              ),
            ),
          ),
        );
      yield* sourceLease.release;
      yield* assertOrderedEvents(result);
      return result;
    });

  const cancel: ForensicManagedSandbox["cancel"] = (placement, request) =>
    Effect.gen(function* () {
      if (request.requestedByRef !== input.principal.actorRef) {
        return yield* refuse("forensic cancellation does not bind the authenticated actor");
      }
      const resource = yield* exactResource(placement);
      const interrupt = yield* decodeCommand({
        _tag: "Interrupt",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.commandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.idempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        turnRef: request.turnRef,
        reasonRef: request.reasonRef,
      });
      const interrupted = yield* input.broker.execute(interrupt);
      yield* assertOrderedEvents(interrupted);
      const inspect = yield* decodeCommand({
        _tag: "Inspect",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.inspectCommandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.inspectIdempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
      });
      const settled = yield* input.broker.execute(inspect);
      yield* assertOrderedEvents(settled);
      if (
        settled.turn === null ||
        !["settled", "failed", "interrupted"].includes(settled.turn.status)
      ) {
        return yield* recoveryRequired("forensic cancellation did not structurally settle");
      }
      return settled;
    });

  const remove: ForensicManagedSandbox["delete"] = (placement, request) =>
    Effect.gen(function* () {
      if (request.requestedByRef !== input.principal.actorRef) {
        return yield* refuse("forensic deletion does not bind the authenticated actor");
      }
      const resource = yield* exactResource(placement);
      const revoke: ManagedSandboxCommand = yield* decodeCommand({
        _tag: "Update",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: `${request.commandRef}.revoke`,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: `${request.idempotencyRef}.revoke`,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        capabilities: resource.capabilities.map((capability) => ({
          ...capability,
          state: "revoked",
        })),
      });
      const revoked = yield* input.broker.execute(revoke);
      if (revoked.resource.capabilities.some((capability) => capability.state !== "revoked")) {
        return yield* recoveryRequired("forensic capability cleanup was not durably observed");
      }
      const stopped =
        revoked.resource.facts.lifecycle === "recovery_required"
          ? undefined
          : yield* input.broker.execute(
              yield* decodeCommand({
                _tag: "Stop",
                schema: "openagents.managed_sandbox_command.v1",
                commandRef: `${request.commandRef}.stop`,
                requestedByRef: request.requestedByRef,
                ownerRef: placement.ownerRef,
                tenantRef: placement.tenantRef,
                idempotencyRef: `${request.idempotencyRef}.stop`,
                requestedAt: request.requestedAt,
                sandboxRef: placement.sandboxRef,
                expectedVersion: revoked.resource.version,
                reasonRef: request.reasonRef,
              }),
            );
      if (
        stopped !== undefined &&
        (stopped.resource.facts.lifecycle !== "stopped" ||
          stopped.lifecycleOutcome?.phase !== "stopped" ||
          stopped.lifecycleOutcome.cleanupProof?.zeroProcess !== true ||
          stopped.lifecycleOutcome.cleanupProof.zeroScratch !== true)
      ) {
        return yield* recoveryRequired("forensic process or scratch cleanup was not observed");
      }
      const deleteResource = stopped?.resource ?? revoked.resource;
      const command: ManagedSandboxCommand = yield* decodeCommand({
        _tag: "Delete",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: `${request.commandRef}.delete`,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: `${request.idempotencyRef}.delete`,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: deleteResource.version,
        reasonRef: request.reasonRef,
      });
      const result = yield* input.broker.execute(command);
      const outcome = result.lifecycleOutcome;
      if (
        result.receipt.outcome !== "succeeded" ||
        outcome?.phase !== "deleted" ||
        outcome.forensicDriverRef !== FORENSIC_DRIVER_REF ||
        outcome.cleanupProof?.zeroCompute !== true ||
        outcome.cleanupProof.zeroDisk !== true ||
        outcome.cleanupProof.zeroFirewall !== true ||
        outcome.cleanupProof.zeroProcess !== true ||
        outcome.cleanupProof.zeroScratch !== true ||
        outcome.cleanupProof.zeroIngress !== true ||
        result.resource.capabilities.some((capability) => capability.state !== "revoked")
      ) {
        return yield* recoveryRequired("forensic worker deletion left incomplete cleanup truth");
      }
      yield* assertOrderedEvents(result);
      return yield* decodePlacement({
        ...placement,
        resourceGeneration: result.resource.resourceGeneration,
        state: "cleaned",
        ...(stopped === undefined ? {} : { stopReceiptRef: stopped.receipt.receiptRef }),
        deletionReceiptRef: result.receipt.receiptRef,
        cleanupReceiptRef: outcome.receiptRef,
        updatedAt: outcome.observedAt,
      });
    });

  const artifactAdmission = (resource: ManagedSandboxResource, requestedAt: string) => {
    const capability = resource.capabilities.find(
      (candidate) =>
        candidate.kind === "artifact_read" &&
        candidate.state === "active" &&
        Date.parse(candidate.expiresAt) > Date.parse(requestedAt),
    );
    if (
      capability === undefined ||
      Date.parse(resource.lease.expiresAt) <= Date.parse(requestedAt)
    ) {
      return Effect.fail(refuse("forensic artifact capability is not active for this generation"));
    }
    return Effect.succeed({
      capabilityRef: capability.capabilityRef,
      capabilityState: "active" as const,
      capabilityExpiresAt: capability.expiresAt,
      requestedAt,
      limits: {
        workspaceRootRef: "workspace.managed-sandbox",
        maxFileBytes: Math.min(16 * 1_024 * 1_024, Math.max(1, resource.budget.maxArtifactBytes)),
        maxArtifactBytes: Math.min(
          16 * 1_024 * 1_024,
          Math.max(1, resource.budget.maxArtifactBytes),
        ),
        maxOutputBytes: 131_072,
        maxDurationMillis: Math.min(resource.budget.maxLifetimeSeconds * 1_000, 3_600_000),
        maxCpuMillis: Math.min(resource.budget.maxCpuMillis, 3_600_000),
        maxProcesses: 1,
        maxNetworkBytes: 0,
        networkPolicyRef: "network-policy.managed-sandbox.deny-all",
      },
    });
  };

  const collectArtifact: ForensicManagedSandbox["collectArtifact"] = (placement, requestedAt) =>
    Effect.gen(function* () {
      const resource = yield* exactResource(placement);
      const admission = yield* artifactAdmission(resource, requestedAt);
      const result = yield* input.runtime.artifact({
        principal: input.principal,
        resource,
        operationRef: `operation.forensic.artifact.${placement.placementRef}`,
        idempotencyRef: `idempotency.forensic.artifact.${placement.placementRef}`,
        ...admission,
        path: "workspace/forensic-artifact.tar.zst",
        retentionUntil: new Date(Date.parse(requestedAt) + 24 * 60 * 60 * 1_000).toISOString(),
      });
      const contentDigest = yield* Effect.tryPromise({
        try: async () => {
          const digest = await crypto.subtle.digest(
            "SHA-256",
            Uint8Array.from(result.bytes).buffer,
          );
          return `sha256:${[...new Uint8Array(digest)]
            .map((byte) => byte.toString(16).padStart(2, "0"))
            .join("")}`;
        },
        catch: () => recoveryRequired("forensic artifact digest could not be observed"),
      });
      if (
        result.receipt.outcome !== "succeeded" ||
        result.receipt.sandboxRef !== resource.sandboxRef ||
        result.receipt.resourceGeneration !== resource.resourceGeneration ||
        result.artifact.byteLength > resource.budget.maxArtifactBytes ||
        result.artifact.byteLength !== result.bytes.byteLength ||
        result.artifact.contentDigest !== contentDigest ||
        result.artifact.sourceGeneration !== resource.resourceGeneration ||
        result.receipt.capabilityRef !== admission.capabilityRef
      ) {
        return yield* recoveryRequired("private forensic artifact receipt exceeded exact scope");
      }
      return {
        schema: "openagents.forensic_artifact_delivery_receipt.v1",
        receiptRef: result.receipt.receiptRef,
        sandboxRef: resource.sandboxRef,
        resourceGeneration: resource.resourceGeneration,
        capabilityRef: admission.capabilityRef,
        artifact: result.artifact,
        content: bytesToBase64(result.bytes),
        encoding: "base64",
        contentType: result.contentType,
      };
    });

  return { admit, dispatch, cancel, delete: remove, collectArtifact };
};

const ForensicRouteTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);
const ForensicRouteNonNegativeInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

const ForensicWorkerRouteRequestSchema = S.TaggedUnion({
  Admit: {
    commandRef: S.String,
    idempotencyRef: S.String,
    workUnitRef: S.String,
    attachmentRef: S.String,
    placementRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
  Dispatch: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    idempotencyRef: S.String,
    turnRef: S.String,
    capabilityRef: S.String,
    prompt: S.String.check(S.isMinLength(1), S.isMaxLength(100_000)),
    sourceBinding: S.Struct({
      runRef: S.String,
      authorityRef: S.String,
      bundleRef: S.String,
      coverageRef: S.String,
      coverageDigest: S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/u)),
      sourceDigest: S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/u)),
      materializationReceiptRef: S.String,
    }),
    requestedAt: ForensicRouteTimestamp,
  },
  CollectArtifact: {
    placement: ForensicWorkerPlacementSchema,
    requestedAt: ForensicRouteTimestamp,
  },
  Observe: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    idempotencyRef: S.String,
    afterSequence: ForensicRouteNonNegativeInteger,
    limit: S.Number.check(S.isInt(), S.isBetween({ minimum: 1, maximum: 256 })),
    requestedAt: ForensicRouteTimestamp,
  },
  Cancel: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    inspectCommandRef: S.String,
    idempotencyRef: S.String,
    inspectIdempotencyRef: S.String,
    turnRef: S.String,
    reasonRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
  Delete: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    idempotencyRef: S.String,
    reasonRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
  RecordMetricEvidence: {
    evidence: S.Union([
      ForensicRunEventSchema,
      ForensicProviderUsageReceiptSchema,
      ForensicEvaluatorAdjudicationSchema,
      ForensicReviewerBurdenReceiptSchema,
    ]),
  },
  ReadMetricEvidence: {
    runRef: S.String,
  },
});

type ForensicAuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ForensicManagedSandboxRouteDependencies<Bindings> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    context: ExecutionContext,
  ) => Promise<ForensicAuthenticatedOwner | undefined>;
  enabled: (env: Bindings) => boolean;
  policy: (env: Bindings) => Effect.Effect<BoxV1Policy, BoxV1FacadeError>;
  profileDigest: (env: Bindings) => string | undefined;
  store: (env: Bindings) => BoxV1NativeStore;
  runtime: (env: Bindings) => Effect.Effect<BoxV1Runtime, BoxV1FacadeError>;
  assertSourceReady: (
    env: Bindings,
    binding: Parameters<Parameters<typeof makeForensicManagedSandbox>[0]["assertSourceReady"]>[0],
  ) => Effect.Effect<
    Readonly<{ expiresAt: string; release: Effect.Effect<void, BoxV1FacadeError> }>,
    BoxV1FacadeError
  >;
  appendMetricEvidence: (
    env: Bindings,
    ownerRef: string,
    evidence: unknown,
  ) => Effect.Effect<ForensicMetricEvidenceAppend, BoxV1FacadeError>;
  readMetricEvidence: (
    env: Bindings,
    ownerRef: string,
    runRef: string,
  ) => Effect.Effect<ForensicMetricRunEvidence, BoxV1FacadeError>;
  now?: (() => Date) | undefined;
}>;

const routeJson = (
  body: unknown,
  init: ResponseInit = {},
  decorate?: HttpHeadersDecorator,
): Response => {
  const headers = new Headers(init.headers);
  headers.set("content-type", "application/json; charset=utf-8");
  headers.set("cache-control", "no-store");
  decorate?.(headers);
  return new Response(JSON.stringify(body), { ...init, headers });
};

const routeError = (error: BoxV1FacadeError): Response =>
  routeJson(
    { error: error.code, message: error.message, retryable: error.retryable },
    { status: error.status },
  );

const initialForensicBudget = (policy: BoxV1Policy): ForensicBudget => ({
  maxTimeSeconds: policy.defaultTtlSeconds,
  maxTokens: 200_000,
  maxCostMicros: policy.maxCostMicros,
  maxConcurrency: 1,
  maxArtifactBytes: policy.maxArtifactBytes,
  maxNetworkBytes: policy.maxNetworkBytes,
});

export const makeForensicManagedSandboxRoutes = <Bindings>(
  dependencies: ForensicManagedSandboxRouteDependencies<Bindings>,
) => {
  const now = dependencies.now ?? (() => new Date());
  const handle = (request: Request, env: Bindings, context: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST") {
        return routeJson({ error: "method_not_allowed" }, { status: 405 });
      }
      const owner = yield* Effect.tryPromise({
        try: () => dependencies.authenticateOwner(request, env, context),
        catch: () =>
          new BoxV1FacadeError({
            code: "authentication_required",
            status: 401,
            message: "forensic worker owner authentication is unavailable",
            retryable: false,
          }),
      });
      if (owner === undefined) return routeJson({ error: "unauthorized" }, { status: 401 });
      if (!dependencies.enabled(env)) {
        return routeJson(
          {
            error: "runtime_not_admitted",
            message: "The OpenAgents Cloud forensic worker remains default-off.",
          },
          { status: 503 },
          owner.decorateResponseHeaders,
        );
      }
      const body = yield* Effect.tryPromise({
        try: async () =>
          S.decodeUnknownSync(ForensicWorkerRouteRequestSchema)(await request.json(), {
            onExcessProperty: "error",
          }),
        catch: (error) =>
          new BoxV1FacadeError({
            code: "validation_failed",
            status: 400,
            message: "request failed the forensic worker schema",
            retryable: false,
            details: error,
          }),
      });
      const policy = yield* dependencies.policy(env);
      const profileDigest = dependencies.profileDigest(env);
      if (profileDigest === undefined || !/^sha256:[0-9a-f]{64}$/u.test(profileDigest)) {
        return yield* recoveryRequired("forensic worker profile digest is unavailable");
      }
      const budget = S.decodeUnknownSync(ForensicBudgetSchema)(initialForensicBudget(policy));
      const principal = {
        actorRef: `agent:${owner.userId}`,
        ownerRef: owner.userId,
        tenantRef: owner.userId,
        login: "OpenAgents Forensics",
        email: null,
      } satisfies BoxV1Principal;
      const runtime = yield* dependencies.runtime(env);
      const store = dependencies.store(env);
      const broker = makeManagedSandboxBroker({
        principal,
        policy,
        store,
        runtime,
        now,
      });
      const forensic = makeForensicManagedSandbox({
        broker,
        policy,
        principal,
        runtime,
        profileDigest: profileDigest as `sha256:${string}`,
        now,
        resolveBudget: (budgetRef) =>
          budgetRef === INITIAL_FORENSIC_BUDGET_REF
            ? Effect.succeed(budget)
            : Effect.fail(refuse("forensic budget ref is not admitted")),
        assertSourceReady: (binding) => dependencies.assertSourceReady(env, binding),
      });
      const requestedByRef = `agent:${owner.userId}`;
      const handledAtMs = now().getTime();
      let result: unknown;
      switch (body._tag) {
        case "Admit":
          if (
            Date.parse(body.requestedAt) > handledAtMs + 60_000 ||
            Date.parse(body.requestedAt) + budget.maxTimeSeconds * 1_000 <= handledAtMs
          ) {
            return yield* refuse("forensic admission timestamp is outside its lease window");
          }
          result = yield* forensic.admit({
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            ownerRef: owner.userId,
            tenantRef: owner.userId,
            workUnitRef: body.workUnitRef,
            attachmentRef: body.attachmentRef,
            placementRef: body.placementRef,
            regionRef: `region.google-cloud.${policy.target.region}`,
            budgetRef: INITIAL_FORENSIC_BUDGET_REF,
            requestedAt: body.requestedAt,
            expiresAt: new Date(
              Date.parse(body.requestedAt) + budget.maxTimeSeconds * 1_000,
            ).toISOString(),
            budget,
          });
          break;
        case "Dispatch":
          result = yield* forensic.dispatch(body.placement, {
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            turnRef: body.turnRef,
            capabilityRef: body.capabilityRef,
            requestedAt: body.requestedAt,
            prompt: body.prompt,
            sourceBinding: body.sourceBinding,
          });
          break;
        case "CollectArtifact":
          result = yield* forensic.collectArtifact(body.placement, body.requestedAt);
          break;
        case "Observe": {
          const placement = body.placement;
          const inspected = yield* broker.execute(
            yield* decodeCommand({
              _tag: "Inspect",
              schema: "openagents.managed_sandbox_command.v1",
              commandRef: body.commandRef,
              requestedByRef,
              ownerRef: placement.ownerRef,
              tenantRef: placement.tenantRef,
              idempotencyRef: body.idempotencyRef,
              requestedAt: body.requestedAt,
              sandboxRef: placement.sandboxRef,
            }),
          );
          if (
            inspected.resource.ownerRef !== placement.ownerRef ||
            inspected.resource.tenantRef !== placement.tenantRef ||
            inspected.resource.workUnitRef !== placement.workUnitRef ||
            inspected.resource.attachmentGeneration !== placement.attachmentGeneration ||
            inspected.resource.resourceGeneration !== placement.resourceGeneration
          ) {
            return yield* recoveryRequired(
              "forensic observation does not match the admitted worker generation",
            );
          }
          const page = yield* store.readEvents({
            ownerRef: placement.ownerRef,
            tenantRef: placement.tenantRef,
            sandboxRef: placement.sandboxRef,
            afterSequence: body.afterSequence,
            limit: body.limit,
          });
          const contiguous = page.events.every(
            (event, index) =>
              event.sequence === body.afterSequence + index + 1 &&
              event.resourceGeneration === placement.resourceGeneration,
          );
          if (!contiguous || page.nextSequence < body.afterSequence) {
            return yield* recoveryRequired(
              "forensic observation returned a gapped or foreign-generation event page",
            );
          }
          result = {
            schema: "openagents.forensic_worker_observation.v1",
            placementRef: placement.placementRef,
            sandboxRef: placement.sandboxRef,
            resourceGeneration: placement.resourceGeneration,
            lifecycle: inspected.resource.facts.lifecycle,
            cleanupComplete: inspected.resource.facts.cleanupComplete,
            turn:
              inspected.turn === null
                ? null
                : {
                    turnRef: inspected.turn.turnRef,
                    status: inspected.turn.status,
                    lastEventSequence: inspected.turn.lastEventSequence,
                    createdAt: inspected.turn.createdAt,
                    ...(inspected.turn.startedAt === undefined
                      ? {}
                      : { startedAt: inspected.turn.startedAt }),
                    ...(inspected.turn.settledAt === undefined
                      ? {}
                      : { settledAt: inspected.turn.settledAt }),
                  },
            events: page.events.map((event) => ({
              eventRef: event.eventRef,
              kind: event._tag,
              sequence: event.sequence,
              resourceGeneration: event.resourceGeneration,
              observedAt: event.observedAt,
              ...("turnRef" in event ? { turnRef: event.turnRef } : {}),
            })),
            afterSequence: page.afterSequence,
            nextSequence: page.nextSequence,
            terminalSequence: page.terminalSequence,
            hasMore: page.nextSequence < page.terminalSequence,
            silenceIsTerminal: false,
          };
          break;
        }
        case "Cancel":
          result = yield* forensic.cancel(body.placement, {
            commandRef: body.commandRef,
            inspectCommandRef: body.inspectCommandRef,
            idempotencyRef: body.idempotencyRef,
            inspectIdempotencyRef: body.inspectIdempotencyRef,
            requestedByRef,
            turnRef: body.turnRef,
            reasonRef: body.reasonRef,
            requestedAt: body.requestedAt,
          });
          break;
        case "Delete":
          result = yield* forensic.delete(body.placement, {
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            reasonRef: body.reasonRef,
            requestedAt: body.requestedAt,
          });
          break;
        case "RecordMetricEvidence":
          result = yield* dependencies.appendMetricEvidence(env, owner.userId, body.evidence);
          break;
        case "ReadMetricEvidence":
          result = yield* dependencies.readMetricEvidence(env, owner.userId, body.runRef);
          break;
      }
      return routeJson({ result }, {}, owner.decorateResponseHeaders);
    }).pipe(Effect.catch((error) => Effect.succeed(routeError(error))));

  return { handle };
};
