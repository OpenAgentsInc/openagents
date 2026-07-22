#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const TARGET_SCHEMA_VERSION = "openagents.managed_sandbox_phase2_target.v1";
const ERROR_SCHEMA_VERSION = "openagents.managed_sandbox_phase2_driver_error.v1";
const CHECKPOINT_SCHEMA_VERSION = "openagents.managed_sandbox_content_checkpoint.v1";
const DELETE_SCHEMA_VERSION = "openagents.managed_sandbox_checkpoint_delete_receipt.v1";
const FORK_SCHEMA_VERSION = "openagents.managed_sandbox_fork_receipt.v1";
const RESTORE_SCHEMA_VERSION = "openagents.managed_sandbox_restore_receipt.v1";
const FORMAT_REF = "format.sbx.content-tar.v1";
const MAX_ARCHIVE_BYTES = 512 * 1024 * 1024;
const MAX_COMMAND_BYTES = 2 * 1024 * 1024;

class DriverError extends Error {}

const fail = () => process.exit(2);
if (process.argv[2] !== "--managed-sandbox-phase2") fail();

const required = (name) => {
  const value = process.env[name]?.trim();
  if (!value) throw new DriverError("configuration_unavailable");
  return value;
};

const sha256Hex = (value) => createHash("sha256").update(value).digest("hex");
const sha256Ref = (value) => `sha256:${sha256Hex(value)}`;
const evidenceRef = (kind, value) => `evidence.sbx10.${kind}.${sha256Hex(value).slice(0, 32)}`;

const canonicalJson = (value) => {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value)
      .toSorted()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const readRequest = () => {
  const bytes = readFileSync(0);
  if (bytes.byteLength === 0 || bytes.byteLength > MAX_COMMAND_BYTES) {
    throw new DriverError("request_invalid");
  }
  let request;
  try {
    request = JSON.parse(bytes.toString("utf8"));
  } catch {
    throw new DriverError("request_invalid");
  }
  if (
    request === null ||
    typeof request !== "object" ||
    request.schemaVersion !== TARGET_SCHEMA_VERSION ||
    typeof request.action !== "string" ||
    typeof request.requestRef !== "string"
  ) {
    throw new DriverError("request_invalid");
  }
  return request;
};

let project;
let zone;
let bucket;
let gcloud;

const gcloudRun = (args, options = {}) => {
  const result = spawnSync(gcloud, args, {
    encoding: "utf8",
    maxBuffer: options.maxBuffer ?? 2 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
    timeout: options.timeout ?? 120_000,
  });
  if (result.status !== 0) throw new DriverError("gcloud_operation_failed");
  return result.stdout;
};

const gcloudStatus = (args, options = {}) =>
  spawnSync(gcloud, args, {
    encoding: "utf8",
    maxBuffer: options.maxBuffer ?? 2 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
    timeout: options.timeout ?? 120_000,
  });

const instanceName = (sandboxRef) => `oa-msb-${sha256Hex(sandboxRef).slice(0, 20)}`;
const generationMarker = (sandboxRef, generation) => {
  const resourceRef = `gce-instance-ref://sha256/${sha256Hex(`resource|${sandboxRef}`)}`;
  const diskRef = `gce-disk-ref://sha256/${sha256Hex(`disk|${sandboxRef}`)}`;
  return sha256Hex(`${resourceRef}|${diskRef}|${generation}`).slice(0, 20);
};
const objectUri = (ownerRef, tenantRef, checkpointRef) =>
  `gs://${bucket}/managed-sandbox-checkpoints/v1/${sha256Hex(`${ownerRef}|${tenantRef}`).slice(
    0,
    32,
  )}/${sha256Hex(checkpointRef)}.tar`;

const remoteArgs = (sandboxRef, command) => [
  "compute",
  "ssh",
  `openagents@${instanceName(sandboxRef)}`,
  "--project",
  project,
  "--zone",
  zone,
  "--internal-ip",
  "--quiet",
  "--ssh-key-expire-after=10m",
  "--ssh-flag=-oStrictHostKeyChecking=no",
  "--ssh-flag=-oUserKnownHostsFile=/dev/null",
  "--command",
  command,
];

