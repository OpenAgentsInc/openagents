import { describe, expect, test } from "bun:test"
import { classifySparkBackupReceive, detectSparkBackupBalance, prepareSparkBackupReceive } from "../src/wallet"
import { assertPublicProjectionSafe } from "../src/state"
import {
  createSparkBackupHelper,
  createSparkBackupSendTransfer,
  createSparkBackupSweepTransfer,
  domainFromLightningAddress,
  evaluateDomainFeePolicy,
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
  // #5250: fee fields the real SDK surfaces on the PREPARED payment method
  // (`prepareResponse.paymentMethod`). For a bolt11 method the real Lightning
  // fee is `lightningFeeSats` (+ optional `sparkTransferFeeSats`); a spark
  // address/invoice carries a flat `fee` string.
  preparedLightningFeeSats?: number
  preparedSparkTransferFeeSats?: number
  preparedFee?: string
  // #5250: override the settled send-RESULT fee (`payment.fees`). The rc.22 bug
  // reproduced with this 0/absent while a real fee was charged at prepare time.
  sendResultFees?: bigint | null
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
          // #5250: mirror the SDK `SendPaymentMethod` union — the real computed
          // fee lives on the prepared payment method. Inject it when the test
          // provides one so we can exercise the prepared-fee reconciliation.
          const paymentMethod: Record<string, unknown> = { type }
          if (opts.preparedLightningFeeSats !== undefined) {
            paymentMethod.lightningFeeSats = opts.preparedLightningFeeSats
          }
          if (opts.preparedSparkTransferFeeSats !== undefined) {
            paymentMethod.sparkTransferFeeSats = opts.preparedSparkTransferFeeSats
          }
          if (opts.preparedFee !== undefined) {
            paymentMethod.fee = opts.preparedFee
          }
          return { prepared: true, paymentMethod, paymentRequest: request.paymentRequest }
        },
        sendPayment: async (request: { idempotencyKey?: string; options?: unknown; prepareResponse: unknown }) => {
          opts.onSend?.(request)
          return {
            payment: {
              id: "spark-payment-1",
              amount: BigInt(opts.balanceSats ?? 4242),
              // #5250: default 3 (legacy fake fee); a test may force this to 0/null
              // to reproduce the rc.22 send-result-reports-no-fee case.
              fees: opts.sendResultFees === undefined ? 3n : opts.sendResultFees,
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

  test("send fee reconciles from the PREPARED Lightning fee when the send result reports fee 0 (#5250)", async () => {
    // rc.22 regression: a 44-sat Lightning-Address send reported feeSats:0 but the
    // balance dropped 4,140 (= 44 + 4096). The 4,096 was the real Lightning/LSP
    // routing fee surfaced at PREPARE time on `paymentMethod.lightningFeeSats`;
    // the settled send result reported fees:0. feeSats MUST reflect 4096, not 0.
    const rawLightningAddress = "oa12345@spark.example"
    const resolvedBolt11 = "lnbc44n1mockinvoicefromthelnurlcallbackthatmustnotleak5250"
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
      return new Response(JSON.stringify({ pr: resolvedBolt11 }), { status: 200 })
    }) as typeof fetch
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 44,
            // The real computed fee is on the prepared bolt11 method ...
            preparedLightningFeeSats: 4096,
            // ... while the settled send result reports NO fee (the rc.22 bug).
            sendResultFees: 0n,
          }),
      })

      const result = await transfer({
        amountSats: 44,
        destination: rawLightningAddress,
        idempotencyKey: "test-5250-lnurl-fee",
        // #5254: this 44/4096 case is EXACTLY what the pre-send fee guard now
        // rejects by default. To keep this test focused on #5250's prepared-fee
        // RECONCILIATION (not the guard), raise the ceiling with an explicit
        // operator override so the send proceeds and the fee reconciles. The
        // default-guard rejection of this same case is covered by the dedicated
        // #5254 test below.
        maxFeeSats: 4096,
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected transfer success")
      expect(result.method).toBe("lnurl_pay")
      // The fee is the REAL prepared Lightning fee, NOT the 0 from the send result.
      expect(result.feeSats).toBe(4096)
      expect(result.feeFromPrepared).toBe(true)
      // amount + fee reconciles with the observed 4,140-sat balance delta.
      expect((result.amountSats ?? 0) + (result.feeSats ?? 0)).toBe(4140)
      expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
      expect(JSON.stringify(result)).not.toContain(resolvedBolt11)
      assertPublicProjectionSafe(result)
    } finally {
      globalThis.fetch = originalFetch
    }
  })

  test("send fee sums prepared lightning + spark-transfer fee components when the send result is 0 (#5250)", async () => {
    const rawPaymentRequest = "lnbc1000n1rawpaymentrequestthatmustneverleakpublicly5250"
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 1000,
          preparedLightningFeeSats: 12,
          preparedSparkTransferFeeSats: 4,
          sendResultFees: null,
        }),
    })

    const result = await transfer({
      amountSats: 1000,
      destination: rawPaymentRequest,
      idempotencyKey: "test-5250-bolt11-sum",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected transfer success")
    expect(result.method).toBe("payment_request")
    // 12 (lightning) + 4 (spark transfer) = 16; both prepared components count.
    expect(result.feeSats).toBe(16)
    expect(result.feeFromPrepared).toBe(true)
    expect(JSON.stringify(result)).not.toContain(rawPaymentRequest)
    assertPublicProjectionSafe(result)
  })

  test("send fee TRUSTS a real non-zero send-result fee over the prepared fee (#5250)", async () => {
    const rawPaymentRequest = "lnbc1000n1rawpaymentrequestthatmustneverleakpublicly5250b"
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 1000,
          // Prepared estimate differs from the settled fee; the settled fee wins.
          preparedLightningFeeSats: 99,
          sendResultFees: 7n,
        }),
    })

    const result = await transfer({
      amountSats: 1000,
      destination: rawPaymentRequest,
      idempotencyKey: "test-5250-settled-wins",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected transfer success")
    // The authoritative settled fee (7) wins over the prepared estimate (99).
    expect(result.feeSats).toBe(7)
    expect(result.feeFromPrepared).toBe(false)
    assertPublicProjectionSafe(result)
  })

  // -------------------------------------------------------------------------
  // #5254 — PRE-SEND FEE GUARD.
  // -------------------------------------------------------------------------

  test("REJECTS the 44-sat / 4096-fee send PRE-DISPATCH (fee_too_high), sendPayment NOT called, zero movement (#5254)", async () => {
    // The rc.22 money-path bug: a 44-sat external send carried a REAL 4,096-sat
    // prepared Lightning fee (>93x the amount). The guard must refuse it BEFORE
    // sendPayment so zero sats move.
    const rawPaymentRequest = "lnbc44n1rawpaymentrequestthatmustneverleakpublicly5254"
    let sendCalls = 0
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 44,
          // Real computed fee surfaced on the prepared bolt11 method.
          preparedLightningFeeSats: 4096,
          onSend: () => {
            sendCalls += 1
          },
        }),
    })

    const result = await transfer({
      amountSats: 44,
      destination: rawPaymentRequest,
      idempotencyKey: "test-5254-reject",
    })

    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected fee-guard rejection")
    // Operator-legible, public-safe failure ref: integers only, carries the reason.
    expect(result.failureRef).toBe("wallet.spark_backup_send.fee_too_high:prepared=4096:amount=44")
    // CRITICAL: the send was refused before dispatch — sendPayment never ran.
    expect(sendCalls).toBe(0)
    expect(JSON.stringify(result)).not.toContain(rawPaymentRequest)
    assertPublicProjectionSafe(result)
  })

  test("PASSES a normal send with a sane fee and dispatches it (#5254)", async () => {
    // amount 1000, prepared fee 16 → ceiling max(50, 500)=500 → 16 ≤ 500 → PASS.
    const rawPaymentRequest = "lnbc1000n1rawpaymentrequestthatmustneverleakpublicly5254ok"
    let sendCalls = 0
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 1000,
          preparedLightningFeeSats: 12,
          preparedSparkTransferFeeSats: 4,
          sendResultFees: null,
          onSend: () => {
            sendCalls += 1
          },
        }),
    })

    const result = await transfer({
      amountSats: 1000,
      destination: rawPaymentRequest,
      idempotencyKey: "test-5254-pass",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected normal send to pass")
    expect(result.feeSats).toBe(16)
    // The send actually dispatched (guard did not block a legitimate send).
    expect(sendCalls).toBe(1)
    assertPublicProjectionSafe(result)
  })

  test("PASSES a native spark_native send (fee 0) trivially — never rejected (#5254, #5225)", async () => {
    let sendCalls = 0
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 5000,
          // No prepared fee fields → a native Spark send carries fee 0.
          sendResultFees: 0n,
          onSend: () => {
            sendCalls += 1
          },
        }),
    })

    const result = await transfer({
      amountSats: 1,
      destination: RAW_SPARK_NATIVE_ADDRESS,
      idempotencyKey: "test-5254-native",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected native send to pass")
    expect(result.method).toBe("spark_native")
    // Fee 0 ≤ any ceiling: a 1-sat native send is never blocked by the guard.
    expect(result.feeSats).toBe(0)
    expect(sendCalls).toBe(1)
    expect(JSON.stringify(result)).not.toContain(RAW_SPARK_NATIVE_ADDRESS)
    assertPublicProjectionSafe(result)
  })

  test("OPERATOR OVERRIDE (maxFeeSats) forces the 44/4096 send through (#5254)", async () => {
    // The same 44/4096 case the guard rejects by default proceeds when the
    // operator explicitly raises the ceiling via the per-call maxFeeSats input.
    const rawPaymentRequest = "lnbc44n1rawpaymentrequestthatmustneverleakpublicly5254ovr"
    let sendCalls = 0
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 44,
          preparedLightningFeeSats: 4096,
          sendResultFees: 0n,
          onSend: () => {
            sendCalls += 1
          },
        }),
    })

    const result = await transfer({
      amountSats: 44,
      destination: rawPaymentRequest,
      idempotencyKey: "test-5254-override",
      maxFeeSats: 5000,
    })

    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected override to force the send through")
    // The expensive send dispatched and the fee reconciled from the prepared method.
    expect(result.feeSats).toBe(4096)
    expect(result.feeFromPrepared).toBe(true)
    expect(sendCalls).toBe(1)
    assertPublicProjectionSafe(result)
  })

  test("OPERATOR OVERRIDE via PYLON_SPARK_MAX_FEE_SATS env forces the 44/4096 send through (#5254)", async () => {
    const rawPaymentRequest = "lnbc44n1rawpaymentrequestthatmustneverleakpublicly5254env"
    let sendCalls = 0
    const transfer = createSparkBackupSendTransfer({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      loadModule: async () =>
        fakeSparkModule({
          balanceSats: 44,
          preparedLightningFeeSats: 4096,
          sendResultFees: 0n,
          onSend: () => {
            sendCalls += 1
          },
        }),
    })

    const priorEnv = process.env.PYLON_SPARK_MAX_FEE_SATS
    process.env.PYLON_SPARK_MAX_FEE_SATS = "5000"
    try {
      const result = await transfer({
        amountSats: 44,
        destination: rawPaymentRequest,
        idempotencyKey: "test-5254-env-override",
      })
      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected env override to force the send through")
      expect(result.feeSats).toBe(4096)
      expect(sendCalls).toBe(1)
      assertPublicProjectionSafe(result)
    } finally {
      if (priorEnv === undefined) delete process.env.PYLON_SPARK_MAX_FEE_SATS
      else process.env.PYLON_SPARK_MAX_FEE_SATS = priorEnv
    }
  })

  // -------------------------------------------------------------------------
  // #5257 — PER-DESTINATION-DOMAIN FEE POLICY (the "blacklist") + attribution.
  // -------------------------------------------------------------------------

  // Mock the LNURL-pay resolve fetch (GET /.well-known/lnurlp/<name> -> callback)
  // so an LA destination resolves to a BOLT11 that the SDK fake then prepares.
  const installLnurlFetch = (resolvedBolt11: string) => {
    const original = globalThis.fetch
    globalThis.fetch = (async (url: string | URL) => {
      const u = typeof url === "string" ? url : url.toString()
      if (u.includes("/.well-known/lnurlp/")) {
        return new Response(
          JSON.stringify({
            tag: "payRequest",
            callback: "https://bitnob.io/cb",
            minSendable: 1,
            maxSendable: 1_000_000_000,
          }),
          { status: 200 },
        )
      }
      return new Response(JSON.stringify({ pr: resolvedBolt11 }), { status: 200 })
    }) as typeof fetch
    return () => {
      globalThis.fetch = original
    }
  }

  const withDenyEnv = async (key: string, value: string | undefined, fn: () => Promise<void>) => {
    const prior = process.env[key]
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
    try {
      await fn()
    } finally {
      if (prior === undefined) delete process.env[key]
      else process.env[key] = prior
    }
  }

  test("REFUSES an LA send to a DENIED domain pre-dispatch (destination_fee_policy), no LNURL resolve, zero movement (#5257)", async () => {
    const rawLightningAddress = "alice@bitnob.io"
    let sendCalls = 0
    let fetched = false
    const original = globalThis.fetch
    globalThis.fetch = (async (url: string | URL) => {
      fetched = true
      throw new Error(`a denied domain must not trigger LNURL resolve: ${String(url)}`)
    }) as typeof fetch
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({ balanceSats: 5000, onSend: () => { sendCalls += 1 } }),
      })
      await withDenyEnv("PYLON_SPARK_DENY_DOMAINS", "foo.example,bitnob.io", async () => {
        const result = await transfer({
          amountSats: 5000,
          destination: rawLightningAddress,
          idempotencyKey: "test-5257-deny",
        })
        expect(result.ok).toBe(false)
        if (result.ok) throw new Error("expected domain-policy refusal")
        // Public-safe failure ref carries the BARE domain, never the full LA.
        expect(result.failureRef).toBe("wallet.spark_backup_send.destination_fee_policy:bitnob.io")
        expect(result.failureRef).not.toContain("alice@")
        // Refused pre-resolve AND pre-dispatch: no LNURL fetch, no sendPayment.
        expect(fetched).toBe(false)
        expect(sendCalls).toBe(0)
        expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
        assertPublicProjectionSafe(result)
      })
    } finally {
      globalThis.fetch = original
    }
  })

  test("REFUSES an LA send that violates the per-domain fee bound pre-dispatch (destination_fee_policy), zero movement (#5257)", async () => {
    // amount 1000, prepared fee 200, PYLON_SPARK_DOMAIN_FEE_MAX_PCT=10 →
    // bound max(50, 100)=100 → 200 > 100 → REFUSE on the domain bound.
    // (The #5254 magnitude guard alone would PASS this: 200 ≤ max(50, 500)=500.)
    const rawLightningAddress = "bob@bitnob.io"
    const resolvedBolt11 = "lnbc1000n1mock5257feeboundinvoicethatmustnotleak"
    let sendCalls = 0
    const restore = installLnurlFetch(resolvedBolt11)
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 1000,
            preparedLightningFeeSats: 200,
            sendResultFees: 0n,
            onSend: () => { sendCalls += 1 },
          }),
      })
      await withDenyEnv("PYLON_SPARK_DOMAIN_FEE_MAX_PCT", "10", async () => {
        const result = await transfer({
          amountSats: 1000,
          destination: rawLightningAddress,
          idempotencyKey: "test-5257-feebound",
        })
        expect(result.ok).toBe(false)
        if (result.ok) throw new Error("expected per-domain fee-bound refusal")
        expect(result.failureRef).toBe("wallet.spark_backup_send.destination_fee_policy:bitnob.io")
        expect(sendCalls).toBe(0)
        expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
        expect(JSON.stringify(result)).not.toContain(resolvedBolt11)
        assertPublicProjectionSafe(result)
      })
    } finally {
      restore()
    }
  })

  test("PASSES an allowed/normal LA domain and dispatches, attributing the domain (#5257)", async () => {
    const rawLightningAddress = "carol@bitnob.io"
    const resolvedBolt11 = "lnbc1000n1mock5257okinvoicethatmustnotleak"
    let sendCalls = 0
    const restore = installLnurlFetch(resolvedBolt11)
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 1000,
            preparedLightningFeeSats: 12,
            sendResultFees: null,
            onSend: () => { sendCalls += 1 },
          }),
      })
      // No deny env set → a normal domain with a sane fee passes and dispatches.
      const result = await transfer({
        amountSats: 1000,
        destination: rawLightningAddress,
        idempotencyKey: "test-5257-pass",
      })
      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected normal LA send to pass")
      expect(result.method).toBe("lnurl_pay")
      expect(sendCalls).toBe(1)
      // The bare domain is attributed; the full LA never appears.
      expect(result.destinationDomain).toBe("bitnob.io")
      expect(JSON.stringify(result)).not.toContain(rawLightningAddress)
      expect(JSON.stringify(result)).not.toContain(resolvedBolt11)
      assertPublicProjectionSafe(result)
    } finally {
      restore()
    }
  })

  test("OPERATOR ALLOWLIST override forces a denied domain through (never silent) (#5257)", async () => {
    const rawLightningAddress = "dave@bitnob.io"
    const resolvedBolt11 = "lnbc1000n1mock5257allowoverrideinvoice"
    let sendCalls = 0
    const restore = installLnurlFetch(resolvedBolt11)
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 1000,
            preparedLightningFeeSats: 12,
            sendResultFees: null,
            onSend: () => { sendCalls += 1 },
          }),
      })
      await withDenyEnv("PYLON_SPARK_DENY_DOMAINS", "bitnob.io", async () => {
        await withDenyEnv("PYLON_SPARK_ALLOW_DOMAINS", "bitnob.io", async () => {
          const result = await transfer({
            amountSats: 1000,
            destination: rawLightningAddress,
            idempotencyKey: "test-5257-allow-override",
          })
          // The allowlist override wins over the deny list: the send proceeds.
          expect(result.ok).toBe(true)
          if (!result.ok) throw new Error("expected allowlist override to force the send")
          expect(sendCalls).toBe(1)
          expect(result.destinationDomain).toBe("bitnob.io")
          assertPublicProjectionSafe(result)
        })
      })
    } finally {
      restore()
    }
  })

  test("OPERATOR fee-ceiling override (maxFeeSats) also forces a fee-bound-flagged domain through (#5257)", async () => {
    const rawLightningAddress = "erin@bitnob.io"
    const resolvedBolt11 = "lnbc1000n1mock5257feeoverrideinvoice"
    let sendCalls = 0
    const restore = installLnurlFetch(resolvedBolt11)
    try {
      const transfer = createSparkBackupSendTransfer({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        loadModule: async () =>
          fakeSparkModule({
            balanceSats: 1000,
            preparedLightningFeeSats: 200,
            sendResultFees: 0n,
            onSend: () => { sendCalls += 1 },
          }),
      })
      await withDenyEnv("PYLON_SPARK_DOMAIN_FEE_MAX_PCT", "10", async () => {
        const result = await transfer({
          amountSats: 1000,
          destination: rawLightningAddress,
          idempotencyKey: "test-5257-fee-override",
          // Explicit operator acceptance also overrides the per-domain policy.
          maxFeeSats: 5000,
        })
        expect(result.ok).toBe(true)
        if (!result.ok) throw new Error("expected fee override to force the flagged send")
        expect(sendCalls).toBe(1)
        expect(result.feeSats).toBe(200)
        assertPublicProjectionSafe(result)
      })
    } finally {
      restore()
    }
  })

  test("native spark_native + a bolt11 send are UNAFFECTED by the domain policy (#5257)", async () => {
    // Even with a deny list AND a strict per-domain fee bound configured, a native
    // Spark send and a bare BOLT11 send (no domain) are never subject to the policy.
    await withDenyEnv("PYLON_SPARK_DENY_DOMAINS", "bitnob.io,anything.example", async () => {
      await withDenyEnv("PYLON_SPARK_DOMAIN_FEE_MAX_PCT", "1", async () => {
        // Native Spark.
        let nativeSendCalls = 0
        const nativeTransfer = createSparkBackupSendTransfer({
          apiKey: "k",
          mnemonic: TEST_MNEMONIC,
          loadModule: async () =>
            fakeSparkModule({ balanceSats: 5000, sendResultFees: 0n, onSend: () => { nativeSendCalls += 1 } }),
        })
        const nativeResult = await nativeTransfer({
          amountSats: 1,
          destination: RAW_SPARK_NATIVE_ADDRESS,
          idempotencyKey: "test-5257-native-unaffected",
        })
        expect(nativeResult.ok).toBe(true)
        if (!nativeResult.ok) throw new Error("expected native send to pass")
        expect(nativeResult.method).toBe("spark_native")
        expect(nativeResult.destinationDomain).toBeNull()
        expect(nativeSendCalls).toBe(1)
        assertPublicProjectionSafe(nativeResult)

        // Bare BOLT11 (no LNURL resolve, no domain).
        const rawPaymentRequest = "lnbc1000n1rawbolt11unaffectedbydomainpolicy5257"
        let bolt11SendCalls = 0
        const bolt11Transfer = createSparkBackupSendTransfer({
          apiKey: "k",
          mnemonic: TEST_MNEMONIC,
          loadModule: async () =>
            fakeSparkModule({
              balanceSats: 1000,
              preparedLightningFeeSats: 12,
              sendResultFees: null,
              onSend: () => { bolt11SendCalls += 1 },
            }),
        })
        const bolt11Result = await bolt11Transfer({
          amountSats: 1000,
          destination: rawPaymentRequest,
          idempotencyKey: "test-5257-bolt11-unaffected",
        })
        expect(bolt11Result.ok).toBe(true)
        if (!bolt11Result.ok) throw new Error("expected bolt11 send to pass")
        expect(bolt11Result.method).toBe("payment_request")
        expect(bolt11Result.destinationDomain).toBeNull()
        expect(bolt11SendCalls).toBe(1)
        assertPublicProjectionSafe(bolt11Result)
      })
    })
  })

  test("domainFromLightningAddress extracts the bare public-safe domain, null for non-LA (#5257)", () => {
    expect(domainFromLightningAddress("alice@bitnob.io")).toBe("bitnob.io")
    expect(domainFromLightningAddress("  ALICE@Bitnob.IO  ")).toBe("bitnob.io")
    expect(domainFromLightningAddress("lnbc1000n1notalightningaddress")).toBeNull()
    expect(domainFromLightningAddress(RAW_SPARK_NATIVE_ADDRESS)).toBeNull()
    expect(domainFromLightningAddress("")).toBeNull()
  })

  test("evaluateDomainFeePolicy: deny, fee-bound, allow-override, and non-LA passthrough (#5257)", () => {
    // Deny list.
    expect(
      evaluateDomainFeePolicy({
        domain: "bitnob.io",
        amountSats: 1000,
        preparedFeeSats: 10,
        env: { PYLON_SPARK_DENY_DOMAINS: "bitnob.io" } as NodeJS.ProcessEnv,
      }),
    ).toEqual({ refuse: true, domain: "bitnob.io", reason: "deny_list" })
    // Fee bound (200 > max(50, 10% of 1000)=100).
    expect(
      evaluateDomainFeePolicy({
        domain: "bitnob.io",
        amountSats: 1000,
        preparedFeeSats: 200,
        env: { PYLON_SPARK_DOMAIN_FEE_MAX_PCT: "10" } as NodeJS.ProcessEnv,
      }),
    ).toEqual({ refuse: true, domain: "bitnob.io", reason: "fee_bound" })
    // Allowlist override beats the deny list.
    expect(
      evaluateDomainFeePolicy({
        domain: "bitnob.io",
        amountSats: 1000,
        preparedFeeSats: 10,
        env: {
          PYLON_SPARK_DENY_DOMAINS: "bitnob.io",
          PYLON_SPARK_ALLOW_DOMAINS: "bitnob.io",
        } as NodeJS.ProcessEnv,
      }),
    ).toEqual({ refuse: false })
    // Explicit fee override beats the per-domain bound.
    expect(
      evaluateDomainFeePolicy({
        domain: "bitnob.io",
        amountSats: 1000,
        preparedFeeSats: 200,
        hasExplicitFeeOverride: true,
        env: { PYLON_SPARK_DOMAIN_FEE_MAX_PCT: "10" } as NodeJS.ProcessEnv,
      }),
    ).toEqual({ refuse: false })
    // Non-LA (null domain) is never subject to the policy.
    expect(
      evaluateDomainFeePolicy({
        domain: null,
        amountSats: 1000,
        preparedFeeSats: 999_999,
        env: { PYLON_SPARK_DENY_DOMAINS: "bitnob.io" } as NodeJS.ProcessEnv,
      }),
    ).toEqual({ refuse: false })
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

describe("resolveSparkBackupHelper (default-ON with OFF override, #5304)", () => {
  test("enabled BY DEFAULT: resolves a helper with a seed + credential and NO env flag", () => {
    // #5304: the Spark backup wallet is provisioned + enabled by default so a
    // fresh node is payable out of the box. With a seed present (and the
    // embedded default Breez key as final fallback) the helper resolves with no
    // PYLON_SPARK_BACKUP_ENABLED flag at all.
    expect(
      resolveSparkBackupHelper({ env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv, mnemonic: TEST_MNEMONIC }),
    ).not.toBeNull()
  })

  test("default-ON with NO env at all (embedded key + seed)", () => {
    expect(
      resolveSparkBackupHelper({ env: {} as NodeJS.ProcessEnv, mnemonic: TEST_MNEMONIC }),
    ).not.toBeNull()
  })

  test("OFF override (PYLON_SPARK_BACKUP_DISABLED=1) returns null even with seed + credential", () => {
    expect(
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
        mnemonic: TEST_MNEMONIC,
      }),
    ).toBeNull()
  })

  test("OFF override (PYLON_SPARK_BACKUP_ENABLED=0) returns null even with seed + credential", () => {
    expect(
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_ENABLED: "0" } as NodeJS.ProcessEnv,
        mnemonic: TEST_MNEMONIC,
      }),
    ).toBeNull()
  })

  test("explicit enabled:true wires the helper regardless of env override", () => {
    // The receive/status/payout CLI paths pass enabled:true and must always
    // wire the in-process helper (the #5194 read-path fix), even if someone set
    // an OFF override in the shell.
    expect(
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
        mnemonic: TEST_MNEMONIC,
        enabled: true,
      }),
    ).not.toBeNull()
  })

  test("uses the embedded default Breez key when enabled with no env credential", () => {
    // The embedded DEFAULT_OPENAGENTS_SPARK_API_KEY (committed historically,
    // owner-authorized) is the final fallback, so an enabled node with a seed
    // resolves a helper even with no env key.
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


// A fake Breez SDK Spark module whose SDK *connect* (build) NEVER resolves, to
// simulate the rc.30 contention/stall where opening the SDK blocks on a held
// SQLite/SDK connection on `storage.sql`. The adapter's `withTimeout(build,
// timeoutMs)` MUST bound it so the read returns a classified `timeout` instead
// of hanging forever (#5312). (We hang at connect rather than at a read op so
// the bound is the configurable `timeoutMs`, keeping the test fast; the #5197
// 45s read-sync floor only applies AFTER the SDK is open.)
function hangingConnectSparkModule() {
  const never = <T>(): Promise<T> => new Promise<T>(() => {})
  return {
    defaultConfig: (network: string) => ({ network, apiKey: undefined as string | undefined }),
    // Opening the SDK never completes — exactly the contended/locked-storage case.
    connect: (_req: { config: { apiKey?: string }; seed: { mnemonic: string } }) =>
      never<Record<string, unknown>>(),
  }
}

describe("#5312 backup-status read is hard-bounded under SDK/storage contention", () => {
  test("a hanging SDK open times out (bounded) and classifies a public-safe `timeout` reason", async () => {
    const started = Date.now()
    const helper = createSparkBackupHelper({
      apiKey: "k",
      mnemonic: TEST_MNEMONIC,
      // Small bound so the regression runs fast; production bounds the SDK build
      // at DEFAULT_SPARK_TIMEOUT_MS (15s) and the one-shot CLI wraps the whole
      // read in its own 12s hard wall-clock bound + forced exit.
      timeoutMs: 80,
      loadModule: async () => hangingConnectSparkModule(),
    })
    const detected = await detectSparkBackupBalance(helper)
    const elapsed = Date.now() - started
    // BOUNDED: the read returned far inside any external alarm rather than hanging.
    expect(elapsed).toBeLessThan(5_000)
    expect(detected.helperReady).toBe(false)
    // #5194 reason surfacing reused: the stall is classified as a timeout.
    expect(detected.helperUnavailableReason).toBe("timeout")
    expect(detected.detectedBalanceSats).toBeNull()
  })

  test("a hanging SDK open in classify stays bounded and reports helper-unavailable, not a hang", async () => {
    const started = Date.now()
    const projection = await classifySparkBackupReceive({
      enabled: true,
      embeddedCredentialAvailable: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      helper: createSparkBackupHelper({
        apiKey: "k",
        mnemonic: TEST_MNEMONIC,
        timeoutMs: 80,
        loadModule: async () => hangingConnectSparkModule(),
      }),
    })
    const elapsed = Date.now() - started
    expect(elapsed).toBeLessThan(5_000)
    expect(projection.state).toBe("helper-unavailable")
    expect(projection.helperReady).toBe(false)
    // Public-safe: no raw spark material leaked in the bounded projection.
    assertPublicProjectionSafe(projection)
  })

  test("with a cached address, the bounded fallback yields a public-safe cached-address-ready projection (node stays payable)", async () => {
    // This mirrors the bounded one-shot timeout fallback
    // (`buildBoundedBackupStatusTimeoutBody`): classify with NO helper wired
    // (cannot hang) but WITH a cached target -> cached-address-ready.
    const projection = await classifySparkBackupReceive({
      enabled: true,
      embeddedCredentialAvailable: true,
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      cachedAddress: RAW_SPARK_NATIVE_ADDRESS,
    })
    expect(projection.state).toBe("cached-address-ready")
    // The redacted ref is present; the raw spark1… target is NOT in the projection.
    expect(projection.receiveTargetRef).toBeTruthy()
    assertPublicProjectionSafe(projection)
  })
})
