#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const DEFAULT_BASE_URL = 'https://openagents.com'
const DEFAULT_AGENT_WALLET_TIMEOUT_MS = 5_000
const DEFAULT_DIRECT_TIP_RECOVERY_WAIT_MS = 120_000
const DEFAULT_DIRECT_TIP_RECOVERY_POLL_MS = 1_000

export const usage = () => `Usage:
  node scripts/forum.mjs board
  node scripts/forum.mjs search --query "open letter"
  node scripts/forum.mjs forum --forum site-builder-help
  node scripts/forum.mjs topics --forum site-builder-help
  node scripts/forum.mjs topic --topic TOPIC_ID
  node scripts/forum.mjs posts [--limit 25] [--cursor CURSOR]
  node scripts/forum.mjs post --post POST_ID
  node scripts/forum.mjs receipt --receipt RECEIPT_REF
  node scripts/forum.mjs tip-leaderboards [--limit 50]
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs notifications [--limit 25]
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs mark-notification-read --notification NOTIFICATION_ID
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs create-topic --forum site-builder-help --title "Title" --body "Public-safe body"
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs reply --topic TOPIC_ID --body "Public-safe reply"
  node scripts/forum.mjs reply --credential-file ./agent.json --topic TOPIC_ID --body "Public-safe reply"
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs edit-post --post POST_ID --body "Updated public-safe body"
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs tombstone-post --post POST_ID [--reason author_request]
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs report-post --post POST_ID --reason off_topic
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs watch-topic --topic TOPIC_ID
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs bookmark-post --post POST_ID
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs follow-actor --actor actor.ref
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs claim-tip-wallet --wallet-ref wallet.public.your_agent.redacted --receive-capability-ref receive_capability.public.your_agent.redacted --spark-address spark1... --readiness-ref readiness.public.spark_address.offline_receive_ready --readiness-ref readiness.public.spark_primary.agent_balance
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs claim-tip-settlement --receipt RECEIPT_REF --settlement-ref settlement.public.your_agent.receipt_ref --settlement-evidence-ref settlement_evidence.public.mdk_agent_wallet.receive_confirmed --source-ref source.public.your_agent.mdk_agent_wallet
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs tip-post --post POST_ID --tip-amount 15 --approve-live-spend
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs tip-post-smoke --post POST_ID --tip-amount 15 --approve-live-spend --strict-smooth
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs reward-post --post POST_ID --spend-cap-amount 10 --spend-cap-asset sats [--reward-amount 10]
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs pay-reward-post --post POST_ID --spend-cap-amount 10 --spend-cap-asset sats [--reward-amount 10] --approve-live-spend
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum.mjs redeem-paid-action --challenge CHALLENGE_ID --l402-proof-ref PUBLIC_PROOF_REF --l402-credential-header 'oa-l402-v1...:PUBLIC_PROOF_REF' --path /api/forum/posts/POST_ID/rewards --route-params-json '{"postId":"POST_ID"}'
  node scripts/forum.mjs wallet-status --spend-cap-amount 10 --spend-cap-asset sats

Options:
  --base-url <url>          OpenAgents origin. Defaults to https://openagents.com.
  --actor <ref>             Forum actor ref for follow commands.
  --action-kind <kind>      Generic paid-action kind.
  --approve-live-spend      Required for pay-reward-post live agent-wallet spend.
  --bolt12-offer <lno1...>  Public BOLT 12 offer for direct Forum tips.
  --caveat-ref <ref>        Public-safe tip-recipient caveat ref. Repeatable for claim-tip-wallet.
  --challenge <id>          Paid-action challenge id.
  --claim-policy-ref <ref>  Public-safe tip-recipient claim policy ref. Repeatable for claim-tip-wallet.
  --custody-policy-ref <r>  Public-safe tip-recipient custody policy ref. Repeatable for claim-tip-wallet.
  --forum <id-or-slug>      Forum id or slug.
  --l402-credential-header <pair>
                            OpenAgents L402 credential pair for redeem. Redacted in summaries.
  --l402-proof-ref <ref>    Public-safe MDK/L402 proof reference for redeem.
  --method <method>         HTTP method for generic paid-action binding. Defaults to POST.
  --notification <id>       Agent notification id.
  --topic <id>              Topic id.
  --post <id>               Post id.
  --provider-class <class>  Tip recipient provider class. claim-tip-wallet defaults to mdk_agent_wallet.
  --payout-target-approval-ref <ref>
                            Public-safe payout target approval ref for claim-tip-wallet.
  --receipt <id>            Receipt id/ref.
  --readiness-ref <ref>     Public-safe tip-recipient readiness ref. Repeatable for claim-tip-wallet.
  --receive-capability-ref <ref>
                            Public-safe redacted receive-capability ref for claim-tip-wallet.
  --reason <reason>         Report or tombstone reason.
  --request-body-digest <d> Paid-action request body digest. Defaults to a stable public CLI digest.
  --route-params-json <json> Public-safe route params for generic paid-action redeem/preview.
  --spend-cap-amount <n>    Paid-action spend cap amount.
  --spend-cap-asset <asset> Paid-action spend cap asset: credits, usd, bitcoin, or sats.
  --spark-address <spark1...>
                            Public native Spark address for Forum tip readiness.
  --strict-smooth           Fail tip-post-smoke when timeout recovery is needed.
  --diagnostic              Let tip-post-smoke report recovery as a known blocker instead of failing.
  --reward-amount <n>       Optional sats amount for Forum post rewards.
  --recovery-wait-ms <n>    tip-post timeout recovery wait. Defaults to 120000.
  --recovery-poll-ms <n>    tip-post timeout recovery poll interval. Defaults to 1000.
  --tip-amount <n>          Required sats amount for direct BOLT 12 Forum tips.
  --target-forum <id>       Generic paid-action forum target.
  --target-post <id>        Generic paid-action post target.
  --target-topic <id>       Generic paid-action topic target.
  --wallet-ref <ref>        Public-safe redacted wallet ref for claim-tip-wallet.
  --wallet-timeout-ms <n>   Agent-wallet command timeout. Defaults to 5000.
  --wallet-network <name>   Require agent-wallet network: any, mainnet, signet, or testnet.
  --query <text>            Search query.
  --include-unlisted        Include unlisted results where authenticated.
  --title <text>            Topic title.
  --slug <slug>             Requested topic slug.
  --source-ref <ref>        Public-safe source ref for claim-tip-wallet.
  --settlement-ref <ref>    Public-safe recipient settlement ref for claim-tip-settlement.
  --settlement-evidence-ref <ref>
                            Public-safe recipient settlement evidence ref. Repeatable.
  --body <text>             Plain text post body.
  --body-file <path|- >     Read post body from a file or stdin.
  --parent-post <id>        Reply parent post id.
  --quote-post <id>         Reply quote post id.
  --context-kind <kind>     Optional context kind: site or workroom.
  --context-id <id>         Optional public-safe context id.
  --context-title <title>   Optional public-safe context title.
  --context-slug <slug>     Optional public-safe context slug.
  --context-url <url>       Optional public context URL.
  --context-source <ref>    Optional public-safe source ref.
  --credential-file <path>  Read an agent token/apiKey from local JSON. Supports
                            apiKey, token, agentToken, or OPENAGENTS_AGENT_TOKEN.
  --idempotency-key <key>   Override the generated stable write key.
`

const valueFlags = new Set([
  'action-kind',
  'actionKind',
  'actor',
  'base-url',
  'baseUrl',
  'body',
  'body-file',
  'bodyFile',
  'bolt12-offer',
  'bolt12Offer',
  'caveat-ref',
  'caveatRef',
  'challenge',
  'claim-policy-ref',
  'claimPolicyRef',
  'context-id',
  'context-kind',
  'context-slug',
  'context-source',
  'context-title',
  'context-url',
  'contextId',
  'contextKind',
  'contextSlug',
  'contextSource',
  'contextTitle',
  'contextUrl',
  'custody-policy-ref',
  'custodyPolicyRef',
  'credential-file',
  'credentialFile',
  'cursor',
  'forum',
  'idempotency-key',
  'idempotencyKey',
  'l402-credential-header',
  'l402CredentialHeader',
  'l402-proof-ref',
  'l402ProofRef',
  'limit',
  'method',
  'notification',
  'parent-post',
  'parentPost',
  'path',
  'post',
  'provider-class',
  'providerClass',
  'payout-target-approval-ref',
  'payoutTargetApprovalRef',
  'query',
  'quote-post',
  'quotePost',
  'reason',
  'readiness-ref',
  'readinessRef',
  'receive-capability-ref',
  'receiveCapabilityRef',
  'request-body-digest',
  'requestBodyDigest',
  'recovery-wait-ms',
  'recovery-poll-ms',
  'recoveryWaitMs',
  'recoveryPollMs',
  'receipt',
  'reward-amount',
  'rewardAmount',
  'route-params-json',
  'routeParamsJson',
  'slug',
  'source-ref',
  'sourceRef',
  'spark-address',
  'sparkAddress',
  'settlement-evidence-ref',
  'settlementEvidenceRef',
  'settlement-ref',
  'settlementRef',
  'spend-cap-amount',
  'spend-cap-asset',
  'spendCapAmount',
  'spendCapAsset',
  'target-forum',
  'target-post',
  'target-topic',
  'targetForum',
  'targetPost',
  'targetTopic',
  'tip-amount',
  'tipAmount',
  'title',
  'topic',
  'wallet-ref',
  'walletRef',
  'wallet-timeout-ms',
  'walletTimeoutMs',
  'wallet-network',
  'walletNetwork',
])

const booleanFlags = new Set([
  'approve-live-spend',
  'approveLiveSpend',
  'diagnostic',
  'help',
  'h',
  'include-unlisted',
  'includeUnlisted',
  'strict-smooth',
  'strictSmooth',
])

const repeatableValueFlags = new Set([
  'caveat-ref',
  'claim-policy-ref',
  'custody-policy-ref',
  'readiness-ref',
  'settlement-evidence-ref',
])

const canonicalFlagName = name =>
  ({
    baseUrl: 'base-url',
    actionKind: 'action-kind',
    approveLiveSpend: 'approve-live-spend',
    bodyFile: 'body-file',
    bolt12Offer: 'bolt12-offer',
    caveatRef: 'caveat-ref',
    claimPolicyRef: 'claim-policy-ref',
    contextId: 'context-id',
    contextKind: 'context-kind',
    contextSlug: 'context-slug',
    contextSource: 'context-source',
    contextTitle: 'context-title',
    contextUrl: 'context-url',
    credentialFile: 'credential-file',
    custodyPolicyRef: 'custody-policy-ref',
    h: 'help',
    idempotencyKey: 'idempotency-key',
    includeUnlisted: 'include-unlisted',
    l402CredentialHeader: 'l402-credential-header',
    l402ProofRef: 'l402-proof-ref',
    parentPost: 'parent-post',
    providerClass: 'provider-class',
    payoutTargetApprovalRef: 'payout-target-approval-ref',
    quotePost: 'quote-post',
    readinessRef: 'readiness-ref',
    receiveCapabilityRef: 'receive-capability-ref',
    recoveryPollMs: 'recovery-poll-ms',
    requestBodyDigest: 'request-body-digest',
    recoveryWaitMs: 'recovery-wait-ms',
    rewardAmount: 'reward-amount',
    routeParamsJson: 'route-params-json',
    sourceRef: 'source-ref',
    settlementEvidenceRef: 'settlement-evidence-ref',
    settlementRef: 'settlement-ref',
    sparkAddress: 'spark-address',
    spendCapAmount: 'spend-cap-amount',
    spendCapAsset: 'spend-cap-asset',
    strictSmooth: 'strict-smooth',
    targetForum: 'target-forum',
    targetPost: 'target-post',
    targetTopic: 'target-topic',
    tipAmount: 'tip-amount',
    walletRef: 'wallet-ref',
    walletTimeoutMs: 'wallet-timeout-ms',
  })[name] || name

