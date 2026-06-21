#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents.com'
const requiredPromiseRefs = [
  'training.decentralized_training_launch.v1',
  'pylon.install_without_wallet_knowledge.v1',
  'models.tassadar_percepta_executor.v1',
  'training.public_gradient_windows.v1',
  'pylon.first_real_model_training_run.v1',
]

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_BASE_URL || defaultBaseUrl,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]

    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

export const usage = () => `Usage:
  node scripts/tassadar-live-page-smoke.mjs
  node scripts/tassadar-live-page-smoke.mjs --base-url http://localhost:5173

Options:
  --base-url <url>  OpenAgents origin. Defaults to https://openagents.com.

This smoke verifies the public /tassadar route, app assets, retired web-scene
guardrail, live run summary, product-promise gates, and at least one public
proof route when a settlement row exists. It is intentionally dependency-free.
`

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) => {
  if (typeof pathOrUrl !== 'string' || pathOrUrl.length === 0) {
    throw new Error('Expected a non-empty URL or path.')
  }

  return new URL(pathOrUrl, baseUrl).toString()
}

const requestText = async (fetchImpl, baseUrl, path, init = {}) => {
  const url = absoluteUrl(baseUrl, path)
  const response = await fetchImpl(url, {
    ...init,
    headers: {
      accept: 'text/html,application/xhtml+xml',
      ...(init.headers || {}),
    },
  })
  const body = await response.text()

  return { body, response, url }
}

const requestJson = async (fetchImpl, baseUrl, path, init = {}) => {
  const url = absoluteUrl(baseUrl, path)
  const response = await fetchImpl(url, {
    ...init,
    headers: {
      accept: 'application/json',
      ...(init.headers || {}),
    },
  })
  const body = await response.json().catch(() => null)

  return { body, response, url }
}

const okStatus = response => response.status >= 200 && response.status < 300

const scriptSrcsFromHtml = html =>
  [...String(html).matchAll(/<script\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/gi)]
    .map(match => match[1])
    .filter(Boolean)

const addCheck = (checks, name, passed, details = {}) => {
  checks.push({
    details,
    name,
    passed,
  })
}

const assertCheck = (checks, name, condition, details = {}) => {
  addCheck(checks, name, Boolean(condition), details)
  assert(condition, `${name} failed`)
}

const promiseRecordsFrom = body => {
  if (Array.isArray(body?.promises)) {
    return body.promises
  }

  if (body?.promises && typeof body.promises === 'object') {
    return Object.values(body.promises)
  }

  if (Array.isArray(body?.records)) {
    return body.records
  }

  return []
}

const firstProofUrlFrom = summary => {
  const rows = Array.isArray(summary?.settlementRows)
    ? summary.settlementRows
    : []
  const withApiUrl = rows.find(
    row => typeof row?.apiUrl === 'string' && row.apiUrl.length > 0,
  )

  return withApiUrl?.apiUrl ?? null
}

