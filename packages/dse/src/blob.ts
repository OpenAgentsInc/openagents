import { Schema } from "effect";

export type BlobId = string;

export type BlobRef = {
  readonly id: BlobId;
  readonly hash: string;
  readonly size: number;
  readonly mime?: string | undefined;
};

export const BlobRefSchema: Schema.Schema<BlobRef> = Schema.Struct({
  id: Schema.String,
  hash: Schema.String,
  size: Schema.Number,
  mime: Schema.optional(Schema.String)
});

export function isBlobRef(value: unknown): value is BlobRef {
  if (!value || typeof value !== "object") return false;
  const v = value as any;
  return (
    typeof v.id === "string" &&
    typeof v.hash === "string" &&
    typeof v.size === "number" &&
    (v.mime === undefined || typeof v.mime === "string")
  );
}
