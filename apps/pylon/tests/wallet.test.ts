import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state"
import {
  admitPayoutTarget,
  appendLedgerEvent,
  classifyMdkReceiveFailure,
  describeMdkPortConflict,
  DEFAULT_MDK_WALLET_PORT,
  classifyMdkWallet,
  classifySparkBackupReceive,
  mdkScopedAgentWalletStatus,
  detectSparkBackupBalance,
  isSparkBackupDefaultEnabled,
  preflightLegacySparkMigration,
  prepareSparkBackupReceive,
  projectWalletBalance,
  readCachedSparkTarget,
  receiveWithFallback,
  receiveWithMdk,
  recommendSparkSweep,
  recoverLegacyMdkBalance,
  reclaimStaleMdkDaemonPidfile,
  registerSparkPayoutTarget,
  reportWalletReadiness,
  requestPayoutTargetAdmission,
  sparkPayoutTargetRef,
  sendWithMdk,
  sendWithSparkBackup,
  sparkBackupTargetPath,
  sweepSparkBackupToMdk,
  withPayoutTargetReadiness,
  withUnifiedWalletBalance,
  withSparkPrimaryWalletBalance,
  writeCachedSparkTarget,
  type SparkBackupCommand,
  type SparkBackupHelper,
  type SparkBackupSendTransfer,
  type SparkBackupSweepTransfer,
  type WalletCommandRunner,
} from "../src/wallet"

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-wallet-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

const runner =
  (responses: Record<string, { exitCode?: number; stdout?: unknown; stderr?: string }>): WalletCommandRunner =>
  async (args) => {
    const key = args.join(" ")
    const response = responses[key] ?? { exitCode: 1, stderr: `unexpected command: ${key}` }
    return {
      exitCode: response.exitCode ?? 0,
      stdout: typeof response.stdout === "string" ? response.stdout : JSON.stringify(response.stdout ?? {}),
      stderr: response.stderr ?? "",
    }
  }

describe("payout-target readiness visibility (Gap #2)", () => {
  // Helper: a default/unconfigured Spark-primary status (daemon offline, no
  // payout target) — the shape `wallet status` reports for a fresh node.
  const unconfiguredStatus = () => withSparkPrimaryWalletBalance(mdkScopedAgentWalletStatus())

  test("surfaces the unregistered-payout-target blocker when payoutTargetRefs is empty (default/unconfigured branch)", () => {
    const status = withPayoutTargetReadiness(unconfiguredStatus(), [])
    expect(status.payoutTargetRefs).toEqual([])
    expect(status.blockerRefs).toContain("blocker.wallet.payout_target_unregistered")
  })

  test("surfaces the unregistered-payout-target blocker on the LIVE send-ready branch too (Orrery case)", async () => {
    // A wallet can be send-ready yet have NO payout target registered.
    const mdkStatus = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )
    expect(mdkStatus.sendReady).toBe(true)
    const status = withPayoutTargetReadiness(mdkStatus, [])
    expect(status.sendReady).toBe(true)
    expect(status.payoutTargetRefs).toEqual([])
    expect(status.blockerRefs).toContain("blocker.wallet.payout_target_unregistered")
  })

  test("does NOT add the blocker and populates payoutTargetRefs when a target is registered", () => {
    const status = withPayoutTargetReadiness(unconfiguredStatus(), [
      "payout.spark.deadbeefdeadbeefdeadbeef",
    ])
    expect(status.payoutTargetRefs).toEqual(["payout.spark.deadbeefdeadbeefdeadbeef"])
    expect(status.blockerRefs).not.toContain("blocker.wallet.payout_target_unregistered")
  })

  test("ignores empty/whitespace/null refs and de-duplicates registered refs", () => {
    const status = withPayoutTargetReadiness(unconfiguredStatus(), [
      null,
      "   ",
      "payout.spark.deadbeefdeadbeefdeadbeef",
      "payout.spark.deadbeefdeadbeefdeadbeef",
      undefined,
    ])
    expect(status.payoutTargetRefs).toEqual(["payout.spark.deadbeefdeadbeefdeadbeef"])
    expect(status.blockerRefs).not.toContain("blocker.wallet.payout_target_unregistered")
  })

  test("the resulting projection is public-safe (redacted refs only)", () => {
    const status = withPayoutTargetReadiness(unconfiguredStatus(), [
      "payout.spark.deadbeefdeadbeefdeadbeef",
    ])
    expect(() => assertPublicProjectionSafe(status)).not.toThrow()
    expect(JSON.stringify(status)).not.toContain("spark1")
  })

  test("rejects a raw Spark address smuggled into payoutTargetRefs", () => {
    expect(() =>
      withPayoutTargetReadiness(unconfiguredStatus(), [
        "spark1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
      ]),
    ).toThrow()
  })
})

