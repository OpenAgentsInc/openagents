import { Effect, JSONSchema, Layer, Schema } from "effect";

import {
  BlobStore,
  CompiledArtifact,
  Hash,
  Policy,
  Receipt,
  type BlobRef,
  type DseSignature
} from "@openagentsinc/dse";

export type SqlTag = <T extends Record<string, unknown> = Record<string, unknown>>(
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => ReadonlyArray<T> | undefined;

export function initDseTables(sql: SqlTag) {
  // Compiled artifacts + active pointers (pointer-only promotion).
  sql`create table if not exists dse_artifacts (
    signature_id text not null,
    compiled_id  text not null,
    json         text not null,
    created_at   integer not null,
    primary key (signature_id, compiled_id)
  )`;

  sql`create table if not exists dse_active_artifacts (
    signature_id text primary key,
    compiled_id  text not null,
    updated_at   integer not null
  )`;

  // Minimal history so "rollback" can be implemented without additional infra.
  sql`create table if not exists dse_active_artifact_history (
    signature_id text not null,
    compiled_id  text not null,
    updated_at   integer not null
  )`;

  // Predict receipts (small, hash-first).
  sql`create table if not exists dse_receipts (
    id           text primary key,
    signature_id text not null,
    compiled_id  text not null,
    json         text not null,
    created_at   integer not null
  )`;

  // Content-addressed blobs for large prompt context.
  sql`create table if not exists dse_blobs (
    id         text primary key,
    mime       text,
    text       text not null,
    size       integer not null,
    created_at integer not null
  )`;
}

function nowMs(): number {
  return Date.now();
}

function byteLengthUtf8(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

export type EnsureDefaultArtifactsResult = {
  readonly ok: boolean;
  readonly installed: ReadonlyArray<{
    readonly signatureId: string;
    readonly compiled_id: string;
  }>;
  readonly skipped: ReadonlyArray<{
    readonly signatureId: string;
    readonly reason: "already_active";
  }>;
  readonly errors: ReadonlyArray<{
    readonly signatureId: string;
    readonly message: string;
  }>;
};

export async function ensureDefaultArtifacts(
  sql: SqlTag,
  signatures: ReadonlyArray<DseSignature<any, any>>
): Promise<EnsureDefaultArtifactsResult> {
  const installed: Array<{ signatureId: string; compiled_id: string }> = [];
  const skipped: Array<{ signatureId: string; reason: "already_active" }> = [];
  const errors: Array<{ signatureId: string; message: string }> = [];

  for (const signature of signatures) {
    try {
      const activeRows =
        sql<{ compiled_id: string }>`
          select compiled_id from dse_active_artifacts where signature_id = ${signature.id}
        ` || [];

      const activeCompiledId = activeRows[0]?.compiled_id ?? null;
      if (activeCompiledId) {
        const artifactOk =
          (sql<{ ok: number }>`
            select 1 as ok from dse_artifacts
            where signature_id = ${signature.id} and compiled_id = ${activeCompiledId}
            limit 1
          ` || [])[0]?.ok === 1;

        if (artifactOk) {
          skipped.push({ signatureId: signature.id, reason: "already_active" });
          continue;
        }
      }

      const inputSchemaHash = await Hash.sha256IdFromCanonicalJson(
        JSONSchema.make(signature.input)
      );
      const outputSchemaHash = await Hash.sha256IdFromCanonicalJson(
        JSONSchema.make(signature.output)
      );
      const promptIrHash = await Hash.sha256IdFromCanonicalJson(signature.prompt);
      const paramsHash = await Hash.sha256IdFromCanonicalJson(signature.defaults.params);

      // For now, compiled_id == paramsHash (see packages/dse/src/hashes.ts).
      const compiled_id = paramsHash;
      if (activeCompiledId && activeCompiledId !== compiled_id) {
        errors.push({
          signatureId: signature.id,
          message:
            "Active artifact pointer exists but artifact is missing; refusing to override active compiled_id."
        });
        continue;
      }

      const artifact: CompiledArtifact.DseCompiledArtifactV1 = {
        format: "openagents.dse.compiled_artifact",
        formatVersion: 1,
        signatureId: signature.id,
        compiled_id,
        createdAt: new Date().toISOString(),
        hashes: {
          inputSchemaHash,
          outputSchemaHash,
          promptIrHash,
          paramsHash
        },
        params: signature.defaults.params,
        eval: {
          evalVersion: 1,
          kind: "unscored",
          notes: "default-installed (runtime)"
        },
        optimizer: {
          id: "default_install.v1"
        },
        provenance: {}
      };

      const encoded = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(
        artifact
      );
      const json = JSON.stringify(encoded);
      const createdAtMs = Date.parse(artifact.createdAt);
      const createdAt = Number.isFinite(createdAtMs) ? createdAtMs : nowMs();

      sql`
        insert into dse_artifacts (signature_id, compiled_id, json, created_at)
        values (${artifact.signatureId}, ${artifact.compiled_id}, ${json}, ${createdAt})
        on conflict(signature_id, compiled_id) do nothing
      `;

      const ts = nowMs();
      sql`
        insert into dse_active_artifact_history (signature_id, compiled_id, updated_at)
        values (${artifact.signatureId}, ${artifact.compiled_id}, ${ts})
      `;
      sql`
        insert into dse_active_artifacts (signature_id, compiled_id, updated_at)
        values (${artifact.signatureId}, ${artifact.compiled_id}, ${ts})
        on conflict(signature_id) do update set compiled_id = excluded.compiled_id, updated_at = excluded.updated_at
      `;

      installed.push({ signatureId: signature.id, compiled_id });
    } catch (cause) {
      const message =
        cause && typeof cause === "object" && "message" in cause
          ? String((cause as any).message)
          : "Failed to ensure default artifact";
      errors.push({ signatureId: signature.id, message });
    }
  }

  return { ok: errors.length === 0, installed, skipped, errors };
}

export function layerDseFromSql(sql: SqlTag): Layer.Layer<
  Policy.PolicyRegistryService | BlobStore.BlobStoreService | Receipt.ReceiptRecorderService
> {
  const policyLayer = Layer.succeed(
    Policy.PolicyRegistryService,
    Policy.PolicyRegistryService.of({
      getActive: (signatureId) =>
        Effect.try({
          try: () => {
            const rows =
              sql<{ compiled_id: string }>`
                select compiled_id from dse_active_artifacts where signature_id = ${signatureId}
              ` || [];
            const row = rows[0];
            return row ? { compiledId: row.compiled_id } : null;
          },
          catch: (cause) =>
            Policy.PolicyRegistryError.make({
              message: "Failed to read active DSE policy",
              cause
            })
        }),

      setActive: (signatureId, policy) =>
        Effect.try({
          try: () => {
            const ts = nowMs();
            sql`
              insert into dse_active_artifact_history (signature_id, compiled_id, updated_at)
              values (${signatureId}, ${policy.compiledId}, ${ts})
            `;
            sql`
              insert into dse_active_artifacts (signature_id, compiled_id, updated_at)
              values (${signatureId}, ${policy.compiledId}, ${ts})
              on conflict(signature_id) do update set compiled_id = excluded.compiled_id, updated_at = excluded.updated_at
            `;
          },
          catch: (cause) =>
            Policy.PolicyRegistryError.make({
              message: "Failed to set active DSE policy",
              cause
            })
        }),

      clearActive: (signatureId) =>
        Effect.try({
          try: () => void sql`delete from dse_active_artifacts where signature_id = ${signatureId}`,
          catch: (cause) =>
            Policy.PolicyRegistryError.make({
              message: "Failed to clear active DSE policy",
              cause
            })
        }),

      getArtifact: (signatureId, compiledId) =>
        Effect.try({
          try: () => {
            const rows =
              sql<{ json: string }>`
                select json from dse_artifacts
                where signature_id = ${signatureId} and compiled_id = ${compiledId}
              ` || [];
            const row = rows[0];
            if (!row) return null;
            const parsed: unknown = JSON.parse(row.json);
            return Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(
              parsed
            );
          },
          catch: (cause) =>
            Policy.PolicyRegistryError.make({
              message: "Failed to read DSE artifact",
              cause
            })
        }),

      putArtifact: (artifact) =>
        Effect.try({
          try: () => {
            const encoded = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(
              artifact
            );
            const json = JSON.stringify(encoded);
            const createdAtMs = Date.parse(artifact.createdAt);
            const createdAt = Number.isFinite(createdAtMs) ? createdAtMs : nowMs();
            sql`
              insert into dse_artifacts (signature_id, compiled_id, json, created_at)
              values (${artifact.signatureId}, ${artifact.compiled_id}, ${json}, ${createdAt})
              on conflict(signature_id, compiled_id) do nothing
            `;
          },
          catch: (cause) =>
            Policy.PolicyRegistryError.make({
              message: "Failed to write DSE artifact",
              cause
            })
        })
    })
  );

  const blobLayer = Layer.succeed(
    BlobStore.BlobStoreService,
    BlobStore.BlobStoreService.of({
      putText: (options) =>
        Effect.tryPromise({
          try: async () => {
            const id = await Hash.sha256IdFromString(options.text);
            const size = byteLengthUtf8(options.text);
            const ts = nowMs();

            sql`
              insert into dse_blobs (id, mime, text, size, created_at)
              values (${id}, ${options.mime ?? null}, ${options.text}, ${size}, ${ts})
              on conflict(id) do nothing
            `;

            const ref: BlobRef = {
              id,
              hash: id,
              size,
              ...(options.mime ? { mime: options.mime } : {})
            };
            return ref;
          },
          catch: (cause) =>
            BlobStore.BlobStoreError.make({
              message: "Failed to put text blob",
              cause
            })
        }),

      getText: (id) =>
        Effect.try({
          try: () => {
            const rows =
              sql<{ text: string }>`select text from dse_blobs where id = ${id}` ||
              [];
            const row = rows[0];
            return row ? row.text : null;
          },
          catch: (cause) =>
            BlobStore.BlobStoreError.make({
              message: "Failed to read blob text",
              cause
            })
        })
    })
  );

  const receiptLayer = Layer.succeed(
    Receipt.ReceiptRecorderService,
    Receipt.ReceiptRecorderService.of({
      record: (receipt) =>
        Effect.try({
          try: () => {
            const encoded = Schema.encodeSync(Receipt.PredictReceiptV1Schema)(receipt);
            const json = JSON.stringify(encoded);
            const ts = nowMs();
            sql`
              insert into dse_receipts (id, signature_id, compiled_id, json, created_at)
              values (${receipt.receiptId}, ${receipt.signatureId}, ${receipt.compiled_id}, ${json}, ${ts})
              on conflict(id) do nothing
            `;
          },
          catch: (cause) =>
            Receipt.ReceiptRecorderError.make({
              message: "Failed to record receipt",
              cause
            })
        })
    })
  );

  return Layer.mergeAll(policyLayer, blobLayer, receiptLayer);
}

export function rollbackActiveArtifact(sql: SqlTag, signatureId: string): {
  readonly ok: boolean;
  readonly from: string | null;
  readonly to: string | null;
  readonly message?: string;
} {
  const history =
    sql<{ compiled_id: string }>`
      select compiled_id from dse_active_artifact_history
      where signature_id = ${signatureId}
      order by updated_at desc
      limit 2
    ` || [];

  const current = history[0]?.compiled_id ?? null;
  const previous = history[1]?.compiled_id ?? null;

  if (!previous) {
    return {
      ok: false,
      from: current,
      to: null,
      message: "No previous artifact to roll back to."
    };
  }

  const ts = nowMs();
  sql`
    insert into dse_active_artifact_history (signature_id, compiled_id, updated_at)
    values (${signatureId}, ${previous}, ${ts})
  `;
  sql`
    insert into dse_active_artifacts (signature_id, compiled_id, updated_at)
    values (${signatureId}, ${previous}, ${ts})
    on conflict(signature_id) do update set compiled_id = excluded.compiled_id, updated_at = excluded.updated_at
  `;

  return { ok: true, from: current, to: previous };
}

export function listReceipts(sql: SqlTag, options: {
  readonly signatureId?: string;
  readonly limit?: number;
}): ReadonlyArray<unknown> {
  const limit = Math.max(1, Math.min(200, options.limit ?? 50));
  const rows =
    options.signatureId
      ? sql<{ json: string }>`
          select json from dse_receipts
          where signature_id = ${options.signatureId}
          order by created_at desc
          limit ${limit}
        `
      : sql<{ json: string }>`
          select json from dse_receipts
          order by created_at desc
          limit ${limit}
        `;

  const parsed: Array<unknown> = [];
  for (const row of rows ?? []) {
    try {
      parsed.push(JSON.parse(row.json));
    } catch {
      // Skip malformed rows; receipts are best-effort diagnostics.
    }
  }
  return parsed;
}
