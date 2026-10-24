import type { V3 } from "./math.ts";

// XYZ-to-RGB matrix derived from BT.709 primaries and the D65 white point:
// https://www.itu.int/rec/R-REC-BT.709-6-201506-I/en
const XYZ_TO_REC709: readonly V3[] = [
  [3.240969941904522, -1.5373831775700937, -0.49861076029300344],
  [-0.9692436362808796, 1.8759675015077202, 0.041555057407175626],
  [0.05563007969699365, -0.20397695888897655, 1.0569715142428786],
];

function quadratic(t: number, coefficients: V3): number {
  return coefficients[0] + t * (coefficients[1] + t * coefficients[2]);
}

/**
 * Converts a Planckian-locus temperature to linear Rec.709 light RGB.
 *
 * Uses Krystek's public CIE 1960 UCS approximation, with XYZ luminance Y=1:
 * https://doi.org/10.1002/col.5080100109
 * https://colour.readthedocs.io/en/v0.4.1/generated/colour.temperature.CCT_to_uv_Krystek1985.html
 * Finite inputs clamp to its 1000–15000 K domain. Out-of-gamut negative RGB
 * channels clamp to zero; positive values retain their energy and headroom.
 */
export function colorTemperatureRec709(kelvin: number): V3 {
  if (!Number.isFinite(kelvin))
    throw new Error("Use a finite color temperature.");

  // Express the published rational coefficients in thousands of kelvin.
  const t = Math.min(15000, Math.max(1000, kelvin)) / 1000;
  const u =
    quadratic(t, [0.860117757, 0.154118254, 0.128641212]) /
    quadratic(t, [1, 0.842420235, 0.708145163]);
  const v =
    quadratic(t, [0.317398726, 0.0422806245, 0.0420481691]) /
    quadratic(t, [1, -0.0289741816, 0.161456053]);

  const denominator = 2 * u - 8 * v + 4;
  const x = (3 * u) / denominator;
  const y = (2 * v) / denominator;
  const xyz: V3 = [x / y, 1, (1 - x - y) / y];
  return XYZ_TO_REC709.map((row) =>
    Math.max(0, row[0] * xyz[0] + row[1] * xyz[1] + row[2] * xyz[2]),
  ) as V3;
}
