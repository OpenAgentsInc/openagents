import { Buffer } from 'node:buffer'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_START_SITE_BUILD_COMMAND,
  AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
  AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
  AUTOPILOT_START_SITE_TEMPLATE_ID,
  AUTOPILOT_START_SITE_WORKER_ENTRY,
  assertAutopilotStartSiteLaneIsPublicSafe,
  createAutopilotStartSiteContainerBuildLane,
  createAutopilotStartSiteTemplate,
} from './sites-start-template'
import { selectSiteBuilderPreviewTier } from './sites-builder-preview-runner'

const dogfoodInput = {
  baseUrl: 'https://sites.openagents.com/openagents-funnel',
  customerSegment: 'teams buying AI-operated software delivery',
  description:
    'OpenAgents turns operator-reviewed AI work into public, verifiable software delivery.',
  primaryActionLabel: 'Book the audit',
  primaryActionPath: '/business',
  secondaryActionLabel: 'Read the agent guide',
  secondaryActionPath: '/llms.txt',
  siteId: 'site_project_openagents_funnel',
  slug: 'openagents-funnel',
  title: 'OpenAgents AI operations funnel',
  vertical: 'AI operations',
} as const

const fileByPath = (path: string) => {
  const template = createAutopilotStartSiteTemplate(dogfoodInput)
  const file = template.files.find(item => item.path === path)

  if (file === undefined) {
    throw new Error(`Missing template file ${path}`)
  }

  return file.text
}

describe('Autopilot Start Site template v1', () => {
  test('ships a canonical TanStack Start Worker shape with agent-ready surfaces', () => {
    const template = createAutopilotStartSiteTemplate(dogfoodInput)
    const paths = template.files.map(file => file.path)
    const pkg = JSON.parse(fileByPath('package.json')) as {
      dependencies: Record<string, string>
      devDependencies: Record<string, string>
      packageManager: string
      scripts: Record<string, string>
    }

    expect(template).toMatchObject({
      buildCommand: AUTOPILOT_START_SITE_BUILD_COMMAND,
      dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
      runtimeKind: 'workers_for_platforms',
      templateId: AUTOPILOT_START_SITE_TEMPLATE_ID,
      workerModulePath: AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
      wranglerMain: AUTOPILOT_START_SITE_WORKER_ENTRY,
    })
    expect(paths).toEqual(
      expect.arrayContaining([
        'AGENTS.md',
        'bun.lock',
        'package.json',
        'src/routes/-server.ts',
        'src/routes/__root.tsx',
        'src/routes/index.tsx',
        'src/server.ts',
        'src/styles.css',
        'tsr.config.json',
        'vite.config.ts',
        'wrangler.jsonc',
      ]),
    )
    expect(pkg.packageManager).toBe('bun@1.3.11')
    expect(pkg.scripts.build).toBe('vite build --logLevel warn')
    expect(pkg.dependencies['@tanstack/react-start']).toBe('1.168.26')
    expect(pkg.dependencies['@tanstack/react-router']).toBe('1.170.16')
    expect(pkg.dependencies['@openagentsinc/ui']).toBe('0.1.0')
    expect(pkg.devDependencies.tailwindcss).toBe('4.2.2')
    expect(fileByPath('wrangler.jsonc')).toContain(
      `"main": "${AUTOPILOT_START_SITE_WORKER_ENTRY}"`,
    )
    expect(fileByPath('src/routes/-server.ts')).toContain('createServerFn')
    expect(fileByPath('src/server.ts')).toContain('/.well-known/openagents.json')
    expect(fileByPath('src/server.ts')).toContain('/llms.txt')
    expect(fileByPath('src/server.ts')).toContain('/sitemap.xml')
    expect(fileByPath('src/routes/__root.tsx')).toContain(
      'application/ld+json',
    )
    expect(fileByPath('src/styles.css')).toContain(
      "@import '@openagentsinc/ui/react.css'",
    )
  })

  test('plans the metered Container build and WfP deployment handoff', () => {
    const lane = createAutopilotStartSiteContainerBuildLane(dogfoodInput)
    const deployInput = lane.deployInputForVersion('site_version_start_v1')

    assertAutopilotStartSiteLaneIsPublicSafe(lane)
    expect(selectSiteBuilderPreviewTier(lane.previewCandidateBeforeBuild))
      .toMatchObject({
        containerWorkGated: true,
        previewKind: 'container',
        tier: 'container_metered',
      })
    expect(selectSiteBuilderPreviewTier(lane.previewCandidateAfterBuild))
      .toMatchObject({
        containerWorkGated: false,
        previewKind: 'workers_for_platforms',
        tier: 'wfp_staging',
      })
    expect(lane.saveVersionInput).toMatchObject({
      buildCommand: AUTOPILOT_START_SITE_BUILD_COMMAND,
      buildStatus: 'saved',
      siteId: dogfoodInput.siteId,
      sourceKind: 'autopilot_generated',
      workerModuleText: expect.stringContaining('export default'),
    })
    expect(lane.saveVersionInput.metadata).toMatchObject({
      containerBuild: {
        lockfilePath: 'bun.lock',
        outputWorkerModulePath: AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
        tier: 'container_metered',
      },
      deployGate: {
        dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
        perSiteSecrets: 'worker_bindings_only',
        runtimeKind: 'workers_for_platforms',
      },
      template: {
        id: AUTOPILOT_START_SITE_TEMPLATE_ID,
        serverFns: true,
        ssrDefault: true,
        typedRoutes: true,
      },
    })
    expect(deployInput).toMatchObject({
      dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
      healthCheck: { status: 'passed' },
      launchChecklist: {
        audienceReviewed: true,
        buildReviewed: true,
        secretsReviewed: true,
        sourceReviewed: true,
        urlReviewed: true,
      },
      runtimeKind: 'workers_for_platforms',
      runtimeScriptName: 'oa-site-openagents-funnel-start-v1',
      siteId: dogfoodInput.siteId,
      uploadReceiptRef: lane.uploadReceiptRef,
      versionId: 'site_version_start_v1',
    })
    expect(JSON.stringify(lane.staticAssetsManifest)).toContain(
      'assets/start-site.css',
    )
    expect(lane.buildLogText).toContain('status=passed')
  })

  test('emits an executable Worker module for the dogfood funnel site', async () => {
    const lane = createAutopilotStartSiteContainerBuildLane(dogfoodInput)
    const encoded = Buffer.from(lane.workerModuleText).toString('base64')
    const workerModule = await import(`data:text/javascript;base64,${encoded}`)
    const worker = workerModule.default as {
      fetch: (request: Request) => Promise<Response> | Response
    }
    const landing = await worker.fetch(
      new Request('https://sites.openagents.com/openagents-funnel/'),
    )
    const manifest = await worker.fetch(
      new Request(
        'https://worker.openagents.local/.well-known/openagents.json',
      ),
    )
    const llms = await worker.fetch(
      new Request('https://worker.openagents.local/llms.txt'),
    )

    await expect(landing.text()).resolves.toContain(
      'OpenAgents AI operations funnel',
    )
    await expect(manifest.json()).resolves.toMatchObject({
      name: 'OpenAgents AI operations funnel',
      surfaces: {
        agent: '/.well-known/openagents.json',
        llms: '/llms.txt',
        sitemap: '/sitemap.xml',
      },
    })
    await expect(llms.text()).resolves.toContain(
      'OpenAgents turns operator-reviewed AI work',
    )
  })
})
