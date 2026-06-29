import { describe, expect, test } from "bun:test"
import { assertPublicProjectionSafe } from "../src/state"
import {
  preflightLegacySparkMigration,
  sweepSparkBackupToMdk,
  type LegacySparkCommandRunner,
  type SparkBackupCommand,
  type SparkBackupHelper,
  type SparkBackupSweepTransfer,
  type WalletCommandRunner,
} from "../src/wallet"
import {
  DEFAULT_OPENAGENTS_SPARK_API_KEY,
  legacySparkHelperRunner,
  resolveLegacySparkApiKey,
} from "../src/spark-backup-helper"

// A LegacySparkCommandRunner backed by canned JSON, NO live network/key.
const legacyRunner =
  (responses: Record<string, { exitCode?: number; stdout?: unknown; stderr?: string }>): LegacySparkCommandRunner =>
  async (args) => {
    const key = args.join(" ")
    const response = responses[key] ?? { exitCode: 1, stderr: `unexpected command: ${key}` }
    return {
      exitCode: response.exitCode ?? 0,
      stdout: typeof response.stdout === "string" ? response.stdout : JSON.stringify(response.stdout ?? {}),
      stderr: response.stderr ?? "",
    }
  }

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

const EMPTY_ENV = {} as NodeJS.ProcessEnv
const TEST_MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
const RAW_MDK_RECEIVE_TARGET = "lnbc42420n1rawmdkreceivetargetthatmustneverleakpublicly"

const mdkRunnerWithVerifiedCredit = (beforeSats: number, afterSats: number): WalletCommandRunner => {
  let balanceCalls = 0
  return async (args) => {
    const key = args.join(" ")
    if (key === "balance") {
      const balance = balanceCalls === 0 ? beforeSats : afterSats
      balanceCalls += 1
      return { exitCode: 0, stdout: JSON.stringify({ balance_sats: balance }), stderr: "" }
    }
    if (key.startsWith("receive ")) {
      return { exitCode: 0, stdout: JSON.stringify({ invoice: RAW_MDK_RECEIVE_TARGET }), stderr: "" }
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
    feeSats: 1,
  }
}

describe("#5085 legacy migrate-spark rewired to embedded-key Bun helper", () => {
  test("embedded credential + helper balance => migration recommended, NO breez_api_key_missing (no env key)", async () => {
    const preflight = await preflightLegacySparkMigration({
      destinationInvoiceReady: true,
      dryRun: true,
      embeddedCredentialAvailable: true,
      env: EMPTY_ENV,
      helperRunner: legacyRunner({
        status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } },
      }),
      // identity mnemonic is present (the preflight checks presence via env in
      // this fake env; mark it present explicitly).
      identityMnemonicPath: undefined,
    })

    expect(preflight.legacyBalanceDetected).toBe(true)
    expect(preflight.legacySpendableBalanceSats).toBe(4242)
    expect(preflight.helperInitReady).toBe(true)
    expect(preflight.legacyCredentialReady).toBe(true)
    // The exact dead-end the RC tester hit must be GONE.
    expect(preflight.blockerRefs).not.toContain("blocker.wallet.legacy_spark.breez_api_key_missing")
    assertPublicProjectionSafe(preflight)
  })

  test("embedded credential alone marks legacyCredentialReady even with empty env and a failing helper", async () => {
    const preflight = await preflightLegacySparkMigration({
      dryRun: true,
      embeddedCredentialAvailable: true,
      env: EMPTY_ENV,
      helperRunner: legacyRunner({
        // Network/helper init failure that is NOT a missing-key error.
        status: { exitCode: 1, stderr: "spark sdk build timed out" },
      }),
    })

    expect(preflight.legacyCredentialReady).toBe(true)
    // Credential is present, so the missing-key dead-end is not raised; only the
    // generic helper-init blocker remains.
    expect(preflight.blockerRefs).not.toContain("blocker.wallet.legacy_spark.breez_api_key_missing")
    expect(preflight.blockerRefs).toContain("blocker.wallet.legacy_spark.helper_init_failed")
    assertPublicProjectionSafe(preflight)
  })

  test("without embedded flag and no env key, the missing-key dead-end still applies (regression guard)", async () => {
    const preflight = await preflightLegacySparkMigration({
      dryRun: true,
      env: { PYLON_LEGACY_SPARK_BALANCE_SATS: "4242" } as NodeJS.ProcessEnv,
      helperRunner: legacyRunner({
        status: { exitCode: 1, stderr: "Missing Breez API key" },
      }),
    })
    expect(preflight.blockerRefs).toContain("blocker.wallet.legacy_spark.breez_api_key_missing")
  })

  test("consent path triggers the receive-side sweep into MDK (embedded credential, fake helper)", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      embeddedCredentialAvailable: true,
      env: EMPTY_ENV,
      helper: sparkHelper({
        status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } },
      }),
      confirmSweep: true,
      destinationReady: true,
      mdkRunner: mdkRunnerWithVerifiedCredit(100, 4242 + 100),
      now: () => new Date("2026-06-16T00:00:00.000Z"),
      transfer: successfulTransfer,
    })

    expect(reconcile.state).toBe("swept-to-mdk")
    expect(reconcile.sweptAmountSats).toBe(4242)
    expect(reconcile.claimedDepositCount).toBe(0)
    expect(reconcile.mdkCreditState).toBe("verified")
    expect(JSON.stringify(reconcile)).not.toContain(RAW_MDK_RECEIVE_TARGET)
    expect(reconcile.publicReceiptRefs[0]).toMatch(/^receipt\.pylon\.spark_backup_reconcile\.[a-f0-9]{24}$/)
    assertPublicProjectionSafe(reconcile)
  })

  test("sweep without any credential is still blocked (receive-only boundary preserved)", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      env: EMPTY_ENV,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242 } } }),
      confirmSweep: true,
      destinationReady: true,
    })
    expect(reconcile.state).toBe("credential-missing")
  })

  test("consent required before any sweep even with embedded credential", async () => {
    const reconcile = await sweepSparkBackupToMdk({
      enabled: true,
      embeddedCredentialAvailable: true,
      env: EMPTY_ENV,
      helper: sparkHelper({ status: { stdout: { balance_sats: 4242, unclaimed_deposit_count: 1 } } }),
      confirmSweep: false,
      destinationReady: true,
    })
    expect(reconcile.state).toBe("consent-required")
    expect(reconcile.sweptAmountSats).toBeNull()
  })
})

