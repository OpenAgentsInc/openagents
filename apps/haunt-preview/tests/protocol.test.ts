import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ASSET,
  PROJECT,
  ShaderVersions,
  sourceHash,
  validEdit,
} from "../src/protocol.ts";

test("invalid and out-of-order drafts cannot replace the last working pipeline", () => {
  const released: string[] = [];
  const versions = new ShaderVersions<string>((r) => released.push(r));
  versions.begin(1);
  versions.stage(1, "one", "working");
  versions.commit();
  versions.begin(2); // slow valid compile
  versions.begin(3); // a newer invalid source
  assert.equal(versions.stage(2, "two", "stale"), false);
  assert.equal(versions.commit(), undefined);
  assert.equal(versions.active?.resource, "working");
  versions.begin(4);
  versions.stage(4, "four", "fixed");
  assert.equal(versions.active?.resource, "working"); // not installed until frame boundary
  assert.equal(versions.commit()?.resource, "fixed");
  assert.deepEqual(released, ["stale", "working"]);
});

test("a new invalid draft cancels a prepared but not displayed pipeline", () => {
  const released: number[] = [];
  const versions = new ShaderVersions<number>((r) => released.push(r));
  versions.begin(1);
  versions.stage(1, "one", 1);
  versions.commit();
  versions.begin(2);
  versions.stage(2, "two", 2);
  versions.begin(3);
  assert.equal(versions.commit(), undefined);
  assert.equal(versions.active?.resource, 1);
  assert.deepEqual(released, [2]);
  versions.dispose();
  versions.dispose();
  assert.equal(versions.stage(3, "late", 3), false);
  assert.deepEqual(released, [2, 1, 3]);
  assert.equal(versions.begin(4), false);
});

test("the bridge rejects foreign targets, replay shapes, and oversized UTF-8", async () => {
  const source = "@fragment fn main() {}";
  const edit = {
    type: "shader",
    project: PROJECT,
    preview: "p",
    token: "secret",
    request: "1",
    revision: 1,
    asset: ASSET,
    hash: await sourceHash(source),
    source,
    savedAt: 42,
  };
  assert.ok(validEdit(edit, "p", "secret"));
  for (const change of [
    { token: "foreign" },
    { preview: "elsewhere" },
    { project: "elsewhere" },
    { asset: "../secrets" },
    { revision: NaN },
    { revision: 0 },
    { hash: "bad" },
    { source: "é".repeat(32769) },
    { savedAt: Infinity },
  ]) {
    assert.equal(validEdit({ ...edit, ...change }, "p", "secret"), false);
  }
});
