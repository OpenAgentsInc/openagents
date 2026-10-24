import assert from "node:assert/strict";
import test from "node:test";
import { compactGeneratedWGSL } from "../packages/renderer/wgsl-source.ts";
import { MATERIAL } from "../packages/renderer/shaders.ts";

test("generated WGSL compaction retains tokens across nested comments and line boundaries", () => {
  assert.equal(
    compactGeneratedWGSL(
      "a/* outer // not a line comment /* nested */ */b\n // end\n let\t x = 1; // rest\n",
    ),
    "a b\nlet x = 1;",
  );
  assert.throws(() => compactGeneratedWGSL("let x=1; /*"));
  assert.equal(
    compactGeneratedWGSL("let x= a / b;let y=a * b;"),
    "let x= a / b;let y=a * b;",
  );
});

test("the full stock shader remains within the existing hot-edit and inspection bound", () => {
  assert(new TextEncoder().encode(MATERIAL).length <= 65536);
  assert(MATERIAL.includes("@fragment fn fs"));
  assert(MATERIAL.includes("AF_PROBE_GATHER"));
  assert(MATERIAL.includes("AF_MATERIAL_FEATURES"));
});
