// PORTAL-1 (#8652): seed the first demo client-portal engagement through the
// admin seam (portal-routes.ts admin-bearer routes), so /portal demos
// non-empty immediately.
//
// The fixture is a GENERIC strategic-consulting / business-formation demo:
// agent-authored, professional tone, no client names (public repo).
//
// Usage (admin bearer required; never print the token):
//   OPENAGENTS_ADMIN_API_TOKEN=... node --import tsx scripts/seed-portal-demo.ts \
//     --base-url https://openagents.com \
//     [--client-email client@example.com]   # bind a real client identity
//     [--engagement <engagementId>]         # seed into an existing engagement
//     [--status active|preparing]           # default: active
//
// Binding a real client later (operator runbook):
//   curl -sS -X POST "$BASE/api/portal/admin/engagements/<id>/bind" \
//     -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
//     -H 'content-type: application/json' \
//     -d '{"clientEmail":"client@example.com"}'
// The client then logs into openagents.com with that email (or GitHub account
// whose session email matches); their first /portal visit pins their user id.
//
// BIND BY THE IDENTITY THE CLIENT ACTUALLY SIGNS IN WITH (#8652 reopen).
// An email binding only matches if it EXACTLY equals the client's session
// email (their GitHub primary email for GitHub logins). Guessing wrong ships
// the client an empty portal. When the client already has an account, prefer
// the authoritative user-id bind (verify the prod `users.id` first):
//   curl -sS -X POST "$BASE/api/portal/admin/engagements/<id>/bind" \
//     -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
//     -H 'content-type: application/json' \
//     -d '{"clientUserId":"github:<id>"}'
// After ANY bind, run the real-browser gate before owner/client handoff:
//   scripts/portal-browser-smoke.ts (docs/DEPLOYMENT.md "Portal real-browser
//   smoke").

type SeedItem = Readonly<{
  kind: 'post'
  channel: string
  variant: 'a' | 'b'
  pairRef: string
  title: string
  body: string
}>

export const PORTAL_DEMO_ENGAGEMENT_NAME =
  'Strategic Consulting — Business Formation (Demo)'

export const PORTAL_DEMO_CONTENT_ITEMS: ReadonlyArray<SeedItem> = [
  {
    kind: 'post',
    channel: 'linkedin',
    variant: 'a',
    pairRef: 'week1-entity-choice',
    title: 'LLC or C-corp? Decide from your funding plan, not a template',
    body: 'Most founders pick an entity by copying whatever their last company used. The better question: who funds you in the next 24 months? Bootstrapped services business — an LLC keeps taxes simple and distributions flexible. Raising institutional capital — investors will expect a Delaware C-corp with a standard equity structure. Choosing wrong is fixable, but conversions cost legal fees and time you could spend selling. Decide from the funding plan first; the paperwork follows.',
  },
  {
    kind: 'post',
    channel: 'linkedin',
    variant: 'b',
    pairRef: 'week1-entity-choice',
    title: 'The 3-question entity test we run with every new founder',
    body: 'Before any formation paperwork, we ask three questions. 1) Will outside investors own equity in the next two years? 2) Do you need to retain earnings in the company, or distribute them? 3) How many owners, and in how many states? The answers sort almost every new business cleanly into LLC or C-corp territory — and they surface the handful of cases that genuinely need a specialist. Formation is a strategy decision wearing legal clothes.',
  },
  {
    kind: 'post',
    channel: 'linkedin',
    variant: 'a',
    pairRef: 'week2-compliance-floor',
    title: 'The compliance floor: five filings new businesses actually miss',
    body: 'New owners obsess over the formation certificate and then miss the boring floor underneath it: the EIN, the registered agent that must stay current, the annual report your state quietly requires, the sales-tax registration that triggers on your first taxable sale, and the ownership-information filing many businesses now owe. None of these are hard. All of them have deadlines with penalties. A one-page compliance calendar in week one is the cheapest insurance a new company can buy.',
  },
  {
    kind: 'post',
    channel: 'linkedin',
    variant: 'b',
    pairRef: 'week2-compliance-floor',
    title: 'Formation is day one. The compliance calendar is the business.',
    body: 'Filing the entity is the easy part — a few hundred dollars and an afternoon. What separates companies that stay clean from companies that pay penalties is the calendar that follows: annual reports, franchise taxes, registered-agent renewals, payroll registrations as you hire, and license renewals by locality. We build that calendar with owners in the first week, because retrofitting compliance after a missed deadline always costs more than maintaining it.',
  },
]

const parseArgs = (argv: ReadonlyArray<string>) => {
  const args: Record<string, string> = {}
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    if (key?.startsWith('--')) {
      const value = argv[index + 1]
      if (value !== undefined && !value.startsWith('--')) {
        args[key.slice(2)] = value
        index += 1
      } else {
        args[key.slice(2)] = 'true'
      }
    }
  }
  return args
}

const main = async (): Promise<void> => {
  const args = parseArgs(process.argv.slice(2))
  const baseUrl = (args['base-url'] ?? 'https://openagents.com').replace(
    /\/+$/u,
    '',
  )
  const token = process.env.OPENAGENTS_ADMIN_API_TOKEN?.trim()
  if (token === undefined || token === '') {
    console.error('OPENAGENTS_ADMIN_API_TOKEN is required (never print it).')
    process.exit(2)
  }

  const adminFetch = async (path: string, body?: unknown) => {
    const response = await fetch(`${baseUrl}${path}`, {
      method: body === undefined ? 'GET' : 'POST',
      headers: {
        authorization: `Bearer ${token}`,
        ...(body === undefined
          ? {}
          : { 'content-type': 'application/json' }),
        accept: 'application/json',
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })
    const parsed = (await response.json().catch(() => null)) as Record<
      string,
      unknown
    > | null
    if (!response.ok) {
      throw new Error(
        `${path} failed (${response.status}): ${JSON.stringify(parsed)}`,
      )
    }
    return parsed ?? {}
  }

  let engagementId = args['engagement']
  if (engagementId === undefined) {
    const created = await adminFetch('/api/portal/admin/engagements', {
      name: PORTAL_DEMO_ENGAGEMENT_NAME,
      status: args['status'] ?? 'active',
      ...(args['client-email'] !== undefined
        ? { clientEmail: args['client-email'] }
        : {}),
    })
    engagementId = (created.engagement as { id?: string } | undefined)?.id
    if (engagementId === undefined) {
      throw new Error('engagement create did not return an id')
    }
    console.log(`created engagement: ${engagementId}`)
  } else if (args['client-email'] !== undefined) {
    await adminFetch(`/api/portal/admin/engagements/${engagementId}/bind`, {
      clientEmail: args['client-email'],
    })
    console.log(`bound clientEmail to engagement: ${engagementId}`)
  }

  const seeded = await adminFetch(
    `/api/portal/admin/engagements/${engagementId}/content-items`,
    { items: PORTAL_DEMO_CONTENT_ITEMS },
  )
  const items = Array.isArray(seeded.items) ? seeded.items : []
  console.log(`seeded ${items.length} content items (2 A/B pairs)`)

  const verify = await adminFetch(`/api/portal/admin/engagements/${engagementId}`)
  const verifyItems = Array.isArray(verify.items) ? verify.items : []
  console.log(
    JSON.stringify(
      {
        engagementId,
        name: (verify.engagement as { name?: string } | undefined)?.name,
        status: (verify.engagement as { status?: string } | undefined)?.status,
        itemCount: verifyItems.length,
        itemIds: verifyItems.map(item => (item as { id?: string }).id),
      },
      null,
      2,
    ),
  )
}

if (import.meta.main) {
  main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  })
}
