import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

type PayloadVector = {
  name: string;
  payload: JsonValue;
  canonical_json: string;
  sha256: string;
};

type PayloadFixture = {
  version: string;
  algorithm: string;
  vectors: PayloadVector[];
};

const fixturePath = resolve(
  process.cwd(),
  "..",
  "..",
  "docs",
  "protocol",
  "testdata",
  "khala_payload_hash_vectors.v1.json",
);

const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as PayloadFixture;

const canonicalJson = (value: JsonValue): string => {
  if (value === null) {
    return "null";
  }

  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry)).join(",")}]`;
  }

  if (typeof value === "object") {
    const entries = Object.entries(value).sort(([left], [right]) =>
      left < right ? -1 : left > right ? 1 : 0,
    );
    const encodedEntries = entries.map(
      ([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child as JsonValue)}`,
    );

    return `{${encodedEntries.join(",")}}`;
  }

  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error(`non-finite number is not supported in canonical JSON: ${value}`);
    }

    if (Object.is(value, -0)) {
      return "0";
    }

    return JSON.stringify(value);
  }

  return JSON.stringify(value);
};

const sha256CanonicalJson = (value: JsonValue): string => {
  const canonical = canonicalJson(value);
  const hash = createHash("sha256").update(canonical).digest("hex");
  return `sha256:${hash}`;
};

describe("khala payload hash vectors", () => {
  it("matches canonical json and hash fixtures", () => {
    expect(fixture.version).toBe("khala.payload_hash.v1");
    expect(fixture.algorithm).toBe("sha256");

    for (const vector of fixture.vectors) {
      expect(canonicalJson(vector.payload)).toBe(vector.canonical_json);
      expect(sha256CanonicalJson(vector.payload)).toBe(vector.sha256);
    }
  });
});
