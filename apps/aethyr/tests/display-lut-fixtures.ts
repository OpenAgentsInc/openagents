import {
  DISPLAY_LUT_SHAPER_A,
  displayLutGridInput,
  type DisplayLutDescriptor,
} from "../packages/renderer/display-lut.ts";

const roundEven = (n: number) => {
  const floor = Math.floor(n),
    fraction = n - floor;
  return fraction > 0.5 || (fraction === 0.5 && floor % 2 !== 0)
    ? floor + 1
    : floor;
};

/** Owned IEEE binary16 encoder for synthetic fixtures, including ties to even. */
export function fixtureHalf(value: number): number {
  const sign = value < 0 || Object.is(value, -0) ? 32768 : 0;
  const magnitude = Math.abs(value);
  if (!Number.isFinite(value) || magnitude > 65504)
    throw new RangeError("Fixture value exceeds finite binary16");
  if (magnitude < 2 ** -14) return sign | roundEven(magnitude / 2 ** -24);
  let exponent = Math.floor(Math.log2(magnitude));
  let fraction = roundEven((magnitude / 2 ** exponent - 1) * 1024);
  if (fraction === 1024) {
    exponent++;
    fraction = 0;
  }
  return sign | ((exponent + 15) << 10) | fraction;
}

export function fixtureDescriptor(edge = 33): DisplayLutDescriptor {
  return {
    schema: "aethyrforge.display-lut.v1",
    id: "owned-synthetic-response",
    edge,
    inputSpace: "linear-rec709",
    outputSpace: "tone-linear-rec709",
    exposure: "external",
    shaper: { kind: "log2-1p", a: DISPLAY_LUT_SHAPER_A, max: 64 },
    order: "r-fastest",
    format: "rgba16float-le",
    byteLength: edge ** 3 * 8,
    sha256: "0".repeat(64),
    url: "owned-synthetic.bin",
  };
}

export function fixtureBytes(
  transform: (
    color: readonly [number, number, number],
    grid: readonly [number, number, number],
  ) => readonly number[],
  edge = 33,
  descriptor?: DisplayLutDescriptor,
): Uint8Array<ArrayBuffer> {
  const bytes = new Uint8Array(edge ** 3 * 8),
    view = new DataView(bytes.buffer);
  for (let b = 0; b < edge; b++)
    for (let g = 0; g < edge; g++)
      for (let r = 0; r < edge; r++) {
        const input: [number, number, number] = [
          displayLutGridInput(r, descriptor ?? edge),
          displayLutGridInput(g, descriptor ?? edge),
          displayLutGridInput(b, descriptor ?? edge),
        ];
        const rgb = transform(input, [
          r / (edge - 1),
          g / (edge - 1),
          b / (edge - 1),
        ]);
        const offset = (r + edge * (g + edge * b)) * 8;
        for (let channel = 0; channel < 4; channel++)
          view.setUint16(
            offset + channel * 2,
            fixtureHalf(channel === 3 ? 1 : rgb[channel]),
            true,
          );
      }
  return bytes;
}

export async function fixtureHash(
  bytes: Uint8Array<ArrayBuffer>,
): Promise<string> {
  return Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
}
