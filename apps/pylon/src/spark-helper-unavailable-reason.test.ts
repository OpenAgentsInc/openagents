// #5194 regression: a Spark read that degrades to `helper-unavailable` must
// surface a bounded, public-safe REASON so the operator can tell a corrupt
// local wallet DB (storage init failure, upstream of getInfo) from a blocked
// network or a module that did not load. Before this fix the read returned a
// bare `helperReady:false` with no explanation — the exact wall that blocked a
// contributor from registering a Spark payout target.
import { describe, expect, it } from "bun:test"
import {
  classifySparkHelperFailureReason,
  createSparkBackupHelper,
} from "./spark-backup-helper.js"
import { classifySparkBackupReceive, detectSparkBackupBalance } from "./wallet.js"

// A fake module whose SDK build() throws — the failure happens UPSTREAM of
// getInfo, exactly like the corrupt-`storage.sql` case on the reporting host.
const failingBuildModule = (buildError: Error) =>
  (async () => ({
    defaultConfig: () => ({}),
    SdkBuilder: {
      new: () => ({
        withStorage: () => ({
          build: async () => {
            throw buildError
          },
        }),
      }),
    },
  })) as never

describe("classifySparkHelperFailureReason (#5194)", () => {
  it("maps a storage-init / migration failure to db_init_failed", () => {
    expect(
      classifySparkHelperFailureReason(
        "spark backup helper status: Failed to initialize database at '/x/storage.sql': Migration failed at version 0: file is not a database",
      ),
    ).toBe("db_init_failed")
  })
  it("maps a withTimeout failure to timeout", () => {
    expect(classifySparkHelperFailureReason("spark sdk build timed out")).toBe("timeout")
  })
  it("maps a connect/network failure to network_unreachable", () => {
    expect(
      classifySparkHelperFailureReason("Unable to connect. Is the computer able to access the url?"),
    ).toBe("network_unreachable")
  })
  it("maps a module-load failure to module_load_failed", () => {
    expect(
      classifySparkHelperFailureReason("Cannot find module '@breeztech/breez-sdk-spark'"),
    ).toBe("module_load_failed")
  })
  it("falls back to unknown for an unrecognized message and empty input", () => {
    expect(classifySparkHelperFailureReason("some entirely novel error")).toBe("unknown")
    expect(classifySparkHelperFailureReason("")).toBe("unknown")
  })
})

describe("spark read paths surface a reason on helper-unavailable (#5194)", () => {
  const dbErr = new Error(
    "Failed to initialize database at '/home/.pylon/spark/storage.sql': Migration failed at version 0: file is not a database",
  )

  it("register-payout address path reports db_init_failed (was a silent helper-unavailable)", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: "seed words here",
      loadModule: failingBuildModule(dbErr),
      timeoutMs: 5000,
    })
    const projection = await classifySparkBackupReceive({
      enabled: true,
      helper,
      kind: "spark-address",
      embeddedCredentialAvailable: true,
    })
    expect(projection.state).toBe("helper-unavailable")
    expect(projection.helperReady).toBe(false)
    // The fix: a bounded, public-safe reason instead of nothing.
    expect(projection.helperUnavailableReason).toBe("db_init_failed")
    // Safety: the raw private message (the storage path / SDK internals) is NOT
    // in the projection — only the bounded enum reason. (The schema string
    // intentionally contains the literal "openagents.pylon.*"; we assert on the
    // private filesystem path fragment, not that substring.)
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain("storage.sql")
    expect(serialized).not.toContain("/home/.pylon/")
    expect(serialized).not.toContain("Migration failed")
  })

  it("backup-status (status) read reports db_init_failed instead of a silent failure", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: "seed words here",
      loadModule: failingBuildModule(dbErr),
      timeoutMs: 5000,
    })
    const detected = await detectSparkBackupBalance(helper)
    expect(detected.helperReady).toBe(false)
    expect(detected.detectedBalanceSats).toBe(null)
    expect(detected.helperUnavailableReason).toBe("db_init_failed")
  })

  it("a healthy read carries no failure reason", async () => {
    const okSdk = {
      syncWallet: async () => ({}),
      getInfo: async () => ({ balanceSats: 0 }),
      listUnclaimedDeposits: async () => ({ deposits: [] }),
      listPayments: async () => ({ payments: [] }),
      receivePayment: async () => ({ paymentRequest: "spark1examplexxxxxxxxxxxxxxxxxxxxxxxxxxxx" }),
      disconnect: async () => {},
    }
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: "seed words here",
      loadModule: (async () => ({
        defaultConfig: () => ({}),
        SdkBuilder: { new: () => ({ withStorage: () => ({ build: async () => okSdk }) }) },
      })) as never,
      timeoutMs: 5000,
    })
    const detected = await detectSparkBackupBalance(helper)
    expect(detected.helperReady).toBe(true)
    expect(detected.helperUnavailableReason).toBe(null)
  })
})
