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
  const completedAt = new Date().toISOString();
  const startedAt = new Date(Date.parse(completedAt) - 5 * 60_000).toISOString();
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
        startedAt,
        completedAt,
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
        OA_LIVEKIT_PRIVACY_FIXTURE_FILES: "I_ACCEPT_TEST_FIXTURES_ONLY",
      },
    },
  );

const runSecretManager = (fixtureValue, output = join(fixtureValue.root, "privacy-gcp.json")) => {
  const fakeBin = join(fixtureValue.root, "bin");
  mkdirSync(fakeBin);
  const fakeGcloud = join(fakeBin, "gcloud");
  writeFileSync(
    fakeGcloud,
    `#!/usr/bin/env node
if (!process.argv.includes("--project=openagentsgemini") || process.argv.some((value) => value.startsWith("--out-file"))) process.exit(2);
const secret = process.argv.find((value) => value.startsWith("--secret="));
if (secret === "--secret=oa-livekit-prod-openai-api-key") process.stdout.write(${JSON.stringify(
      `sk-${"o".repeat(40)}`,
    )});
else if (secret === "--secret=sarah-nostr-identity-secret") process.stdout.write(${JSON.stringify(
      "sarah-private-key-material-must-never-reach-workers",
    )});
else process.exitCode = 2;
`,
    { mode: 0o700 },
  );
  chmodSync(fakeGcloud, 0o700);
  return spawnSync(
    process.execPath,
    [
      script,
      "--source-base-revision",
      revision,
      "--gcp-secret-manager",
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
        PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
      },
    },
  );
};

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

test("consumes the exact production Secret Manager resources without writing their values", () => {
  const inputs = fixture();
  const output = join(inputs.root, "privacy-gcp.json");
  const result = runSecretManager(inputs, output);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.includes(readFileSync(inputs.openAiKey, "utf8")), false);
  assert.equal(result.stdout.includes(readFileSync(inputs.sarahPrivateKey, "utf8")), false);
  assert.equal(
    readFileSync(output, "utf8").includes(readFileSync(inputs.openAiKey, "utf8")),
    false,
  );
  assert.equal(JSON.parse(readFileSync(output, "utf8")).outcome, "passed");
});

test("refuses production secret files without the fixture-only gate", () => {
  const inputs = fixture();
  const result = spawnSync(
    process.execPath,
    [
      script,
      "--source-base-revision",
      revision,
      "--openai-key-file",
      inputs.openAiKey,
      "--sarah-private-key-file",
      inputs.sarahPrivateKey,
      "--retention-canary-file",
      inputs.canary,
      ...inputs.scopeArguments,
      "--output",
      join(inputs.root, "refused.json"),
      "--apply",
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        OA_LIVEKIT_OWNER_GATE: "I_ACCEPT_EP263_LIVEKIT_GCP_COST",
        OA_LIVEKIT_PRIVACY_FIXTURE_FILES: "",
      },
    },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /deterministic test fixtures/u);
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