describe("#5085 resolveLegacySparkApiKey / legacySparkHelperRunner", () => {
  test("resolveLegacySparkApiKey falls back to the embedded default with no env key", () => {
    expect(resolveLegacySparkApiKey(EMPTY_ENV)).toBe(DEFAULT_OPENAGENTS_SPARK_API_KEY)
    expect(resolveLegacySparkApiKey({ OPENAGENTS_SPARK_API_KEY: "env-key" } as NodeJS.ProcessEnv)).toBe("env-key")
  })

  test("legacySparkHelperRunner maps ['status'] to the helper's status command, no env key needed", async () => {
    // Inject a fake Breez SDK module so no live network/key is used. The runner
    // must init and return balance from `status` with NO 'Missing Breez API key'.
    const fakeModule = {
      defaultConfig: (_network: string) => ({}),
      connect: async (_req: unknown) => ({
        getInfo: async () => ({ balanceSats: 7777 }),
        listUnclaimedDeposits: async () => ({ deposits: [] }),
        receivePayment: async () => ({ paymentRequest: "redacted" }),
        listPayments: async () => ({ payments: [] }),
        disconnect: async () => {},
      }),
    }
    const runnerFn = legacySparkHelperRunner({
      env: EMPTY_ENV,
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeModule as never,
    })
    const result = await runnerFn(["status"])
    expect(result.exitCode).toBe(0)
    expect(result.stderr).not.toMatch(/missing\s+breez\s+api\s+key/i)
    const parsed = JSON.parse(result.stdout) as { balance_sats?: number }
    expect(parsed.balance_sats).toBe(7777)
  })

  test("legacySparkHelperRunner with no mnemonic reports unavailable (graceful fallback)", async () => {
    const runnerFn = legacySparkHelperRunner({ env: EMPTY_ENV, mnemonic: null })
    const result = await runnerFn(["status"])
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("missing identity or recovery mnemonic")
  })
})
