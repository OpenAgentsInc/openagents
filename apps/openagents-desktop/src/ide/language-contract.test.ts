import { Exit, Schema } from "effect";
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import {
  IdeLanguageCapabilitySchema,
  IdeLanguageItemRefSchema,
  IdeLanguageItemSchema,
  IdeLanguageRequestSchema,
  IdeLanguageResultRefSchema,
  IdeLanguageResultSchema,
  IdeMonacoLocalLanguageStateSchema,
  IdeMonacoProjectLanguageProjectionSchema,
} from "./language-contract.ts";
import { makeIdeLanguageRequestFixture, makeIdeLanguageResultFixture } from "./language-fixture.ts";
import { IdeDiagnosticRefSchema, IdeServiceGenerationSchema } from "./project-contract.ts";
import { IdeLanguageBenchmarkReceiptSchema } from "./language-benchmark-contract.ts";

describe("IDE generation-safe language contract", () => {
  test("admits the complete first-corpus capability vocabulary", () => {
    const expected = [
      "diagnostics", "completion", "completion_resolve", "hover", "definition",
      "declaration", "type_definition", "references", "document_symbols",
      "workspace_symbols", "rename_preview", "format_document", "format_range",
      "code_actions", "semantic_tokens", "inlay_hints", "folding_ranges",
    ];
    expect(expected.every(value => Exit.isSuccess(Schema.decodeUnknownExit(IdeLanguageCapabilitySchema)(value)))).toBe(true);
    expect(expected.map(capability => Schema.decodeUnknownSync(IdeLanguageRequestSchema)(
      makeIdeLanguageRequestFixture(capability.replaceAll("_", "-"), capability as never),
    ).capability)).toEqual(expected);
  });

  test("binds every item and Monaco projection to exact document and service generations", () => {
    const request = makeIdeLanguageRequestFixture("projection");
    const resultRef = IdeLanguageResultRefSchema.make("ide.language-result.projection.diagnostics");
    const diagnostic = IdeLanguageItemSchema.cases.Diagnostic.make({
      itemRef: IdeLanguageItemRefSchema.make("ide.language-item.projection.1"),
      resultRef,
      pathRef: request.pathRef,
      range: request.range,
      diagnosticRef: IdeDiagnosticRefSchema.make("ide.diagnostic.projection.1"),
      severity: "error",
      source: "typescript",
      code: "2322",
      message: "Type 'string' is not assignable to type 'number'.",
    });
    const result = Schema.decodeUnknownSync(IdeLanguageResultSchema)(makeIdeLanguageResultFixture(request, [diagnostic]));
    expect(result).toMatchObject({
      documentRef: request.documentRef,
      documentGeneration: request.documentGeneration,
      documentVersion: request.documentVersion,
      serviceGeneration: 1,
      evidenceTier: "project_local",
    });
    const projection = IdeMonacoProjectLanguageProjectionSchema.make({
      documentRef: request.documentRef,
      documentGeneration: request.documentGeneration,
      documentVersion: request.documentVersion,
      serviceGeneration: result.serviceGeneration,
      evidenceTier: "project_local",
      resultRefs: [result.resultRef],
      diagnostics: [{
        diagnosticRef: diagnostic.diagnosticRef,
        severity: diagnostic.severity,
        message: diagnostic.message,
        source: diagnostic.source,
        range: diagnostic.range!,
      }],
      semanticTokens: [],
      inlayHints: [],
      foldingRanges: [],
    });
    expect(projection.diagnostics).toHaveLength(1);
  });

  test("keeps document-local worker evidence distinct and root-redacted", () => {
    const request = makeIdeLanguageRequestFixture("redacted");
    const local = IdeMonacoLocalLanguageStateSchema.cases.Ready.make({
      language: "typescript",
      workerGeneration: IdeServiceGenerationSchema.make(1),
      documentRef: request.documentRef,
      documentGeneration: request.documentGeneration,
      documentVersion: request.documentVersion,
      evidenceTier: "document_local",
      capabilities: ["syntax", "completion", "hover", "format", "folding"],
    });
    expect(local.evidenceTier).toBe("document_local");
    const serialized = JSON.stringify(makeIdeLanguageResultFixture(request));
    expect(serialized).not.toContain("/Users/");
    expect(serialized).not.toContain("/private/");
    expect(serialized).not.toContain("C:\\");
  });

  test("decodes the checked real-worker benchmark and complete capability corpus", () => {
    const receipt = Schema.decodeUnknownSync(IdeLanguageBenchmarkReceiptSchema)(JSON.parse(readFileSync(
      path.resolve(import.meta.dirname, "../../benchmarks/ide/2026-07-19-ide-06-language.json"),
      "utf8",
    )));
    expect(receipt.corpus.capabilitiesExercised).toEqual(IdeLanguageCapabilitySchema.literals);
    expect(receipt.cancellationFence).toEqual({ scheduled: 100, committed: 1, superseded: 99 });
    expect(receipt.restart.recoveredServiceGeneration).toBeGreaterThanOrEqual(2);
    expect(receipt.resources).toMatchObject({ activeWorkersAfter: 0, pendingRequestsAfter: 0 });
    expect(receipt.budgets.passed).toBe(true);
  });
});
