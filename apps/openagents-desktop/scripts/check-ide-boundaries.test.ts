import { describe, expect, test } from "vite-plus/test";

import { inspectIdeBoundaries, isHandMirroredBoundaryDeclaration } from "./check-ide-boundaries.ts";

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
});