const remoteCheckpoint = (requestRef) => {
  const key = sha256Hex(requestRef).slice(0, 32);
  const directory = `/var/lib/openagents/managed-sandbox-checkpoints/${key}`;
  return { directory, archive: `${directory}/content.tar` };
};

const remoteCreate = (sandboxRef, requestRef) => {
  const remote = remoteCheckpoint(requestRef);
  const command =
    `set -eu; install -d -m 0700 ${remote.directory}; ` +
    `/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py ` +
    `create ${remote.archive}`;
  const output = gcloudRun(remoteArgs(sandboxRef, command), {
    maxBuffer: 1024 * 1024,
    timeout: 10 * 60_000,
  });
  let result;
  try {
    result = JSON.parse(output);
  } catch {
    throw new DriverError("guest_checkpoint_invalid");
  }
  return { remote, result };
};

const remoteCleanup = (sandboxRef, remote) => {
  const result = gcloudStatus(
    remoteArgs(sandboxRef, `rm -f ${remote.archive}; rmdir ${remote.directory}`),
  );
  if (result.status !== 0) throw new DriverError("guest_checkpoint_cleanup_failed");
};

const copyFromGuest = (sandboxRef, remoteArchive, localArchive) => {
  gcloudRun(
    [
      "compute",
      "scp",
      `openagents@${instanceName(sandboxRef)}:${remoteArchive}`,
      localArchive,
      "--project",
      project,
      "--zone",
      zone,
      "--internal-ip",
      "--quiet",
      "--ssh-key-expire-after=10m",
      "--scp-flag=-oStrictHostKeyChecking=no",
      "--scp-flag=-oUserKnownHostsFile=/dev/null",
    ],
    { timeout: 5 * 60_000 },
  );
};

const copyToGuest = (sandboxRef, localArchive, remoteArchive) => {
  gcloudRun(
    [
      "compute",
      "scp",
      localArchive,
      `openagents@${instanceName(sandboxRef)}:${remoteArchive}`,
      "--project",
      project,
      "--zone",
      zone,
      "--internal-ip",
      "--quiet",
      "--ssh-key-expire-after=10m",
      "--scp-flag=-oStrictHostKeyChecking=no",
      "--scp-flag=-oUserKnownHostsFile=/dev/null",
    ],
    { timeout: 5 * 60_000 },
  );
};

const remotePrepare = (sandboxRef, remote) => {
  gcloudRun(remoteArgs(sandboxRef, `install -d -m 0700 ${remote.directory}`));
};

const remoteRestore = (sandboxRef, requestRef, contentDigest) => {
  const remote = remoteCheckpoint(requestRef);
  const command =
    `set -eu; install -d -m 0700 ${remote.directory}; ` +
    `/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py ` +
    `restore ${remote.archive} ${contentDigest}`;
  const output = gcloudRun(remoteArgs(sandboxRef, command), {
    maxBuffer: 1024 * 1024,
    timeout: 10 * 60_000,
  });
  try {
    return { remote, result: JSON.parse(output) };
  } catch {
    throw new DriverError("guest_restore_invalid");
  }
};

const hashFile = (path) => {
  const digest = createHash("sha256");
  const descriptor = openSync(path, "r");
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  try {
    let count;
    do {
      count = readSync(descriptor, buffer, 0, buffer.byteLength, null);
      if (count > 0) digest.update(buffer.subarray(0, count));
    } while (count > 0);
  } finally {
    closeSync(descriptor);
  }
  return `sha256:${digest.digest("hex")}`;
};

const describeObject = (uri) => {
  const result = gcloudStatus(["storage", "objects", "describe", uri, "--format=json"]);
  if (result.status !== 0) return undefined;
  try {
    const value = JSON.parse(result.stdout);
    if (value === null || typeof value !== "object") return undefined;
    const metadata = value.custom_fields ?? value.metadata ?? {};
    if (metadata === null || typeof metadata !== "object" || Array.isArray(metadata)) {
      throw new DriverError("checkpoint_object_metadata_invalid");
    }
    return { ...value, metadata };
  } catch {
    throw new DriverError("checkpoint_object_metadata_invalid");
  }
};

