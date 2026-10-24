import assert from "node:assert/strict";
import test from "node:test";
import {
  PREVIEW_PROTOCOL,
  isPreviewEvent,
  previewEventTransfers,
} from "../packages/editor-services/protocol.ts";
test("pixel ownership transfers across both preview hops with no retained sender buffer", () => {
  const original = {
    protocol: PREVIEW_PROTOCOL,
    sessionId: "owned",
    requestId: "frame",
    type: "capturePixels",
    pixels: new Uint8Array([1, 2, 3, 255]).buffer,
  };
  assert.equal(isPreviewEvent(original, "owned"), true);
  const preview = structuredClone(original, {
    transfer: previewEventTransfers(original),
  });
  assert.equal(original.pixels.byteLength, 0);
  const editor = structuredClone(preview, {
    transfer: previewEventTransfers(preview),
  });
  assert.equal(preview.pixels.byteLength, 0);
  assert.deepEqual([...new Uint8Array(editor.pixels)], [1, 2, 3, 255]);
  assert.equal(isPreviewEvent(editor, "owned"), true);
  assert.equal(isPreviewEvent(original, "owned"), false);
  for (const pixels of [
    new ArrayBuffer(0),
    new ArrayBuffer(3),
    new ArrayBuffer(32 * 1024 * 1024 + 4),
    new Uint8Array(4),
  ])
    assert.equal(isPreviewEvent({ ...editor, pixels }, "owned"), false);
  assert.equal(
    isPreviewEvent({ ...editor, requestId: undefined }, "owned"),
    false,
  );
  assert.deepEqual(
    previewEventTransfers({ type: "capture", blob: new Blob(["legacy"]) }),
    [],
  );
});
