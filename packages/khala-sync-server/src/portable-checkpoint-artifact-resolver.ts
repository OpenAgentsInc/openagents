import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  PortableCheckpointCustodyObjectManifestSchema,
  PortableCommandExecutionClaimSchema,
  PortableRef,
  PylonPortableCheckpointBundleSchema,
  type PortableCheckpointCustodyObjectManifest,
  type PortableCommandExecutionClaim,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import type { PortablePhaseTargetCheckpointArtifact } from "./portable-phase-target-adapter.js";
import { computePortableAgentGraphDigest } from "./portable-session-authority.js";
import type { PortableCommandCheckpointArtifactResolver } from "./portable-session-command-runner.js";
import {
  computePortableCheckpointDigest,
  type PortableCheckpointBundle,
} from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";

const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

const Sha256Digest = Schema.String.check(Schema.isPattern(SHA256));
const PortableTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);
const CustodyPayloadSchema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_checkpoint_artifact_custody_payload.v3"),
  checkpointRef: PortableRef,
  artifactRef: PortableRef,
  digest: Sha256Digest,
  bundle: PylonPortableCheckpointBundleSchema,
  createdAt: PortableTimestamp,
  expiresAt: PortableTimestamp,
  bytesBase64: Schema.String.check(Schema.isPattern(BASE64)),
});
const EncryptedEnvelopeSchema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_checkpoint_artifact_custody_encrypted.v2"),
  algorithm: Schema.Literal("aes-256-gcm"),
  objectRef: PortableRef,
  policy: Schema.Literals(["owner_managed", "openagents_managed"]),
  keyRef: PortableRef,
  nonceBase64: Schema.String.check(Schema.isPattern(BASE64)),
  authTagBase64: Schema.String.check(Schema.isPattern(BASE64)),
  ciphertextBase64: Schema.String.check(Schema.isPattern(BASE64)),
});

const decodeManifest = Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema);
const decodeClaim = Schema.decodeUnknownSync(PortableCommandExecutionClaimSchema);
const decodePayload = Schema.decodeUnknownSync(CustodyPayloadSchema);
const decodeEnvelope = Schema.decodeUnknownSync(EncryptedEnvelopeSchema);
const decodeBundle = Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema);

const digest = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const stableFailureRef = (code: string, scopeRef: string): string =>
  `failure.portable-checkpoint-artifact.${createHash("sha256")
    .update(`${code}\u0000${scopeRef}`)
    .digest("hex")}`;
const exact = (left: unknown, right: unknown): boolean =>
  canonicalJson(left) === canonicalJson(right);

export class PortableCommittedCheckpointArtifactError extends Schema.TaggedErrorClass<PortableCommittedCheckpointArtifactError>()(
  "PortableCommittedCheckpointArtifactError",
  {
    code: Schema.Literals([
      "invalid_scope",
      "authority_missing",
      "authority_mismatch",
      "phase_mismatch",
      "artifact_unavailable",
      "artifact_tombstoned",
      "artifact_expired",
      "manifest_mismatch",
      "ciphertext_mismatch",
      "custody_unavailable",
      "payload_mismatch",
    ]),
    failureRef: PortableRef,
  },
) {}

export type PortableCommittedCheckpointArtifactScope = Readonly<{
  commandExecutionClaimRef: string;
  ownerRef: string;
  sessionRef: string;
  artifact: PortablePhaseTargetCheckpointArtifact;
}>;

/** The reader returns newly owned bytes. The resolver always clears them. */
export type PortablePrivateCheckpointArtifactObject = Readonly<{
  state: "pending" | "committed" | "deleted";
  tombstoned: boolean;
  phaseOperationRef: string;
  phaseResultRef: string;
  manifest: unknown;
  encryptedObjectBytes: Uint8Array;
}>;

export type PortablePrivateCheckpointArtifactReader = Readonly<{
  read: (objectRef: string) => Promise<PortablePrivateCheckpointArtifactObject>;
}>;

