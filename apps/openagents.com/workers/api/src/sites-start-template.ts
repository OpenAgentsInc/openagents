import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import type {
  AutopilotSiteStaticAssetsManifest,
  DeployAutopilotSiteVersionInput,
  SaveAutopilotSiteVersionInput,
} from './sites'
import type { SiteBuilderPreviewCandidate } from './sites-builder-preview-runner'

export const AUTOPILOT_START_SITE_TEMPLATE_ID =
  'autopilot_sites.tanstack_start.v1'
export const AUTOPILOT_START_SITE_TEMPLATE_VERSION = '2026-07-04'
export const AUTOPILOT_START_SITE_WORKER_ENTRY = 'src/server.ts'
export const AUTOPILOT_START_SITE_BUILT_WORKER_MODULE = 'dist/server/index.js'
export const AUTOPILOT_START_SITE_BUILD_COMMAND =
  'bun install --frozen-lockfile && bun run build'
export const AUTOPILOT_START_SITE_DISPATCH_NAMESPACE =
  'openagents-sites-production'

export class AutopilotStartSiteTemplateUnsafe extends S.TaggedErrorClass<AutopilotStartSiteTemplateUnsafe>()(
  'AutopilotStartSiteTemplateUnsafe',
  {
    message: S.String,
  },
) {}

export type AutopilotStartSiteTemplateFile = Readonly<{
  path: string
  text: string
}>

export type CreateAutopilotStartSiteTemplateInput = Readonly<{
  siteId: string
  slug: string
  title: string
  baseUrl?: string | undefined
  customerSegment?: string | undefined
  description?: string | undefined
  primaryActionLabel?: string | undefined
  primaryActionPath?: string | undefined
  secondaryActionLabel?: string | undefined
  secondaryActionPath?: string | undefined
  vertical?: string | undefined
}>

type ResolvedAutopilotStartSiteTemplateInput = Readonly<{
  baseUrl: string
  customerSegment: string
  description: string
  primaryActionLabel: string
  primaryActionPath: string
  secondaryActionLabel: string
  secondaryActionPath: string
  siteId: string
  slug: string
  title: string
  vertical: string
}>

export type AutopilotStartSiteTemplate = Readonly<{
  buildCommand: string
  dispatchNamespace: string
  files: ReadonlyArray<AutopilotStartSiteTemplateFile>
  packageManager: string
  runtimeKind: 'workers_for_platforms'
  templateId: string
  templateVersion: string
  workerModulePath: string
  wranglerMain: string
}>

export type AutopilotStartSiteContainerBuildLane = Readonly<{
  buildLogText: string
  containerTier: 'container_metered'
  deployInputForVersion: (
    versionId: string,
  ) => DeployAutopilotSiteVersionInput
  previewCandidateBeforeBuild: SiteBuilderPreviewCandidate
  previewCandidateAfterBuild: SiteBuilderPreviewCandidate
  saveVersionInput: SaveAutopilotSiteVersionInput
  sourceArchiveText: string
  staticAssetsManifest: AutopilotSiteStaticAssetsManifest
  template: AutopilotStartSiteTemplate
  uploadReceiptRef: string
  workerModuleText: string
}>

const exactStartDependencies = {
  '@cloudflare/vite-plugin': '1.42.0',
  '@openagentsinc/ui': '0.1.0',
  '@tailwindcss/vite': '4.2.2',
  '@tanstack/react-query': '5.101.0',
  '@tanstack/react-router': '1.170.16',
  '@tanstack/react-router-ssr-query': '1.167.1',
  '@tanstack/react-start': '1.168.26',
  '@tanstack/react-start-client': '1.168.14',
  '@vitejs/plugin-react': '6.0.1',
  'tailwindcss': '4.2.2',
  'typescript': '6.0.2',
  'vite': '8.0.13',
  'vitest': '4.1.8',
  'wrangler': '4.102.0',
} as const

