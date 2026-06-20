// Operational grounding for the Artanis Pylon support responder
// (promise artanis.pylon_support_responder.v1; issue #5540).
//
// Root cause of the cinder-atlas grounding defect (forum topic 7ba5d586):
// the reply composer grounded ONLY on product-promise-registry copy, so it
// restated promise claims instead of answering concrete operational
// questions (how to make `sparkPayoutTargetReady` true, how to keep
// executor-trace capability refs through heartbeat). The fix widens
// grounding to the actual Pylon/Tassadar runbooks.
//
// These facts are a curated, public-safe distillation of:
//   - apps/pylon/README.md (wallet readiness, presence/heartbeat, work lanes)
//   - apps/pylon/docs/mdk-wallet-readiness-ledger.md (payout-target admission,
//     send-readiness preflight, readiness states)
//   - apps/openagents.com/docs/2026-06-10-tassadar-executor-trace-homework-internal.md
//     (executor-trace homework, capability refs, settlement closeout)
//
// They carry only public-safe refs, command shapes, blocker names, and
// readiness states - never wallet secrets, raw invoices/offers, mnemonics,
// preimages, payment material, or private payloads. Keep this list aligned
// with those runbooks when the operational surface changes.

export type ArtanisOperationalFact = Readonly<{
  topic: string
  fact: string
  sourceRef: string
}>

export const artanisOperationalGrounding = (): ReadonlyArray<ArtanisOperationalFact> => [
  {
    fact: 'To make the agent wallet payout-target ready, admit a public-safe payout target and have it accepted server-side. Locally: `pylon wallet admit-payout-target --kind bolt12_offer --ref payout.bolt12.<hash>`. To request server admission: `pylon wallet request-payout-target-admission --base-url https://openagents.com --kind bolt12_offer --ref payout.bolt12.<hash>`. Supported kinds: bolt12_offer, bolt11_invoice, bip353_name, lnurl_pay. The command emits a public-safe receipt ref, never a raw invoice/offer.',
    sourceRef: 'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
    topic: 'payout_target_readiness',
  },
  {
    fact: 'Pylon classifies wallet readiness into explicit states: daemon-offline, balance-unknown, receive-ready, send-ready, send-ready-blocked, payout-target-admitted, payable-pending-settlement, settlement-recorded. balance unknown/offline is NOT zero, and receive readiness is NOT send readiness. Mnemonic-only restores, zero outbound capacity, or missing send-readiness evidence produce blocker.wallet.send_readiness_unproven.',
    sourceRef: 'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
    topic: 'wallet_readiness_states',
  },
  {
    fact: 'Run `pylon wallet status` to read the send-readiness preflight (sendReadinessPreflight). Send readiness is true only when the wallet daemon reports send readiness, balance is known, outbound capacity is known and positive, the wallet is not a mnemonic-only restore, and MDK_WALLET_PORT is explicitly set. An unset port reports blocker.wallet.mdk_port_unset (the default 3456 can cross-talk with the wrong wallet); set MDK_WALLET_PORT to any free port to clear it.',
    sourceRef: 'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
    topic: 'send_readiness_preflight',
  },
  {
    fact: 'Pylon keeps presence and capability refs current by heartbeat: `pylon presence register --base-url https://openagents.com` then `pylon presence heartbeat --base-url https://openagents.com`. Capability refs (including executor-trace capability) are carried by the heartbeat projection; if a Pylon stops heartbeating, its advertised capabilities go stale. Keep the heartbeat loop running so executor-trace capability refs stay live for dispatch.',
    sourceRef: 'apps/pylon/README.md',
    topic: 'presence_heartbeat_capabilities',
  },
  {
    fact: 'Executor-trace homework produces a public-safe executor-trace artifact carrying only public-safe refs. Paid settlement of an executor-trace run stays blocked until an operator-funded executor-trace closeout exists; the closeout/settlement requires a settlement/ledger ref. The local smoke is `bun run smoke:tassadar:executor-trace`.',
    sourceRef: 'apps/openagents.com/docs/2026-06-10-tassadar-executor-trace-homework-internal.md',
    topic: 'executor_trace_settlement',
  },
  {
    fact: 'No-spend assignment lanes do not require send readiness: `pylon assignment run-no-spend --base-url https://openagents.com` polls for a no-spend lease, applies local admission gates, submits artifact/proof refs, and closes with settlementState not_applicable and payoutClaimAllowed false. Paid leases are blocked unless wallet send readiness is explicitly proven, so a contributor can start earning recognition/work before payout-target readiness is green.',
    sourceRef: 'apps/pylon/README.md',
    topic: 'no_spend_assignment_lane',
  },
]
