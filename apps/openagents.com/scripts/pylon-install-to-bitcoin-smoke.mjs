#!/usr/bin/env node

const allowedModes = new Set([
  'ci_no_spend',
  'live_small_sats',
  'sandbox_fake_payment',
])

const usage = `Usage:
  node scripts/pylon-install-to-bitcoin-smoke.mjs --mode ci_no_spend
  node scripts/pylon-install-to-bitcoin-smoke.mjs --mode sandbox_fake_payment --payment-ref <ref> --settlement-ref <ref>
  node scripts/pylon-install-to-bitcoin-smoke.mjs --mode live_small_sats --amount-sats 1 --spend-cap-sats 10 --operator-approved --wallet-home-mode original_funded_wallet_home --payment-ref <ref> --settlement-ref <ref>

This script emits the public-safe checklist for the Pylon install-to-bitcoin
launch smoke. It does not execute wallet commands, spend bitcoin, or call the
OpenAgents API. Live payment execution still belongs to the operator-run MDK
agent-wallet command sequence retained in the smoke bundle.`

const parseArgs = argv => {
  const flags = {
    amountSats: 1,
    mode: 'ci_no_spend',
    operatorApproved: false,
    paymentRefs: [],
    settlementRefs: [],
    spendCapSats: 10,
    walletHomeMode: 'unknown',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]

    if (arg === '--help' || arg === '-h') {
      return { help: true }
    }

    if (arg === '--operator-approved') {
      flags.operatorApproved = true
      continue
    }

    const next = argv[index + 1]

    if (next === undefined) {
      throw new Error(`Missing value for ${arg}`)
    }

    if (arg === '--mode') {
      flags.mode = next
      index += 1
    } else if (arg === '--amount-sats') {
      flags.amountSats = Number.parseInt(next, 10)
      index += 1
    } else if (arg === '--spend-cap-sats') {
      flags.spendCapSats = Number.parseInt(next, 10)
      index += 1
    } else if (arg === '--wallet-home-mode') {
      flags.walletHomeMode = next
      index += 1
    } else if (arg === '--payment-ref') {
      flags.paymentRefs.push(next)
      index += 1
    } else if (arg === '--settlement-ref') {
      flags.settlementRefs.push(next)
      index += 1
    } else {
      throw new Error(`Unknown argument: ${arg}`)
    }
  }

  return flags
}

const blockerRefsFor = flags => [
  ...(flags.amountSats > flags.spendCapSats
    ? ['blocker.pylon_install_to_bitcoin.spend_cap_exceeded']
    : []),
  ...(flags.mode === 'live_small_sats' && !flags.operatorApproved
    ? ['blocker.pylon_install_to_bitcoin.operator_approval_missing']
    : []),
  ...(flags.mode === 'live_small_sats' &&
  flags.walletHomeMode !== 'original_funded_wallet_home'
    ? ['blocker.pylon_install_to_bitcoin.mdk_send_readiness_missing']
    : []),
  ...(flags.mode === 'live_small_sats' && flags.paymentRefs.length === 0
    ? ['blocker.pylon_install_to_bitcoin.payment_receipt_missing']
    : []),
  ...(flags.mode === 'live_small_sats' && flags.settlementRefs.length === 0
    ? ['blocker.pylon_install_to_bitcoin.settlement_receipt_missing']
    : []),
]

const planFor = flags => {
  if (!allowedModes.has(flags.mode)) {
    throw new Error(`Unsupported mode: ${flags.mode}`)
  }

  const blockers = blockerRefsFor(flags)
  const liveReady = flags.mode === 'live_small_sats' && blockers.length === 0

  return {
    authority: {
      liveWalletSpendAllowed: liveReady,
      operatorApprovedLiveSpend: flags.operatorApproved,
      settlementClaimAllowed:
        liveReady || flags.mode === 'sandbox_fake_payment',
    },
    blockers,
    mdk: {
      commands: [
        'npx @moneydevkit/agent-wallet@latest status',
        'npx @moneydevkit/agent-wallet@latest init',
        'npx @moneydevkit/agent-wallet@latest balance',
        ...(liveReady
          ? [
              'npx @moneydevkit/agent-wallet@latest send <redacted_bolt11_invoice>',
            ]
          : []),
      ],
      rawPaymentMaterialRetained: false,
      walletHomeMode: flags.walletHomeMode,
    },
    mode: flags.mode,
    refsToRetain: {
      assignment: ['assignment.public.install_to_bitcoin.<run_ref>'],
      closeout: ['closeout.public.install_to_bitcoin.<run_ref>'],
      heartbeat: ['heartbeat.public.install_to_bitcoin.<run_ref>'],
      install: ['install.public.pylon.latest.<run_ref>'],
      payment: flags.paymentRefs,
      publicProjection: ['projection.public.install_to_bitcoin.<run_ref>'],
      payoutReadiness: ['payout_readiness.public.install_to_bitcoin.<run_ref>'],
      register: ['registration.public.install_to_bitcoin.<run_ref>'],
      settlement: flags.settlementRefs,
      wallet: ['wallet_readiness.public.install_to_bitcoin.<run_ref>'],
    },
    spendCapSats: flags.spendCapSats,
    status: liveReady
      ? 'live_settled_bitcoin_ready'
      : blockers.length > 0
        ? 'blocked'
        : flags.mode === 'sandbox_fake_payment'
          ? 'sandbox_fake_payment_ready'
          : 'ci_no_spend_ready',
  }
}

try {
  const flags = parseArgs(process.argv.slice(2))

  if (flags.help) {
    console.log(usage)
    process.exit(0)
  }

  console.log(JSON.stringify(planFor(flags), null, 2))
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error))
  console.error(usage)
  process.exit(1)
}