describe("MDK wallet readiness and ledger", () => {
  test("reclaims stale MDK daemon pidfiles before wallet commands", async () => {
    await withTempHome(async (home) => {
      const pidfile = join(home, ".mdk-wallet", "daemon.pid")
      await mkdir(join(home, ".mdk-wallet"), { recursive: true })
      await Bun.write(pidfile, JSON.stringify({ pid: 424242, port: 3456 }))

      const result = await reclaimStaleMdkDaemonPidfile({
        homeDir: home,
        isProcessLive: () => false,
      })

      expect(result).toMatchObject({
        pid: 424242,
        reason: "dead_pid",
        reclaimed: true,
      })
      expect(await Bun.file(pidfile).exists()).toBe(false)
    })
  })

  test("keeps MDK daemon pidfiles for live processes", async () => {
    await withTempHome(async (home) => {
      const pidfile = join(home, ".mdk-wallet", "daemon.pid")
      await mkdir(join(home, ".mdk-wallet"), { recursive: true })
      await Bun.write(pidfile, JSON.stringify({ pid: 424242, port: 3456 }))

      const result = await reclaimStaleMdkDaemonPidfile({
        homeDir: home,
        isProcessLive: () => true,
      })

      expect(result).toMatchObject({
        pid: 424242,
        reason: "live_pid",
        reclaimed: false,
      })
      expect(await Bun.file(pidfile).exists()).toBe(true)
    })
  })

  test("reclaims malformed MDK daemon pidfiles", async () => {
    await withTempHome(async (home) => {
      const pidfile = join(home, ".mdk-wallet", "daemon.pid")
      await mkdir(join(home, ".mdk-wallet"), { recursive: true })
      await Bun.write(pidfile, "{not-json")

      const result = await reclaimStaleMdkDaemonPidfile({ homeDir: home })

      expect(result).toMatchObject({
        pid: null,
        reason: "invalid_json",
        reclaimed: true,
      })
      expect(await Bun.file(pidfile).exists()).toBe(false)
    })
  })

  test("classifies daemon offline and unknown balance separately", async () => {
    const offline = await classifyMdkWallet(runner({ balance: { exitCode: 1, stderr: "daemon unavailable" } }))
    const unknown = await classifyMdkWallet(runner({ balance: { stdout: { ok: true } } }))

    expect(offline.readiness).toBe("daemon-offline")
    expect(offline.balanceSats).toBeNull()
    expect(offline.sendReady).toBe(false)
    expect(unknown.readiness).toBe("balance-unknown")
    expect(unknown.receiveReady).toBe(false)
  })

  test("classifies receive-ready without overclaiming send readiness", async () => {
    const status = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, restored_mnemonic_only: true, outbound_capacity_sats: 0 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )

    expect(status.balanceSats).toBe(123)
    expect(status.receiveReady).toBe(true)
    expect(status.sendReady).toBe(false)
    expect(status.readiness).toBe("send-ready-blocked")
    expect(status.blockerRefs).toContain("blocker.wallet.send_readiness_unproven")
    expect(status.blockerRefs).toContain("blocker.wallet.mnemonic_only_restore_not_send_ready")
    expect(status.unifiedBalance).toMatchObject({
      mdkSpendableSats: 123,
      sparkBackupClaimableSats: null,
      sparkBackupCreditedSats: null,
      sparkBackupPendingSweepSats: 0,
      totalVisibleSats: 123,
    })
    expect(status.unifiedBalance.caveatRefs).toContain(
      "caveat.wallet.total_visible_is_not_single_spendable_balance",
    )
    expect(status.unifiedBalance.caveatRefs).toContain(
      "caveat.wallet.mdk_send_readiness_tracked_separately",
    )
    expect(status.sendReadinessPreflight).toMatchObject({
      mode: "mnemonic-only-restore",
      outboundCapacityKnown: true,
      outboundCapacityPositive: false,
      portConfigured: true,
      sendReady: false,
    })
  })

  test("requires explicit MDK_WALLET_PORT before classifying send-ready", async () => {
    const withoutPort = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      {} as NodeJS.ProcessEnv,
    )
    const withPort = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )

    expect(withoutPort.sendReady).toBe(false)
    expect(withoutPort.sendReadinessPreflight.portIsolationRef).toBe("mdk.port.default_possible_crosstalk")
    expect(withoutPort.blockerRefs).toContain("blocker.wallet.mdk_port_unset")
    expect(withPort.sendReady).toBe(true)
    expect(withPort.readiness).toBe("send-ready")
    expect(withPort.unifiedBalance).toMatchObject({
      mdkSpendableSats: 123,
      totalVisibleSats: 123,
    })
    expect(withPort.unifiedBalance.caveatRefs).not.toContain(
      "caveat.wallet.mdk_send_readiness_tracked_separately",
    )
    expect(withPort.sendReadinessPreflight).toMatchObject({
      mode: "original-wallet-home",
      outboundCapacityKnown: true,
      outboundCapacityPositive: true,
      portConfigured: true,
      sendReady: true,
    })
  })

  test("turns v0.2.5 Spark missing Breez API key into an actionable migration blocker", async () => {
    await withTempHome(async (home) => {
      const identityMnemonicPath = join(home, "identity.mnemonic")
      await Bun.write(identityMnemonicPath, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n")
      const preflight = await preflightLegacySparkMigration({
        dryRun: true,
        env: {
          PYLON_LEGACY_SPARK_BALANCE_SATS: "4242",
        } as NodeJS.ProcessEnv,
        helperRunner: runner({
          status: {
            exitCode: 1,
            stderr: "Missing Breez API key",
          },
        }),
        identityMnemonicPath,
      })

      expect(preflight).toMatchObject({
        state: "blocked",
        dryRun: true,
        legacyBalanceDetected: true,
        legacySpendableBalanceSats: 4242,
        helperInitReady: false,
        legacyCredentialReady: false,
        mnemonicBackedRecoveryReady: false,
        migrationRecommended: false,
        recoveryMode: "unavailable",
      })
      expect(preflight.guidedRecovery).toMatchObject({
        localRecoveryAvailable: true,
        localRecoverySelected: false,
        destinationState: "not-ready",
        consentState: "required",
      })
      expect(preflight.guidedRecovery.userFacingAnswer).toContain("No manual Breez credential")
      expect(preflight.guidedRecovery.nextStepSummary).toContain("Prepare the new wallet destination")
      expect(preflight.blockerRefs).toContain("blocker.wallet.legacy_spark.breez_api_key_missing")
      expect(preflight.blockerRefs).toContain("blocker.wallet.legacy_spark.helper_init_failed")
      expect(preflight.nextActionRefs).toContain(
        "action.wallet.legacy_spark.rerun_with_mnemonic_recovery_local_only",
      )
      assertPublicProjectionSafe(preflight)
    })
  })

  test("uses local mnemonic recovery instead of blocking on missing Breez credential when requested", async () => {
    await withTempHome(async (home) => {
      const identityMnemonicPath = join(home, "identity.mnemonic")
      await Bun.write(identityMnemonicPath, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n")
      const preflight = await preflightLegacySparkMigration({
        destinationInvoiceReady: true,
        dryRun: true,
        env: {
          PYLON_LEGACY_SPARK_BALANCE_SATS: "4242",
        } as NodeJS.ProcessEnv,
        helperRunner: runner({
          status: {
            exitCode: 1,
            stderr: "Missing Breez API key",
          },
        }),
        identityMnemonicPath,
        mnemonicRecoveryRequested: true,
      })

      expect(preflight).toMatchObject({
        state: "consent-required",
        dryRun: true,
        helperInitReady: true,
        legacyCredentialReady: true,
        mnemonicBackedRecoveryReady: true,
        migrationRecommended: true,
        recoveryMode: "local-recovery",
      })
      expect(preflight.guidedRecovery).toMatchObject({
        localRecoveryAvailable: true,
        localRecoverySelected: true,
        destinationState: "ready",
        consentState: "required",
      })
      expect(preflight.guidedRecovery.nextStepSummary).toContain("explicit consent")
      expect(preflight.blockerRefs).not.toContain("blocker.wallet.legacy_spark.breez_api_key_missing")
      expect(preflight.nextActionRefs).toEqual([
        "action.wallet.legacy_spark.review_private_local_recovery_plan",
        "action.wallet.legacy_spark.review_and_confirm_migrate_spark_yes",
      ])
      assertPublicProjectionSafe(preflight)
    })
  })

  test("keeps mnemonic recovery blocked until a destination invoice is prepared", async () => {
    const preflight = await preflightLegacySparkMigration({
      dryRun: true,
      env: {
        PYLON_LEGACY_SPARK_BALANCE_SATS: "4242",
      } as NodeJS.ProcessEnv,
      helperRunner: runner({
        status: {
          exitCode: 1,
          stderr: "Missing Breez API key",
        },
      }),
      mnemonicRecoveryRequested: true,
    })

    expect(preflight).toMatchObject({
      state: "blocked",
      destinationInvoiceReady: false,
      mnemonicBackedRecoveryReady: true,
      recoveryMode: "local-recovery",
    })
    expect(preflight.blockerRefs).toContain("blocker.wallet.legacy_spark.destination_invoice_not_ready")
    expect(preflight.nextActionRefs).toContain("action.wallet.legacy_spark.prepare_mdk_destination_invoice")
    assertPublicProjectionSafe(preflight)
  })

  test("requires consent before executing mnemonic-only legacy Spark recovery", async () => {
    const baseOptions = {
      destinationInvoiceReady: true,
      env: {
        PYLON_LEGACY_SPARK_BALANCE_SATS: "4242",
        PYLON_LEGACY_SPARK_UNCLAIMED_DEPOSIT_COUNT: "1",
      } as NodeJS.ProcessEnv,
      helperRunner: runner({
        status: {
          exitCode: 1,
          stderr: "Missing Breez API key",
        },
      }),
      mnemonicRecoveryRequested: true,
    }

    const consentRequired = await preflightLegacySparkMigration({
      ...baseOptions,
      dryRun: false,
    })
    const migrated = await preflightLegacySparkMigration({
      ...baseOptions,
      dryRun: false,
      now: () => new Date("2026-06-10T13:00:00.000Z"),
      yes: true,
    })

    expect(consentRequired).toMatchObject({
      state: "consent-required",
      explicitConsentRequired: true,
      publicReceiptRefs: [],
      recoveryMode: "local-recovery",
    })
    expect(migrated).toMatchObject({
      state: "migrated",
      explicitConsentRequired: false,
      recoveryMode: "local-recovery",
    })
    expect(migrated.guidedRecovery).toMatchObject({
      localRecoverySelected: true,
      destinationState: "ready",
      consentState: "accepted",
      nextStepSummary: "Migration completed with public-safe receipt refs only.",
    })
    expect(migrated.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.legacy_spark_migration\.[a-f0-9]{24}$/)
    assertPublicProjectionSafe(migrated)
  })

  test("requires explicit consent before a ready legacy Spark migration can execute", async () => {
    await withTempHome(async (home) => {
      const identityMnemonicPath = join(home, "identity.mnemonic")
      await Bun.write(identityMnemonicPath, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\n")
      const baseOptions = {
        destinationInvoiceReady: true,
        env: {
          OPENAGENTS_SPARK_API_KEY: "redacted-test-key",
        } as NodeJS.ProcessEnv,
        helperRunner: runner({
          status: {
            stdout: {
              balance_sats: 5000,
              unclaimed_deposit_count: 0,
            },
          },
        }),
        identityMnemonicPath,
      }

      const ready = await preflightLegacySparkMigration({
        ...baseOptions,
        dryRun: true,
      })
      const migrated = await preflightLegacySparkMigration({
        ...baseOptions,
        dryRun: false,
        now: () => new Date("2026-06-10T12:00:00.000Z"),
        yes: true,
      })

      expect(ready).toMatchObject({
        state: "consent-required",
        explicitConsentRequired: true,
        helperInitReady: true,
        legacyCredentialReady: true,
        migrationRecommended: true,
      })
      expect(ready.nextActionRefs).toContain("action.wallet.legacy_spark.review_and_confirm_migrate_spark_yes")
      expect(migrated).toMatchObject({
        state: "migrated",
        explicitConsentRequired: false,
        migrationRecommended: false,
      })
      expect(migrated.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.legacy_spark_migration\.[a-f0-9]{24}$/)
      assertPublicProjectionSafe(migrated)
    })
  })

  test("requires explicit consent before residual legacy MDK balance recovery can execute", async () => {
    const calls: string[] = []
    const fake: WalletCommandRunner = async (args) => {
      calls.push(args.join(" "))
      if (args[0] === "balance") {
        return { exitCode: 0, stdout: JSON.stringify({ balance_sats: 2500 }), stderr: "" }
      }
      return { exitCode: 1, stdout: "", stderr: `unexpected command: ${args.join(" ")}` }
    }

    const recovery = await recoverLegacyMdkBalance({
      destination: "local-mdk-recovery-destination-token",
      dryRun: false,
      runner: fake,
    })

    expect(recovery).toMatchObject({
      schema: "openagents.pylon.legacy_mdk_recovery.v0.1",
      state: "consent-required",
      legacyBalanceDetected: true,
      legacyBalanceSats: 2500,
      explicitConsentRequired: true,
      primaryRailReenabled: false,
      publicReceiptRefs: [],
    })
    expect(calls).toEqual(["balance"])
    expect(recovery.destinationRef).toMatch(/^wallet\.legacy_mdk_recovery\.destination\.[a-f0-9]{24}$/)
    expect(JSON.stringify(recovery)).not.toContain("local-mdk-recovery-destination-token")
    assertPublicProjectionSafe(recovery)
  })

  test("recovers residual legacy MDK balance only through the explicit local recovery command", async () => {
    const calls: string[] = []
    const fake: WalletCommandRunner = async (args) => {
      calls.push(args.join(" "))
      if (args[0] === "balance") {
        return { exitCode: 0, stdout: JSON.stringify({ balance_sats: 2500 }), stderr: "" }
      }
      if (args[0] === "send") {
        return {
          exitCode: 0,
          stdout: JSON.stringify({ local_helper_detail: "sensitive-helper-output" }),
          stderr: "",
        }
      }
      return { exitCode: 1, stdout: "", stderr: `unexpected command: ${args.join(" ")}` }
    }

    const recovery = await recoverLegacyMdkBalance({
      amountSats: 2000,
      destination: "local-mdk-recovery-destination-token",
      dryRun: false,
      now: () => new Date("2026-06-26T12:00:00.000Z"),
      runner: fake,
      yes: true,
    })

    expect(calls).toEqual([
      "balance",
      "send local-mdk-recovery-destination-token 2000",
    ])
    expect(recovery).toMatchObject({
      state: "recovered",
      explicitConsentRequired: false,
      primaryRailReenabled: false,
      requestedAmountSats: 2000,
      failureRefs: [],
    })
    expect(recovery.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.legacy_mdk_recovery\.[a-f0-9]{24}$/)
    const body = JSON.stringify(recovery)
    expect(body).not.toContain("local-mdk-recovery-destination-token")
    expect(body).not.toContain("sensitive-helper-output")
    assertPublicProjectionSafe(recovery)
  })

  test("redacts legacy MDK recovery send failures", async () => {
    const recovery = await recoverLegacyMdkBalance({
      destination: "local-mdk-recovery-destination-token",
      dryRun: false,
      runner: runner({
        balance: { stdout: { balance_sats: 2500 } },
        "send local-mdk-recovery-destination-token": {
          exitCode: 1,
          stderr: "failed with sensitive local helper detail",
        },
      }),
      yes: true,
    })

    expect(recovery.state).toBe("recovery-failed")
    expect(recovery.failureRefs[0]).toMatch(/^wallet\.legacy_mdk_recovery\.failure\.[a-f0-9]{24}$/)
    const body = JSON.stringify(recovery)
    expect(body).not.toContain("local-mdk-recovery-destination-token")
    expect(body).not.toContain("sensitive local helper detail")
    assertPublicProjectionSafe(recovery)
  })

  test("blocks invalid residual legacy MDK recovery amounts", async () => {
    const recovery = await recoverLegacyMdkBalance({
      amountSats: 0,
      destination: "local-mdk-recovery-destination-token",
      runner: runner({
        balance: { stdout: { balance_sats: 2500 } },
      }),
      yes: true,
    })

    expect(recovery.state).toBe("blocked")
    expect(recovery.blockerRefs).toContain("blocker.wallet.legacy_mdk.amount_invalid")
    expect(recovery.nextActionRefs).toContain("action.wallet.legacy_mdk.provide_positive_recovery_amount")
    assertPublicProjectionSafe(recovery)
  })

  test("admits only public-safe payout target refs", () => {
    expect(admitPayoutTarget({ kind: "bolt12_offer", ref: "payout.bolt12.abc123" })).toEqual({
      kind: "bolt12_offer",
      payoutTargetRef: "payout.bolt12.abc123",
      readiness: "payout-target-admitted",
    })
    expect(() => admitPayoutTarget({ kind: "bolt11_invoice", ref: "lnbc10n1rawinvoice" })).toThrow("public-safe")
  })

  test("admits a spark_address payout target only as a payout.spark.* ref (#5252)", () => {
    expect(admitPayoutTarget({ kind: "spark_address", ref: "payout.spark.deadbeefdeadbeefdeadbeef" })).toEqual({
      kind: "spark_address",
      payoutTargetRef: "payout.spark.deadbeefdeadbeefdeadbeef",
      readiness: "payout-target-admitted",
    })
    // A raw spark1… can never be admitted as a public payout-target ref.
    expect(() =>
      admitPayoutTarget({
        kind: "spark_address",
        ref: "spark1qqqqraw0000000000000000000000address",
      }),
    ).toThrow("public-safe")
  })

  test("sparkPayoutTargetRef redacts the raw address to a stable digest ref (#5252)", () => {
    const raw = "spark1pqqqqq0000000000000000000000000000example"
    const ref = sparkPayoutTargetRef(raw)
    expect(ref).toMatch(/^payout\.spark\.[0-9a-f]{24}$/)
    // The redacted ref must NOT contain the raw spark1… anywhere.
    expect(ref).not.toContain("spark1")
    // Stable / idempotent: same input -> same digest ref.
    expect(sparkPayoutTargetRef(raw)).toBe(ref)
    // assertPublicProjectionSafe must accept the redacted ref but reject the raw.
    expect(() => assertPublicProjectionSafe({ payoutTargetRef: ref })).not.toThrow()
    expect(() => assertPublicProjectionSafe({ payoutTargetRef: raw })).toThrow()
    expect(() => sparkPayoutTargetRef("   ")).toThrow("non-empty")
  })

  test("registerSparkPayoutTarget posts raw address only in the private body and projects only the digest (#5252)", async () => {
    const rawSparkAddress = "spark1pqqqqq0000000000000000000000000000agentpayout"
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; url: string }> = []
    const fetchImpl: typeof fetch = async (input, init) => {
      requests.push({
        body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        headers: new Headers(init?.headers),
        url: input.toString(),
      })
      return new Response(JSON.stringify({ ok: true, payoutTargetRef: sparkPayoutTargetRef(rawSparkAddress) }), {
        status: 200,
      })
    }

    const result = await registerSparkPayoutTarget(
      { rawSparkAddress },
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: fetchImpl,
        now: () => new Date("2026-06-17T12:00:00.000Z"),
        pylonRef: "pylon.test.wallet",
      },
    )

    const expectedRef = sparkPayoutTargetRef(rawSparkAddress)
    expect(result.ok).toBe(true)
    expect(result.payoutTargetRef).toBe(expectedRef)
    // The result the caller logs/prints must NOT carry the raw address.
    expect(JSON.stringify({ ok: result.ok, payoutTargetRef: result.payoutTargetRef })).not.toContain("spark1")

    const request = requests[0]
    expect(request?.url).toBe("https://openagents.test/api/pylons/pylon.test.wallet/spark-payout-target")
    expect(request?.headers.get("authorization")).toBe("Bearer oa_agent_test")
    // The raw address rides ONLY the authenticated request body under rawSparkAddress.
    expect(request?.body.rawSparkAddress).toBe(rawSparkAddress)
    expect(request?.body.payoutTargetRef).toBe(expectedRef)
    // The public-safe portion of the body (everything except the raw field) must
    // not contain a spark1…; only the digest ref does.
    const { rawSparkAddress: _raw, ...publicView } = request?.body ?? {}
    expect(JSON.stringify(publicView)).not.toContain("spark1")
    expect(() => assertPublicProjectionSafe(publicView)).not.toThrow()
  })

  test("reports wallet readiness and payout-target admission with public-safe event bodies", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; url: string }> = []
    const fetchImpl: typeof fetch = async (input, init) => {
      requests.push({
        body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        headers: new Headers(init?.headers),
        url: input.toString(),
      })
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }
    const status = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )

    await reportWalletReadiness({ status }, {
      agentToken: "oa_agent_test",
      baseUrl: "https://openagents.test",
      fetch: fetchImpl,
      now: () => new Date("2026-06-10T12:00:00.000Z"),
      pylonRef: "pylon.test.wallet",
    })
    await requestPayoutTargetAdmission(
      { kind: "bolt12_offer", ref: "payout.bolt12.test" },
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: fetchImpl,
        now: () => new Date("2026-06-10T12:00:00.000Z"),
        pylonRef: "pylon.test.wallet",
      },
    )

    expect(requests[0]?.url).toBe("https://openagents.test/api/pylons/pylon.test.wallet/wallet-readiness")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer oa_agent_test")
    expect(requests[0]?.body.walletReady).toBe(true)
    expect(requests[0]?.body.walletRef).toStartWith("wallet.public.mdk.")
    expect(JSON.stringify(requests[0]?.body)).not.toContain("123")
    expect(requests[1]?.url).toBe("https://openagents.test/api/pylons/pylon.test.wallet/payout-target-admission")
    expect(requests[1]?.body.payoutTargetRef).toBe("payout.bolt12.test")
    expect(() => assertPublicProjectionSafe(requests[1]?.body ?? {})).not.toThrow()
  })

  test("reports Spark-primary wallet readiness with Spark public refs (#5178)", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; url: string }> = []
    const fetchImpl: typeof fetch = async (input, init) => {
      requests.push({
        body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        headers: new Headers(init?.headers),
        url: input.toString(),
      })
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }
    const mdkStatus = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )
    const sparkStatus = withSparkPrimaryWalletBalance(mdkStatus, {
      schema: "openagents.pylon.spark_backup_receive.v0.1",
      enabled: true,
      state: "address-ready",
      selectedBecauseRefs: [],
      receiveTargetRef: "wallet.backup.spark_address.redacted",
      lightningAddressRef: null,
      rawTargetAvailableLocally: true,
      credentialReady: true,
      helperReady: true,
      detectedBalanceSats: 50_000,
      unclaimedDepositCount: 0,
      blockerRefs: [],
      nextActionRefs: [],
      publicReceiptRefs: [],
      contentRedacted: true,
    })

    await reportWalletReadiness({ status: sparkStatus }, {
      agentToken: "oa_agent_test",
      baseUrl: "https://openagents.test",
      fetch: fetchImpl,
      now: () => new Date("2026-06-17T12:00:00.000Z"),
      pylonRef: "pylon.test.spark",
    })

    expect(requests[0]?.body.walletReady).toBe(true)
    expect(requests[0]?.body.walletRef).toStartWith("wallet.public.spark.")
    expect(requests[0]?.body.balanceRefs).toEqual(["balance.public.spark.reported_redacted"])
    expect(requests[0]?.body.liquidityRefs).toEqual(["liquidity.public.spark_send_ready_redacted"])
    expect(JSON.stringify(requests[0]?.body)).not.toContain("50000")
    expect(JSON.stringify(requests[0]?.body)).not.toContain("123")
    expect(() => assertPublicProjectionSafe(requests[0]?.body ?? {})).not.toThrow()
  })

  test("redacts receive and send receipts to refs and records settlement ledger idempotently", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const state = await ensurePylonLocalState(summary)
      const fake = runner({
        "receive 1000": { stdout: { invoice: "lnbc10n1rawinvoice", payment_hash: "hash" } },
        "send payout.bolt12.abc123 21": { stdout: { payment_hash: "hash", preimage: "secret" } },
      })

      const receive = await receiveWithMdk(1000, fake)
      const send = await sendWithMdk("payout.bolt12.abc123", 21, fake)
      const eventId = await appendLedgerEvent(state.paths, {
        kind: "settlement-recorded",
        ref: send.receiptRef,
        data: { settlementRef: send.receiptRef },
      })
      const duplicate = await appendLedgerEvent(state.paths, {
        kind: "settlement-recorded",
        ref: send.receiptRef,
        data: { settlementRef: send.receiptRef },
      })
      const ledger = await readFile(state.paths.ledger, "utf8")

      expect(receive.ok).toBe(true)
      expect(receive.receiptRef.startsWith("wallet.mdk_receive_target.")).toBe(true)
      expect(send.ok).toBe(true)
      expect(send.receiptRef.startsWith("wallet.payment.")).toBe(true)
      expect(eventId).toBe(duplicate)
      expect(ledger.trim().split("\n")).toHaveLength(1)
      expect(ledger).not.toContain("lnbc")
      expect(ledger).not.toContain("preimage")
    })
  })

  test("rejects raw wallet and payment material in public projection", () => {
    expect(() => assertPublicProjectionSafe({ invoice: "lnbc10n1rawinvoice" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ note: "payment preimage abc" })).toThrow("private-data-shaped")
  })
})

