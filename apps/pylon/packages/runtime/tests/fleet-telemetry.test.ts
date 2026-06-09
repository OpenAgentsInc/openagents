import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  canSelectProviderAccountForLease,
  CHATGPT_CODEX_PROVIDER,
  makeProbeAuthHealthSignal,
  makeStaticProbeFleetTelemetryClient,
  recordProbeAuthHealthSignal,
  type PublicProviderAccount,
} from "../src";

const providerAccountRef = "provider-account_primary" as PublicProviderAccount["providerAccountRef"];

const account = (overrides: Partial<PublicProviderAccount> = {}): PublicProviderAccount => ({
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef,
  authMode: "chatgpt_device_code",
  status: "connected",
  health: "healthy",
  secretRef: "codex-auth://provider-account_primary" as PublicProviderAccount["secretRef"],
  ...overrides,
});

describe("Probe fleet telemetry", () => {
  test("reports rate-limit failure and requests Omega failover", async () => {
    const telemetry = makeStaticProbeFleetTelemetryClient();
    const receipt = await Effect.runPromise(
      recordProbeAuthHealthSignal(
        telemetry.client,
        makeProbeAuthHealthSignal({
          providerAccountRef,
          authGrantRef: "provider-auth-grant_1" as never,
          leaseRef: "lease_1",
          assignmentId: "assignment_1",
          runnerSessionId: "runner_session_1",
          outcome: "failure",
          failureClass: "rate_limited",
          observedAt: "2026-06-07T00:00:00.000Z",
        }),
      ),
    );

    expect(telemetry.reported).toHaveLength(1);
    expect(telemetry.failovers).toHaveLength(1);
    expect(receipt).toMatchObject({
      kind: "probe_auth_failover_requested",
      failureClass: "rate_limited",
      contentRedacted: true,
    });
    expect(telemetry.reported[0]?.patch).toMatchObject({
      health: "unhealthy",
      cooldownReason: "rate_limited",
    });
  });

  test("maps reauth failures to requires_reauth", async () => {
    const telemetry = makeStaticProbeFleetTelemetryClient();

    await Effect.runPromise(
      recordProbeAuthHealthSignal(
        telemetry.client,
        makeProbeAuthHealthSignal({
          providerAccountRef,
          leaseRef: "lease_1",
          outcome: "failure",
          failureClass: "refresh_failed",
          observedAt: "2026-06-07T00:00:00.000Z",
        }),
      ),
    );

    expect(telemetry.reported[0]?.patch).toMatchObject({
      health: "requires_reauth",
      status: "expired",
      reauthRequiredReason: "refresh_failed",
    });
    expect(telemetry.failovers).toHaveLength(1);
  });

  test("maps low-credit failures to low-credit fleet state", async () => {
    const telemetry = makeStaticProbeFleetTelemetryClient();

    await Effect.runPromise(
      recordProbeAuthHealthSignal(
        telemetry.client,
        makeProbeAuthHealthSignal({
          providerAccountRef,
          leaseRef: "lease_1",
          outcome: "failure",
          failureClass: "low_credit",
          observedAt: "2026-06-07T00:00:00.000Z",
        }),
      ),
    );

    expect(telemetry.reported[0]?.patch).toMatchObject({
      health: "unhealthy",
      lowCredit: true,
      cooldownReason: "low_credit",
    });
  });

  test("does not request failover for successful or non-auth outcomes", async () => {
    const telemetry = makeStaticProbeFleetTelemetryClient();

    await Effect.runPromise(
      recordProbeAuthHealthSignal(
        telemetry.client,
        makeProbeAuthHealthSignal({
          providerAccountRef,
          leaseRef: "lease_1",
          outcome: "success",
          observedAt: "2026-06-07T00:00:00.000Z",
        }),
      ),
    );

    await Effect.runPromise(
      recordProbeAuthHealthSignal(
        telemetry.client,
        makeProbeAuthHealthSignal({
          providerAccountRef,
          leaseRef: "lease_1",
          outcome: "failure",
          failureClass: "non_auth_failure",
          observedAt: "2026-06-07T00:00:01.000Z",
        }),
      ),
    );

    expect(telemetry.failovers).toHaveLength(0);
  });

  test("lease eligibility skips low-credit, cooldown, and unhealthy accounts", () => {
    const now = new Date("2026-06-07T00:00:00.000Z");

    expect(canSelectProviderAccountForLease(account(), now)).toBe(true);
    expect(canSelectProviderAccountForLease(account({ lowCredit: true }), now)).toBe(false);
    expect(canSelectProviderAccountForLease(account({ health: "requires_reauth" }), now)).toBe(false);
    expect(canSelectProviderAccountForLease(account({ cooldownUntil: "2099-01-01T00:00:00.000Z" }), now)).toBe(false);
    expect(canSelectProviderAccountForLease(account({ leaseLimit: 0 }), now)).toBe(false);
  });

  test("rejects raw credential material in telemetry metadata", async () => {
    const telemetry = makeStaticProbeFleetTelemetryClient();

    await expect(
      Effect.runPromise(
        recordProbeAuthHealthSignal(
          telemetry.client,
          makeProbeAuthHealthSignal({
            providerAccountRef,
            outcome: "failure",
            failureClass: "provider_unavailable",
            observedAt: "2026-06-07T00:00:00.000Z",
            metadata: {
              access_token: "raw-token",
            },
          }),
        ),
      ),
    ).rejects.toMatchObject({ _tag: "ProbePublicProjectionUnsafe" });
  });
});