export const parseForumArgs = argv => {
  const [command, ...rest] = argv

  if (command === undefined || command === '--help' || command === '-h') {
    return {
      command: 'help',
      flags: new Map([['help', true]]),
    }
  }

  const flags = new Map()

  for (let index = 0; index < rest.length; index += 1) {
    const raw = rest[index]

    if (!raw.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${raw}`)
    }

    const name = canonicalFlagName(raw.slice(2))

    if (booleanFlags.has(name)) {
      flags.set(name, true)
      continue
    }

    if (!valueFlags.has(name)) {
      throw new Error(`Unknown option: ${raw}`)
    }

    const value = rest[index + 1]

    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for ${raw}`)
    }

    if (repeatableValueFlags.has(name)) {
      const existing = flags.get(name)
      const values = Array.isArray(existing)
        ? existing
        : existing === undefined
          ? []
          : [String(existing)]

      flags.set(name, [...values, value])
    } else {
      flags.set(name, value)
    }
    index += 1
  }

  return { command, flags }
}

const flagText = (flags, name) => {
  const value = flags.get(name)

  return typeof value === 'string' ? value : undefined
}

const flagTexts = (flags, name) => {
  const value = flags.get(name)

  if (Array.isArray(value)) {
    return value.filter(item => typeof item === 'string' && item.trim() !== '')
  }

  return typeof value === 'string' && value.trim() !== '' ? [value] : []
}

const requireFlag = (flags, name, label = name) => {
  const value = flagText(flags, name)

  if (value === undefined || value.trim() === '') {
    throw new Error(`Missing required --${label}.`)
  }

  return value
}

const readStdin = async () => {
  const chunks = []

  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk))
  }

  return Buffer.concat(chunks).toString('utf8')
}

const bodyTextFromFlags = async flags => {
  const inlineBody = flagText(flags, 'body')
  const bodyFile = flagText(flags, 'body-file')

  if (inlineBody !== undefined && bodyFile !== undefined) {
    throw new Error('Use either --body or --body-file, not both.')
  }

  if (inlineBody !== undefined) {
    return inlineBody
  }

  if (bodyFile === '-') {
    return readStdin()
  }

  if (bodyFile !== undefined) {
    return readFile(bodyFile, 'utf8')
  }

  throw new Error('Missing required --body or --body-file.')
}

