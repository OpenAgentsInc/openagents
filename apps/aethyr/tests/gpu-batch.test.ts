// The batched web replay (ADR 0004) against a mock pass: the words Rust
// writes for one of every command (crates/aethyr/gpu/fixtures/batch.json,
// from af_gpu::batch::canonical) become the same WebGPU calls, in order.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { OP, replay } from "../apps/render-lab/gpu-batch.ts";

const fixture = JSON.parse(
  readFileSync(new URL("../../../crates/aethyr/gpu/fixtures/batch.json", import.meta.url), "utf8"),
) as { words: number[] };

function mockPass() {
  const calls: unknown[][] = [];
  const record = (name: string) => (...args: unknown[]) => void calls.push([name, ...args]);
  const pass = Object.fromEntries(
    [
      "setPipeline",
      "setBindGroup",
      "setVertexBuffer",
      "setIndexBuffer",
      "draw",
      "drawIndexed",
      "drawIndirect",
      "drawIndexedIndirect",
      "dispatchWorkgroups",
      "dispatchWorkgroupsIndirect",
      "end",
    ].map((name) => [name, record(name)]),
  );
  return { pass: pass as any, calls };
}

// The handle table: slot h holds "#h".
const handle = Array.from({ length: 64 }, (_, h) => `#${h}`);

test("the Rust fixture replays as the same calls, in order", () => {
  const { pass, calls } = mockPass();
  const ended = replay(pass, Uint32Array.from(fixture.words), handle);
  assert.equal(ended, true);
  assert.deepEqual(calls, [
    ["setPipeline", "#7"],
    ["setBindGroup", 1, "#42"],
    ["setVertexBuffer", 1, "#9", 5_000_000_000, 4096],
    ["setVertexBuffer", 0, "#3", 0],
    ["setIndexBuffer", "#4", "uint32", 256],
    ["setIndexBuffer", "#5", "uint16", 0, 64],
    ["draw", 3, 1, 0, 0],
    ["drawIndexed", 36, 120, 6, -4, 2],
    ["drawIndirect", "#11", 16],
    ["drawIndexedIndirect", "#12", 2 ** 40 + 20],
    ["dispatchWorkgroups", 64, 2, 1],
    ["dispatchWorkgroupsIndirect", "#13", 12],
    ["end"],
  ]);
});

test("every opcode is covered by the fixture", () => {
  const seen = new Set<number>();
  const words = fixture.words;
  // Walk by the same arities the replay uses.
  const arity: Record<number, number> = { 1: 1, 2: 2, 3: 6, 4: 6, 5: 4, 6: 5, 7: 3, 8: 3, 9: 3, 10: 3, 11: 0 };
  for (let i = 0; i < words.length; i += 1 + arity[words[i]]!) seen.add(words[i]!);
  assert.deepEqual([...seen].sort((a, b) => a - b), Object.values(OP).sort((a, b) => a - b));
});

test("a pass split across replays keeps going until it ends", () => {
  const { pass, calls } = mockPass();
  assert.equal(replay(pass, Uint32Array.from([OP.setPipeline, 2, OP.draw, 3, 1, 0, 0]), handle), false);
  assert.equal(replay(pass, Uint32Array.from([OP.endPass]), handle), true);
  assert.deepEqual(calls.map((c) => c[0]), ["setPipeline", "draw", "end"]);
});

test("an unknown word is refused, not guessed", () => {
  const { pass } = mockPass();
  assert.throws(() => replay(pass, Uint32Array.from([99]), handle), /unknown batched command 99/);
});
