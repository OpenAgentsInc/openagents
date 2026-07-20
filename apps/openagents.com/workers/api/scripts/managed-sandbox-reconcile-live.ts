#!/usr/bin/env -S pnpm exec tsx
/* eslint-disable no-await-in-loop, no-underscore-dangle -- lifecycle recovery is serialized and Effect tags use `_tag`. */

import {
  type ManagedSandboxCommand,
  type ManagedSandboxResource,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
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

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value.length === 0) {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
};

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const args = process.argv.slice(2);
let apply = false;
let evidence = resolvePath("artifacts/managed-sandbox-reconcile-live.json");
const sandboxRefs: Array<string> = [];
for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (argument === "--apply") {
    apply = true;
  } else if (argument === "--sandbox-ref" && args[index + 1] !== undefined) {
    sandboxRefs.push(args[index + 1]!);
    index += 1;
  } else if (argument === "--evidence" && args[index + 1] !== undefined) {
    evidence = resolvePath(args[index + 1]!);
    index += 1;
  } else {
    throw new Error(
      "usage: managed-sandbox-reconcile-live.ts --apply --sandbox-ref REF [--sandbox-ref REF ...] [--evidence PATH]",
    );
  }
}
if (
  !apply ||
  sandboxRefs.length === 0 ||
  process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST"
) {
  throw new Error("live reconcile is default-off and requires one or more exact sandbox refs");
}

const databaseUrl = required("OA_MANAGED_SANDBOX_DATABASE_URL");
const ownerUserId = required("OA_MANAGED_SANDBOX_OWNER_USER_ID");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const runtimeEnv = {
  KHALA_SYNC_DB: { connectionString: databaseUrl },
  OA_MANAGED_SANDBOX_CONTROL_URL: required("OA_MANAGED_SANDBOX_CONTROL_URL"),
  OA_MANAGED_SANDBOX_CONTROL_TOKEN: required("OA_MANAGED_SANDBOX_CONTROL_TOKEN"),
  OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY: required("OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY"),
  OA_MANAGED_SANDBOX_IMAGE_DIGEST: required("OA_MANAGED_SANDBOX_IMAGE_DIGEST"),
  OA_MANAGED_SANDBOX_PROFILE_DIGEST: required("OA_MANAGED_SANDBOX_PROFILE_DIGEST"),
  OA_MANAGED_SANDBOX_CODEX_MODEL: "gpt-5.6",
  OA_MANAGED_SANDBOX_CLAUDE_MODEL: "claude-sonnet-4-6",
  OA_MANAGED_SANDBOX_CLAUDE_LOCATION: "us-east5",
} as OpenAgentsWorkerEnv;

const list = (collection: "instances" | "firewall-rules" | "disks"): ReadonlyArray<string> => {
  const command = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") command.push("--zones", zone);
  command.push("--filter", "name~^oa-msb-", "--format", "value(name)");
  return execFileSync(gcloud, command, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] })
    .split("\n")
    .filter((line) => line.trim().length > 0);
};
const inventory = () => ({
  compute: list("instances").length,
  firewall: list("firewall-rules").length,
  scratch: list("disks").length,
});

const client = await defaultMakeKhalaSyncSqlClient(databaseUrl);
const store = managedSandboxBoxV1StoreForEnv(runtimeEnv);
const policy = await Effect.runPromise(managedSandboxBoxV1PolicyForEnv(runtimeEnv));
const runtime = await Effect.runPromise(managedSandboxBoxV1RuntimeForEnv(runtimeEnv));
const principal = {
  actorRef: "principal.owner.live-reconcile",
  ownerRef: ownerUserId,
  tenantRef: ownerUserId,
};
const broker = makeManagedSandboxBroker({ principal, policy, runtime, store });
const before = inventory();
const transitions: Array<Readonly<{ sandboxRefDigest: string; lifecycle: string }>> = [];

const inspect = (sandboxRef: string) =>
  Effect.runPromise(store.inspect({ ownerRef: ownerUserId, tenantRef: ownerUserId, sandboxRef }));

const command = (
  resource: ManagedSandboxResource,
  tag: "Stop" | "Delete",
): ManagedSandboxCommand => {
  const digest = sha256(`${resource.sandboxRef}\n${tag}\n${Date.now()}\n${process.pid}`);
  return {
    schema: "openagents.managed_sandbox_command.v1",
    _tag: tag,
    commandRef: `command.owner.reconcile.${tag.toLowerCase()}.${digest.slice(0, 32)}`,
    idempotencyRef: `idempotency.owner.reconcile.${digest.slice(0, 32)}`,
    requestedByRef: principal.actorRef,
    ownerRef: ownerUserId,
    tenantRef: ownerUserId,
    requestedAt: new Date().toISOString(),
    expectedVersion: resource.version,
    sandboxRef: resource.sandboxRef,
    reasonRef: `reason.owner.reconcile.${tag.toLowerCase()}`,
  } as ManagedSandboxCommand;
};

