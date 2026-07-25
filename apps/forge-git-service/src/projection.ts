import { randomUUID } from "node:crypto";

import { Context, Effect, Layer, Option, Schema } from "effect";

import { ForgeGitDatabase } from "./database.js";
import type { ForgeGitMirrorReceipt, ForgeGitRef, ForgeGitSession } from "./model.js";

export const ForgeGitProjectionReceipt = Schema.Struct({
  projectedAt: Schema.String,
  receivePackRef: Schema.String,
  refCount: Schema.Number,
});
export interface ForgeGitProjectionReceipt extends Schema.Schema.Type<
  typeof ForgeGitProjectionReceipt
> {}

export class ForgeGitProjectionError extends Schema.TaggedErrorClass<ForgeGitProjectionError>()(
  "ForgeGitProjectionError",
  {
    operation: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export interface ForgeGitProjectionShape {
  readonly projectReceive: (input: {
    readonly mirrorReceipt: Option.Option<ForgeGitMirrorReceipt>;
    readonly refsAfter: ReadonlyArray<ForgeGitRef>;
    readonly refsBefore: ReadonlyArray<ForgeGitRef>;
    readonly repositoryRef: string;
    readonly session: ForgeGitSession;
    readonly tenantRef: string;
  }) => Effect.Effect<ForgeGitProjectionReceipt, ForgeGitProjectionError>;
}

export class ForgeGitProjection extends Context.Service<
  ForgeGitProjection,
  ForgeGitProjectionShape
>()("@openagentsinc/forge-git-service/Projection") {}

export const layerProjection = Layer.effect(
  ForgeGitProjection,
  Effect.gen(function* () {
    const database = yield* ForgeGitDatabase;

    const projectReceive = Effect.fn("ForgeGitProjection.projectReceive")(function* (
      input: Parameters<ForgeGitProjectionShape["projectReceive"]>[0],
    ) {
      const projectedAt = new Date().toISOString();
      const receivePackRef = `receive-pack.forge.stock-git.${randomUUID()}`;
      const changeRef = `change.forge.stock-git.${randomUUID()}`;
      const mirror = Option.getOrUndefined(input.mirrorReceipt);
      const packEvidence = mirror?.evidence.find((item) => item.objectKey.endsWith(".pack"));
      const packfileRef = mirror?.manifestKey ?? `mirror.unavailable.${receivePackRef}`;
      const packfileSha256 = packEvidence?.sha256 ?? "0".repeat(64);
      const packfileBytes = packEvidence?.bytes ?? 0;
      const before = new Map(input.refsBefore.map((ref) => [ref.refName, ref.objectId]));
      const after = new Map(input.refsAfter.map((ref) => [ref.refName, ref.objectId]));
      const updates = [
        ...input.refsAfter
          .filter((ref) => before.get(ref.refName) !== ref.objectId)
          .map((ref) => ({
            action: before.has(ref.refName) ? "update" : "create",
            newObjectId: ref.objectId,
            oldObjectId: before.get(ref.refName) ?? "0".repeat(40),
            refName: ref.refName,
          })),
        ...input.refsBefore
          .filter((ref) => !after.has(ref.refName))
          .map((ref) => ({
            action: "delete",
            newObjectId: "0".repeat(ref.objectId.length),
            oldObjectId: ref.objectId,
            refName: ref.refName,
          })),
      ];
      const sourceRefs = [
        "issue.public.github.OpenAgentsInc.openagents.9244",
        receivePackRef,
        ...(mirror === undefined ? [] : [mirror.manifestKey]),
      ];

      yield* Effect.tryPromise({
        try: () =>
          database.sql.begin(async (sql) => {
            await sql`
                INSERT INTO forge_git_receive_pack_intakes (
                  tenant_ref,
                  receive_pack_ref,
                  repository_ref,
                  token_ref,
                  subject_ref,
                  change_ref,
                  packfile_ref,
                  packfile_sha256,
                  packfile_bytes,
                  object_format,
                  state,
                  command_count,
                  ref_updates_json,
                  source_refs_json,
                  rejection_code,
                  rejection_reason,
                  created_at,
                  updated_at
                ) VALUES (
                  ${input.tenantRef},
                  ${receivePackRef},
                  ${input.repositoryRef},
                  ${input.session.tokenRef},
                  ${input.session.subjectRef},
                  ${changeRef},
                  ${packfileRef},
                  ${packfileSha256},
                  ${packfileBytes},
                  'sha1',
                  'accepted',
                  ${updates.length},
                  ${JSON.stringify(updates)},
                  ${JSON.stringify(sourceRefs)},
                  NULL,
                  NULL,
                  ${projectedAt},
                  ${projectedAt}
                )
              `;

            for (const ref of input.refsAfter) {
              await sql`
                  INSERT INTO forge_git_refs (
                    tenant_ref,
                    repository_ref,
                    ref_name,
                    object_id,
                    previous_object_id,
                    object_format,
                    state,
                    updated_by_change_ref,
                    updated_by_packfile_ref,
                    updated_by_receive_pack_ref,
                    source_refs_json,
                    created_at,
                    updated_at
                  ) VALUES (
                    ${input.tenantRef},
                    ${input.repositoryRef},
                    ${ref.refName},
                    ${ref.objectId},
                    ${before.get(ref.refName) ?? null},
                    'sha1',
                    'active',
                    ${changeRef},
                    ${packfileRef},
                    ${receivePackRef},
                    ${JSON.stringify(sourceRefs)},
                    ${projectedAt},
                    ${projectedAt}
                  )
                  ON CONFLICT (tenant_ref, repository_ref, ref_name)
                  DO UPDATE SET
                    object_id = EXCLUDED.object_id,
                    previous_object_id = forge_git_refs.object_id,
                    object_format = EXCLUDED.object_format,
                    state = 'active',
                    updated_by_change_ref = EXCLUDED.updated_by_change_ref,
                    updated_by_packfile_ref = EXCLUDED.updated_by_packfile_ref,
                    updated_by_receive_pack_ref =
                      EXCLUDED.updated_by_receive_pack_ref,
                    source_refs_json = EXCLUDED.source_refs_json,
                    updated_at = EXCLUDED.updated_at
                `;
              await sql`
                  INSERT INTO forge_git_objects (
                    tenant_ref,
                    repository_ref,
                    object_id,
                    object_format,
                    packfile_ref,
                    packfile_sha256,
                    first_seen_at,
                    latest_seen_at,
                    source_refs_json
                  ) VALUES (
                    ${input.tenantRef},
                    ${input.repositoryRef},
                    ${ref.objectId},
                    'sha1',
                    ${packfileRef},
                    ${packfileSha256},
                    ${projectedAt},
                    ${projectedAt},
                    ${JSON.stringify(sourceRefs)}
                  )
                  ON CONFLICT (tenant_ref, repository_ref, object_id)
                  DO UPDATE SET
                    latest_seen_at = EXCLUDED.latest_seen_at,
                    source_refs_json = EXCLUDED.source_refs_json
                `;
            }

            for (const ref of input.refsBefore) {
              if (after.has(ref.refName)) continue;
              await sql`
                  UPDATE forge_git_refs
                  SET
                    object_id = NULL,
                    previous_object_id = ${ref.objectId},
                    state = 'deleted',
                    updated_by_change_ref = ${changeRef},
                    updated_by_packfile_ref = ${packfileRef},
                    updated_by_receive_pack_ref = ${receivePackRef},
                    source_refs_json = ${JSON.stringify(sourceRefs)},
                    updated_at = ${projectedAt}
                  WHERE tenant_ref = ${input.tenantRef}
                    AND repository_ref = ${input.repositoryRef}
                    AND ref_name = ${ref.refName}
                `;
            }
          }),
        catch: (cause) =>
          new ForgeGitProjectionError({
            cause,
            operation: "ForgeGitProjection.projectReceive",
          }),
      });

      return ForgeGitProjectionReceipt.make({
        projectedAt,
        receivePackRef,
        refCount: input.refsAfter.length,
      });
    });

    return ForgeGitProjection.of({ projectReceive });
  }),
);

export const layerNoopProjection = Layer.succeed(
  ForgeGitProjection,
  ForgeGitProjection.of({
    projectReceive: Effect.fn("ForgeGitProjection.noop.projectReceive")(function* (input) {
      return ForgeGitProjectionReceipt.make({
        projectedAt: new Date().toISOString(),
        receivePackRef: `receive-pack.test.${randomUUID()}`,
        refCount: input.refsAfter.length,
      });
    }),
  }),
);
