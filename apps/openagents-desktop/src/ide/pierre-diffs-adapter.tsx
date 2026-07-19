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
  tokyoNightPierreCssVariables,
} from "./tokyo-night-theme.ts";

const BoundedPatchSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(4 * 1024 * 1024),
);

export const PierreDiffAnnotationSchema = Schema.Struct({
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

let tokyoNightRegistered = false;

const registerOwnedTokyoNightTheme = (): void => {
  if (tokyoNightRegistered) return;
  tokyoNightRegistered = true;
  const projection = tokyoNightDesktopThemeProjection;
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
};

export const decodePierreDiffProjection = Schema.decodeUnknownSync(PierreDiffProjectionSchema);
export const decodePierreDiffCollectionProjection = Schema.decodeUnknownSync(
  PierreDiffCollectionProjectionSchema,
);

export const PierreDiffAdapter = (
  props: Readonly<{
    projection: PierreDiffProjection;
    onSelectionChange?: (selection: SelectedLineRange | null) => void;
  }>,
): ReactNode => {
  registerOwnedTokyoNightTheme();
  const projection = decodePierreDiffProjection(props.projection);
  const style = tokyoNightPierreCssVariables() as CSSProperties;
  return (
    <PatchDiff<string>
      patch={projection.patch}
      disableWorkerPool
      lineAnnotations={projection.annotations.map((annotation) => ({
        side: annotation.side,
        lineNumber: annotation.lineNumber,
        metadata: annotation.label,
      }))}
      selectedLines={projection.selection}
      renderAnnotation={(annotation) => (
        <span data-oa-pierre-annotation="true">{annotation.metadata}</span>
      )}
      style={style}
      options={{
        theme: tokyoNightDesktopThemeProjection.pierre.themeName,
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

export const PierreDiffCollectionAdapter = (
  props: Readonly<{
    projection: PierreDiffCollectionProjection;
    onObservation?: (observation: PierreDiffScaleObservation) => void;
  }>,
): ReactNode => {
  registerOwnedTokyoNightTheme();
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
          ...tokyoNightPierreCssVariables(),
          height: `${projection.viewportHeight}px`,
          overflow: "auto",
        } as CSSProperties
      }
      options={{
        theme: tokyoNightDesktopThemeProjection.pierre.themeName,
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
