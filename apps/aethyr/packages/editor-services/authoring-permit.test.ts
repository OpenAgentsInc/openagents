import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { AuthoringPermit, type HostGrant } from "./authoring-permit";
// The deployed adapter injects the public Rust SDK digest; this fixture tests permission scope.
const digest = (action: unknown) =>
  createHash("sha256").update(JSON.stringify(action)).digest("hex");
const project = { id: "haunted-house", branch: "main", revision: 0 };
const action = {
  type: "propose",
  operations: [{ type: "move", target: "werewolf-1", position: [-2, 0, -14] }],
};
const grant: HostGrant = {
  token: "a".repeat(64),
  action_sha256: createHash("sha256")
    .update(JSON.stringify(action))
    .digest("hex"),
  session: "session-1",
  release: "house-v2",
  project,
  draft: { id: "private-draft", branch: "house", revision: 0 },
  capability: "propose",
  expires_at: 200,
};
test("guest mutations fail; confirmed permission executes only its exact action once", async () => {
  const gate = new AuthoringPermit(digest);
  await assert.rejects(gate.consume(action, project, 100));
  gate.authorize(grant, project, "house-v2", 100);
  await gate.consume(action, project, 100);
  await assert.rejects(gate.consume(action, project, 100));
  assert.throws(() => gate.authorize(grant, project, "house-v2", 100));
});
test("expiry, wrong release/project/action, and revoked access cannot execute", async () => {
  for (const change of [
    { release: "old" },
    { project: { ...project, branch: "another" } },
    { expires_at: 0 },
  ]) {
    assert.throws(() =>
      new AuthoringPermit(digest).authorize(
        { ...grant, ...change },
        project,
        "house-v2",
        100,
      ),
    );
  }
  const gate = new AuthoringPermit(digest);
  gate.authorize(grant, project, "house-v2", 100);
  await assert.rejects(
    gate.consume({ ...action, operations: [] }, project, 100),
  );
  gate.authorize(grant, project, "house-v2", 100);
  gate.revoke();
  await assert.rejects(gate.consume(action, project, 100));
  gate.authorize(grant, project, "house-v2", 100);
  await assert.rejects(gate.consume(action, project, 200000));
});