const downloadObject = (uri, path) => {
  const object = describeObject(uri);
  const advertisedSize = Number(object?.size);
  if (
    !Number.isSafeInteger(advertisedSize) ||
    advertisedSize < 0 ||
    advertisedSize > MAX_ARCHIVE_BYTES
  ) {
    throw new DriverError("checkpoint_object_size_invalid");
  }
  gcloudRun(["storage", "cp", uri, path, "--quiet"], {
    timeout: 5 * 60_000,
  });
  const size = statSync(path).size;
  if (size > MAX_ARCHIVE_BYTES) throw new DriverError("checkpoint_object_too_large");
  return { contentBytes: size, contentDigest: hashFile(path) };
};

const checkpointResult = (command, content, completedAt, verifiedAt) => ({
  schema: CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: command.checkpointRef,
  ownerRef: command.ownerRef,
  tenantRef: command.tenantRef,
  sourceSandboxRef: command.sourceSandboxRef,
  sourceResourceGeneration: command.sourceResourceGeneration,
  sourceImageDigest: command.sourceImageDigest,
  sourceToolchainDigest: command.sourceToolchainDigest,
  repositoryRef: command.repositoryRef,
  repositoryRevisionRef: command.repositoryRevisionRef,
  repositoryPostImageDigest: command.repositoryPostImageDigest,
  contentDigest: content.contentDigest,
  contentBytes: content.contentBytes,
  formatRef: command.formatRef,
  state: "completed",
  completedAt,
  verifiedAt,
  retainedUntil: command.retainedUntil,
  deleteOnExpiry: true,
  omissions: {
    credentials: "excluded",
    accountSecrets: "excluded",
    providerHiddenState: "excluded",
    processMemory: "excluded",
    processTable: "excluded",
    ptyState: "excluded",
    sockets: "excluded",
    ports: "excluded",
    networkIdentity: "excluded",
  },
  evidenceRefs: [evidenceRef("checkpoint.object", command.checkpointRef)],
});

const validateCreate = (request) => {
  const command = request.command;
  if (
    command?.["_tag"] !== "CreateCheckpoint" ||
    command.commandRef !== request.requestRef ||
    command.formatRef !== FORMAT_REF ||
    typeof command.ownerRef !== "string" ||
    typeof command.tenantRef !== "string" ||
    typeof command.checkpointRef !== "string" ||
    typeof command.sourceSandboxRef !== "string" ||
    !Number.isSafeInteger(command.sourceResourceGeneration) ||
    typeof command.repositoryPostImageDigest !== "string" ||
    typeof command.retainedUntil !== "string" ||
    typeof command.requestedAt !== "string"
  ) {
    throw new DriverError("create_request_invalid");
  }
  return command;
};

const verifiedLocalContent = (path, expectedDigest, expectedBytes) => {
  const contentBytes = statSync(path).size;
  const contentDigest = hashFile(path);
  if (contentBytes !== expectedBytes || contentDigest !== expectedDigest) {
    throw new DriverError("checkpoint_content_mismatch");
  }
  return { contentBytes, contentDigest };
};

