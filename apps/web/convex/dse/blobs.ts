import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "../autopilot/access";

const nowMs = () => Date.now();

function byteLengthUtf8(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

function toHex(bytes: ArrayBuffer): string {
  const u8 = new Uint8Array(bytes);
  let out = "";
  for (let i = 0; i < u8.length; i++) {
    out += u8[i]!.toString(16).padStart(2, "0");
  }
  return out;
}

async function sha256IdFromString(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return `sha256:${toHex(digest)}`;
}

export const putTextImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly runId: string; readonly text: string; readonly mime?: string | undefined },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const text = String(args.text ?? "");
    const mime = typeof args.mime === "string" && args.mime.length > 0 ? args.mime : undefined;

    const blobId = yield* tryPromise(() => sha256IdFromString(text));
    const size = byteLengthUtf8(text);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseBlobs")
        .withIndex("by_threadId_runId_blobId", (q) =>
          q.eq("threadId", args.threadId).eq("runId", args.runId).eq("blobId", blobId),
        )
        .unique(),
    );

    if (!existing) {
      yield* tryPromise(() =>
        ctx.db.insert("dseBlobs", {
          threadId: args.threadId,
          runId: args.runId,
          blobId,
          ...(mime ? { mime } : {}),
          text,
          size,
          createdAtMs: nowMs(),
        }),
      );
    }

    return {
      ok: true as const,
      blob: {
        id: blobId,
        hash: blobId,
        size,
        ...(mime ? { mime } : {}),
      },
    };
  });

export const putText = effectMutation({
  args: {
    threadId: v.string(),
    runId: v.string(),
    text: v.string(),
    mime: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    blob: v.object({
      id: v.string(),
      hash: v.string(),
      size: v.number(),
      mime: v.optional(v.string()),
    }),
  }),
  handler: putTextImpl,
});

export const getTextImpl = (
  ctx: EffectQueryCtx,
  args: { readonly threadId: string; readonly runId: string; readonly blobId: string },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseBlobs")
        .withIndex("by_threadId_runId_blobId", (q) =>
          q.eq("threadId", args.threadId).eq("runId", args.runId).eq("blobId", args.blobId),
        )
        .unique(),
    );

    return { ok: true as const, text: row ? String((row as any).text ?? "") : null };
  });

export const getText = effectQuery({
  args: {
    threadId: v.string(),
    runId: v.string(),
    blobId: v.string(),
  },
  returns: v.object({ ok: v.boolean(), text: v.union(v.string(), v.null()) }),
  handler: getTextImpl,
});

