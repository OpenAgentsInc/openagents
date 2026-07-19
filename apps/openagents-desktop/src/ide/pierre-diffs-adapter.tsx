import { parsePatchFiles, registerCustomTheme } from "@pierre/diffs";
import {
  CodeView,
  PatchDiff,
  type CodeViewHandle,
  type SelectedLineRange,
} from "@pierre/diffs/react";
import { Schema } from "effect";
import { useEffect, useMemo, useRef, type CSSProperties, type ReactNode } from "react";

import {
  tokyoNightDesktopThemeProjection,
} from "./tokyo-night-theme.ts";
import {
  khalaEditorDesktopThemeProjection,
  khalaEditorPierreCssVariables,
} from "./khala-editor-theme.ts";
import {
  ideReviewIntent,
  type IdeReviewIntent,
} from "./review-contract.ts";
import type { IdeReviewSource } from "./project-contract.ts";

const BoundedPatchSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(4 * 1024 * 1024),
);

export const PierreDiffAnnotationSchema = Schema.Struct({
  kind: Schema.Literals([
    "diagnostic",
    "conflict",
    "comment",
    "proposal_rationale",
    "stale",
    "unavailable",
  ]),
  side: Schema.Literals(["deletions", "additions"]),
  lineNumber: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240)),
}).annotate({ identifier: "PierreDiffAnnotation" });
export type PierreDiffAnnotation = typeof PierreDiffAnnotationSchema.Type;

export const PierreDiffSelectionSchema = Schema.Struct({
  start: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  side: Schema.Literals(["deletions", "additions"]),
  end: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  endSide: Schema.Literals(["deletions", "additions"]),
}).annotate({ identifier: "PierreDiffSelection" });
export type PierreDiffSelection = typeof PierreDiffSelectionSchema.Type;

/** Projection-only input: deliberately no root, grant, bridge, Git, or apply field. */
export const PierreDiffProjectionSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.pierre-diff-projection.v1"),
  reviewRef: Schema.String.check(Schema.isPattern(/^ide\.review\.[A-Za-z0-9][A-Za-z0-9._-]*$/u)),
  fileRef: Schema.String.check(Schema.isPattern(/^ide\.file\.[A-Za-z0-9][A-Za-z0-9._-]*$/u)),
  patch: BoundedPatchSchema,
  mode: Schema.Literals(["unified", "split"]),
  contextLines: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(1),
    Schema.isLessThanOrEqualTo(100),
  ),
  selection: Schema.NullOr(PierreDiffSelectionSchema),
  annotations: Schema.Array(PierreDiffAnnotationSchema).check(Schema.isMaxLength(500)),
}).annotate({ identifier: "PierreDiffProjection" });
export type PierreDiffProjection = typeof PierreDiffProjectionSchema.Type;

export const PierreDiffCollectionProjectionSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.pierre-diff-collection.v1"),
  reviewRef: Schema.String.check(Schema.isPattern(/^ide\.review\.[A-Za-z0-9][A-Za-z0-9._-]*$/u)),
  files: Schema.Array(
    Schema.Struct({
      fileRef: Schema.String.check(Schema.isPattern(/^ide\.file\.[A-Za-z0-9][A-Za-z0-9._-]*$/u)),
      patch: BoundedPatchSchema,
    }),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  mode: Schema.Literals(["unified", "split"]),
  contextLines: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(1),
    Schema.isLessThanOrEqualTo(100),
  ),
  viewportHeight: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(100),
    Schema.isLessThanOrEqualTo(2_000),
  ),
}).annotate({ identifier: "PierreDiffCollectionProjection" });
export type PierreDiffCollectionProjection = typeof PierreDiffCollectionProjectionSchema.Type;

export const PierreDiffScaleObservationSchema = Schema.Struct({
  totalItems: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  renderedItems: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  scrollHeight: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
}).annotate({ identifier: "PierreDiffScaleObservation" });
export type PierreDiffScaleObservation = typeof PierreDiffScaleObservationSchema.Type;

