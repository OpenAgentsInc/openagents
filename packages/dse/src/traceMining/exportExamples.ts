import { Effect, Schema } from "effect";

import { canonicalJson } from "../internal/canonicalJson.js";
import type { PredictReceiptV1 } from "../runtime/receipt.js";

import {
  decodeRlmTraceDocV1CompatibleSync,
  type RlmTraceDocV1,
  type RlmTraceFinalEventV1,
  type RlmTraceInputEventV1
} from "./rlmTrace.js";

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
    if (isInputEvent(ev)) {
      return ev.input ?? null;
    }
  }
  return null;
}

function isInputEvent(
  event: RlmTraceDocV1["events"][number]
): event is RlmTraceInputEventV1 {
  return "_tag" in event && event._tag === "Input";
}

function isFinalEvent(
  event: RlmTraceDocV1["events"][number]
): event is RlmTraceFinalEventV1 {
  return "_tag" in event && event._tag === "Final";
}

function findFinalOutput(doc: RlmTraceDocV1): unknown | null {
  // Events are appended sequentially; take the last terminal output we can find.
  for (let i = doc.events.length - 1; i >= 0; i--) {
    const ev = doc.events[i];
    if (!ev) {
      continue;
    }
    if (isFinalEvent(ev)) {
      return ev.output ?? null;
    }
    if ("action" in ev && ev.action._tag === "Final") {
      return ev.action.output ?? null;
    }
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
      try: () => decodeRlmTraceDocV1CompatibleSync(json),
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
