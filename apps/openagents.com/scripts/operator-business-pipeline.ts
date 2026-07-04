#!/usr/bin/env bun

type CliArgs = Readonly<{
  baseUrl: string
  command:
    | 'advance'
    | 'approve-outreach-template'
    | 'create'
    | 'grant-credit'
    | 'link-credit-redemption'
    | 'metrics'
    | 'partner-route'
    | 'record-outreach-send'
    | 'render-outreach'
    | 'suppress-outreach'
  flags: ReadonlyMap<string, string>
  token: string
}>

const usage = `Usage:
  bun apps/openagents.com/scripts/operator-business-pipeline.ts metrics
  bun apps/openagents.com/scripts/operator-business-pipeline.ts create --pipeline-ref REF --vertical VERTICAL --source-ref REF --owner-role operator [--receipt-ref REF] [--quoted-min-usd-cents N] [--quoted-max-usd-cents N] [--quoted-band LABEL] [--partner-route true]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts advance --pipeline-ref REF --stage STAGE --receipt-ref REF [--next-action-due-at YYYY-MM-DD]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts partner-route --pipeline-ref REF --state candidate|offered|accepted|declined|none [--peer-ref REF] [--approval-receipt-ref REF] [--offer-ref REF] [--scope-summary-ref REF] [--due-window-ref REF] [--budget-range-ref REF] [--privacy-tier-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts render-outreach --pipeline-ref REF --subject-ref REF --audit-report-ref REF [--finding-refs ref1,ref2] [--observed-fact TEXT] [--template-version-ref REF] [--draft-ref REF] [--source-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts approve-outreach-template --template-version-ref REF --approval-receipt-ref REF --approved-by-ref REF [--source-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts suppress-outreach --subject-ref REF --reason existing_customer --source-ref REF [--suppression-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts record-outreach-send --pipeline-ref REF --draft-ref REF --mailbox-ref REF --source-ref REF [--approval-receipt-ref REF] [--daily-mailbox-send-cap 95] [--send-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts grant-credit --pipeline-ref REF --account-ref agent:REF [--amount-usd-cents 10000] [--grant-ref REF] [--window-ref REF] [--window-grant-cap 25] [--engagement-ref REF]
  bun apps/openagents.com/scripts/operator-business-pipeline.ts link-credit-redemption --pipeline-ref REF --grant-ref REF --redemption-receipt-ref REF

Env:
  OPENAGENTS_ADMIN_API_TOKEN is required.
  OPENAGENTS_BASE_URL defaults to https://openagents.com.`

