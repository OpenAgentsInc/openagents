import assert from "node:assert/strict";
import test from "node:test";
import { CaptureRequests } from "../packages/editor-services/capture-requests.ts";

test("only an explicitly requested download can produce a download", async () => {
  const held = new CaptureRequests(),
    blob = new Blob(["owned"]);
  assert.equal(held.complete("unknown", blob), undefined);
  const pending = held.blob("frame");
  assert.equal(held.complete("another-frame", blob), undefined);
  assert.equal(held.busy, true);
  assert.equal(held.complete("frame", blob), "blob");
  assert.equal(await pending, blob);
  assert.equal(held.complete("frame", blob), undefined);
  held.download("download");
  assert.equal(held.complete("download", blob), "download");
  assert.equal(held.busy, false);
});
test("errors, cancellation and late responses leave no download or retained flight", async () => {
  const held = new CaptureRequests(),
    blob = new Blob(["owned"]);
  for (const cancelled of [false, true]) {
    const pending = held.blob("frame");
    const rejected = assert.rejects(pending, /failed|cancelled/);
    if (cancelled) held.cancel();
    else held.fail("frame", new Error("failed"));
    await rejected;
    assert.equal(held.busy, false);
    assert.equal(held.complete("frame", blob), undefined);
  }
  let expired = "";
  const timed = new CaptureRequests((id) => (expired = id));
  const pending = timed.blob("slow", 5);
  await assert.rejects(pending, /too long/);
  assert.equal(expired, "slow");
  assert.equal(timed.busy, false);
  assert.equal(timed.complete("slow", blob), undefined);
});
test("one flight is enforced and 10907 sequential frames retain no previous requests", async () => {
  const held = new CaptureRequests(),
    blob = new Blob([new Uint8Array(4)]);
  const first = held.blob("first");
  await assert.rejects(held.blob("second"), /current screenshot/);
  assert.throws(() => held.download("third"));
  held.complete("first", blob);
  await first;
  for (let i = 0; i < 10907; i++) {
    const pending = held.blob(String(i));
    assert.equal(held.complete(String(i), blob), "blob");
    assert.equal(await pending, blob);
    assert.equal(held.busy, false);
  }
});
test("pixel flights reject other result types and release every response, error, cancellation and timeout", async () => {
  const held = new CaptureRequests(),
    pixels = new ArrayBuffer(4),
    blob = new Blob(["wrong type"]);
  const first = held.pixels("pixels");
  assert.equal(held.complete("pixels", blob), undefined);
  assert.equal(held.completePixels("wrong", pixels), false);
  assert.equal(held.busy, true);
  await assert.rejects(held.pixels("overlap"), /current screenshot/);
  assert.equal(held.completePixels("pixels", pixels), true);
  assert.equal(await first, pixels);
  const image = held.blob("image");
  assert.equal(held.completePixels("image", pixels), false);
  held.complete("image", blob);
  await image;
  for (const cancel of [false, true]) {
    const pending = held.pixels("failed"),
      check = assert.rejects(pending, /failed|cancelled/);
    if (cancel) held.cancel();
    else held.fail("failed", new Error("failed"));
    await check;
    assert.equal(held.completePixels("failed", pixels), false);
    assert.equal(held.busy, false);
  }
  await assert.rejects(held.pixels("slow", 5), /too long/);
  assert.equal(held.completePixels("slow", pixels), false);
  for (let i = 0; i < 10907; i++) {
    const pending = held.pixels(String(i));
    held.completePixels(String(i), pixels);
    assert.equal(await pending, pixels);
    assert.equal(held.busy, false);
  }
});