const RAW_SPARK_ADDRESS = "sp1pgssy9raw7examplespark0address0material0that0must0never0leak0publicly00"
const RAW_MDK_RECEIVE_TARGET = "lnbc42420n1rawmdkreceivetargetthatmustneverleakpublicly"

const sparkHelper =
  (responses: Partial<Record<SparkBackupCommand, { exitCode?: number; stdout?: unknown; stderr?: string }>>): SparkBackupHelper =>
  async (command) => {
    const response = responses[command] ?? { exitCode: 1, stderr: `unexpected spark command: ${command}` }
    return {
      exitCode: response.exitCode ?? 0,
      stdout: typeof response.stdout === "string" ? response.stdout : JSON.stringify(response.stdout ?? {}),
      stderr: response.stderr ?? "",
    }
  }

describe("Spark balance read consistency (#5197)", () => {
  test("a non-forced (fallback) status read flags balanceRefreshing, not a confirmed balance", async () => {
    const detected = await detectSparkBackupBalance(
      sparkHelper({ status: { stdout: { balance_sats: 12406, balance_synced: false } } }),
    )
    expect(detected.helperReady).toBe(true)
    expect(detected.detectedBalanceSats).toBe(12406)
    expect(detected.balanceRefreshing).toBe(true)
  })

  test("a force-synced status read is not flagged refreshing", async () => {
    const detected = await detectSparkBackupBalance(
      sparkHelper({ status: { stdout: { balance_sats: 28790, balance_synced: true } } }),
    )
    expect(detected.detectedBalanceSats).toBe(28790)
    expect(detected.balanceRefreshing).toBe(false)
  })

  test("an older helper without balance_synced is treated as synced (back-compat)", async () => {
    const detected = await detectSparkBackupBalance(
      sparkHelper({ status: { stdout: { balance_sats: 28790 } } }),
    )
    expect(detected.balanceRefreshing).toBe(false)
  })
})

