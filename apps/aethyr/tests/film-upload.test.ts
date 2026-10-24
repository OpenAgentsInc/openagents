import assert from "node:assert/strict";
import test from "node:test";
import { FilmUploadLedger } from "../tools/film/upload-ledger.ts";
const a = "a".repeat(64),
  b = "b".repeat(64);
test("lost upload acknowledgements retry identical pixels without a second encode", () => {
  const ledger = new FilmUploadLedger();
  assert.equal(ledger.reserve(0), false);
  ledger.verify(a);
  ledger.commit(a);
  ledger.release();
  assert.equal(ledger.next, 1);
  assert.equal(ledger.reserve(0), true);
  ledger.verify(a);
  ledger.commit(a);
  ledger.release();
  assert.equal(ledger.next, 1);
  assert.equal(ledger.reserve(0), true);
  assert.throws(() => ledger.verify(b), /different pixels/);
  ledger.release();
  assert.equal(ledger.reserve(1), false);
  ledger.commit(b);
  ledger.release();
  assert.equal(ledger.next, 2);
});
test("aborted bodies release the same frame and admission rejects gaps and overlap", () => {
  const ledger = new FilmUploadLedger();
  assert.throws(() => ledger.reserve(-1));
  assert.throws(() => ledger.reserve(2));
  ledger.reserve(0);
  assert.throws(() => ledger.reserve(0), /still arriving/);
  ledger.release();
  assert.equal(ledger.next, 0);
  assert.equal(ledger.reserve(0), false);
  ledger.commit(a);
  ledger.release();
  assert.equal(ledger.next, 1);
});
