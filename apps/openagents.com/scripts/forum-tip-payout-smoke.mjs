#!/usr/bin/env node

import { pathToFileURL } from 'node:url'
import { parseForumArgs, runForumCli } from './forum.mjs'

const defaultSpendCapAmount = '10'
const defaultSpendCapAsset = 'sats'

export const parseArgs = argv => {
  const options = {
    approveLiveSpend: false,
    baseUrl: process.env.OPENAGENTS_BASE_URL || 'https://openagents.com',
    post: process.env.OPENAGENTS_FORUM_TIP_SMOKE_POST || '',
    spendCapAmount:
      process.env.OPENAGENTS_FORUM_TIP_SMOKE_AMOUNT || defaultSpendCapAmount,
    spendCapAsset:
      process.env.OPENAGENTS_FORUM_TIP_SMOKE_ASSET || defaultSpendCapAsset,
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
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum-tip-payout-smoke.mjs --post POST_ID
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum-tip-payout-smoke.mjs --post POST_ID --approve-live-spend

Options:
  --post <id>              Ready-recipient Forum post to tip.
  --spend-cap-amount <n>   Defaults to 10.
  --spend-cap-asset <a>    Defaults to sats.
  --approve-live-spend     Attempt live wallet payment after preview.
  --base-url <url>         Defaults to https://openagents.com.
`

export const redact = text =>
  String(text)
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/-]+/g, 'oa_agent_<redacted>')
    .replace(/oa-l402-v1[A-Za-z0-9._~+/-]*/g, 'oa-l402-v1<redacted>')
    .replace(/lnbc[A-Za-z0-9]+/gi, 'lnbc<redacted>')
    .replace(/lntb[A-Za-z0-9]+/gi, 'lntb<redacted>')
    .replace(/lno1[A-Za-z0-9]+/gi, 'lno1<redacted>')

const parseJson = text => JSON.parse(text)

const publicChallengeSummary = preview => {
  const challenge = preview?.challenge
  const l402 = challenge?.l402

  if (challenge === null || typeof challenge !== 'object') {
    return null
  }

  return {
    challengeId:
      typeof challenge.challengeId === 'string'
        ? challenge.challengeId
        : null,
    environment:
      l402 !== null && typeof l402?.environment === 'string'
        ? l402.environment
        : null,
    paymentRequired: preview.paymentRequired === true,
    provider:
      l402 !== null && typeof l402?.provider === 'string'
        ? l402.provider
        : null,
    providerPayoutAuthority:
      l402 !== null && typeof l402?.providerPayoutAuthority === 'boolean'
        ? l402.providerPayoutAuthority
        : null,
    recipientReadinessRef:
      typeof challenge.recipientReadinessRef === 'string'
        ? challenge.recipientReadinessRef
        : null,
    sandbox:
      l402 !== null && typeof l402?.sandbox === 'boolean'
        ? l402.sandbox
        : null,
    settlementAuthority:
      l402 !== null && typeof l402?.settlementAuthority === 'string'
        ? l402.settlementAuthority
        : null,
  }
}

export const runForumTipPayoutSmoke = async (
  options,
  env = process.env,
  runner = runForumCli,
) => {
  if (!options.post) {
    throw new Error('--post is required.')
  }

  const common = [
    '--base-url',
    options.baseUrl,
    '--post',
    options.post,
    '--spend-cap-amount',
    options.spendCapAmount,
    '--spend-cap-asset',
    options.spendCapAsset,
  ]
  const preview = parseJson(
    await runner(['reward-post', ...common], env),
  )
  const payment =
    options.approveLiveSpend === true
      ? parseJson(
          await runner(
            ['pay-reward-post', ...common, '--approve-live-spend'],
            env,
          ),
        )
      : null
  const output = {
    challenge: publicChallengeSummary(preview),
    livePaymentApproved: options.approveLiveSpend === true,
    payment:
      payment === null
        ? null
        : {
            livePaymentAttempted: payment.livePaymentAttempted === true,
            reasonRef: payment.reasonRef ?? null,
            receiptRef: payment.receipt?.receiptRef ?? null,
            status: payment.status ?? null,
          },
    post: options.post,
    preview: {
      paymentRequired: preview.paymentRequired === true,
      writeDenialRef: preview.writeDenial?.denialRef ?? null,
    },
    publicSafe: true,
    spendCap: {
      amount: Number(options.spendCapAmount),
      asset: options.spendCapAsset === 'bitcoin' ? 'sats' : options.spendCapAsset,
    },
  }
  const serialized = JSON.stringify(output)

  if (
    /(oa_agent_|Bearer |oa-l402-v1|lnbc|lntb|lno1|payment_hash|preimage|mnemonic|secret|token)/i.test(
      serialized,
    )
  ) {
    throw new Error('Smoke output contains private payment or token material.')
  }

  return output
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))

  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runForumTipPayoutSmoke({
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