describe("Spark backup receive (slice 1: inert, opt-in, receive-only)", () => {
  test("MDK receive success does not call the Spark backup helper", async () => {
    let helperCalls = 0
    const helper: SparkBackupHelper = async () => {
      helperCalls += 1
      return { exitCode: 0, stdout: JSON.stringify({ spark_address: RAW_SPARK_ADDRESS }), stderr: "" }
    }
    const result = await receiveWithFallback(1000, {
      runner: runner({ "receive 1000": { stdout: { invoice: "lnbc10n1rawinvoice" } } }),
      sparkBackup: { enabled: true, env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv, helper },
    })
    expect(result.ok).toBe(true)
    expect(result.rail).toBe("mdk")
    expect(helperCalls).toBe(0)
  })

  test("MDK daemon offline selects Spark backup receive when enabled", async () => {
    const result = await receiveWithFallback(1000, {
      runner: runner({ "receive 1000": { exitCode: 1, stderr: "MDK daemon offline" } }),
      sparkBackup: {
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
      },
    })
    expect(result.rail).toBe("spark_backup")
    if (result.rail !== "spark_backup") throw new Error("expected spark_backup rail")
    expect(result.ok).toBe(true)
    expect(result.receiptRef).toMatch(/^wallet\.backup_receive\.[a-f0-9]{24}$/)
    expect(result.mdkFailureRef).toMatch(/^wallet\.receive_failure\.offline\.[a-f0-9]{24}$/)
    expect(result.sparkBackup.projection.state).toBe("receive-selected-mdk-offline")
    expect(result.sparkBackup.projection.selectedBecauseRefs).toEqual([result.mdkFailureRef])
    assertPublicProjectionSafe(result.sparkBackup.projection)
  })

  test("MDK validation error does NOT switch rails", async () => {
    const result = await receiveWithFallback(1000, {
      runner: runner({ "receive 1000": { exitCode: 1, stderr: "invalid amount: must be positive" } }),
      sparkBackup: {
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
      },
    })
    expect(result.rail).toBe("mdk")
    expect(result.ok).toBe(false)
    if (result.rail !== "mdk" || !("mdkFailureClass" in result)) throw new Error("expected mdk validation result")
    expect(result.mdkFailureClass).toBe("validation")
    expect(classifyMdkReceiveFailure({ exitCode: 1, stdout: "", stderr: "invalid amount" }).class).toBe("validation")
    expect(classifyMdkReceiveFailure({ exitCode: 1, stdout: "", stderr: "connection refused" }).class).toBe("offline")
    expect(classifyMdkReceiveFailure({ exitCode: 1, stdout: "", stderr: "init timed out" }).class).toBe("offline")
  })

  test("MDK wallet port conflict surfaces actionable MDK_WALLET_PORT guidance (#5505)", () => {
    const env: NodeJS.ProcessEnv = {}

    // EADDRINUSE-class bind failure on the default :3456 (the Orange-wallet
    // first-run collision Larry reported) → detected + clear remediation.
    const eaddr = describeMdkPortConflict(
      { exitCode: 1, stdout: "", stderr: "Error: listen EADDRINUSE: address already in use 127.0.0.1:3456" },
      env,
    )
    expect(eaddr.isPortConflict).toBe(true)
    expect(eaddr.port).toBe(DEFAULT_MDK_WALLET_PORT)
    expect(eaddr.portConfigured).toBe(false)
    expect(eaddr.message).toContain("MDK_WALLET_PORT")
    expect(eaddr.message).toContain("3456")
    // Public-safe: env var name + port only, no secrets.
    expect(eaddr.ref.startsWith("wallet.mdk_port_conflict.")).toBe(true)

    // Phrasing variants are recognized too.
    expect(
      describeMdkPortConflict({ exitCode: 1, stdout: "", stderr: "failed to bind: port 3456 in use" }, env)
        .isPortConflict,
    ).toBe(true)

    // A configured port is echoed back in the guidance.
    const configured = describeMdkPortConflict(
      { exitCode: 1, stdout: "", stderr: "EADDRINUSE 127.0.0.1:3458" },
      { MDK_WALLET_PORT: "3458" },
    )
    expect(configured.isPortConflict).toBe(true)
    expect(configured.port).toBe(3458)
    expect(configured.portConfigured).toBe(true)
    expect(configured.message).toContain("3458")

    // Unrelated failures are NOT misclassified as a port conflict.
    const unrelated = describeMdkPortConflict({ exitCode: 1, stdout: "", stderr: "invalid amount" }, env)
    expect(unrelated.isPortConflict).toBe(false)
    expect(unrelated.message).toBe(null)
    expect(describeMdkPortConflict({ exitCode: 1, stdout: "", stderr: "connection refused" }, env).isPortConflict).toBe(
      false,
    )
  })

  test("offline failure with the OFF override stays on MDK (#5304)", async () => {
    // #5304: the Spark backup is ON by default, so "stays on MDK" now hinges on
    // the explicit OFF override (no Spark fallback is attempted when disabled).
    const result = await receiveWithFallback(1000, {
      runner: runner({ "receive 1000": { exitCode: 1, stderr: "MDK daemon offline" } }),
      sparkBackup: { env: { PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv },
    })
    expect(result.rail).toBe("mdk")
    expect(result.ok).toBe(false)
  })

  test("Spark helper missing returns a typed blocker", async () => {
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper: sparkHelper({ address: { exitCode: 1, stderr: "helper not installed" } }),
    })
    expect(projection.state).toBe("helper-unavailable")
    expect(projection.helperReady).toBe(false)
    expect(projection.blockerRefs).toContain("blocker.wallet.spark_backup.helper_unavailable")
    expect(projection.receiveTargetRef).toBeNull()
    assertPublicProjectionSafe(projection)
  })

  test("missing Spark credential returns a typed blocker", async () => {
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: {} as NodeJS.ProcessEnv,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(projection.state).toBe("credential-missing")
    expect(projection.credentialReady).toBe(false)
    expect(projection.blockerRefs).toContain("blocker.wallet.spark_backup.credential_missing")
    assertPublicProjectionSafe(projection)
  })

  test("embedded default credential is credential-ready out-of-box, no env key (#5078)", async () => {
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: {} as NodeJS.ProcessEnv,
      embeddedCredentialAvailable: true,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(projection.state).toBe("address-ready")
    expect(projection.credentialReady).toBe(true)
    expect(projection.blockerRefs).not.toContain(
      "blocker.wallet.spark_backup.credential_missing",
    )
    expect(projection.receiveTargetRef).toMatch(
      /^wallet\.backup\.spark_address\.[a-f0-9]{24}$/,
    )
    assertPublicProjectionSafe(projection)
  })

  test("cached Spark address works as cached-address-ready when helper is offline", async () => {
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper: sparkHelper({ address: { exitCode: 1, stderr: "helper offline" } }),
      cachedAddress: RAW_SPARK_ADDRESS,
    })
    expect(projection.state).toBe("cached-address-ready")
    expect(projection.receiveTargetRef).toMatch(/^wallet\.backup\.spark_address\.[a-f0-9]{24}$/)
    expect(projection.blockerRefs).toContain("blocker.wallet.spark_backup.sync_unavailable")
    assertPublicProjectionSafe(projection)
  })

  test("--show-local-target is required before raw target output is allowed", async () => {
    const withoutFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(withoutFlag.ok).toBe(true)
    expect(withoutFlag.localTarget).toBeUndefined()
    expect(withoutFlag.receiptRef).toMatch(/^wallet\.backup_receive\.[a-f0-9]{24}$/)
    assertPublicProjectionSafe(withoutFlag.projection)
    // The redacted result object must not carry the raw address anywhere.
    expect(JSON.stringify(withoutFlag.projection)).not.toContain(RAW_SPARK_ADDRESS)

    const withFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
      showLocalTarget: true,
    })
    expect(withFlag.localTarget).toBe(RAW_SPARK_ADDRESS)
    // Even with the local flag, the projection itself stays redacted.
    expect(JSON.stringify(withFlag.projection)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(withFlag.projection)
  })

  test("lightning-address kind yields a redacted lightningAddressRef (#5078)", async () => {
    const RAW_LIGHTNING_ADDRESS = "oab38ad12345abcd9@spark.money"
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      kind: "lightning-address",
      helper: sparkHelper({ "lightning-address": { stdout: { lightning_address: RAW_LIGHTNING_ADDRESS } } }),
    })
    expect(projection.state).toBe("address-ready")
    expect(projection.lightningAddressRef).toMatch(/^wallet\.backup\.lightning_address\.[a-f0-9]{24}$/)
    // The spark-address target ref is not used for the lightning kind.
    expect(projection.receiveTargetRef).toBeNull()
    // Raw address must never appear in the public projection.
    expect(JSON.stringify(projection)).not.toContain(RAW_LIGHTNING_ADDRESS)
    assertPublicProjectionSafe(projection)
  })

  test("lightning-address: --show-local-target required before raw address output (#5078)", async () => {
    const RAW_LIGHTNING_ADDRESS = "oab38ad12345abcd9@spark.money"
    const withoutFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      kind: "lightning-address",
      helper: sparkHelper({ "lightning-address": { stdout: { lightning_address: RAW_LIGHTNING_ADDRESS } } }),
    })
    expect(withoutFlag.ok).toBe(true)
    expect(withoutFlag.localTarget).toBeUndefined()
    expect(withoutFlag.receiptRef).toMatch(/^wallet\.backup_receive\.[a-f0-9]{24}$/)
    expect(JSON.stringify(withoutFlag.projection)).not.toContain(RAW_LIGHTNING_ADDRESS)
    assertPublicProjectionSafe(withoutFlag.projection)

    const withFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      kind: "lightning-address",
      showLocalTarget: true,
      helper: sparkHelper({ "lightning-address": { stdout: { lightning_address: RAW_LIGHTNING_ADDRESS } } }),
    })
    expect(withFlag.localTarget).toBe(RAW_LIGHTNING_ADDRESS)
    // Even with the local flag, the projection itself stays redacted.
    expect(JSON.stringify(withFlag.projection)).not.toContain(RAW_LIGHTNING_ADDRESS)
    assertPublicProjectionSafe(withFlag.projection)
  })

  test("lightning-address: unsupported helper yields helper-unavailable", async () => {
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      kind: "lightning-address",
      helper: sparkHelper({ "lightning-address": { exitCode: 1, stderr: "lightning address unsupported" } }),
    })
    expect(projection.state).toBe("helper-unavailable")
    expect(projection.lightningAddressRef).toBeNull()
    assertPublicProjectionSafe(projection)
  })

  test("unified balance folds MDK and Spark backup buckets without spend overclaim", async () => {
    const mdkStatus = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )
    const helper = sparkHelper({
      address: { stdout: { spark_address: RAW_SPARK_ADDRESS } },
      status: {
        stdout: {
          balance_sats: 50_000,
          claimable_htlc_count: 1,
          claimable_htlc_sats: 7_000,
          unclaimed_deposit_count: 0,
        },
      },
    })
    const sparkBackup = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
    })
    const detected = await detectSparkBackupBalance(helper)
    sparkBackup.detectedBalanceSats = detected.detectedBalanceSats
    sparkBackup.claimableHtlcSats = detected.claimableHtlcSats
    sparkBackup.claimableHtlcCount = detected.claimableHtlcCount
    sparkBackup.unclaimedDepositCount = detected.unclaimedDepositCount

    const status = withUnifiedWalletBalance(mdkStatus, sparkBackup)

    expect(status.unifiedBalance).toMatchObject({
      mdkSpendableSats: 123,
      sparkBackupClaimableSats: 7_000,
      sparkBackupCreditedSats: 50_000,
      sparkBackupPendingSweepSats: 50_000,
      totalVisibleSats: 57_123,
    })
    expect(status.unifiedBalance.sourceRefs).toContain("source.wallet.spark_backup.status")
    expect(status.unifiedBalance.caveatRefs).toContain(
      "caveat.wallet.spark_backup_is_not_mdk_spendable_until_sweep_receipt",
    )
    expect(status.unifiedBalance.nextActionRefs).toContain(
      "action.wallet.spark_backup.review_consent_sweep",
    )
    expect(JSON.stringify(status.unifiedBalance)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(status.unifiedBalance)
  })

  test("Spark-primary wallet status exposes one agent balance and excludes MDK from totals (#5178)", async () => {
    const mdkStatus = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )
    const helper = sparkHelper({
      address: { stdout: { spark_address: RAW_SPARK_ADDRESS } },
      status: {
        stdout: {
          balance_sats: 50_000,
          claimable_htlc_count: 1,
          claimable_htlc_sats: 7_000,
          unclaimed_deposit_count: 0,
        },
      },
    })
    const sparkBackup = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
    })
    const detected = await detectSparkBackupBalance(helper)
    sparkBackup.detectedBalanceSats = detected.detectedBalanceSats
    sparkBackup.claimableHtlcSats = detected.claimableHtlcSats
    sparkBackup.claimableHtlcCount = detected.claimableHtlcCount
    sparkBackup.unclaimedDepositCount = detected.unclaimedDepositCount

    const status = withSparkPrimaryWalletBalance(mdkStatus, sparkBackup)

    expect(status.balanceSats).toBe(50_000)
    expect(status.receiveReady).toBe(true)
    expect(status.sendReady).toBe(true)
    expect(status.readiness).toBe("send-ready")
    expect(status.blockerRefs).toContain("blocker.wallet.spark_primary.claim_pending")
    expect(status.unifiedBalance).toMatchObject({
      schema: "openagents.pylon.unified_wallet_balance.v0.2",
      primaryRail: "spark",
      primaryBalanceSats: 50_000,
      primarySpendableSats: 50_000,
      mdkSpendableSats: null,
      sparkBackupClaimableSats: 7_000,
      sparkBackupCreditedSats: 50_000,
      sparkBackupPendingSweepSats: 0,
      totalVisibleSats: 50_000,
    })
    expect(status.unifiedBalance.sourceRefs).toContain("source.wallet.spark_primary.balance")
    expect(status.unifiedBalance.caveatRefs).toContain("caveat.wallet.mdk_excluded_from_agent_primary_balance")
    expect(status.unifiedBalance.nextActionRefs).toContain("action.wallet.spark_backup.run_backup_claim")
    expect(JSON.stringify(status.unifiedBalance)).not.toContain("123")
    expect(JSON.stringify(status.unifiedBalance)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(status.unifiedBalance)
  })

  test("`balance --json` projects the same send-ready local wallet source as `wallet status` (never empty {}) (#5402)", async () => {
    // Regression for the v1.0.0 shakeout bug: `pylon balance --json` returned an
    // empty `{}` because it called the network earnings endpoint instead of the
    // local primary-wallet projection `wallet status --json` reads. Both now flow
    // through the SAME projection; this asserts the balance subset is populated
    // and mirrors the wallet-status balance source.
    const mdkStatus = await classifyMdkWallet(
      runner({ balance: { stdout: { balance_sats: 123, send_ready: true, outbound_capacity_sats: 21 } } }),
      { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv,
    )
    const helper = sparkHelper({
      address: { stdout: { spark_address: RAW_SPARK_ADDRESS } },
      status: {
        stdout: {
          balance_sats: 50_000,
          claimable_htlc_count: 0,
          claimable_htlc_sats: 0,
          unclaimed_deposit_count: 0,
        },
      },
    })
    const sparkBackup = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
    })
    const detected = await detectSparkBackupBalance(helper)
    sparkBackup.detectedBalanceSats = detected.detectedBalanceSats
    sparkBackup.claimableHtlcSats = detected.claimableHtlcSats
    sparkBackup.claimableHtlcCount = detected.claimableHtlcCount
    sparkBackup.unclaimedDepositCount = detected.unclaimedDepositCount

    // Same local projection source `wallet status --json` uses.
    const status = withSparkPrimaryWalletBalance(mdkStatus, sparkBackup)
    const projected = projectWalletBalance(status)

    // Not the old empty `{}`: the balance subset is populated.
    expect(Object.keys(projected).length).toBeGreaterThan(0)
    expect(projected.balanceSats).toBe(50_000)
    expect(projected.daemonOnline).toBe(true)
    expect(projected.receiveReady).toBe(true)
    expect(projected.sendReady).toBe(true)
    expect(projected.readiness).toBe("send-ready")

    // Mirrors the wallet-status balance source exactly.
    expect(projected.balanceSats).toBe(status.balanceSats)
    expect(projected.sendReady).toBe(status.sendReady)
    expect(projected.receiveReady).toBe(status.receiveReady)
    expect(projected.readiness).toBe(status.readiness)
    expect(projected.daemonOnline).toBe(status.daemonOnline)
    expect(projected.unifiedBalance).toBe(status.unifiedBalance)

    // Projection-safe: no seeds, raw Spark addresses, offers, or invoices.
    expect(JSON.stringify(projected)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(projected)
  })

  test("assertPublicProjectionSafe rejects projections containing raw Spark material", () => {
    expect(() => assertPublicProjectionSafe({ note: `target ${RAW_SPARK_ADDRESS}` })).toThrow("private-data-shaped")
    expect(() => assertPublicProjectionSafe({ note: "spark1qqexampleinvoicematerialthatleaks0000" })).toThrow(
      "private-data-shaped",
    )
    expect(() => assertPublicProjectionSafe({ spark_address: "anything" })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ sparkInvoice: "anything" })).toThrow("not public-safe")
    // Redacted refs remain allowed.
    expect(() =>
      assertPublicProjectionSafe({ receiveTargetRef: "wallet.backup.spark_address.deadbeefdeadbeefdeadbeef" }),
    ).not.toThrow()
    expect(() =>
      assertPublicProjectionSafe({ blockerRefs: ["blocker.wallet.spark_backup.credential_missing"] }),
    ).not.toThrow()
  })

  test("backup-receive ledger events are idempotent", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const state = await ensurePylonLocalState(summary)
      const prepared = await prepareSparkBackupReceive({
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
      })
      const ref = prepared.receiptRef as string
      const first = await appendLedgerEvent(state.paths, {
        kind: "backup-receive-selected",
        ref,
        data: { receiptRef: ref, rail: "spark_backup" },
      })
      const second = await appendLedgerEvent(state.paths, {
        kind: "backup-receive-selected",
        ref,
        data: { receiptRef: ref, rail: "spark_backup" },
      })
      const ledger = await readFile(state.paths.ledger, "utf8")
      expect(first).toBe(second)
      expect(ledger.trim().split("\n")).toHaveLength(1)
      expect(ledger).not.toContain(RAW_SPARK_ADDRESS)
      expect(ledger).not.toContain("sp1")
    })
  })

  test("detected balance recommends migrate-spark but does NOT mark settlement", () => {
    const credited = recommendSparkSweep({ detectedBalanceSats: 4242, unclaimedDepositCount: 1 })
    expect(credited.recommendsMigrateSpark).toBe(true)
    expect(credited.state).toBe("sweep-to-mdk-recommended")
    expect(credited.settlementMarked).toBe(false)
    expect(credited.nextActionRefs).toContain("action.wallet.spark_backup.run_migrate_spark_with_consent")

    const empty = recommendSparkSweep({ detectedBalanceSats: 0 })
    expect(empty.recommendsMigrateSpark).toBe(false)
    expect(empty.settlementMarked).toBe(false)
  })

  test("claimable HTLCs recommend backup-claim before sweep", () => {
    const claimable = recommendSparkSweep({
      claimableHtlcCount: 1,
      claimableHtlcSats: 50_000,
      detectedBalanceSats: 0,
      unclaimedDepositCount: 0,
    })

    expect(claimable.state).toBe("claim-pending")
    expect(claimable.recommendsMigrateSpark).toBe(false)
    expect(claimable.settlementMarked).toBe(false)
    expect(claimable.nextActionRefs).toEqual(["action.wallet.spark_backup.run_backup_claim"])

    const claimableAndCredited = recommendSparkSweep({
      claimableHtlcCount: 1,
      claimableHtlcSats: 7_000,
      detectedBalanceSats: 50_000,
      unclaimedDepositCount: 0,
    })

    expect(claimableAndCredited.state).toBe("claim-pending")
    expect(claimableAndCredited.recommendsMigrateSpark).toBe(true)
    expect(claimableAndCredited.nextActionRefs).toEqual([
      "action.wallet.spark_backup.run_backup_claim",
      "action.wallet.spark_backup.run_migrate_spark_with_consent",
    ])
  })

  test("#5304: enabled BY DEFAULT (no env flag) — provisions an address-ready projection", async () => {
    // A fresh node with NO env flags: the Spark backup is ON by default and,
    // with the embedded credential + a ready helper, resolves a redacted
    // address-ready receive target. helperReady:true, no flag required.
    const projection = await classifySparkBackupReceive({
      env: {} as NodeJS.ProcessEnv,
      embeddedCredentialAvailable: true,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(projection.enabled).toBe(true)
    expect(projection.state).toBe("address-ready")
    expect(projection.helperReady).toBe(true)
    expect(projection.credentialReady).toBe(true)
    expect(projection.receiveTargetRef).toMatch(/^wallet\.backup\.spark_address\.[a-f0-9]{24}$/)
    // Redaction: the raw spark1… address never appears in the projection.
    expect(JSON.stringify(projection)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(projection)
  })

  test("#5304: the OFF override (PYLON_SPARK_BACKUP_DISABLED=1) keeps it disabled", async () => {
    const projection = await classifySparkBackupReceive({
      env: { PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
      embeddedCredentialAvailable: true,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(projection.enabled).toBe(false)
    expect(projection.state).toBe("disabled")
    expect(projection.receiveTargetRef).toBeNull()
    expect(JSON.stringify(projection)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(projection)
  })

  test("#5304: the OFF override (PYLON_SPARK_BACKUP_ENABLED=0) keeps it disabled", async () => {
    const projection = await classifySparkBackupReceive({
      env: { PYLON_SPARK_BACKUP_ENABLED: "0" } as NodeJS.ProcessEnv,
      embeddedCredentialAvailable: true,
      helper: sparkHelper({ address: { stdout: { spark_address: RAW_SPARK_ADDRESS } } }),
    })
    expect(projection.enabled).toBe(false)
    expect(projection.state).toBe("disabled")
    assertPublicProjectionSafe(projection)
  })

  test("#5304: isSparkBackupDefaultEnabled is ON by default, OFF only on explicit override", () => {
    expect(isSparkBackupDefaultEnabled({} as NodeJS.ProcessEnv)).toBe(true)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_ENABLED: "1" } as NodeJS.ProcessEnv)).toBe(true)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_ENABLED: "true" } as NodeJS.ProcessEnv)).toBe(true)
    // garbage / unknown values still leave it ON
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_ENABLED: "yes" } as NodeJS.ProcessEnv)).toBe(true)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv)).toBe(false)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_DISABLED: "true" } as NodeJS.ProcessEnv)).toBe(false)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_ENABLED: "0" } as NodeJS.ProcessEnv)).toBe(false)
    expect(isSparkBackupDefaultEnabled({ PYLON_SPARK_BACKUP_ENABLED: "false" } as NodeJS.ProcessEnv)).toBe(false)
  })

  test("caches the raw Spark target only in mode-0600 local private state", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const state = await ensurePylonLocalState(summary)
      expect(await readCachedSparkTarget(state.paths)).toBeNull()

      await writeCachedSparkTarget(state.paths, RAW_SPARK_ADDRESS)
      expect(await readCachedSparkTarget(state.paths)).toBe(RAW_SPARK_ADDRESS)

      const targetPath = sparkBackupTargetPath(state.paths)
      const { statSync } = await import("node:fs")
      // Mode 0600: only the owner can read/write the raw receive material.
      expect(statSync(targetPath).mode & 0o777).toBe(0o600)

      // The cached file lives under the private wallet/spark-backup dir, not
      // public state, and is not posted anywhere by these functions.
      expect(targetPath.startsWith(`${home}/wallet/spark-backup/`)).toBe(true)
    })
  })

  test("a cached target lets the helper-offline path report cached-address-ready", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const state = await ensurePylonLocalState(summary)
      await writeCachedSparkTarget(state.paths, RAW_SPARK_ADDRESS)
      const cached = await readCachedSparkTarget(state.paths)

      const projection = await classifySparkBackupReceive({
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: sparkHelper({ address: { exitCode: 1, stderr: "helper offline" } }),
        cachedAddress: cached,
      })
      expect(projection.state).toBe("cached-address-ready")
      expect(JSON.stringify(projection)).not.toContain(RAW_SPARK_ADDRESS)
      assertPublicProjectionSafe(projection)
    })
  })
})

