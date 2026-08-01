import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  PRIVACY_SCOPE_EXPORT_SCHEMA,
  PRIVACY_SCOPE_MANIFEST,
  PRIVACY_SCOPES,
  scanSarahLiveKitPrivacy,
} from "./livekit-privacy-scan-lib.mjs";

const revision = "a".repeat(40);
const openAiKey = Buffer.from(`sk-${"o".repeat(40)}`);
const sarahPrivateKey = Buffer.from("sarah-private-key-material-must-never-reach-workers");
const canary = Buffer.from("acceptance-retention-canary-unique-value");

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "livekit-privacy-scan-"));
  const scopeInputs = {};
  for (const scope of PRIVACY_SCOPES) {
    const directory = join(root, scope);
    mkdirSync(directory);
    writeFileSync(join(directory, "export.json"), '{"state":"redacted"}\n');
    writeFileSync(
      join(directory, PRIVACY_SCOPE_MANIFEST),
      JSON.stringify({
        schemaVersion: PRIVACY_SCOPE_EXPORT_SCHEMA,
        scope,
        sourceBaseRevision: revision,
        collectionMode: "read_only",
        complete: true,
        startedAt: "2026-07-30T23:58:00.000Z",
        completedAt: "2026-07-30T23:59:00.000Z",
        objectCount: 1,
        byteCount: 21,
      }),
    );
    scopeInputs[scope] = directory;
  }
  return { root, scopeInputs };
};

const scan = (scopeInputs) =>
  scanSarahLiveKitPrivacy({
    scopeInputs,
    openAiKey,
    sarahPrivateKey,
    canaries: [canary],
    sourceBaseRevision: revision,
    observedAt: "2026-07-31T00:00:00.000Z",
  });

const addPayload = (directory, name, bytes) => {
  writeFileSync(join(directory, name), bytes);
  const manifestPath = join(directory, PRIVACY_SCOPE_MANIFEST);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  manifest.objectCount += 1;
  manifest.byteCount += Buffer.byteLength(bytes);
  writeFileSync(manifestPath, JSON.stringify(manifest));
};

test("passes complete clean exports and emits only redacted aggregate evidence", () => {
  const { scopeInputs } = fixture();
  const result = scan(scopeInputs);
  assert.equal(result.outcome, "passed");
  assert.equal(result.results.scopeResults.length, 8);
  assert.equal(result.results.findings, 0);
  const serialized = JSON.stringify(result);
  assert.equal(serialized.includes(openAiKey.toString()), false);
  assert.equal(serialized.includes(sarahPrivateKey.toString()), false);
  assert.equal(serialized.includes(canary.toString()), false);
  assert.match(serialized, /"state":"complete"/u);
});

test("rejects an OpenAI key in either packaged client scope", () => {
  for (const scope of ["packaged_omega", "packaged_clients"]) {
    const { scopeInputs } = fixture();
    addPayload(scopeInputs[scope], "bundle.bin", openAiKey);
    const result = scan(scopeInputs);
    assert.equal(result.outcome, "failed");
    assert.ok(result.results.findings > 0);
  }
});

test("rejects Sarah private identity material in a worker pod export", () => {
  const { scopeInputs } = fixture();
  addPayload(scopeInputs.pods, "worker-environment.bin", sarahPrivateKey);
  const result = scan(scopeInputs);
  assert.equal(result.outcome, "failed");
  assert.ok(result.results.findings > 0);
});

test("rejects retained canaries, transcript payloads, and raw media in runtime scopes", () => {
  const { scopeInputs } = fixture();
  addPayload(scopeInputs.logs, "log-export.bin", canary);
  addPayload(scopeInputs.redis, "keys.json", '{"transcript":"retained words"}');
  addPayload(
    scopeInputs.object_storage,
    "object.bin",
    Buffer.concat([Buffer.from("RIFF0000WAVE"), Buffer.alloc(8)]),
  );
  const result = scan(scopeInputs);
  assert.equal(result.outcome, "failed");
  assert.ok(result.results.findings > 0);
  assert.equal(result.results.transcriptObjects, 1);
  assert.equal(result.results.rawMediaObjects, 1);
});

test("fails closed for missing scopes, empty exports, and symlinks", () => {
  const missing = fixture();
  delete missing.scopeInputs.traces;
  assert.throws(() => scan(missing.scopeInputs), /exactly the eight/u);

  const empty = fixture();
  const emptyDirectory = join(empty.root, "empty");
  mkdirSync(emptyDirectory);
  writeFileSync(join(emptyDirectory, "empty.bin"), "");
  writeFileSync(
    join(emptyDirectory, PRIVACY_SCOPE_MANIFEST),
    JSON.stringify({
      schemaVersion: PRIVACY_SCOPE_EXPORT_SCHEMA,
      scope: "logs",
      sourceBaseRevision: revision,
      collectionMode: "read_only",
      complete: true,
      startedAt: "2026-07-30T23:58:00.000Z",
      completedAt: "2026-07-30T23:59:00.000Z",
      objectCount: 1,
      byteCount: 0,
    }),
  );
  assert.throws(
    () =>
      scanSarahLiveKitPrivacy({
        scopeInputs: { ...empty.scopeInputs, logs: emptyDirectory },
        openAiKey,
        sarahPrivateKey,
        canaries: [canary],
        sourceBaseRevision: revision,
        observedAt: "2026-07-31T00:00:00.000Z",
      }),
    /no evidence bytes/u,
  );

  const linked = fixture();
  symlinkSync(join(linked.scopeInputs.logs, "export.json"), join(linked.scopeInputs.logs, "link"));
  assert.throws(() => scan(linked.scopeInputs), /symbolic link/u);
});

test("fails closed when scope exports do not share one bounded capture window", () => {
  const { scopeInputs } = fixture();
  const manifestPath = join(scopeInputs.traces, PRIVACY_SCOPE_MANIFEST);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  manifest.startedAt = "2026-07-30T20:00:00.000Z";
  manifest.completedAt = "2026-07-30T20:01:00.000Z";
  writeFileSync(manifestPath, JSON.stringify(manifest));
  assert.throws(() => scan(scopeInputs), /one bounded capture window/u);
});
