#!/usr/bin/env -S pnpm exec tsx

import { BoxApi, Configuration, ResponseError } from "@asciidev/box-sdk";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

type PromptProvider = "codex" | "claude";

type Residue = Readonly<{
  compute: number;
  firewall: number;
  scratch: number;
  ingress: number;
  grants: number;
}>;

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value.length === 0) {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
};

const sha256 = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");

const sleep = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const parseArgs = (): { apply: boolean; evidence: string } => {
  const args = process.argv.slice(2);
  let apply = false;
  let evidence = resolve("artifacts/managed-sandbox-sbx09-box-live.json");
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      apply = true;
    } else if (argument === "--evidence" && args[index + 1] !== undefined) {
      evidence = resolve(args[index + 1]!);
      index += 1;
    } else {
      throw new Error("usage: managed-sandbox-box-live-acceptance.ts --apply [--evidence PATH]");
    }
  }
  return { apply, evidence };
};

const { apply, evidence } = parseArgs();
if (!apply || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live acceptance is default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const basePath = required("OA_MANAGED_SANDBOX_BOX_BASE_URL").replace(/\/$/, "");
const accessToken = required("OA_MANAGED_SANDBOX_BOX_TOKEN");
const foreignToken = process.env.OA_MANAGED_SANDBOX_BOX_FOREIGN_TOKEN?.trim();
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const profileDigest = required("OA_MANAGED_SANDBOX_PROFILE_DIGEST");
const sourceRevision = required("OA_MANAGED_SANDBOX_SOURCE_REVISION");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const stamp = `${Date.now()}-${process.pid}`;
const keyPrefix = `sbx09-${sha256(stamp).slice(0, 16)}`;

const apiFor = (token: string) => new BoxApi(new Configuration({ basePath, accessToken: token }));

const api = apiFor(accessToken);
const retryHeaders =
  (operation: string) =>
  async ({ init }: { init: RequestInit }): Promise<RequestInit> => ({
    ...init,
    headers: {
      ...(init.headers as Record<string, string>),
      "idempotency-key": `${keyPrefix}-${operation}`,
    },
  });

const responseError = async (
  operation: Promise<unknown>,
  expectedStatus: number,
  expectedCode: string,
): Promise<void> => {
  const caught = await operation.then(() => undefined).catch((error) => error);
  if (!(caught instanceof ResponseError)) {
    throw new Error(`expected ResponseError ${expectedStatus}/${expectedCode}`);
  }
  const body = (await caught.response.json()) as { code?: string };
  if (caught.response.status !== expectedStatus || body.code !== expectedCode) {
    throw new Error(`unexpected Box error ${caught.response.status}/${body.code ?? "unknown"}`);
  }
};

const createWithReplay = async () => {
  let lastError: unknown;
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    try {
      return await api.create(
        { createBoxRequest: { ttlSeconds: 900, noEnv: true } },
        retryHeaders("create"),
      );
    } catch (error) {
      lastError = error;
      if (attempt < 2) await sleep(5_000);
    }
  }
  throw lastError;
};

const pollPrompt = async (boxId: string, promptId: string) => {
  for (let attempt = 1; attempt <= 180; attempt += 1) {
    const result = await api.promptRunStatus({ boxId, promptId });
    if (result.promptRun.done) return result.promptRun;
    await sleep(3_000);
  }
  throw new Error(`prompt ${promptId} did not structurally settle`);
};

const count = (collection: "instances" | "firewall-rules" | "disks", filter: string): number => {
  const args = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") args.push("--zones", zone);
  args.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return output.split("\n").filter((line) => line.trim().length > 0).length;
};

const observeResidue = (suffix: string): Residue => {
  const firewallNames = [
    `oa-msb-egress-${suffix}`,
    `oa-msb-broker-${suffix}`,
    `oa-msb-metadata-${suffix}`,
    `oa-msb-ssh-${suffix}`,
    `oa-msb-ingress-${suffix}`,
  ];
  return {
    compute: count("instances", `name=oa-msb-${suffix}`),
    firewall: firewallNames.reduce(
      (total, name) => total + count("firewall-rules", `name=${name}`),
      0,
    ),
    scratch: count("disks", `name=oa-msb-${suffix}`),
    ingress: firewallNames
      .filter((name) => name.includes("-ssh-") || name.includes("-ingress-"))
      .reduce((total, name) => total + count("firewall-rules", `name=${name}`), 0),
    grants: 0,
  };
};