/** The adapter owns custody authority. Raw keys never cross this interface. */
export type PortableCheckpointCustodyDecryptor = Readonly<{
  decrypt: (
    input: Readonly<{
      manifest: PortableCheckpointCustodyObjectManifest;
      encryptedObjectBytes: Uint8Array;
    }>,
  ) => Promise<Uint8Array>;
}>;

type ExecutionRow = Readonly<{ claim_json: unknown }>;
type PhaseRow = Readonly<{
  operation_ref: string;
  command_execution_claim_ref: string;
  owner_user_id: string;
  session_ref: string;
  attachment_ref: string;
  attachment_generation: string | number;
  target_ref: string;
  pylon_ref: string;
  checkpoint_ref: string | null;
  expires_at: Date | string;
  state: string;
  result_ref: string | null;
  result_status: string | null;
  result_checkpoint_ref: string | null;
  result_checkpoint_object_ref: string | null;
  result_checkpoint_digest: string | null;
  completed_at: Date | string | null;
}>;

export type PortableCommittedCheckpointArtifactResolverConfig = Readonly<{
  sql: SyncSql;
  objects: PortablePrivateCheckpointArtifactReader;
  custody: PortableCheckpointCustodyDecryptor;
  now?: () => string;
}>;

/** Read-only committed artifact resolver. It does not schedule or retain work. */
export class PortableCommittedCheckpointArtifactResolver {
  private readonly now: () => string;

  constructor(private readonly config: PortableCommittedCheckpointArtifactResolverConfig) {
    this.now = config.now ?? (() => new Date().toISOString());
  }

  readonly resolveEffect = Effect.fn("PortableCommittedCheckpointArtifactResolver.resolve")(
    (scope: PortableCommittedCheckpointArtifactScope) => {
      const nowInstant = (scopeRef: string) => this.nowInstant(scopeRef);
      const readClaim = (at: Date) => this.readClaim(scope, at);
      const readPhase = (claim: PortableCommandExecutionClaim, at: Date) =>
        this.readPhase(scope, claim, at);
      const readObject = () => this.config.objects.read(scope.artifact.checkpointObjectRef);
      const artifactUnavailable = () =>
        this.failure("artifact_unavailable", scope.artifact.checkpointObjectRef);
      const validatePrivateObject = (
        claim: PortableCommandExecutionClaim,
        phase: PhaseRow,
        privateObject: PortablePrivateCheckpointArtifactObject,
        at: Date,
      ) => this.validatePrivateObject(scope, claim, phase, privateObject, at);
      const validateEnvelope = (
        manifest: PortableCheckpointCustodyObjectManifest,
        encrypted: Uint8Array,
      ) => this.validateEnvelope(manifest, encrypted);
      const decrypt = (
        manifest: PortableCheckpointCustodyObjectManifest,
        encryptedObjectBytes: Uint8Array,
      ) => this.config.custody.decrypt({ manifest, encryptedObjectBytes });
      const custodyUnavailable = (objectRef: string) =>
        this.failure("custody_unavailable", objectRef);
      const decodeResolvedPayload = (
        claim: PortableCommandExecutionClaim,
        manifest: PortableCheckpointCustodyObjectManifest,
        plaintext: Uint8Array,
      ) => this.decodeAndValidatePayload(scope, claim, manifest, plaintext);
      return Effect.gen(function* () {
        const now = nowInstant(scope.commandExecutionClaimRef);
        const claim = yield* readClaim(now);
        const phase = yield* readPhase(claim, now);
        const privateObject = yield* Effect.tryPromise({
          try: readObject,
          catch: (): PortableCommittedCheckpointArtifactError => artifactUnavailable(),
        });
        if (!(privateObject.encryptedObjectBytes instanceof Uint8Array)) {
          throw artifactUnavailable();
        }
        const encrypted = privateObject.encryptedObjectBytes;
        let plaintext: Uint8Array | undefined;
        try {
          const manifest = validatePrivateObject(claim, phase, privateObject, now);
          validateEnvelope(manifest, encrypted);
          plaintext = yield* Effect.tryPromise({
            try: () => decrypt(manifest, encrypted),
            catch: (): PortableCommittedCheckpointArtifactError =>
              custodyUnavailable(manifest.objectRef),
          });
          if (!(plaintext instanceof Uint8Array) || plaintext.byteLength === 0) {
            throw custodyUnavailable(manifest.objectRef);
          }
          return decodeResolvedPayload(claim, manifest, plaintext);
        } finally {
          encrypted.fill(0);
          plaintext?.fill(0);
        }
      });
    },
  );

