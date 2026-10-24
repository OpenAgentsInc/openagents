/// <reference types="@webgpu/types" />
// Batched web calls (ADR 0004): replays one pass's commands, written by Rust
// into Wasm memory (`crates/aethyr/gpu/src/batch.rs`), with a single import
// call instead of one call each. Opcodes and layouts match `batch::op`;
// `tests/gpu-batch.test.ts` replays the Rust fixture to keep them in step.

export const OP = {
  setPipeline: 1,
  setBindGroup: 2,
  setVertexBuffer: 3,
  setIndexBuffer: 4,
  draw: 5,
  drawIndexed: 6,
  drawIndirect: 7,
  drawIndexedIndirect: 8,
  dispatch: 9,
  dispatchIndirect: 10,
  endPass: 11,
} as const;

/// The pass methods a replay calls: a render or compute pass encoder.
export type PassLike = GPURenderPassEncoder | GPUComputePassEncoder;

const TWO_32 = 4294967296;
const ALL = 0xffffffff;

/// `low + high · 2^32`; `u64::MAX` (WGPU_WHOLE_SIZE) means "to the end".
const offset = (low: number, high: number) => low + high * TWO_32;
const size = (low: number, high: number) => (low === ALL && high === ALL ? undefined : low + high * TWO_32);

/**
 * Replays `words` on `pass`. `get` turns a handle into its object. Returns
 * whether the pass ended (the caller frees its handle). Throws on a word it
 * does not know, rather than guessing.
 */
export function replay(pass: PassLike, words: Uint32Array, get: (handle: number) => any): boolean {
  let ended = false;
  let i = 0;
  const n = words.length;
  const p = pass as any;
  while (i < n) {
    switch (words[i]) {
      case OP.setPipeline:
        p.setPipeline(get(words[i + 1]));
        i += 2;
        break;
      case OP.setBindGroup:
        p.setBindGroup(words[i + 1], get(words[i + 2]));
        i += 3;
        break;
      case OP.setVertexBuffer:
        p.setVertexBuffer(words[i + 1], get(words[i + 2]), offset(words[i + 3], words[i + 4]), size(words[i + 5], words[i + 6]));
        i += 7;
        break;
      case OP.setIndexBuffer:
        p.setIndexBuffer(get(words[i + 1]), words[i + 2] ? "uint32" : "uint16", offset(words[i + 3], words[i + 4]), size(words[i + 5], words[i + 6]));
        i += 7;
        break;
      case OP.draw:
        p.draw(words[i + 1], words[i + 2], words[i + 3], words[i + 4]);
        i += 5;
        break;
      case OP.drawIndexed:
        // The base vertex is a signed 32-bit value.
        p.drawIndexed(words[i + 1], words[i + 2], words[i + 3], words[i + 4] | 0, words[i + 5]);
        i += 6;
        break;
      case OP.drawIndirect:
        p.drawIndirect(get(words[i + 1]), offset(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case OP.drawIndexedIndirect:
        p.drawIndexedIndirect(get(words[i + 1]), offset(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case OP.dispatch:
        p.dispatchWorkgroups(words[i + 1], words[i + 2], words[i + 3]);
        i += 4;
        break;
      case OP.dispatchIndirect:
        p.dispatchWorkgroupsIndirect(get(words[i + 1]), offset(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case OP.endPass:
        pass.end();
        ended = true;
        i += 1;
        break;
      default:
        throw new Error(`unknown batched command ${words[i]} at word ${i}`);
    }
  }
  return ended;
}