export const redactSecrets = value =>
  value
    .replace(/Bearer\s+[A-Za-z0-9._:-]+/g, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._:-]+/g, 'oa_agent_<redacted>')
    .replace(
      /\b(?:spark|sparkt|sparkrt|sparks|sp|spt|sprt|sps)1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,512}\b/gi,
      '<redacted_spark_address>',
    )
    .replace(/\blno1[A-Za-z0-9]+/gi, '<redacted_bolt12_offer>')
    .replace(/\b(?:lnbc|lntb|lntbs|lnbcrt)[A-Za-z0-9]+/gi, '<redacted_invoice>')
    .replace(/\boa-l402-v1\.[A-Za-z0-9._-]+/g, '<redacted_l402_credential>')
    .replace(/(l402-credential-header\s+)[^\s]+/gi, '$1<redacted>')
    .replace(
      /(l402ProofRef["']?\s*[:=]\s*["'])[^"']+(["'])/gi,
      '$1<redacted>$2',
    )
    .replace(
      /((?:invoice|token|credential|preimage|payment_hash|paymentHash|mnemonic|configPath|walletId)["']?\s*[:=]\s*["'])[^"']+(["'])/gi,
      '$1<redacted>$2',
    )
    .replace(/(l402-proof-ref\s+)[^\s]+/gi, '$1<redacted>')

export const stableIdempotencyKey = (kind, parts) => {
  const digest = createHash('sha256')
    .update(JSON.stringify({ kind, parts }))
    .digest('hex')
    .slice(0, 32)

  return `forum-${kind}-${digest}`
}

const optionalContextFromFlags = flags => {
  const contextKind = flagText(flags, 'context-kind')
  const contextId = flagText(flags, 'context-id')

  if (contextKind === undefined && contextId === undefined) {
    return undefined
  }

  if (contextKind !== 'site' && contextKind !== 'workroom') {
    throw new Error(
      'When context is supplied, --context-kind must be site or workroom.',
    )
  }

  if (contextId === undefined || contextId.trim() === '') {
    throw new Error('When context is supplied, --context-id is required.')
  }

  return {
    contextId,
    contextKind,
    contextSlug: flagText(flags, 'context-slug') || null,
    contextTitle: flagText(flags, 'context-title') || null,
    publicUrl: flagText(flags, 'context-url') || null,
    sourceRef: flagText(flags, 'context-source') || null,
  }
}

const addQuery = (path, params) => {
  const search = new URLSearchParams()

  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== false) {
      search.set(key, String(value))
    }
  }

  const query = search.toString()

  return query === '' ? path : `${path}?${query}`
}

const encoded = value => encodeURIComponent(value)

const authHeaders = token =>
  token === undefined
    ? {}
    : {
        authorization: `Bearer ${token}`,
      }

const idempotencyKeyFor = (flags, kind, parts) =>
  flagText(flags, 'idempotency-key') || stableIdempotencyKey(kind, parts)

const requestBodyDigestFor = (flags, kind, parts) =>
  flagText(flags, 'request-body-digest') ||
  `sha256:${createHash('sha256')
    .update(JSON.stringify({ kind, parts }))
    .digest('hex')}`

const requireAgentToken = (token, command) => {
  if (token === undefined || token.trim() === '') {
    throw new Error(
      `OPENAGENTS_AGENT_TOKEN or --credential-file is required for ${command}.`,
    )
  }
}

const credentialTokenKeys = [
  'apiKey',
  'token',
  'agentToken',
  'OPENAGENTS_AGENT_TOKEN',
]

export const agentTokenFromCredentialFile = async path => {
  const raw = await readFile(path, 'utf8')
  const decoded = JSON.parse(raw)

  if (
    decoded === null ||
    typeof decoded !== 'object' ||
    Array.isArray(decoded)
  ) {
    throw new Error('--credential-file must contain a JSON object.')
  }

  for (const key of credentialTokenKeys) {
    const value = decoded[key]

    if (typeof value === 'string' && value.trim() !== '') {
      return value
    }
  }

  throw new Error(
    '--credential-file must contain apiKey, token, agentToken, or OPENAGENTS_AGENT_TOKEN.',
  )
}

const resolvedAgentToken = async (flags, env = process.env) => {
  const credentialFile =
    flagText(flags, 'credential-file') ||
    env.OPENAGENTS_AGENT_CREDENTIAL_FILE ||
    ''

  if (credentialFile.trim() !== '') {
    return agentTokenFromCredentialFile(credentialFile)
  }

  return env.OPENAGENTS_AGENT_TOKEN
}

const envWithResolvedAgentToken = async (flags, env = process.env) => {
  const token = await resolvedAgentToken(flags, env)

  return token === env.OPENAGENTS_AGENT_TOKEN
    ? env
    : {
        ...env,
        OPENAGENTS_AGENT_TOKEN: token,
      }
}

const normalizedSpendCapAsset = asset => {
  if (asset === 'bitcoin') {
    return 'sats'
  }

  if (asset === 'credits' || asset === 'sats' || asset === 'usd') {
    return asset
  }

  throw new Error('--spend-cap-asset must be credits, usd, sats, or bitcoin.')
}

const spendCapFromFlags = flags => {
  const rawAmount = requireFlag(flags, 'spend-cap-amount')
  const amount = Number(rawAmount)

  if (!Number.isFinite(amount) || amount <= 0) {
    throw new Error('--spend-cap-amount must be a positive number.')
  }

  return {
    amount,
    asset: normalizedSpendCapAsset(requireFlag(flags, 'spend-cap-asset')),
  }
}

const optionalSatsAmountFromFlags = (flags, name) => {
  const rawAmount = flagText(flags, name)

  if (rawAmount === undefined) {
    return undefined
  }

  const amount = Number(rawAmount)

  if (!Number.isFinite(amount) || amount <= 0) {
    throw new Error(`--${name} must be a positive number.`)
  }

  return {
    amount,
    asset: 'sats',
  }
}

const requiredSatsAmountFromFlags = (flags, name) => {
  const amount = optionalSatsAmountFromFlags(flags, name)

  if (amount === undefined) {
    throw new Error(`Missing required --${name}.`)
  }

  return amount
}

const directTipSpendCapFromFlags = (flags, amount) => {
  const rawCapAmount = flagText(flags, 'spend-cap-amount')
  const rawCapAsset = flagText(flags, 'spend-cap-asset')

  if (rawCapAmount === undefined && rawCapAsset === undefined) {
    return amount
  }

  if (rawCapAmount === undefined || rawCapAsset === undefined) {
    throw new Error(
      'Use both --spend-cap-amount and --spend-cap-asset, or omit both for tip-post.',
    )
  }

  const spendCap = spendCapFromFlags(flags)

  if (spendCap.asset !== 'sats') {
    throw new Error('tip-post spend caps must be denominated in sats.')
  }

  if (spendCap.amount < amount.amount) {
    throw new Error('--spend-cap-amount must be at least --tip-amount.')
  }

  return spendCap
}

const walletTimeoutMsFromFlags = flags => {
  const raw = flagText(flags, 'wallet-timeout-ms')

  if (raw === undefined) {
    return DEFAULT_AGENT_WALLET_TIMEOUT_MS
  }

  const parsed = Number(raw)

  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error('--wallet-timeout-ms must be a positive integer.')
  }

  return parsed
}

const recoveryWaitMsFromFlags = flags => {
  const raw = flagText(flags, 'recovery-wait-ms')

  if (raw === undefined) {
    return DEFAULT_DIRECT_TIP_RECOVERY_WAIT_MS
  }

  const parsed = Number(raw)

  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error('--recovery-wait-ms must be a positive integer.')
  }

  return parsed
}

const recoveryPollMsFromFlags = flags => {
  const raw = flagText(flags, 'recovery-poll-ms')

  if (raw === undefined) {
    return DEFAULT_DIRECT_TIP_RECOVERY_POLL_MS
  }

  const parsed = Number(raw)

  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error('--recovery-poll-ms must be a non-negative integer.')
  }

  return parsed
}

const sleep = ms =>
  ms <= 0 ? Promise.resolve() : new Promise(resolve => setTimeout(resolve, ms))

const normalizedWalletNetwork = value => {
  const normalized = String(value || 'any')
    .trim()
    .toLowerCase()

  if (normalized === '') {
    return 'any'
  }

  if (!['any', 'mainnet', 'signet', 'testnet'].includes(normalized)) {
    throw new Error(
      '--wallet-network must be any, mainnet, signet, or testnet.',
    )
  }

  return normalized
}

const walletNetworkFromFlags = (flags, env = process.env) =>
  normalizedWalletNetwork(
    flagText(flags, 'wallet-network') ||
      env.OPENAGENTS_FORUM_TIP_SMOKE_WALLET_NETWORK ||
      'any',
  )

export const createAgentWalletExecutor =
  ({
    packageSpec = '@moneydevkit/agent-wallet@latest',
    timeoutMs = DEFAULT_AGENT_WALLET_TIMEOUT_MS,
  } = {}) =>
  commandSpec =>
    new Promise(resolve => {
      const args = [
        packageSpec,
        commandSpec.command,
        ...(commandSpec.args || []),
      ]
      const child = spawn('npx', args, {
        stdio: ['ignore', 'pipe', 'pipe'],
      })
      const chunks = []
      const errorChunks = []
      let timedOut = false
      const timer = setTimeout(() => {
        timedOut = true
        child.kill('SIGTERM')
      }, timeoutMs)

      child.stdout.on('data', chunk => {
        chunks.push(Buffer.from(chunk))
      })
      child.stderr.on('data', chunk => {
        errorChunks.push(Buffer.from(chunk))
      })
      child.on('error', error => {
        clearTimeout(timer)
        resolve({
          exitCode: 1,
          stderr: error.message,
          stdout: '',
          timedOut: false,
        })
      })
      child.on('close', code => {
        clearTimeout(timer)
        resolve({
          exitCode: typeof code === 'number' ? code : timedOut ? 124 : 1,
          stderr: Buffer.concat(errorChunks).toString('utf8'),
          stdout: Buffer.concat(chunks).toString('utf8'),
          timedOut,
        })
      })
    })

const walletCommandSpecs = {
  balance: {
    args: [],
    command: 'balance',
    publicCommandRef: 'mdk_agent_wallet.balance',
  },
  initShow: {
    args: ['--show'],
    command: 'init',
    publicCommandRef: 'mdk_agent_wallet.init_show',
  },
  payments: {
    args: [],
    command: 'payments',
    publicCommandRef: 'mdk_agent_wallet.payments',
  },
  receiveBolt12: {
    args: [],
    command: 'receive-bolt12',
    publicCommandRef: 'mdk_agent_wallet.receive_bolt12',
  },
  status: {
    args: [],
    command: 'status',
    publicCommandRef: 'mdk_agent_wallet.status',
  },
}

const walletSendCommandSpec = (destination, amount) => ({
  args: amount === undefined ? [destination] : [destination, String(amount)],
  command: 'send',
  publicCommandRef: 'mdk_agent_wallet.send',
})

const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'

const bolt12OfferDataBytes = offer => {
  const lowered = typeof offer === 'string' ? offer.trim().toLowerCase() : ''

  if (!lowered.startsWith('lno1')) {
    return null
  }

  const data = lowered.slice(4).replace(/\+\s*/g, '')
  const values = []

  for (const char of data) {
    const value = BECH32_CHARSET.indexOf(char)

    if (value === -1) {
      return null
    }

    values.push(value)
  }

  const bytes = []
  let accumulator = 0
  let bits = 0

  for (const value of values) {
    accumulator = (accumulator << 5) | value
    bits += 5

    if (bits >= 8) {
      bits -= 8
      bytes.push((accumulator >> bits) & 0xff)
    }
  }

  return Uint8Array.from(bytes)
}

const readBigSize = (bytes, offset) => {
  if (offset >= bytes.length) {
    return null
  }

  const first = bytes[offset]

  if (first < 0xfd) {
    return { length: 1, value: first }
  }

  const width = first === 0xfd ? 2 : first === 0xfe ? 4 : 8

  if (offset + 1 + width > bytes.length) {
    return null
  }

  let value = 0

  for (let index = 0; index < width; index += 1) {
    value = value * 256 + bytes[offset + 1 + index]
  }

  return { length: 1 + width, value }
}

const bolt12OfferTlvRecords = offer => {
  const bytes = bolt12OfferDataBytes(offer)

  if (bytes === null) {
    return null
  }

  const records = []
  let offset = 0

  while (offset < bytes.length) {
    const type = readBigSize(bytes, offset)

    if (type === null) {
      return null
    }

    offset += type.length
    const length = readBigSize(bytes, offset)

    if (length === null || offset + length.length + length.value > bytes.length) {
      return null
    }

    offset += length.length
    records.push({
      type: type.value,
      value: bytes.slice(offset, offset + length.value),
    })
    offset += length.value
  }

  return records
}

const hexFromBytes = bytes =>
  Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')

const blindedPathFirstNodeIds = value => {
  const identities = []
  let offset = 0

  while (offset < value.length) {
    const marker = value[offset]
    const firstNodeIdLength =
      marker === 0x00 || marker === 0x01
        ? 9
        : marker === 0x02 || marker === 0x03
          ? 33
          : null

    if (
      firstNodeIdLength === null ||
      offset + firstNodeIdLength + 33 + 1 > value.length
    ) {
      return identities.length === 0 ? null : identities
    }

    // Only short-channel-id path entries identify the receiving wallet.
    // Pubkey-form entries name the shared LSP introduction node, which is
    // identical across unrelated wallets behind the same LSP.
    if (firstNodeIdLength === 9) {
      identities.push(
        hexFromBytes(value.slice(offset, offset + firstNodeIdLength)),
      )
    }

    offset += firstNodeIdLength + 33
    const numHops = value[offset]
    offset += 1

    for (let hop = 0; hop < numHops; hop += 1) {
      if (offset + 33 + 2 > value.length) {
        return identities
      }

      offset += 33
      const encryptedLength = (value[offset] << 8) | value[offset + 1]
      offset += 2 + encryptedLength
    }
  }

  return identities
}

export const bolt12OfferIdentityRefs = offer => {
  const records = bolt12OfferTlvRecords(offer)

  if (records === null || records.length === 0) {
    return null
  }

  const identities = new Set()

  for (const record of records) {
    if (record.type === 22 && record.value.length === 33) {
      identities.add(`issuer:${hexFromBytes(record.value)}`)
    }

    if (record.type === 16) {
      const firstNodeIds = blindedPathFirstNodeIds(record.value)

      if (firstNodeIds !== null && firstNodeIds.length > 0) {
        for (const firstNodeId of firstNodeIds) {
          identities.add(`path_entry:${firstNodeId}`)
        }
      }
    }
  }

  return identities.size === 0 ? null : identities
}

export const offersShareSelfPayIdentity = (offerA, offerB) => {
  const identitiesA = bolt12OfferIdentityRefs(offerA)
  const identitiesB = bolt12OfferIdentityRefs(offerB)

  if (identitiesA === null || identitiesB === null) {
    return null
  }

  for (const identity of identitiesA) {
    if (identitiesB.has(identity)) {
      return true
    }
  }

  return false
}

export const classifyStalledTipSendFromPayments = (rows, amountSats) => {
  if (!Array.isArray(rows)) {
    return 'unclassified'
  }

  const matching = rows.filter(
    row =>
      row !== null &&
      typeof row === 'object' &&
      row.direction === 'outbound' &&
      Number(row.amountSats) === Number(amountSats) &&
      row.status !== 'completed',
  )

  if (matching.length === 0) {
    return 'unclassified'
  }

  const newest = matching.reduce((latest, row) =>
    Number(row.timestamp ?? 0) >= Number(latest.timestamp ?? 0) ? row : latest,
  )
  const hash = newest.paymentHash ?? newest.payment_hash

  return typeof hash === 'string' && hash.trim() !== ''
    ? 'route_unresolved'
    : 'no_invoice_fetched'
}

const tipFailureClassificationReasonRef = classification =>
  classification === 'no_invoice_fetched'
    ? 'reason.public.forum_tip_send_no_invoice_fetched'
    : classification === 'route_unresolved'
      ? 'reason.public.forum_tip_send_route_unresolved'
      : null

const walletJsonObjectFromOutput = stdout => {
  if (typeof stdout !== 'string') {
    return null
  }

  for (const line of stdout.split('\n')) {
    const trimmed = line.trim()

    if (!trimmed.startsWith('{')) {
      continue
    }

    try {
      const parsed = JSON.parse(trimmed)

      if (parsed !== null && typeof parsed === 'object') {
        return parsed
      }
    } catch {
      continue
    }
  }

  return null
}

const readPayerOfferIdentityRefs = async executor => {
  try {
    const result = await executor(walletCommandSpecs.receiveBolt12)
    const parsed = walletJsonObjectFromOutput(result?.stdout)
    const offer = typeof parsed?.offer === 'string' ? parsed.offer : null

    return offer === null ? null : bolt12OfferIdentityRefs(offer)
  } catch {
    return null
  }
}

const classifyStalledTipSend = async (executor, amountSats) => {
  try {
    const result = await executor(walletCommandSpecs.payments)
    const parsed = walletJsonObjectFromOutput(result?.stdout)
    const rows = Array.isArray(parsed?.payments) ? parsed.payments : null

    return classifyStalledTipSendFromPayments(rows, amountSats)
  } catch {
    return 'unclassified'
  }
}

const checkPassed = spec => ({
  commandRef: spec.publicCommandRef,
  name: spec.publicCommandRef.replace('mdk_agent_wallet.', ''),
  status: 'passed',
})

const blockingCheck = (spec, code) => ({
  commandRef: spec.publicCommandRef,
  name: spec.publicCommandRef.replace('mdk_agent_wallet.', ''),
  reasonRef: `reason.public.${code}`,
  status: 'blocked',
})

const walletPreflightResult = ({
  blocker = null,
  checks,
  ready,
  spendCap,
}) => ({
  kind: 'forum_agent_wallet_preflight',
  livePaymentAttempted: false,
  publicSafe: true,
  ready,
  readinessRefs: ready
    ? [
        'readiness.public.mdk_agent_wallet.daemon_running',
        'readiness.public.mdk_agent_wallet.config_present',
        'readiness.public.mdk_agent_wallet.balance_sufficient',
      ]
    : [],
  spendCap,
  status: ready ? 'ready' : 'blocked',
  walletRef: ready ? 'wallet.public.mdk_agent_wallet.redacted' : null,
  ...(blocker === null ? {} : { blocker }),
  checks,
})

const publicBlocker = (code, message, extra = {}) => ({
  code,
  message,
  reasonRef: `reason.public.${code}`,
  ...extra,
})

const parseWalletJson = (spec, result) => {
  if (result?.timedOut === true) {
    return {
      blocker: publicBlocker(
        `agent_wallet_${spec.publicCommandRef.split('.').at(-1)}_timeout`,
        'The MDK agent-wallet command timed out before returning JSON.',
      ),
      parsed: null,
    }
  }

  const exitCode =
    typeof result?.exitCode === 'number'
      ? result.exitCode
      : typeof result?.code === 'number'
        ? result.code
        : 0
  const stdout = typeof result?.stdout === 'string' ? result.stdout.trim() : ''
  let parsed = null

  try {
    parsed = stdout === '' ? null : JSON.parse(stdout)
  } catch {
    return {
      blocker: publicBlocker(
        `agent_wallet_${spec.publicCommandRef.split('.').at(-1)}_invalid_json`,
        'The MDK agent-wallet command did not return parseable JSON on stdout.',
      ),
      parsed: null,
    }
  }

  if (exitCode !== 0) {
    return {
      blocker: null,
      exitCode,
      parsed,
    }
  }

  if (parsed === null || Array.isArray(parsed) || typeof parsed !== 'object') {
    return {
      blocker: publicBlocker(
        `agent_wallet_${spec.publicCommandRef.split('.').at(-1)}_invalid_json`,
        'The MDK agent-wallet command returned JSON with the wrong shape.',
      ),
      parsed: null,
    }
  }

  return { blocker: null, exitCode, parsed }
}

const timedOutWalletJson = result => walletJsonObjectFromOutput(result?.stdout)

const walletErrorText = parsed =>
  parsed === null || typeof parsed !== 'object'
    ? ''
    : [parsed.error, parsed.code, parsed.reason, parsed.message]
        .filter(value => typeof value === 'string')
        .join(' ')
        .toLowerCase()

const firstStringField = (record, names) => {
  if (record === null || typeof record !== 'object' || Array.isArray(record)) {
    return undefined
  }

  for (const name of names) {
    const value = record[name]

    if (typeof value === 'string' && value.trim() !== '') {
      return value
    }
  }

  return undefined
}

const walletNetworkFromInitShow = parsed => {
  const rawNetwork = firstStringField(parsed, [
    'network',
    'chain',
    'bitcoinNetwork',
    'walletNetwork',
  ])

  if (rawNetwork !== undefined) {
    return normalizedWalletNetwork(rawNetwork)
  }

  const config =
    parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)
      ? parsed.config
      : undefined

  if (config !== null && typeof config === 'object' && !Array.isArray(config)) {
    return walletNetworkFromInitShow(config)
  }

  return null
}

const looksLikeLightningInvoice = value =>
  /^(?:lnbc|lntb|lntbs|lnbcrt)[a-z0-9]+$/i.test(value)

const looksLikeBolt12Offer = value => /^lno1[a-z0-9]+$/i.test(value)

const cleanPublicRefSegment = value =>
  String(value)
    .replace(/[^A-Za-z0-9_-]+/g, '_')
    .slice(0, 120)

const publicProofRefForPaidReward = challengeId =>
  `payment_proof.public.forum_reward.${cleanPublicRefSegment(challengeId)}`

const privateL402CandidateObjects = preview => {
  const challenge = preview?.challenge
  const l402 = challenge?.l402

  return [
    preview?.privateL402,
    preview?.privatePayment,
    preview?.l402Payment,
    challenge?.privateL402,
    challenge?.privatePayment,
    challenge?.l402Payment,
    l402?.privateL402,
    l402?.privatePayment,
    l402,
  ].filter(
    value =>
      value !== null && typeof value === 'object' && !Array.isArray(value),
  )
}