const emergencyCleanup = (suffix: string): void => {
  const run = (args: ReadonlyArray<string>) => {
    try {
      execFileSync(gcloud, [...args], { stdio: "ignore" });
    } catch {
      // The independent post-cleanup residue oracle remains authoritative.
    }
  };
  run([
    "compute",
    "instances",
    "delete",
    `oa-msb-${suffix}`,
    "--project",
    projectId,
    "--zone",
    zone,
    "--quiet",
  ]);
  run([
    "compute",
    "firewall-rules",
    "delete",
    `oa-msb-egress-${suffix}`,
    `oa-msb-broker-${suffix}`,
    `oa-msb-metadata-${suffix}`,
    `oa-msb-ssh-${suffix}`,
    `oa-msb-ingress-${suffix}`,
    "--project",
    projectId,
    "--quiet",
  ]);
  run([
    "compute",
    "disks",
    "delete",
    `oa-msb-${suffix}`,
    "--project",
    projectId,
    "--zone",
    zone,
    "--quiet",
  ]);
};

const sdkEvidencePath = resolve(
  import.meta.dirname,
  "../../../../../docs/sol/evidence/2026-07-19-sbx03-box-v1-conformance.json",
);
const sdkEvidence = JSON.parse(readFileSync(sdkEvidencePath, "utf8")) as {
  sdk: {
    package: string;
    version: string;
    license: string;
    integrity: string;
    tarballSha256: string;
  };
  openApiSha256: string;
  translatorRef: string;
};

let boxId: string | undefined;
let resourceSuffix: string | undefined;
let passed = false;
let failure: string | undefined;
let deleted = false;
let emergencyCleanupAttempted = false;
let preEmergencyResidue: Residue = {
  compute: 0,
  firewall: 0,
  scratch: 0,
  ingress: 0,
  grants: 0,
};
let residue: Residue = preEmergencyResidue;
const proof = {
  authentication: false,
  invalidTokenDenied: false,
  crossOwnerDenied: false,
  createReplay: false,
  updateReplay: false,
  fileRoundTrip: false,
  command: false,
  artifact: false,
  unsupportedTyped501: false,
  codexStructuralCompletion: false,
  claudeStructuralCompletion: false,
  orderedReconnect: false,
  interruptReplay: false,
  stopReplay: false,
  resumeReplay: false,
  staleGenerationCursorDenied: false,
  resumePersistence: false,
  deleteReplay: false,
};
const providerEvents: Record<PromptProvider, ReadonlyArray<string>> = {
  codex: [],
  claude: [],
};
let artifactDigest: string | undefined;
let nativeEventCount = 0;
let maximumNativeEventSequence = 0;

