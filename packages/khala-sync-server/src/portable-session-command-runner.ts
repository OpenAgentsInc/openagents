import { createHash } from "node:crypto";

import {
  PortableRef,
  PortableSessionCommandSchema,
  PortableTargetDescriptorSchema,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimRequest,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { canonicalJson } from "@openagentsinc/khala-sync";
import { Schema } from "effect";

import type { SyncTransactionWriter } from "./outbox-writer.js";
import {
  PostgresPortablePhaseTarget,
  type PortablePhaseTargetCheckpointArtifact,
} from "./portable-phase-target-adapter.js";
import {
  readPortableSessionAuthoritySnapshot,
  type PortableSessionAuthoritySnapshot,
} from "./portable-session-authority.js";
import {
  PortableSessionCommandConsumer,
  type PortableSessionCommandConsumerResult,
  type PortableSessionCommandResolver,
} from "./portable-session-command-consumer.js";
import { PostgresPortableSessionCommandQueue } from "./portable-session-command-queue.js";
import {
  PostgresPortableSessionMoveRuntime,
  type PortableSessionMoveRuntimeBrokerConfig,
  type PortableSessionMoveRuntimeInput,
} from "./portable-session-move-runtime.js";
import type {
  PortableCapabilityTransfer,
  PortableCheckpointBundle,
} from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

const decodeCommand = Schema.decodeUnknownSync(PortableSessionCommandSchema);
const decodeTarget = Schema.decodeUnknownSync(PortableTargetDescriptorSchema);
const decodeRefs = Schema.decodeUnknownSync(Schema.Array(PortableRef));

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;

const refDigest = (prefix: string, ...values: ReadonlyArray<string>): string =>
  `${prefix}.${createHash("sha256").update(values.join("\u0000")).digest("hex")}`;

const fingerprint = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;

const assertRefsOnly = (value: unknown, message: string): void => {
  if (FORBIDDEN_PRIVATE_MATERIAL.test(JSON.stringify(value))) {
    throw new PortableSessionCommandRunnerError("unsafe_facts", message);
  }
};

const exactRefs = (values: ReadonlyArray<string>, field: string): ReadonlyArray<string> => {
  if (values.some((value) => !SAFE_REF.test(value)) || new Set(values).size !== values.length) {
    throw new PortableSessionCommandRunnerError(
      "invalid_facts",
      `${field} must contain unique public-safe refs`,
    );
  }
  return values;
};

export class PortableSessionCommandRunnerError extends Error {
  readonly _tag = "PortableSessionCommandRunnerError";
  override readonly name = "PortableSessionCommandRunnerError";

  constructor(
    readonly code:
      | "authority_missing"
      | "authority_mismatch"
      | "target_mismatch"
      | "pylon_mismatch"
      | "capability_mismatch"
      | "artifact_mismatch"
      | "invalid_facts"
      | "unsafe_facts",
    message: string,
  ) {
    super(message);
  }
}

export type PortableCommandCapabilityGrantFact = Readonly<{
  sourceLeaseRef: string;
  destinationSourceGrantRef: string;
  expiresAt: string;
}>;

export type PortableCommandCapabilityGrantFactResolver = Readonly<{
  resolve: (
    scope: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      sessionRef: string;
      sourceAttachmentRef: string;
      sourceGeneration: number;
      sourceTargetRef: string;
      destinationAttachmentRef: string;
      destinationGeneration: number;
      destinationTargetRef: string;
      sourceLeaseRefs: ReadonlyArray<string>;
    }>,
  ) => Promise<ReadonlyArray<PortableCommandCapabilityGrantFact>>;
}>;

export type PortableCommandPylonBindingResolver = Readonly<{
  resolve: (
    scope: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      targetRef: string;
    }>,
  ) => Promise<
    Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      targetRef: string;
      pylonRef: string;
    }>
  >;
}>;

export type PortableCommandCheckpointArtifactResolver = Readonly<{
  resolve: (
    scope: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      sessionRef: string;
      artifact: PortablePhaseTargetCheckpointArtifact;
    }>,
  ) => Promise<PortableCheckpointBundle>;
}>;

export type PostgresPortableSessionCommandResolverConfig = Readonly<{
  sql: SyncSql;
  broker: PortableSessionMoveRuntimeBrokerConfig;
  pylonBindings: PortableCommandPylonBindingResolver;
  capabilityGrantFacts: PortableCommandCapabilityGrantFactResolver;
  checkpointArtifacts: PortableCommandCheckpointArtifactResolver;
  now?: () => string;
  readAuthoritySnapshot?: typeof readPortableSessionAuthoritySnapshot;
}>;

type TargetRow = Readonly<{
  target_ref?: unknown;
  target_class?: unknown;
  adapter_ref?: unknown;
  compatibility_ref?: unknown;
  isolation?: unknown;
  data_posture?: unknown;
  health?: unknown;
}>;

