import { Context, Effect, Layer, Schema } from "effect";

import type { BlobRef } from "../blob.js";

import { canonicalJson } from "../internal/canonicalJson.js";

export class VarSpaceError extends Schema.TaggedError<VarSpaceError>()(
  "VarSpaceError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type VarValue =
  | { readonly _tag: "Json"; readonly value: unknown; readonly approxChars: number }
  | { readonly _tag: "Blob"; readonly blob: BlobRef };

export type VarMeta =
  | { readonly name: string; readonly kind: "json"; readonly approxChars: number }
  | { readonly name: string; readonly kind: "blob"; readonly blob: BlobRef };

export type VarSpace = {
  readonly get: (name: string) => Effect.Effect<VarValue | null, VarSpaceError>;
  readonly put: (name: string, value: VarValue) => Effect.Effect<void, VarSpaceError>;
  readonly putJson: (name: string, value: unknown) => Effect.Effect<void, VarSpaceError>;
  readonly putBlob: (name: string, blob: BlobRef) => Effect.Effect<void, VarSpaceError>;
  readonly del: (name: string) => Effect.Effect<void, VarSpaceError>;
  readonly list: () => Effect.Effect<ReadonlyArray<VarMeta>, VarSpaceError>;
};

export class VarSpaceService extends Context.Tag("@openagentsinc/dse/VarSpace")<
  VarSpaceService,
  VarSpace
>() {}

function validateVarName(name: string): string | null {
  const trimmed = name.trim();
  if (trimmed.length === 0) return "Var name must be non-empty";
  if (trimmed.length > 128) return "Var name too long (max 128 chars)";
  return null;
}

export function layerInMemory(options?: {
  readonly maxVars?: number | undefined;
  readonly maxJsonChars?: number | undefined;
}): Layer.Layer<VarSpaceService> {
  const maxVars = Math.max(1, Math.floor(options?.maxVars ?? 200));
  const maxJsonChars = Math.max(1, Math.floor(options?.maxJsonChars ?? 50_000));

  return Layer.sync(VarSpaceService, () => {
    const vars = new Map<string, VarValue>();

    const get: VarSpace["get"] = (name) =>
      Effect.sync(() => vars.get(name) ?? null);

    const put: VarSpace["put"] = (name, value) =>
      Effect.gen(function* () {
        const err = validateVarName(name);
        if (err) {
          return yield* Effect.fail(
            VarSpaceError.make({ message: `Invalid var name (${name}): ${err}` })
          );
        }
        if (!vars.has(name) && vars.size + 1 > maxVars) {
          return yield* Effect.fail(
            VarSpaceError.make({
              message: `VarSpace limit exceeded: maxVars=${maxVars}`
            })
          );
        }

        if (value._tag === "Json") {
          if (value.approxChars > maxJsonChars) {
            return yield* Effect.fail(
              VarSpaceError.make({
                message: `VarSpace JSON value too large: maxJsonChars=${maxJsonChars} observed=${value.approxChars} (name=${name})`
              })
            );
          }
        }

        vars.set(name, value);
      });

    const putJson: VarSpace["putJson"] = (name, value) =>
      Effect.gen(function* () {
        const rendered = yield* Effect.try({
          try: () => canonicalJson(value),
          catch: (cause) =>
            VarSpaceError.make({
              message: `Failed to canonicalize JSON for var (name=${name})`,
              cause
            })
        });

        const vv: VarValue = { _tag: "Json", value, approxChars: rendered.length };
        yield* put(name, vv);
      });

    const putBlob: VarSpace["putBlob"] = (name, blob) =>
      put(name, { _tag: "Blob", blob });

    const del: VarSpace["del"] = (name) =>
      Effect.sync(() => void vars.delete(name));

    const list: VarSpace["list"] = () =>
      Effect.sync(() => {
        const out: Array<VarMeta> = [];
        for (const [name, v] of vars.entries()) {
          if (v._tag === "Json") out.push({ name, kind: "json", approxChars: v.approxChars });
          else out.push({ name, kind: "blob", blob: v.blob });
        }
        out.sort((a, b) => a.name.localeCompare(b.name));
        return out;
      });

    return VarSpaceService.of({ get, put, putJson, putBlob, del, list });
  });
}