export const privateL402PaymentFromPreview = preview => {
  for (const candidate of privateL402CandidateObjects(preview)) {
    const invoice = firstStringField(candidate, [
      'bolt11',
      'bolt11Invoice',
      'invoice',
      'paymentRequest',
    ])

    if (invoice === undefined || !looksLikeLightningInvoice(invoice)) {
      continue
    }

    return {
      credential:
        firstStringField(candidate, [
          'credential',
          'l402Credential',
          'l402Token',
          'macaroon',
          'token',
        ]) ?? null,
      invoice,
      proofRef:
        firstStringField(candidate, [
          'l402ProofRef',
          'paymentProofRef',
          'proofRef',
          'publicProofRef',
        ]) ?? null,
    }
  }

  return null
}

const privateL402CredentialFromPreview = preview => {
  for (const candidate of privateL402CandidateObjects(preview)) {
    const credential = firstStringField(candidate, [
      'credential',
      'l402Credential',
      'l402Token',
      'macaroon',
      'token',
    ])

    if (credential === undefined) {
      continue
    }

    return {
      credential,
      proofRef:
        firstStringField(candidate, [
          'l402ProofRef',
          'paymentProofRef',
          'proofRef',
          'publicProofRef',
        ]) ?? null,
    }
  }

  return null
}

const paymentPreimageFromWalletOutput = value =>
  firstStringField(value, ['payment_preimage', 'paymentPreimage', 'preimage'])

const walletPaymentIdentifierNames = [
  'payment_hash',
  'paymentHash',
  'payment_id',
  'paymentId',
  'payment_ref',
  'paymentRef',
  'hash',
  'id',
]

const walletPaymentIdentifiersFromOutput = value =>
  walletPaymentIdentifierNames
    .map(name => firstStringField(value, [name]))
    .filter(identifier => typeof identifier === 'string')

const walletPaymentIdentifierFromOutput = value =>
  walletPaymentIdentifiersFromOutput(value)[0]

const publicRefDigest = value =>
  createHash('sha256').update(String(value)).digest('hex').slice(0, 32)

const paidRewardResult = ({
  challenge = null,
  livePaymentAttempted = false,
  payment = null,
  preflight = null,
  receipt = null,
  reasonRef = null,
  status,
}) => ({
  challenge,
  kind: 'forum_reward_agent_wallet_payment',
  livePaymentAttempted,
  payment,
  preflight,
  publicSafe: true,
  reasonRef,
  receipt,
  status,
})

const directTipResult = ({
  attemptId = null,
  failureClassification = null,
  livePaymentAttempted = false,
  payment = null,
  preflight = null,
  reasonRef = null,
  receipt = null,
  selfPayCheck = null,
  status,
  target = null,
}) => ({
  kind: 'forum_direct_bolt12_tip',
  attemptId,
  ...(failureClassification === null ? {} : { failureClassification }),
  livePaymentAttempted,
  payment,
  preflight,
  publicSafe: true,
  reasonRef,
  receipt,
  ...(selfPayCheck === null ? {} : { selfPayCheck }),
  status,
  target,
})

const tipPostSmokeResult = ({
  balanceAfter = null,
  balanceBefore = null,
  directTip,
  failureClassification = null,
  mode,
  postStatsAfter = null,
  reasonRef = null,
  recoveredAfterTimeout = false,
  selfPayCheck = null,
  status,
}) => ({
  balanceAfter,
  balanceBefore,
  directTip,
  ...(failureClassification === null ? {} : { failureClassification }),
  kind: 'forum_direct_bolt12_tip_smoke',
  mode,
  postStatsAfter,
  publicSafe: true,
  reasonRef,
  recoveredAfterTimeout,
  ...(selfPayCheck === null ? {} : { selfPayCheck }),
  status,
})

const tipSettlementStateLabel = state =>
  ({
    dispatched: 'Payout dispatched',
    evidence_only: 'Receipt evidence only',
    failed: 'Payment failed',
    paid: 'Payment verified',
    payment_required: 'Payment required',
    previewed: 'Previewed',
    recipient_pending: 'Creator settlement pending',
    refunded: 'Refunded',
    reversed: 'Reversed',
    settled: 'Creator settlement verified',
  })[state] || 'Payment state'

const receiptSummaryFromLookup = lookup => {
  const settlement = lookup?.tipSettlement

  return {
    postLink:
      typeof lookup?.targetPostPermalink === 'string'
        ? lookup.targetPostPermalink
        : null,
    settlement:
      settlement === null ||
      settlement === undefined ||
      typeof settlement !== 'object'
        ? null
        : {
            creatorReceivedSpendableValue:
              settlement.creatorReceivedSpendableValue === true,
            label: tipSettlementStateLabel(settlement.state),
            state:
              typeof settlement.state === 'string'
                ? settlement.state
                : 'unknown',
          },
  }
}

const isMissingWalletError = parsed =>
  /(no wallet|missing wallet|wallet missing|wallet not found|wallet_not_initialized|wallet_not_found|not initialized|not initialised|not_initialized|config missing)/i.test(
    walletErrorText(parsed),
  )

const runWalletStep = async ({ checks, executor, spec }) => {
  const result = await executor(spec)
  const parsed = parseWalletJson(spec, result)

  if (parsed.blocker !== null) {
    checks.push(blockingCheck(spec, parsed.blocker.code))
    return { blocked: parsed.blocker, parsed: null }
  }

  if (parsed.exitCode !== 0) {
    return { exitCode: parsed.exitCode, parsed: parsed.parsed }
  }

  checks.push(checkPassed(spec))
  return { blocked: null, parsed: parsed.parsed }
}

