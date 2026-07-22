import type {
  PortablePrivateCheckpointArtifactReader,
  SyncSql,
} from "@openagentsinc/khala-sync-server";
import { Effect, Schema } from "effect";

import {
  PortableCheckpointArtifactError,
  type makePortableCheckpointArtifactService,
} from "./portable-checkpoint-artifact-service";

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const Sha256Digest = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/));
const PhaseBindingRowSchema = Schema.Struct({
  operation_ref: PortableRef,
  state: Schema.Literal("completed"),
  result_ref: PortableRef,
  result_status: Schema.Literal("completed"),
  result_checkpoint_object_ref: PortableRef,
  result_checkpoint_manifest_digest: Sha256Digest,
});
const decodePortableRef = Schema.decodeUnknownSync(PortableRef);
const decodePhaseBindingRow = Schema.decodeUnknownSync(PhaseBindingRowSchema);

type ArtifactService = Pick<
  ReturnType<typeof makePortableCheckpointArtifactService>,
  "readPrivateCommitted"
>;

export type PortablePrivateCheckpointArtifactReaderDependencies = Readonly<{
  sql: SyncSql;
  artifacts: ArtifactService;
}>;

const fail = (code: PortableCheckpointArtifactError["code"], operation: string) =>
  new PortableCheckpointArtifactError({ code, operation });

export const makePortablePrivateCheckpointArtifactReader = (
  dependencies: PortablePrivateCheckpointArtifactReaderDependencies,
): PortablePrivateCheckpointArtifactReader => {
  const readEffect = Effect.fn("PortablePrivateCheckpointArtifactReader.read")(function* (
    objectRefInput: string,
  ) {
    const objectRef = yield* Effect.try({
      try: () => decodePortableRef(objectRefInput),
      catch: () => fail("invalid", "private_phase_object_ref"),
    });
    const rows = yield* Effect.tryPromise({
      try: async () => {
        const result: Array<unknown> = await dependencies.sql`
          SELECT operation_ref, state, result_ref, result_status,
                 result_checkpoint_object_ref, result_checkpoint_manifest_digest
          FROM khala_sync_portable_phase_operations
          WHERE result_checkpoint_object_ref = ${objectRef}
        `;
        return result;
      },
      catch: () => fail("unavailable", "private_phase_binding_read"),
    });
    if (rows.length !== 1) {
      return yield* fail("conflict", "private_phase_binding");
    }
    const phase = yield* Effect.try({
      try: () => decodePhaseBindingRow(rows[0]),
      catch: () => fail("conflict", "private_phase_binding"),
    });
    if (phase.result_checkpoint_object_ref !== objectRef) {
      return yield* fail("conflict", "private_phase_binding");
    }
    const artifact = yield* dependencies.artifacts.readPrivateCommitted({
      manifestDigest: phase.result_checkpoint_manifest_digest,
      objectRef,
      phaseOperationRef: phase.operation_ref,
    });
    return {
      state: "committed" as const,
      tombstoned: false,
      phaseOperationRef: phase.operation_ref,
      phaseResultRef: phase.result_ref,
      manifest: artifact.manifest,
      encryptedObjectBytes: artifact.encryptedObjectBytes,
    };
  });

  return {
    read: (objectRef) => Effect.runPromise(readEffect(objectRef)),
  };
};