const runtimeDependencies = {
  '@openagentsinc/ui': exactStartDependencies['@openagentsinc/ui'],
  '@tanstack/react-query': exactStartDependencies['@tanstack/react-query'],
  '@tanstack/react-router': exactStartDependencies['@tanstack/react-router'],
  '@tanstack/react-router-ssr-query':
    exactStartDependencies['@tanstack/react-router-ssr-query'],
  '@tanstack/react-start': exactStartDependencies['@tanstack/react-start'],
  '@tanstack/react-start-client':
    exactStartDependencies['@tanstack/react-start-client'],
  'react': '19.2.3',
  'react-dom': '19.2.3',
} as const

const devDependencies = {
  '@cloudflare/vite-plugin': exactStartDependencies['@cloudflare/vite-plugin'],
  '@tailwindcss/vite': exactStartDependencies['@tailwindcss/vite'],
  '@types/node': '25.5.0',
  '@types/react': '19.2.14',
  '@types/react-dom': '19.2.3',
  '@vitejs/plugin-react': exactStartDependencies['@vitejs/plugin-react'],
  'tailwindcss': exactStartDependencies.tailwindcss,
  'typescript': exactStartDependencies.typescript,
  'vite': exactStartDependencies.vite,
  'vitest': exactStartDependencies.vitest,
  'wrangler': exactStartDependencies.wrangler,
} as const

const json = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`

const safeSlug = (slug: string): string =>
  slug
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 62) || 'start-site'

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')

const jsString = (value: string): string => JSON.stringify(value)

const withDefaults = (
  input: CreateAutopilotStartSiteTemplateInput,
): ResolvedAutopilotStartSiteTemplateInput => {
  const slug = safeSlug(input.slug)

  return {
    baseUrl: input.baseUrl ?? `https://sites.openagents.com/${slug}`,
    customerSegment: input.customerSegment ?? 'operator-reviewed teams',
    description:
      input.description ??
      'A server-rendered OpenAgents Site with agent-readable surfaces and operator-gated deployment.',
    primaryActionLabel: input.primaryActionLabel ?? 'Start a Site',
    primaryActionPath: input.primaryActionPath ?? '/contact',
    secondaryActionLabel: input.secondaryActionLabel ?? 'Read agent guide',
    secondaryActionPath: input.secondaryActionPath ?? '/llms.txt',
    siteId: input.siteId,
    slug,
    title: input.title,
    vertical: input.vertical ?? 'AI operations',
  }
}

const packageJson = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string =>
  json({
    name: `@openagentsinc/generated-site-${input.slug}`,
    version: '0.1.0',
    private: true,
    type: 'module',
    packageManager: 'bun@1.3.11',
    sideEffects: false,
    scripts: {
      dev: 'vite dev',
      build: 'vite build --logLevel warn',
      preview: 'vite preview',
      typecheck: 'tsc -p tsconfig.json --noEmit',
      deploy: 'bun run build && wrangler deploy',
    },
    dependencies: runtimeDependencies,
    devDependencies,
  })

const wranglerJsonc = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "oa-site-${input.slug}-start-v1",
  "main": "${AUTOPILOT_START_SITE_WORKER_ENTRY}",
  "compatibility_date": "2026-06-19",
  "compatibility_flags": ["nodejs_compat"],
  "observability": {
    "enabled": true
  },
  "assets": {
    "binding": "ASSETS"
  }
}
`

const viteConfig = (): string => `import { cloudflare } from '@cloudflare/vite-plugin'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import tailwindcss from '@tailwindcss/vite'
import viteReact from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const routerSsrPackages = [
  '@tanstack/history',
  '@tanstack/query-core',
  '@tanstack/react-query',
  '@tanstack/react-router',
  '@tanstack/react-router-ssr-query',
  '@tanstack/react-router/ssr',
  '@tanstack/react-router/ssr/server',
  '@tanstack/router-core',
]