try {
  const me = await api.me();
  const limits = await api.limits();
  if (me.user.login === undefined || me.user.login.length === 0 || !limits.canStart) {
    throw new Error("programmatic principal is not admitted to start a sandbox");
  }
  proof.authentication = true;

  await responseError(apiFor(`${keyPrefix}-invalid`).me(), 401, "authentication_required");
  proof.invalidTokenDenied = true;

  const created = await createWithReplay();
  boxId = created.box.id;
  resourceSuffix = sha256(boxId).slice(0, 20);
  if (created.box.state !== "ready") {
    throw new Error(`create settled as ${created.box.state}`);
  }
  const createReplay = await api.create(
    { createBoxRequest: { ttlSeconds: 900, noEnv: true } },
    retryHeaders("create"),
  );
  if (createReplay.box.id !== boxId) throw new Error("create replay diverged");
  proof.createReplay = true;

  if (foreignToken !== undefined && foreignToken.length > 0) {
    await responseError(apiFor(foreignToken).get({ boxId }), 403, "permission_denied");
    proof.crossOwnerDenied = true;
  }

  const updated = await api.update(
    { boxId, updateBoxRequest: { ttlSeconds: 840 } },
    retryHeaders("update"),
  );
  const updateReplay = await api.update(
    { boxId, updateBoxRequest: { ttlSeconds: 840 } },
    retryHeaders("update"),
  );
  if (updated.box.updatedAt !== updateReplay.box.updatedAt) {
    throw new Error("update replay diverged");
  }
  proof.updateReplay = true;

  const content = `managed-sandbox-live-${sha256(stamp).slice(0, 20)}`;
  const written = await api.writeFile({
    boxId,
    fileWriteRequest: { path: "workspace/sbx09-live.txt", content },
  });
  const read = await api.readFile({
    boxId,
    path: "workspace/sbx09-live.txt",
  });
  if (read.content !== content || written.size !== content.length) {
    throw new Error("guest file round trip diverged");
  }
  proof.fileRoundTrip = true;

  const command = await api.command({
    boxId,
    commandRequest: { command: "pwd" },
  });
  if (!command.success || command.exitCode !== 0 || command.stdout.trim() !== "workspace") {
    throw new Error("bounded guest command failed");
  }
  proof.command = true;

  const artifact = await api.artifact({
    boxId,
    path: "workspace/sbx09-live.txt",
  });
  const artifactBytes = new Uint8Array(await artifact.arrayBuffer());
  if (new TextDecoder().decode(artifactBytes) !== content) {
    throw new Error("artifact bytes diverged from the guest file");
  }
  artifactDigest = `sha256:${sha256(artifactBytes)}`;
  proof.artifact = true;

  await responseError(api.fork({ boxId }), 501, "capability_not_implemented");
  proof.unsupportedTyped501 = true;

  for (const provider of ["codex", "claude"] as const) {
    const queued = await api.prompt(
      {
        boxId,
        promptRequest: {
          provider,
          prompt: `Reply with exactly OPENAGENTS_${provider.toUpperCase()}_SBX09_OK.`,
        },
      },
      retryHeaders(`prompt-${provider}`),
    );
    const terminal = await pollPrompt(boxId, queued.promptId);
    if (terminal.status !== "finished") {
      throw new Error(`${provider} turn settled as ${terminal.status}`);
    }
    proof[`${provider}StructuralCompletion`] = true;
    const page = await api.events({ boxId, limit: 100, sort: "asc" });
    providerEvents[provider] = page.events
      .filter((event) => event.taskId === queued.promptId)
      .map((event) => event.type);
  }

  const firstPage = await api.events({ boxId, limit: 1, sort: "asc" });
  const replayedFirstPage = await api.events({ boxId, limit: 1, sort: "asc" });
  if (JSON.stringify(firstPage) !== JSON.stringify(replayedFirstPage)) {
    throw new Error("event reconnect replay diverged");
  }
  const staleCursor = firstPage.pageInfo?.nextCursor;
  if (staleCursor === undefined || staleCursor === null) {
    throw new Error("event stream did not return a resumable cursor");
  }
  let cursor: string | null | undefined = null;
  do {
    const page = await api.events({ boxId, limit: 20, sort: "asc", cursor });
    for (const event of page.events) {
      const sequence = (event.data as { nativeEventSequence?: number } | undefined)
        ?.nativeEventSequence;
      if (sequence !== undefined) {
        nativeEventCount += 1;
        if (sequence <= maximumNativeEventSequence) {
          throw new Error("native event sequence was not strictly ascending");
        }
        maximumNativeEventSequence = sequence;
      }
    }
    cursor = page.pageInfo?.nextCursor;
  } while (cursor !== null && cursor !== undefined);
  proof.orderedReconnect = true;

  const interruptible = await api.prompt(
    {
      boxId,
      promptRequest: {
        provider: "codex",
        prompt: "Use the shell tool to run sleep 120, then reply OPENAGENTS_INTERRUPT_MISSED.",
      },
    },
    retryHeaders("prompt-interrupt"),
  );
  await sleep(2_000);
  const interrupted = await api.interrupt({ boxId }, retryHeaders("interrupt"));
  const interruptReplay = await api.interrupt({ boxId }, retryHeaders("interrupt"));
  if (interrupted.status !== interruptReplay.status) {
    throw new Error("interrupt replay diverged");
  }
  const interruptedTerminal = await pollPrompt(boxId, interruptible.promptId);
  if (interruptedTerminal.status !== "failed") {
    throw new Error("interrupted turn did not settle as failed");
  }
  proof.interruptReplay = true;

  const stopped = await api.stop({ boxId }, retryHeaders("stop"));
  const stopReplay = await api.stop({ boxId }, retryHeaders("stop"));
  if (stopped.box?.state !== "archived" || stopReplay.box?.state !== "archived") {
    throw new Error("stop did not settle as archived");
  }
  proof.stopReplay = true;

  const resumed = await api.resume({ boxId }, retryHeaders("resume"));
  const resumeReplay = await api.resume({ boxId }, retryHeaders("resume"));
  if (resumed.box?.state !== "ready" || resumeReplay.box?.state !== "ready") {
    throw new Error("resume did not settle as ready");
  }
  proof.resumeReplay = true;

  await responseError(api.events({ boxId, cursor: staleCursor, sort: "asc" }), 409, "conflict");
  proof.staleGenerationCursorDenied = true;

  const persisted = await api.readFile({
    boxId,
    path: "workspace/sbx09-live.txt",
  });
  if (persisted.content !== content) {
    throw new Error("guest file did not persist through stop/resume");
  }
  proof.resumePersistence = true;

  const removed = await api.remove({ boxId }, retryHeaders("delete"));
  const removeReplay = await api.remove({ boxId }, retryHeaders("delete"));
  if (removed.status !== "deleted" || removeReplay.status !== "deleted") {
    throw new Error("delete did not settle or replay as deleted");
  }
  deleted = true;
  proof.deleteReplay = true;
  passed = true;
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  if (boxId !== undefined && !deleted) {
    try {
      await api.interrupt({ boxId }, retryHeaders("cleanup-interrupt"));
    } catch {
      // No active prompt is an expected cleanup state.
    }
    try {
      const removed = await api.remove({ boxId }, retryHeaders("cleanup-delete"));
      deleted = removed.status === "deleted";
    } catch {
      try {
        await api.stop({ boxId }, retryHeaders("cleanup-stop"));
        const removed = await api.remove({ boxId }, retryHeaders("cleanup-delete"));
        deleted = removed.status === "deleted";
      } catch {
        // The exact GCP residue oracle below remains authoritative.
      }
    }
  }

  if (resourceSuffix !== undefined) {
    preEmergencyResidue = observeResidue(resourceSuffix);
    if (Object.values(preEmergencyResidue).some((value) => value !== 0)) {
      emergencyCleanupAttempted = true;
      passed = false;
      failure ??= "independent GCP residue oracle found Box-owned resources";
      emergencyCleanup(resourceSuffix);
    }
    residue = observeResidue(resourceSuffix);
    if (Object.values(residue).some((value) => value !== 0)) {
      passed = false;
      failure = "independent GCP residue oracle found Box-owned resources after cleanup";
    }
  }
}