const createCheckpoint = (request) => {
  const command = validateCreate(request);
  const retainedAt = Date.parse(command.retainedUntil);
  const requestedAt = Date.parse(command.requestedAt);
  const observedAt = Math.max(Date.now(), requestedAt);
  if (!Number.isFinite(retainedAt) || retainedAt <= observedAt) {
    throw new DriverError("checkpoint_retention_expired");
  }
  const completedAt = new Date(observedAt).toISOString();
  const verifiedAt = completedAt;
  const fingerprint = sha256Ref(canonicalJson(command));
  const uri = objectUri(command.ownerRef, command.tenantRef, command.checkpointRef);
  const local = mkdtempSync(join(tmpdir(), "oa-msb-phase2-create-"));
  const localArchive = join(local, "content.tar");
  let remote;
  try {
    const existing = describeObject(uri);
    if (existing !== undefined) {
      const metadata = existing.metadata ?? {};
      if (metadata.oa_command_fingerprint !== fingerprint) {
        throw new DriverError("checkpoint_idempotency_conflict");
      }
      const content = downloadObject(uri, localArchive);
      if (
        content.contentDigest !== metadata.oa_content_digest ||
        String(content.contentBytes) !== metadata.oa_content_bytes ||
        metadata.oa_repository_post_image_digest !== command.repositoryPostImageDigest ||
        metadata.oa_retained_until !== command.retainedUntil ||
        typeof metadata.oa_completed_at !== "string" ||
        typeof metadata.oa_verified_at !== "string"
      ) {
        throw new DriverError("checkpoint_object_corrupt");
      }
      return checkpointResult(command, content, metadata.oa_completed_at, metadata.oa_verified_at);
    }

    const created = remoteCreate(command.sourceSandboxRef, request.requestRef);
    remote = created.remote;
    if (
      created.result?.formatRef !== FORMAT_REF ||
      created.result.repositoryPostImageDigest !== command.repositoryPostImageDigest ||
      typeof created.result.contentDigest !== "string" ||
      !Number.isSafeInteger(created.result.contentBytes) ||
      created.result.contentBytes < 0 ||
      created.result.contentBytes > MAX_ARCHIVE_BYTES
    ) {
      throw new DriverError("guest_checkpoint_scope_conflict");
    }
    copyFromGuest(command.sourceSandboxRef, remote.archive, localArchive);
    const content = verifiedLocalContent(
      localArchive,
      created.result.contentDigest,
      created.result.contentBytes,
    );
    const metadata = [
      `oa_command_fingerprint=${fingerprint}`,
      `oa_content_digest=${content.contentDigest}`,
      `oa_content_bytes=${content.contentBytes}`,
      `oa_repository_post_image_digest=${command.repositoryPostImageDigest}`,
      `oa_completed_at=${completedAt}`,
      `oa_verified_at=${verifiedAt}`,
      `oa_retained_until=${command.retainedUntil}`,
    ].join(",");
    const upload = gcloudStatus(
      [
        "storage",
        "cp",
        localArchive,
        uri,
        "--if-generation-match=0",
        `--custom-metadata=${metadata}`,
        "--quiet",
      ],
      { timeout: 5 * 60_000 },
    );
    if (upload.status !== 0) {
      const raced = describeObject(uri);
      if (raced?.metadata?.oa_command_fingerprint !== fingerprint) {
        throw new DriverError("checkpoint_idempotency_conflict");
      }
    }
    const stored = describeObject(uri);
    const storedMetadata = stored?.metadata ?? {};
    if (
      storedMetadata.oa_command_fingerprint !== fingerprint ||
      storedMetadata.oa_content_digest !== content.contentDigest ||
      storedMetadata.oa_content_bytes !== String(content.contentBytes) ||
      storedMetadata.oa_repository_post_image_digest !== command.repositoryPostImageDigest ||
      storedMetadata.oa_retained_until !== command.retainedUntil ||
      typeof storedMetadata.oa_completed_at !== "string" ||
      typeof storedMetadata.oa_verified_at !== "string"
    ) {
      throw new DriverError("checkpoint_object_metadata_invalid");
    }
    const readbackPath = join(local, "readback.tar");
    const readback = downloadObject(uri, readbackPath);
    if (
      readback.contentDigest !== content.contentDigest ||
      readback.contentBytes !== content.contentBytes
    ) {
      throw new DriverError("checkpoint_readback_failed");
    }
    return checkpointResult(
      command,
      content,
      storedMetadata.oa_completed_at,
      storedMetadata.oa_verified_at,
    );
  } finally {
    if (remote !== undefined) remoteCleanup(command.sourceSandboxRef, remote);
    rmSync(local, { recursive: true, force: true });
  }
};