const settleObservedCleanup = async (
  resource: ManagedSandboxResource,
  commandRef: string,
): Promise<void> => {
  if (resource.facts.lifecycle !== "deleting")
    throw new Error("cleanup settlement requires deleting");
  if (!Object.values(inventory()).every((count) => count === 0)) {
    throw new Error("cleanup settlement requires an independent zero-residue inventory");
  }
  const observedAt = new Date().toISOString();
  const digest = sha256(
    `${resource.sandboxRef}\n${commandRef}\n${resource.lastEventSequence + 1}\ncleanup`,
  );
  await Effect.runPromise(
    store.settle({
      ownerRef: ownerUserId,
      tenantRef: ownerUserId,
      sandboxRef: resource.sandboxRef,
      commandRef,
      expectedResourceGeneration: resource.resourceGeneration,
      events: [
        {
          schema: "openagents.managed_sandbox_event.v1",
          _tag: "CleanupObserved",
          eventRef: `event.owner.reconcile.${digest.slice(0, 40)}`,
          sandboxRef: resource.sandboxRef,
          resourceGeneration: resource.resourceGeneration,
          sequence: resource.lastEventSequence + 1,
          observedAt,
        },
      ],
      outcome: "succeeded",
      artifactRefs: [`artifact.managed-sandbox.zero-residue.${digest.slice(0, 40)}`],
      observedAt,
    }),
  );
};

let failure: string | undefined;
try {
  for (const sandboxRef of sandboxRefs) {
    let resource = await inspect(sandboxRef);
    const activeRows = await client.sql<Array<{ active_command_ref: string | null }>>`
      SELECT active_command_ref
      FROM khala_sync_managed_sandboxes
      WHERE sandbox_ref = ${sandboxRef}
        AND owner_user_id = ${ownerUserId}
        AND tenant_ref = ${ownerUserId}
      LIMIT 1
    `;
    const activeCommandRef = activeRows[0]?.active_command_ref ?? undefined;
    if (activeCommandRef !== undefined) {
      const reservation = await Effect.runPromise(
        store.reservation({
          ownerRef: ownerUserId,
          tenantRef: ownerUserId,
          commandRef: activeCommandRef,
        }),
      );
      if (
        reservation !== undefined &&
        ["provisioning", "stopping", "resuming", "deleting"].includes(resource.facts.lifecycle) &&
        ["Create", "Stop", "Resume", "Delete"].includes(reservation.command._tag)
      ) {
        await Effect.runPromise(
          broker.execute(
            reservation.command,
            reservation.command._tag === "Create"
              ? { attachmentGeneration: reservation.resource.attachmentGeneration }
              : {},
          ),
        ).catch(async (error: unknown) => {
          resource = await inspect(sandboxRef);
          if (reservation.command._tag !== "Delete") throw error;
          await settleObservedCleanup(resource, reservation.command.commandRef);
        });
        resource = await inspect(sandboxRef);
      } else if (
        reservation?.status === "pending" &&
        reservation.command._tag === "Interrupt" &&
        ["failed", "settled"].includes(resource.facts.runtimeState)
      ) {
        const observedAt = new Date().toISOString();
        const eventDigest = sha256(
          `${sandboxRef}\n${reservation.command.commandRef}\n${resource.lastEventSequence + 1}`,
        );
        await Effect.runPromise(
          store.settle({
            ownerRef: ownerUserId,
            tenantRef: ownerUserId,
            sandboxRef,
            commandRef: reservation.command.commandRef,
            expectedResourceGeneration: resource.resourceGeneration,
            events: [
              {
                schema: "openagents.managed_sandbox_event.v1",
                _tag: "OperationFailed",
                eventRef: `event.owner.reconcile.${eventDigest.slice(0, 40)}`,
                sandboxRef,
                resourceGeneration: resource.resourceGeneration,
                sequence: resource.lastEventSequence + 1,
                operationRef: reservation.command.commandRef,
                errorRef: "reason.interrupt_after_terminal",
                observedAt,
              },
            ],
            outcome: "failed",
            errorCode: "reason.interrupt_after_terminal",
            observedAt,
          }),
        );
        resource = await inspect(sandboxRef);
      }
    }
    if (["ready", "idle", "running"].includes(resource.facts.lifecycle)) {
      await Effect.runPromise(broker.execute(command(resource, "Stop")));
      resource = await inspect(sandboxRef);
    }
    if (["stopped", "failed", "recovery_required"].includes(resource.facts.lifecycle)) {
      const deleteCommand = command(resource, "Delete");
      await Effect.runPromise(broker.execute(deleteCommand)).catch(async () => {
        resource = await inspect(sandboxRef);
        await settleObservedCleanup(resource, deleteCommand.commandRef);
      });
      resource = await inspect(sandboxRef);
    }
    if (resource.facts.lifecycle !== "deleted" || !resource.facts.cleanupComplete) {
      throw new Error(`exact reconcile did not delete ${sha256(sandboxRef).slice(0, 16)}`);
    }
    transitions.push({
      sandboxRefDigest: `sha256:${sha256(sandboxRef)}`,
      lifecycle: resource.facts.lifecycle,
    });
  }
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  await client.end().catch(() => undefined);
}

const after = inventory();
const passed = failure === undefined && Object.values(after).every((count) => count === 0);
const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_live_reconcile.v1",
  capturedAt: new Date().toISOString(),
  passed,
  ...(failure === undefined ? {} : { failure }),
  transitions,
  before,
  after,
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(`${JSON.stringify({ passed, evidence, before, after })}\n`);
if (!passed) process.exit(1);
