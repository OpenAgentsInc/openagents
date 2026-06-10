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
    )

    expect(status.balanceSats).toBe(123)
    expect(status.receiveReady).toBe(true)
    expect(status.sendReady).toBe(false)
    expect(status.readiness).toBe("send-ready-blocked")
    expect(status.blockerRefs).toContain("blocker.wallet.send_readiness_unproven")
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
