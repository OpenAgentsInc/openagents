import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  PRIVACY_SCOPE_EXPORT_SCHEMA,
  PRIVACY_SCOPE_MANIFEST,
} from "./livekit-privacy-scan-lib.mjs";

const script = resolve(import.meta.dirname, "livekit-privacy-export.mjs");
const revision = "a".repeat(40);
const startedAt = "2026-07-31T09:00:00.000Z";
const completedAt = "2026-07-31T09:05:00.000Z";

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "livekit-privacy-export-"));
  writeFileSync(join(root, "first.json"), '{"state":"redacted"}\n');
  mkdirSync(join(root, "nested"));
  writeFileSync(join(root, "nested", "second.bin"), Buffer.from([1, 2, 3]));
  return root;
};

const run = (root, extra = [], env = {}) =>
  spawnSync(
    process.execPath,
    [
      script,
      "--scope",
      "logs",
      "--source-base-revision",
      revision,
      "--started-at",
      startedAt,
      "--completed-at",
      completedAt,
      "--input",
      root,
      "--apply",
      ...extra,
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        OA_LIVEKIT_OWNER_GATE: "I_ACCEPT_EP263_LIVEKIT_GCP_COST",
        ...env,
      },
    },
  );

test("seals a complete export without printing object names", () => {
  const root = fixture();
  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.includes("first.json"), false);
  assert.equal(result.stdout.includes("second.bin"), false);
  const manifestPath = join(root, PRIVACY_SCOPE_MANIFEST);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.deepEqual(manifest, {
    schemaVersion: PRIVACY_SCOPE_EXPORT_SCHEMA,
    scope: "logs",
    sourceBaseRevision: revision,
    collectionMode: "read_only",
    complete: true,
    startedAt,
    completedAt,
    objectCount: 2,
    byteCount: 24,
  });
  assert.equal(readFileSync(manifestPath).length > 0, true);
});

test("requires the owner gate and refuses to overwrite a manifest", () => {
  const gated = run(fixture(), [], { OA_LIVEKIT_OWNER_GATE: "" });
  assert.notEqual(gated.status, 0);
  assert.match(gated.stderr, /OA_LIVEKIT_OWNER_GATE/u);

  const root = fixture();
  assert.equal(run(root).status, 0);
  const repeated = run(root);
  assert.notEqual(repeated.status, 0);
  assert.match(repeated.stderr, /already exists/u);
});

test("rejects symbolic links and empty evidence", () => {
  const linked = fixture();
  symlinkSync(join(linked, "first.json"), join(linked, "linked.json"));
  const linkedResult = run(linked);
  assert.notEqual(linkedResult.status, 0);
  assert.match(linkedResult.stderr, /symbolic link/u);

  const empty = mkdtempSync(join(tmpdir(), "livekit-privacy-export-empty-"));
  writeFileSync(join(empty, "empty.bin"), "");
  const emptyResult = run(empty);
  assert.notEqual(emptyResult.status, 0);
  assert.match(emptyResult.stderr, /no evidence bytes/u);
});

test("writes a private manifest and rejects an invalid capture window", () => {
  const root = fixture();
  assert.equal(run(root).status, 0);
  assert.equal(statSync(join(root, PRIVACY_SCOPE_MANIFEST)).mode & 0o077, 0);

  const invalid = fixture();
  const result = run(invalid, ["--completed-at", "2026-07-31T12:00:01.000Z"]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /capture window/u);
});
