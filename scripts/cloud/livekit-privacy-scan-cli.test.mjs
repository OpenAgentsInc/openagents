import assert from "node:assert/strict";
import {
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  PRIVACY_SCOPE_EXPORT_SCHEMA,
  PRIVACY_SCOPE_MANIFEST,
  PRIVACY_SCOPES,
} from "./livekit-privacy-scan-lib.mjs";

const script = resolve(import.meta.dirname, "livekit-privacy-scan.mjs");
const revision = "a".repeat(40);

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "livekit-privacy-scan-cli-"));
  const scopeArguments = [];
  for (const scope of PRIVACY_SCOPES) {
    const directory = join(root, scope);
    mkdirSync(directory);
    const payload = '{"state":"redacted"}\n';
    writeFileSync(join(directory, "export.json"), payload);
    writeFileSync(
      join(directory, PRIVACY_SCOPE_MANIFEST),
      JSON.stringify({
        schemaVersion: PRIVACY_SCOPE_EXPORT_SCHEMA,
        scope,
        sourceBaseRevision: revision,
        collectionMode: "read_only",
        complete: true,
        startedAt: "2026-07-31T09:00:00.000Z",
        completedAt: "2026-07-31T09:05:00.000Z",
        objectCount: 1,
        byteCount: Buffer.byteLength(payload),
      }),
    );
    scopeArguments.push("--scope", `${scope}=${directory}`);
  }
  const secret = (name, contents) => {
    const path = join(root, name);
    writeFileSync(path, contents, { mode: 0o600 });
    chmodSync(path, 0o600);
    return path;
  };
  return {
    root,
    scopeArguments,
    openAiKey: secret("openai-key", `sk-${"o".repeat(40)}`),
    sarahPrivateKey: secret(
      "sarah-private-key",
      "sarah-private-key-material-must-never-reach-workers",
    ),
    canary: secret("retention-canary", "acceptance-retention-canary-unique-value"),
  };
};

const run = (fixtureValue, output = join(fixtureValue.root, "privacy.json")) =>
  spawnSync(
    process.execPath,
    [
      script,
      "--source-base-revision",
      revision,
      "--openai-key-file",
      fixtureValue.openAiKey,
      "--sarah-private-key-file",
      fixtureValue.sarahPrivateKey,
      "--retention-canary-file",
      fixtureValue.canary,
      ...fixtureValue.scopeArguments,
      "--output",
      output,
      "--apply",
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        OA_LIVEKIT_OWNER_GATE: "I_ACCEPT_EP263_LIVEKIT_GCP_COST",
      },
    },
  );

test("accepts mode-0600 regular secret inputs without printing their values", () => {
  const inputs = fixture();
  const output = join(inputs.root, "privacy.json");
  const result = run(inputs, output);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.includes(readFileSync(inputs.openAiKey, "utf8")), false);
  assert.equal(result.stdout.includes(readFileSync(inputs.sarahPrivateKey, "utf8")), false);
  assert.equal(result.stdout.includes(readFileSync(inputs.canary, "utf8")), false);
  assert.equal(JSON.parse(readFileSync(output, "utf8")).outcome, "passed");
});

test("rejects group-readable and symbolic-link secret inputs", () => {
  const permissive = fixture();
  chmodSync(permissive.openAiKey, 0o640);
  const permissiveResult = run(permissive);
  assert.notEqual(permissiveResult.status, 0);
  assert.match(permissiveResult.stderr, /mode 0600 or stricter/u);

  const linked = fixture();
  const linkedPath = join(linked.root, "linked-openai-key");
  symlinkSync(linked.openAiKey, linkedPath);
  linked.openAiKey = linkedPath;
  const linkedResult = run(linked);
  assert.notEqual(linkedResult.status, 0);
  assert.match(linkedResult.stderr, /regular files/u);
});