if (foreignToken === undefined || foreignToken.length === 0) {
  passed = false;
  failure ??= "cross-owner token was not configured";
}
if (!Object.values(proof).every(Boolean)) {
  passed = false;
  failure ??= "one or more live Box proof rows did not pass";
}

const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_sbx09_box_live_acceptance.v1",
  capturedAt: new Date().toISOString(),
  passed,
  ...(failure === undefined ? {} : { failure }),
  environment: "staging",
  sourceRevision,
  imageDigest,
  profileDigest,
  sandboxRefDigest: boxId === undefined ? null : `sha256:${sha256(boxId)}`,
  sdk: sdkEvidence.sdk,
  openApiSha256: sdkEvidence.openApiSha256,
  translatorRef: sdkEvidence.translatorRef,
  proof,
  providerEvents,
  nativeEventCount,
  maximumNativeEventSequence,
  artifactDigest: artifactDigest ?? null,
  deleted,
  preEmergencyResidue,
  emergencyCleanupAttempted,
  residue,
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, {
  mode: 0o600,
});
process.stdout.write(
  `${JSON.stringify({ passed, evidence, deleted, emergencyCleanupAttempted, residue })}\n`,
);
if (!passed) {
  process.stderr.write(`${failure ?? "managed-sandbox Box live acceptance failed"}\n`);
  process.exit(1);
}
