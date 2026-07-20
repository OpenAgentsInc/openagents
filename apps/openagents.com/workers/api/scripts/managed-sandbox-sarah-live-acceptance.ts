#!/usr/bin/env -S pnpm exec tsx

import {
  type ManagedSandboxResource,
  ManagedSandboxResourceSchema,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";

import type { OpenAgentsWorkerEnv } from "../src/bindings";
import { defaultMakeKhalaSyncSqlClient } from "../src/khala-sync-push-routes";
import {
  managedSandboxBoxV1PolicyForEnv,
  managedSandboxBoxV1RuntimeForEnv,
  managedSandboxBoxV1StoreForEnv,
} from "../src/managed-sandbox-box-v1-adapter";
import { makeManagedSandboxBroker } from "../src/managed-sandbox-broker";
import type { SarahAgentToolResult } from "../src/sarah-agent-runtime";
import { makeSarahManagedSandboxTools } from "../src/sarah-managed-sandbox";
import { ensureSarahPrincipal } from "../src/sarah-owner-routes";

type Residue = Readonly<{
  compute: number;
  firewall: number;
  scratch: number;
  ingress: number;
}>;

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value.length === 0) {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
};

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const sleep = (milliseconds: number): Promise<void> =>
  new Promise((complete) => setTimeout(complete, milliseconds));

