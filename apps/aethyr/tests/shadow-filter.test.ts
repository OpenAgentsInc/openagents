import assert from "node:assert/strict";
import test from "node:test";
import { SHADOW_BOX_FILTER_WGSL } from "../packages/renderer/shadow-filter.ts";

// The reference explicitly samples nine bilinear lookups over a 4 by 4 depth mask.
const bilinear = (mask: number, x: number, y: number) => {
  const ix = Math.floor(x),
    iy = Math.floor(y),
    fx = x - ix,
    fy = y - iy;
  const texel = (a: number, b: number) => (mask >> ((b + 1) * 4 + a + 1)) & 1;
  return (
    (texel(ix, iy) * (1 - fx) + texel(ix + 1, iy) * fx) * (1 - fy) +
    (texel(ix, iy + 1) * (1 - fx) + texel(ix + 1, iy + 1) * fx) * fy
  );
};

test("four weighted comparisons retain the full original 3 by 3 bilinear shadow kernel", () => {
  for (const x of [0, 0.01, 0.25, 0.5, 0.75, 0.99])
    for (const y of [0, 0.13, 0.5, 0.87, 0.999])
      for (let mask = 0; mask < 65536; mask += 31) {
        let reference = 0;
        for (let j = -1; j <= 1; j++)
          for (let i = -1; i <= 1; i++)
            reference += bilinear(mask, x + i, y + j) / 9;
        const wx = 2 - x,
          wy = 2 - y,
          hx = 1 + x,
          hy = 1 + y;
        const ax = -1 + 1 / wx,
          ay = -1 + 1 / wy,
          bx = 1 + x / hx,
          by = 1 + y / hy;
        const actual =
          (bilinear(mask, ax, ay) * wx * wy +
            bilinear(mask, bx, ay) * hx * wy +
            bilinear(mask, ax, by) * wx * hy +
            bilinear(mask, bx, by) * hx * hy) /
          9;
        assert(Math.abs(actual - reference) < 1e-12);
      }
  // Keep the shipped WGSL coordinate centers and normalization tied to this proof.
  assert.match(
    SHADOW_BOX_FILTER_WGSL,
    /origin-vec2\(0\.5\)\+vec2\(1\.0\)\/low/,
  );
  assert.match(SHADOW_BOX_FILTER_WGSL, /origin\+vec2\(1\.5\)\+fraction\/high/);
  assert.match(SHADOW_BOX_FILTER_WGSL, /high\.x\*high\.y\)\/9\.0/);
});
