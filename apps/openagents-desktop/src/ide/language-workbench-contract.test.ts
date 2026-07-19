import { describe, expect, test } from "vite-plus/test";

import {
  IdeLanguageItemRefSchema,
  IdeLanguageItemSchema,
  IdeLanguageRequestResponseSchema,
  IdeLanguageResultRefSchema,
  IdeLanguageServiceSnapshotSchema,
  IdeLanguageStartRefSchema,
} from "./language-contract.ts";
import { makeIdeLanguageRequestFixture, makeIdeLanguageResultFixture } from "./language-fixture.ts";
import {
  emptyIdeLanguageWorkbenchState,
  languageItemsFor,
  monacoProjectLanguageProjection,
  withLanguageRequestStarted,
  withLanguageResponse,
} from "./language-workbench-contract.ts";
import { IdeDiagnosticRefSchema, IdeLanguageServiceRefSchema, IdePlacementRefSchema, IdeServiceGenerationSchema } from "./project-contract.ts";
import { IdeMonacoModelVersion } from "./monaco-document-contract.ts";

const readyService = IdeLanguageServiceSnapshotSchema.cases.Ready.make({
  serviceRef: IdeLanguageServiceRefSchema.make("ide.language-service.typescript"),
  serviceGeneration: IdeServiceGenerationSchema.make(1),
  startRef: IdeLanguageStartRefSchema.make("ide.language-start.1.0"),
  placementRef: IdePlacementRefSchema.make("ide.placement.project-local"),
  evidenceTier: "project_local",
  executable: "typescript/lib/tsserverlibrary",
  providerVersion: "6.0.3",
  capabilities: [],
  startedAt: "2026-07-19T00:00:00.000Z",
  activeRequests: 0,
  queuedRequests: 0,
  restartCount: 0,
});

describe("IDE language workbench projection", () => {
  test("projects one receipt into Problems and Monaco without losing evidence refs", () => {
    const request = makeIdeLanguageRequestFixture("shared");
    const diagnostic = IdeLanguageItemSchema.cases.Diagnostic.make({
      itemRef: IdeLanguageItemRefSchema.make("ide.language-item.shared.1"),
      resultRef: IdeLanguageResultRefSchema.make("ide.language-result.shared.diagnostics"),
      pathRef: request.pathRef,
      range: request.range,
      diagnosticRef: IdeDiagnosticRefSchema.make("ide.diagnostic.shared.1"),
      severity: "warning",
      source: "typescript",
      code: "6133",
      message: "Value is declared but never read.",
    });
    const result = makeIdeLanguageResultFixture(request, [diagnostic]);
    const started = withLanguageRequestStarted(emptyIdeLanguageWorkbenchState(), request);
    const state = withLanguageResponse(started, IdeLanguageRequestResponseSchema.cases.Result.make({ result, service: readyService }));
    const binding = {
      capability: "diagnostics" as const,
      documentRef: request.documentRef,
      documentGeneration: request.documentGeneration,
      documentVersion: request.documentVersion,
    };
    expect(languageItemsFor(state, binding)[0]?.itemRef).toBe(diagnostic.itemRef);
    const projection = monacoProjectLanguageProjection(state, binding);
    expect(projection?.resultRefs).toContain(result.resultRef);
    expect(projection?.diagnostics[0]?.diagnosticRef).toBe(diagnostic.diagnosticRef);
  });

  test("never projects a result onto a newer Monaco model version", () => {
    const request = makeIdeLanguageRequestFixture("stale-model");
    const result = makeIdeLanguageResultFixture(request);
    const state = withLanguageResponse(
      withLanguageRequestStarted(emptyIdeLanguageWorkbenchState(), request),
      IdeLanguageRequestResponseSchema.cases.Result.make({ result, service: readyService }),
    );
    expect(monacoProjectLanguageProjection(state, {
      documentRef: request.documentRef,
      documentGeneration: request.documentGeneration,
      documentVersion: IdeMonacoModelVersion.make(request.documentVersion + 1),
    })).toBeNull();
  });

  test("ignores responses that are no longer active", () => {
    const request = makeIdeLanguageRequestFixture("late");
    const state = emptyIdeLanguageWorkbenchState();
    expect(withLanguageResponse(state, IdeLanguageRequestResponseSchema.cases.Result.make({
      result: makeIdeLanguageResultFixture(request),
      service: readyService,
    }))).toEqual(state);
  });
});