export const PierreReviewProjectionOptionsSchema = Schema.Struct({
  mode: Schema.Literals(["unified", "split"]),
  contextLines: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(1),
    Schema.isLessThanOrEqualTo(100),
  ),
  selection: Schema.NullOr(PierreDiffSelectionSchema),
  annotations: Schema.Array(PierreDiffAnnotationSchema).check(Schema.isMaxLength(500)),
}).annotate({ identifier: "PierreReviewProjectionOptions" });
export type PierreReviewProjectionOptions = typeof PierreReviewProjectionOptionsSchema.Type;

export const PierreReviewProjectionResultSchema = Schema.TaggedUnion({
  Ready: { projection: PierreDiffProjectionSchema },
  Refused: {
    reason: Schema.Literals([
      "source_stale",
      "source_unavailable",
      "content_unavailable",
    ]),
  },
}).annotate({ identifier: "PierreReviewProjectionResult" });
export type PierreReviewProjectionResult = typeof PierreReviewProjectionResultSchema.Type;

let ownedThemesRegistered = false;

const registerOwnedEditorThemes = (): void => {
  if (ownedThemesRegistered) return;
  ownedThemesRegistered = true;
  for (const projection of [
    khalaEditorDesktopThemeProjection,
    tokyoNightDesktopThemeProjection,
  ]) {
    registerCustomTheme(projection.pierre.themeName, async () => ({
      name: projection.pierre.themeName,
      type: "dark",
      colors: {
        "editor.background": projection.palette.background,
        "editor.foreground": projection.palette.foreground,
      },
      tokenColors: projection.monaco.rules.map((rule) => ({
        scope: [rule.token],
        settings: {
          foreground: rule.foreground,
          ...(rule.fontStyle === "" ? {} : { fontStyle: rule.fontStyle }),
        },
      })),
    }));
  }
};

export const decodePierreDiffProjection = Schema.decodeUnknownSync(PierreDiffProjectionSchema);
export const decodePierreDiffCollectionProjection = Schema.decodeUnknownSync(
  PierreDiffCollectionProjectionSchema,
);

/**
 * The sole review-domain -> Pierre projection. Root/grant/Git/document/policy
 * fields cannot cross because the output schema has nowhere to represent them.
 */
export const projectReviewSourceToPierre = (
  source: IdeReviewSource,
  rawOptions: PierreReviewProjectionOptions,
): PierreReviewProjectionResult => {
  const options = Schema.decodeUnknownSync(PierreReviewProjectionOptionsSchema)(rawOptions);
  if (source.lifecycle._tag === "Stale") {
    return PierreReviewProjectionResultSchema.cases.Refused.make({ reason: "source_stale" });
  }
  if (source.lifecycle._tag === "Unavailable") {
    return PierreReviewProjectionResultSchema.cases.Refused.make({ reason: "source_unavailable" });
  }
  if (source.patch === null) {
    return PierreReviewProjectionResultSchema.cases.Refused.make({ reason: "content_unavailable" });
  }
  const fileRef = source.fileRef ?? `ide.file.aggregate-${String(source.reviewRef).slice("ide.review.".length)}`;
  return PierreReviewProjectionResultSchema.cases.Ready.make({
    projection: PierreDiffProjectionSchema.make({
      schemaVersion: "openagents.desktop.pierre-diff-projection.v1",
      reviewRef: source.reviewRef,
      fileRef,
      patch: source.patch,
      mode: options.mode,
      contextLines: options.contextLines,
      selection: options.selection,
      annotations: options.annotations,
    }),
  });
};

