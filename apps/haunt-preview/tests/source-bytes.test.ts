import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { sourceBytesHash, sourceHash } from "../src/protocol.ts";

test("distinct malformed UTF-8 bytes hash differently", async () => {
  const a = new Uint8Array([0xff]);
  const b = new Uint8Array([0xfe]);
  assert.notEqual(await sourceBytesHash(a), await sourceBytesHash(b));
});

test("identical offset subarray and standalone bytes hash equally", async () => {
  const backing = new Uint8Array([9, 8, 7, 6, 5]);
  const view = backing.subarray(1, 4);
  const standalone = new Uint8Array([8, 7, 6]);
  assert.equal(await sourceBytesHash(view), await sourceBytesHash(standalone));
});

test("SharedArrayBuffer-backed view hashes a copied exact view", async (t) => {
  if (typeof SharedArrayBuffer === "undefined") {
    t.skip("SharedArrayBuffer is unavailable");
    return;
  }
  const backing = new SharedArrayBuffer(5);
  const view = new Uint8Array(backing);
  view.set([9, 8, 7, 6, 5]);
  const subview = view.subarray(1, 4);
  const standalone = new Uint8Array([8, 7, 6]);
  assert.equal(await sourceBytesHash(subview), await sourceBytesHash(standalone));
});

test("string sourceHash equals Node createHash sha256 expected hex", async () => {
  const source = "haunted-house\n\u0000\u00ff";
  const expected = createHash("sha256").update(source, "utf8").digest("hex");
  assert.equal(await sourceHash(source), expected);
});