const targetFromRow = (row: TargetRow, ownerRef: string): PortableTargetDescriptor => {
  try {
    return decodeTarget({
      targetRef: row.target_ref,
      targetClass: row.target_class,
      adapterRef: row.adapter_ref,
      ownerRef,
      compatibilityRef: row.compatibility_ref,
      isolation: row.isolation,
      dataPosture: row.data_posture,
      health: row.health,
    });
  } catch {
    throw new PortableSessionCommandRunnerError(
      "target_mismatch",
      "portable target authority is invalid",
    );
  }
};

const sameSet = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean =>
  left.length === right.length && left.every((value) => right.includes(value));

/**
 * Resolves one durable execution claim into the existing canonical move
 * runtime. This resolver does not create movement authority. It only binds
 * the accepted command, current authority, target directory, and authenticated
 * refs-only capability, Pylon, and artifact facts to that exact claim.
 */
export class PostgresPortableSessionCommandResolver implements PortableSessionCommandResolver {
  private readonly now: () => string;
  private readonly authorityReader: typeof readPortableSessionAuthoritySnapshot;

  constructor(private readonly config: PostgresPortableSessionCommandResolverConfig) {
    this.now = config.now ?? (() => new Date().toISOString());
    this.authorityReader = config.readAuthoritySnapshot ?? readPortableSessionAuthoritySnapshot;
  }