const validateCheckpoint = (request) => {
  const checkpoint = request.checkpoint;
  if (
    checkpoint?.schema !== CHECKPOINT_SCHEMA_VERSION ||
    checkpoint.checkpointRef !== request.requestRef ||
    checkpoint.formatRef !== FORMAT_REF ||
    typeof checkpoint.ownerRef !== "string" ||
    typeof checkpoint.tenantRef !== "string" ||
    typeof checkpoint.contentDigest !== "string" ||
    !Number.isSafeInteger(checkpoint.contentBytes) ||
    checkpoint.contentBytes < 0 ||
    checkpoint.contentBytes > MAX_ARCHIVE_BYTES
  ) {
    throw new DriverError("checkpoint_request_invalid");
  }
  return checkpoint;
};

const verifyCheckpoint = (request) => {
  const checkpoint = validateCheckpoint(request);
  const uri = objectUri(checkpoint.ownerRef, checkpoint.tenantRef, checkpoint.checkpointRef);
  if (describeObject(uri) === undefined) throw new DriverError("checkpoint_object_missing");
  const local = mkdtempSync(join(tmpdir(), "oa-msb-phase2-verify-"));
  try {
    const content = downloadObject(uri, join(local, "content.tar"));
    const verified =
      content.contentDigest === checkpoint.contentDigest &&
      content.contentBytes === checkpoint.contentBytes;
    return {
      verified,
      checkpointRef: checkpoint.checkpointRef,
      contentDigest: checkpoint.contentDigest,
      evidenceRefs: [evidenceRef("checkpoint.readback", checkpoint.checkpointRef)],
    };
  } finally {
    rmSync(local, { recursive: true, force: true });
  }
};

const validateRestore = (request) => {
  const command = request.command;
  const checkpoint = request.checkpoint;
  const runtime = request.runtimeContext;
  if (
    command?.["_tag"] !== "RestoreCheckpoint" ||
    command.commandRef !== request.requestRef ||
    checkpoint?.schema !== CHECKPOINT_SCHEMA_VERSION ||
    command.ownerRef !== checkpoint.ownerRef ||
    command.tenantRef !== checkpoint.tenantRef ||
    command.checkpointRef !== checkpoint.checkpointRef ||
    command.expectedSourceResourceGeneration !== checkpoint.sourceResourceGeneration ||
    typeof command.destinationSandboxRef !== "string" ||
    !Array.isArray(command.admittedServiceRefs) ||
    !Array.isArray(command.sourceCapabilityRefs) ||
    runtime?.schema !== "openagents.managed_sandbox_phase2_restore_context.v1" ||
    runtime.ownerRef !== command.ownerRef ||
    runtime.tenantRef !== command.tenantRef ||
    runtime.sandboxRef !== command.destinationSandboxRef ||
    !Number.isSafeInteger(runtime.resourceGeneration) ||
    runtime.resourceGeneration <= checkpoint.sourceResourceGeneration ||
    !Array.isArray(runtime.restoredCapabilityRefs) ||
    runtime.restoredCapabilityRefs.length === 0 ||
    runtime.restoredCapabilityRefs.some(
      (capabilityRef) =>
        typeof capabilityRef !== "string" || command.sourceCapabilityRefs.includes(capabilityRef),
    ) ||
    typeof checkpoint.contentDigest !== "string" ||
    !Number.isSafeInteger(checkpoint.contentBytes) ||
    checkpoint.contentBytes < 0 ||
    checkpoint.contentBytes > MAX_ARCHIVE_BYTES
  ) {
    throw new DriverError("restore_request_invalid");
  }
  return { checkpoint, command, runtime };
};

const observeGeneration = (sandboxRef) => {
  const serial = gcloudRun(
    [
      "compute",
      "instances",
      "get-serial-port-output",
      instanceName(sandboxRef),
      "--project",
      project,
      "--zone",
      zone,
      "--port",
      "1",
    ],
    { maxBuffer: 4 * 1024 * 1024 },
  );
  const generations = Array.from(
    serial.matchAll(/OA_MSB_(?:READY|PROBE):([a-f0-9]{20}):([0-9]+)/gu),
    (match) => ({ generation: Number(match[2]), marker: match[1] }),
  )
    .filter(
      ({ generation, marker }) =>
        Number.isSafeInteger(generation) &&
        generation >= 0 &&
        marker === generationMarker(sandboxRef, generation),
    )
    .map(({ generation }) => generation);
  if (generations.length === 0) throw new DriverError("generation_unavailable");
  return Math.max(...generations);
};

