#!/usr/bin/env -S pnpm exec tsx
/* eslint-disable no-underscore-dangle -- Effect Schema tagged unions intentionally expose `_tag`. */

import { type ManagedSandboxCommand } from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";

import type { OpenAgentsWorkerEnv } from "../../openagents.com/workers/api/src/bindings.ts";
import {
  managedSandboxBoxV1PolicyForEnv,
  managedSandboxBoxV1RuntimeForEnv,
  managedSandboxBoxV1StoreForEnv,
} from "../../openagents.com/workers/api/src/managed-sandbox-box-v1-adapter.ts";
import {
  makeManagedSandboxDesktopRoutes,
  MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH,
  MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH,
} from "../../openagents.com/workers/api/src/managed-sandbox-desktop-routes.ts";
import {
  IdeAgentCodeCommandSchema,
  IdeAgentAttachmentRefSchema,
  IdeAgentAttachmentSchema,
} from "../src/ide/agent-code-contract.ts";
import { openIdeAgentCodeHost, type IdeAgentCodeHost } from "../src/ide/agent-code-host.ts";
import {
  IdeManagedSandboxCommandSchema,
  type IdeManagedSandboxCommandResult,
  type IdeManagedSandboxSnapshot,
} from "../src/ide/managed-sandbox-contract.ts";
import { openIdeManagedSandboxHost } from "../src/ide/managed-sandbox-host.ts";
import {
  IdeAttachmentGenerationSchema,
  IdePlacementGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "../src/ide/project-contract.ts";
import { openWorkspaceService } from "../src/workspace-service.ts";

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
  let evidence = resolvePath("artifacts/managed-sandbox-desktop-live.json");
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--apply") {
      apply = true;
    } else if (args[index] === "--evidence" && args[index + 1] !== undefined) {
      evidence = resolvePath(args[index + 1]!);
      index += 1;
    } else {
      throw new Error("usage: managed-sandbox-live-acceptance.ts --apply [--evidence PATH]");
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
const suffix = sha256(`${stamp}\n${sourceRevision}`).slice(0, 20);
const timestamp = (): string => new Date().toISOString();

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
const residue = (): Residue => {
  const firewalls = list("firewall-rules", "name~^oa-msb-");
  return {
    compute: list("instances", "name~^oa-msb-").length,
    firewall: firewalls.length,
    scratch: list("disks", "name~^oa-msb-").length,
    ingress: firewalls.filter((name) => name.includes("-ssh-") || name.includes("-ingress-"))
      .length,
  };
};

const sourceRoot = resolvePath(import.meta.dirname, "../../..");
const sourceRootDigest = sha256(sourceRoot);
const workspace = openWorkspaceService(sourceRoot, {
  grantRef: `workspace.grant.sbx09-${suffix}`,
});

const attachment = IdeAgentAttachmentSchema.make({
  schemaVersion: "openagents.desktop.ide-agent-code.v1",
  agentAttachmentRef: IdeAgentAttachmentRefSchema.make(`ide.agent-attachment.sbx09-${suffix}`),
  projectRef: IdeProjectRefSchema.make(`ide.project.openagents-${sourceRootDigest.slice(0, 20)}`),
  rootRef: IdeRootRefSchema.make(`ide.root.openagents-${sourceRootDigest.slice(0, 20)}`),
  worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${sourceRevision.slice(0, 20)}`),
  sessionRef: IdeSessionRefSchema.make(`ide.session.sbx09-${suffix}`),
  attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
  placementGeneration: IdePlacementGenerationSchema.make(1),
  grantRef: workspace.grantRef,
  attachedAt: IdeTimestampSchema.make(timestamp()),
  expiresAt: null,
});

const snapshotFrom = (result: IdeManagedSandboxCommandResult): IdeManagedSandboxSnapshot => {
  if (result._tag !== "Succeeded") {
    throw new Error(`Desktop managed-sandbox command refused: ${result.reason}: ${result.message}`);
  }
  return result.snapshot;
};

const proof = {
  httpAdmission: false,
  exactTarget: false,
  create: false,
  ownerScopeDeniedBeforeEffect: false,
  identityContinuity: false,
  longRunningDispatch: false,
  quietNotTerminal: false,
  orderedEvents: false,
  interrupt: false,
  terminalReceipt: false,
  stop: false,
  resume: false,
  deleteCleanup: false,
  noCredentialInSnapshot: false,
  sourceAttachment: false,
};

const before = residue();
let host: Awaited<ReturnType<typeof openIdeManagedSandboxHost>> | undefined;
let agentCodeHost: IdeAgentCodeHost | undefined;
let current: IdeManagedSandboxSnapshot | undefined;
let failure: string | undefined;
let emergencyCleanupAttempted = false;
let passed = false;
let ordinal = 0;
let nativeCreate: ManagedSandboxCommand | undefined;
let turnReceiptRef: string | null = null;
let providerEvents: ReadonlyArray<string> = [];

try {
  agentCodeHost = await openIdeAgentCodeHost(workspace, { persistencePath: null });
  const attached = await agentCodeHost.command(
    IdeAgentCodeCommandSchema.cases.Attach.make({ attachment }),
  );
  if (attached._tag !== "Succeeded") {
    throw new Error(`Desktop source attachment was refused: ${attached.reason}`);
  }
  proof.sourceAttachment =
    attached.snapshot.lifecycle === "attached" &&
    attached.snapshot.attachment?.grantRef === workspace.grantRef;
  const routes = makeManagedSandboxDesktopRoutes<OpenAgentsWorkerEnv>({
    authenticateOwner: async () => ({ userId: ownerUserId }),
    enabled: () => true,
    policy: managedSandboxBoxV1PolicyForEnv,
    store: managedSandboxBoxV1StoreForEnv,
    runtime: managedSandboxBoxV1RuntimeForEnv,
  });
  const policy = await Effect.runPromise(managedSandboxBoxV1PolicyForEnv(runtimeEnv));
  const context: ExecutionContext = {
    props: {},
    tracing: {},
    passThroughOnException: () => undefined,
    waitUntil: () => undefined,
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const request = new Request(input, init);
    if (request.url.endsWith(MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH)) {
      return Effect.runPromise(routes.admission(request, runtimeEnv, context));
    }
    if (request.url.endsWith(MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH)) {
      const body = (await request.clone().json()) as {
        command?: ManagedSandboxCommand;
      };
      if (body.command?._tag === "Create") nativeCreate = body.command;
      const response = await Effect.runPromise(routes.commands(request, runtimeEnv, context));
      const responseBody = (await response.clone().json()) as {
        result?: { turnReceipt?: { receiptRef?: string } | null };
      };
      const observed = responseBody.result?.turnReceipt?.receiptRef;
      if (observed !== undefined) turnReceiptRef = observed;
      return response;
    }
    return new Response(null, { status: 404 });
  };
  host = await openIdeManagedSandboxHost({
    enabled: true,
    credential: () => ({
      ownerUserId,
      accessToken: `owner-gated-live-${suffix}`,
      refreshToken: `owner-gated-live-${suffix}`,
    }),
    baseUrl: "https://staging.openagents.invalid",
    agentCodeHost,
    persistencePath: null,
    fetchImpl,
  });

  const envelope = (label: string) => {
    ordinal += 1;
    return {
      requestRef: `command.desktop.sbx09.${suffix}.${ordinal}.${label}`,
      idempotencyRef: `idempotency.desktop.sbx09.${suffix}.${ordinal}.${label}`,
      requestedAt: IdeTimestampSchema.make(timestamp()),
    };
  };
  const execute = async (
    value: typeof IdeManagedSandboxCommandSchema.Type,
  ): Promise<IdeManagedSandboxSnapshot> => snapshotFrom(await host!.command(value));

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.RefreshAdmission.make({
      ...envelope("admission"),
    }),
  );
  proof.httpAdmission = current.admission._tag === "Available";
  proof.exactTarget =
    current.admission._tag === "Available" &&
    current.admission.imageDigest === imageDigest &&
    current.admission.profileRef === policy.profileRef &&
    current.admission.target.provider === "google_cloud" &&
    current.admission.target.isolation === "gce_vm";

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Create.make({
      ...envelope("create"),
      expectedAttachment: attachment,
      workUnitRef: `work.desktop.sbx09.${suffix}`,
    }),
  );
  proof.create = current.resource?.facts.lifecycle === "ready" && current.binding !== null;
  if (!proof.create || current.resource === null || nativeCreate === undefined) {
    throw new Error("Desktop create did not reach the exact ready target");
  }

  const store = managedSandboxBoxV1StoreForEnv(runtimeEnv);
  const beforeForeign = current.resource.version;
  const foreign = await Effect.runPromise(
    routes.commands(
      new Request(`https://staging.openagents.invalid${MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
          command: {
            ...nativeCreate,
            ownerRef: `owner.foreign.${suffix}`,
            tenantRef: `owner.foreign.${suffix}`,
          },
          attachmentGeneration: 1,
        }),
      }),
      runtimeEnv,
      context,
    ),
  );
  const unchanged = await Effect.runPromise(
    store.inspect({
      ownerRef: ownerUserId,
      tenantRef: ownerUserId,
      sandboxRef: current.resource.sandboxRef,
    }),
  );
  proof.ownerScopeDeniedBeforeEffect =
    foreign.status === 403 && unchanged.version === beforeForeign;

  const snapshots: Array<IdeManagedSandboxSnapshot> = [current];
  const capability = current.resource.capabilities.find(
    (candidate) => candidate.kind === "agent_turn",
  );
  if (capability === undefined) {
    throw new Error("Desktop admission omitted the agent-turn capability");
  }
  const turnRef = `turn.desktop.sbx09.long.${suffix}`;
  const prompt =
    "Use the shell tool to run sleep 300, then reply OPENAGENTS_DESKTOP_INTERRUPT_MISSED.";
  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Dispatch.make({
      ...envelope("dispatch"),
      expectedAttachment: attachment,
      sandboxRef: current.resource.sandboxRef,
      turnRef,
      capabilityRef: capability.capabilityRef,
      prompt,
      promptDigest: `sha256:${sha256(prompt)}`,
      runtime: {
        provider: "codex",
        modelRef: "model.codex.default",
        harnessRef: "harness.openai.codex-sdk.v1",
      },
    }),
  );
  snapshots.push(current);
  providerEvents = current.events.map(({ _tag: eventTag }) => eventTag);
  proof.longRunningDispatch =
    current.turn?.turnRef === turnRef && ["pending", "running"].includes(current.turn.status);
  proof.orderedEvents = current.events.every(
    (event, index) => index === 0 || event.sequence > (current?.events[index - 1]?.sequence ?? -1),
  );

  await sleep(65_000);
  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Inspect.make({
      ...envelope("quiet"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
    }),
  );
  snapshots.push(current);
  proof.quietNotTerminal =
    current.turn?.turnRef === turnRef &&
    !["settled", "failed", "interrupted"].includes(current.turn.status);

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Interrupt.make({
      ...envelope("interrupt"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
      turnRef,
      reasonRef: "reason.owner.desktop.sbx09-interrupt",
    }),
  );
  snapshots.push(current);
  proof.interrupt =
    current.turn?.turnRef === turnRef &&
    ["interrupting", "interrupted"].includes(current.turn.status);

  const waitForTerminal = async (
    snapshot: IdeManagedSandboxSnapshot,
    attempt: number,
  ): Promise<IdeManagedSandboxSnapshot> => {
    if (
      snapshot.turn !== null &&
      ["settled", "failed", "interrupted"].includes(snapshot.turn.status)
    ) {
      return snapshot;
    }
    if (attempt > 90 || snapshot.resource === null) return snapshot;
    await sleep(2_000);
    return waitForTerminal(
      await execute(
        IdeManagedSandboxCommandSchema.cases.Inspect.make({
          ...envelope(`terminal-${attempt}`),
          expectedAttachment: attachment,
          sandboxRef: snapshot.resource.sandboxRef,
        }),
      ),
      attempt + 1,
    );
  };
  current = await waitForTerminal(current, 1);
  snapshots.push(current);
  proof.terminalReceipt =
    current.turn !== null &&
    ["settled", "failed", "interrupted"].includes(current.turn.status) &&
    turnReceiptRef !== null;

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Stop.make({
      ...envelope("stop"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
      reasonRef: "reason.owner.desktop.sbx09-stop",
    }),
  );
  snapshots.push(current);
  proof.stop = current.resource?.facts.lifecycle === "stopped";

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Resume.make({
      ...envelope("resume"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
    }),
  );
  snapshots.push(current);
  proof.resume =
    current.resource?.facts.lifecycle === "ready" && current.resource.resourceGeneration === 2;

  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Stop.make({
      ...envelope("stop-final"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
      reasonRef: "reason.owner.desktop.sbx09-stop-final",
    }),
  );
  current = await execute(
    IdeManagedSandboxCommandSchema.cases.Delete.make({
      ...envelope("delete"),
      expectedAttachment: attachment,
      sandboxRef: current.resource!.sandboxRef,
      reasonRef: "reason.owner.desktop.sbx09-delete",
    }),
  );
  snapshots.push(current);
  proof.deleteCleanup =
    current.resource?.facts.lifecycle === "deleted" && current.resource.facts.cleanupComplete;
  proof.identityContinuity = snapshots.every(
    (snapshot) =>
      snapshot.binding?.projectRef === attachment.projectRef &&
      snapshot.binding.rootRef === attachment.rootRef &&
      snapshot.binding.worktreeRef === attachment.worktreeRef &&
      snapshot.binding.sessionRef === attachment.sessionRef &&
      snapshot.binding.agentAttachmentRef === attachment.agentAttachmentRef,
  );
  const serialized = JSON.stringify(snapshots);
  proof.noCredentialInSnapshot =
    !serialized.includes(`owner-gated-live-${suffix}`) &&
    !serialized.match(/access[_-]?token|refresh[_-]?token|private[_-]?key|serviceaccount\.com/iu);
  passed = Object.values(proof).every(Boolean);
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  if (
    host !== undefined &&
    current?.resource !== null &&
    current?.resource !== undefined &&
    current.resource.facts.lifecycle !== "deleted"
  ) {
    emergencyCleanupAttempted = true;
    const cleanup = async (command: typeof IdeManagedSandboxCommandSchema.Type): Promise<void> => {
      const result = await host!.command(command);
      if (result._tag === "Succeeded") current = result.snapshot;
    };
    try {
      if (
        current.turn !== null &&
        !["settled", "failed", "interrupted"].includes(current.turn.status)
      ) {
        await cleanup(
          IdeManagedSandboxCommandSchema.cases.Interrupt.make({
            requestRef: `command.desktop.sbx09.${suffix}.cleanup-interrupt`,
            idempotencyRef: `idempotency.desktop.sbx09.${suffix}.cleanup-interrupt`,
            requestedAt: IdeTimestampSchema.make(timestamp()),
            expectedAttachment: attachment,
            sandboxRef: current.resource.sandboxRef,
            turnRef: current.turn.turnRef,
            reasonRef: "reason.owner.desktop.sbx09-cleanup-interrupt",
          }),
        );
      }
      if (["ready", "idle", "running"].includes(current.resource.facts.lifecycle)) {
        await cleanup(
          IdeManagedSandboxCommandSchema.cases.Stop.make({
            requestRef: `command.desktop.sbx09.${suffix}.cleanup-stop`,
            idempotencyRef: `idempotency.desktop.sbx09.${suffix}.cleanup-stop`,
            requestedAt: IdeTimestampSchema.make(timestamp()),
            expectedAttachment: attachment,
            sandboxRef: current.resource.sandboxRef,
            reasonRef: "reason.owner.desktop.sbx09-cleanup-stop",
          }),
        );
      }
      await cleanup(
        IdeManagedSandboxCommandSchema.cases.Delete.make({
          requestRef: `command.desktop.sbx09.${suffix}.cleanup-delete`,
          idempotencyRef: `idempotency.desktop.sbx09.${suffix}.cleanup-delete`,
          requestedAt: IdeTimestampSchema.make(timestamp()),
          expectedAttachment: attachment,
          sandboxRef: current.resource!.sandboxRef,
          reasonRef: "reason.owner.desktop.sbx09-cleanup-delete",
        }),
      );
    } catch {
      // The independent GCP residue oracle below remains authoritative.
    }
  }
  await host?.dispose().catch(() => undefined);
  await agentCodeHost?.dispose().catch(() => undefined);
  workspace.dispose();
}

const after = residue();
if (Object.values(after).some((value) => value !== 0)) {
  passed = false;
  failure ??= "independent GCP residue oracle found Desktop-owned resources";
}
if (!Object.values(proof).every(Boolean)) {
  passed = false;
  failure ??= "one or more Desktop live proof rows did not pass";
}

const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_desktop_live_acceptance.v1",
  capturedAt: timestamp(),
  environment: "staging",
  sourceRevision,
  deployedRevisions: { worker: workerRevision, control: controlRevision },
  imageDigest,
  profileDigest,
  passed,
  ...(failure === undefined ? {} : { failure }),
  ownerRefDigest: `sha256:${sha256(ownerUserId)}`,
  projectRefDigest: `sha256:${sha256(attachment.projectRef)}`,
  sandboxRefDigest:
    current?.resource === null || current?.resource === undefined
      ? null
      : `sha256:${sha256(current.resource.sandboxRef)}`,
  proof,
  providerEvents,
  terminalReceiptDigest: turnReceiptRef === null ? null : `sha256:${sha256(turnReceiptRef)}`,
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
  process.stderr.write(`${failure ?? "Desktop live acceptance failed"}\n`);
  process.exit(1);
}
