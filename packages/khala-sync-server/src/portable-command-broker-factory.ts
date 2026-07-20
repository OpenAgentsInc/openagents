import { createHash } from "node:crypto";

import {
  type CapabilityBrokerClock,
  type CapabilitySecretVault,
  PortableRef,
  type PortableTargetClass,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import {
  HttpPortableCapabilityGrantVault,
  makePortableCapabilityTargetAdapter,
  type HttpPortableCapabilityGrantVaultConfig,
  type PortableCapabilityTargetInstallationPort,
} from "./portable-capability-runtime-adapters.js";
import type {
  PortableCommandBrokerFactory,
  PortableCommandGrantAuthorityBinding,
} from "./portable-session-command-runner.js";
import type { PortableCapabilityTransfer } from "./portable-session-move.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const bindingFields = new Set([
  "sourceLeaseRef",
  "grantRef",
  "ownerUserId",
  "kind",
  "providerAccountRef",
  "runnerSessionId",
]);
const transferFields = new Set([
  "sourceLeaseRef",
  "destinationLeaseRef",
  "destinationSourceGrantRef",
  "expiresAt",
]);
const runnerSessionResolutionFields = new Set([
  "commandExecutionClaimRef",
  "destinationTargetRef",
  "sourceGrantRef",
  "sourceLeaseRef",
  "destinationSourceGrantRef",
  "destinationRunnerSessionId",
]);

const stableFailureRef = (code: string, scopeRef: string): string =>
  `failure.portable-command-broker-factory.${createHash("sha256")
    .update(`${code}\u0000${scopeRef}`)
    .digest("hex")}`;

const exactFields = (value: object, allowed: ReadonlySet<string>): boolean =>
  Object.keys(value).every((field) => allowed.has(field));

const unique = (values: ReadonlyArray<string>): boolean => new Set(values).size === values.length;

export class PortableCommandBrokerFactoryError extends Schema.TaggedErrorClass<PortableCommandBrokerFactoryError>()(
  "PortableCommandBrokerFactoryError",
  {
    code: Schema.Literals([
      "invalid_configuration",
      "invalid_scope",
      "capability_mismatch",
      "runner_session_unavailable",
      "target_unsupported",
      "target_mismatch",
    ]),
    failureRef: PortableRef,
  },
) {}

export type PortableCommandTargetInstallationPortResolution = Readonly<{
  targetRef: string;
  targetClass: PortableTargetClass;
  adapterRef: string;
  port: PortableCapabilityTargetInstallationPort;
}>;

/**
 * The composition root resolves one installation port for one exact durable
 * target. A null result means that the target class is not admitted.
 */
export type PortableCommandTargetInstallationPortResolver = Readonly<{
  resolve: (
    input: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      sessionRef: string;
      target: PortableTargetDescriptor;
    }>,
  ) => Promise<PortableCommandTargetInstallationPortResolution | null>;
}>;

export type PortableCommandDestinationRunnerSessionResolution = Readonly<{
  commandExecutionClaimRef: string;
  destinationTargetRef: string;
  sourceGrantRef: string;
  sourceLeaseRef: string;
  destinationSourceGrantRef: string;
  destinationRunnerSessionId: string;
}>;

/** Resolves the new runner binding without exposing runner credentials. */
export type PortableCommandDestinationRunnerSessionResolver = Readonly<{
  resolve: (
    input: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      sessionRef: string;
      destination: PortableTargetDescriptor;
      sourceBinding: PortableCommandGrantAuthorityBinding;
      capabilityTransfers: ReadonlyArray<PortableCapabilityTransfer>;
    }>,
  ) => Promise<PortableCommandDestinationRunnerSessionResolution | null>;
}>;

export type ProductionPortableCommandBrokerFactoryConfig = Readonly<{
  grantAuthority: Omit<HttpPortableCapabilityGrantVaultConfig, "bindings">;
  installationPorts: PortableCommandTargetInstallationPortResolver;
  destinationRunnerSessions?: PortableCommandDestinationRunnerSessionResolver;
  clock?: CapabilityBrokerClock;
  maxTtlMs?: number;
}>;

class CommandScopedMovingCapabilityVault implements CapabilitySecretVault {
  private readonly transfers: ReadonlyMap<
    string,
    Readonly<{
      transfer: PortableCapabilityTransfer;
      sourceGrantRef?: string;
      destinationRunnerSessionId?: string;
    }>
  >;

  constructor(
    private readonly authority: HttpPortableCapabilityGrantVault,
    transfers: ReadonlyArray<PortableCapabilityTransfer>,
    grantBindings: ReadonlyArray<PortableCommandGrantAuthorityBinding>,
    runnerSessions: ReadonlyArray<PortableCommandDestinationRunnerSessionResolution>,
  ) {
    const sessions = new Map(
      runnerSessions.map((resolution) => [resolution.sourceLeaseRef, resolution]),
    );
    this.transfers = new Map(
      transfers.map((transfer) => {
        const session = sessions.get(transfer.sourceLeaseRef);
        const grant = grantBindings.find(
          (binding) => binding.sourceLeaseRef === transfer.sourceLeaseRef,
        )!;
        return [
          transfer.sourceLeaseRef,
          {
            transfer,
            sourceGrantRef: grant.grantRef,
            ...(session === undefined
              ? {}
              : {
                  destinationRunnerSessionId: session.destinationRunnerSessionId,
                }),
          },
        ];
      }),
    );
  }