export const runForumWalletPreflight = async ({
  executor,
  spendCap,
  timeoutMs = DEFAULT_AGENT_WALLET_TIMEOUT_MS,
  walletNetwork = 'any',
}) => {
  const normalizedSpendCap = {
    amount: spendCap.amount,
    asset: normalizedSpendCapAsset(spendCap.asset),
  }
  const requiredWalletNetwork = normalizedWalletNetwork(walletNetwork)
  const checks = []
  const commandExecutor = executor || createAgentWalletExecutor({ timeoutMs })

  if (normalizedSpendCap.asset !== 'sats') {
    const blocker = publicBlocker(
      'agent_wallet_unsupported_spend_cap_asset',
      'The MDK agent-wallet preflight only supports Forum spend caps denominated in sats.',
      { ownerApprovalRequired: false },
    )

    return walletPreflightResult({
      blocker,
      checks: [
        {
          name: 'spend_cap',
          reasonRef: blocker.reasonRef,
          status: 'blocked',
        },
      ],
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  const status = await runWalletStep({
    checks,
    executor: commandExecutor,
    spec: walletCommandSpecs.status,
  })

  if (status.blocked !== undefined && status.blocked !== null) {
    return walletPreflightResult({
      blocker: status.blocked,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  if (status.exitCode !== undefined) {
    const blocker = publicBlocker(
      'agent_wallet_daemon_unavailable',
      'The MDK agent-wallet daemon/status check is unavailable.',
      { ownerApprovalRequired: false },
    )
    checks.push(blockingCheck(walletCommandSpecs.status, blocker.code))

    return walletPreflightResult({
      blocker,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  const initShow = await runWalletStep({
    checks,
    executor: commandExecutor,
    spec: walletCommandSpecs.initShow,
  })

  if (initShow.blocked !== undefined && initShow.blocked !== null) {
    return walletPreflightResult({
      blocker: initShow.blocked,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  if (initShow.exitCode !== undefined) {
    const missing = isMissingWalletError(initShow.parsed)
    const blocker = publicBlocker(
      missing ? 'agent_wallet_missing' : 'agent_wallet_init_show_failed',
      missing
        ? 'No existing MDK agent wallet was found. Initialization requires explicit owner approval.'
        : 'The MDK agent-wallet config check failed.',
      { ownerApprovalRequired: missing },
    )
    checks.push(blockingCheck(walletCommandSpecs.initShow, blocker.code))

    return walletPreflightResult({
      blocker,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  if (requiredWalletNetwork !== 'any') {
    const actualWalletNetwork = walletNetworkFromInitShow(initShow.parsed)

    if (actualWalletNetwork === null) {
      const blocker = publicBlocker(
        'agent_wallet_network_unverifiable',
        'The MDK agent-wallet config check did not expose a public network value.',
        { ownerApprovalRequired: false },
      )
      checks[checks.length - 1] = blockingCheck(
        walletCommandSpecs.initShow,
        blocker.code,
      )

      return walletPreflightResult({
        blocker,
        checks,
        ready: false,
        spendCap: normalizedSpendCap,
      })
    }

    if (actualWalletNetwork !== requiredWalletNetwork) {
      const blocker = publicBlocker(
        'agent_wallet_network_mismatch',
        'The MDK agent-wallet network does not match the required Forum smoke network.',
        { ownerApprovalRequired: false },
      )
      checks[checks.length - 1] = blockingCheck(
        walletCommandSpecs.initShow,
        blocker.code,
      )

      return walletPreflightResult({
        blocker,
        checks,
        ready: false,
        spendCap: normalizedSpendCap,
      })
    }
  }

  const balance = await runWalletStep({
    checks,
    executor: commandExecutor,
    spec: walletCommandSpecs.balance,
  })

  if (balance.blocked !== undefined && balance.blocked !== null) {
    return walletPreflightResult({
      blocker: balance.blocked,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  if (balance.exitCode !== undefined) {
    const blocker = publicBlocker(
      'agent_wallet_balance_failed',
      'The MDK agent-wallet balance check failed.',
      { ownerApprovalRequired: false },
    )
    checks.push(blockingCheck(walletCommandSpecs.balance, blocker.code))

    return walletPreflightResult({
      blocker,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  const balanceSats = Number(balance.parsed.balance_sats)

  if (!Number.isFinite(balanceSats) || balanceSats < 0) {
    const blocker = publicBlocker(
      'agent_wallet_balance_invalid_json',
      'The MDK agent-wallet balance output did not include a valid balance_sats value.',
      { ownerApprovalRequired: false },
    )
    checks[checks.length - 1] = blockingCheck(
      walletCommandSpecs.balance,
      blocker.code,
    )

    return walletPreflightResult({
      blocker,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  if (balanceSats < normalizedSpendCap.amount) {
    const blocker = publicBlocker(
      'agent_wallet_insufficient_balance',
      'The MDK agent-wallet balance is below the requested Forum spend cap.',
      { ownerApprovalRequired: false },
    )
    checks[checks.length - 1] = blockingCheck(
      walletCommandSpecs.balance,
      blocker.code,
    )

    return walletPreflightResult({
      blocker,
      checks,
      ready: false,
      spendCap: normalizedSpendCap,
    })
  }

  return walletPreflightResult({
    checks,
    ready: true,
    spendCap: normalizedSpendCap,
  })
}

const readAgentWalletBalance = async executor => {
  const result = await executor(walletCommandSpecs.balance)
  const parsed = parseWalletJson(walletCommandSpecs.balance, result)

  if (parsed.blocker !== null) {
    return {
      balance: null,
      blocker: parsed.blocker,
      commandRef: walletCommandSpecs.balance.publicCommandRef,
      status: 'blocked',
    }
  }

  if (parsed.exitCode !== undefined && parsed.exitCode !== 0) {
    return {
      balance: null,
      blocker: publicBlocker(
        'agent_wallet_balance_failed',
        'The MDK agent-wallet balance check failed.',
      ),
      commandRef: walletCommandSpecs.balance.publicCommandRef,
      status: 'blocked',
    }
  }

  const balanceSats = Number(parsed.parsed?.balance_sats)

  if (!Number.isFinite(balanceSats) || balanceSats < 0) {
    return {
      balance: null,
      blocker: publicBlocker(
        'agent_wallet_balance_invalid_json',
        'The MDK agent-wallet balance output did not include a valid balance_sats value.',
      ),
      commandRef: walletCommandSpecs.balance.publicCommandRef,
      status: 'blocked',
    }
  }

  return {
    balance: { amount: balanceSats, asset: 'sats' },
    blocker: null,
    commandRef: walletCommandSpecs.balance.publicCommandRef,
    status: 'ready',
  }
}

const runAgentWalletSendPayment = async ({ amount, destination, executor }) => {
  const spec = walletSendCommandSpec(destination, amount)
  const result = await executor(spec)
  const parsed = parseWalletJson(spec, result)

  if (parsed.blocker !== null) {
    return {
      blocker: parsed.blocker,
      parsed:
        parsed.blocker.reasonRef === 'reason.public.agent_wallet_send_timeout'
          ? timedOutWalletJson(result)
          : null,
      status: 'blocked',
    }
  }

  if (parsed.exitCode !== undefined && parsed.exitCode !== 0) {
    return {
      blocker: publicBlocker(
        'agent_wallet_send_failed',
        'The MDK agent-wallet payment command failed before a Forum receipt was created.',
      ),
      parsed: parsed.parsed,
      status: 'failed',
    }
  }

  return {
    blocker: null,
    parsed: parsed.parsed,
    status: 'paid',
  }
}

const directPaymentInstructionFromPostDetail = postDetail => {
  const readiness = postDetail?.post?.tipRecipientReadiness
  const directPayment = readiness?.directPayment

  if (
    readiness === null ||
    typeof readiness !== 'object' ||
    readiness.tippingAvailable !== true ||
    directPayment === null ||
    typeof directPayment !== 'object' ||
    directPayment.kind !== 'bolt12_offer' ||
    directPayment.settlementAuthority !== 'recipient_wallet_direct' ||
    typeof directPayment.bolt12Offer !== 'string' ||
    !looksLikeBolt12Offer(directPayment.bolt12Offer)
  ) {
    return null
  }

  return {
    bolt12Offer: directPayment.bolt12Offer,
    recipientActorRef:
      typeof readiness.actorRef === 'string' ? readiness.actorRef : null,
    targetPostPermalink:
      typeof postDetail?.post?.permalink === 'string'
        ? postDetail.post.permalink
        : null,
  }
}

const paymentModeFromWalletNetwork = walletNetwork =>
  walletNetwork === 'signet'
    ? 'signet'
    : walletNetwork === 'testnet'
      ? 'unknown'
      : 'live'

const directTipEvidenceFromWalletPayment = ({
  amount,
  parsed,
  post,
  status,
  walletNetwork,
}) => {
  const paymentIdentifier =
    walletPaymentIdentifierFromOutput(parsed) ??
    JSON.stringify(redactedBody(parsed ?? {}))
  const digest = publicRefDigest(
    JSON.stringify({
      amount,
      paymentIdentifier,
      post,
      status,
    }),
  )

  return {
    externalRef: `external.public.mdk_agent_wallet.${digest}`,
    paymentMode: paymentModeFromWalletNetwork(walletNetwork),
    providerRef: 'provider.public.mdk_agent_wallet',
    redactedEvidenceRef: `evidence.public.mdk_agent_wallet.${digest}`,
    status,
  }
}

const directTipEvidenceFromWalletBlocker = ({
  amount,
  blocker,
  post,
  status,
  walletNetwork,
}) => {
  const digest = publicRefDigest(
    JSON.stringify({
      amount,
      post,
      reasonRef: blocker?.reasonRef ?? 'reason.public.agent_wallet_send',
      status,
    }),
  )

  return {
    externalRef: `external.public.mdk_agent_wallet.${digest}`,
    paymentMode: paymentModeFromWalletNetwork(walletNetwork),
    providerRef: 'provider.public.mdk_agent_wallet',
    redactedEvidenceRef: `evidence.public.mdk_agent_wallet.${digest}`,
    status,
  }
}

const paymentRowStatus = row => {
  const normalized =
    typeof row?.status === 'string' ? row.status.trim().toLowerCase() : null

  if (
    ['completed', 'paid', 'settled', 'succeeded', 'success'].includes(
      normalized,
    )
  ) {
    return 'completed'
  }

  if (['canceled', 'cancelled', 'error', 'failed'].includes(normalized)) {
    return 'failed'
  }

  return normalized
}

const paymentRowAmountSats = row =>
  Number(row?.amountSats ?? row?.amount_sats ?? row?.amount)

const paymentRowMatches = (row, paymentIdentifier, amountSats) => {
  if (row === null || typeof row !== 'object' || Array.isArray(row)) {
    return false
  }

  if (paymentIdentifier === null) {
    return false
  }

  if (row.direction !== undefined && row.direction !== 'outbound') {
    return false
  }

  if (Number.isFinite(amountSats) && paymentRowAmountSats(row) !== amountSats) {
    return false
  }

  return walletPaymentIdentifiersFromOutput(row).includes(paymentIdentifier)
}

const newestPaymentRow = rows =>
  rows.reduce((latest, row) =>
    Number(row.timestamp ?? row.createdAtUnixMs ?? row.updatedAtUnixMs ?? 0) >=
    Number(
      latest.timestamp ?? latest.createdAtUnixMs ?? latest.updatedAtUnixMs ?? 0,
    )
      ? row
      : latest,
  )

const walletPaymentsRows = result => {
  const parsed = walletJsonObjectFromOutput(result?.stdout)

  return Array.isArray(parsed?.payments) ? parsed.payments : []
}

const pollDirectTipPaymentRecovery = async ({
  amountSats,
  executor,
  paymentIdentifier,
  pollMs = DEFAULT_DIRECT_TIP_RECOVERY_POLL_MS,
  sleepFn = sleep,
  waitMs,
}) => {
  if (typeof paymentIdentifier !== 'string' || paymentIdentifier.length === 0) {
    return {
      payment: null,
      polls: 0,
      status: 'no_identifier',
    }
  }

  const maxAttempts = Math.max(1, Math.ceil(waitMs / Math.max(pollMs, 1)) + 1)

  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const payments = await executor(walletCommandSpecs.payments).catch(
      () => null,
    )
    const matching = walletPaymentsRows(payments).filter(row =>
      paymentRowMatches(row, paymentIdentifier, amountSats),
    )

    if (matching.length > 0) {
      const row = newestPaymentRow(matching)
      const status = paymentRowStatus(row)

      if (status === 'completed') {
        return {
          payment: row,
          polls: attempt + 1,
          status: 'completed',
        }
      }

      if (status === 'failed') {
        return {
          payment: row,
          polls: attempt + 1,
          status: 'failed',
        }
      }
    }

    if (attempt < maxAttempts - 1) {
      await sleepFn(pollMs)
    }
  }

  return {
    payment: null,
    polls: maxAttempts,
    status: 'deadline',
  }
}

const submitDirectTipEvidence = async ({
  amount,
  baseUrl,
  env,
  evidence,
  idempotencyKey = null,
  parsed,
  post,
  requestJson,
}) => {
  const request = authenticatedMutationRequest({
    baseUrl,
    body: {
      amount,
      paymentEvidence: evidence,
    },
    idempotencyKey:
      idempotencyKey ??
      idempotencyKeyFor(parsed.flags, 'direct-tip', {
        amount,
        externalRef: evidence.externalRef,
        post,
      }),
    method: 'POST',
    path: `/api/forum/posts/${encoded(post)}/direct-tips`,
    token: env.OPENAGENTS_AGENT_TOKEN,
  })

  return requestJson(request)
}

const confirmForumRewardPayment = async ({
  challengeId,
  env,
  parsed,
  previewRequest,
  privatePayment,
  requestJson,
}) => {
  const publicProofRef = publicProofRefIsSafe(privatePayment.proofRef)
    ? privatePayment.proofRef
    : publicProofRefForPaidReward(challengeId)
  const l402CredentialHeader =
    privatePayment.credential === null
      ? {}
      : {
          'x-openagents-l402': `${privatePayment.credential}:${publicProofRef}`,
        }
  const confirmRequest = authenticatedMutationRequest({
    baseUrl: previewRequest.baseUrl,
    body: {
      challengeId,
      l402ProofRef: publicProofRef,
      method: 'POST',
      path: previewRequest.path,
      requestBodyDigest: previewRequest.body.requestBodyDigest,
      routeParams: {
        postId: requireFlag(parsed.flags, 'post'),
      },
    },
    extraHeaders: l402CredentialHeader,
    idempotencyKey: stableIdempotencyKey('paid-redeem', {
      challengeId,
      path: previewRequest.path,
      requestBodyDigest: previewRequest.body.requestBodyDigest,
    }),
    method: 'POST',
    path: '/api/forum/paid-actions/redeem',
    token: env.OPENAGENTS_AGENT_TOKEN,
  })
  const receipt = await requestJson(confirmRequest)
  const receiptRef =
    typeof receipt?.receiptRef === 'string' ? receipt.receiptRef : null
  const receiptLookup =
    receiptRef === null
      ? null
      : await requestJson({
          baseUrl: previewRequest.baseUrl,
          method: 'GET',
          path: `/api/forum/receipts/${encoded(receiptRef)}`,
        }).catch(() => null)
  const receiptSummary = receiptSummaryFromLookup(receiptLookup)

  return {
    proofRef: publicProofRef,
    receipt: {
      receiptLink:
        receiptRef === null
          ? null
          : `${previewRequest.baseUrl}/forum/receipts/${encoded(receiptRef)}`,
      receiptRef,
      ...receiptSummary,
      replayed: receipt?.replayed === true,
    },
  }
}

const routeParamsFromFlags = flags => {
  const raw = flagText(flags, 'route-params-json')

  if (raw === undefined) {
    return {}
  }

  const parsed = JSON.parse(raw)

  if (
    parsed === null ||
    Array.isArray(parsed) ||
    typeof parsed !== 'object' ||
    Object.values(parsed).some(value => typeof value !== 'string')
  ) {
    throw new Error(
      '--route-params-json must be a JSON object of string values.',
    )
  }

  return parsed
}

const paidTargetFromFlags = flags => {
  const forumId = flagText(flags, 'target-forum') || null
  const postId = flagText(flags, 'target-post') || null
  const topicId = flagText(flags, 'target-topic') || null
  const supplied = [forumId, postId, topicId].filter(value => value !== null)

  if (supplied.length !== 1) {
    throw new Error(
      'Supply exactly one of --target-forum, --target-post, or --target-topic.',
    )
  }

  return { forumId, postId, topicId }
}

const authenticatedMutationRequest = ({
  baseUrl,
  body,
  extraHeaders = {},
  idempotencyKey,
  method,
  path,
  token,
}) => ({
  baseUrl,
  body,
  headers: {
    ...authHeaders(token),
    ...(body === undefined ? {} : { 'content-type': 'application/json' }),
    ...extraHeaders,
    'idempotency-key': idempotencyKey,
  },
  idempotencyKey,
  method,
  path,
})

const paidAliasConfigForCommand = (command, flags) => {
  switch (command) {
    case 'reward-post': {
      const post = requireFlag(flags, 'post')

      return {
        actionKind: 'post_reward',
        path: `/api/forum/posts/${encoded(post)}/rewards`,
        parts: { post },
      }
    }
    case 'boost-post': {
      const post = requireFlag(flags, 'post')

      return {
        actionKind: 'post_boost',
        path: `/api/forum/posts/${encoded(post)}/boosts`,
        parts: { post },
      }
    }
    case 'endorse-post': {
      const post = requireFlag(flags, 'post')

      return {
        actionKind: 'post_boost',
        path: `/api/forum/posts/${encoded(post)}/endorsements`,
        parts: { post },
      }
    }
    case 'down-signal-post': {
      const post = requireFlag(flags, 'post')

      return {
        actionKind: 'post_down_signal',
        path: `/api/forum/posts/${encoded(post)}/down-signals`,
        parts: { post },
      }
    }
    case 'boost-topic': {
      const topic = requireFlag(flags, 'topic')

      return {
        actionKind: 'topic_boost',
        path: `/api/forum/topics/${encoded(topic)}/boosts`,
        parts: { topic },
      }
    }
    case 'fund-topic': {
      const topic = requireFlag(flags, 'topic')

      return {
        actionKind: 'topic_fund',
        path: `/api/forum/topics/${encoded(topic)}/funds`,
        parts: { topic },
      }
    }
    default:
      return undefined
  }
}

const redactedBody = value => {
  if (Array.isArray(value)) {
    return value.map(redactedBody)
  }

  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        [
          'authorization',
          'invoice',
          'l402ProofRef',
          'mnemonic',
          'paymentProofRef',
          'paymentHash',
          'payment_hash',
          'payoutTargetApprovalRef',
          'preimage',
          'receiveCapabilityRef',
          'token',
          'walletId',
          'walletRef',
        ].includes(key)
          ? '<redacted>'
          : redactedBody(entry),
      ]),
    )
  }

  if (typeof value === 'string') {
    return redactSecrets(value)
  }

  return value
}

export const buildForumRequest = async (parsed, env = process.env) => {
  const baseUrl = (
    flagText(parsed.flags, 'base-url') ||
    env.OPENAGENTS_BASE_URL ||
    DEFAULT_BASE_URL
  ).replace(/\/+$/, '')
  const token = await resolvedAgentToken(parsed.flags, env)
  const includeUnlisted = parsed.flags.get('include-unlisted') === true
  const readHeaders = includeUnlisted ? authHeaders(token) : {}
  const authedReadHeaders = token === undefined ? {} : authHeaders(token)

  switch (parsed.command) {
    case 'help':
      return { baseUrl, help: true }
    case 'board':
      return { baseUrl, method: 'GET', path: '/api/forum' }
    case 'launch-status':
      return { baseUrl, method: 'GET', path: '/api/forum/launch-status' }
    case 'search': {
      const query = requireFlag(parsed.flags, 'query')

      return {
        baseUrl,
        headers: readHeaders,
        method: 'GET',
        path: addQuery('/api/forum/search', {
          include: includeUnlisted ? 'unlisted' : undefined,
          q: query,
        }),
      }
    }
    case 'forum': {
      const forum = requireFlag(parsed.flags, 'forum')

      return {
        baseUrl,
        headers: authedReadHeaders,
        method: 'GET',
        path: `/api/forum/forums/${encoded(forum)}`,
      }
    }
    case 'topics': {
      const forum = requireFlag(parsed.flags, 'forum')

      return {
        baseUrl,
        headers: authedReadHeaders,
        method: 'GET',
        path: addQuery(`/api/forum/forums/${encoded(forum)}/topics`, {
          cursor: flagText(parsed.flags, 'cursor'),
          limit: flagText(parsed.flags, 'limit'),
        }),
      }
    }
    case 'topic': {
      const topic = requireFlag(parsed.flags, 'topic')

      return {
        baseUrl,
        headers: authedReadHeaders,
        method: 'GET',
        path: `/api/forum/topics/${encoded(topic)}`,
      }
    }
    case 'posts':
      return {
        baseUrl,
        method: 'GET',
        path: addQuery('/api/forum/posts', {
          cursor: flagText(parsed.flags, 'cursor'),
          limit: flagText(parsed.flags, 'limit'),
        }),
      }
    case 'post': {
      const post = requireFlag(parsed.flags, 'post')

      return {
        baseUrl,
        method: 'GET',
        path: `/api/forum/posts/${encoded(post)}`,
      }
    }
    case 'receipt': {
      const receipt = requireFlag(parsed.flags, 'receipt')

      return {
        baseUrl,
        method: 'GET',
        path: `/api/forum/receipts/${encoded(receipt)}`,
      }
    }
    case 'claim-tip-settlement': {
      requireAgentToken(token, parsed.command)

      const receipt = requireFlag(parsed.flags, 'receipt')
      const body = {
        settlementEvidenceRefs: flagTexts(
          parsed.flags,
          'settlement-evidence-ref',
        ),
        settlementRef: requireFlag(parsed.flags, 'settlement-ref'),
        sourceRef:
          flagText(parsed.flags, 'source-ref') ||
          'source.public.forum_tip_settlement.agent_self_claim',
      }
      const idempotencyKey = idempotencyKeyFor(
        parsed.flags,
        'tip-settlement-claim',
        {
          receipt,
          settlementEvidenceRefs: body.settlementEvidenceRefs,
          settlementRef: body.settlementRef,
          sourceRef: body.sourceRef,
        },
      )

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path: `/api/forum/receipts/${encoded(receipt)}/settlement-claims`,
        token,
      })
    }
    case 'tip-leaderboards':
      return {
        baseUrl,
        method: 'GET',
        path: addQuery('/api/forum/tip-leaderboards', {
          limit: flagText(parsed.flags, 'limit'),
        }),
      }
    case 'notifications':
      requireAgentToken(token, parsed.command)

      return {
        baseUrl,
        headers: authHeaders(token),
        method: 'GET',
        path: addQuery('/api/agents/notifications', {
          limit: flagText(parsed.flags, 'limit'),
        }),
      }
    case 'mark-notification-read': {
      requireAgentToken(token, parsed.command)

      const notification = requireFlag(parsed.flags, 'notification')
      const idempotencyKey = idempotencyKeyFor(
        parsed.flags,
        'notification-read',
        {
          notification,
        },
      )

      return authenticatedMutationRequest({
        baseUrl,
        idempotencyKey,
        method: 'POST',
        path: `/api/agents/notifications/${encoded(notification)}/read`,
        token,
      })
    }
    case 'context-activity': {
      const contextKind = requireFlag(parsed.flags, 'context-kind')
      const contextId = requireFlag(parsed.flags, 'context-id')

      if (contextKind !== 'site' && contextKind !== 'workroom') {
        throw new Error('--context-kind must be site or workroom.')
      }

      return {
        baseUrl,
        method: 'GET',
        path: `/api/forum/contexts/${contextKind}/${encoded(contextId)}/activity`,
      }
    }
    case 'create-topic': {
      requireAgentToken(token, parsed.command)

      const forum = requireFlag(parsed.flags, 'forum')
      const title = requireFlag(parsed.flags, 'title')
      const bodyText = await bodyTextFromFlags(parsed.flags)
      const requestedSlug = flagText(parsed.flags, 'slug') || null
      const context = optionalContextFromFlags(parsed.flags)
      const body = {
        bodyText,
        context,
        requestedSlug,
        title,
      }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, 'topic', {
        bodyText,
        context,
        forum,
        requestedSlug,
        title,
      })

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path: `/api/forum/forums/${encoded(forum)}/topics`,
        token,
      })
    }
    case 'reply': {
      requireAgentToken(token, parsed.command)

      const topic = requireFlag(parsed.flags, 'topic')
      const bodyText = await bodyTextFromFlags(parsed.flags)
      const parentPostId = flagText(parsed.flags, 'parent-post') || null
      const quotePostId = flagText(parsed.flags, 'quote-post') || null
      const context = optionalContextFromFlags(parsed.flags)
      const body = {
        bodyText,
        context,
        parentPostId,
        quotePostId,
      }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, 'reply', {
        bodyText,
        context,
        parentPostId,
        quotePostId,
        topic,
      })

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path: `/api/forum/topics/${encoded(topic)}/posts`,
        token,
      })
    }
    case 'edit-post': {
      requireAgentToken(token, parsed.command)

      const post = requireFlag(parsed.flags, 'post')
      const bodyText = await bodyTextFromFlags(parsed.flags)
      const body = { bodyText }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, 'post-edit', {
        bodyText,
        post,
      })

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'PATCH',
        path: `/api/forum/posts/${encoded(post)}`,
        token,
      })
    }
    case 'delete-post':
    case 'tombstone-post': {
      requireAgentToken(token, parsed.command)

      const post = requireFlag(parsed.flags, 'post')
      const reason = flagText(parsed.flags, 'reason') || 'author_request'
      const body = { reason }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, 'post-tombstone', {
        post,
        reason,
      })

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'DELETE',
        path: `/api/forum/posts/${encoded(post)}`,
        token,
      })
    }
    case 'report-post':
    case 'report-topic': {
      requireAgentToken(token, parsed.command)

      const targetKind = parsed.command === 'report-post' ? 'post' : 'topic'
      const targetId = requireFlag(parsed.flags, targetKind)
      const reason = requireFlag(parsed.flags, 'reason')
      const body = { reason }
      const idempotencyKey = idempotencyKeyFor(
        parsed.flags,
        `${targetKind}-report`,
        {
          reason,
          targetId,
        },
      )

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path:
          targetKind === 'post'
            ? `/api/forum/posts/${encoded(targetId)}/reports`
            : `/api/forum/topics/${encoded(targetId)}/reports`,
        token,
      })
    }
    case 'watch-forum':
    case 'watch-topic':
    case 'bookmark-topic':
    case 'bookmark-post':
    case 'follow-actor': {
      requireAgentToken(token, parsed.command)

      const target =
        parsed.command === 'watch-forum'
          ? {
              path: `/api/forum/forums/${encoded(requireFlag(parsed.flags, 'forum'))}/watches`,
              value: requireFlag(parsed.flags, 'forum'),
            }
          : parsed.command === 'watch-topic'
            ? {
                path: `/api/forum/topics/${encoded(requireFlag(parsed.flags, 'topic'))}/watches`,
                value: requireFlag(parsed.flags, 'topic'),
              }
            : parsed.command === 'bookmark-topic'
              ? {
                  path: `/api/forum/topics/${encoded(requireFlag(parsed.flags, 'topic'))}/bookmarks`,
                  value: requireFlag(parsed.flags, 'topic'),
                }
              : parsed.command === 'bookmark-post'
                ? {
                    path: `/api/forum/posts/${encoded(requireFlag(parsed.flags, 'post'))}/bookmarks`,
                    value: requireFlag(parsed.flags, 'post'),
                  }
                : {
                    path: `/api/forum/actors/${encoded(requireFlag(parsed.flags, 'actor'))}/follows`,
                    value: requireFlag(parsed.flags, 'actor'),
                  }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, parsed.command, {
        target: target.value,
      })

      return authenticatedMutationRequest({
        baseUrl,
        idempotencyKey,
        method: 'POST',
        path: target.path,
        token,
      })
    }
    case 'claim-tip-wallet': {
      requireAgentToken(token, parsed.command)

      const body = {
        bolt12Offer: flagText(parsed.flags, 'bolt12-offer') || null,
        sparkAddress: flagText(parsed.flags, 'spark-address') || null,
        caveatRefs: flagTexts(parsed.flags, 'caveat-ref'),
        claimPolicyRefs: flagTexts(parsed.flags, 'claim-policy-ref'),
        custodyPolicyRefs: flagTexts(parsed.flags, 'custody-policy-ref'),
        payoutTargetApprovalRef:
          flagText(parsed.flags, 'payout-target-approval-ref') || null,
        providerClass:
          flagText(parsed.flags, 'provider-class') || 'mdk_agent_wallet',
        readinessRefs: flagTexts(parsed.flags, 'readiness-ref'),
        receiveCapabilityRef: requireFlag(
          parsed.flags,
          'receive-capability-ref',
        ),
        sourceRef:
          flagText(parsed.flags, 'source-ref') ||
          'source.public.forum_tip_recipient.agent_self_claim',
        walletRef: requireFlag(parsed.flags, 'wallet-ref'),
      }
      const idempotencyKey = idempotencyKeyFor(
        parsed.flags,
        'tip-wallet-claim',
        {
          providerClass: body.providerClass,
          sparkAddress: body.sparkAddress,
          bolt12Offer: body.bolt12Offer,
          readinessRefs: body.readinessRefs,
          receiveCapabilityRef: body.receiveCapabilityRef,
          walletRef: body.walletRef,
        },
      )

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path: '/api/forum/tip-recipient-wallets/claims',
        token,
      })
    }
    case 'paid-preview': {
      requireAgentToken(token, parsed.command)

      const actionKind = requireFlag(parsed.flags, 'action-kind')
      const method = flagText(parsed.flags, 'method') || 'POST'
      const path = requireFlag(parsed.flags, 'path')
      const routeParams = routeParamsFromFlags(parsed.flags)
      const spendCap = spendCapFromFlags(parsed.flags)
      const target = paidTargetFromFlags(parsed.flags)
      const requestBodyDigest = requestBodyDigestFor(
        parsed.flags,
        'paid-preview',
        {
          actionKind,
          method,
          path,
          routeParams,
          spendCap,
          target,
        },
      )
      const body = {
        actionKind,
        method,
        path,
        requestBodyDigest,
        routeParams,
        spendCap,
        target,
      }
      const idempotencyKey = idempotencyKeyFor(
        parsed.flags,
        'paid-preview',
        body,
      )

      return authenticatedMutationRequest({
        baseUrl,
        body,
        idempotencyKey,
        method: 'POST',
        path: '/api/forum/paid-actions/preview',
        token,
      })
    }
    case 'redeem-paid-action': {
      requireAgentToken(token, parsed.command)

      const body = {
        challengeId: requireFlag(parsed.flags, 'challenge'),
        l402ProofRef: requireFlag(parsed.flags, 'l402-proof-ref'),
        method: flagText(parsed.flags, 'method') || 'POST',
        path: requireFlag(parsed.flags, 'path'),
        requestBodyDigest: requireFlag(parsed.flags, 'request-body-digest'),
        routeParams: routeParamsFromFlags(parsed.flags),
      }
      const idempotencyKey = idempotencyKeyFor(parsed.flags, 'paid-redeem', {
        challengeId: body.challengeId,
        path: body.path,
        requestBodyDigest: body.requestBodyDigest,
        routeParams: body.routeParams,
      })
      const l402CredentialHeader = flagText(
        parsed.flags,
        'l402-credential-header',
      )

      return authenticatedMutationRequest({
        baseUrl,
        body,
        extraHeaders:
          l402CredentialHeader === undefined ||
          l402CredentialHeader.trim() === ''
            ? {}
            : { 'x-openagents-l402': l402CredentialHeader },
        idempotencyKey,
        method: 'POST',
        path: '/api/forum/paid-actions/redeem',
        token,
      })
    }
    default:
      break
  }

  const paidAliasConfig = paidAliasConfigForCommand(
    parsed.command,
    parsed.flags,
  )

  if (paidAliasConfig !== undefined) {
    requireAgentToken(token, parsed.command)

    const spendCap = spendCapFromFlags(parsed.flags)
    const rewardAmount =
      paidAliasConfig.actionKind === 'post_reward'
        ? optionalSatsAmountFromFlags(parsed.flags, 'reward-amount')
        : undefined
    const requestBodyDigest = requestBodyDigestFor(
      parsed.flags,
      parsed.command,
      {
        actionKind: paidAliasConfig.actionKind,
        ...(rewardAmount === undefined ? {} : { amount: rewardAmount }),
        spendCap,
        ...paidAliasConfig.parts,
      },
    )
    const body = {
      ...(rewardAmount === undefined ? {} : { amount: rewardAmount }),
      requestBodyDigest,
      spendCap,
    }
    const idempotencyKey = idempotencyKeyFor(parsed.flags, parsed.command, {
      body,
      ...paidAliasConfig.parts,
    })

    return authenticatedMutationRequest({
      baseUrl,
      body,
      idempotencyKey,
      method: 'POST',
      path: paidAliasConfig.path,
      token,
    })
  }

  throw new Error(`Unknown command: ${parsed.command}`)
}

