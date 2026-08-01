import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";
import { Schema as S } from "effect";

import { Sha256Digest } from "./primitives.ts";

export class ForensicCanonicalizationError extends Error {
  readonly path: ReadonlyArray<string | number>;

  constructor(message: string, path: ReadonlyArray<string | number>) {
    super(`${message} at ${path.length === 0 ? "$" : path.join(".")}`);
    this.name = "ForensicCanonicalizationError";
    this.path = path;
  }
}

export const forensicCanonicalJson = (value: unknown): string => {
  const encode = (current: unknown, path: ReadonlyArray<string | number>): string => {
    if (current === null) return "null";

    switch (typeof current) {
      case "boolean":
        return current ? "true" : "false";
      case "number":
        if (!Number.isFinite(current)) {
          throw new ForensicCanonicalizationError("non-finite number", path);
        }
        return JSON.stringify(current);
      case "string":
        return JSON.stringify(current);
      case "object": {
        if (Array.isArray(current)) {
          const values = current.map((item, index) => {
            if (item === undefined) {
              throw new ForensicCanonicalizationError("undefined array element", [...path, index]);
            }
            return encode(item, [...path, index]);
          });
          return `[${values.join(",")}]`;
        }

        const prototype = Object.getPrototypeOf(current);
        if (prototype !== Object.prototype && prototype !== null) {
          throw new ForensicCanonicalizationError("non-plain object", path);
        }

        const members: Array<string> = [];
        for (const key of Object.keys(current).sort()) {
          const member = Reflect.get(current, key);
          if (member === undefined) continue;
          members.push(`${JSON.stringify(key)}:${encode(member, [...path, key])}`);
        }
        return `{${members.join(",")}}`;
      }
      default:
        throw new ForensicCanonicalizationError(`unsupported ${typeof current}`, path);
    }
  };

  return encode(value, []);
};

export const forensicSha256Digest = (value: unknown): Sha256Digest =>
  Sha256Digest.make(`sha256:${bytesToHex(sha256(utf8ToBytes(forensicCanonicalJson(value))))}`);

export const strictDecode = <Schema extends S.ConstraintDecoder<unknown, never>>(
  schema: Schema,
  input: unknown,
): Schema["Type"] => S.decodeUnknownSync(schema)(input, { onExcessProperty: "error" });

export const canonicalizeForensicContract = <Schema extends S.ConstraintDecoder<unknown, never>>(
  schema: Schema,
  input: unknown,
): Readonly<{ value: Schema["Type"]; canonicalJson: string; digest: Sha256Digest }> => {
  const value = strictDecode(schema, input);
  const canonicalJson = forensicCanonicalJson(value);
  return {
    value,
    canonicalJson,
    digest: forensicSha256Digest(value),
  };
};
