import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, test } from "vitest";

const driver = resolve(import.meta.dirname, "managed-sandbox-phase2-driver.mjs");
const roots: string[] = [];
const digest = (value: string) => `sha256:${createHash("sha256").update(value).digest("hex")}`;
const marker = (sandboxRef: string, generation: number) => {
  const resourceRef = `gce-instance-ref://sha256/${digest(`resource|${sandboxRef}`).slice(7)}`;
  const diskRef = `gce-disk-ref://sha256/${digest(`disk|${sandboxRef}`).slice(7)}`;
  return digest(`${resourceRef}|${diskRef}|${generation}`).slice(7, 27);
};

const fakeGcloud = String.raw`#!/usr/bin/env node
import { appendFileSync, copyFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const root = process.env.FAKE_GCLOUD_ROOT;
const object = join(root, "object.tar");
const metadataPath = join(root, "metadata.json");
appendFileSync(join(root, "calls.jsonl"), JSON.stringify(args) + "\n");

if (args[0] === "compute" && args[1] === "ssh") {
  const command = args[args.indexOf("--command") + 1] ?? "";
  if (command.includes("managed-sandbox-guest-checkpoint.py restore")) {
    const content = readFileSync(join(root, "guest.tar"));
    const contentDigest = "sha256:" + (await import("node:crypto"))
      .createHash("sha256").update(content).digest("hex");
    process.stdout.write(JSON.stringify({
      contentBytes: content.byteLength,
      contentDigest,
      formatRef: "format.sbx.content-tar.v1",
      repositoryPostImageDigest: process.env.FAKE_RESTORE_REPOSITORY_POST_IMAGE_DIGEST,
    }));
  } else if (command.includes("managed-sandbox-guest-checkpoint.py create")) {
    const content = Buffer.from(process.env.FAKE_CONTENT_BASE64, "base64");
    const contentDigest = "sha256:" + (await import("node:crypto"))
      .createHash("sha256").update(content).digest("hex");
    process.stdout.write(JSON.stringify({
      contentBytes: content.byteLength,
      contentDigest,
      formatRef: "format.sbx.content-tar.v1",
      repositoryPostImageDigest: process.env.FAKE_REPOSITORY_POST_IMAGE_DIGEST,
    }));
  }
  process.exit(0);
}

if (args[0] === "compute" && args[1] === "scp") {
  if (args[2].startsWith("openagents@")) {
    writeFileSync(args[3], Buffer.from(process.env.FAKE_CONTENT_BASE64, "base64"));
  } else {
    copyFileSync(args[2], join(root, "guest.tar"));
  }
  process.exit(0);
}

if (args[0] === "compute" && args[1] === "instances") {
  process.stdout.write(process.env.FAKE_SERIAL_OUTPUT);
  process.exit(0);
}

if (args[0] === "storage" && args[1] === "objects" && args[2] === "describe") {
  if (!existsSync(object)) process.exit(1);
  process.stdout.write(readFileSync(metadataPath, "utf8"));
  process.exit(0);
}

if (args[0] === "storage" && args[1] === "cp") {
  if (args[2].startsWith("gs://")) {
    if (!existsSync(object)) process.exit(1);
    copyFileSync(object, args[3]);
    process.exit(0);
  }
  if (existsSync(object) && args.includes("--if-generation-match=0")) process.exit(1);
  copyFileSync(args[2], object);
  const flag = args.find((value) => value.startsWith("--custom-metadata="));
  const metadata = Object.fromEntries(
    flag.slice("--custom-metadata=".length).split(",").map((entry) => {
      const separator = entry.indexOf("=");
      return [entry.slice(0, separator), entry.slice(separator + 1)];
    }),
  );
  writeFileSync(metadataPath, JSON.stringify({
    generation: "1",
    metadata,
    size: String(readFileSync(object).byteLength),
  }));
  process.exit(0);
}

if (args[0] === "storage" && args[1] === "rm") {
  if (!existsSync(object)) process.exit(1);
  rmSync(object);
  rmSync(metadataPath);
  process.exit(0);
}

process.exit(1);
`;