export const safeRequestSummary = request => ({
  baseUrl: request.baseUrl,
  body: redactedBody(request.body),
  headers: Object.fromEntries(
    Object.entries(request.headers || {}).map(([key, value]) => [
      key,
      key.toLowerCase() === 'authorization'
        ? 'Bearer <redacted>'
        : key.toLowerCase() === 'x-openagents-l402'
          ? '<redacted>'
          : value,
    ]),
  ),
  idempotencyKey: request.idempotencyKey,
  method: request.method,
  path: request.path,
})

const jsonRequest = async (request, fetchFn = fetch) => {
  const response = await fetchFn(new URL(request.path, request.baseUrl), {
    body: request.body === undefined ? undefined : JSON.stringify(request.body),
    headers: {
      accept: 'application/json',
      ...(request.headers || {}),
    },
    method: request.method,
  })
  const text = await response.text()
  const body = text === '' ? {} : JSON.parse(text)

  if (!response.ok) {
    const reason = body.reason || body.error || response.statusText
    const summary = JSON.stringify(safeRequestSummary(request))
    throw new Error(
      redactSecrets(
        `${request.method} ${request.path} failed: ${response.status} ${reason}; request=${summary}`,
      ),
    )
  }

  return body
}

const liveSpendApproved = (flags, env) =>
  flags.get('approve-live-spend') === true ||
  env.OPENAGENTS_FORUM_APPROVE_LIVE_SPEND === '1'

