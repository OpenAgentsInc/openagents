import { describe, expect, it } from "vitest";
import { decodeFrame, encodeFrame, FRAME, MAX_HEADER_BYTES } from "./chat-frames";

/** The chat2 wire codec is a cross-language contract (Rust + Swift clients
 * re-implement it); these vectors pin the layout, not just round-tripping. */

const bytesOf = (len: number, seed: number): Uint8Array => {
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = (seed + i * 31) & 0xff;
  return out;
};

describe("chat2 frame codec", () => {
  it("pins the wire layout: [type u8][headerLen u32 LE][header][payload]", () => {
    const frame = encodeFrame(FRAME.push, { batchId: "b1" }, new Uint8Array([9, 8, 7]));
    expect(frame[0]).toBe(FRAME.push);
    const headerJson = JSON.stringify({ batchId: "b1" });
    expect(new DataView(frame.buffer).getUint32(1, true)).toBe(headerJson.length);
    expect(new TextDecoder().decode(frame.subarray(5, 5 + headerJson.length))).toBe(headerJson);
    expect([...frame.subarray(5 + headerJson.length)]).toEqual([9, 8, 7]);
  });

  it("round-trips every frame type, with and without payload", () => {
    for (const type of Object.values(FRAME)) {
      const payload = bytesOf(1000, type);
      const decoded = decodeFrame(encodeFrame(type, { seq: 7, device: "dev-a" }, payload));
      expect(decoded).toBeDefined();
      expect(decoded!.type).toBe(type);
      expect(decoded!.header).toEqual({ seq: 7, device: "dev-a" });
      expect(decoded!.payload).toEqual(payload);

      const bare = decodeFrame(encodeFrame(type, {}));
      expect(bare!.payload.length).toBe(0);
    }
  });

  it("round-trips a subarray view (offset ≠ 0 — the ws buffer case)", () => {
    const inner = encodeFrame(FRAME.row, { seq: 1 }, bytesOf(64, 3));
    const shifted = new Uint8Array(inner.length + 8);
    shifted.set(inner, 8);
    const decoded = decodeFrame(shifted.subarray(8));
    expect(decoded?.header).toEqual({ seq: 1 });
    expect(decoded?.payload).toEqual(bytesOf(64, 3));
  });

  it("rejects malformed frames as undefined, never throws", () => {
    expect(decodeFrame(new Uint8Array(0))).toBeUndefined();
    expect(decodeFrame(new Uint8Array([FRAME.hello]))).toBeUndefined(); // truncated length
    expect(decodeFrame(new Uint8Array([0x7f, 0, 0, 0, 0]))).toBeUndefined(); // unknown type
    // Header length pointing past the buffer.
    const truncated = encodeFrame(FRAME.hello, { cursor: 5 });
    new DataView(truncated.buffer).setUint32(1, 9999, true);
    expect(decodeFrame(truncated)).toBeUndefined();
    // Junk JSON in the header span.
    const junk = new Uint8Array([FRAME.hello, 2, 0, 0, 0, 0x7b, 0x7b]);
    expect(decodeFrame(junk)).toBeUndefined();
    // Valid JSON but not an object.
    const arr = new TextEncoder().encode("[1]");
    const arrFrame = new Uint8Array(5 + arr.length);
    arrFrame[0] = FRAME.hello;
    new DataView(arrFrame.buffer).setUint32(1, arr.length, true);
    arrFrame.set(arr, 5);
    expect(decodeFrame(arrFrame)).toBeUndefined();
  });

  it("rejects oversized headers (payloads are unbounded here; the DO caps frames)", () => {
    const fat = encodeFrame(FRAME.hello, { pad: "x".repeat(MAX_HEADER_BYTES) });
    expect(decodeFrame(fat)).toBeUndefined();
  });
});
