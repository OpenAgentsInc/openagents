import { Schema } from "effect";

import {
  IdeLanguageRequestRefSchema,
  IdeLanguageRejectionReasonSchema,
  IdeLanguageRequestResponseSchema,
  IdeLanguageResultSchema,
  IdeLanguageServiceSnapshotSchema,
  IdeMonacoProjectLanguageProjectionSchema,
  IdeMonacoLocalLanguageStateSchema,
  type IdeLanguageCapability,
  type IdeLanguageItem,
  type IdeLanguageRequest,
  type IdeLanguageRequestResponse,
  type IdeLanguageResult,
  type IdeMonacoProjectLanguageProjection,
} from "./language-contract.ts";
import {
  IdeDocumentGeneration,
  IdeDocumentRef,
  IdeMonacoModelVersion,
} from "./monaco-document-contract.ts";
import { IdeLanguageServiceRefSchema } from "./project-contract.ts";

export const IdeLanguageWorkbenchStateSchema = Schema.Struct({
  service: IdeLanguageServiceSnapshotSchema,
  local: Schema.NullOr(IdeMonacoLocalLanguageStateSchema),
  activeRequestRefs: Schema.Array(IdeLanguageRequestRefSchema).check(Schema.isMaxLength(32)),
  results: Schema.Array(IdeLanguageResultSchema).check(Schema.isMaxLength(64)),
  lastRejection: Schema.NullOr(Schema.Struct({
    requestRef: IdeLanguageRequestRefSchema,
    reason: IdeLanguageRejectionReasonSchema,
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
  })),
  nextRequestOrdinal: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
  ),
  selectedProblemRef: Schema.NullOr(Schema.String.check(Schema.isMaxLength(192))),
  selectedSymbolRef: Schema.NullOr(Schema.String.check(Schema.isMaxLength(192))),
  problemsFilter: Schema.Literals(["all", "errors", "warnings"]),
}).annotate({ identifier: "IdeLanguageWorkbenchState" });
export type IdeLanguageWorkbenchState = typeof IdeLanguageWorkbenchStateSchema.Type;

export const emptyIdeLanguageWorkbenchState = (): IdeLanguageWorkbenchState =>
  IdeLanguageWorkbenchStateSchema.make({
    service: IdeLanguageServiceSnapshotSchema.cases.Unconfigured.make({
      serviceRef: IdeLanguageServiceRefSchema.make("ide.language-service.typescript"),
    }),
    local: null,
    activeRequestRefs: [],
    results: [],
    lastRejection: null,
    nextRequestOrdinal: 0,
    selectedProblemRef: null,
    selectedSymbolRef: null,
    problemsFilter: "all",
  });

export const withLanguageRequestStarted = (
  state: IdeLanguageWorkbenchState,
  request: IdeLanguageRequest,
): IdeLanguageWorkbenchState => IdeLanguageWorkbenchStateSchema.make({
  ...state,
  activeRequestRefs: [
    ...state.activeRequestRefs.filter(ref => ref !== request.requestRef),
    request.requestRef,
  ].slice(-32),
  nextRequestOrdinal: state.nextRequestOrdinal + 1,
  lastRejection: null,
});

export const withLanguageResponse = (
  state: IdeLanguageWorkbenchState,
  response: IdeLanguageRequestResponse,
): IdeLanguageWorkbenchState => {
  const requestRef = response._tag === "Result"
    ? response.result.requestRef
    : response.requestRef;
  if (!state.activeRequestRefs.includes(requestRef)) return state;
  if (response._tag === "Rejected") {
    return IdeLanguageWorkbenchStateSchema.make({
      ...state,
      service: response.service,
      activeRequestRefs: state.activeRequestRefs.filter(ref => ref !== requestRef),
      lastRejection: {
        requestRef,
        reason: response.reason,
        message: response.message,
      },
    });
  }
  const result = response.result;
  const results = [
    ...state.results.filter(existing => !(
      existing.documentRef === result.documentRef &&
      existing.capability === result.capability
    )),
    result,
  ].slice(-64);
  return IdeLanguageWorkbenchStateSchema.make({
    ...state,
    service: response.service,
    activeRequestRefs: state.activeRequestRefs.filter(ref => ref !== requestRef),
    results,
    lastRejection: null,
  });
};