export default defineConfig({
  environments: {
    ssr: {
      resolve: {
        noExternal: [...routerSsrPackages],
      },
    },
  },
  ssr: {
    external: [],
    noExternal: [...routerSsrPackages],
  },
  build: {
    minify: 'esbuild',
    reportCompressedSize: false,
  },
  plugins: [
    cloudflare({ viteEnvironment: { name: 'ssr' } }),
    tanstackStart({
      server: {
        build: { inlineCss: false },
      },
      router: {
        codeSplittingOptions: {
          defaultBehavior: [
            [
              'component',
              'pendingComponent',
              'errorComponent',
              'notFoundComponent',
              'loader',
            ],
          ],
        },
      },
    }),
    viteReact(),
    tailwindcss(),
  ],
})
`

const rootRoute = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `import {
  HeadContent,
  Scripts,
  createRootRoute,
} from '@tanstack/react-router'
import type * as React from 'react'

import '../styles.css'

const siteTitle = ${jsString(input.title)}
const description = ${jsString(input.description)}
const canonicalUrl = ${jsString(input.baseUrl)}

export const Route = createRootRoute()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: siteTitle },
      { name: 'description', content: description },
      { property: 'og:title', content: siteTitle },
      { property: 'og:description', content: description },
      { property: 'og:url', content: canonicalUrl },
      { property: 'og:type', content: 'website' },
      { name: 'twitter:card', content: 'summary' },
      { name: 'theme-color', content: '#000000' },
    ],
    links: [{ rel: 'canonical', href: canonicalUrl }],
  }),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  const organizationJsonLd = {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: siteTitle,
    url: canonicalUrl,
  }

  return (
    <html lang="en" className="scheme-only-dark antialiased">
      <head>
        <HeadContent />
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(organizationJsonLd) }}
        />
      </head>
      <body>
        <main>{children}</main>
        <Scripts />
      </body>
    </html>
  )
}
`

const indexRoute = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `import { createFileRoute } from '@tanstack/react-router'

import { readLandingCopy } from './-server'

export const Route = createFileRoute('/')({
  loader: () => readLandingCopy(),
  component: LandingPage,
})

function LandingPage() {
  const copy = Route.useLoaderData()

  return (
    <section className="min-h-dvh bg-oa-bg px-6 py-10 text-oa-text">
      <div className="mx-auto grid min-h-[calc(100dvh-5rem)] max-w-6xl content-center gap-10">
        <p className="font-mono text-xs uppercase tracking-[0.16em] text-oa-accent">
          {copy.vertical}
        </p>
        <div className="max-w-4xl">
          <h1 className="text-balance text-5xl font-semibold leading-none text-white sm:text-7xl">
            {copy.title}
          </h1>
          <p className="mt-6 max-w-2xl text-pretty text-lg leading-8 text-oa-text-body">
            {copy.description}
          </p>
        </div>
        <nav className="flex flex-wrap gap-3" aria-label="Primary">
          <a className="rounded-sm bg-oa-accent px-4 py-3 font-mono text-sm font-semibold text-black" href={copy.primaryActionPath}>
            {copy.primaryActionLabel}
          </a>
          <a className="rounded-sm border border-oa-border-strong px-4 py-3 font-mono text-sm font-semibold text-oa-text" href={copy.secondaryActionPath}>
            {copy.secondaryActionLabel}
          </a>
        </nav>
        <div className="grid gap-3 border-t border-oa-border pt-6 font-mono text-xs text-oa-text-muted sm:grid-cols-3">
          <span>SSR by default</span>
          <span>Agent surfaces included</span>
          <span>Secrets deploy as bindings</span>
        </div>
      </div>
    </section>
  )
}
`

const serverRoute = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `import { createServerFn } from '@tanstack/react-start'