  async resolve(claim: PortableCommandExecutionClaim): Promise<PortableSessionMoveRuntimeInput> {
    assertRefsOnly(claim, "portable execution claim contains private material");
    const now = new Date(this.now());
    if (
      !["claimed", "pending_reconcile"].includes(claim.state) ||
      new Date(claim.leaseExpiresAt) <= now
    ) {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable execution claim is not active",
      );
    }
    const snapshot = await this.authorityReader(this.config.sql, {
      sessionRef: claim.sessionRef,
      ownerUserId: claim.ownerRef,
    });
    if (snapshot === null) {
      throw new PortableSessionCommandRunnerError(
        "authority_missing",
        "portable session authority is absent",
      );
    }
    return this.resolveSnapshot(claim, snapshot, now);
  }

  private async resolveSnapshot(
    claim: PortableCommandExecutionClaim,
    snapshot: PortableSessionAuthoritySnapshot,
    now: Date,
  ): Promise<PortableSessionMoveRuntimeInput> {
    assertRefsOnly(snapshot, "portable authority contains private material");
    const session = snapshot.session;
    if (session.owner_user_id !== claim.ownerRef || session.session_ref !== claim.sessionRef) {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable authority owner or session differs from the claim",
      );
    }
    const commandRow = snapshot.commands.find((row) => row.command_ref === claim.commandRef);
    if (commandRow === undefined) {
      throw new PortableSessionCommandRunnerError(
        "authority_missing",
        "portable command authority is absent",
      );
    }
    let command: ReturnType<typeof decodeCommand>;
    try {
      command = decodeCommand(parseJson(commandRow.command_json));
    } catch {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable command authority is invalid",
      );
    }
    if (
      command.commandRef !== claim.commandRef ||
      command.ownerRef !== claim.ownerRef ||
      command.sessionRef !== claim.sessionRef ||
      command.kind !== claim.commandKind ||
      command.expectedAttachmentRef !== claim.sourceAttachmentRef ||
      command.expectedGeneration !== claim.sourceGeneration ||
      command.destinationTargetRef !== claim.destinationTargetRef ||
      fingerprint(command) !== claim.commandFingerprint ||
      commandRow.expected_attachment_ref !== claim.sourceAttachmentRef ||
      Number(commandRow.expected_generation) !== claim.sourceGeneration ||
      commandRow.destination_target_ref !== claim.destinationTargetRef ||
      (commandRow.status !== "accepted" && commandRow.status !== "completed") ||
      new Date(command.expiresAt) <= now
    ) {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable command bytes or status differ from the execution claim",
      );
    }

    const sourceAttachment = snapshot.attachments.find(
      (row) => row.attachment_ref === claim.sourceAttachmentRef,
    );
    if (
      sourceAttachment === undefined ||
      Number(sourceAttachment.generation) !== claim.sourceGeneration ||
      sourceAttachment.target_ref !== claim.executorEnvironmentRef
    ) {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable source attachment differs from the execution claim",
      );
    }
    if (
      commandRow.status === "accepted" &&
      (session.current_attachment_ref !== claim.sourceAttachmentRef ||
        Number(session.current_attachment_generation) !== claim.sourceGeneration)
    ) {
      throw new PortableSessionCommandRunnerError(
        "authority_mismatch",
        "portable source generation is stale",
      );
    }

    const sourceRow = snapshot.targets.find(
      (row) => row.target_ref === claim.executorEnvironmentRef,
    );
    const destinationRow = snapshot.targets.find(
      (row) => row.target_ref === claim.destinationTargetRef,
    );
    if (sourceRow === undefined || destinationRow === undefined) {
      throw new PortableSessionCommandRunnerError(
        "target_mismatch",
        "portable source or destination target is not authorized",
      );
    }
    const source = targetFromRow(sourceRow, claim.ownerRef);
    const destination = targetFromRow(destinationRow, claim.ownerRef);
    if (
      source.health !== "ready" ||
      destination.health !== "ready" ||
      source.targetRef === destination.targetRef ||
      source.targetRef !== claim.executorEnvironmentRef ||
      destination.targetRef !== claim.destinationTargetRef
    ) {
      throw new PortableSessionCommandRunnerError(
        "target_mismatch",
        "portable target health or identity differs from the execution claim",
      );
    }
    this.assertBrokerTargets(source, destination);

    let decodedSourceLeaseRefs: ReadonlyArray<string>;
    try {
      decodedSourceLeaseRefs = decodeRefs(parseJson(sourceAttachment.capability_lease_refs_json));
    } catch {
      throw new PortableSessionCommandRunnerError(
        "capability_mismatch",
        "source capability lease authority is invalid",
      );
    }
    const sourceLeaseRefs = exactRefs(decodedSourceLeaseRefs, "source capability leases");
    const destinationAttachmentRef = refDigest(
      "attachment.portable",
      claim.claimRef,
      claim.destinationTargetRef,
    );
    const factScope = {
      commandExecutionClaimRef: claim.claimRef,
      ownerRef: claim.ownerRef,
      sessionRef: claim.sessionRef,
      sourceAttachmentRef: claim.sourceAttachmentRef,
      sourceGeneration: claim.sourceGeneration,
      sourceTargetRef: claim.executorEnvironmentRef,
      destinationAttachmentRef,
      destinationGeneration: claim.sourceGeneration + 1,
      destinationTargetRef: claim.destinationTargetRef,
      sourceLeaseRefs,
    } as const;
    const grantFacts = await this.config.capabilityGrantFacts.resolve(factScope);
    const capabilityTransfers = this.capabilityTransfers(claim, sourceLeaseRefs, grantFacts, now);
    const operationExpiresAt = new Date(
      Math.min(new Date(command.expiresAt).valueOf(), new Date(claim.leaseExpiresAt).valueOf()),
    ).toISOString();
    const targetConfig = (target: PortableTargetDescriptor) => ({
      sql: this.config.sql,
      commandExecutionClaim: claim,
      target,
      operationExpiresAt,
      resolvePylonRef: (
        scope: Readonly<{
          ownerRef: string;
          targetRef: string;
          commandExecutionClaimRef: string;
        }>,
      ) => this.resolvePylon(scope),
      resolveCheckpointBundle: (artifact: PortablePhaseTargetCheckpointArtifact) =>
        this.resolveArtifact(claim, artifact),
      now: this.now,
    });

    return {
      moveRef: claim.claimRef,
      move: {
        command,
        destinationAttachmentRef,
        capabilityTransfers,
        source: new PostgresPortablePhaseTarget(targetConfig(source)),
        destination: new PostgresPortablePhaseTarget(targetConfig(destination)),
      },
      broker: this.config.broker,
    };
  }

  private assertBrokerTargets(
    source: PortableTargetDescriptor,
    destination: PortableTargetDescriptor,
  ): void {
    for (const target of [source, destination]) {
      const binding = this.config.broker.targets.find(
        (candidate) => candidate.targetRef === target.targetRef,
      );
      const adapter = this.config.broker.adapters.find(
        (candidate) => candidate.adapterRef === target.adapterRef,
      );
      if (
        binding === undefined ||
        binding.targetClass !== target.targetClass ||
        binding.adapterRef !== target.adapterRef ||
        binding.ready !== true ||
        adapter === undefined ||
        adapter.targetClass !== target.targetClass
      ) {
        throw new PortableSessionCommandRunnerError(
          "target_mismatch",
          "portable broker target binding differs from durable target authority",
        );
      }
    }
  }

  private capabilityTransfers(
    claim: PortableCommandExecutionClaim,
    sourceLeaseRefs: ReadonlyArray<string>,
    facts: ReadonlyArray<PortableCommandCapabilityGrantFact>,
    now: Date,
  ): ReadonlyArray<PortableCapabilityTransfer> {
    assertRefsOnly(facts, "portable capability facts contain private material");
    const factSources = exactRefs(
      facts.map((fact) => fact.sourceLeaseRef),
      "capability fact source leases",
    );
    if (!sameSet(factSources, sourceLeaseRefs)) {
      throw new PortableSessionCommandRunnerError(
        "capability_mismatch",
        "portable capability facts do not cover the exact source lease set",
      );
    }
    const transfers = facts.map((fact) => {
      if (
        !SAFE_REF.test(fact.destinationSourceGrantRef) ||
        new Date(fact.expiresAt) <= now ||
        new Date(fact.expiresAt) > new Date(claim.leaseExpiresAt)
      ) {
        throw new PortableSessionCommandRunnerError(
          "capability_mismatch",
          "portable capability grant fact is invalid or outside the claim lease",
        );
      }
      return {
        sourceLeaseRef: fact.sourceLeaseRef,
        destinationLeaseRef: refDigest(
          "lease.portable",
          claim.claimRef,
          fact.sourceLeaseRef,
          claim.destinationTargetRef,
        ),
        destinationSourceGrantRef: fact.destinationSourceGrantRef,
        expiresAt: fact.expiresAt,
      };
    });
    exactRefs(
      transfers.map((transfer) => transfer.destinationLeaseRef),
      "destination capability leases",
    );
    return transfers;
  }

  private async resolvePylon(
    scope: Readonly<{
      ownerRef: string;
      targetRef: string;
      commandExecutionClaimRef: string;
    }>,
  ): Promise<string> {
    const binding = await this.config.pylonBindings.resolve(scope);
    assertRefsOnly(binding, "portable Pylon binding contains private material");
    if (
      binding.ownerRef !== scope.ownerRef ||
      binding.targetRef !== scope.targetRef ||
      binding.commandExecutionClaimRef !== scope.commandExecutionClaimRef ||
      !SAFE_REF.test(binding.pylonRef)
    ) {
      throw new PortableSessionCommandRunnerError(
        "pylon_mismatch",
        "portable Pylon binding differs from the exact phase target scope",
      );
    }
    return binding.pylonRef;
  }

  private async resolveArtifact(
    claim: PortableCommandExecutionClaim,
    artifact: PortablePhaseTargetCheckpointArtifact,
  ): Promise<PortableCheckpointBundle> {
    assertRefsOnly(artifact, "portable checkpoint artifact contains private material");
    const bundle = await this.config.checkpointArtifacts.resolve({
      commandExecutionClaimRef: claim.claimRef,
      ownerRef: claim.ownerRef,
      sessionRef: claim.sessionRef,
      artifact,
    });
    assertRefsOnly(bundle, "portable checkpoint bundle contains private material");
    if (
      bundle.checkpoint.checkpointRef !== artifact.checkpointRef ||
      bundle.checkpoint.digest !== artifact.checkpointDigest ||
      bundle.checkpoint.sessionRef !== claim.sessionRef ||
      bundle.checkpoint.sourceAttachmentRef !== claim.sourceAttachmentRef ||
      bundle.checkpoint.sourceGeneration !== claim.sourceGeneration ||
      bundle.executionBinding.ownerRef !== claim.ownerRef ||
      bundle.executionBinding.sessionRef !== claim.sessionRef
    ) {
      throw new PortableSessionCommandRunnerError(
        "artifact_mismatch",
        "portable checkpoint artifact differs from the exact execution claim",
      );
    }
    return bundle;
  }
}

