import { Effect } from "effect";

import { BlobStore } from "@openagentsinc/dse";

export type DatasetBlobV1 = {
  readonly id: string;
  readonly text: string;
  readonly mime?: string | undefined;
};

const isRecord = (u: unknown): u is Record<string, unknown> => Boolean(u) && typeof u === "object";

export const collectDatasetBlobsFromMeta = (meta: unknown): ReadonlyArray<DatasetBlobV1> => {
  if (!isRecord(meta)) return [];
  const blobs = (meta as any).blobs;
  if (!Array.isArray(blobs)) return [];

  const out: DatasetBlobV1[] = [];
  for (const b of blobs) {
    if (!isRecord(b)) continue;
    const id = typeof (b as any).id === "string" ? String((b as any).id) : "";
    const text = typeof (b as any).text === "string" ? String((b as any).text) : "";
    const mime = typeof (b as any).mime === "string" ? String((b as any).mime) : undefined;
    if (!id || !text) continue;
    out.push({ id, text, ...(mime ? { mime } : {}) });
  }
  return out;
};

export const collectDatasetBlobsFromExamples = (examples: ReadonlyArray<unknown>): ReadonlyArray<DatasetBlobV1> => {
  const out: DatasetBlobV1[] = [];
  for (const ex of examples) {
    if (!isRecord(ex)) continue;
    const meta = (ex as any).meta;
    for (const b of collectDatasetBlobsFromMeta(meta)) out.push(b);
  }
  return out;
};

export const seedBlobStoreFromDatasetBlobs = (input: {
  readonly blobs: ReadonlyArray<DatasetBlobV1>;
  readonly maxBlobs?: number | undefined;
  readonly maxTotalChars?: number | undefined;
}) =>
  Effect.gen(function* () {
    const blobStore = yield* BlobStore.BlobStoreService;
    const maxBlobs = Math.max(0, Math.min(2_000, Math.floor(input.maxBlobs ?? 500)));
    const maxTotalChars = Math.max(0, Math.min(20_000_000, Math.floor(input.maxTotalChars ?? 2_000_000)));

    const seen = new Set<string>();
    let totalChars = 0;
    let seeded = 0;

    for (const b of input.blobs) {
      if (seeded >= maxBlobs) break;
      const id = String(b.id ?? "").trim();
      if (!id) continue;
      if (seen.has(id)) continue;
      seen.add(id);

      const text = String(b.text ?? "");
      if (!text) continue;

      totalChars += text.length;
      if (totalChars > maxTotalChars) {
        return yield* Effect.fail(new Error("dataset_blobs_too_large"));
      }

      const ref = yield* blobStore.putText({ text, ...(b.mime ? { mime: b.mime } : {}) });
      // Fail closed: ensure the declared blob id matches the hash of the text.
      if (ref.id !== id) {
        return yield* Effect.fail(new Error("dataset_blob_id_mismatch"));
      }
      seeded++;
    }

    return { seeded, totalChars };
  });