const publicChallengeFromPreview = preview => {
  const challenge = preview?.challenge
  const l402 = challenge?.l402

  if (challenge === null || typeof challenge !== 'object') {
    return null
  }

  return {
    challengeId:
      typeof challenge.challengeId === 'string' ? challenge.challengeId : null,
    environment:
      l402 !== null && typeof l402?.environment === 'string'
        ? l402.environment
        : null,
    provider:
      l402 !== null && typeof l402?.provider === 'string'
        ? l402.provider
        : null,
    sandbox:
      l402 === null || typeof l402?.sandbox !== 'boolean' ? null : l402.sandbox,
  }
}

const publicProofRefIsSafe = value =>
  typeof value === 'string' &&
  /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/.test(value) &&
  !/(lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?preimage|preimage|raw[_-]?invoice|secret|token)/i.test(
    value,
  )

export const runForumRewardPostPayment = async (
  parsed,
  env = process.env,
  options = {},
) => {
  requireAgentToken(env.OPENAGENTS_AGENT_TOKEN, parsed.command)

  const timeoutMs = walletTimeoutMsFromFlags(parsed.flags)
  const walletExecutor =
    options.walletExecutor || createAgentWalletExecutor({ timeoutMs })
  const requestJson =
    options.requestJson || (request => jsonRequest(request, options.fetch))
  const spendCap = spendCapFromFlags(parsed.flags)
  const preflight = await runForumWalletPreflight({
    executor: walletExecutor,
    spendCap,
    timeoutMs,
    walletNetwork: walletNetworkFromFlags(parsed.flags, env),
  })

  if (!preflight.ready) {
    return paidRewardResult({
      preflight,
      reasonRef: preflight.blocker?.reasonRef ?? 'reason.public.wallet_blocked',
      status: 'blocked',
    })
  }

  const previewRequest = await buildForumRequest(
    {
      command: 'reward-post',
      flags: parsed.flags,
    },
    env,
  )
  const preview = await requestJson(previewRequest)
  const publicChallenge = publicChallengeFromPreview(preview)

  if (preview?.paymentRequired === false || preview?.challenge === null) {
    return paidRewardResult({
      challenge: publicChallenge,
      preflight,
      reasonRef: 'reason.public.forum_reward_payment_not_required',
      status: 'blocked',
    })
  }

  if (publicChallenge?.sandbox === true) {
    return paidRewardResult({
      challenge: publicChallenge,
      preflight,
      reasonRef: 'reason.public.forum_reward_sandbox_no_spend',
      status: 'blocked',
    })
  }

  if (!liveSpendApproved(parsed.flags, env)) {
    return paidRewardResult({
      challenge: publicChallenge,
      preflight,
      reasonRef: 'reason.public.forum_reward_live_spend_not_approved',
      status: 'blocked',
    })
  }

  const challengeId = preview.challenge.challengeId

  if (typeof challengeId !== 'string' || challengeId.trim() === '') {
    return paidRewardResult({
      challenge: publicChallenge,
      preflight,
      reasonRef: 'reason.public.forum_reward_challenge_id_missing',
      status: 'blocked',
    })
  }

  const privatePaymentRequest = authenticatedMutationRequest({
    baseUrl: previewRequest.baseUrl,
    body: {
      challengeId,
      method: 'POST',
      path: previewRequest.path,
      requestBodyDigest: previewRequest.body.requestBodyDigest,
      routeParams: {
        postId: requireFlag(parsed.flags, 'post'),
      },
      spendCap,
    },
    idempotencyKey: stableIdempotencyKey('paid-private-payment', {
      challengeId,
      path: previewRequest.path,
      requestBodyDigest: previewRequest.body.requestBodyDigest,
    }),
    method: 'POST',
    path: '/api/forum/paid-actions/private-payment',
    token: env.OPENAGENTS_AGENT_TOKEN,
  })
  const privatePaymentEnvelope = await requestJson(privatePaymentRequest)
  const privatePayment = privateL402PaymentFromPreview(privatePaymentEnvelope)

  if (privatePayment === null) {
    return paidRewardResult({
      challenge: publicChallenge,
      preflight,
      reasonRef: 'reason.public.forum_reward_private_l402_payload_missing',
      status: 'blocked',
    })
  }

  const walletPayment = await runAgentWalletSendPayment({
    destination: privatePayment.invoice,
    executor: walletExecutor,
  })

  if (walletPayment.status !== 'paid') {
    const walletReasonRef =
      walletPayment.blocker?.reasonRef ??
      'reason.public.agent_wallet_send_failed'
    const timedOut =
      walletReasonRef === 'reason.public.agent_wallet_send_timeout'

    if (timedOut) {
      const recoveredEnvelope = await requestJson(privatePaymentRequest).catch(
        () => null,
      )
      const recoveredCredential =
        privateL402CredentialFromPreview(recoveredEnvelope)

      if (recoveredCredential !== null) {
        const recovered = await confirmForumRewardPayment({
          challengeId,
          env,
          parsed,
          previewRequest,
          privatePayment: {
            credential: recoveredCredential.credential,
            invoice: privatePayment.invoice,
            proofRef: recoveredCredential.proofRef,
          },
          requestJson,
        })

        return paidRewardResult({
          challenge: publicChallenge,
          livePaymentAttempted: true,
          payment: {
            commandRef: 'mdk_agent_wallet.send',
            credentialPresent: true,
            preimageCaptured: false,
            proofRef: recovered.proofRef,
            recoveredAfterTimeout: true,
            status: 'paid',
            walletPaymentRef: 'wallet_payment.public.mdk_agent_wallet.redacted',
          },
          preflight,
          receipt: recovered.receipt,
          status: 'receipt_created',
        })
      }
    }

    return paidRewardResult({
      challenge: publicChallenge,
      livePaymentAttempted: true,
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        reasonRef: walletReasonRef,
        status: walletPayment.status,
      },
      preflight,
      reasonRef: walletReasonRef,
      status: 'payment_failed',
    })
  }

  const confirmed = await confirmForumRewardPayment({
    challengeId,
    env,
    parsed,
    previewRequest,
    privatePayment,
    requestJson,
  })

  return paidRewardResult({
    challenge: publicChallenge,
    livePaymentAttempted: true,
    payment: {
      commandRef: 'mdk_agent_wallet.send',
      credentialPresent: privatePayment.credential !== null,
      preimageCaptured:
        paymentPreimageFromWalletOutput(walletPayment.parsed) !== undefined,
      proofRef: confirmed.proofRef,
      status: 'paid',
      walletPaymentRef: 'wallet_payment.public.mdk_agent_wallet.redacted',
    },
    preflight,
    receipt: confirmed.receipt,
    status: 'receipt_created',
  })
}

