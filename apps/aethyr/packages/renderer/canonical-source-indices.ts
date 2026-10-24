import {
  deriveStockDepthPolicy,
  STOCK_RASTER_MATERIAL_FLAGS,
} from "./stock-raster-policy.ts";
import { STOCK_MATERIAL_FEATURES } from "./material-specialization.ts";
import type { RasterOrientation } from "./raster-orientation.ts";

export interface CanonicalIndexDraw {
  firstIndex: number;
  indexCount: number;
  baseVertex: number;
  material: number;
  rasterOrientation?: RasterOrientation;
}

export const MAX_CANONICAL_SOURCE_INDICES = 4_194_304;

/** Copy mirrored stock opaque triangles when the bounded optional buffer fits. */
export function prepareCanonicalSourceIndices<T extends CanonicalIndexDraw>(
  indices: Uint32Array,
  variants: ReadonlyMap<string, readonly T[]>,
  features: (material: number) => number | undefined,
):
  | {
      indices: Uint32Array;
      variants: Map<string, T[]>;
      reversedRanges: number;
      reversedRecords: number;
      addedBytes: number;
    }
  | undefined {
  const ranges = new Map<
    string,
    { firstIndex: number; count: number; target: number }
  >();
  const selected = new Map<string, Set<number>>();
  let added = 0,
    oversized = false;
  for (const [mode, draws] of variants) {
    const records = new Set<number>();
    for (let i = 0; i < draws.length; i++) {
      const d = draws[i],
        flags = features(d.material);
      // Alpha-tested depth still reads model orientation in its fragment stage.
      if (
        d.rasterOrientation !== "negative" ||
        flags === undefined ||
        (flags &
          (STOCK_RASTER_MATERIAL_FLAGS.doubleSided |
            STOCK_MATERIAL_FEATURES.glass |
            STOCK_MATERIAL_FEATURES.flame)) !==
          0 ||
        deriveStockDepthPolicy(flags, "negative").fragmentRequired
      )
        continue;
      if (
        !Number.isSafeInteger(d.firstIndex) ||
        d.firstIndex < 0 ||
        !Number.isSafeInteger(d.indexCount) ||
        d.indexCount < 0 ||
        d.indexCount % 3 ||
        d.firstIndex + d.indexCount > indices.length ||
        !Number.isSafeInteger(d.baseVertex)
      )
        throw new Error(
          "Canonical winding needs complete bounded source triangles.",
        );
      const key = `${d.firstIndex}:${d.indexCount}`;
      if (!ranges.has(key)) {
        oversized ||= added + d.indexCount > MAX_CANONICAL_SOURCE_INDICES;
        ranges.set(key, {
          firstIndex: d.firstIndex,
          count: d.indexCount,
          target: indices.length + added,
        });
        added += d.indexCount;
      }
      records.add(i);
    }
    selected.set(mode, records);
  }
  // A valid large scene keeps its original draw path; no partial rewrite is published.
  if (oversized || !added) return undefined;
  const output = new Uint32Array(indices.length + added);
  output.set(indices);
  for (const range of ranges.values()) {
    for (let i = 0; i < range.count; i += 3) {
      const source = range.firstIndex + i,
        target = range.target + i;
      output[target] = indices[source];
      output[target + 1] = indices[source + 2];
      output[target + 2] = indices[source + 1];
    }
  }
  const result = new Map<string, T[]>();
  let reversedRecords = 0;
  for (const [mode, draws] of variants) {
    result.set(
      mode,
      draws.map((draw, index) => {
        if (!selected.get(mode)!.has(index)) return draw;
        reversedRecords++;
        return {
          ...draw,
          originalFirstIndex: draw.firstIndex,
          firstIndex: ranges.get(`${draw.firstIndex}:${draw.indexCount}`)!
            .target,
          rasterOrientation: "positive",
        };
      }),
    );
  }
  return {
    indices: output,
    variants: result,
    reversedRanges: ranges.size,
    reversedRecords,
    addedBytes: added * 4,
  };
}
