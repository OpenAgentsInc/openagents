import assert from "node:assert/strict";
import { test } from "node:test";
import { CommandQueue } from "../src/command-queue.ts";
import { MaterialBridge } from "../src/material-bridge.ts";
import { MAX_SOURCE_BYTES } from "../src/protocol.ts";

type Preview = Record<string, unknown>;
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
};
async function until(condition: () => boolean) {
  for (let i = 0; i < 100; i++) {
    if (condition()) return;
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  assert.fail("The expected commit did not finish");
}
function fixture(pending = false) {
  let bridge!: ReturnType<typeof MaterialBridge>;
  const editor = new CommandQueue();
  let project = { revision: 0, materialDraft: { source: "baseline" } };
  let file = "baseline", failWrite = false, failSubmit = false, previewSource = "";
  let previewPromise: Promise<Preview> | undefined;
  const commits: string[] = [], writes: string[] = [], errors: unknown[] = [];
  const requests: Array<{ source: string; resolve: (value: Preview) => void }> = [];
  bridge = MaterialBridge({
    project: () => project,
    submit: (source: string, _origin: string) => editor.run(async () => {
      if (failSubmit) { failSubmit = false; throw new Error("async submit failed"); }
      commits.push(source);
      const committed = { revision: project.revision + 1, materialDraft: { source } };
      project = committed;
      await bridge.onProject(committed);
      return { revision: committed.revision };
    }),
    preview: (source: string) => {
      if (source === previewSource && previewPromise) return previewPromise;
      const request = deferred<Preview>();
      previewSource = source;
      previewPromise = request.promise;
      requests.push({ source, resolve: request.resolve });
      if (!pending) request.resolve({ type: "accepted", source });
      return request.promise;
    },
    write: async (source: string) => {
      writes.push(source);
      if (writes.length > 20) throw new Error("Unexpected recursive source writes");
      if (failWrite) { failWrite = false; throw new Error("write failed"); }
      file = source;
      bridge.fileChanged(source);
    },
    error: (error: unknown) => { errors.push(error); },
  });
  return {
    bridge, editor, commits, writes, errors, requests,
    project: () => structuredClone(project),
    file: () => file,
    emit: (source: string) => { file = source; bridge.fileChanged(source); },
    failNext: () => { failWrite = true; },
    failNextSubmit: () => { failSubmit = true; },
  };
}

test("source commits proceed while previews wait, and each caller gets its own request", { timeout: 2000 }, async () => {
  const f = fixture(true);
  const first = f.bridge.save("first");
  const second = f.bridge.save("second");
  const settled = Promise.allSettled([first, second]);
  try {
    await until(() => f.requests.length === 2);
    assert.deepEqual(f.commits, ["first", "second"]);
    f.requests[0].resolve({ type: "superseded", source: "first" });
    assert.deepEqual(await first, { type: "superseded", source: "first" });
    f.requests[1].resolve({ type: "accepted", source: "second" });
    assert.deepEqual(await second, { type: "accepted", source: "second" });
    assert.equal(f.project().materialDraft.source, "second");
    assert.equal(f.file(), "second");
    assert.deepEqual(f.errors, []);
  } finally {
    for (const request of f.requests) request.resolve({ type: "cleanup" });
    await settled;
  }
});

test("a newest file rewrite is retained behind queued work and mirrored writes do not recurse", { timeout: 2000 }, async () => {
  const f = fixture();
  const gate = deferred<void>();
  const blocked = f.editor.run(() => gate.promise);
  const first = f.bridge.save("hold");
  first.catch(() => {});
  f.emit("middle");
  f.emit("baseline");
  gate.resolve();
  await blocked;
  await first;
  await until(() => f.commits.length >= 3 && f.file() === "baseline");
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.deepEqual(f.commits, ["hold", "middle", "baseline"]);
  assert.equal(f.project().materialDraft.source, "baseline");
  assert.deepEqual(f.errors, []);
});

test("retrying a failed source mirror repairs the file without another history entry", { timeout: 2000 }, async () => {
  const f = fixture();
  f.failNext();
  await assert.rejects(f.bridge.save("first"), /write failed/);
  assert.equal(f.project().materialDraft.source, "first");
  assert.equal(f.file(), "baseline");
  assert.deepEqual(await f.bridge.save("first"), { type: "accepted", source: "first" });
  assert.deepEqual(f.commits, ["first"]);
  assert.equal(f.file(), "first");
  assert.deepEqual(f.errors, []);
});

test("oversized source is rejected before committing and file errors reach the host", { timeout: 2000 }, async () => {
  const f = fixture();
  const oversized = "a".repeat(MAX_SOURCE_BYTES + 1);
  await assert.rejects(async () => f.bridge.save(oversized));
  assert.deepEqual(f.commits, []);
  assert.deepEqual(f.writes, []);
  assert.doesNotThrow(() => f.emit(oversized));
  await until(() => f.errors.length === 1);
  assert.deepEqual(f.commits, []);
});

test("retrying a mirror reuses the pending preview for that revision", { timeout: 2000 }, async () => {
  const f = fixture(true);
  f.failNext();
  await assert.rejects(f.bridge.save("first"), /write failed/);
  const retry = f.bridge.save("first");
  const settled = Promise.allSettled([retry]);
  try {
    await until(() => f.file() === "first");
    assert.deepEqual(f.commits, ["first"]);
    assert.equal(f.requests.length, 1);
    f.requests[0].resolve({ type: "accepted", source: "first", request: "original" });
    assert.deepEqual(await retry, { type: "accepted", source: "first", request: "original" });
  } finally {
    for (const request of f.requests) request.resolve({ type: "cleanup" });
    await settled;
  }
});

test("a file callback reports an asynchronous failed command and accepts the next edit", { timeout: 2000 }, async () => {
  const f = fixture();
  f.failNextSubmit();
  assert.doesNotThrow(() => f.emit("from-file"));
  await until(() => f.errors.length === 1);
  assert.match(String(f.errors[0]), /async submit failed/);
  assert.deepEqual(f.commits, []);
  f.emit("next");
  await until(() => f.project().materialDraft.source === "next");
  assert.deepEqual(f.commits, ["next"]);
  assert.equal(f.file(), "next");
  assert.equal(f.errors.length, 1);
});