  readonly resolve = (
    scope: PortableCommittedCheckpointArtifactScope,
  ): Promise<PortableCheckpointBundle> => Effect.runPromise(this.resolveEffect(scope));

  readonly commandResolver = (): PortableCommandCheckpointArtifactResolver => ({
    resolve: this.resolve,
  });

  private failure(code: PortableCommittedCheckpointArtifactError["code"], scopeRef: string) {
    return new PortableCommittedCheckpointArtifactError({
      code,
      failureRef: stableFailureRef(code, scopeRef),
    });
  }

  private nowInstant(scopeRef: string): Date {
    const now = new Date(this.now());
    if (!Number.isFinite(now.valueOf())) throw this.failure("invalid_scope", scopeRef);
    return now;
  }

  private readClaim(scope: PortableCommittedCheckpointArtifactScope, now: Date) {
    return Effect.tryPromise({
      try: async () => {
        const rows: ExecutionRow[] = await this.config.sql`
          SELECT jsonb_build_object(
            'schema', 'openagents.portable_command_execution.v1',
            'claimRef', claim_ref, 'commandRef', command_ref,
            'ownerRef', owner_user_id, 'sessionRef', session_ref,
            'commandKind', command_kind, 'commandFingerprint', command_fingerprint,
            'claimFingerprint', claim_fingerprint, 'sourceAttachmentRef', source_attachment_ref,
            'sourceGeneration', source_generation, 'destinationTargetRef', destination_target_ref,
            'executorEnvironmentRef', executor_environment_ref, 'workerInstanceRef', worker_instance_ref,
            'claimGeneration', claim_generation, 'leaseRevision', lease_revision, 'state', state,
            'claimedAt', claimed_at, 'leaseExpiresAt', lease_expires_at, 'updatedAt', updated_at,
            'terminalStatus', terminal_status, 'pendingReconcileRef', pending_reconcile_ref,
            'outcomeRef', outcome_ref, 'evidenceRefs', evidence_refs_json
          ) AS claim_json
          FROM khala_sync_portable_command_executions
          WHERE claim_ref = ${scope.commandExecutionClaimRef}
            AND owner_user_id = ${scope.ownerRef} AND session_ref = ${scope.sessionRef}
        `;
        if (rows[0] === undefined)
          throw this.failure("authority_missing", scope.commandExecutionClaimRef);
        const claim = decodeClaim(rows[0].claim_json);
        if (
          claim.claimRef !== scope.commandExecutionClaimRef ||
          claim.ownerRef !== scope.ownerRef ||
          claim.sessionRef !== scope.sessionRef ||
          claim.state !== "claimed" ||
          claim.terminalStatus !== null ||
          claim.pendingReconcileRef !== null ||
          claim.outcomeRef !== null ||
          new Date(claim.claimedAt) > now ||
          new Date(claim.leaseExpiresAt) <= now
        ) {
          throw this.failure("authority_mismatch", scope.commandExecutionClaimRef);
        }
        return claim;
      },
      catch: (cause) =>
        cause instanceof PortableCommittedCheckpointArtifactError
          ? cause
          : this.failure("authority_missing", scope.commandExecutionClaimRef),
    });
  }