const checkpointContent = "deterministic content-only checkpoint\n";
const repositoryPostImageDigest = digest("repository-post-image");
const restoreSandboxRef = "sandbox.sbx10.driver.restore";

const createCommand = {
  _tag: "CreateCheckpoint",
  schema: "openagents.managed_sandbox_phase2_command.v1",
  commandRef: "command.sbx10.driver.create",
  idempotencyRef: "idempotency.sbx10.driver.create",
  ownerRef: "owner.sbx10.driver",
  tenantRef: "tenant.sbx10.driver",
  requestedAt: "2099-07-22T03:05:00.000Z",
  checkpointRef: "checkpoint.sbx10.driver",
  sourceSandboxRef: "sandbox.sbx10.driver",
  sourceResourceGeneration: 7,
  sourceImageDigest: digest("source-image"),
  sourceToolchainDigest: digest("source-toolchain"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.cc70f05462",
  repositoryPostImageDigest,
  formatRef: "format.sbx.content-tar.v1",
  retainedUntil: "2099-07-23T03:05:00.000Z",
};

const makeFixture = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-msb-phase2-driver-test-"));
  roots.push(root);
  const executable = join(root, "fake-gcloud.mjs");
  writeFileSync(executable, fakeGcloud);
  chmodSync(executable, 0o755);
  return {
    root,
    env: {
      ...process.env,
      FAKE_CONTENT_BASE64: Buffer.from(checkpointContent).toString("base64"),
      FAKE_GCLOUD_ROOT: root,
      FAKE_REPOSITORY_POST_IMAGE_DIGEST: repositoryPostImageDigest,
      FAKE_RESTORE_REPOSITORY_POST_IMAGE_DIGEST: repositoryPostImageDigest,
      FAKE_SERIAL_OUTPUT: [
        `OA_MSB_READY:${marker(createCommand.sourceSandboxRef, 3)}:3`,
        `OA_MSB_PROBE:${marker(createCommand.sourceSandboxRef, 4)}:4`,
        `OA_MSB_READY:${marker(restoreSandboxRef, 8)}:8`,
        "OA_MSB_READY:0123456789abcdef0123:99",
        "",
      ].join("\n"),
      OA_MANAGED_SANDBOX_GCLOUD_BIN: executable,
      OA_MANAGED_SANDBOX_PHASE2_BUCKET: "oa-managed-sandbox-checkpoints-test",
      OA_MANAGED_SANDBOX_PROJECT_ID: "openagents-test",
      OA_MANAGED_SANDBOX_ZONE: "us-central1-a",
    },
  };
};

const invoke = (request: unknown, env: NodeJS.ProcessEnv) => {
  const result = spawnSync("node", [driver, "--managed-sandbox-phase2"], {
    encoding: "utf8",
    env,
    input: JSON.stringify(request),
  });
  return {
    ...result,
    output: result.status === 0 ? JSON.parse(result.stdout).result : undefined,
  };
};

const createRequest = (command = createCommand) => ({
  schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
  action: "create_checkpoint",
  requestRef: command.commandRef,
  command,
});

const restoreRequest = (checkpoint: unknown) => ({
  schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
  action: "restore_checkpoint",
  requestRef: "command.sbx10.driver.restore",
  command: {
    _tag: "RestoreCheckpoint",
    schema: "openagents.managed_sandbox_phase2_command.v1",
    commandRef: "command.sbx10.driver.restore",
    idempotencyRef: "idempotency.sbx10.driver.restore",
    ownerRef: createCommand.ownerRef,
    tenantRef: createCommand.tenantRef,
    requestedAt: "2099-07-22T03:08:00.000Z",
    checkpointRef: createCommand.checkpointRef,
    destinationSandboxRef: restoreSandboxRef,
    expectedSourceResourceGeneration: createCommand.sourceResourceGeneration,
    admittedServiceRefs: ["service.agent-runtime"],
    sourceCapabilityRefs: ["capability.sbx10.source"],
  },
  checkpoint,
  runtimeContext: {
    schema: "openagents.managed_sandbox_phase2_restore_context.v1",
    ownerRef: createCommand.ownerRef,
    tenantRef: createCommand.tenantRef,
    sandboxRef: restoreSandboxRef,
    resourceGeneration: 8,
    restoredCapabilityRefs: ["capability.sbx10.restore.fresh"],
  },
});

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

