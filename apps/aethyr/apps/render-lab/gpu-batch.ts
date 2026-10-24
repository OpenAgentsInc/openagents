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

/** `lo + hi · 2^32`, without a double when `hi` is zero. */
const join = (lo: number, hi: number) => (hi === 0 ? lo : lo + hi * TWO_32);

/**
 * Replays `words` on `pass`, with handles indexing `table`. Returns whether
 * the pass ended (the caller frees its handle). Throws on a word it does not
 * know, rather than guessing.
 *
 * This loop runs once per command on every frame, so it stays flat: the
 * cases are literal opcodes (a case on `OP.x` reads a property each time),
 * the commands a draw loop makes come first, and offsets and sizes are
 * computed inline. A size of `u64::MAX` (WGPU_WHOLE_SIZE) is left out.
 * `lo + hi * 2^32` is always a double, which V8 allocates; a value whose high
 * word is zero passes the low word as it is, as TypeScript callers do.
 */
export function replay(pass: PassLike, words: Uint32Array, table: ArrayLike<any>): boolean {
  const p = pass as any;
  const n = words.length;
  let i = 0;
  while (i < n) {
    switch (words[i]) {
      case 3: {
        // setVertexBuffer: slot, buffer, offset (lo, hi), size (lo, hi)
        const lo = words[i + 5];
        const hi = words[i + 6];
        const offset = words[i + 4] === 0 ? words[i + 3] : words[i + 3] + words[i + 4] * TWO_32;
        if (hi === 0) p.setVertexBuffer(words[i + 1], table[words[i + 2]], offset, lo);
        else if (lo === ALL && hi === ALL) p.setVertexBuffer(words[i + 1], table[words[i + 2]], offset);
        else p.setVertexBuffer(words[i + 1], table[words[i + 2]], offset, lo + hi * TWO_32);
        i += 7;
        break;
      }
      case 6:
        // drawIndexed: indices, instances, first index, base vertex (signed), first instance
        p.drawIndexed(words[i + 1], words[i + 2], words[i + 3], words[i + 4] | 0, words[i + 5]);
        i += 6;
        break;
      case 2:
        p.setBindGroup(words[i + 1], table[words[i + 2]]);
        i += 3;
        break;
      case 5:
        p.draw(words[i + 1], words[i + 2], words[i + 3], words[i + 4]);
        i += 5;
        break;
      case 1:
        p.setPipeline(table[words[i + 1]]);
        i += 2;
        break;
      case 4: {
        const lo = words[i + 5];
        const hi = words[i + 6];
        const format = words[i + 2] ? "uint32" : "uint16";
        const offset = words[i + 4] === 0 ? words[i + 3] : words[i + 3] + words[i + 4] * TWO_32;
        if (hi === 0) p.setIndexBuffer(table[words[i + 1]], format, offset, lo);
        else if (lo === ALL && hi === ALL) p.setIndexBuffer(table[words[i + 1]], format, offset);
        else p.setIndexBuffer(table[words[i + 1]], format, offset, lo + hi * TWO_32);
        i += 7;
        break;
      }
      case 7:
        p.drawIndirect(table[words[i + 1]], join(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case 8:
        p.drawIndexedIndirect(table[words[i + 1]], join(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case 9:
        p.dispatchWorkgroups(words[i + 1], words[i + 2], words[i + 3]);
        i += 4;
        break;
      case 10:
        p.dispatchWorkgroupsIndirect(table[words[i + 1]], join(words[i + 2], words[i + 3]));
        i += 4;
        break;
      case 11:
        p.end();
        // Nothing follows the end of a pass in one replay.
        return true;
      default:
        throw new Error(`unknown batched command ${words[i]} at word ${i}`);
    }
  }
  return false;
}