  private readPhase(
    scope: PortableCommittedCheckpointArtifactScope,
    claim: PortableCommandExecutionClaim,
    now: Date,
  ) {
    return Effect.tryPromise({
      try: async () => {
        const rows: PhaseRow[] = await this.config.sql`
          SELECT operation_ref, command_execution_claim_ref, owner_user_id, session_ref,
                 attachment_ref, attachment_generation, target_ref, pylon_ref, checkpoint_ref,
                 expires_at, state, result_ref, result_status, result_checkpoint_ref,
                 result_checkpoint_object_ref, result_checkpoint_digest, completed_at
          FROM khala_sync_portable_phase_operations
          WHERE command_execution_claim_ref = ${claim.claimRef} AND kind = 'checkpoint-create'
        `;
        const phase = rows[0];
        if (
          phase === undefined ||
          phase.command_execution_claim_ref !== claim.claimRef ||
          phase.owner_user_id !== claim.ownerRef ||
          phase.session_ref !== claim.sessionRef ||
          phase.attachment_ref !== claim.sourceAttachmentRef ||
          Number(phase.attachment_generation) !== claim.sourceGeneration ||
          phase.target_ref !== claim.executorEnvironmentRef ||
          phase.checkpoint_ref !== scope.artifact.checkpointRef ||
          phase.state !== "completed" ||
          phase.result_status !== "completed" ||
          phase.result_ref === null ||
          phase.result_checkpoint_ref !== scope.artifact.checkpointRef ||
          phase.result_checkpoint_object_ref !== scope.artifact.checkpointObjectRef ||
          phase.result_checkpoint_digest !== scope.artifact.checkpointDigest ||
          phase.completed_at === null ||
          new Date(phase.completed_at) > now ||
          new Date(phase.expires_at) <= new Date(phase.completed_at)
        ) {
          throw this.failure("phase_mismatch", claim.claimRef);
        }
        return phase;
      },
      catch: (cause) =>
        cause instanceof PortableCommittedCheckpointArtifactError
          ? cause
          : this.failure("phase_mismatch", claim.claimRef),
    });
  }

  private validatePrivateObject(
    scope: PortableCommittedCheckpointArtifactScope,
    claim: PortableCommandExecutionClaim,
    phase: PhaseRow,
    privateObject: PortablePrivateCheckpointArtifactObject,
    now: Date,
  ) {
    if (privateObject.state !== "committed") {
      throw this.failure("artifact_unavailable", scope.artifact.checkpointObjectRef);
    }
    if (privateObject.tombstoned) {
      throw this.failure("artifact_tombstoned", scope.artifact.checkpointObjectRef);
    }
    let manifest: PortableCheckpointCustodyObjectManifest;
    try {
      manifest = decodeManifest(privateObject.manifest);
    } catch {
      throw this.failure("manifest_mismatch", scope.artifact.checkpointObjectRef);
    }
    const createdAt = new Date(manifest.createdAt);
    const expiresAt = new Date(manifest.expiresAt);
    if (
      privateObject.phaseOperationRef !== phase.operation_ref ||
      privateObject.phaseResultRef !== phase.result_ref ||
      manifest.objectRef !== scope.artifact.checkpointObjectRef ||
      manifest.checkpointRef !== scope.artifact.checkpointRef ||
      manifest.checkpointDigest !== scope.artifact.checkpointDigest ||
      !exact(manifest.commandClaim, claim) ||
      manifest.ownerRef !== claim.ownerRef ||
      manifest.sourcePylonRef !== phase.pylon_ref ||
      manifest.targetRef !== claim.destinationTargetRef ||
      manifest.sessionRef !== claim.sessionRef ||
      manifest.sourceAttachmentRef !== claim.sourceAttachmentRef ||
      manifest.sourceGeneration !== claim.sourceGeneration ||
      privateObject.encryptedObjectBytes.byteLength === 0 ||
      privateObject.encryptedObjectBytes.byteLength > manifest.byteLimit ||
      manifest.objectDigest !== digest(privateObject.encryptedObjectBytes) ||
      createdAt < new Date(claim.claimedAt) ||
      createdAt > now ||
      expiresAt <= now ||
      expiresAt > new Date(claim.leaseExpiresAt) ||
      manifest.retentionSeconds !== Math.floor((expiresAt.valueOf() - createdAt.valueOf()) / 1_000)
    ) {
      throw this.failure(
        expiresAt <= now ? "artifact_expired" : "manifest_mismatch",
        scope.artifact.checkpointObjectRef,
      );
    }
    return manifest;
  }

