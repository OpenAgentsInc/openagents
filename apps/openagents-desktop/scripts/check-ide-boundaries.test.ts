import { describe, expect, test } from "vite-plus/test";

import {
  cursorRendererAuthorityViolations,
  inspectIdeBoundaries,
  isHandMirroredBoundaryDeclaration,
} from "./check-ide-boundaries.ts";

describe("IDE architecture boundary", () => {
  test("keeps boundary types schema-derived and widgets/native code non-authoritative", () => {
    expect(inspectIdeBoundaries()).toEqual([]);
  });

  test("rejects hand-authored types/interfaces while admitting Schema-derived aliases", () => {
    expect(isHandMirroredBoundaryDeclaration("export type ProjectRef = string | null")).toBe(true);
    expect(isHandMirroredBoundaryDeclaration("export interface ProjectRef { value: string }")).toBe(
      true,
    );
    expect(
      isHandMirroredBoundaryDeclaration("export type ProjectRef = typeof ProjectRefSchema.Type"),
    ).toBe(false);
  });

  test("keeps cursor renderer projections away from host, provider, and Monaco mutation authority", () => {
    expect(cursorRendererAuthorityViolations(`
      import type { IdeCursorCandidate } from "../ide/cursor-contract.ts"
      export const label = (candidate: IdeCursorCandidate) => candidate._tag
    `)).toEqual([]);
    expect(cursorRendererAuthorityViolations(`
      import { writeFile } from "node:fs"
      import { IdeCursorProvider } from "../ide/cursor-provider.ts"
      editor.executeEdits("cursor", [])
    `)).toEqual([
      "node-host-import",
      "cursor-provider-access",
      "direct-monaco-mutation",
      "filesystem-mutation",
    ]);
  });
});
