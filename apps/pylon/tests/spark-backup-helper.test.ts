import { describe, expect, test } from "bun:test"
import { classifySparkBackupReceive, prepareSparkBackupReceive } from "../src/wallet"
import { assertPublicProjectionSafe } from "../src/state"
import { createSparkBackupHelper, resolveSparkBackupHelper } from "../src/spark-backup-helper"

const RAW_SPARK_ADDRESS = "sp1pgssy9fakesparkaddressmaterialthatmustnotleakpublicly00000000000000"
const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

// A fake Breez SDK Spark module honoring exactly the receive-only surface the
// adapter uses. No real WASM/network. Records connect/disconnect for lifecycle
// assertions.
function fakeSparkModule(opts: {
  balanceSats?: number
  paymentRequest?: string
  payments?: number
  unclaimed?: number
  failConnect?: boolean
  failKind?: string
  onDisconnect?: () => void
}) {
  let connected = false
  return {
    defaultConfig: (network: string) => ({ network, apiKey: undefined as string | undefined }),
    connect: async (req: { config: { apiKey?: string }; seed: { mnemonic: string } }) => {
      if (opts.failConnect) throw new Error(opts.failKind ?? "connect failed")
      if (!req.config.apiKey) throw new Error("missing breez api key")
      if (!req.seed.mnemonic) throw new Error("missing wallet seed")
      connected = true
      return {
        getInfo: async () => ({ balanceSats: opts.balanceSats ?? 0 }),
        receivePayment: async (r: { paymentMethod: { type: string } }) => {
          if (r.paymentMethod.type !== "sparkAddress") throw new Error("only sparkAddress receive")
          return { paymentRequest: opts.paymentRequest ?? RAW_SPARK_ADDRESS, fee: 0n }
        },
        listPayments: async () => ({ payments: Array.from({ length: opts.payments ?? 0 }, (_, i) => ({ id: i })) }),
        listUnclaimedDeposits: async () => ({
          deposits: Array.from({ length: opts.unclaimed ?? 0 }, (_, i) => ({ txid: String(i), vout: 0 })),
        }),
        disconnect: () => {
          if (!connected) return
          opts.onDisconnect?.()
          connected = false
        },
      }
    },
  }
}

describe("Spark backup helper adapter (slice 2: real Breez SDK contract via fake)", () => {
  test("address command maps SDK paymentRequest to spark_address helper JSON", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ paymentRequest: RAW_SPARK_ADDRESS }),
    })
    const result = await helper("address")
    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({ spark_address: RAW_SPARK_ADDRESS })
  })

  test("status command maps getInfo + unclaimed deposits", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ balanceSats: 4242, unclaimed: 2 }),
    })
    const result = await helper("status")
    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({ balance_sats: 4242, unclaimed_deposit_count: 2 })
  })

  test("history command returns only a count, never raw payment material", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ payments: 3 }),
    })
    const result = await helper("history")
    expect(JSON.parse(result.stdout)).toEqual({ payment_count: 3 })
  })

  test("unclaimed-deposits command returns the deposit count", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ unclaimed: 5 }),
    })
    const result = await helper("unclaimed-deposits")
    expect(JSON.parse(result.stdout)).toEqual({ unclaimed_deposit_count: 5 })
  })

  test("missing API key fails before loading the SDK (inert)", async () => {
    let loaded = false
    const helper = createSparkBackupHelper({
      apiKey: "",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => {
        loaded = true
        return fakeSparkModule({})
      },
    })
    const result = await helper("address")
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("missing breez api key")
    expect(loaded).toBe(false)
  })

  test("missing mnemonic fails before loading the SDK", async () => {
    let loaded = false
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: "",
      loadModule: async () => {
        loaded = true
        return fakeSparkModule({})
      },
    })
    const result = await helper("address")
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("missing wallet seed")
    expect(loaded).toBe(false)
  })

  test("connect failure degrades to a failed helper result (no throw)", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ failConnect: true, failKind: "spark daemon unreachable" }),
    })
    const result = await helper("address")
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("spark daemon unreachable")
  })

  test("disconnects the short-lived SDK session after each command", async () => {
    let disconnects = 0
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ onDisconnect: () => { disconnects += 1 } }),
    })
    await helper("address")
    await helper("status")
    expect(disconnects).toBe(2)
  })

  test("a missing-credential SDK error feeds slice-1 credential classification path", async () => {
    // When the SDK reports a missing key, the helper stderr carries it; the
    // adapter never throws. Slice-1 classify only calls the helper after the
    // env credential gate passes, so this proves the adapter is safe to call.
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ failConnect: true, failKind: "Missing Breez API key" }),
    })
    const projection = await classifySparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
    })
    expect(projection.state).toBe("helper-unavailable")
    assertPublicProjectionSafe(projection)
  })

  test("adapter integrates with slice-1 prepare flow, redacting raw target without --show-local-target", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ paymentRequest: RAW_SPARK_ADDRESS }),
    })
    const withoutFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
    })
    expect(withoutFlag.ok).toBe(true)
    expect(withoutFlag.localTarget).toBeUndefined()
    expect(JSON.stringify(withoutFlag.projection)).not.toContain(RAW_SPARK_ADDRESS)
    assertPublicProjectionSafe(withoutFlag.projection)

    const withFlag = await prepareSparkBackupReceive({
      enabled: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper,
      showLocalTarget: true,
    })
    expect(withFlag.localTarget).toBe(RAW_SPARK_ADDRESS)
    expect(JSON.stringify(withFlag.projection)).not.toContain(RAW_SPARK_ADDRESS)
  })
})

describe("resolveSparkBackupHelper (inert-by-default gate)", () => {
  test("returns null when not opt-in enabled", () => {
    expect(
      resolveSparkBackupHelper({ env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv, mnemonic: TEST_MNEMONIC }),
    ).toBeNull()
  })

  test("returns null when enabled but no credential", () => {
    expect(
      resolveSparkBackupHelper({ env: { PYLON_SPARK_BACKUP_ENABLED: "1" } as NodeJS.ProcessEnv, mnemonic: TEST_MNEMONIC }),
    ).toBeNull()
  })

  test("returns null when enabled + credential but no seed", () => {
    expect(
      resolveSparkBackupHelper({
        env: { PYLON_SPARK_BACKUP_ENABLED: "1", OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        mnemonic: null,
      }),
    ).toBeNull()
  })

  test("returns a helper when enabled + credential + seed are all present", () => {
    const helper = resolveSparkBackupHelper({
      env: { PYLON_SPARK_BACKUP_ENABLED: "1", OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ paymentRequest: RAW_SPARK_ADDRESS }),
    })
    expect(typeof helper).toBe("function")
  })

  test("the resolved helper actually maps SDK results through the contract", async () => {
    const helper = resolveSparkBackupHelper({
      env: { PYLON_SPARK_BACKUP_ENABLED: "1", OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      mnemonic: TEST_MNEMONIC,
      loadModule: async () => fakeSparkModule({ paymentRequest: RAW_SPARK_ADDRESS }),
    })
    expect(helper).not.toBeNull()
    const result = await helper!("address")
    expect(JSON.parse(result.stdout)).toEqual({ spark_address: RAW_SPARK_ADDRESS })
  })
})
