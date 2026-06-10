import { mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state"
import {
  admitPayoutTarget,
  appendLedgerEvent,
  classifyMdkWallet,
  preflightLegacySparkMigration,
  receiveWithMdk,
  reportWalletReadiness,
  requestPayoutTargetAdmission,
  sendWithMdk,
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

describe("MDK wallet readiness and ledger", () => {
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

  test("admits only public-safe payout target refs", () => {
    expect(admitPayoutTarget({ kind: "bolt12_offer", ref: "payout.bolt12.abc123" })).toEqual({
      kind: "bolt12_offer",
      payoutTargetRef: "payout.bolt12.abc123",
      readiness: "payout-target-admitted",
    })
    expect(() => admitPayoutTarget({ kind: "bolt11_invoice", ref: "lnbc10n1rawinvoice" })).toThrow("public-safe")
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
      expect(receive.receiptRef.startsWith("wallet.receive.")).toBe(true)
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
