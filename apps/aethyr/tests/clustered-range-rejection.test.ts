import assert from "node:assert/strict";
import test from "node:test";
import { WGSL_CLUSTERED_LIGHTING } from "../packages/renderer/clustered-lighting.ts";

const words = new Uint32Array(1);
const floats = new Float32Array(words.buffer);
function fromBits(bits: number): number {
  words[0] = bits;
  return floats[0];
}
function bits(value: number): number {
  floats[0] = value;
  return words[0];
}

test("the early range guard cannot reject a contributing float32 light at its boundary", () => {
  const guard =
    /rangeSquared>=(\S+)&&dist2>rangeSquared\*(\S+)\{continue;\}/.exec(
      WGSL_CLUSTERED_LIGHTING,
    );
  assert.ok(guard);
  const floor = Math.fround(Number(guard[1]));
  const margin = Math.fround(Number(guard[2]));
  assert.ok(floor > 2 ** -126);
  assert.ok(margin > 1);
  let rejections = 0;
  // Cover every positive float32 exponent, changing mantissas, and both sides
  // of the squared boundary. The lower bound allows substantially more sqrt
  // error than WGSL's inverseSqrt-plus-division accuracy envelope.
  for (let exponent = 0; exponent < 255; exponent++) {
    for (const mantissa of [0, 1, 0x12345, 0x3fffff, 0x7ffffe, 0x7fffff]) {
      const range = fromBits((exponent << 23) | mantissa);
      if (!range) continue;
      const rangeSquared = Math.fround(range * range);
      if (!Number.isFinite(rangeSquared)) continue;
      const boundary = bits(rangeSquared);
      for (let offset = -192; offset <= 192; offset++) {
        if (boundary + offset < 0 || boundary + offset >= 0x7f800000) continue;
        const distanceSquared = fromBits(boundary + offset);
        const early =
          rangeSquared >= floor &&
          distanceSquared > Math.fround(rangeSquared * margin);
        if (!early) continue;
        rejections++;
        const minimumDistance =
          Math.sqrt(distanceSquared) * (1 - 16 * 2 ** -23);
        assert.ok(
          minimumDistance >= range,
          `False early rejection for radius bits ${bits(range).toString(16)} and squared distance ${distanceSquared}`,
        );
      }
    }
  }
  assert.ok(rejections > 50000);
});

test("contributing lights retain the original distance, falloff, and deferred record expressions", () => {
  const rangeTest =
    "let distance=sqrt(dist2);if distance>=position.w{continue;}";
  const testOffset = WGSL_CLUSTERED_LIGHTING.indexOf(rangeTest);
  assert.ok(testOffset > 0);
  for (const read of [
    "let emission=clusterVector(record+4u)",
    "let direction=clusterVector(record+8u)",
    "let cone=clusterVector(record+12u)",
  ])
    assert.ok(WGSL_CLUSTERED_LIGHTING.indexOf(read) > testOffset);
  assert.ok(
    WGSL_CLUSTERED_LIGHTING.includes(
      "let l=delta/max(distance,0.001);var attenuation=pow(max(1.0-pow(distance/position.w,4.0),0.0),2.0)/(dist2+0.0001);",
    ),
  );
  assert.ok(
    WGSL_CLUSTERED_LIGHTING.includes(
      "if cone.w>=0.0{attenuation=pow(max(1.0-dist2/(position.w*position.w),0.0),max(cone.w,0.0001));}",
    ),
  );
});
