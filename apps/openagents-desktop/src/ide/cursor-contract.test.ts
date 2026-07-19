import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";
import {
  IdeCursorAnchorSchema,
  IdeCursorRequestSchema,
  IdeCursorSchemaVersion,
  decodeIdeCursorRequest,
} from "./cursor-contract.ts";
import { ideCursorFixtureAnchor } from "./cursor-fixture.ts";
import {
  IdeCursorBenchmarkReceiptSchema,
  ideCursorBenchmarkThresholds,
} from "./cursor-benchmark-contract.ts";

describe("IDE-09 cursor contract", () => {
  test("fixes the version and latency envelope before provider implementation", () => {
    expect(Schema.decodeUnknownExit(IdeCursorSchemaVersion)("openagents.ide-cursor.v1")._tag).toBe(
      "Success",
    );
    expect(ideCursorBenchmarkThresholds.completionFirstCandidateP95Ms).toBe(120);
    expect(Schema.decodeUnknownExit(IdeCursorBenchmarkReceiptSchema)({})._tag).toBe("Failure");
  });

  test("rejects unstructured requests instead of inferring authority", () => {
    expect(decodeIdeCursorRequest({ requestRef: "cursor-ish" })).toBeNull();
    expect(Schema.decodeUnknownExit(IdeCursorRequestSchema)({})._tag).toBe("Failure");
  });

  test("binds authority identity to a distinct Monaco document state", () => {
    const anchor = ideCursorFixtureAnchor();
    expect(anchor.sourceDocumentRef).not.toBe(anchor.documentRef);
    expect(anchor.sourceDocumentGeneration).toBe(0);
    expect(anchor.documentGeneration).toBe(1);
    expect(anchor.documentSequence).toBe(0);
    expect(anchor.modelVersion).toBe(1);
    expect(anchor.selectionVersion).toBe(0);
    const { modelVersion: _modelVersion, ...withoutModelVersion } = anchor;
    expect(Schema.decodeUnknownExit(IdeCursorAnchorSchema)(withoutModelVersion)._tag).toBe(
      "Failure",
    );
  });
});
