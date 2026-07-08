import { readdir, stat } from 'node:fs/promises'
import { join } from 'node:path'

type Budget = Readonly<{
  actual: number
  budget: number
  label: string
}>

const KiB = 1024

const routeBudgets = [
  { path: '/', marker: 'Launch UI v2 is out!' },
  { path: '/activity', marker: 'Live public activity' },
  { path: '/business', marker: 'Agents that work.' },
  { path: '/business/kpi/engagement.public.vertical_pipeline_1', marker: 'Scorekeeper' },
  { path: '/docs', marker: 'OpenAgents docs' },
  { path: '/docs/api', marker: 'Developer API' },
  { path: '/blog', marker: 'OpenAgents Blog' },
  { path: '/blog/introducing-khala-code', marker: 'Introducing Khala Code' },
  { path: '/clients-preview', marker: 'Autopilot control surface' },
  { path: '/components', marker: 'Component library' },
  { path: '/code', marker: 'Code, on your own capacity' },
  { path: '/code/download', marker: 'Install paths, with the Codex requirement kept visible' },
  { path: '/download', marker: 'Download Autopilot for Mac' },
  { path: '/sites/demo-checkout', marker: 'Demo checkout' },
  {
    path: '/pylon/codex/assignments/assignment.public.khala_coding.chatcmpl_example',
    marker: 'Pylon Codex assignment',
  },
  { path: '/artanis', marker: 'ARTANIS console' },
  { path: '/artanis/traces', marker: 'Artanis execution tree' },
  { path: '/artanis/accounts', marker: 'Operator account observability' },
  { path: '/adjutant', marker: 'Loading public goal.' },
  { path: '/agents/artanis', marker: 'ARTANIS console' },
  { path: '/workspaces/workspace.public.invite_example', marker: 'Open your project workspace' },
  { path: '/autopilot/legal', marker: 'For legal teams' },
  { path: '/gym', marker: 'OpenAgents Gym' },
  { path: '/login', marker: 'Log in to OpenAgents' },
  { path: '/mirrorcode', marker: 'MirrorCode, powered by Khala' },
  { path: '/onboarding', marker: 'Stop Babysitting Your AI' },
  { path: '/preview/landing', marker: 'Software, built by agents.' },
  { path: '/promises', marker: 'Product promises' },
  { path: '/pylons', marker: 'Run a Pylon node' },
  { path: '/run', marker: 'Tassadar lives in the Verse' },
  // Live-fetch route (same posture as `/pylons`): the marker is the honest
  // pre-fetch idle state, since the fixture id below has no real backing
  // share in an isolated preview without the `workers/api` Worker attached.
  {
    path: '/share/123e4567-e89b-42d3-a456-426614174000',
    marker: 'Loading share',
  },
  { path: '/stats', marker: 'Network Stats' },
  { path: '/terms', marker: 'Terms of Service' },
  { path: '/privacy', marker: 'Privacy Policy' },
  // Deprecated-for-now (owner decision, 2026-07-05): both routes render the
  // temporarily-unavailable notice, not the real page. See
  // docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md.
  { path: '/training/runs', marker: 'temporarily unavailable' },
  {
    path: '/training/runs/run.cs336.a1.demo',
    marker: 'temporarily unavailable',
  },
] as const

const assetDir = join(import.meta.dir, '../../dist/client/assets')

const isVendorChunk = (filename: string): boolean =>
  /^(react|tanstack-|icons|rolldown-runtime|start-|router-|index-)/.test(
    filename,
  )

const check = (budget: Budget): string | undefined =>
  budget.actual <= budget.budget
    ? undefined
    : `${budget.label}: ${(budget.actual / KiB).toFixed(1)} KiB > ${(budget.budget / KiB).toFixed(1)} KiB`

const fail = (messages: ReadonlyArray<string>): never => {
  throw new Error(`Start funnel budget failed:\n${messages.join('\n')}`)
}

export async function main(): Promise<void> {
  const files = await readdir(assetDir)
  const jsFiles = files.filter(file => file.endsWith('.js'))
  const sizes = await Promise.all(
    jsFiles.map(async file => ({ file, size: (await stat(join(assetDir, file))).size })),
  )
  const totalJs = sizes.reduce((sum, entry) => sum + entry.size, 0)
  const routeChunks = sizes.filter(entry => !isVendorChunk(entry.file))
  const checks: Budget[] = [
    {
      actual: totalJs,
      budget: 780 * KiB,
      label: 'total client JS across Start funnel routes',
    },
    ...routeChunks.map(entry => ({
      actual: entry.size,
      budget: 120 * KiB,
      label: `route chunk ${entry.file}`,
    })),
  ]
  const failures = checks.map(check).filter((message): message is string => message !== undefined)

  if (failures.length > 0) {
    fail(failures)
  }

  process.stdout.write(
    JSON.stringify(
      {
        schema: 'openagents.start_funnel_route_budget.v1',
        totalClientJsKiB: Number((totalJs / KiB).toFixed(1)),
        routeChunkCount: routeChunks.length,
        routeChunks: routeChunks.map(entry => ({
          file: entry.file,
          rawKiB: Number((entry.size / KiB).toFixed(1)),
        })),
        routes: routeBudgets,
      },
      null,
      2,
    ) + '\n',
  )
}

if (import.meta.main) {
  await main()
}
