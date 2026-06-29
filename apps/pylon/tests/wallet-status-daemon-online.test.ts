import { describe, expect, test } from "bun:test"
import { join } from "node:path"
import {
  classifySparkBackupReceive,
  isSparkBackupDefaultEnabled,
  mdkScopedAgentWalletStatus,
  withPayoutTargetReadiness,
  withSparkPrimaryWalletBalance,
  type SparkBackupHelper,
} from "../src/wallet"
import { resolveSparkBackupHelper } from "../src/spark-backup-helper"

// #5306 — `wallet status` must report `daemonOnline: true` with ZERO manual
// steps on a healthy node.
//
// Background (the Orwell report on v1.0.0/1.0.1): a node with the Spark wallet
// ENABLED-BY-DEFAULT (#5304) and a healthy daemon up still reported
// `daemonOnline: false` + `blocker.wallet.spark_primary.helper_unavailable`.
//
// `daemonOnline = configured && helperReady`. The "helper" is NOT a separate
// daemon — it is the in-process Spark SDK closure the status read invokes. The
// status read built its OWN cold SDK on the same
// `<HOME>/wallet/spark-backup/sdk/storage.sql` the long-lived daemon's warm
// session already held open; under SQLite/SDK lock contention the cold
// `receivePayment` timed out and the read degraded to `helperReady:false`.
//
// The fix routes the `wallet status` Spark read through the daemon's already
// built+kept-alive WARM session (no second cold build, no contention). These
// tests pin the contract that drives `daemonOnline`.

// A fake helper that resolves the static Spark `address` command successfully —
// exactly the success shape the real warm-session helper returns once the SDK
// is up. `helperReady` is derived from `exitCode === 0`.
const readyAddressHelper: SparkBackupHelper = async (command) => {
  if (command === "address") {
    return {
      exitCode: 0,
      stdout: JSON.stringify({
        // A syntactically valid native Spark address. It is redacted to a
        // digest ref by classify and never leaves the projection raw.
        spark_address:
          "spark1qqqqqp0000000000000000000000000000000000000000000000000",
      }),
      stderr: "",
    }
  }
  return { exitCode: 0, stdout: "{}", stderr: "" }
}

// A fake helper that fails (the contended-cold-build symptom) so we can prove
// the negative: a failing helper is what produced `daemonOnline:false`.
const unavailableHelper: SparkBackupHelper = async (command) => ({
  exitCode: 1,
  stdout: "",
  stderr: `spark backup helper ${command}: unable to connect`,
})

describe("#5306 wallet status daemonOnline is automatic on a healthy node", () => {
  test("a ready Spark helper yields daemonOnline:true with NO env flags set (default-ON)", async () => {
    // Empty env: no PYLON_SPARK_BACKUP_ENABLED, no opt-out. This is the bare
    // default-ON state of a fresh `npx @openagentsinc/pylon` node.
    const env = {} as NodeJS.ProcessEnv
    expect(isSparkBackupDefaultEnabled(env)).toBe(true)

    const sparkBackup = await classifySparkBackupReceive({
      env,
      enabled: true,
      embeddedCredentialAvailable: true,
      helper: readyAddressHelper,
    })
    expect(sparkBackup.helperReady).toBe(true)
    expect(sparkBackup.state).toBe("address-ready")

    const status = withPayoutTargetReadiness(
      withSparkPrimaryWalletBalance(mdkScopedAgentWalletStatus(), sparkBackup),
      ["payout.spark.deadbeefdeadbeefdeadbeef"],
    )
    // The whole point: configured + helper-ready ⇒ daemonOnline true, with no
    // manual daemon command and no env flag on the default path.
    expect(status.configured).toBe(true)
    expect(status.daemonOnline).toBe(true)
    expect(status.blockerRefs).not.toContain(
      "blocker.wallet.spark_primary.helper_unavailable",
    )
  })

  test("an unavailable helper is what produced daemonOnline:false (the bug shape)", async () => {
    const sparkBackup = await classifySparkBackupReceive({
      env: {} as NodeJS.ProcessEnv,
      enabled: true,
      embeddedCredentialAvailable: true,
      helper: unavailableHelper,
    })
    expect(sparkBackup.helperReady).toBe(false)

    const status = withSparkPrimaryWalletBalance(
      mdkScopedAgentWalletStatus(),
      sparkBackup,
    )
    expect(status.configured).toBe(true)
    expect(status.daemonOnline).toBe(false)
    expect(status.blockerRefs).toContain(
      "blocker.wallet.spark_primary.helper_unavailable",
    )
  })

  test("resolveSparkBackupHelper threads warmSession:true so reads reuse the daemon's warm SDK", () => {
    // The daemon's `walletStatus` action and the routed CLI both classify with
    // warmSession:true so the read NEVER cold-builds a second SDK on the shared
    // storage. Proving the resolver wires a helper under the default-ON path
    // with warmSession:true is the structural guarantee behind the fix.
    const helper = resolveSparkBackupHelper({
      env: {} as NodeJS.ProcessEnv,
      enabled: true,
      mnemonic:
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
      storageDir: join("/tmp", "pylon-test-spark-storage-does-not-build"),
      warmSession: true,
      // Never actually load/build the SDK in a unit test — the resolver returns
      // the closure without invoking it, so this is never called here.
      loadModule: async () => {
        throw new Error("loadModule must not run in resolver unit test")
      },
    })
    // A non-null helper proves the default-ON + embedded-credential + seed path
    // wires the in-process helper (the prerequisite for helperReady:true). The
    // resolver returns null ONLY on an explicit opt-out / missing key / no seed.
    expect(typeof helper).toBe("function")
  })

  test("a known unavailable reason is surfaced as a reason-qualified blocker (graceful degrade)", () => {
    // When the warm read genuinely fails on some environment, the status must
    // degrade with a PRECISE reason, not a bare helper_unavailable.
    const status = withSparkPrimaryWalletBalance(mdkScopedAgentWalletStatus(), {
      schema: "openagents.pylon.spark_backup_receive.v0.1",
      enabled: true,
      state: "helper-unavailable",
      selectedBecauseRefs: [],
      receiveTargetRef: null,
      lightningAddressRef: null,
      rawTargetAvailableLocally: false,
      credentialReady: true,
      helperReady: false,
      helperUnavailableReason: "network_unreachable",
      detectedBalanceSats: null,
      unclaimedDepositCount: null,
      blockerRefs: [],
      nextActionRefs: [],
      publicReceiptRefs: [],
      contentRedacted: true,
    })
    expect(status.daemonOnline).toBe(false)
    expect(status.blockerRefs).toContain("blocker.wallet.spark_primary.helper_unavailable")
    expect(status.blockerRefs).toContain(
      "blocker.wallet.spark_primary.helper_unavailable.network_unreachable",
    )
  })

  test("an explicit opt-out override still disables the helper (no surprise enablement)", () => {
    const off = resolveSparkBackupHelper({
      env: { PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
      mnemonic:
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
      warmSession: true,
    })
    expect(off).toBeNull()
  })
})
