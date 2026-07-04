#!/usr/bin/env bun

type CliArgs = Readonly<{
  baseUrl: string
  command: 'advance' | 'create' | 'metrics'
  flags: ReadonlyMap<string, string>
  token: string
}>

const usage = `Usage:
  bun apps/openagents.com/scripts/operator-business-pipeline.ts metrics
  bun apps/openagents.com/scripts/operator-business-pipeline.ts create --pipeline-ref REF --vertical VERTICAL --source-ref REF --owner-role operator [--receipt-ref REF] [--quoted-min-usd-cents N] [--quoted-max-usd-cents N] [--quoted-band LABEL] [--partner-route true]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts advance --pipeline-ref REF --stage STAGE --receipt-ref REF [--next-action-due-at YYYY-MM-DD]

Env:
  OPENAGENTS_ADMIN_API_TOKEN is required.
  OPENAGENTS_BASE_URL defaults to https://openagents.com.`

const parseArgs = (argv: ReadonlyArray<string>): CliArgs => {
  const [command, ...rest] = argv
  if (command !== 'create' && command !== 'advance' && command !== 'metrics') {
    throw new Error(usage)
  }

  const flags = new Map<string, string>()
  for (let index = 0; index < rest.length; index += 1) {
    const flag = rest[index]
    const value = rest[index + 1]
    if (flag === undefined || !flag.startsWith('--') || value === undefined) {
      throw new Error(usage)
    }
    flags.set(flag.slice(2), value)
    index += 1
  }

  const token = process.env.OPENAGENTS_ADMIN_API_TOKEN
  if (token === undefined || token.trim() === '') {
    throw new Error('OPENAGENTS_ADMIN_API_TOKEN is required.')
  }

  return {
    baseUrl: process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com',
    command,
    flags,
    token,
  }
}

const required = (flags: ReadonlyMap<string, string>, key: string): string => {
  const value = flags.get(key)
  if (value === undefined || value.trim() === '') {
    throw new Error(`--${key} is required.`)
  }
  return value
}

const optionalNumber = (
  flags: ReadonlyMap<string, string>,
  key: string,
): number | undefined => {
  const value = flags.get(key)
  if (value === undefined) return undefined
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) throw new Error(`--${key} must be numeric.`)
  return parsed
}

const requestJson = async (
  args: CliArgs,
  path: string,
  init: RequestInit = {},
): Promise<unknown> => {
  const response = await fetch(new URL(path, args.baseUrl), {
    ...init,
    headers: {
      authorization: `Bearer ${args.token}`,
      'content-type': 'application/json',
      ...(init.headers ?? {}),
    },
  })
  const body = await response.json().catch(() => ({}))
  if (!response.ok) {
    throw new Error(JSON.stringify(body))
  }
  return body
}

const main = async (): Promise<void> => {
  const args = parseArgs(process.argv.slice(2))

  if (args.command === 'metrics') {
    console.log(
      JSON.stringify(
        await requestJson(args, '/api/operator/business/pipeline/metrics'),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'create') {
    const receiptRef = args.flags.get('receipt-ref')
    console.log(
      JSON.stringify(
        await requestJson(args, '/api/operator/business/pipeline', {
          body: JSON.stringify({
            ownerRole: args.flags.get('owner-role') ?? 'operator',
            partnerRouteFlag: args.flags.get('partner-route') === 'true',
            pipelineRef: required(args.flags, 'pipeline-ref'),
            quotedBandLabel: args.flags.get('quoted-band'),
            quotedMaxUsdCents: optionalNumber(args.flags, 'quoted-max-usd-cents'),
            quotedMinUsdCents: optionalNumber(args.flags, 'quoted-min-usd-cents'),
            receiptRefs: receiptRef === undefined ? [] : [receiptRef],
            sourceRef: required(args.flags, 'source-ref'),
            vertical: required(args.flags, 'vertical'),
          }),
          method: 'POST',
        }),
        null,
        2,
      ),
    )
    return
  }

  console.log(
    JSON.stringify(
      await requestJson(
        args,
        `/api/operator/business/pipeline/${encodeURIComponent(
          required(args.flags, 'pipeline-ref'),
        )}/advance`,
        {
          body: JSON.stringify({
            nextActionDueAt: args.flags.get('next-action-due-at'),
            receiptRef: required(args.flags, 'receipt-ref'),
            stage: required(args.flags, 'stage'),
          }),
          method: 'POST',
        },
      ),
      null,
      2,
    ),
  )
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