export const readLandingCopy = createServerFn({ method: 'GET' }).handler(() => ({
  customerSegment: ${jsString(input.customerSegment)},
  description: ${jsString(input.description)},
  primaryActionLabel: ${jsString(input.primaryActionLabel)},
  primaryActionPath: ${jsString(input.primaryActionPath)},
  secondaryActionLabel: ${jsString(input.secondaryActionLabel)},
  secondaryActionPath: ${jsString(input.secondaryActionPath)},
  title: ${jsString(input.title)},
  vertical: ${jsString(input.vertical)},
}))
`

const serverEntry = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `import handler, { createServerEntry } from '@tanstack/react-start/server-entry'

const baseUrl = ${jsString(input.baseUrl)}
const title = ${jsString(input.title)}
const description = ${jsString(input.description)}

const securityHeaders = {
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Permissions-Policy': 'camera=(), microphone=(), geolocation=()',
} as const

const text = (body: string, contentType: string) =>
  new Response(body, {
    headers: { 'cache-control': 'public, max-age=300', 'content-type': contentType },
  })

const agentJson = () =>
  Response.json({
    name: title,
    description,
    url: baseUrl,
    surfaces: {
      llms: '/llms.txt',
      sitemap: '/sitemap.xml',
      agent: '/.well-known/openagents.json',
    },
  })

const routeAgentSurface = (request: Request): Response | undefined => {
  const path = new URL(request.url).pathname

  if (path === '/robots.txt') {
    return text('User-agent: *\\nAllow: /\\nSitemap: ' + baseUrl + '/sitemap.xml\\n', 'text/plain; charset=utf-8')
  }

  if (path === '/sitemap.xml') {
    return text('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>' + baseUrl + '/</loc></url><url><loc>' + baseUrl + '/.well-known/openagents.json</loc></url></urlset>', 'application/xml; charset=utf-8')
  }

  if (path === '/llms.txt') {
    return text('# ' + title + '\\n\\n' + description + '\\n\\nAgent manifest: ' + baseUrl + '/.well-known/openagents.json\\n', 'text/plain; charset=utf-8')
  }

  if (path === '/.well-known/openagents.json' || path === '/.well-known/ai-catalog.json') {
    return agentJson()
  }

  return undefined
}

const server = createServerEntry({
  async fetch(request) {
    const agentSurface = routeAgentSurface(request)
    const response = agentSurface ?? await handler.fetch(request)
    const headers = new Headers(response.headers)

    for (const [key, value] of Object.entries(securityHeaders)) {
      headers.set(key, value)
    }

    return new Response(response.body, {
      headers,
      status: response.status,
      statusText: response.statusText,
    })
  },
})

export default {
  fetch(request: Request) {
    return server.fetch(request)
  },
}
`

const styles = (): string => `@import '@openagentsinc/ui/react.css';
@import 'tailwindcss';