const parseArgs = (): { apply: boolean; evidence: string } => {
  const args = process.argv.slice(2);
  let apply = false;
  let evidence = resolvePath("artifacts/managed-sandbox-sarah-live.json");
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      apply = true;
    } else if (argument === "--evidence" && args[index + 1] !== undefined) {
      evidence = resolvePath(args[index + 1]!);
      index += 1;
    } else {
      throw new Error("usage: managed-sandbox-sarah-live-acceptance.ts --apply [--evidence PATH]");
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

const databaseUrl = required("OA_MANAGED_SANDBOX_DATABASE_URL");
const ownerUserId = required("OA_MANAGED_SANDBOX_OWNER_USER_ID");
const controlUrl = required("OA_MANAGED_SANDBOX_CONTROL_URL");
const controlToken = required("OA_MANAGED_SANDBOX_CONTROL_TOKEN");
const brokerSigningKey = required("OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const profileDigest = required("OA_MANAGED_SANDBOX_PROFILE_DIGEST");
const sourceRevision = required("OA_MANAGED_SANDBOX_SOURCE_REVISION");
const workerRevision = required("OA_MANAGED_SANDBOX_WORKER_REVISION");
const controlRevision = required("OA_MANAGED_SANDBOX_CONTROL_REVISION");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const stamp = `${Date.now()}-${process.pid}`;
const suffix = sha256(stamp).slice(0, 20);
const workUnitRef = `work.sarah.sbx09.${suffix}`;
const attachmentRef = `attachment.sarah.sbx09.${suffix}`;

const runtimeEnv = {
  KHALA_SYNC_DB: { connectionString: databaseUrl },
  OA_MANAGED_SANDBOX_CONTROL_URL: controlUrl,
  OA_MANAGED_SANDBOX_CONTROL_TOKEN: controlToken,
  OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY: brokerSigningKey,
  OA_MANAGED_SANDBOX_IMAGE_DIGEST: imageDigest,
  OA_MANAGED_SANDBOX_PROFILE_DIGEST: profileDigest,
  OA_MANAGED_SANDBOX_CODEX_MODEL: process.env.OA_MANAGED_SANDBOX_CODEX_MODEL?.trim() || "gpt-5.6",
  OA_MANAGED_SANDBOX_CLAUDE_MODEL:
    process.env.OA_MANAGED_SANDBOX_CLAUDE_MODEL?.trim() || "claude-sonnet-4-6",
  OA_MANAGED_SANDBOX_CLAUDE_LOCATION:
    process.env.OA_MANAGED_SANDBOX_CLAUDE_LOCATION?.trim() || "us-east5",
} as OpenAgentsWorkerEnv;

const list = (
  collection: "instances" | "firewall-rules" | "disks",
  filter: string,
): ReadonlyArray<string> => {
  const args = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") args.push("--zones", zone);
  args.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return output.split("\n").filter((line) => line.trim().length > 0);
};

const count = (collection: "instances" | "firewall-rules" | "disks", filter: string): number =>
  list(collection, filter).length;

const globalResidue = (): Residue => {
  const firewallNames = list("firewall-rules", "name~^oa-msb-");
  return {
    compute: count("instances", "name~^oa-msb-"),
    firewall: firewallNames.length,
    scratch: count("disks", "name~^oa-msb-"),
    ingress: firewallNames.filter((name) => name.includes("-ssh-") || name.includes("-ingress-"))
      .length,
  };
};

const toolResultResource = (result: SarahAgentToolResult) => {
  const body = JSON.parse(result.content) as { resource?: unknown };
  if (body.resource === undefined) {
    throw new Error("Sarah tool result omitted the managed-sandbox resource");
  }
  return S.decodeUnknownSync(ManagedSandboxResourceSchema)(body.resource, {
    onExcessProperty: "error",
  });
};

const toolResultBody = (result: SarahAgentToolResult) =>
  JSON.parse(result.content) as Readonly<{
    resource?: unknown;
    turn?: Readonly<{
      turnRef?: string;
      status?: string;
      lastEventSequence?: number;
    }> | null;
    turnReceipt?: Readonly<{ receiptRef?: string }> | null;
    events?: ReadonlyArray<
      Readonly<{
        _tag?: string;
        turnEventSequence?: number;
      }>
    >;
    command?: Readonly<{ commandRef?: string }>;
    targetReceipt?: Readonly<{ receiptRef?: string }>;
    activity?: ReadonlyArray<
      Readonly<{
        order?: number;
        receiptRef?: string;
        state?: string;
      }>
    >;
  }>;

const exactScope = (resource: ManagedSandboxResource, idempotencyRef: string) => ({
  programRef: "program.managed_agent_sandboxes",
  repositoryRef: "OpenAgentsInc/openagents",
  targetRef: resource.target.targetRef,
  imageDigest: resource.imageDigest,
  profileRef: resource.profileRef,
  sandboxRef: resource.sandboxRef,
  workUnitRef: resource.workUnitRef,
  resourceGeneration: resource.resourceGeneration,
  expectedVersion: resource.version,
  budget: resource.budget,
  capabilityRefs: resource.capabilities.map((capability) => capability.capabilityRef),
  idempotencyRef,
});

const proof = {
  exactEightToolInventory: false,
  noGenericAdminTool: false,
  authorityCreate: false,
  ownerScopedList: false,
  inspect: false,
  longRunningDispatch: false,
  orderedActivity: false,
  quietNotTerminal: false,
  interruptReplay: false,
  terminalReceipt: false,
  stop: false,
  resume: false,
  deleteCleanup: false,
  crossOwnerDeniedBeforeEffect: false,
};

const before = globalResidue();
const client = await defaultMakeKhalaSyncSqlClient(databaseUrl);
let currentResource: ManagedSandboxResource | undefined;
let currentTurnRef: string | undefined;
let failure: string | undefined;
let emergencyCleanupAttempted = false;
let passed = false;
let toolOrdinal = 0;
let providerEvents: ReadonlyArray<string> = [];
let terminalReceiptRef: string | null = null;
let authorityReceiptRefs: ReadonlyArray<string> = [];
let targetReceiptRefs: ReadonlyArray<string> = [];
let sarahThreadRef: string | undefined;

try {
  const sarahPrincipal = await ensureSarahPrincipal(client.sql, ownerUserId);
  sarahThreadRef = sarahPrincipal.threadRef;
  const policy = await Effect.runPromise(managedSandboxBoxV1PolicyForEnv(runtimeEnv));
  const runtime = await Effect.runPromise(managedSandboxBoxV1RuntimeForEnv(runtimeEnv));
  const store = managedSandboxBoxV1StoreForEnv(runtimeEnv);
  const principal = {
    actorRef: "principal.sarah",
    ownerRef: ownerUserId,
    tenantRef: ownerUserId,
    login: "Sarah",
    email: null,
  };
  const broker = makeManagedSandboxBroker({
    principal,
    policy,
    runtime,
    store,
  });
  const tools = makeSarahManagedSandboxTools({
    sql: client.sql,
    ownerUserId,
    threadRef: sarahPrincipal.threadRef,
    turnId: `turn.sarah.sbx09.${suffix}`,
    principal,
    policy,
    broker,
    runtimeAdmitted: true,
  });
  const names = tools.map((tool) => tool.definition.name);
  proof.exactEightToolInventory =
    names.length === 8 &&
    names.join("\n") ===
      [
        "managed_sandbox_create",
        "managed_sandboxes_list",
        "managed_sandbox_inspect",
        "managed_sandbox_dispatch",
        "managed_sandbox_interrupt",
        "managed_sandbox_stop",
        "managed_sandbox_resume",
        "managed_sandbox_delete",
      ].join("\n");
  proof.noGenericAdminTool = !names
    .join(" ")
    .match(/gcloud|shell|database|topology|credential|filesystem|container_admin|full_auto_start/u);
  if (!proof.exactEightToolInventory || !proof.noGenericAdminTool) {
    throw new Error("Sarah managed-sandbox tool inventory widened");
  }

  const execute = async (name: string, args: unknown) => {
    const tool = tools.find((candidate) => candidate.definition.name === name);
    if (tool === undefined) throw new Error(`missing Sarah tool ${name}`);
    toolOrdinal += 1;
    const result = await Effect.runPromise(
      tool.execute(args, {
        id: `tool.sarah.sbx09.${toolOrdinal}`,
        type: "function",
        function: { name, arguments: JSON.stringify(args) },
      }),
    );
    if (result.authorityReceiptRef !== undefined) {
      authorityReceiptRefs = [...authorityReceiptRefs, result.authorityReceiptRef];
    }
    const body = toolResultBody(result);
    targetReceiptRefs = [
      ...targetReceiptRefs,
      ...(body.activity ?? [])
        .map((activity) => activity.receiptRef)
        .filter((ref): ref is string => typeof ref === "string"),
    ];
    return result;
  };

  const common = {
    programRef: "program.managed_agent_sandboxes",
    repositoryRef: "OpenAgentsInc/openagents",
    targetRef: policy.target.targetRef,
    imageDigest: policy.imageDigest,
    profileRef: policy.profileRef,
  };
  const ttlSeconds = Math.min(900, policy.maxTtlSeconds);
  const budget = {
    currency: "USD" as const,
    maxCostMicros: policy.maxCostMicros,
    maxCpuMillis: Math.min(policy.maxCpuMillis, ttlSeconds * 1_000),
    maxNetworkBytes: policy.maxNetworkBytes,
    maxArtifactBytes: policy.maxArtifactBytes,
    maxLifetimeSeconds: ttlSeconds,
  };

  const created = await execute("managed_sandbox_create", {
    ...common,
    workUnitRef,
    attachmentRef,
    attachmentGeneration: 1,
    ttlSeconds,
    budget,
    capabilityKinds: ["agent_turn", "command", "file_read", "file_write", "artifact_read"],
    idempotencyRef: `sbx09-${suffix}-create`,
  });
  currentResource = toolResultResource(created);
  proof.authorityCreate =
    created.authorityAllowed !== false &&
    !created.isError &&
    currentResource.facts.lifecycle === "ready";
  if (!proof.authorityCreate) throw new Error("Sarah create did not reach ready");

  const listed = await execute("managed_sandboxes_list", {
    ...common,
    idempotencyRef: `sbx09-${suffix}-list`,
  });
  const listedBody = JSON.parse(listed.content) as {
    resources?: ReadonlyArray<{ sandboxRef?: string; ownerRef?: string }>;
  };
  proof.ownerScopedList =
    listed.authorityAllowed !== false &&
    listedBody.resources?.some(
      (resource) =>
        resource.sandboxRef === currentResource?.sandboxRef && resource.ownerRef === ownerUserId,
    ) === true;

  const inspected = await execute("managed_sandbox_inspect", {
    ...exactScope(currentResource, `sbx09-${suffix}-inspect`),
  });
  currentResource = toolResultResource(inspected);
  proof.inspect = inspected.authorityAllowed !== false && !inspected.isError;

  const foreignTools = makeSarahManagedSandboxTools({
    sql: client.sql,
    ownerUserId: `owner.foreign.${suffix}`,
    threadRef: `thread.sarah.${sha256(`foreign\n${stamp}`).slice(0, 24)}`,
    turnId: `turn.sarah.foreign.${suffix}`,
    principal: {
      actorRef: "principal.sarah",
      ownerRef: `owner.foreign.${suffix}`,
      tenantRef: `owner.foreign.${suffix}`,
      login: "Sarah",
      email: null,
    },
    policy,
    broker,
    runtimeAdmitted: true,
  });
  const foreignInspect = foreignTools.find(
    (tool) => tool.definition.name === "managed_sandbox_inspect",
  );
  if (foreignInspect === undefined) throw new Error("missing foreign inspect");
  const beforeForeign = currentResource.version;
  const foreign = await Effect.runPromise(
    foreignInspect.execute(
      {
        ...exactScope(currentResource, `sbx09-${suffix}-foreign`),
        sandboxRef: currentResource.sandboxRef,
      },
      {
        id: "tool.sarah.sbx09.foreign",
        type: "function",
        function: { name: "managed_sandbox_inspect", arguments: "{}" },
      },
    ),
  );
  const afterForeign = await Effect.runPromise(
    store.inspect({
      ownerRef: ownerUserId,
      tenantRef: ownerUserId,
      sandboxRef: currentResource.sandboxRef,
    }),
  );
  proof.crossOwnerDeniedBeforeEffect =
    foreign.authorityAllowed === false &&
    foreign.isError === true &&
    afterForeign.version === beforeForeign;

  const agentCapability = currentResource.capabilities.find(
    (capability) => capability.kind === "agent_turn",
  );
  if (agentCapability === undefined) {
    throw new Error("Sarah create omitted the agent-turn capability");
  }
  currentTurnRef = `turn.sarah.sbx09.long.${suffix}`;
  const dispatched = await execute("managed_sandbox_dispatch", {
    ...exactScope(currentResource, `sbx09-${suffix}-dispatch`),
    turnRef: currentTurnRef,
    capabilityRef: agentCapability.capabilityRef,
    prompt: "Use the shell tool to run sleep 300, then reply OPENAGENTS_SARAH_INTERRUPT_MISSED.",
    runtime: {
      provider: "codex",
      modelRef: "model.codex.default",
      harnessRef: "harness.openai.codex-sdk.v1",
    },
  });
  currentResource = toolResultResource(dispatched);
  const dispatchBody = toolResultBody(dispatched);
  const dispatchEvents = dispatchBody.events ?? [];
  providerEvents = dispatchEvents
    .map(({ _tag: eventTag }) => eventTag)
    .filter((tag): tag is string => typeof tag === "string");
  proof.longRunningDispatch =
    dispatched.authorityAllowed !== false &&
    dispatchBody.turn?.turnRef === currentTurnRef &&
    ["pending", "running"].includes(dispatchBody.turn.status ?? "");
  proof.orderedActivity = dispatchEvents.every(
    (event, index) =>
      index === 0 ||
      (event.turnEventSequence ?? 0) > (dispatchEvents[index - 1]?.turnEventSequence ?? -1),
  );
  if (!proof.longRunningDispatch || !proof.orderedActivity) {
    throw new Error("Sarah long-running dispatch did not start structurally");
  }

  await sleep(65_000);
  const quiet = await execute("managed_sandbox_inspect", {
    ...exactScope(currentResource, `sbx09-${suffix}-quiet`),
  });
  currentResource = toolResultResource(quiet);
  const quietBody = toolResultBody(quiet);
  proof.quietNotTerminal =
    quietBody.turn?.turnRef === currentTurnRef &&
    !["settled", "failed", "interrupted"].includes(quietBody.turn.status ?? "") &&
    quietBody.turnReceipt == null;

  const interruptInput = {
    ...exactScope(currentResource, `sbx09-${suffix}-interrupt`),
    turnRef: currentTurnRef,
    reasonRef: "reason.owner.sbx09.sarah-interrupt",
  };
  const interrupted = await execute("managed_sandbox_interrupt", interruptInput);
  currentResource = toolResultResource(interrupted);
  const replayed = await execute("managed_sandbox_interrupt", interruptInput);
  currentResource = toolResultResource(replayed);
  const interruptedBody = toolResultBody(interrupted);
  const replayedBody = toolResultBody(replayed);
  proof.interruptReplay =
    interruptedBody.command?.commandRef !== undefined &&
    interruptedBody.command.commandRef === replayedBody.command?.commandRef &&
    interruptedBody.targetReceipt?.receiptRef !== undefined &&
    interruptedBody.targetReceipt.receiptRef === replayedBody.targetReceipt?.receiptRef;

  const waitForTerminal = async (
    resource: ManagedSandboxResource,
    attempt: number,
  ): Promise<ManagedSandboxResource> => {
    const terminal = await execute("managed_sandbox_inspect", {
      ...exactScope(resource, `sbx09-${suffix}-terminal-${attempt}`),
    });
    const nextResource = toolResultResource(terminal);
    const body = toolResultBody(terminal);
    if (
      body.turn?.turnRef === currentTurnRef &&
      ["settled", "failed", "interrupted"].includes(body.turn.status ?? "") &&
      body.turnReceipt?.receiptRef !== undefined
    ) {
      terminalReceiptRef = body.turnReceipt.receiptRef;
      proof.terminalReceipt = true;
      return nextResource;
    }
    if (attempt >= 90) return nextResource;
    await sleep(2_000);
    return waitForTerminal(nextResource, attempt + 1);
  };
  currentResource = await waitForTerminal(currentResource, 1);
  if (!proof.terminalReceipt) {
    throw new Error("Sarah interrupted turn did not produce a terminal receipt");
  }

  const stopped = await execute("managed_sandbox_stop", {
    ...exactScope(currentResource, `sbx09-${suffix}-stop`),
    reasonRef: "reason.owner.sbx09.sarah-stop",
  });
  currentResource = toolResultResource(stopped);
  proof.stop = currentResource.facts.lifecycle === "stopped";

  const resumed = await execute("managed_sandbox_resume", {
    ...exactScope(currentResource, `sbx09-${suffix}-resume`),
  });
  currentResource = toolResultResource(resumed);
  proof.resume =
    currentResource.facts.lifecycle === "ready" && currentResource.resourceGeneration === 2;

  const stoppedAgain = await execute("managed_sandbox_stop", {
    ...exactScope(currentResource, `sbx09-${suffix}-stop-final`),
    reasonRef: "reason.owner.sbx09.sarah-stop-final",
  });
  currentResource = toolResultResource(stoppedAgain);
  const deleted = await execute("managed_sandbox_delete", {
    ...exactScope(currentResource, `sbx09-${suffix}-delete`),
    reasonRef: "reason.owner.sbx09.sarah-delete",
  });
  currentResource = toolResultResource(deleted);
  proof.deleteCleanup =
    currentResource.facts.lifecycle === "deleted" && currentResource.facts.cleanupComplete;
  passed = Object.values(proof).every(Boolean);
} catch (error) {
  failure =
    error instanceof Error && error.message.length > 0
      ? error.message
      : typeof error === "object" &&
          error !== null &&
          "reason" in error &&
          typeof error.reason === "string"
        ? error.reason
        : String(error);
} finally {
  if (currentResource !== undefined && currentResource.facts.lifecycle !== "deleted") {
    emergencyCleanupAttempted = true;
    try {
      const policy = await Effect.runPromise(managedSandboxBoxV1PolicyForEnv(runtimeEnv));
      const runtime = await Effect.runPromise(managedSandboxBoxV1RuntimeForEnv(runtimeEnv));
      const principal = {
        actorRef: "principal.sarah",
        ownerRef: ownerUserId,
        tenantRef: ownerUserId,
        login: "Sarah",
        email: null,
      };
      const cleanupTools = makeSarahManagedSandboxTools({
        sql: client.sql,
        ownerUserId,
        threadRef: (await ensureSarahPrincipal(client.sql, ownerUserId)).threadRef,
        turnId: `turn.sarah.sbx09.cleanup.${suffix}`,
        principal,
        policy,
        broker: makeManagedSandboxBroker({
          principal,
          policy,
          runtime,
          store: managedSandboxBoxV1StoreForEnv(runtimeEnv),
        }),
        runtimeAdmitted: true,
      });
      const cleanupExecute = async (name: string, args: unknown) => {
        const tool = cleanupTools.find((candidate) => candidate.definition.name === name);
        if (tool === undefined) return undefined;
        return Effect.runPromise(
          tool.execute(args, {
            id: `tool.sarah.sbx09.cleanup.${name}`,
            type: "function",
            function: { name, arguments: JSON.stringify(args) },
          }),
        );
      };
      currentResource = await Effect.runPromise(
        managedSandboxBoxV1StoreForEnv(runtimeEnv).inspect({
          ownerRef: ownerUserId,
          tenantRef: ownerUserId,
          sandboxRef: currentResource.sandboxRef,
        }),
      );
      if (currentTurnRef !== undefined && currentResource.facts.runtimeState !== "none") {
        await cleanupExecute("managed_sandbox_interrupt", {
          ...exactScope(currentResource, `sbx09-${suffix}-cleanup-interrupt`),
          turnRef: currentTurnRef,
          reasonRef: "reason.owner.sbx09.cleanup-interrupt",
        }).catch(() => undefined);
        currentResource = await Effect.runPromise(
          managedSandboxBoxV1StoreForEnv(runtimeEnv).inspect({
            ownerRef: ownerUserId,
            tenantRef: ownerUserId,
            sandboxRef: currentResource.sandboxRef,
          }),
        );
      }
      if (["ready", "idle", "running"].includes(currentResource.facts.lifecycle)) {
        const stopped = await cleanupExecute("managed_sandbox_stop", {
          ...exactScope(currentResource, `sbx09-${suffix}-cleanup-stop`),
          reasonRef: "reason.owner.sbx09.cleanup-stop",
        });
        if (stopped !== undefined) currentResource = toolResultResource(stopped);
      }
      if (currentResource.facts.lifecycle !== "deleted") {
        const deleted = await cleanupExecute("managed_sandbox_delete", {
          ...exactScope(currentResource, `sbx09-${suffix}-cleanup-delete`),
          reasonRef: "reason.owner.sbx09.cleanup-delete",
        });
        if (deleted !== undefined) currentResource = toolResultResource(deleted);
      }
    } catch {
      // The independent GCP residue oracle below remains authoritative.
    }
  }
  await client.end().catch(() => undefined);
}

const after = globalResidue();
if (Object.values(after).some((value) => value !== 0)) {
  passed = false;
  failure ??= "independent GCP residue oracle found Sarah-owned resources";
}
if (!Object.values(proof).every(Boolean)) {
  passed = false;
  failure ??= "one or more Sarah live proof rows did not pass";
}

const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_sarah_live_acceptance.v1",
  capturedAt: new Date().toISOString(),
  environment: "staging",
  sourceRevision,
  deployedRevisions: {
    worker: workerRevision,
    control: controlRevision,
  },
  imageDigest,
  profileDigest,
  passed,
  ...(failure === undefined ? {} : { failure }),
  ownerRefDigest: `sha256:${sha256(ownerUserId)}`,
  sandboxRefDigest:
    currentResource === undefined ? null : `sha256:${sha256(currentResource.sandboxRef)}`,
  threadRefDigest: sarahThreadRef === undefined ? null : `sha256:${sha256(sarahThreadRef)}`,
  proof,
  providerEvents,
  terminalReceiptRef,
  authorityReceiptDigests: [...new Set(authorityReceiptRefs.map((ref) => `sha256:${sha256(ref)}`))],
  targetReceiptDigests: [...new Set(targetReceiptRefs.map((ref) => `sha256:${sha256(ref)}`))],
  emergencyCleanupAttempted,
  before,
  after,
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, {
  mode: 0o600,
});
process.stdout.write(`${JSON.stringify({ passed, evidence, emergencyCleanupAttempted, after })}\n`);
if (!passed) {
  process.stderr.write(`${failure ?? "Sarah live acceptance failed"}\n`);
  process.exit(1);
}
