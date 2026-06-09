#!/usr/bin/env node

import { pathToFileURL } from 'node:url'
import { runForumCli } from './forum.mjs'
import { runForumTipPayoutSmoke } from './forum-tip-payout-smoke.mjs'

const defaultSpendCapAmount = '10'
const defaultSpendCapAsset = 'sats'

export const parseArgs = argv => {
  const options = {
    approveLiveSpend: false,
    baseUrl: process.env.OPENAGENTS_BASE_URL || 'https://openagents.com',
    post: process.env.OPENAGENTS_FORUM_TIP_SMOKE_POST || '',
    spendCapAmount:
      process.env.OPENAGENTS_MDK_FORUM_READINESS_AMOUNT ||
      defaultSpendCapAmount,
    spendCapAsset:
      process.env.OPENAGENTS_MDK_FORUM_READINESS_ASSET ||
      defaultSpendCapAsset,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]

    if (value === '--approve-live-spend') {
      options.approveLiveSpend = true
    } else if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--post') {
      options.post = argv[++index] || options.post
    } else if (value === '--spend-cap-amount') {
      options.spendCapAmount = argv[++index] || options.spendCapAmount
    } else if (value === '--spend-cap-asset') {
      options.spendCapAsset = argv[++index] || options.spendCapAsset
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

export const usage = () => `Usage:
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/mdk-forum-readiness-smoke.mjs --post POST_ID
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/mdk-forum-readiness-smoke.mjs --post POST_ID --approve-live-spend

Options:
  --post <id>              Ready-recipient Forum post to tip.
  --spend-cap-amount <n>   Defaults to 10.
  --spend-cap-asset <a>    Defaults to sats.
  --approve-live-spend     Attempt live wallet payment after preview.
  --base-url <url>         Defaults to https://openagents.com.
`

const parseJson = text => JSON.parse(text)

const forbiddenOutputPattern =
  /(oa_agent_|Bearer |oa-l402-v1|lnbc|lntb|lno1|payment_hash|paymentHash|preimage|mnemonic|secret|token|configPath|walletHome|\/\.mdk-wallet)/i

export const redact = text =>
  String(text)
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/-]+/g, 'oa_agent_<redacted>')
    .replace(/oa-l402-v1[A-Za-z0-9._~+/-]*/g, 'oa-l402-v1<redacted>')
    .replace(/lnbc[A-Za-z0-9]+/gi, 'lnbc<redacted>')
    .replace(/lntb[A-Za-z0-9]+/gi, 'lntb<redacted>')
    .replace(/lno1[A-Za-z0-9]+/gi, 'lno1<redacted>')

const summarizeWalletPreflight = preflight => ({
  blockerRef: preflight?.blocker?.reasonRef ?? null,
  checks: Array.isArray(preflight?.checks)
    ? preflight.checks.map(check => ({
        commandRef: check.commandRef ?? null,
        reasonRef: check.reasonRef ?? null,
        status: check.status ?? null,
      }))
    : [],
  livePaymentAttempted: preflight?.livePaymentAttempted === true,
  readinessRefs: Array.isArray(preflight?.readinessRefs)
    ? preflight.readinessRefs
    : [],
  ready: preflight?.ready === true,
  status: preflight?.status ?? null,
  walletRef: preflight?.walletRef ?? null,
})

const hostedPayoutGate = challenge => {
  if (challenge?.paymentRequired !== true) {
    return {
      blockerRefs: ['blocker.product_promises.forum_tip_challenge_not_payable'],
      directPayoutEnabled: false,
      state: 'blocked',
    }
  }

  if (challenge.providerPayoutAuthority !== true) {
    return {
      blockerRefs: [
        'blocker.product_promises.hosted_mdk_direct_payout_authority_disabled',
      ],
      directPayoutEnabled: false,
      state: 'evidence_only',
    }
  }

  return {
    blockerRefs: [],
    directPayoutEnabled: true,
    state: 'enabled',
  }
}

const restoreSendGate = wallet => {
  if (wallet.ready === true) {
    return {
      blockerRefs: [],
      sendReady: true,
      state: 'ready',
    }
  }

  return {
    blockerRefs: [
      wallet.blockerRef ||
        'blocker.product_promises.mdk_agent_wallet_send_readiness_missing',
    ],
    sendReady: false,
    state: 'blocked',
  }
}

export const runMdkForumReadinessSmoke = async (
  options,
  env = process.env,
  runner = runForumCli,
) => {
  if (!options.post) {
    throw new Error('--post is required.')
  }

  const baseOptions = {
    approveLiveSpend: options.approveLiveSpend,
    baseUrl: options.baseUrl,
    post: options.post,
    spendCapAmount: options.spendCapAmount,
    spendCapAsset: options.spendCapAsset,
  }
  const walletPreflight = parseJson(
    await runner(
      [
        'wallet-status',
        '--spend-cap-amount',
        options.spendCapAmount,
        '--spend-cap-asset',
        options.spendCapAsset,
      ],
      env,
    ),
  )
  const payout = await runForumTipPayoutSmoke(baseOptions, env, runner)
  const wallet = summarizeWalletPreflight(walletPreflight)
  const output = {
    baseUrl: options.baseUrl,
    gates: {
      hostedPayout: hostedPayoutGate(payout.challenge),
      restoreSend: restoreSendGate(wallet),
    },
    payout,
    post: options.post,
    publicSafe: true,
    spendCap: payout.spendCap,
    wallet,
  }
  const serialized = JSON.stringify(output)

  if (forbiddenOutputPattern.test(serialized)) {
    throw new Error('Smoke output contains private wallet or payment material.')
  }

  return output
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))

  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runMdkForumReadinessSmoke({
    ...options,
    baseUrl: options.baseUrl.replace(/\/+$/, ''),
  })

  console.log(JSON.stringify(output, null, 2))
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  main().catch(error => {
    console.error(redact(error instanceof Error ? error.message : String(error)))
    process.exitCode = 1
  })
}