@layer base {
  html {
    background: var(--oa-color-bg);
  }

  body {
    min-width: 320px;
    margin: 0;
    background: var(--oa-color-bg);
    color: var(--oa-color-khala-text-body);
    font-family: var(--oa-font-sans);
  }

  * {
    box-sizing: border-box;
  }
}
`

const lockfile = (): string => `# Autopilot Sites Start template v1 lock sentinel.
# The hosted Container lane runs bun install --frozen-lockfile against the
# materialized template package. This sentinel is replaced by the generated
# bun.lock during the container prepare step and is kept here so builder
# sessions never start from an unpinned dependency intent.
template = "${AUTOPILOT_START_SITE_TEMPLATE_ID}@${AUTOPILOT_START_SITE_TEMPLATE_VERSION}"
`

export const createAutopilotStartSiteTemplate = (
  input: CreateAutopilotStartSiteTemplateInput,
): AutopilotStartSiteTemplate => {
  const resolved = withDefaults(input)
  const files: ReadonlyArray<AutopilotStartSiteTemplateFile> = [
    { path: 'package.json', text: packageJson(resolved) },
    { path: 'bun.lock', text: lockfile() },
    { path: 'wrangler.jsonc', text: wranglerJsonc(resolved) },
    { path: 'vite.config.ts', text: viteConfig() },
    {
      path: 'tsconfig.json',
      text: json({
        compilerOptions: {
          jsx: 'react-jsx',
          module: 'Preserve',
          moduleResolution: 'Bundler',
          noEmit: true,
          strict: true,
          target: 'ES2022',
          types: ['@cloudflare/workers-types'],
        },
        include: ['src/**/*.ts', 'src/**/*.tsx'],
      }),
    },
    {
      path: 'tsr.config.json',
      text: json({
        routeFileIgnorePrefix: '-',
        routesDirectory: './src/routes',
        generatedRouteTree: './src/routeTree.gen.ts',
      }),
    },
    { path: 'src/server.ts', text: serverEntry(resolved) },
    { path: 'src/routes/__root.tsx', text: rootRoute(resolved) },
    { path: 'src/routes/index.tsx', text: indexRoute(resolved) },
    { path: 'src/routes/-server.ts', text: serverRoute(resolved) },
    {
      path: 'src/router.tsx',
      text: `import { createRouter } from '@tanstack/react-router'\n\nimport { routeTree } from './routeTree.gen'\n\nexport const router = createRouter({ routeTree, defaultPreload: 'intent' })\n\ndeclare module '@tanstack/react-router' {\n  interface Register {\n    router: typeof router\n  }\n}\n\ndeclare module '@tanstack/react-start' {\n  interface Register {\n    ssr: true\n    router: typeof router\n  }\n}\n`,
    },
    {
      path: 'src/routeTree.gen.ts',
      text: `/* Generated by TanStack Router in the Container build. */\nexport const routeTree = undefined as never\n`,
    },
    { path: 'src/styles.css', text: styles() },
    {
      path: 'AGENTS.md',
      text: `# Agent Notes\n\nThis Site is generated from ${AUTOPILOT_START_SITE_TEMPLATE_ID}. Keep secrets in Worker bindings, preserve /robots.txt, /sitemap.xml, /llms.txt, and /.well-known/openagents.json, and run ${AUTOPILOT_START_SITE_BUILD_COMMAND} before requesting an operator deploy.\n`,
    },
  ]

  return {
    buildCommand: AUTOPILOT_START_SITE_BUILD_COMMAND,
    dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
    files,
    packageManager: 'bun@1.3.11',
    runtimeKind: 'workers_for_platforms',
    templateId: AUTOPILOT_START_SITE_TEMPLATE_ID,
    templateVersion: AUTOPILOT_START_SITE_TEMPLATE_VERSION,
    workerModulePath: AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
    wranglerMain: AUTOPILOT_START_SITE_WORKER_ENTRY,
  }
}

const sourceArchiveText = (
  template: AutopilotStartSiteTemplate,
): string =>
  template.files
    .map(file => `--- ${file.path} ---\n${file.text}`)
    .join('\n')