  withSourceGrantMaterial: CapabilitySecretVault["withSourceGrantMaterial"] = (input) =>
    this.authority.withSourceGrantMaterial(input);

  revokeSourceGrant: CapabilitySecretVault["revokeSourceGrant"] = async (input) => {
    const binding = this.transfers.get(input.leaseRef);
    if (
      binding === undefined ||
      (binding.sourceGrantRef !== undefined && binding.sourceGrantRef !== input.sourceGrantRef)
    ) {
      throw new PortableCommandBrokerFactoryError({
        code: "capability_mismatch",
        failureRef: stableFailureRef("capability_mismatch", input.leaseRef),
      });
    }
    await this.authority.reissue({
      sourceGrantRef: input.sourceGrantRef,
      destinationGrantRef: binding.transfer.destinationSourceGrantRef,
      ...(binding.destinationRunnerSessionId === undefined
        ? {}
        : { runnerSessionId: binding.destinationRunnerSessionId }),
      requestedAction: "portable_session_resume",
    });
    await this.authority.revokeSourceGrant(input);
  };
}

/** Creates a fresh, exact broker authority for each accepted command. */
export const createProductionPortableCommandBrokerFactory = (
  config: ProductionPortableCommandBrokerFactoryConfig,
): PortableCommandBrokerFactory => {
  if (
    typeof config.installationPorts?.resolve !== "function" ||
    !config.grantAuthority.baseUrl.startsWith("https://") ||
    config.grantAuthority.serviceBearer.length < 8 ||
    (config.maxTtlMs !== undefined &&
      (!Number.isSafeInteger(config.maxTtlMs) || config.maxTtlMs <= 0))
  ) {
    throw new PortableCommandBrokerFactoryError({
      code: "invalid_configuration",
      failureRef: stableFailureRef("invalid_configuration", "configuration"),
    });
  }

  const failure = (code: PortableCommandBrokerFactoryError["code"], scopeRef: string) =>
    new PortableCommandBrokerFactoryError({
      code,
      failureRef: stableFailureRef(code, scopeRef),
    });

  const createEffect = Effect.fn("ProductionPortableCommandBrokerFactory.create")(
    (input: Parameters<PortableCommandBrokerFactory["create"]>[0]) =>
      Effect.tryPromise({
        try: async () => {
          const { claim, source, destination, grantBindings, capabilityTransfers } = input;
          if (
            ![
              claim.claimRef,
              claim.ownerRef,
              claim.sessionRef,
              claim.executorEnvironmentRef,
              claim.destinationTargetRef,
              source.targetRef,
              source.adapterRef,
              destination.targetRef,
              destination.adapterRef,
            ].every((ref) => SAFE_REF.test(ref)) ||
            source.ownerRef !== claim.ownerRef ||
            destination.ownerRef !== claim.ownerRef ||
            source.targetRef !== claim.executorEnvironmentRef ||
            destination.targetRef !== claim.destinationTargetRef ||
            source.targetRef === destination.targetRef ||
            source.adapterRef === destination.adapterRef ||
            source.health !== "ready" ||
            destination.health !== "ready"
          ) {
            throw failure("invalid_scope", claim.claimRef);
          }

          const sourceLeaseRefs = capabilityTransfers.map((transfer) => transfer.sourceLeaseRef);
          const destinationLeaseRefs = capabilityTransfers.map(
            (transfer) => transfer.destinationLeaseRef,
          );
          const destinationGrantRefs = capabilityTransfers.map(
            (transfer) => transfer.destinationSourceGrantRef,
          );
          const sourceGrantRefs = grantBindings.map((binding) => binding.grantRef);
          const bindingLeaseRefs = grantBindings.map((binding) => binding.sourceLeaseRef);
          if (
            grantBindings.length !== capabilityTransfers.length ||
            !unique(sourceLeaseRefs) ||
            !unique(destinationLeaseRefs) ||
            !unique(destinationGrantRefs) ||
            !unique(sourceGrantRefs) ||
            !unique(bindingLeaseRefs) ||
            bindingLeaseRefs.some((ref) => !sourceLeaseRefs.includes(ref)) ||
            sourceLeaseRefs.some((ref) => !SAFE_REF.test(ref)) ||
            destinationLeaseRefs.some(
              (ref) => !SAFE_REF.test(ref) || sourceLeaseRefs.includes(ref),
            ) ||
            destinationGrantRefs.some(
              (ref) => !SAFE_REF.test(ref) || sourceGrantRefs.includes(ref),
            ) ||
            capabilityTransfers.some((transfer) => {
              const expiresAt = new Date(transfer.expiresAt);
              return (
                !exactFields(transfer, transferFields) ||
                !Number.isFinite(expiresAt.valueOf()) ||
                expiresAt <= new Date(claim.claimedAt) ||
                expiresAt > new Date(claim.leaseExpiresAt)
              );
            }) ||
            grantBindings.some(
              (binding) =>
                !exactFields(binding, bindingFields) ||
                !SAFE_REF.test(binding.sourceLeaseRef) ||
                !SAFE_REF.test(binding.grantRef) ||
                binding.ownerUserId !== claim.ownerRef ||
                !["provider", "github"].includes(binding.kind) ||
                (binding.kind === "provider" &&
                  (binding.providerAccountRef === undefined ||
                    !SAFE_REF.test(binding.providerAccountRef))) ||
                (binding.kind === "github" && binding.providerAccountRef !== undefined) ||
                (binding.runnerSessionId !== undefined && !SAFE_REF.test(binding.runnerSessionId)),
            )
          ) {
            throw failure("capability_mismatch", claim.claimRef);
          }

          const runnerBindings = grantBindings.filter(
            (binding) => binding.runnerSessionId !== undefined,
          );
          if (runnerBindings.length > 0 && config.destinationRunnerSessions === undefined) {
            throw failure("capability_mismatch", claim.claimRef);
          }
          const runnerSessions = await Promise.all(
            runnerBindings.map(async (sourceBinding) => {
              const resolution = await config
                .destinationRunnerSessions!.resolve({
                  commandExecutionClaimRef: claim.claimRef,
                  ownerRef: claim.ownerRef,
                  sessionRef: claim.sessionRef,
                  destination,
                  sourceBinding,
                  capabilityTransfers,
                })
                .catch(() => {
                  throw failure("runner_session_unavailable", sourceBinding.grantRef);
                });
              if (
                resolution === null ||
                !exactFields(resolution, runnerSessionResolutionFields) ||
                resolution.commandExecutionClaimRef !== claim.claimRef ||
                resolution.destinationTargetRef !== destination.targetRef ||
                resolution.sourceGrantRef !== sourceBinding.grantRef ||
                resolution.sourceLeaseRef !== sourceBinding.sourceLeaseRef ||
                !SAFE_REF.test(resolution.destinationRunnerSessionId) ||
                resolution.destinationRunnerSessionId === sourceBinding.runnerSessionId
              ) {
                throw failure("capability_mismatch", sourceBinding.grantRef);
              }
              const transfer = capabilityTransfers.find(
                (candidate) => candidate.sourceLeaseRef === resolution.sourceLeaseRef,
              );
              if (
                transfer === undefined ||
                transfer.destinationSourceGrantRef !== resolution.destinationSourceGrantRef
              ) {
                throw failure("capability_mismatch", sourceBinding.grantRef);
              }
              return resolution;
            }),
          );
          if (!unique(runnerSessions.map((resolution) => resolution.sourceLeaseRef))) {
            throw failure("capability_mismatch", claim.claimRef);
          }

          const resolveTarget = async (target: PortableTargetDescriptor) => {
            const resolution = await config.installationPorts.resolve({
              commandExecutionClaimRef: claim.claimRef,
              ownerRef: claim.ownerRef,
              sessionRef: claim.sessionRef,
              target,
            });
            if (resolution === null) throw failure("target_unsupported", target.targetRef);
            if (
              resolution.targetRef !== target.targetRef ||
              resolution.targetClass !== target.targetClass ||
              resolution.adapterRef !== target.adapterRef ||
              typeof resolution.port?.install !== "function" ||
              typeof resolution.port?.wipe !== "function"
            ) {
              throw failure("target_mismatch", target.targetRef);
            }
            return makePortableCapabilityTargetAdapter({
              adapterRef: target.adapterRef,
              targetClass: target.targetClass,
              port: resolution.port,
            });
          };

          const [sourceAdapter, destinationAdapter] = await Promise.all([
            resolveTarget(source),
            resolveTarget(destination),
          ]);
          const authority = new HttpPortableCapabilityGrantVault({
            ...config.grantAuthority,
            bindings: grantBindings.map(
              ({ sourceLeaseRef: _sourceLeaseRef, ...binding }) => binding,
            ),
          });
          return {
            vault: new CommandScopedMovingCapabilityVault(
              authority,
              capabilityTransfers,
              grantBindings,
              runnerSessions,
            ),
            targets: [source, destination].map((target) => ({
              targetRef: target.targetRef,
              targetClass: target.targetClass,
              adapterRef: target.adapterRef,
              ready: true,
            })),
            adapters: [sourceAdapter, destinationAdapter],
            ...(config.clock === undefined ? {} : { clock: config.clock }),
            ...(config.maxTtlMs === undefined ? {} : { maxTtlMs: config.maxTtlMs }),
          };
        },
        catch: (cause) =>
          cause instanceof PortableCommandBrokerFactoryError
            ? cause
            : failure("target_unsupported", input.claim.claimRef),
      }),
  );

  return { create: (input) => Effect.runPromise(createEffect(input)) };
};
