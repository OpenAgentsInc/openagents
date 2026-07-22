import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import type {
  PortableCommandGrantAuthorityBinding,
  SyncSql,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test, vi } from "vitest";

import {
  makePortableSessionCommandRuntimeAdapters,
  type PortableSessionCommandRuntimeAdapterEnv,
} from "./portable-session-command-runtime-adapters";
import { PORTABLE_SESSION_COMMAND_DISPATCH_FLAG } from "./portable-session-command-dispatch-scheduled";

const sql = Object.assign(
  async () => [],
  { begin: async <A>(run: (transaction: SyncSql) => Promise<A>) => run(sql) },
) as unknown as SyncSql;

const completeEnv = (): PortableSessionCommandRuntimeAdapterEnv => ({
  PORTABLE_SESSION_COMMAND_GRANT_AUTHORITY_BASE_URL:
    "https://grants.openagents.test",
  PORTABLE_SESSION_COMMAND_MANAGED_INSTALLATION_BASE_URL:
    "https://install.openagents.test",
  PORTABLE_SESSION_COMMAND_SERVICE_BEARER: "service-bearer-private-123456",
  ARTIFACTS_GCS_BUCKET: "oa-artifacts-test",
  ARTIFACTS_GCS_ENDPOINT: "https://storage.googleapis.com",
  ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID: "GOOG1EPORTABLETEST",
  ARTIFACTS_GCS_HMAC_SECRET: "gcs-hmac-private-secret-123456",
  PORTABLE_CHECKPOINT_KMS_KEY_RESOURCE:
    "projects/openagents-test/locations/us-central1/keyRings/portable/cryptoKeys/checkpoints",
  PORTABLE_CHECKPOINT_KMS_KEY_REF: "key.portable-checkpoint.test.v1",
});

const descriptor = (
  targetClass: PortableTargetDescriptor["targetClass"],
): PortableTargetDescriptor => ({
  targetRef: `target.ide13.runtime.${targetClass}`,
  targetClass,
  adapterRef: `adapter.ide13.runtime.${targetClass}`,
  ownerRef: "owner.ide13.runtime",
  compatibilityRef: "compatibility.ide13.runtime",
  isolation: "dedicated_microvm",
  dataPosture: "openagents_managed_region",
  health: "ready",
});

const source = descriptor("owner_local");
const commandClaim = (
  destination: PortableTargetDescriptor,
): PortableCommandExecutionClaim => ({
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: `claim.ide13.runtime.${destination.targetClass}`,
  commandRef: "command.ide13.runtime",
  ownerRef: source.ownerRef,
  sessionRef: "session.ide13.runtime",
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.runtime.source",
  sourceGeneration: 1,
  destinationTargetRef: destination.targetRef,
  executorEnvironmentRef: source.targetRef,
  workerInstanceRef: "worker.ide13.runtime",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: "2026-07-20T12:00:00.000Z",
  leaseExpiresAt: "2026-07-20T12:10:00.000Z",
  updatedAt: "2026-07-20T12:00:00.000Z",
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
});

const grantBinding: PortableCommandGrantAuthorityBinding = {
  sourceLeaseRef: "lease.ide13.runtime.source",
  grantRef: "grant.ide13.runtime.source",
  ownerUserId: source.ownerRef,
  kind: "github",
};

const capabilityTransfer = {
  sourceLeaseRef: grantBinding.sourceLeaseRef,
  destinationLeaseRef: "lease.ide13.runtime.destination",
  destinationSourceGrantRef: "grant.ide13.runtime.destination",
  expiresAt: "2026-07-20T12:09:00.000Z",
} as const;