const restoreCheckpoint = (request) => {
  const { checkpoint, command, runtime } = validateRestore(request);
  const uri = objectUri(checkpoint.ownerRef, checkpoint.tenantRef, checkpoint.checkpointRef);
  const local = mkdtempSync(join(tmpdir(), "oa-msb-phase2-restore-"));
  const localArchive = join(local, "content.tar");
  let remote;
  try {
    const content = downloadObject(uri, localArchive);
    if (
      content.contentDigest !== checkpoint.contentDigest ||
      content.contentBytes !== checkpoint.contentBytes
    ) {
      throw new DriverError("checkpoint_object_corrupt");
    }
    remote = remoteCheckpoint(request.requestRef);
    remotePrepare(command.destinationSandboxRef, remote);
    copyToGuest(command.destinationSandboxRef, localArchive, remote.archive);
    const restored = remoteRestore(
      command.destinationSandboxRef,
      request.requestRef,
      checkpoint.contentDigest,
    );
    remote = restored.remote;
    if (
      restored.result?.formatRef !== FORMAT_REF ||
      restored.result.contentDigest !== checkpoint.contentDigest ||
      restored.result.contentBytes !== checkpoint.contentBytes ||
      restored.result.repositoryPostImageDigest !== checkpoint.repositoryPostImageDigest
    ) {
      throw new DriverError("guest_restore_scope_conflict");
    }
    const restoredResourceGeneration = observeGeneration(command.destinationSandboxRef);
    if (restoredResourceGeneration !== runtime.resourceGeneration) {
      throw new DriverError("restore_generation_conflict");
    }
    return {
      schema: RESTORE_SCHEMA_VERSION,
      receiptRef: evidenceRef("restore", command.commandRef),
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      checkpointRef: command.checkpointRef,
      sandboxRef: command.destinationSandboxRef,
      checkpointSourceGeneration: checkpoint.sourceResourceGeneration,
      restoredResourceGeneration,
      admittedServiceRefs: command.admittedServiceRefs,
      restartedServiceRefs: [],
      sourceCapabilityRefs: command.sourceCapabilityRefs,
      restoredCapabilityRefs: runtime.restoredCapabilityRefs,
      grantPolicy: "mint_fresh",
      processSessionContinuity: "discontinuous",
      processMemoryRestored: false,
      ptyRestored: false,
      socketsRestored: false,
      outcome: "restored",
      observedAt: new Date().toISOString(),
      evidenceRefs: [evidenceRef("restore.readback", command.commandRef)],
    };
  } finally {
    if (remote !== undefined) remoteCleanup(command.destinationSandboxRef, remote);
    rmSync(local, { recursive: true, force: true });
  }
};

const validateFork = (request) => {
  const command = request.command;
  const checkpoint = request.checkpoint;
  const runtime = request.runtimeContext;
  if (
    command?.["_tag"] !== "ForkFromCheckpoint" ||
    command.commandRef !== request.requestRef ||
    checkpoint?.schema !== CHECKPOINT_SCHEMA_VERSION ||
    command.ownerRef !== checkpoint.ownerRef ||
    command.tenantRef !== checkpoint.tenantRef ||
    command.checkpointRef !== checkpoint.checkpointRef ||
    command.expectedSourceSandboxRef !== checkpoint.sourceSandboxRef ||
    command.expectedSourceResourceGeneration !== checkpoint.sourceResourceGeneration ||
    !Array.isArray(command.sourceCapabilityRefs) ||
    runtime?.schema !== "openagents.managed_sandbox_phase2_fork_context.v1" ||
    runtime.ownerRef !== command.ownerRef ||
    runtime.tenantRef !== command.tenantRef ||
    runtime.sourceSandboxRef !== checkpoint.sourceSandboxRef ||
    runtime.sourceResourceGeneration !== checkpoint.sourceResourceGeneration ||
    typeof runtime.forkSandboxRef !== "string" ||
    runtime.forkSandboxRef === checkpoint.sourceSandboxRef ||
    runtime.forkResourceGeneration !== 1 ||
    !Array.isArray(runtime.forkCapabilityRefs) ||
    runtime.forkCapabilityRefs.length === 0 ||
    runtime.forkCapabilityRefs.some(
      (capabilityRef) =>
        typeof capabilityRef !== "string" || command.sourceCapabilityRefs.includes(capabilityRef),
    ) ||
    typeof runtime.cleanupObligationRef !== "string" ||
    typeof checkpoint.contentDigest !== "string" ||
    !Number.isSafeInteger(checkpoint.contentBytes) ||
    checkpoint.contentBytes < 0 ||
    checkpoint.contentBytes > MAX_ARCHIVE_BYTES
  ) {
    throw new DriverError("fork_request_invalid");
  }
  return { checkpoint, command, runtime };
};