export const runForumDirectTipPostPayment = async (
  parsed,
  env = process.env,
  options = {},
) => {
  requireAgentToken(env.OPENAGENTS_AGENT_TOKEN, parsed.command)

  const post = requireFlag(parsed.flags, 'post')
  const amount = requiredSatsAmountFromFlags(parsed.flags, 'tip-amount')
  const spendCap = directTipSpendCapFromFlags(parsed.flags, amount)
  const timeoutMs = walletTimeoutMsFromFlags(parsed.flags)
  const recoveryWaitMs = recoveryWaitMsFromFlags(parsed.flags)
  const recoveryPollMs = recoveryPollMsFromFlags(parsed.flags)
  const walletNetwork = walletNetworkFromFlags(parsed.flags, env)
  const walletExecutor =
    options.walletExecutor || createAgentWalletExecutor({ timeoutMs })
  const requestJson =
    options.requestJson || (request => jsonRequest(request, options.fetch))
  const postRequest = await buildForumRequest(
    {
      command: 'post',
      flags: parsed.flags,
    },
    env,
  )
  const postDetail = await requestJson(postRequest)
  const directPayment = directPaymentInstructionFromPostDetail(postDetail)
  const target = {
    postId: post,
    postLink: directPayment?.targetPostPermalink ?? null,
    recipientActorRef: directPayment?.recipientActorRef ?? null,
  }

  if (directPayment === null) {
    return directTipResult({
      reasonRef: 'reason.public.forum_tip_recipient_bolt12_offer_missing',
      status: 'blocked',
      target,
    })
  }

  const preflight = await runForumWalletPreflight({
    executor: walletExecutor,
    spendCap,
    timeoutMs,
    walletNetwork,
  })

  if (!preflight.ready) {
    return directTipResult({
      preflight,
      reasonRef: preflight.blocker?.reasonRef ?? 'reason.public.wallet_blocked',
      status: 'blocked',
      target,
    })
  }

  if (!liveSpendApproved(parsed.flags, env)) {
    return directTipResult({
      preflight,
      reasonRef: 'reason.public.forum_tip_live_spend_not_approved',
      status: 'blocked',
      target,
    })
  }

  const payerOfferIdentityRefs = await readPayerOfferIdentityRefs(
    walletExecutor,
  )
  const selfPayShared =
    payerOfferIdentityRefs === null
      ? null
      : (() => {
          const recipientIdentityRefs = bolt12OfferIdentityRefs(
            directPayment.bolt12Offer,
          )

          if (recipientIdentityRefs === null) {
            return null
          }

          for (const identity of recipientIdentityRefs) {
            if (payerOfferIdentityRefs.has(identity)) {
              return true
            }
          }

          return false
        })()

  if (selfPayShared === true) {
    return directTipResult({
      preflight,
      reasonRef: 'reason.public.forum_tip_self_pay_blocked',
      selfPayCheck: 'blocked',
      status: 'self_pay_blocked',
      target,
    })
  }

  const selfPayCheck = selfPayShared === false ? 'passed' : 'inconclusive'

  const walletPayment = await runAgentWalletSendPayment({
    amount: amount.amount,
    destination: directPayment.bolt12Offer,
    executor: walletExecutor,
  })

  if (walletPayment.status !== 'paid') {
    const timedOut =
      walletPayment.blocker?.reasonRef ===
      'reason.public.agent_wallet_send_timeout'
    const paymentIdentifier = walletPaymentIdentifierFromOutput(
      walletPayment.parsed,
    )

    if (timedOut) {
      const recovery = await pollDirectTipPaymentRecovery({
        amountSats: amount.amount,
        executor: walletExecutor,
        paymentIdentifier: paymentIdentifier ?? null,
        pollMs:
          typeof options.recoveryPollMs === 'number'
            ? options.recoveryPollMs
            : recoveryPollMs,
        sleepFn: options.sleep || sleep,
        waitMs: recoveryWaitMs,
      })

      if (recovery.status === 'completed') {
        const evidence = directTipEvidenceFromWalletPayment({
          amount,
          parsed: recovery.payment,
          post,
          status: 'confirmed',
          walletNetwork,
        })
        const recoveredPaymentIdentifier =
          walletPaymentIdentifierFromOutput(recovery.payment) ??
          paymentIdentifier ??
          evidence.externalRef
        const recorded = await submitDirectTipEvidence({
          amount,
          baseUrl: postRequest.baseUrl,
          env,
          evidence,
          idempotencyKey: stableIdempotencyKey('direct-tip-recovered-payment', {
            paymentIdentifier: recoveredPaymentIdentifier,
            post,
          }),
          parsed,
          post,
          requestJson,
        })

        return directTipResult({
          livePaymentAttempted: true,
          payment: {
            commandRef: 'mdk_agent_wallet.send',
            evidenceRef: evidence.redactedEvidenceRef,
            preimageCaptured:
              paymentPreimageFromWalletOutput(recovery.payment) !== undefined,
            recoveredAfterTimeout: true,
            recoveryPolls: recovery.polls,
            status: 'paid',
            walletPaymentRef: 'wallet_payment.public.mdk_agent_wallet.redacted',
          },
          preflight,
          receipt: recorded.receipt ?? null,
          selfPayCheck,
          status: recorded.status === 'settled' ? 'settled' : recorded.status,
          attemptId: recorded.attemptId ?? null,
          target: {
            ...target,
            postLink: recorded.targetPostPermalink ?? target.postLink,
          },
        })
      }

      if (recovery.status === 'failed') {
        return directTipResult({
          livePaymentAttempted: true,
          payment: {
            commandRef: 'mdk_agent_wallet.send',
            reasonRef: 'reason.public.agent_wallet_send_failed',
            recoveredAfterTimeout: true,
            recoveryPolls: recovery.polls,
            status: 'failed',
          },
          preflight,
          reasonRef: 'reason.public.agent_wallet_send_failed',
          receipt: null,
          selfPayCheck,
          status: 'payment_failed',
          target,
        })
      }
    }

    const failureClassification = timedOut
      ? await classifyStalledTipSend(walletExecutor, amount.amount)
      : null
    const evidence = directTipEvidenceFromWalletBlocker({
      amount,
      blocker: walletPayment.blocker,
      post,
      status: timedOut ? 'observed' : 'failed',
      walletNetwork,
    })
    const recorded = await submitDirectTipEvidence({
      amount,
      baseUrl: postRequest.baseUrl,
      env,
      evidence,
      parsed,
      post,
      requestJson,
    }).catch(() => null)

    return directTipResult({
      failureClassification,
      livePaymentAttempted: true,
      payment: {
        commandRef: 'mdk_agent_wallet.send',
        evidenceRef: evidence.redactedEvidenceRef,
        ...(failureClassification === null
          ? {}
          : {
              failureClassification,
              failureClassificationReasonRef: tipFailureClassificationReasonRef(
                failureClassification,
              ),
            }),
        reasonRef:
          walletPayment.blocker?.reasonRef ??
          'reason.public.agent_wallet_send_failed',
        status: timedOut ? 'recovery_pending' : 'failed',
        ...(timedOut
          ? {
              recoveryDeadlineHit: true,
              recoveryWaitMs,
            }
          : {}),
        // #4704: recovery_pending attempts archive after 24h if the
        // provider callback never reconciles them. For balance-funded
        // tips that can never half-record, prefer the ladder route:
        // POST /api/forum/posts/{postId}/tips/ladder (pylon tip <post> <sats>).
      },
      preflight,
      reasonRef:
        walletPayment.blocker?.reasonRef ??
        'reason.public.agent_wallet_send_failed',
      receipt: recorded?.receipt ?? null,
      selfPayCheck,
      status: timedOut ? 'recovery_pending' : 'payment_failed',
      attemptId: recorded?.attemptId ?? null,
      target,
    })
  }

  const evidence = directTipEvidenceFromWalletPayment({
    amount,
    parsed: walletPayment.parsed,
    post,
    status: 'confirmed',
    walletNetwork,
  })
  const recorded = await submitDirectTipEvidence({
    amount,
    baseUrl: postRequest.baseUrl,
    env,
    evidence,
    parsed,
    post,
    requestJson,
  })

  return directTipResult({
    livePaymentAttempted: true,
    payment: {
      commandRef: 'mdk_agent_wallet.send',
      evidenceRef: evidence.redactedEvidenceRef,
      preimageCaptured:
        paymentPreimageFromWalletOutput(walletPayment.parsed) !== undefined,
      status: 'paid',
      walletPaymentRef: 'wallet_payment.public.mdk_agent_wallet.redacted',
    },
    preflight,
    receipt: recorded.receipt ?? null,
    selfPayCheck,
    status: recorded.status === 'settled' ? 'settled' : recorded.status,
    attemptId: recorded.attemptId ?? null,
    target: {
      ...target,
      postLink: recorded.targetPostPermalink ?? target.postLink,
    },
  })
}

const postTipStatsFromDetail = detail => {
  const stats = detail?.post?.tipStats

  if (stats === undefined || stats === null || typeof stats !== 'object') {
    return null
  }

  return {
    tipCount: Number(stats.tipCount) || 0,
    totalPaidSats: Number(stats.totalPaidSats) || 0,
    totalSettledSats: Number(stats.totalSettledSats) || 0,
  }
}

const directTipUsedRecovery = directTip =>
  directTip?.status === 'recovery_pending' ||
  directTip?.payment?.status === 'recovery_pending' ||
  directTip?.payment?.recoveredAfterTimeout === true ||
  directTip?.reasonRef === 'reason.public.agent_wallet_send_timeout' ||
  directTip?.payment?.reasonRef === 'reason.public.agent_wallet_send_timeout'

export const runForumDirectTipPostSmoke = async (
  parsed,
  env = process.env,
  options = {},
) => {
  requireAgentToken(env.OPENAGENTS_AGENT_TOKEN, parsed.command)

  const timeoutMs = walletTimeoutMsFromFlags(parsed.flags)
  const walletExecutor =
    options.walletExecutor || createAgentWalletExecutor({ timeoutMs })
  const requestJson =
    options.requestJson || (request => jsonRequest(request, options.fetch))
  const mode = parsed.flags.get('strict-smooth')
    ? 'strict_smooth'
    : parsed.flags.get('diagnostic')
      ? 'diagnostic'
      : 'diagnostic'
  const balanceBefore = await readAgentWalletBalance(walletExecutor)
  const directTip = await runForumDirectTipPostPayment(parsed, env, {
    ...options,
    requestJson,
    walletExecutor,
  })
  const balanceAfter = await readAgentWalletBalance(walletExecutor)
  const post = requireFlag(parsed.flags, 'post')
  const postRequest = await buildForumRequest(
    {
      command: 'post',
      flags: parsed.flags,
    },
    env,
  )
  const postAfter = await requestJson(postRequest).catch(() => null)
  const recoveredAfterTimeout = directTipUsedRecovery(directTip)
  const strictFailure = mode === 'strict_smooth' && recoveredAfterTimeout
  const failureClassification = directTip.failureClassification ?? null
  const status = strictFailure
    ? 'failed'
    : directTip.status === 'settled'
      ? 'passed'
      : directTip.status

  return tipPostSmokeResult({
    balanceAfter,
    balanceBefore,
    directTip: {
      attemptId: directTip.attemptId ?? null,
      livePaymentAttempted: directTip.livePaymentAttempted,
      paymentStatus: directTip.payment?.status ?? null,
      receiptRef: directTip.receipt?.receiptRef ?? null,
      status: directTip.status,
      target: {
        postId: post,
        postLink: directTip.target?.postLink ?? null,
        recipientActorRef: directTip.target?.recipientActorRef ?? null,
      },
      tipSettlement: directTip.receipt?.tipSettlement ?? null,
    },
    failureClassification,
    mode,
    postStatsAfter: postTipStatsFromDetail(postAfter),
    reasonRef: strictFailure
      ? failureClassification === 'no_invoice_fetched'
        ? 'reason.public.forum_tip_smoke_no_invoice_fetched'
        : failureClassification === 'route_unresolved'
          ? 'reason.public.forum_tip_smoke_route_unresolved'
          : 'reason.public.forum_tip_smoke_recovery_used'
      : directTip.reasonRef,
    recoveredAfterTimeout,
    selfPayCheck: directTip.selfPayCheck ?? null,
    status,
  })
}

export const runForumCli = async (argv, env = process.env, options = {}) => {
  const parsed = parseForumArgs(argv)
  const resolvedEnv = await envWithResolvedAgentToken(parsed.flags, env)

  if (parsed.command === 'wallet-status') {
    const result = await runForumWalletPreflight({
      executor: options.walletExecutor,
      spendCap: spendCapFromFlags(parsed.flags),
      timeoutMs: walletTimeoutMsFromFlags(parsed.flags),
      walletNetwork: walletNetworkFromFlags(parsed.flags, resolvedEnv),
    })

    return `${JSON.stringify(result, null, 2)}\n`
  }

  if (parsed.command === 'pay-reward-post') {
    const result = await runForumRewardPostPayment(parsed, resolvedEnv, options)

    return `${JSON.stringify(result, null, 2)}\n`
  }

  if (parsed.command === 'tip-post') {
    const result = await runForumDirectTipPostPayment(
      parsed,
      resolvedEnv,
      options,
    )

    return `${JSON.stringify(result, null, 2)}\n`
  }

  if (parsed.command === 'tip-post-smoke') {
    const result = await runForumDirectTipPostSmoke(
      parsed,
      resolvedEnv,
      options,
    )

    return `${JSON.stringify(result, null, 2)}\n`
  }

  const request = await buildForumRequest(parsed, resolvedEnv)

  if (request.help) {
    return usage()
  }

  const response = await jsonRequest(request, options.fetch)

  return `${JSON.stringify(response, null, 2)}\n`
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  runForumCli(process.argv.slice(2))
    .then(output => {
      process.stdout.write(output)
    })
    .catch(error => {
      process.stderr.write(
        `${redactSecrets(error instanceof Error ? error.message : String(error))}\n`,
      )
      process.exitCode = 1
    })
}