export const languageResultFor = (
  state: IdeLanguageWorkbenchState,
  input: Readonly<{
    capability: IdeLanguageCapability;
    documentRef: typeof IdeDocumentRef.Type;
    documentGeneration: typeof IdeDocumentGeneration.Type;
    documentVersion: typeof IdeMonacoModelVersion.Type;
  }>,
): IdeLanguageResult | null => state.results.findLast(result =>
  result.capability === input.capability &&
  result.documentRef === input.documentRef &&
  result.documentGeneration === input.documentGeneration &&
  result.documentVersion === input.documentVersion &&
  result.state._tag !== "Stale" &&
  result.state._tag !== "Cancelled"
) ?? null;

export const languageItemsFor = (
  state: IdeLanguageWorkbenchState,
  input: Parameters<typeof languageResultFor>[1],
): ReadonlyArray<IdeLanguageItem> => languageResultFor(state, input)?.items ?? [];

export const withLanguageProblemSelected = (
  state: IdeLanguageWorkbenchState,
  diagnosticRef: string | null,
): IdeLanguageWorkbenchState => ({ ...state, selectedProblemRef: diagnosticRef });

export const withLanguageSymbolSelected = (
  state: IdeLanguageWorkbenchState,
  symbolRef: string | null,
): IdeLanguageWorkbenchState => ({ ...state, selectedSymbolRef: symbolRef });

export const withLanguageProblemsFilter = (
  state: IdeLanguageWorkbenchState,
  filter: IdeLanguageWorkbenchState["problemsFilter"],
): IdeLanguageWorkbenchState => ({ ...state, problemsFilter: filter });

export const monacoProjectLanguageProjection = (
  state: IdeLanguageWorkbenchState,
  binding: Readonly<{
    documentRef: typeof IdeDocumentRef.Type;
    documentGeneration: typeof IdeDocumentGeneration.Type;
    documentVersion: typeof IdeMonacoModelVersion.Type;
  }>,
): IdeMonacoProjectLanguageProjection | null => {
  if (state.service._tag !== "Ready") return null;
  const service = state.service;
  const results = state.results.filter(result =>
    result.documentRef === binding.documentRef &&
    result.documentGeneration === binding.documentGeneration &&
    result.documentVersion === binding.documentVersion &&
    result.serviceGeneration === service.serviceGeneration &&
    result.state._tag !== "Stale" &&
    result.state._tag !== "Cancelled"
  );
  if (results.length === 0) return null;
  const items = results.flatMap(result => result.items);
  return IdeMonacoProjectLanguageProjectionSchema.make({
    ...binding,
    serviceGeneration: service.serviceGeneration,
    evidenceTier: "project_local",
    resultRefs: results.map(result => result.resultRef).slice(0, 16),
    diagnostics: items.flatMap(item => item._tag === "Diagnostic" && item.range !== null
      ? [{
          diagnosticRef: item.diagnosticRef,
          severity: item.severity,
          message: item.message,
          source: item.source,
          range: item.range,
        }]
      : []).slice(0, 2_000),
    semanticTokens: items.flatMap(item => item._tag === "SemanticToken" && item.range !== null
      ? [{ itemRef: item.itemRef, tokenType: item.tokenType, range: item.range }]
      : []).slice(0, 5_000),
    inlayHints: items.flatMap(item => item._tag === "InlayHint" && item.range !== null
      ? [{ itemRef: item.itemRef, label: item.label, hintKind: item.hintKind, range: item.range }]
      : []).slice(0, 2_000),
    foldingRanges: items.flatMap(item => item._tag === "FoldingRange" && item.range !== null
      ? [{ itemRef: item.itemRef, foldKind: item.foldKind, range: item.range }]
      : []).slice(0, 2_000),
  });
};