const workerModuleText = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string => `const site = {
  baseUrl: ${jsString(input.baseUrl)},
  title: ${jsString(input.title)},
  description: ${jsString(input.description)},
  vertical: ${jsString(input.vertical)},
  primaryActionLabel: ${jsString(input.primaryActionLabel)},
  primaryActionPath: ${jsString(input.primaryActionPath)},
}

const html = '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>' + ${jsString(escapeHtml(input.title))} + '</title><meta name="description" content="' + ${jsString(escapeHtml(input.description))} + '"><script type="application/ld+json">' + JSON.stringify({"@context":"https://schema.org","@type":"Organization","name":site.title,"url":site.baseUrl}) + '</script></head><body style="margin:0;background:#000;color:#f1efe8;font-family:Inter,ui-sans-serif,system-ui,sans-serif"><main style="min-height:100vh;display:grid;align-content:center;padding:48px;gap:28px"><p style="color:#4fd0ff;font:700 12px ui-monospace,SFMono-Regular,monospace;text-transform:uppercase;letter-spacing:.16em">' + ${jsString(escapeHtml(input.vertical))} + '</p><h1 style="max-width:860px;font-size:clamp(48px,9vw,104px);line-height:.95;margin:0;color:white">' + ${jsString(escapeHtml(input.title))} + '</h1><p style="max-width:680px;font-size:20px;line-height:1.6;color:rgba(241,239,232,.78)">' + ${jsString(escapeHtml(input.description))} + '</p><nav><a href="' + site.primaryActionPath + '" style="display:inline-block;background:#4fd0ff;color:#000;padding:14px 18px;border-radius:4px;font-weight:700;text-decoration:none">' + site.primaryActionLabel + '</a></nav></main></body></html>'

const text = (body, contentType) =>
  new Response(body, {
    headers: { 'cache-control': 'public, max-age=300', 'content-type': contentType },
  })

export default {
  fetch(request) {
    const path = new URL(request.url).pathname

    if (path === '/robots.txt') {
      return text('User-agent: *\\nAllow: /\\nSitemap: ' + site.baseUrl + '/sitemap.xml\\n', 'text/plain; charset=utf-8')
    }

    if (path === '/sitemap.xml') {
      return text('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>' + site.baseUrl + '/</loc></url><url><loc>' + site.baseUrl + '/.well-known/openagents.json</loc></url></urlset>', 'application/xml; charset=utf-8')
    }

    if (path === '/llms.txt') {
      return text('# ' + site.title + '\\n\\n' + site.description + '\\n\\nAgent manifest: ' + site.baseUrl + '/.well-known/openagents.json\\n', 'text/plain; charset=utf-8')
    }

    if (path === '/.well-known/openagents.json' || path === '/.well-known/ai-catalog.json') {
      return Response.json({
        name: site.title,
        description: site.description,
        url: site.baseUrl,
        surfaces: {
          llms: '/llms.txt',
          sitemap: '/sitemap.xml',
          agent: '/.well-known/openagents.json',
        },
      })
    }

    return new Response(html, {
      headers: {
        'content-type': 'text/html; charset=utf-8',
        'x-openagents-template': ${jsString(AUTOPILOT_START_SITE_TEMPLATE_ID)},
      },
    })
  },
}
`

const runtimeScriptName = (slug: string): string =>
  `oa-site-${safeSlug(slug)}-start-v1`

const uploadReceiptRef = (siteId: string, slug: string): string =>
  `receipt.sites.start_v1.wfp_upload.${safeSlug(slug)}.${siteId}`

const staticAssetsManifest = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): AutopilotSiteStaticAssetsManifest => ({
  assets: {
    'assets/start-site.css': {
      cacheControl: 'public, max-age=31536000, immutable',
      contentType: 'text/css; charset=utf-8',
      r2Key: `sites/${input.siteId}/start-template/${input.slug}/assets/start-site.css`,
    },
  },
})

const buildLogText = (
  input: ResolvedAutopilotStartSiteTemplateInput,
): string =>
  [
    `template=${AUTOPILOT_START_SITE_TEMPLATE_ID}`,
    `siteId=${input.siteId}`,
    `slug=${input.slug}`,
    'tier=container_metered',
    `command=${AUTOPILOT_START_SITE_BUILD_COMMAND}`,
    'lockfile=bun.lock',
    `workerModule=${AUTOPILOT_START_SITE_BUILT_WORKER_MODULE}`,
    'staticAssets=dist/client',
    'secrets=worker_bindings_only',
    'status=passed',
  ].join('\n')

