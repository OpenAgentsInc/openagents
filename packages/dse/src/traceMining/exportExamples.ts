import { Effect, Schema } from "effect";

import { canonicalJson } from "../internal/canonicalJson.js";
import type { PredictReceiptV1 } from "../runtime/receipt.js";

import { RlmTraceDocV1Schema, type RlmTraceDocV1 } from "./rlmTrace.js";

export type DseExampleCandidateV1 = {
  readonly signatureId: string;
  readonly exampleId: string;
  readonly inputJson: unknown;
  readonly expectedJson: unknown;
  readonly tags?: ReadonlyArray<string> | undefined;
  readonly source?: string | undefined;
};

export class TraceExportError extends Schema.TaggedError<TraceExportError>()(
  "TraceExportError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

function parseJson(text: string): Effect.Effect<unknown, TraceExportError> {
  return Effect.try({
    try: () => JSON.parse(text),
    catch: (cause) =>
      TraceExportError.make({
        message: "Invalid JSON trace blob",
        cause
      })
  });
}

function findInput(doc: RlmTraceDocV1): unknown | null {
  for (const ev of doc.events) {
    if (!ev || typeof ev !== "object") continue;
    const tag = (ev as any)._tag;
    if (tag === "Input") return (ev as any).input ?? null;
  }
  return null;
}

function findFinalOutput(doc: RlmTraceDocV1): unknown | null {
  // Events are appended sequentially; take the last Final action we can find.
  for (let i = doc.events.length - 1; i >= 0; i--) {
    const ev = doc.events[i];
    if (!ev || typeof ev !== "object") continue;
    const action = (ev as any).action;
    if (!action || typeof action !== "object") continue;
    if (String((action as any)._tag ?? "") !== "Final") continue;
    if (!("output" in (action as any))) continue;
    return (action as any).output ?? null;
  }
  return null;
}

export function candidateExampleFromRlmTrace(options: {
  readonly receipt: PredictReceiptV1;
  readonly traceText: string;
}): Effect.Effect<DseExampleCandidateV1, TraceExportError> {
  return Effect.gen(function* () {
    const json = yield* parseJson(options.traceText);
    const doc = yield* Effect.try({
      try: () => Schema.decodeUnknownSync(RlmTraceDocV1Schema)(json),
      catch: (cause) =>
        TraceExportError.make({
          message: "Trace blob is not a valid openagents.dse.rlm_trace v1 document",
          cause
        })
    });

    const signatureId = options.receipt.signatureId;
    if (doc.signatureId && doc.signatureId !== signatureId) {
      return yield* Effect.fail(
        TraceExportError.make({
          message: `Trace signatureId mismatch (trace=${doc.signatureId} receipt=${signatureId})`
        })
      );
    }

    const inputJson = findInput(doc);
    if (inputJson == null) {
      return yield* Effect.fail(
        TraceExportError.make({
          message: "Trace is missing Input event (required for exporting candidate examples)"
        })
      );
    }

    const expectedJson = findFinalOutput(doc);
    if (expectedJson == null) {
      return yield* Effect.fail(
        TraceExportError.make({
          message: "Trace is missing Final output (required for exporting candidate examples)"
        })
      );
    }

    const exampleId = `trace:${options.receipt.receiptId}`;
    const tags = [
      "trace_export",
      ...(options.receipt.strategyId ? [`strategy:${options.receipt.strategyId}`] : [])
    ];

    const blobId = options.receipt.rlmTrace?.blob?.id;
    const source = canonicalJson({
      kind: "openagents.trace_export",
      receiptId: options.receipt.receiptId,
      signatureId,
      ...(blobId ? { blobId } : {})
    });

    return {
      signatureId,
      exampleId,
      inputJson,
      expectedJson,
      tags,
      source
    };
  });
}