  private validateEnvelope(
    manifest: PortableCheckpointCustodyObjectManifest,
    encryptedObjectBytes: Uint8Array,
  ) {
    let ciphertext: Uint8Array | undefined;
    try {
      const envelope = decodeEnvelope(
        JSON.parse(new TextDecoder().decode(encryptedObjectBytes)) as unknown,
      );
      ciphertext = Uint8Array.from(Buffer.from(envelope.ciphertextBase64, "base64"));
      if (
        envelope.objectRef !== manifest.objectRef ||
        envelope.policy !== manifest.custodyPolicy ||
        envelope.keyRef !== manifest.keyRef ||
        ciphertext.byteLength === 0 ||
        digest(ciphertext) !== manifest.ciphertextDigest
      )
        throw new Error();
    } catch {
      throw this.failure("ciphertext_mismatch", manifest.objectRef);
    } finally {
      ciphertext?.fill(0);
    }
  }

  private decodeAndValidatePayload(
    scope: PortableCommittedCheckpointArtifactScope,
    claim: PortableCommandExecutionClaim,
    manifest: PortableCheckpointCustodyObjectManifest,
    plaintext: Uint8Array,
  ): PortableCheckpointBundle {
    let artifactBytes: Uint8Array | undefined;
    try {
      const payload = decodePayload(JSON.parse(new TextDecoder().decode(plaintext)) as unknown);
      artifactBytes = Uint8Array.from(Buffer.from(payload.bytesBase64, "base64"));
      const bundle = decodeBundle(payload.bundle);
      const payloadCreatedAt = new Date(payload.createdAt);
      const payloadExpiresAt = new Date(payload.expiresAt);
      if (
        payload.checkpointRef !== manifest.checkpointRef ||
        payload.artifactRef !== manifest.artifactRef ||
        payload.digest !== manifest.artifactDigest ||
        digest(artifactBytes) !== manifest.artifactDigest ||
        digest(canonicalJson(bundle)) !== manifest.bundleDigest ||
        bundle.checkpoint.checkpointRef !== scope.artifact.checkpointRef ||
        bundle.checkpoint.digest !== scope.artifact.checkpointDigest ||
        bundle.checkpoint.digest !== computePortableCheckpointDigest(bundle.checkpoint) ||
        bundle.checkpoint.graphDigest !== computePortableAgentGraphDigest(bundle.graph) ||
        bundle.checkpoint.sessionRef !== claim.sessionRef ||
        bundle.checkpoint.sourceAttachmentRef !== claim.sourceAttachmentRef ||
        bundle.checkpoint.sourceGeneration !== claim.sourceGeneration ||
        bundle.executionBinding.ownerRef !== claim.ownerRef ||
        bundle.executionBinding.sessionRef !== claim.sessionRef ||
        manifest.createdAt !==
          new Date(
            Math.max(payloadCreatedAt.valueOf(), new Date(claim.claimedAt).valueOf()),
          ).toISOString() ||
        manifest.expiresAt !==
          new Date(
            Math.min(payloadExpiresAt.valueOf(), new Date(claim.leaseExpiresAt).valueOf()),
          ).toISOString() ||
        FORBIDDEN_PRIVATE_MATERIAL.test(canonicalJson(bundle))
      )
        throw new Error();
      return bundle;
    } catch {
      throw this.failure("payload_mismatch", manifest.objectRef);
    } finally {
      artifactBytes?.fill(0);
    }
  }
}