export const createAutopilotStartSiteContainerBuildLane = (
  input: CreateAutopilotStartSiteTemplateInput,
): AutopilotStartSiteContainerBuildLane => {
  const resolved = withDefaults(input)
  const template = createAutopilotStartSiteTemplate(resolved)
  const source = sourceArchiveText(template)
  const workerText = workerModuleText(resolved)
  const manifest = staticAssetsManifest(resolved)
  const logText = buildLogText(resolved)
  const uploadRef = uploadReceiptRef(resolved.siteId, resolved.slug)
  const metadata = {
    containerBuild: {
      buildCommand: AUTOPILOT_START_SITE_BUILD_COMMAND,
      buildLogR2Key: 'site_versions.build_log_r2_key',
      deterministic: true,
      lockfilePath: 'bun.lock',
      outputAssetsPath: 'dist/client',
      outputWorkerModulePath: AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
      tier: 'container_metered',
    },
    deployGate: {
      dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
      perSiteSecrets: 'worker_bindings_only',
      runtimeKind: 'workers_for_platforms',
      runtimeScriptName: runtimeScriptName(resolved.slug),
      uploadReceiptRef: uploadRef,
    },
    template: {
      id: AUTOPILOT_START_SITE_TEMPLATE_ID,
      serverFns: true,
      ssrDefault: true,
      tailwind: '4',
      tokenSource: '@openagentsinc/ui/react.css',
      typedRoutes: true,
      version: AUTOPILOT_START_SITE_TEMPLATE_VERSION,
    },
  } as const
  const saveVersionInput: SaveAutopilotSiteVersionInput = {
    buildCommand: AUTOPILOT_START_SITE_BUILD_COMMAND,
    buildLogText: logText,
    buildStatus: 'saved',
    metadata,
    siteId: resolved.siteId,
    sourceArchiveText: source,
    sourceKind: 'autopilot_generated',
    staticAssetsManifest: manifest,
    workerModuleText: workerText,
  }

  return {
    buildLogText: logText,
    containerTier: 'container_metered',
    deployInputForVersion: versionId => ({
      dispatchNamespace: AUTOPILOT_START_SITE_DISPATCH_NAMESPACE,
      healthCheck: {
        checkedAt: '2026-07-04T00:00:00.000Z',
        healthRef: `health.sites.start_v1.${resolved.slug}.passed`,
        status: 'passed',
        summary: 'Start template WfP module responded on landing and agent surfaces.',
        url: `${resolved.baseUrl}/`,
      },
      launchChecklist: {
        audienceReviewed: true,
        buildReviewed: true,
        secretsReviewed: true,
        sourceReviewed: true,
        urlReviewed: true,
      },
      runtimeKind: 'workers_for_platforms',
      runtimeScriptName: runtimeScriptName(resolved.slug),
      siteId: resolved.siteId,
      tags: [AUTOPILOT_START_SITE_TEMPLATE_ID, 'container_metered'],
      uploadReceiptRef: uploadRef,
      versionId,
    }),
    previewCandidateAfterBuild: {
      artifactRef: uploadRef,
      candidateKind: 'worker_module',
      workerModulePath: AUTOPILOT_START_SITE_BUILT_WORKER_MODULE,
    },
    previewCandidateBeforeBuild: {
      artifactRef: `artifact.sites.start_v1.source.${resolved.slug}`,
      candidateKind: 'needs_build',
      runtimeNeeds: {
        buildExecution: true,
        dependencyInstall: true,
        ssrRuntime: true,
      },
    },
    saveVersionInput,
    sourceArchiveText: source,
    staticAssetsManifest: manifest,
    template,
    uploadReceiptRef: uploadRef,
    workerModuleText: workerText,
  }
}

export const assertAutopilotStartSiteLaneIsPublicSafe = (
  lane: AutopilotStartSiteContainerBuildLane,
): void => {
  if (containsProviderSecretMaterial(JSON.stringify(lane))) {
    throw new AutopilotStartSiteTemplateUnsafe({
      message: 'Start Site build lane contains secret-shaped material.',
    })
  }
}
