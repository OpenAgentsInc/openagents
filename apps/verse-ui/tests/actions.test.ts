import { test } from "node:test";
import assert from "node:assert/strict";
import { actionNames } from "../src/actions";

test("the tool finder cannot confirm, publish, save, or execute a supplied command", () => {
  assert.ok(Object.keys(actionNames).length > 5);
  for (const id of Object.keys(actionNames))
    assert.doesNotMatch(
      id,
      /confirm|submit|publish|revoke|save|generate|apply|send/,
    );
});
