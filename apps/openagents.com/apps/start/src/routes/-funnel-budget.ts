import { readdir, stat } from 'node:fs/promises'
import { join } from 'node:path'

type Budget = Readonly<{
  actual: number
  budget: number
  label: string
}>

const KiB = 1024

const routeBudgets = [
  { path: '/', marker: 'OpenAgents' },
  { path: '/activity', marker: 'Live public activity' },
  { path: '/business', marker: 'Agents that work.' },
  { path: '/docs', marker: 'OpenAgents docs' },
  { path: '/docs/api', marker: 'Developer API' },
  { path: '/blog', marker: 'OpenAgents Blog' },
  { path: '/blog/introducing-khala-code', marker: 'Introducing Khala Code' },
  { path: '/clients-preview', marker: 'Autopilot control surface' },
  { path: '/components', marker: 'Component library' },
  { path: '/code/download', marker: 'Install paths, with the Codex requirement kept visible' },
  { path: '/autopilot/legal', marker: 'For legal teams' },
  { path: '/gym', marker: 'OpenAgents Gym' },
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
      budget: 760 * KiB,
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