describe("managed-sandbox phase-two Google Cloud driver", () => {
  test("creates and reads back an opaque, content-only checkpoint", () => {
    const fixture = makeFixture();
    const result = invoke(createRequest(), fixture.env);

    expect(result.status).toBe(0);
    expect(result.output).toMatchObject({
      checkpointRef: createCommand.checkpointRef,
      contentBytes: Buffer.byteLength(checkpointContent),
      contentDigest: digest(checkpointContent),
      formatRef: "format.sbx.content-tar.v1",
      repositoryPostImageDigest,
      state: "completed",
    });
    expect(readFileSync(join(fixture.root, "object.tar"), "utf8")).toBe(checkpointContent);
    const calls = readFileSync(join(fixture.root, "calls.jsonl"), "utf8");
    for (const privateRef of [
      createCommand.ownerRef,
      createCommand.tenantRef,
      createCommand.checkpointRef,
      createCommand.sourceSandboxRef,
    ]) {
      expect(calls).not.toContain(privateRef);
    }
  });

  test("replays the stored result without recapturing guest content", () => {
    const fixture = makeFixture();
    const first = invoke(createRequest(), fixture.env);
    const second = invoke(createRequest(), fixture.env);
    const calls = readFileSync(join(fixture.root, "calls.jsonl"), "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line) as string[]);

    expect(first.status).toBe(0);
    expect(second.status).toBe(0);
    expect(second.output).toEqual(first.output);
    expect(
      calls.filter((args) =>
        args.some((value) => value.includes("managed-sandbox-guest-checkpoint.py")),
      ),
    ).toHaveLength(1);
  });

  test("refuses an idempotency conflict without overwriting the object", () => {
    const fixture = makeFixture();
    expect(invoke(createRequest(), fixture.env).status).toBe(0);
    const conflict = invoke(
      createRequest({ ...createCommand, repositoryRevisionRef: "commit.conflict" }),
      fixture.env,
    );

    expect(conflict.status).not.toBe(0);
    expect(conflict.stdout).toBe("");
    expect(conflict.stderr).toBe("");
    expect(readFileSync(join(fixture.root, "object.tar"), "utf8")).toBe(checkpointContent);
  });

  test("verifies exact bytes and reports later corruption", () => {
    const fixture = makeFixture();
    const created = invoke(createRequest(), fixture.env);
    const request = {
      schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
      action: "verify_checkpoint",
      requestRef: createCommand.checkpointRef,
      checkpoint: created.output,
    };

    expect(invoke(request, fixture.env).output.verified).toBe(true);
    writeFileSync(join(fixture.root, "object.tar"), "tampered\n");
    expect(invoke(request, fixture.env).output.verified).toBe(false);
  });

  test("verifies bytes before a generation-guarded deletion", () => {
    const fixture = makeFixture();
    const created = invoke(createRequest(), fixture.env);
    const command = {
      _tag: "DeleteCheckpoint",
      schema: "openagents.managed_sandbox_phase2_command.v1",
      commandRef: "command.sbx10.driver.delete",
      idempotencyRef: "idempotency.sbx10.driver.delete",
      ownerRef: createCommand.ownerRef,
      tenantRef: createCommand.tenantRef,
      requestedAt: "2099-07-22T03:08:00.000Z",
      checkpointRef: createCommand.checkpointRef,
      reason: "owner_requested",
    };
    const deleted = invoke(
      {
        schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
        action: "delete_checkpoint",
        requestRef: command.commandRef,
        command,
        checkpoint: created.output,
      },
      fixture.env,
    );

    expect(deleted.status).toBe(0);
    expect(deleted.output).toMatchObject({
      checkpointRef: createCommand.checkpointRef,
      contentDeleted: true,
      outcome: "deleted",
    });
    expect(() => readFileSync(join(fixture.root, "object.tar"))).toThrow();
  });

  test("observes the highest authenticated guest generation marker", () => {
    const fixture = makeFixture();
    const result = invoke(
      {
        schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
        action: "observe_resource_generation",
        requestRef: createCommand.sourceSandboxRef,
        ownerRef: createCommand.ownerRef,
        tenantRef: createCommand.tenantRef,
        sandboxRef: createCommand.sourceSandboxRef,
      },
      fixture.env,
    );

    expect(result.status).toBe(0);
    expect(result.output.resourceGeneration).toBe(4);
  });

  test("restores exact bytes with fresh capabilities and no process continuity", () => {
    const fixture = makeFixture();
    const created = invoke(createRequest(), fixture.env);
    const restored = invoke(restoreRequest(created.output), fixture.env);

    expect(restored.status).toBe(0);
    expect(restored.output).toMatchObject({
      checkpointRef: createCommand.checkpointRef,
      sandboxRef: restoreSandboxRef,
      checkpointSourceGeneration: 7,
      restoredResourceGeneration: 8,
      admittedServiceRefs: ["service.agent-runtime"],
      restartedServiceRefs: [],
      sourceCapabilityRefs: ["capability.sbx10.source"],
      grantPolicy: "mint_fresh",
      processSessionContinuity: "discontinuous",
      processMemoryRestored: false,
      ptyRestored: false,
      socketsRestored: false,
      outcome: "restored",
    });
    expect(restored.output.restoredCapabilityRefs).toHaveLength(1);
    expect(restored.output.restoredCapabilityRefs).not.toContain("capability.sbx10.source");
    const calls = readFileSync(join(fixture.root, "calls.jsonl"), "utf8");
    expect(calls).toContain("managed-sandbox-guest-checkpoint.py restore");
    expect(calls).toContain("rm -f");
    expect(calls).not.toContain(restoreSandboxRef);
  });

  test("refuses corrupt bytes before copying them to the destination", () => {
    const fixture = makeFixture();
    const created = invoke(createRequest(), fixture.env);
    writeFileSync(join(fixture.root, "object.tar"), "tampered\n");
    const restored = invoke(restoreRequest(created.output), fixture.env);

    expect(restored.status).not.toBe(0);
    expect(() => readFileSync(join(fixture.root, "guest.tar"))).toThrow();
  });

  test("refuses a restored post-image mismatch and removes guest scratch bytes", () => {
    const fixture = makeFixture();
    const created = invoke(createRequest(), fixture.env);
    const restored = invoke(restoreRequest(created.output), {
      ...fixture.env,
      FAKE_RESTORE_REPOSITORY_POST_IMAGE_DIGEST: digest("wrong-post-image"),
    });

    expect(restored.status).not.toBe(0);
    expect(restored.stdout).toBe("");
    const calls = readFileSync(join(fixture.root, "calls.jsonl"), "utf8");
    expect(calls).toContain("rm -f");
  });

  test("keeps fork and an unprepared restore closed", () => {
    const fixture = makeFixture();
    const result = invoke(
      {
        schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
        action: "fork_from_checkpoint",
        requestRef: "command.sbx10.driver.fork",
      },
      fixture.env,
    );

    expect(result.status).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toBe("");

    const created = invoke(createRequest(), fixture.env);
    const restore = restoreRequest(created.output);
    delete (restore as { runtimeContext?: unknown }).runtimeContext;
    const unpreparedRestore = invoke(restore, fixture.env);
    expect(unpreparedRestore.status).not.toBe(0);
    expect(unpreparedRestore.stdout).toBe("");
  });
});