const parseArgs = (argv: ReadonlyArray<string>): CliArgs => {
  const [command, ...rest] = argv
  if (
    command !== 'create' &&
    command !== 'advance' &&
    command !== 'approve-outreach-template' &&
    command !== 'grant-credit' &&
    command !== 'link-credit-redemption' &&
    command !== 'metrics' &&
    command !== 'partner-route' &&
    command !== 'record-outreach-send' &&
    command !== 'render-outreach' &&
    command !== 'suppress-outreach'
  ) {
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

const optionalRefs = (
  flags: ReadonlyMap<string, string>,
  key: string,
): ReadonlyArray<string> =>
  flags
    .get(key)
    ?.split(',')
    .map(value => value.trim())
    .filter(Boolean) ?? []

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

  if (args.command === 'partner-route') {
    console.log(
      JSON.stringify(
        await requestJson(
          args,
          `/api/operator/business/pipeline/${encodeURIComponent(
            required(args.flags, 'pipeline-ref'),
          )}/partner-route`,
          {
            body: JSON.stringify({
              approvalReceiptRef: args.flags.get('approval-receipt-ref'),
              budgetRangeRef: args.flags.get('budget-range-ref'),
              dueWindowRef: args.flags.get('due-window-ref'),
              offerRef: args.flags.get('offer-ref'),
              peerRef: args.flags.get('peer-ref'),
              privacyTierRef: args.flags.get('privacy-tier-ref'),
              scopeSummaryRef: args.flags.get('scope-summary-ref'),
              state: required(args.flags, 'state'),
            }),
            method: 'POST',
          },
        ),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'render-outreach') {
    console.log(
      JSON.stringify(
        await requestJson(
          args,
          `/api/operator/business/pipeline/${encodeURIComponent(
            required(args.flags, 'pipeline-ref'),
          )}/outreach-drafts`,
          {
            body: JSON.stringify({
              auditReportRef: required(args.flags, 'audit-report-ref'),
              draftRef: args.flags.get('draft-ref'),
              findingRefs: optionalRefs(args.flags, 'finding-refs'),
              observedFact: args.flags.get('observed-fact'),
              sourceRef: args.flags.get('source-ref'),
              subjectRef: required(args.flags, 'subject-ref'),
              templateVersionRef: args.flags.get('template-version-ref'),
            }),
            method: 'POST',
          },
        ),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'approve-outreach-template') {
    console.log(
      JSON.stringify(
        await requestJson(args, '/api/operator/business/outreach/template-approvals', {
          body: JSON.stringify({
            approvalReceiptRef: required(args.flags, 'approval-receipt-ref'),
            approvedByRef: required(args.flags, 'approved-by-ref'),
            sourceRef: args.flags.get('source-ref'),
            templateVersionRef: required(args.flags, 'template-version-ref'),
          }),
          method: 'POST',
        }),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'suppress-outreach') {
    console.log(
      JSON.stringify(
        await requestJson(args, '/api/operator/business/outreach/suppressions', {
          body: JSON.stringify({
            reason: required(args.flags, 'reason'),
            sourceRef: required(args.flags, 'source-ref'),
            subjectRef: required(args.flags, 'subject-ref'),
            suppressionRef: args.flags.get('suppression-ref'),
          }),
          method: 'POST',
        }),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'record-outreach-send') {
    console.log(
      JSON.stringify(
        await requestJson(
          args,
          `/api/operator/business/pipeline/${encodeURIComponent(
            required(args.flags, 'pipeline-ref'),
          )}/outreach-sends`,
          {
            body: JSON.stringify({
              approvalReceiptRef: args.flags.get('approval-receipt-ref'),
              dailyMailboxSendCap: optionalNumber(args.flags, 'daily-mailbox-send-cap'),
              draftRef: required(args.flags, 'draft-ref'),
              mailboxRef: required(args.flags, 'mailbox-ref'),
              sendRef: args.flags.get('send-ref'),
              sourceRef: required(args.flags, 'source-ref'),
            }),
            method: 'POST',
          },
        ),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'grant-credit') {
    console.log(
      JSON.stringify(
        await requestJson(
          args,
          `/api/operator/business/pipeline/${encodeURIComponent(
            required(args.flags, 'pipeline-ref'),
          )}/starter-credit-grants`,
          {
            body: JSON.stringify({
              accountRef: required(args.flags, 'account-ref'),
              amountUsdCents: optionalNumber(args.flags, 'amount-usd-cents'),
              engagementRef: args.flags.get('engagement-ref'),
              grantRef: args.flags.get('grant-ref'),
              windowGrantCap: optionalNumber(args.flags, 'window-grant-cap'),
              windowRef: args.flags.get('window-ref'),
            }),
            method: 'POST',
          },
        ),
        null,
        2,
      ),
    )
    return
  }

  if (args.command === 'link-credit-redemption') {
    console.log(
      JSON.stringify(
        await requestJson(
          args,
          `/api/operator/business/pipeline/${encodeURIComponent(
            required(args.flags, 'pipeline-ref'),
          )}/starter-credit-redemptions`,
          {
            body: JSON.stringify({
              grantRef: required(args.flags, 'grant-ref'),
              redemptionReceiptRef: required(args.flags, 'redemption-receipt-ref'),
            }),
            method: 'POST',
          },
        ),
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