export const PierreDiffAdapter = (
  props: Readonly<{
    projection: PierreDiffProjection;
    onSelectionChange?: (selection: SelectedLineRange | null) => void;
  }>,
): ReactNode => {
  registerOwnedEditorThemes();
  const projection = decodePierreDiffProjection(props.projection);
  const style = khalaEditorPierreCssVariables() as CSSProperties;
  return (
    <PatchDiff<PierreDiffAnnotation>
      patch={projection.patch}
      disableWorkerPool
      lineAnnotations={projection.annotations.map((annotation) => ({
        side: annotation.side,
        lineNumber: annotation.lineNumber,
        metadata: annotation,
      }))}
      selectedLines={projection.selection}
      renderAnnotation={(annotation) => (
        <span data-oa-pierre-annotation={annotation.metadata.kind}>
          <strong>{annotation.metadata.kind.replaceAll("_", " ")}:</strong> {annotation.metadata.label}
        </span>
      )}
      style={style}
      options={{
        theme: khalaEditorDesktopThemeProjection.pierre.themeName,
        themeType: "dark",
        diffStyle: projection.mode,
        collapsedContextThreshold: projection.contextLines,
        expansionLineCount: projection.contextLines,
        enableLineSelection: true,
        controlledSelection: true,
        onLineSelected: props.onSelectionChange,
        lineHoverHighlight: "both",
        diffIndicators: "bars",
        overflow: "scroll",
      }}
    />
  );
};

export const PierreReviewAdapter = (
  props: Readonly<{
    source: IdeReviewSource;
    options: PierreReviewProjectionOptions;
    onIntent?: (intent: IdeReviewIntent) => void;
  }>,
): ReactNode => {
  const result = projectReviewSourceToPierre(props.source, props.options);
  if (result._tag === "Refused") {
    const copy = result.reason === "source_stale"
      ? "This review is stale. Refresh the exact source before continuing."
      : result.reason === "source_unavailable"
        ? "This review source is unavailable."
        : "Review content was withheld by policy.";
    return <div role="alert" data-oa-pierre-refusal={result.reason}>{copy}</div>;
  }
  return <div
    data-oa-pierre-review={props.source._tag}
    data-review-ref={props.source.reviewRef}
    data-review-layout={props.options.mode}
  >
    <span className="oa-react-sr-only">
      {props.source.base.label} compared with {props.source.target.label}. Additions and deletions include non-color line markers.
    </span>
    <PierreDiffAdapter
      projection={result.projection}
      onSelectionChange={(selection) => {
        if (props.onIntent === undefined) return;
        props.onIntent(ideReviewIntent(props.source, "select", {
          layout: props.options.mode,
          contextLines: props.options.contextLines,
          selection: selection === null ? null : {
            startLine: selection.start,
            startSide: selection.side === "deletions" ? "base" : "target",
            endLine: selection.end,
            endSide: selection.endSide === "deletions" ? "base" : "target",
          },
        }));
      }}
    />
  </div>;
};

export const PierreDiffCollectionAdapter = (
  props: Readonly<{
    projection: PierreDiffCollectionProjection;
    onObservation?: (observation: PierreDiffScaleObservation) => void;
  }>,
): ReactNode => {
  registerOwnedEditorThemes();
  const projection = decodePierreDiffCollectionProjection(props.projection);
  const viewRef = useRef<CodeViewHandle<string>>(null);
  const items = useMemo(
    () =>
      projection.files.map((file) => {
        const parsed = parsePatchFiles(file.patch, file.fileRef, true)[0]?.files[0];
        if (parsed === undefined) throw new Error(`Pierre could not parse ${file.fileRef}`);
        return {
          id: file.fileRef,
          type: "diff" as const,
          fileDiff: parsed,
        };
      }),
    [projection.files],
  );
  useEffect(() => {
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        const instance = viewRef.current?.getInstance();
        props.onObservation?.({
          totalItems: items.length,
          renderedItems: instance?.getRenderedItems().length ?? 0,
          scrollHeight: instance?.getScrollHeight() ?? 0,
        });
      });
    });
    return () => cancelAnimationFrame(frame);
  }, [items.length, props.onObservation]);
  return (
    <CodeView<string>
      ref={viewRef}
      items={items}
      disableWorkerPool
      style={
        {
          ...khalaEditorPierreCssVariables(),
          height: `${projection.viewportHeight}px`,
          overflow: "auto",
        } as CSSProperties
      }
      options={{
        theme: khalaEditorDesktopThemeProjection.pierre.themeName,
        themeType: "dark",
        diffStyle: projection.mode,
        collapsedContextThreshold: projection.contextLines,
        expansionLineCount: projection.contextLines,
        diffIndicators: "bars",
        overflow: "scroll",
        stickyHeaders: true,
      }}
    />
  );
};