const forkFromCheckpoint = (request) => {
  const { checkpoint, command, runtime } = validateFork(request);
  const uri = objectUri(checkpoint.ownerRef, checkpoint.tenantRef, checkpoint.checkpointRef);
  const local = mkdtempSync(join(tmpdir(), "oa-msb-phase2-fork-"));
  const localArchive = join(local, "content.tar");
  let remote;
  try {
    const content = downloadObject(uri, localArchive);
    if (
      content.contentDigest !== checkpoint.contentDigest ||
      content.contentBytes !== checkpoint.contentBytes
    ) {
      throw new DriverError("checkpoint_object_corrupt");
    }
    remote = remoteCheckpoint(request.requestRef);
    remotePrepare(runtime.forkSandboxRef, remote);
    copyToGuest(runtime.forkSandboxRef, localArchive, remote.archive);
    const restored = remoteRestore(
      runtime.forkSandboxRef,
      request.requestRef,
      checkpoint.contentDigest,
    );
    remote = restored.remote;
    if (
      restored.result?.formatRef !== FORMAT_REF ||
      restored.result.contentDigest !== checkpoint.contentDigest ||
      restored.result.contentBytes !== checkpoint.contentBytes ||
      restored.result.repositoryPostImageDigest !== checkpoint.repositoryPostImageDigest
    ) {
      throw new DriverError("guest_restore_scope_conflict");
    }
    const forkResourceGeneration = observeGeneration(runtime.forkSandboxRef);
    if (forkResourceGeneration !== runtime.forkResourceGeneration) {
      throw new DriverError("fork_generation_conflict");
    }
    return {
      schema: FORK_SCHEMA_VERSION,
      receiptRef: evidenceRef("fork", command.commandRef),
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      checkpointRef: command.checkpointRef,
      sourceSandboxRef: checkpoint.sourceSandboxRef,
      sourceResourceGeneration: checkpoint.sourceResourceGeneration,
      forkSandboxRef: runtime.forkSandboxRef,
      forkResourceGeneration,
      sourceCapabilityRefs: command.sourceCapabilityRefs,
      forkCapabilityRefs: runtime.forkCapabilityRefs,
      grantPolicy: "mint_fresh",
      cleanupObligationRef: runtime.cleanupObligationRef,
      stateTransfer: {
        credentials: "excluded",
        accountSecrets: "excluded",
        providerHiddenState: "excluded",
        processMemory: "excluded",
        processTable: "excluded",
        ptyState: "excluded",
        sockets: "excluded",
        ports: "excluded",
        networkIdentity: "excluded",
      },
      processSessionContinuity: "none",
      outcome: "created",
      observedAt: new Date().toISOString(),
      evidenceRefs: [evidenceRef("fork.readback", command.commandRef)],
    };
  } finally {
    if (remote !== undefined) remoteCleanup(runtime.forkSandboxRef, remote);
    rmSync(local, { recursive: true, force: true });
  }
};

