import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { DEFAULT_FILMIC_CONTROLS } from "../packages/renderer/filmic.ts";
import {
  loadSourceDisplay,
  validateSourceDisplay,
} from "../packages/renderer/source-display.ts";

const controls = { ...DEFAULT_FILMIC_CONTROLS, whiteClip: 1 };
const bytes = new Uint8Array(33 ** 3 * 8);
const view = new DataView(bytes.buffer);
for (let i = 0; i < 33 ** 3; i++) view.setUint16(i * 8 + 6, 0x3c00, true);
function recipe() {
  return {
    schema: "aethyrforge.source-display.v1",
    scenario: "day",
    controls,
    lut: {
      schema: "aethyrforge.display-lut.v1",
      id: "owned-test",
      edge: 33,
      inputSpace: "linear-rec709",
      outputSpace: "tone-linear-rec709",
      exposure: "external",
      shaper: { kind: "log2-1p", a: 2 ** -10, max: 64 },
      order: "r-fastest",
      format: "rgba16float-le",
      byteLength: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      url: "/private-assets/response.bin",
    },
  };
}

test("source display profiles require matching scenario, film and shadow controls", () => {
  const source = recipe();
  const admitted = validateSourceDisplay(source, "day", controls);
  assert.equal(admitted.lut.id, "owned-test");
  assert.notEqual(admitted.lut, source.lut);
  assert.notEqual(
    admitted.controls.shadowContrast,
    source.controls.shadowContrast,
  );
  assert.throws(
    () => validateSourceDisplay(source, "night", controls),
    /scenario/,
  );
  assert.throws(
    () =>
      validateSourceDisplay(source, "day", { ...controls, whiteClip: 0.04 }),
    /film settings/,
  );
  assert.throws(
    () =>
      validateSourceDisplay(source, "day", {
        ...controls,
        shadowContrast: [0.85, 0.85, 0.85],
      }),
    /shadow settings/,
  );
  assert.throws(
    () =>
      validateSourceDisplay(
        {
          ...source,
          lut: { ...source.lut, url: "https://example.com/response.bin" },
        },
        "day",
        controls,
      ),
    /private asset/,
  );
});

test("bounded source display loading verifies original response bytes before admission", async () => {
  const calls: string[] = [];
  const load = async (url: string) => {
    calls.push(url);
    return new Response(
      url.endsWith(".json") ? JSON.stringify(recipe()) : bytes,
    );
  };
  const admitted = await loadSourceDisplay(
    "/private-assets/display.json",
    "day",
    controls,
    load,
  );
  assert.deepEqual(admitted.bytes, bytes);
  assert.deepEqual(calls, [
    "/private-assets/display.json",
    "/private-assets/response.bin",
  ]);
  await assert.rejects(
    loadSourceDisplay(
      "https://example.com/display.json",
      "day",
      controls,
      load,
    ),
    /private asset/,
  );
  assert.equal(calls.length, 2);
  await assert.rejects(
    loadSourceDisplay(
      "/private-assets/display.json",
      "day",
      controls,
      async () => new Response("x".repeat(65537)),
    ),
    /size limit/,
  );
  await assert.rejects(
    loadSourceDisplay(
      "/private-assets/display.json",
      "day",
      controls,
      async (url) =>
        new Response(
          url.endsWith(".json")
            ? JSON.stringify(recipe())
            : new Uint8Array(bytes.length + 1),
        ),
    ),
    /size limit/,
  );
  await assert.rejects(
    loadSourceDisplay(
      "/private-assets/display.json",
      "day",
      controls,
      async (url) =>
        new Response(
          url.endsWith(".json")
            ? JSON.stringify(recipe())
            : new Uint8Array(bytes.length),
        ),
    ),
    /hash/,
  );
});
