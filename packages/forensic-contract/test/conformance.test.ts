import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  FORENSIC_CONTRACT_SCHEMA_IDS,
  FORENSIC_CONTRACT_SCHEMAS,
  type ForensicContractSchemaId,
} from "../src/index.ts";
import { strictDecode } from "../src/canonical.ts";

const PositiveFixtureFileSchema = S.Struct({
  schema: S.Literal("openagents.forensic_conformance_fixtures.v1"),
  contracts: S.Record(S.String, S.Unknown),
});

const NegativeFixtureFileSchema = S.Struct({
  schema: S.Literal("openagents.forensic_negative_conformance_fixtures.v1"),
  mutation: S.Struct({
    kind: S.Literal("replace_schema"),
    value: S.String,
  }),
  contracts: S.Array(S.String),
});

const readJson = (relativePath: string): unknown =>
  JSON.parse(readFileSync(new URL(relativePath, import.meta.url), "utf8"));

const positiveFixture = strictDecode(
  PositiveFixtureFileSchema,
  readJson("../../../fixtures/forensics/positive.v1.json"),
);
const negativeFixture = strictDecode(
  NegativeFixtureFileSchema,
  readJson("../../../fixtures/forensics/negative.v1.json"),
);

const schemaFor = (schemaId: ForensicContractSchemaId) => FORENSIC_CONTRACT_SCHEMAS[schemaId];

describe("forensic contract conformance fixtures", () => {
  it("has exactly one positive and one negative fixture for every registered schema", () => {
    expect(Object.keys(positiveFixture.contracts).toSorted()).toEqual(FORENSIC_CONTRACT_SCHEMA_IDS);
    expect(negativeFixture.contracts.toSorted()).toEqual(FORENSIC_CONTRACT_SCHEMA_IDS);
    expect(new Set(negativeFixture.contracts).size).toBe(FORENSIC_CONTRACT_SCHEMA_IDS.length);
  });

  for (const schemaId of FORENSIC_CONTRACT_SCHEMA_IDS) {
    it(`accepts the positive ${schemaId} fixture`, () => {
      const value = positiveFixture.contracts[schemaId];
      expect(value).toBeDefined();
      expect(() => strictDecode(schemaFor(schemaId), value)).not.toThrow();
    });

    it(`rejects the negative ${schemaId} fixture`, () => {
      const value = positiveFixture.contracts[schemaId];
      expect(value).toBeDefined();
      expect(typeof value).toBe("object");
      expect(value).not.toBeNull();
      expect(Array.isArray(value)).toBe(false);
      if (typeof value !== "object" || value === null || Array.isArray(value)) {
        throw new Error(`positive fixture for ${schemaId} must be an object`);
      }
      const invalid = Object.assign({}, value, { schema: negativeFixture.mutation.value });
      expect(() => strictDecode(schemaFor(schemaId), invalid)).toThrow();
    });
  }

  it("rejects unknown properties at the strict boundary", () => {
    const schemaId = "openagents.forensic_target_snapshot.v1";
    const value = positiveFixture.contracts[schemaId];
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new Error("target fixture must be an object");
    }
    expect(() =>
      strictDecode(schemaFor(schemaId), Object.assign({}, value, { token: "secret" })),
    ).toThrow(/token/);
  });
});
