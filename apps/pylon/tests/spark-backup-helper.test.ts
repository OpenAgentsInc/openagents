import { describe, expect, test } from "bun:test"
import { classifySparkBackupReceive, prepareSparkBackupReceive } from "../src/wallet"
import { assertPublicProjectionSafe } from "../src/state"
import {
  createSparkBackupHelper,
  createSparkBackupSendTransfer,
  createSparkBackupSweepTransfer,
  isSparkAddress,
  resolveSparkBackupHelper,
} from "../src/spark-backup-helper"

// #5225: a native Spark address (bech32m `spark1…` HRP, ~68 chars on real infra).
// Synthetic but shape-faithful; never a real address and must not leak in refs.
const RAW_SPARK_NATIVE_ADDRESS =
  "spark1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3jn54khce6mua7lqpzr"

const RAW_SPARK_ADDRESS = "sp1pgssy9fakesparkaddressmaterialthatmustnotleakpublicly00000000000000"
const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

// A fake Breez SDK Spark module honoring exactly the receive-only surface the
// adapter uses. No real WASM/network. Records connect/disconnect for lifecycle
// assertions.
function fakeSparkModule(opts: {
  balanceSats?: number
  paymentRequest?: string
  paymentRecords?: ReadonlyArray<unknown>
  payments?: number
  unclaimed?: number
  failConnect?: boolean
  failKind?: string
  onDisconnect?: () => void
  onLnurlPay?: (request: { idempotencyKey?: string; prepareResponse: unknown }) => void
  onParse?: (input: string) => void
  onPrepareLnurlPay?: (request: { amount: bigint; payRequest: unknown }) => void
  onPrepareSend?: (request: { paymentRequest: string; amount?: bigint }) => void
  onSend?: (request: { idempotencyKey?: string; options?: unknown; prepareResponse: unknown }) => void
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
        listPayments: async () => ({
          payments:
            opts.paymentRecords ??
            Array.from({ length: opts.payments ?? 0 }, (_, i) => ({ id: i })),
        }),
        listUnclaimedDeposits: async () => ({
          deposits: Array.from({ length: opts.unclaimed ?? 0 }, (_, i) => ({ txid: String(i), vout: 0 })),
        }),
        parse: async (input: string) => {
          opts.onParse?.(input)
          if (input.includes("@")) {
            return {
              type: "lightningAddress",
              address: input,
              payRequest: {
                callback: "https://spark.example/lnurl/callback",
                minSendable: 1,
                maxSendable: 100_000_000,
                metadataStr: "[]",
                commentAllowed: 0,
                domain: "spark.example",
                url: "https://spark.example/.well-known/lnurlp/test",
                address: input,
              },
            }
          }
          return { type: "bolt11Invoice" }
        },
        prepareLnurlPay: async (request: { amount: bigint; payRequest: unknown }) => {
          opts.onPrepareLnurlPay?.(request)
          return { preparedLnurl: true, amountSats: Number(request.amount), payRequest: request.payRequest }
        },
        lnurlPay: async (request: { idempotencyKey?: string; prepareResponse: unknown }) => {
          opts.onLnurlPay?.(request)
          return {
            payment: {
              id: "spark-lnurl-payment-1",
              amount: BigInt(opts.balanceSats ?? 4242),
              fees: 4n,
              status: "complete",
            },
          }
        },
        prepareSendPayment: async (request: { paymentRequest: string; amount?: bigint }) => {
          opts.onPrepareSend?.(request)
          // #5225: mirror the real SDK — a native `spark1…` destination resolves
          // to a NON-bolt11 payment method (`sparkAddress`); everything else
          // (BOLT11 invoices, LNURL-resolved BOLT11s) resolves to `bolt11Invoice`.
          const type = request.paymentRequest.trim().toLowerCase().startsWith("spark1")
            ? "sparkAddress"
            : "bolt11Invoice"
          return { prepared: true, paymentMethod: { type }, paymentRequest: request.paymentRequest }
        },
        sendPayment: async (request: { idempotencyKey?: string; options?: unknown; prepareResponse: unknown }) => {
          opts.onSend?.(request)
          return {
            payment: {
              id: "spark-payment-1",
              amount: BigInt(opts.balanceSats ?? 4242),
              fees: 3n,
              status: "complete",
            },
          }
        },
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
    expect(JSON.parse(result.stdout)).toEqual({
      balance_sats: 4242,
      balance_synced: true,
      unclaimed_deposit_count: 2,
      claimable_htlc_count: 0,
      claimable_htlc_sats: 0,
    })
  })

  test("status command counts pending waitingForPreimage HTLCs", async () => {
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 0,
          paymentRecords: [
            {
              amount: 50_000n,
              details: {
                htlcDetails: { status: "waitingForPreimage" },
              },
              status: "pending",
            },
            {
              amount: 3_000,
              details: {
                htlcDetails: { status: "completed" },
              },
              status: "pending",
            },
          ],
        }),
    })
    const result = await helper("status")

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toMatchObject({
      balance_sats: 0,
      claimable_htlc_count: 1,
      claimable_htlc_sats: 50_000,
    })
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

  test("sweep transfer pays caller-provided MDK target and returns public refs only", async () => {
    const rawReceiveTarget = "lnbc42420n1rawmdkreceivetargetthatmustneverleakpublicly"
    let preparedTarget: string | null = null
    let sentIdempotencyKey: string | null = null
    const transfer = createSparkBackupSweepTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 4242,
          onPrepareSend: (request) => {
            preparedTarget = request.paymentRequest
            expect(request.amount).toBe(4242n)
          },
          onSend: (request) => {
            sentIdempotencyKey = request.idempotencyKey ?? null
            expect(request.options).toMatchObject({
              type: "bolt11Invoice",
              preferSpark: true,
            })
          },
        }),
    })

    const result = await transfer({
      amountSats: 4242,
      destination: rawReceiveTarget,
      idempotencyKey: "test-sweep-key",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected transfer success")
    expect(preparedTarget).toBe(rawReceiveTarget)
    // #5185: the SDK TransferId must be a valid UUID, not the raw idempotency key.
    expect(sentIdempotencyKey).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    )
    expect(sentIdempotencyKey).not.toBe("test-sweep-key")
    expect(result.transferRef).toMatch(/^wallet\.spark_backup_transfer\.[a-f0-9]{24}$/)
    expect(result.amountSats).toBe(4242)
    expect(result.feeSats).toBe(3)
    expect(JSON.stringify(result)).not.toContain(rawReceiveTarget)
    assertPublicProjectionSafe(result)
  })

  test("send transfer pays a BOLT11/payment request with public refs only", async () => {
    const rawPaymentRequest = "lnbc42420n1rawpaymentrequestthatmustneverleakpublicly"
    let preparedTarget: string | null = null
    let sentIdempotencyKey: string | null = null
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 2100,
          onPrepareSend: (request) => {
            preparedTarget = request.paymentRequest
            expect(request.amount).toBe(2100n)
          },
          onSend: (request) => {
            sentIdempotencyKey = request.idempotencyKey ?? null
            expect(request.options).toMatchObject({
              type: "bolt11Invoice",
              preferSpark: true,
            })
          },
        }),
    })

    const result = await transfer({
      amountSats: 2100,
      destination: rawPaymentRequest,
      idempotencyKey: "test-send-key",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected transfer success")
    expect(preparedTarget).toBe(rawPaymentRequest)
    // #5185: the SDK TransferId must be a valid UUID, not the raw idempotency key.
    expect(sentIdempotencyKey).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    )
    expect(sentIdempotencyKey).not.toBe("test-send-key")
    expect(result.transferRef).toMatch(/^wallet\.spark_backup_send\.[a-f0-9]{24}$/)
    expect(result.sparkPaymentRef).toMatch(/^wallet\.spark_backup_send_payment\.[a-f0-9]{24}$/)
    expect(result.amountSats).toBe(2100)
    expect(result.feeSats).toBe(3)
    expect(result.method).toBe("payment_request")
    expect(JSON.stringify(result)).not.toContain(rawPaymentRequest)
    assertPublicProjectionSafe(result)
  })

  test("isSparkAddress recognizes spark1 addresses and rejects everything else (#5225)", () => {
    expect(isSparkAddress(RAW_SPARK_NATIVE_ADDRESS)).toBe(true)
    // Tolerates surrounding whitespace / case like the Lightning-address helper.
    expect(isSparkAddress(`  ${RAW_SPARK_NATIVE_ADDRESS.toUpperCase()}  `)).toBe(true)
    // Not a Spark address: BOLT11, Lightning Address, empty, wrong HRP.
    expect(isSparkAddress("lnbc42420n1rawpaymentrequest")).toBe(false)
    expect(isSparkAddress("oa12345@spark.example")).toBe(false)
    expect(isSparkAddress("")).toBe(false)
    expect(isSparkAddress("spark1")).toBe(false)
    expect(isSparkAddress("bc1qexampleonchainaddressnotspark")).toBe(false)
  })

  test("send transfer routes a native Spark address Spark→Spark (spark_native), no LNURL, no Lightning options/fallback (#5225)", async () => {
    let preparedTarget: string | null = null
    let sentIdempotencyKey: string | null = null
    let sentOptions: unknown = "unset"
    let parseTouched = false
    let lnurlTouched = false
    let sendCalls = 0
    // The LNURL resolve path must never be touched for a native Spark address.
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (url: string | URL) => {
      throw new Error(`fetch must not be called for a native Spark send: ${String(url)}`)
    }) as typeof fetch
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 3000,
            onParse: () => {
              parseTouched = true
            },
            onLnurlPay: () => {
              lnurlTouched = true
            },
            onPrepareSend: (request) => {
              preparedTarget = request.paymentRequest
              expect(request.amount).toBe(3000n)
            },
            onSend: (request) => {
              sendCalls += 1
              sentIdempotencyKey = request.idempotencyKey ?? null
              sentOptions = request.options
            },
          }),
      })

      const result = await transfer({
        amountSats: 3000,
        destination: RAW_SPARK_NATIVE_ADDRESS,
        idempotencyKey: "test-spark-native-key",
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected native Spark transfer success")
      // Native classification.
      expect(result.method).toBe("spark_native")
      // The destination went straight to prepare — NOT through LNURL resolve.
      expect(preparedTarget).toBe(RAW_SPARK_NATIVE_ADDRESS)
      expect(parseTouched).toBe(false)
      expect(lnurlTouched).toBe(false)
      // sendPayment was called exactly once with NO bolt11Invoice/preferSpark
      // options (native rail) and NEVER a preferSpark:false Lightning fallback.
      expect(sendCalls).toBe(1)
      expect(sentOptions).toBeUndefined()
      // Valid-UUID TransferId, not the raw idempotency key (#5185 preserved).
      expect(sentIdempotencyKey).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      )
      expect(sentIdempotencyKey).not.toBe("test-spark-native-key")
      expect(result.transferRef).toMatch(/^wallet\.spark_backup_send\.[a-f0-9]{24}$/)
      expect(result.sparkPaymentRef).toMatch(/^wallet\.spark_backup_send_payment\.[a-f0-9]{24}$/)
      expect(result.amountSats).toBe(3000)
      // Native Spark→Spark carries no Lightning routing fee; the fake reports 3.
      expect(JSON.stringify(result)).not.toContain(RAW_SPARK_NATIVE_ADDRESS)
      assertPublicProjectionSafe(result)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test("send transfer resolves a Lightning Address to a BOLT11 and pays it via sendPayment (#5195)", async () => {
    const rawLightningAddress = "oa12345@spark.example"
    const resolvedBolt11 = "lnbc50u1mockinvoicefromthelnurlcallbackthatmustnotleak"
    let preparedTarget: string | null = null
    let sentIdempotencyKey: string | null = null
    let calledAmountMsat: string | null = null
    const originalFetch = globalThis.fetch
    globalThis.fetch = (async (url: string | URL) => {
      const u = typeof url === "string" ? url : url.toString()
      if (u.includes("/.well-known/lnurlp/")) {
        return new Response(
          JSON.stringify({
            tag: "payRequest",
            callback: "https://spark.example/cb",
            minSendable: 1000,
            maxSendable: 1_000_000_000,
          }),
          { status: 200 },
        )
      }
      calledAmountMsat = new URL(u).searchParams.get("amount")
      return new Response(JSON.stringify({ pr: resolvedBolt11 }), { status: 200 })
    }) as typeof fetch
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 5000,
            onPrepareSend: (request) => {
              preparedTarget = request.paymentRequest
            },
            onSend: (request) => {
              sentIdempotencyKey = request.idempotencyKey ?? null
            },
          }),
      })

      const result = await transfer({
        amountSats: 5000,
        destination: rawLightningAddress,
        idempotencyKey: "test-lnurl-send-key",
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected transfer success")
      // #5195: the LA was resolved to a BOLT11 via LNURL-pay and paid through
      // sendPayment (NOT the SDK lnurlPay), with the amount in msat.
      expect(preparedTarget).toBe(resolvedBolt11)
      expect(calledAmountMsat).toBe("5000000")
      expect(result.method).toBe("lnurl_pay")
      // valid-UUID TransferId, not the raw idempotency key.
      expect(sentIdempotencyKey).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      )
      expect(sentIdempotencyKey).not.toBe("test-lnurl-send-key")
      expect(result.transferRef).toMatch(/^wallet\.spark_backup_send\.[a-f0-9]{24}$/)
      expect(result.amountSats).toBe(5000)
      expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
      expect(JSON.stringify(result)).not.toContain(resolvedBolt11)
      assertPublicProjectionSafe(result)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test("send transfer reports a CLEAR lnurl_resolve failure (not send-pending) when an external LNURL host hangs (#5195 follow-up)", async () => {
    // Simulate a slow/aborting external LNURL server: the metadata fetch aborts
    // via the per-fetch AbortSignal. This must surface as a distinct, clean
    // resolve FAILURE (ok:false with reason) classified as a normal failure ref,
    // NOT the generic "timed out" that the outer catch maps to INDETERMINATE
    // (send-pending).
    const rawLightningAddress = "someone@slowhost.example"
    const originalFetch = globalThis.fetch
    let sawSignal = false
    globalThis.fetch = (async (_url: string | URL, init?: { signal?: AbortSignal }) => {
      sawSignal = init?.signal instanceof AbortSignal
      throw new DOMException("The operation timed out.", "TimeoutError")
    }) as typeof fetch
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () => fakeSparkModule({ balanceSats: 5000 }),
      })

      const result = await transfer({
        amountSats: 500,
        destination: rawLightningAddress,
        idempotencyKey: "test-lnurl-hang-key",
      })

      // A resolve failure is a REAL failure with a reason, never indeterminate.
      expect(result.ok).toBe(false)
      if (result.ok) throw new Error("expected resolve failure")
      expect(result.failureRef).toMatch(/^wallet\.spark_backup_send_failure\.[a-f0-9]{24}$/)
      expect(result.failureRef).not.toContain("indeterminate")
      expect(sawSignal).toBe(true)
      expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
      assertPublicProjectionSafe(result)
    } finally {
      globalThis.fetch = originalFetch
    }
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

  test("uses the embedded default Breez key when enabled with no env credential", () => {
    // The embedded DEFAULT_OPENAGENTS_SPARK_API_KEY (committed historically,
    // owner-authorized) is the final fallback, so an enabled node with a seed
    // resolves a helper even with no env key. Inert-by-default now rests on the
    // PYLON_SPARK_BACKUP_ENABLED flag, NOT on key presence.
    expect(
      resolveSparkBackupHelper({ env: { PYLON_SPARK_BACKUP_ENABLED: "1" } as NodeJS.ProcessEnv, mnemonic: TEST_MNEMONIC }),
    ).not.toBeNull()
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