export const runTassadarLivePageSmoke = async ({
  baseUrl = defaultBaseUrl,
  fetchImpl = globalThis.fetch,
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')

  const origin = trimBaseUrl(baseUrl)
  const checks = []
  const page = await requestText(fetchImpl, origin, '/tassadar')

  assertCheck(checks, 'tassadar_route_200', okStatus(page.response), {
    status: page.response.status,
    url: page.url,
  })

  const assetSrcs = scriptSrcsFromHtml(page.body)
  assertCheck(
    checks,
    'tassadar_route_has_script_assets',
    assetSrcs.length > 0,
    {
      assetCount: assetSrcs.length,
    },
  )

  const assets = []
  const assetBodies = []
  for (const src of assetSrcs) {
    const asset = await requestText(fetchImpl, origin, src, {
      headers: { accept: '*/*' },
    })
    const passed = okStatus(asset.response) && asset.body.length > 0
    addCheck(checks, 'script_asset_reachable', passed, {
      bytes: asset.body.length,
      src,
      status: asset.response.status,
      url: asset.url,
    })
    assert(passed, `script_asset_reachable failed for ${src}`)
    assets.push({
      bytes: asset.body.length,
      src,
      status: asset.response.status,
      url: asset.url,
    })
    assetBodies.push(asset.body)
  }

  const appAssetText = assetBodies.join('\n')
  assertCheck(
    checks,
    'tassadar_web_scene_retired',
    appAssetText.includes('Tassadar lives in the Verse') &&
      !appAssetText.includes('oa-tassadar-run'),
    {
      hasRetiredCopy: appAssetText.includes('Tassadar lives in the Verse'),
      hasLegacyElement: appAssetText.includes('oa-tassadar-run'),
    },
  )

  const summaryResult = await requestJson(
    fetchImpl,
    origin,
    '/api/public/tassadar-run-summary',
  )
  const summary = summaryResult.body

  assertCheck(
    checks,
    'summary_endpoint_200',
    okStatus(summaryResult.response),
    {
      status: summaryResult.response.status,
      url: summaryResult.url,
    },
  )
  assertCheck(
    checks,
    'summary_has_run_ref',
    typeof summary?.runRef === 'string',
    {
      runRef: summary?.runRef,
    },
  )
  assertCheck(
    checks,
    'summary_has_generated_at',
    typeof summary?.generatedAt === 'string' && summary.generatedAt.length > 0,
    { generatedAt: summary?.generatedAt },
  )
  assertCheck(
    checks,
    'summary_has_staleness_contract',
    typeof summary?.staleness?.contractVersion === 'string' &&
      typeof summary?.staleness?.composition === 'string' &&
      typeof summary?.staleness?.maxStalenessSeconds === 'number',
    { staleness: summary?.staleness },
  )
  assertCheck(
    checks,
    'summary_has_typed_settlement_rows',
    Array.isArray(summary?.settlementRows),
    { settlementRowCount: summary?.settlementRows?.length ?? 0 },
  )
  assertCheck(
    checks,
    'summary_has_rejected_replay_projection',
    Array.isArray(summary?.realGradient?.rejectedReplayPairs),
    {
      rejectedReplayPairCount:
        summary?.realGradient?.rejectedReplayPairs?.length ?? 0,
    },
  )

  const pylonStatsResult = await requestJson(
    fetchImpl,
    origin,
    '/api/public/pylon-stats',
  )
  const pylonStats = pylonStatsResult.body

  assertCheck(
    checks,
    'pylon_stats_endpoint_200',
    okStatus(pylonStatsResult.response),
    {
      status: pylonStatsResult.response.status,
      url: pylonStatsResult.url,
    },
  )
  assertCheck(
    checks,
    'pylon_stats_context_fields_present',
    typeof pylonStats?.pylonsOnlineNow === 'number' &&
      typeof pylonStats?.pylonsAssignmentReadyNow === 'number' &&
      typeof pylonStats?.trainingAcceptedContributors === 'number' &&
      typeof pylonStats?.trainingModelProgressContributors === 'number',
    {
      pylonsOnlineNow: pylonStats?.pylonsOnlineNow,
      trainingAcceptedContributors: pylonStats?.trainingAcceptedContributors,
      trainingModelProgressContributors:
        pylonStats?.trainingModelProgressContributors,
    },
  )

  const promisesResult = await requestJson(
    fetchImpl,
    origin,
    '/api/public/product-promises',
  )
  const promiseRecords = promiseRecordsFrom(promisesResult.body)
  const promiseRefs = new Set(
    promiseRecords
      .map(
        record =>
          record?.promiseId || record?.promiseRef || record?.id || record?.ref,
      )
      .filter(Boolean),
  )

  assertCheck(
    checks,
    'product_promises_endpoint_200',
    okStatus(promisesResult.response),
    {
      status: promisesResult.response.status,
      url: promisesResult.url,
    },
  )
  assertCheck(
    checks,
    'product_promise_gate_refs_present',
    requiredPromiseRefs.every(ref => promiseRefs.has(ref)),
    {
      missingPromiseRefs: requiredPromiseRefs.filter(
        ref => !promiseRefs.has(ref),
      ),
      promiseRefCount: promiseRefs.size,
    },
  )

  const proofUrl = firstProofUrlFrom(summary)
  let proof = null

  if (proofUrl) {
    const proofResult = await requestJson(fetchImpl, origin, proofUrl)
    const passed = okStatus(proofResult.response)
    addCheck(checks, 'first_settlement_proof_route_200', passed, {
      status: proofResult.response.status,
      url: proofResult.url,
    })
    assert(passed, `first_settlement_proof_route_200 failed for ${proofUrl}`)
    proof = {
      status: proofResult.response.status,
      url: proofResult.url,
    }
  } else {
    addCheck(checks, 'first_settlement_proof_route_200', true, {
      skipped: true,
      reason: 'No settlement row with apiUrl exists in this projection.',
    })
  }

  const output = {
    assets,
    baseUrl: origin,
    checks,
    ok: checks.every(check => check.passed),
    pylonStats: {
      asOfLabel: pylonStats?.asOfLabel ?? null,
      publicRealSatsSettled24h: pylonStats?.publicRealSatsSettled24h ?? null,
      pylonsOnlineNow: pylonStats?.pylonsOnlineNow ?? null,
      trainingAcceptedContributors:
        pylonStats?.trainingAcceptedContributors ?? null,
      trainingModelProgressContributors:
        pylonStats?.trainingModelProgressContributors ?? null,
    },
    proof,
    run: {
      generatedAt: summary?.generatedAt ?? null,
      runRef: summary?.runRef ?? null,
      runState: summary?.runState ?? null,
      settlementRowCount: summary?.settlementRows?.length ?? 0,
      staleness: summary?.staleness ?? null,
    },
  }

  return output
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))

  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runTassadarLivePageSmoke({
    baseUrl: options.baseUrl,
  })

  console.log(JSON.stringify(output, null, 2))
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  })
}