describe("portable session command production runtime adapters", () => {
  test("does not infer or enable the dispatch flag from runtime configuration", () => {
    const env = completeEnv();
    expect(env).not.toHaveProperty(PORTABLE_SESSION_COMMAND_DISPATCH_FLAG);
  });

  test("composes the production broker, destination resolver, GCS reader, and KMS custody", () => {
    const adapters = makePortableSessionCommandRuntimeAdapters(completeEnv(), sql);

    expect(adapters?.brokerFactory).toMatchObject({ create: expect.any(Function) });
    expect(adapters?.checkpointArtifacts.commandResolver()).toMatchObject({
      resolve: expect.any(Function),
    });
  });

  test.each([
    "PORTABLE_SESSION_COMMAND_GRANT_AUTHORITY_BASE_URL",
    "PORTABLE_SESSION_COMMAND_MANAGED_INSTALLATION_BASE_URL",
    "PORTABLE_SESSION_COMMAND_SERVICE_BEARER",
    "ARTIFACTS_GCS_BUCKET",
    "ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID",
    "ARTIFACTS_GCS_HMAC_SECRET",
    "PORTABLE_CHECKPOINT_KMS_KEY_RESOURCE",
    "PORTABLE_CHECKPOINT_KMS_KEY_REF",
  ] as const)("returns undefined when %s is absent", (field) => {
    const env = { ...completeEnv(), [field]: undefined };
    expect(makePortableSessionCommandRuntimeAdapters(env, sql)).toBeUndefined();
  });

  test.each([
    ["PORTABLE_SESSION_COMMAND_GRANT_AUTHORITY_BASE_URL", "http://grants.openagents.test"],
    ["PORTABLE_SESSION_COMMAND_GRANT_AUTHORITY_BASE_URL", "https://grants.openagents.test/"],
    ["PORTABLE_SESSION_COMMAND_MANAGED_INSTALLATION_BASE_URL", "https://user@install.openagents.test"],
    ["ARTIFACTS_GCS_ENDPOINT", "http://storage.googleapis.com"],
  ] as const)("rejects an inexact HTTPS authority in %s", (field, value) => {
    expect(
      makePortableSessionCommandRuntimeAdapters(
        { ...completeEnv(), [field]: value },
        sql,
      ),
    ).toBeUndefined();
  });

  test.each([
    ["PORTABLE_SESSION_COMMAND_SERVICE_BEARER", "short"],
    ["ARTIFACTS_GCS_BUCKET", "Invalid Bucket"],
    ["ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID", "short"],
    ["ARTIFACTS_GCS_HMAC_SECRET", "short"],
    ["PORTABLE_CHECKPOINT_KMS_KEY_RESOURCE", "projects/openagents-test/keys/raw"],
    ["PORTABLE_CHECKPOINT_KMS_KEY_REF", "unsafe key ref"],
  ] as const)("rejects malformed production configuration in %s", (field, value) => {
    expect(
      makePortableSessionCommandRuntimeAdapters(
        { ...completeEnv(), [field]: value },
        sql,
      ),
    ).toBeUndefined();
  });

  test.each(["owner_managed", "managed_provider"] as const)(
    "keeps unsupported %s targets fail-closed",
    async (targetClass) => {
      const adapters = makePortableSessionCommandRuntimeAdapters(completeEnv(), sql);
      if (adapters === undefined) throw new Error("runtime adapters unavailable");
      const destination = descriptor(targetClass);
      await expect(
        adapters.brokerFactory.create({
          claim: commandClaim(destination),
          source,
          destination,
          destinationAttachmentRef: "attachment.ide13.runtime.destination",
          destinationGeneration: 2,
          grantBindings: [grantBinding],
          capabilityTransfers: [capabilityTransfer],
        }),
      ).rejects.toMatchObject({ code: "target_unsupported" });
    },
  );

  test("does not fetch, log, or serialize private configuration during composition", () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const consoleSpies = [
      vi.spyOn(console, "debug"),
      vi.spyOn(console, "info"),
      vi.spyOn(console, "log"),
      vi.spyOn(console, "warn"),
      vi.spyOn(console, "error"),
    ];
    try {
      const env = completeEnv();
      const adapters = makePortableSessionCommandRuntimeAdapters(env, sql);
      const serialized = JSON.stringify(adapters);
      expect(adapters).toBeDefined();
      expect(fetchSpy).not.toHaveBeenCalled();
      for (const spy of consoleSpies) expect(spy).not.toHaveBeenCalled();
      expect(serialized).not.toContain("service-bearer-private-123456");
      expect(serialized).not.toContain("gcs-hmac-private-secret-123456");
      expect(serialized).not.toContain("GOOG1EPORTABLETEST");
    } finally {
      fetchSpy.mockRestore();
      for (const spy of consoleSpies) spy.mockRestore();
    }
  });
});
