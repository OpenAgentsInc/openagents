import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'

const docsDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = join(docsDir, '..', '..', '..', '..')
const readRepoFile = (...parts) => readFileSync(join(repoRoot, ...parts), 'utf8')

test('Terraform sends only the apex /docs paths to the isolated docs backend', () => {
  const loadBalancer = readRepoFile('infra', 'modules', 'global-external-lb', 'main.tf')
  const production = readRepoFile('infra', 'prod', 'main.tf')

  assert.match(loadBalancer, /paths\s*=\s*\["\/docs", "\/docs\/\*"\]/)
  assert.match(loadBalancer, /service\s*=\s*google_compute_backend_service\.docs\.id/)
  assert.match(loadBalancer, /hosts\s*=\s*var\.monolith_only_hosts/)
  assert.match(loadBalancer, /data\s+"google_compute_backend_service"\s+"components"/)
  assert.match(production, /name\s*=\s*"openagents-docs"/)
  assert.match(production, /docs_host\s*=\s*"openagents\.com"/)
  assert.match(production, /monolith_only_hosts\s*=\s*\["auth\.openagents\.com"\]/)
})

test('the docs deploy is static, secretless, and pinned to its own service', () => {
  const dockerfile = readRepoFile(
    'apps',
    'openagents.com',
    'apps',
    'docs',
    'Dockerfile',
  )
  const deploy = readRepoFile(
    'apps',
    'openagents.com',
    'apps',
    'docs',
    'deploy-cloudrun.sh',
  )

  assert.match(dockerfile, /FROM node:24\.13\.1-bookworm-slim/)
  assert.doesNotMatch(dockerfile, /pnpm|blume|astro/i)
  assert.match(deploy, /SERVICE="openagents-docs"/)
  assert.match(deploy, /gcloud run deploy "\$SERVICE"/)
  assert.match(deploy, /--min 1/)
  assert.doesNotMatch(deploy, /--set-secrets|--set-env-vars|--add-cloudsql-instances/)
})

test('docs navigation uses the Astro client router without a visible page crossfade', () => {
  const docsRoot = join(repoRoot, 'apps', 'openagents.com', 'apps', 'docs')
  const header = readFileSync(join(docsRoot, 'components', 'Header.astro'), 'utf8')
  const theme = readFileSync(join(docsRoot, 'theme.css'), 'utf8')

  assert.match(header, /import \{ ClientRouter \} from 'astro:transitions'/)
  assert.match(header, /<ClientRouter fallback="swap" \/>/)
  assert.match(header, /target\.pathname!=='\/docs'/)
  assert.match(header, /target\.pathname\.startsWith\('\/docs\/'\)/)
  assert.match(header, /link\.dataset\.astroReload=''/)
  assert.match(theme, /::view-transition-old\(root\)/)
  assert.match(theme, /::view-transition-new\(root\)/)
  assert.match(theme, /animation-duration: 0\.01ms/)
})

test('docs use the landing site sans stack and reserve Commit Mono for code', () => {
  const docsRoot = join(repoRoot, 'apps', 'openagents.com', 'apps', 'docs')
  const docsTheme = readFileSync(join(docsRoot, 'theme.css'), 'utf8')
  const landingLayout = readRepoFile(
    'apps',
    'openagents.com',
    'apps',
    'astro',
    'src',
    'layouts',
    'Layout.astro',
  )
  const sansStack = /ui-sans-serif, -apple-system, BlinkMacSystemFont, ['"]Segoe UI['"], sans-serif/

  assert.match(landingLayout, sansStack)
  assert.match(docsTheme, new RegExp(`--blume-font-display: ${sansStack.source}`))
  assert.match(docsTheme, new RegExp(`--blume-font-body: ${sansStack.source}`))
  assert.match(docsTheme, /--blume-font-mono: 'Commit Mono'/)
})

test('the drawer toggle disappears when Blume switches to its static desktop sidebar', () => {
  const theme = readRepoFile(
    'apps',
    'openagents.com',
    'apps',
    'docs',
    'theme.css',
  )

  assert.match(theme, /@media \(min-width: 64rem\)/)
  assert.match(
    theme,
    /\.oa-icon-button\[data-blume-nav-toggle\]\s*\{\s*display:\s*none;/,
  )
})

test('the docs header stays fixed while the document grid preserves its space', () => {
  const theme = readRepoFile(
    'apps',
    'openagents.com',
    'apps',
    'docs',
    'theme.css',
  )

  assert.match(theme, /\.oa-docs-header\s*\{[^}]*position:\s*fixed;/s)
  assert.match(theme, /\[data-blume-doc-grid\]\s*\{[^}]*padding-top:\s*4rem;/s)
  assert.match(theme, /overscroll-behavior-y:\s*none;/)
})

test('human docs navigation omits promises while agent docs retain machine authority', () => {
  const docsRoot = join(repoRoot, 'apps', 'openagents.com', 'apps', 'docs')
  const header = readFileSync(join(docsRoot, 'components', 'Header.astro'), 'utf8')
  const agentDocs = readFileSync(join(docsRoot, 'content', 'agent-readable.mdx'), 'utf8')

  assert.doesNotMatch(header, />Promises</)
  assert.match(header, /class="oa-docs-brand" href="\/"/)
  assert.match(header, /class="oa-docs-section" href="\/docs"/)
  assert.equal(existsSync(join(docsRoot, 'content', 'product-promises.mdx')), false)
  assert.match(agentDocs, /\/api\/public\/product-promises/)
  assert.match(agentDocs, /machine-facing evidence, not human website navigation/)
})

test('future docs are separated from live MVP guidance and carry explicit status', () => {
  const futureDir = join(
    repoRoot,
    'apps',
    'openagents.com',
    'apps',
    'docs',
    'content',
    'future',
  )
  const config = readFileSync(join(dirname(futureDir), '..', 'blume.config.ts'), 'utf8')
  const meta = readFileSync(join(futureDir, 'meta.ts'), 'utf8')
  const pages = [
    'index.mdx',
    'marketplaces.mdx',
    'nostr.mdx',
    'bitcoin-and-lightning.mdx',
    'remote-workrooms.mdx',
  ]

  assert.match(meta, /title: 'Future \/ Advanced'/)
  assert.match(meta, /collapsed: true/)
  assert.match(config, /sidebar:\s*\{\s*display: 'group'/s)

  for (const page of pages) {
    const source = readFileSync(join(futureDir, page), 'utf8')
    assert.match(source, /\*\*Status:\*\*/)
    assert.match(source, /not (?:a live feature|part of the current MVP)/i)
  }
})
