import { test } from "node:test";
import assert from "node:assert/strict";
import { awaitAcceptedCommand } from "./accepted-command.ts";

test("draft receipt waits for matching source without advancing world history", async () => {
  let state = { revision: 4, phase: "ready", authoring: { draft: "previous" } };
  let settled = false;
  const pending = awaitAcceptedCommand({
    receipt: () => ({ ok: true, message: "Kept", revision: 4 }),
    status: () => state,
    closed: () => false,
    draft: "accepted",
    timeoutMs: 1000,
  }).then(() => {
    settled = true;
  });
  await new Promise((resolve) => setTimeout(resolve, 60));
  assert.equal(settled, false);
  state = { ...state, authoring: { draft: "unrelated" } };
  await new Promise((resolve) => setTimeout(resolve, 40));
  assert.equal(settled, false);
  state = { ...state, authoring: { draft: "accepted" } };
  await pending;
  assert.equal(state.revision, 4);
});

test("closed, failed and timed-out commands never acknowledge a stale draft", async () => {
  const base = {
    receipt: () => ({ ok: true, message: "Kept", revision: 4 }),
    status: () => ({
      revision: 4,
      phase: "ready",
      authoring: { draft: "old" },
    }),
    closed: () => false,
    draft: "new",
    timeoutMs: 30,
  };
  await assert.rejects(awaitAcceptedCommand(base), /too long/);
  await assert.rejects(
    awaitAcceptedCommand({ ...base, closed: () => true }),
    /closed/,
  );
  await assert.rejects(
    awaitAcceptedCommand({
      ...base,
      receipt: () => ({ ok: false, message: "Refused", revision: 4 }),
    }),
    /Refused/,
  );
  await assert.rejects(
    awaitAcceptedCommand({
      ...base,
      status: () => ({ ...base.status(), phase: "error" }),
    }),
    /could not open/,
  );
});