const deleteCheckpoint = (request) => {
  const command = request.command;
  const checkpoint = request.checkpoint;
  if (
    command?.["_tag"] !== "DeleteCheckpoint" ||
    command.commandRef !== request.requestRef ||
    checkpoint?.schema !== CHECKPOINT_SCHEMA_VERSION ||
    command.ownerRef !== checkpoint.ownerRef ||
    command.tenantRef !== checkpoint.tenantRef ||
    command.checkpointRef !== checkpoint.checkpointRef
  ) {
    throw new DriverError("delete_request_invalid");
  }
  const uri = objectUri(checkpoint.ownerRef, checkpoint.tenantRef, checkpoint.checkpointRef);
  const object = describeObject(uri);
  if (object === undefined || typeof object.generation !== "string") {
    throw new DriverError("checkpoint_object_missing");
  }
  const local = mkdtempSync(join(tmpdir(), "oa-msb-phase2-delete-"));
  try {
    const content = downloadObject(uri, join(local, "content.tar"));
    if (
      content.contentDigest !== checkpoint.contentDigest ||
      content.contentBytes !== checkpoint.contentBytes
    ) {
      throw new DriverError("checkpoint_object_corrupt");
    }
    gcloudRun(["storage", "rm", uri, `--if-generation-match=${object.generation}`, "--quiet"]);
    if (describeObject(uri) !== undefined) {
      throw new DriverError("checkpoint_delete_unverified");
    }
    return {
      schema: DELETE_SCHEMA_VERSION,
      receiptRef: evidenceRef("checkpoint.delete", command.commandRef),
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      checkpointRef: command.checkpointRef,
      sourceSandboxRef: checkpoint.sourceSandboxRef,
      sourceResourceGeneration: checkpoint.sourceResourceGeneration,
      contentDigest: checkpoint.contentDigest,
      contentDeleted: true,
      outcome: "deleted",
      reason: command.reason,
      deletedAt: new Date().toISOString(),
      evidenceRefs: [evidenceRef("checkpoint.object.delete", command.checkpointRef)],
    };
  } finally {
    rmSync(local, { recursive: true, force: true });
  }
};

const observeResourceGeneration = (request) => {
  if (
    typeof request.ownerRef !== "string" ||
    typeof request.tenantRef !== "string" ||
    typeof request.sandboxRef !== "string" ||
    request.requestRef !== request.sandboxRef
  ) {
    throw new DriverError("generation_request_invalid");
  }
  return {
    ownerRef: request.ownerRef,
    tenantRef: request.tenantRef,
    sandboxRef: request.sandboxRef,
    resourceGeneration: observeGeneration(request.sandboxRef),
    evidenceRefs: [evidenceRef("sandbox.generation", request.sandboxRef)],
  };
};

const execute = (request) => {
  switch (request.action) {
    case "create_checkpoint":
      return createCheckpoint(request);
    case "verify_checkpoint":
      return verifyCheckpoint(request);
    case "observe_resource_generation":
      return observeResourceGeneration(request);
    case "fork_from_checkpoint":
      return forkFromCheckpoint(request);
    case "restore_checkpoint":
      return restoreCheckpoint(request);
    case "delete_checkpoint":
      return deleteCheckpoint(request);
    default:
      throw new DriverError("phase2_action_not_integrated");
  }
};

try {
  project = required("OA_MANAGED_SANDBOX_PROJECT_ID");
  zone = required("OA_MANAGED_SANDBOX_ZONE");
  bucket = required("OA_MANAGED_SANDBOX_PHASE2_BUCKET");
  gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
  if (!/^[a-z0-9][a-z0-9._-]{1,220}[a-z0-9]$/u.test(bucket)) {
    throw new DriverError("bucket_invalid");
  }
  const request = readRequest();
  const result = execute(request);
  process.stdout.write(
    JSON.stringify({
      schemaVersion: TARGET_SCHEMA_VERSION,
      action: request.action,
      requestRef: request.requestRef,
      result,
    }),
  );
  process.stdout.write("\n");
} catch (error) {
  const reasonRef =
    error instanceof DriverError && /^[a-z0-9_]{1,80}$/u.test(error.message)
      ? error.message
      : "internal_driver_failure";
  process.stdout.write(`${JSON.stringify({ schemaVersion: ERROR_SCHEMA_VERSION, reasonRef })}\n`);
  process.exitCode = 2;
}