export type PostgresPortableSessionCommandRunnerConfig = Readonly<{
  sql: SyncSql;
  transaction: <A>(run: (writer: SyncTransactionWriter) => Promise<A>) => Promise<A>;
  broker: PortableSessionMoveRuntimeBrokerConfig;
  pylonBindings: PortableCommandPylonBindingResolver;
  capabilityGrantFacts: PortableCommandCapabilityGrantFactResolver;
  checkpointArtifacts: PortableCommandCheckpointArtifactResolver;
  now?: () => string;
}>;

/** Production entry point for one exact durable portable command claim. */
export class PostgresPortableSessionCommandRunner {
  private readonly consumer: PortableSessionCommandConsumer;

  constructor(config: PostgresPortableSessionCommandRunnerConfig) {
    const queue = new PostgresPortableSessionCommandQueue(config.sql, config.now);
    const runtime = new PostgresPortableSessionMoveRuntime({
      sql: config.sql,
      transaction: config.transaction,
    });
    const resolver = new PostgresPortableSessionCommandResolver({
      sql: config.sql,
      broker: config.broker,
      pylonBindings: config.pylonBindings,
      capabilityGrantFacts: config.capabilityGrantFacts,
      checkpointArtifacts: config.checkpointArtifacts,
      ...(config.now === undefined ? {} : { now: config.now }),
    });
    this.consumer = new PortableSessionCommandConsumer({
      queue,
      resolver,
      runtime,
      ...(config.now === undefined ? {} : { now: config.now }),
    });
  }

  execute(
    request: PortableCommandExecutionClaimRequest,
  ): Promise<PortableSessionCommandConsumerResult> {
    return this.consumer.execute(request);
  }
}
