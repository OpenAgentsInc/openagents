import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH,
  MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH,
  makeManagedSandboxDesktopRoutes,
} from "./managed-sandbox-desktop-routes";
import type { BoxV1Policy } from "./managed-sandbox-box-v1-routes";

const now = "2026-07-19T16:00:00.000Z";
const imageDigest = `sha256:${"d".repeat(64)}`;
const policy: BoxV1Policy = {
  target: {
    targetRef: "target.gcp.managed-sandbox.us-central1",
    targetClass: "openagents_managed",
    provider: "google_cloud",
    adapterRef: "adapter.oa-codex-control.gce.v1",
    region: "us-central1",
    isolation: "gce_vm",
    dataPosture: "openagents_managed_region",
  },
  imageDigest,
  profileRef: "profile.sbx.gce.e2-small.v1",
  defaultTtlSeconds: 3_600,
  maxTtlSeconds: 86_400,
  maxActiveBoxes: 2,
  maxCostMicros: 10_000,
  maxCpuMillis: 3_600_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
};

const body = {
  schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
  attachment: {
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    agentAttachmentRef: "ide.agent-attachment.fixture",
    projectRef: "ide.project.fixture",
    rootRef: "ide.root.fixture",
    worktreeRef: "ide.worktree.fixture",
    sessionRef: "ide.session.fixture",
    attachmentGeneration: 1,
    placementGeneration: 1,
    grantRef: "grant.ide.fixture",
    attachedAt: now,
    expiresAt: null,
  },
};

const context = {} as ExecutionContext;

const routes = (enabled: boolean, authenticated = true) =>
  makeManagedSandboxDesktopRoutes({
    authenticateOwner: async () => (authenticated ? { userId: "owner.fixture" } : undefined),
    enabled: () => enabled,
    policy: () => Effect.succeed(policy),
    store: () => {
      throw new Error("store must not run during admission or a disabled command");
    },
    runtime: () => {
      throw new Error("runtime must not run during admission or a disabled command");
    },
    now: () => new Date(now),
  });

describe("managed-sandbox Desktop broker routes", () => {
  test("uses the exact endpoints expected by the main-owned Desktop host", () => {
    expect(MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH).toBe("/api/managed-sandboxes/desktop/admission");
    expect(MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH).toBe("/api/managed-sandboxes/desktop/commands");
  });

  test("returns typed unavailable while rollout is default-off", async () => {
    const response = await Effect.runPromise(
      routes(false).admission(
        new Request(`https://openagents.com${MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH}`, {
          method: "POST",
          body: JSON.stringify(body),
        }),
        {},
        context,
      ),
    );
    expect(response.status).toBe(200);
    await expect(response.json()).resolves.toMatchObject({
      admission: { _tag: "Unavailable" },
    });
  });

  test("returns only exact admitted GCP policy when enabled and authenticated", async () => {
    const response = await Effect.runPromise(
      routes(true).admission(
        new Request(`https://openagents.com${MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH}`, {
          method: "POST",
          body: JSON.stringify(body),
        }),
        {},
        context,
      ),
    );
    expect(response.status).toBe(200);
    await expect(response.json()).resolves.toMatchObject({
      admission: {
        _tag: "Available",
        target: policy.target,
        imageDigest,
        profileRef: policy.profileRef,
        networkPosture: "deny_all",
      },
    });
  });

  test("rejects unauthenticated admission and disabled mutation before target effects", async () => {
    const unauthorized = await Effect.runPromise(
      routes(true, false).admission(
        new Request(`https://openagents.com${MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH}`, {
          method: "POST",
          body: JSON.stringify(body),
        }),
        {},
        context,
      ),
    );
    expect(unauthorized.status).toBe(401);

    const disabled = await Effect.runPromise(
      routes(false).commands(
        new Request(`https://openagents.com${MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH}`, {
          method: "POST",
          body: JSON.stringify({}),
        }),
        {},
        context,
      ),
    );
    expect(disabled.status).toBe(503);
    await expect(disabled.json()).resolves.toMatchObject({
      error: "runtime_not_admitted",
    });
  });
});
