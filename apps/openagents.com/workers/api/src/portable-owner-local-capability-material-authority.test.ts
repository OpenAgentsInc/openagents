import { describe, expect, test } from "vitest";

import {
  makePortableOwnerLocalCapabilityMaterialAuthority,
  type PortableOwnerLocalCapabilityMaterialAuthorityDependencies,
} from "./portable-owner-local-capability-material-authority";
import type { PortableOwnerLocalCapabilityMaterialAuthority } from "./portable-owner-local-capability-operation-routes";

const runnerSessionRef = "runner.ide13.destination.1";
const sourceGrantRef = "grant.ide13.source.1";
const destinationGrantRef = "grant.ide13.destination.1";

const authority = (
  capability: PortableOwnerLocalCapabilityMaterialAuthority["capability"] = "provider",
): PortableOwnerLocalCapabilityMaterialAuthority => ({
  schema: "openagents.portable_owner_local_capability_operation.v1",
  operationRef: `operation.owner-local-capability.${"1".repeat(64)}`,
  commandExecutionClaimRef: "claim.ide13.command.1",
  claimRef: "claim.ide13.operation.1",
  actorAgentUserId: "agent.ide13.owner.1",
  ownerRef: "owner.ide13.1",
  pylonRef: "pylon.ide13.1",
  targetRef: "target.ide13.destination.1",
  sessionRef: "session.ide13.1",
  attachmentRef: "attachment.ide13.destination.1",
  attachmentGeneration: 2,
  workerInstanceRef: "worker.ide13.1",
  claimGeneration: 1,
  expectedLeaseRevision: 1,
  expectedLeaseExpiresAt: "2026-07-20T15:05:00.000Z",
  destinationGrantRef,
  sourceGrantRef,
  capability,
  permissionRefs: ["permission.provider.use"],
  operationExpiresAt: "2026-07-20T15:05:00.000Z",
});

const providerGrant = (overrides: Readonly<Record<string, unknown>> = {}) => ({
  grantRef: destinationGrantRef,
  userId: "owner.ide13.1",
  provider: "chatgpt-codex",
  providerAccountRef: "provider.account.ide13.1",
  runnerSessionId: runnerSessionRef,
  requestedAction: "portable_session_resume",
  metadataJson: JSON.stringify({ reissuedFromGrantRef: sourceGrantRef }),
  status: "issued",
  expiresAt: "2026-07-20T15:05:00.000Z",
  ...overrides,
});

const githubGrant = (overrides: Readonly<Record<string, unknown>> = {}) => ({
  grantRef: destinationGrantRef,
  userId: "owner.ide13.1",
  connectionRef: "github.connection.ide13.1",
  secretRef: "github.secret.ide13.1",
  runnerSessionId: runnerSessionRef,
  requestedAction: "portable_session_resume",
  metadataJson: JSON.stringify({ reissuedFromGrantRef: sourceGrantRef }),
  status: "issued",
  expiresAt: "2026-07-20T15:05:00.000Z",
  ...overrides,
});

const dependencies = (
  overrides: Partial<PortableOwnerLocalCapabilityMaterialAuthorityDependencies> = {},
): PortableOwnerLocalCapabilityMaterialAuthorityDependencies => ({
  recheckAuthority: async () => ({ destinationRunnerSessionRef: runnerSessionRef }),
  readProviderGrant: async () => providerGrant(),
  resolveProviderGrant: async () => ({
    grantRef: destinationGrantRef,
    ownerUserId: "owner.ide13.1",
    providerAccountRef: "provider.account.ide13.1",
    runnerSessionId: runnerSessionRef,
    requestedAction: "portable_session_resume",
    status: "used",
  }),
  readProviderMaterial: async () => new TextEncoder().encode("provider-private"),
  readGitHubGrant: async () => githubGrant(),
  resolveGitHubGrant: async () => ({
    grantRef: destinationGrantRef,
    connectionRef: "github.connection.ide13.1",
    runnerSessionId: runnerSessionRef,
    requestedAction: "portable_session_resume",
    scopes: ["repo"],
  }),
  readGitHubConnection: async () => ({
    connectionRef: "github.connection.ide13.1",
    secretRef: "github.secret.ide13.1",
    scopes: ["repo"],
  }),
  readGitHubMaterial: async () => new TextEncoder().encode("github-private"),
  githubScopesSatisfy: scopes => scopes.includes("repo"),
  providerKind: "chatgpt-codex",
  now: () => new Date("2026-07-20T15:00:00.000Z"),
  ...overrides,
});

describe("owner-local capability material authority", () => {
  test("resolves exact provider material with the authenticated Pylon actor", async () => {
    const calls: Array<string> = [];
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority(),
      dependencies({
        resolveProviderGrant: async input => {
          calls.push(input.actorAgentUserId);
          return {
            grantRef: input.grantRef,
            ownerUserId: "owner.ide13.1",
            providerAccountRef: input.providerAccountRef,
            runnerSessionId: input.runnerSessionRef,
            requestedAction: "portable_session_resume",
            status: "used",
          };
        },
      }),
    );
    expect(new TextDecoder().decode(await redeem())).toBe("provider-private");
    expect(calls).toEqual(["agent.ide13.owner.1"]);
  });

  test("resolves exact GitHub material only with admitted scopes", async () => {
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority("scm_write"),
      dependencies(),
    );
    expect(new TextDecoder().decode(await redeem())).toBe("github-private");
  });

  test.each([
    ["owner", { userId: "owner.ide13.other" }],
    ["runner session", { runnerSessionId: "runner.ide13.other" }],
    ["source ancestry", { metadataJson: JSON.stringify({ reissuedFromGrantRef: "grant.other" }) }],
  ])("refuses a provider grant with wrong %s", async (_label, override) => {
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority(),
      dependencies({ readProviderGrant: async () => providerGrant(override) }),
    );
    await expect(redeem()).rejects.toThrow("provider destination grant scope is invalid");
  });

  test("refuses GitHub material when connection scopes drift", async () => {
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority("scm_read"),
      dependencies({
        readGitHubConnection: async () => ({
          connectionRef: "github.connection.ide13.1",
          secretRef: "github.secret.ide13.1",
          scopes: ["read:user"],
        }),
      }),
    );
    await expect(redeem()).rejects.toThrow("GitHub destination connection scope is invalid");
  });

  test("refuses tool and API kinds without a typed materializer", async () => {
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority("tool"),
      dependencies(),
    );
    await expect(redeem()).rejects.toThrow("capability kind has no admitted material authority");
  });

  test("zeroes material when final authority differs after custody read", async () => {
    let checks = 0;
    const material = new TextEncoder().encode("zero-on-drift");
    const redeem = makePortableOwnerLocalCapabilityMaterialAuthority(
      authority(),
      dependencies({
        recheckAuthority: async () => ({
          destinationRunnerSessionRef:
            ++checks === 1 ? runnerSessionRef : "runner.ide13.replaced",
        }),
        readProviderMaterial: async () => material,
      }),
    );
    await expect(redeem()).rejects.toThrow("destination runner session authority changed");
    expect(material.every(byte => byte === 0)).toBe(true);
  });
});
