import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Effect, Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ALL_WORK_CONTRACT_SCHEMAS,
  ALL_WORK_CONTRACT_DEFINITION_SHA256,
  type AllWorkContractSchemaName,
  encodeAllWorkCanonicalJson,
  negotiateAllWorkProtocol,
  validateWorkReadRequestFrame,
  validateWorkSnapshotSemantics,
  validateWorkSummarySuccessor,
  WorkIndexReadRequestSchema,
  WorkReadRequestFrameSchema,
  WorkSnapshotSchema,
  WorkSummarySchema,
} from "../src/index.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const readJson = (path: string): unknown =>
  JSON.parse(readFileSync(resolve(packageRoot, path), "utf8"));

const FixtureIndexSchema = S.Struct({
  contractRef: S.Literal("openagents.all_work_boundary.v1"),
  definitionSha256: S.String,
  fixtures: S.Array(
    S.Struct({
      file: S.String,
      schema: S.Literals([
        "WorkSummary",
        "WorkSnapshot",
        "IssueProjection",
        "ProtocolError",
        "ProtocolInitializeRequest",
        "ProtocolInitializeResult",
        "WorkIndexSubscriptionRequest",
        "WorkIndexSubscriptionEvent",
        "WorkIndexReadRequest",
        "WorkIndexReadResult",
        "WorkSnapshotReadRequest",
        "WorkSnapshotReadResult",
        "WorkReadRequestFrame",
        "WorkReadResponseFrame",
      ]),
      valid: S.Boolean,
      canonical: S.Boolean,
    }),
  ),
});
const decodeFixtureIndex = S.decodeUnknownSync(FixtureIndexSchema);
const fixtureIndex = decodeFixtureIndex(readJson("generated/fixture-index.json"), {
  onExcessProperty: "error",
});

const decodeFixture = (schemaName: AllWorkContractSchemaName, input: unknown): unknown =>
  S.decodeUnknownSync(ALL_WORK_CONTRACT_SCHEMAS[schemaName])(input, {
    onExcessProperty: "error",
  });
const strictDecode = { onExcessProperty: "error" } as const;
const parseWorkIndexReadRequest = (input: unknown) =>
  S.decodeUnknownSync(WorkIndexReadRequestSchema)(input, strictDecode);
const parseWorkReadRequestFrame = (input: unknown) =>
  S.decodeUnknownSync(WorkReadRequestFrameSchema)(input, strictDecode);
const parseWorkSnapshot = (input: unknown) =>
  S.decodeUnknownSync(WorkSnapshotSchema)(input, strictDecode);
const parseWorkSummary = (input: unknown) =>
  S.decodeUnknownSync(WorkSummarySchema)(input, strictDecode);

describe("OpenAgents All Work generated boundary", () => {
  it("accepts every positive fixture and rejects every negative fixture", () => {
    expect(fixtureIndex.definitionSha256).toBe(ALL_WORK_CONTRACT_DEFINITION_SHA256);
    for (const fixture of fixtureIndex.fixtures) {
      const input = readJson(`fixtures/${fixture.file}`);
      const decode = () => decodeFixture(fixture.schema, input);
      if (fixture.valid) expect(decode).not.toThrow();
      else expect(decode).toThrow();
    }
  });

  it("preserves absent, required-nullable, and optional-nullable distinctions", () => {
    const absent = parseWorkIndexReadRequest({});
    const explicitNull = parseWorkIndexReadRequest({ cursor: null });
    expect("cursor" in absent).toBe(false);
    expect(explicitNull.cursor).toBeNull();

    const summary = readJson("fixtures/valid/work-summary.json");
    expect(parseWorkSummary(summary).agentDelegate).toBeNull();
    expect(() =>
      parseWorkSummary({
        ...parseWorkSummary(summary),
        assignee: undefined,
      }),
    ).toThrow();
  });

  it("keeps canonical fixture bytes deterministic", () => {
    const source = readJson("fixtures/valid/work-summary.json");
    const canonical = readFileSync(
      resolve(packageRoot, "generated/canonical/work-summary.canonical.json"),
      "utf8",
    );
    expect(canonical).toBe(encodeAllWorkCanonicalJson(source));
    expect(() => encodeAllWorkCanonicalJson({ unsafe: Number.MAX_SAFE_INTEGER + 1 })).toThrow();
  });

  it("negotiates v1 only as an explicit rollback and gates Work reads on v2", () => {
    const v1 = Effect.runSync(
      negotiateAllWorkProtocol({
        supportedVersions: ["omega-effectd.v1"],
        requestedCapabilities: [],
      }),
    );
    expect(v1).toMatchObject({ selectedVersion: "omega-effectd.v1", capabilities: [] });

    const v2 = Effect.runSync(
      negotiateAllWorkProtocol({
        supportedVersions: ["omega-effectd.v1", "omega-effectd.v2"],
        requestedCapabilities: ["work.index.read", "work.snapshot.read"],
      }),
    );
    expect(v2).toMatchObject({
      selectedVersion: "omega-effectd.v2",
      capabilities: ["work.index.read", "work.snapshot.read"],
    });

    const invalidV1Read = parseWorkReadRequestFrame({
      method: "work.index.read",
      id: "request-v1-invalid",
      version: "omega-effectd.v1",
      params: {},
    });
    expect(Effect.runSyncExit(validateWorkReadRequestFrame(invalidV1Read))._tag).toBe("Failure");
  });

  it("enforces same-identity Issue projection semantics outside generated structure", () => {
    const snapshot = parseWorkSnapshot(readJson("fixtures/valid/work-snapshot.json"));
    expect(Effect.runSyncExit(validateWorkSnapshotSemantics(snapshot))._tag).toBe("Success");

    const mismatched = parseWorkSnapshot({
      ...snapshot,
      issue:
        snapshot.issue === null || snapshot.issue === undefined
          ? snapshot.issue
          : { ...snapshot.issue, workRef: "work:other:1" },
    });
    expect(Effect.runSyncExit(validateWorkSnapshotSemantics(mismatched))._tag).toBe("Failure");
  });

  it("enforces monotonic revisions and cursor coherence per source authority", () => {
    const previous = parseWorkSummary(readJson("fixtures/valid/work-summary.json"));
    const advanced = parseWorkSummary({
      ...previous,
      revision: previous.revision + 1,
      completeness: { ...previous.completeness, cursor: "cursor:forensics:8" },
    });
    expect(Effect.runSyncExit(validateWorkSummarySuccessor(previous, advanced))._tag).toBe(
      "Success",
    );

    const regressed = parseWorkSummary({ ...previous, revision: previous.revision - 1 });
    expect(Effect.runSyncExit(validateWorkSummarySuccessor(previous, regressed))._tag).toBe(
      "Failure",
    );

    const cursorOnly = parseWorkSummary({
      ...previous,
      completeness: { ...previous.completeness, cursor: "cursor:forensics:8" },
    });
    expect(Effect.runSyncExit(validateWorkSummarySuccessor(previous, cursorOnly))._tag).toBe(
      "Failure",
    );
  });
});
