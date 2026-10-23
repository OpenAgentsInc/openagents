import assert from "node:assert/strict";
import { test } from "node:test";
import { readChanges } from "../src/changes.ts";

const row = {
  id: "one",
  author: "Test creator",
  at: "2026-10-22T12:00:00Z",
  summary: "Changed the lantern light",
  revision: 1,
  scope: "draft",
  earnings: null,
};
test("local changes retain attribution but cannot claim a payment or public link", () => {
  const [actual] = readChanges(
    JSON.stringify({
      schema: 1,
      changes: [
        {
          ...row,
          earnings: { amount: 1000 },
          url: "https://example.com",
          scope: "world",
        },
      ],
    }),
  );
  assert.equal(actual.author, row.author);
  assert.equal(actual.earnings, null);
  assert.equal(actual.url, undefined);
  assert.equal(actual.scope, "draft");
});
test("malformed and oversized histories fail instead of becoming empty logs", () => {
  assert.throws(() => readChanges(JSON.stringify({ schema: 2, changes: [] })));
  assert.throws(() =>
    readChanges(
      JSON.stringify({ schema: 1, changes: [{ ...row, at: "yesterday" }] }),
    ),
  );
  assert.throws(() =>
    readChanges(JSON.stringify({ schema: 1, changes: Array(65).fill(row) })),
  );
  assert.throws(() => readChanges(" ".repeat(64 * 1024 + 1)));
});