describe("Spark backup send / withdraw (#5177)", () => {
  const enabledEnv = { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_ENABLED: "1" } as NodeJS.ProcessEnv
  const RAW_SPARK_PAYMENT_REQUEST = "lnbc2100n1rawsparkpaymentrequestthatmustneverleakpublicly"
  const successfulSend: SparkBackupSendTransfer = async ({ amountSats, destination }) => {
    expect(destination).toBe(RAW_SPARK_PAYMENT_REQUEST)
    return {
      ok: true,
      transferRef: "wallet.spark_backup_send.deadbeefdeadbeefdeadbeef",
      sparkPaymentRef: "wallet.spark_backup_send_payment.deadbeefdeadbeefdeadbeef",
      amountSats,
      feeSats: 2,
      method: "payment_request",
      status: "complete",
    }
  }

  test("OFF override (PYLON_SPARK_BACKUP_DISABLED=1): no spend (#5304)", async () => {
    // #5304: the backup is ON by default, so "no spend" now hinges on the
    // explicit OFF override rather than the absence of an opt-in flag. With the
    // override set, the send is disabled and the transfer is never invoked.
    let called = false
    const send = await sendWithSparkBackup({
      amountSats: 2100,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      confirmSend: true,
      transfer: async () => {
        called = true
        return { ok: false, failureRef: "wallet.spark_backup_send.should_not_call" }
      },
      env: { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
    })
    expect(send.state).toBe("disabled")
    expect(called).toBe(false)
    expect(send.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(send)
  })

  test("requires explicit confirm-send before moving Spark funds", async () => {
    let called = false
    const send = await sendWithSparkBackup({
      amountSats: 2100,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      env: enabledEnv,
      transfer: async () => {
        called = true
        return {
          ok: false,
          failureRef: "wallet.spark_backup_send.should_not_call",
        }
      },
    })
    expect(send.state).toBe("consent-required")
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.send_consent_required")
    expect(called).toBe(false)
    expect(JSON.stringify(send)).not.toContain(RAW_SPARK_PAYMENT_REQUEST)
    assertPublicProjectionSafe(send)
  })

  test("confirm-send emits a dereferenceable public-safe receipt", async () => {
    const send = await sendWithSparkBackup({
      amountSats: 2100,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      confirmSend: true,
      env: enabledEnv,
      transfer: successfulSend,
      now: () => new Date("2026-06-17T00:00:00.000Z"),
    })
    expect(send.state).toBe("sent")
    expect(send.amountSats).toBe(2100)
    expect(send.feeSats).toBe(2)
    expect(send.destinationRef).toMatch(/^wallet\.spark_backup_send\.destination\.[a-f0-9]{24}$/)
    expect(send.transferRef).toBe("wallet.spark_backup_send.deadbeefdeadbeefdeadbeef")
    expect(send.sparkPaymentRef).toBe("wallet.spark_backup_send_payment.deadbeefdeadbeefdeadbeef")
    expect(send.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_send\.[a-f0-9]{24}$/)
    expect(JSON.stringify(send)).not.toContain(RAW_SPARK_PAYMENT_REQUEST)
    expect(JSON.stringify(send)).not.toContain("lnbc")
    assertPublicProjectionSafe(send)
  })

  test("a native Spark send surfaces method spark_native in the public projection (#5225)", async () => {
    const RAW_SPARK_NATIVE = "spark1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3jn54khce6mua7lqpzr"
    const nativeSend: SparkBackupSendTransfer = async ({ amountSats, destination }) => {
      expect(destination).toBe(RAW_SPARK_NATIVE)
      return {
        ok: true,
        transferRef: "wallet.spark_backup_send.deadbeefdeadbeefdeadbeef",
        sparkPaymentRef: "wallet.spark_backup_send_payment.deadbeefdeadbeefdeadbeef",
        amountSats,
        // Native Spark→Spark carries no Lightning routing fee.
        feeSats: 0,
        method: "spark_native",
        status: "completed",
      }
    }
    const send = await sendWithSparkBackup({
      amountSats: 3000,
      destination: RAW_SPARK_NATIVE,
      confirmSend: true,
      env: enabledEnv,
      transfer: nativeSend,
      now: () => new Date("2026-06-17T00:00:00.000Z"),
    })
    expect(send.state).toBe("sent")
    expect(send.method).toBe("spark_native")
    expect(send.feeSats).toBe(0)
    expect(send.amountSats).toBe(3000)
    expect(send.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_send\.[a-f0-9]{24}$/)
    expect(JSON.stringify(send)).not.toContain(RAW_SPARK_NATIVE)
    assertPublicProjectionSafe(send)
  })

  test("a timed-out (indeterminate) send is marked pending, not failed, and offers no retry (#5196)", async () => {
    const send = await sendWithSparkBackup({
      amountSats: 21000,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      confirmSend: true,
      env: enabledEnv,
      transfer: async () => ({
        ok: false,
        failureRef: "wallet.spark_backup_send_indeterminate.deadbeefdeadbeefdeadbeef",
      }),
    })
    expect(send.state).toBe("send-pending")
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.send_outcome_pending")
    expect(send.nextActionRefs).toContain("action.wallet.spark_backup.verify_balance_before_retry")
    // Must NOT invite a blind retry — the funds may have moved (double-spend risk).
    expect(send.nextActionRefs).not.toContain(
      "action.wallet.spark_backup.retry_send_after_fixing_transfer",
    )
    expect(send.state).not.toBe("send-failed")
    expect(send.publicReceiptRefs).toEqual([])
    expect(JSON.stringify(send)).not.toContain(RAW_SPARK_PAYMENT_REQUEST)
    assertPublicProjectionSafe(send)
  })

  test("a fee-guard rejection surfaces a distinct send_fee_too_high blocker + raise-ceiling action (#5254)", async () => {
    const send = await sendWithSparkBackup({
      amountSats: 44,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      confirmSend: true,
      env: enabledEnv,
      // Mirror the helper's pre-send fee-guard rejection: a public-safe,
      // operator-legible failureRef carrying `fee_too_high` and integers only.
      transfer: async () => ({
        ok: false,
        failureRef: "wallet.spark_backup_send.fee_too_high:prepared=4096:amount=44",
      }),
    })
    expect(send.state).toBe("send-failed")
    // Distinct blocker, NOT the generic send_failed.
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.send_fee_too_high")
    expect(send.blockerRefs).not.toContain("blocker.wallet.spark_backup.send_failed")
    // Next action points the operator at the override / amount adjustment.
    expect(send.nextActionRefs).toContain("action.wallet.spark_backup.raise_max_fee_or_adjust_amount")
    // The legible failureRef is carried through (numbers only, public-safe).
    expect(send.failureRefs).toContain("wallet.spark_backup_send.fee_too_high:prepared=4096:amount=44")
    expect(send.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(send)
  })

  test("a per-domain policy refusal surfaces the destination_fee_policy blocker + attributes the bare domain, full LA redacted (#5257)", async () => {
    const RAW_LIGHTNING_ADDRESS = "alice@bitnob.io"
    const send = await sendWithSparkBackup({
      amountSats: 5000,
      destination: RAW_LIGHTNING_ADDRESS,
      confirmSend: true,
      env: enabledEnv,
      // Mirror the helper's pre-dispatch domain-policy refusal: a public-safe
      // failureRef carrying the BARE domain (never the full `name@domain`).
      transfer: async () => ({
        ok: false,
        failureRef: "wallet.spark_backup_send.destination_fee_policy:bitnob.io",
      }),
    })
    expect(send.state).toBe("send-failed")
    // Distinct domain-policy blocker, NOT the generic send_failed or fee_too_high.
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.destination_fee_policy")
    expect(send.blockerRefs).not.toContain("blocker.wallet.spark_backup.send_failed")
    expect(send.blockerRefs).not.toContain("blocker.wallet.spark_backup.send_fee_too_high")
    expect(send.nextActionRefs).toContain("action.wallet.spark_backup.allow_destination_domain_or_adjust")
    // The bare domain is attributed into the projection (public-safe).
    expect(send.destinationDomain).toBe("bitnob.io")
    expect(send.failureRefs).toContain("wallet.spark_backup_send.destination_fee_policy:bitnob.io")
    expect(send.publicReceiptRefs).toEqual([])
    // The full Lightning Address NEVER appears anywhere in the projection.
    expect(JSON.stringify(send)).not.toContain(RAW_LIGHTNING_ADDRESS)
    expect(JSON.stringify(send)).not.toContain("alice@")
    assertPublicProjectionSafe(send)
  })

  test("a successful LA send attributes the bare destination domain while keeping the full LA redacted (#5257)", async () => {
    const RAW_LIGHTNING_ADDRESS = "carol@bitnob.io"
    const laSend: SparkBackupSendTransfer = async ({ amountSats, destination }) => {
      expect(destination).toBe(RAW_LIGHTNING_ADDRESS)
      return {
        ok: true,
        transferRef: "wallet.spark_backup_send.deadbeefdeadbeefdeadbeef",
        sparkPaymentRef: "wallet.spark_backup_send_payment.deadbeefdeadbeefdeadbeef",
        amountSats,
        feeSats: 12,
        feeFromPrepared: true,
        // The helper returns the bare domain as public-safe attribution.
        destinationDomain: "bitnob.io",
        method: "lnurl_pay",
        status: "complete",
      }
    }
    const send = await sendWithSparkBackup({
      amountSats: 1000,
      destination: RAW_LIGHTNING_ADDRESS,
      confirmSend: true,
      env: enabledEnv,
      transfer: laSend,
      now: () => new Date("2026-06-17T00:00:00.000Z"),
    })
    expect(send.state).toBe("sent")
    expect(send.method).toBe("lnurl_pay")
    expect(send.destinationDomain).toBe("bitnob.io")
    expect(send.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_send\.[a-f0-9]{24}$/)
    // The full LA stays redacted to destinationRef; only the bare domain surfaces.
    expect(JSON.stringify(send)).not.toContain(RAW_LIGHTNING_ADDRESS)
    expect(JSON.stringify(send)).not.toContain("carol@")
    assertPublicProjectionSafe(send)
  })

  test("maxFeeSats override is threaded to the transfer (#5254)", async () => {
    let seenMaxFee: number | undefined = -1
    const send = await sendWithSparkBackup({
      amountSats: 44,
      destination: RAW_SPARK_PAYMENT_REQUEST,
      confirmSend: true,
      env: enabledEnv,
      maxFeeSats: 5000,
      transfer: async (input) => {
        seenMaxFee = input.maxFeeSats
        return { ok: true, transferRef: "wallet.spark_backup_send.aaaaaaaaaaaaaaaaaaaaaaaa", sparkPaymentRef: "wallet.spark_backup_send_payment.bbbbbbbbbbbbbbbbbbbbbbbb", amountSats: 44, feeSats: 4096, feeFromPrepared: true, method: "payment_request", status: "complete" }
      },
      now: () => new Date("2026-06-17T00:00:00.000Z"),
    })
    expect(seenMaxFee).toBe(5000)
    expect(send.state).toBe("sent")
    assertPublicProjectionSafe(send)
  })

  test("invalid request returns blockers and no receipt", async () => {
    const send = await sendWithSparkBackup({
      amountSats: 0,
      destination: "",
      confirmSend: true,
      env: enabledEnv,
      transfer: successfulSend,
    })
    expect(send.state).toBe("invalid-request")
    expect(send.publicReceiptRefs).toEqual([])
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.send_destination_required")
    expect(send.blockerRefs).toContain("blocker.wallet.spark_backup.send_amount_required")
    assertPublicProjectionSafe(send)
  })
})

describe("Spark backup reconcile / sweep (slice 3: consented receive-side sweep, NOT payout)", () => {
  const enabledEnv = { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv
  const mdkRunnerWithVerifiedCredit = (
    beforeSats: number,
    afterSats: number,
    receiveTarget = RAW_MDK_RECEIVE_TARGET,
  ): WalletCommandRunner => {
    let balanceCalls = 0
    return async (args) => {
      const key = args.join(" ")
      if (key === "balance") {
        const balance = balanceCalls === 0 ? beforeSats : afterSats
        balanceCalls += 1
        return {
          exitCode: 0,
          stdout: JSON.stringify({ balance_sats: balance, send_ready: true, outbound_capacity_sats: 21 }),
          stderr: "",
        }
      }
      if (key.startsWith("receive ")) {
        return {
          exitCode: 0,
          stdout: JSON.stringify({ invoice: receiveTarget, payment_hash: "redacted-test-hash" }),
          stderr: "",
        }
      }
      return { exitCode: 1, stdout: "", stderr: `unexpected mdk command: ${key}` }
    }
  }
  const successfulTransfer: SparkBackupSweepTransfer = async ({ amountSats, destination }) => {
    expect(destination).toBe(RAW_MDK_RECEIVE_TARGET)
    return {
      ok: true,
      transferRef: "wallet.spark_backup_transfer.deadbeefdeadbeefdeadbeef",
      amountSats,
      feeSats: 3,
    }
  }

  test("OFF override -> disabled, no movement (#5304)", async () => {
    // #5304: ON by default; the OFF override is what now keeps a sweep inert.
    const reconcile = await sweepSparkBackupToMdk({
      env: { PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
    })
    expect(reconcile.enabled).toBe(false)
    expect(reconcile.state).toBe("disabled")
    expect(reconcile.sweptAmountSats).toBeNull()
    expect(reconcile.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(reconcile)
  })

  test("missing credential -> typed blocker, no movement", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: {} as NodeJS.ProcessEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } } }),
    })
    expect(reconcile.state).toBe("credential-missing")
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.credential_missing")
    expect(reconcile.sweptAmountSats).toBeNull()
    assertPublicProjectionSafe(reconcile)
  })

  test("helper unavailable -> typed blocker, no movement", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { exitCode: 1, stderr: "helper offline" } }),
    })
    expect(reconcile.state).toBe("helper-unavailable")
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.helper_unavailable")
    assertPublicProjectionSafe(reconcile)
  })

  test("nothing detected -> nothing-to-sweep, no receipt, exit-clean", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 0, unclaimed_deposit_count: 0 } } }),
    })
    expect(reconcile.state).toBe("nothing-to-sweep")
    expect(reconcile.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(reconcile)
  })

  test("WITHOUT --confirm-sweep refuses to move detected funds (consent-required)", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: false,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } } }),
    })
    expect(reconcile.state).toBe("consent-required")
    expect(reconcile.consentRequired).toBe(true)
    expect(reconcile.sweptAmountSats).toBeNull()
    expect(reconcile.detectedBalanceSats).toBe(4242)
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.sweep_consent_required")
    expect(reconcile.nextActionRefs).toContain("action.wallet.spark_backup.rerun_with_confirm_sweep")
    assertPublicProjectionSafe(reconcile)
  })

  test("consent given but MDK destination not ready -> sweep-failed, funds untouched", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: false,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } } }),
    })
    expect(reconcile.state).toBe("sweep-failed")
    expect(reconcile.sweptAmountSats).toBeNull()
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.mdk_destination_not_ready")
    assertPublicProjectionSafe(reconcile)
  })

  test("claimable but uncredited HTLCs must be claimed before sweep", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({
        status: {
          stdout: {
            balance_sats: 0,
            claimable_htlc_count: 1,
            claimable_htlc_sats: 50_000,
            unclaimed_deposit_count: 0,
          },
        },
      }),
    })
    expect(reconcile.state).toBe("sweep-failed")
    expect(reconcile.claimableHtlcCount).toBe(1)
    expect(reconcile.claimableHtlcSats).toBe(50_000)
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.claim_required_before_sweep")
    expect(reconcile.nextActionRefs).toContain("action.wallet.spark_backup.run_backup_claim_before_sweep")
    expect(reconcile.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(reconcile)
  })

  test("consent + destination ready still blocks when MDK cannot create a receive target", async () => {
    const failingMdkRunner: WalletCommandRunner = async (args) => {
      const key = args.join(" ")
      if (key === "balance") {
        return { exitCode: 0, stdout: JSON.stringify({ balance_sats: 100 }), stderr: "" }
      }
      if (key.startsWith("receive ")) {
        return { exitCode: 1, stdout: "", stderr: "mdk receive unavailable" }
      }
      return { exitCode: 1, stdout: "", stderr: `unexpected mdk command: ${key}` }
    }
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 0 } } }),
      mdkRunner: failingMdkRunner,
      transfer: successfulTransfer,
    })
    expect(reconcile.state).toBe("sweep-failed")
    expect(reconcile.mdkCreditState).toBe("failed")
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.mdk_receive_target_failed")
    expect(reconcile.failureRefs[0]).toMatch(/^wallet\.mdk_receive_request_failure\.[a-f0-9]{24}$/)
    assertPublicProjectionSafe(reconcile)
  })

  test("consent + destination ready still blocks when transfer adapter is unavailable", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 0 } } }),
      mdkRunner: mdkRunnerWithVerifiedCredit(100, 100),
    })
    expect(reconcile.state).toBe("sweep-failed")
    expect(reconcile.mdkCreditState).toBe("failed")
    expect(reconcile.blockerRefs).toContain("blocker.wallet.spark_backup.transfer_unavailable")
    expect(reconcile.publicReceiptRefs).toEqual([])
    assertPublicProjectionSafe(reconcile)
  })

  test("transfer sent but MDK balance not yet increased stays pending, not spendable", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 0 } } }),
      mdkRunner: mdkRunnerWithVerifiedCredit(100, 100),
      transfer: successfulTransfer,
      verificationAttempts: 1,
    })
    expect(reconcile.state).toBe("sweep-pending-mdk-credit")
    expect(reconcile.mdkCreditState).toBe("pending")
    expect(reconcile.mdkCreditedSats).toBe(0)
    expect(reconcile.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_transfer\.[a-f0-9]{24}$/)
    assertPublicProjectionSafe(reconcile)
  })

  test("consent + destination ready sweeps mock-detected balance and emits a redacted reconcile receipt", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({
        status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 2 } },
        "unclaimed-deposits": { stdout: { unclaimed_deposit_count: 2 } },
      }),
      mdkRunner: mdkRunnerWithVerifiedCredit(100, 4242 + 100),
      now: () => new Date("2026-06-16T00:00:00.000Z"),
      transfer: successfulTransfer,
    })
    expect(reconcile.state).toBe("swept-to-mdk")
    expect(reconcile.consentRequired).toBe(false)
    expect(reconcile.sweptAmountSats).toBe(4242)
    expect(reconcile.claimedDepositCount).toBe(0)
    expect(reconcile.transferFeeSats).toBe(3)
    expect(reconcile.mdkCreditState).toBe("verified")
    expect(reconcile.mdkBalanceBeforeSats).toBe(100)
    expect(reconcile.mdkBalanceAfterSats).toBe(4242 + 100)
    expect(reconcile.mdkCreditedSats).toBe(4242)
    expect(reconcile.mdkReceiveTargetRef).toMatch(/^wallet\.mdk_receive_target\.[a-f0-9]{24}$/)
    expect(reconcile.sparkTransferRef).toBe("wallet.spark_backup_transfer.deadbeefdeadbeefdeadbeef")
    expect(reconcile.publicReceiptRefs).toHaveLength(1)
    expect(reconcile.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_reconcile\.[a-f0-9]{24}$/)
    expect(JSON.stringify(reconcile)).not.toContain(RAW_MDK_RECEIVE_TARGET)
    assertPublicProjectionSafe(reconcile)
  })

  test("falls back to unclaimed-deposits command when status omits the deposit count", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({
        status: { stdout: { balance_sats: 1000 } },
        "unclaimed-deposits": { stdout: { unclaimed_deposit_count: 3 } },
      }),
      mdkRunner: mdkRunnerWithVerifiedCredit(0, 1000),
      transfer: successfulTransfer,
    })
    expect(reconcile.state).toBe("swept-to-mdk")
    expect(reconcile.claimedDepositCount).toBe(0)
    expect(reconcile.unclaimedDepositCount).toBe(3)
    assertPublicProjectionSafe(reconcile)
  })

  test("reconcile projection NEVER leaks raw Spark material", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: enabledEnv,
      confirmSweep: true,
      destinationReady: true,
      helper: sparkHelper({
        status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1, spark_address: RAW_SPARK_ADDRESS } },
      }),
      mdkRunner: mdkRunnerWithVerifiedCredit(100, 4242 + 100),
      transfer: successfulTransfer,
    })
    expect(JSON.stringify(reconcile)).not.toContain(RAW_SPARK_ADDRESS)
    expect(JSON.stringify(reconcile)).not.toContain(RAW_MDK_RECEIVE_TARGET)
    expect(JSON.stringify(reconcile)).not.toContain("sp1")
    assertPublicProjectionSafe(reconcile)
  })
})
