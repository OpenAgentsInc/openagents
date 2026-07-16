#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import matter from 'gray-matter'
import GithubSlugger from 'github-slugger'
import { Marked, Renderer, type Token, type Tokens } from 'marked'
import { createHighlighter } from 'shiki'

import {
  decodeDocsFrontmatter,
  type DocsHeading,
  type DocsPage,
  type DocsPageManifestEntry,
  type DocsSearchRecord,
} from '../src/docs/content-schema'
import { docsNavigationDefinition } from '../src/docs/docs-navigation'

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const contentRoot = path.join(appRoot, 'content', 'docs')
const generatedRoot = path.join(appRoot, 'src', 'docs', 'generated')
const generatedPagesRoot = path.join(generatedRoot, 'pages')
const publicDocsRoot = path.join(appRoot, 'public', 'docs')
const siteUrl = 'https://openagents.com'
const docsDescription =
  'Use, understand, and verify the local-first OpenAgents Desktop Codex workroom.'

type SourceDocument = Readonly<{
  body: string
  description: string
  filePath: string
  group: string
  lastModified: string
  rawSource: string
  sidebarLabel: string
  slug: string
  title: string
}>

const normalizeSlash = (value: string): string => value.split(path.sep).join('/')

const routePath = (slug: string): string => slug === '' ? '/docs' : `/docs/${slug}`

const rawMarkdownPath = (slug: string): string =>
  slug === '' ? '/docs/index.md' : `/docs/${slug}.md`

const generatedPageName = (slug: string): string =>
  `${slug === '' ? 'index' : slug.replaceAll('/', '__')}.generated.ts`

const escapeHtml = (value: string): string => value
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&#39;')

const collectMarkdownFiles = async (directory: string): Promise<ReadonlyArray<string>> => {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(entries.map(entry => {
    const target = path.join(directory, entry.name)
    return entry.isDirectory()
      ? collectMarkdownFiles(target)
      : Promise.resolve(entry.name.endsWith('.md') ? [target] : [])
  }))
  return nested.flat().sort((left, right) => left.localeCompare(right))
}

const slugForFile = (filePath: string): string => {
  const relative = normalizeSlash(path.relative(contentRoot, filePath)).replace(/\.md$/, '')
  return relative === 'index' ? '' : relative.replace(/\/index$/, '')
}

const normalizedFrontmatter = (input: Record<string, unknown>): Record<string, unknown> => ({
  ...input,
  lastModified: input['lastModified'] instanceof Date
    ? input['lastModified'].toISOString().slice(0, 10)
    : String(input['lastModified'] ?? ''),
})

const readSourceDocument = async (filePath: string): Promise<SourceDocument> => {
  const rawSource = await readFile(filePath, 'utf8')
  const parsed = matter(rawSource)
  const frontmatter = decodeDocsFrontmatter(normalizedFrontmatter(parsed.data))
  const slug = slugForFile(filePath)
  const group = docsNavigationDefinition.find(candidate => candidate.slugs.includes(slug))
  if (group === undefined) {
    throw new Error(`Public docs source is not in explicit navigation: ${filePath}`)
  }
  return {
    body: parsed.content.trim(),
    description: frontmatter.description,
    filePath,
    group: group.label,
    lastModified: frontmatter.lastModified,
    rawSource,
    sidebarLabel: frontmatter.sidebar.label ?? frontmatter.title,
    slug,
    title: frontmatter.title,
  }
}

const assertContentGraph = (documents: ReadonlyArray<SourceDocument>): void => {
  const slugs = documents.map(document => document.slug)
  const uniqueSlugs = new Set(slugs)
  if (uniqueSlugs.size !== slugs.length) {
    throw new Error('Public docs contain duplicate route slugs')
  }

  const declaredSlugs = docsNavigationDefinition.flatMap(group => group.slugs)
  const missing = declaredSlugs.filter(slug => !uniqueSlugs.has(slug))
  const undeclared = slugs.filter(slug => !declaredSlugs.includes(slug))
  if (missing.length > 0 || undeclared.length > 0) {
    throw new Error(
      `Public docs navigation mismatch (missing=${missing.join(',') || 'none'}; undeclared=${undeclared.join(',') || 'none'})`,
    )
  }
}

const internalMarkdownLinks = (source: string): ReadonlyArray<string> =>
  Array.from(source.matchAll(/\[[^\]]*\]\(([^)]+)\)/g), match => match[1] ?? '')
    .filter(href => href.startsWith('/docs'))

const assertInternalLinks = (documents: ReadonlyArray<SourceDocument>): void => {
  const validPaths = new Set(documents.map(document => routePath(document.slug)))
  const broken = documents.flatMap(document =>
    internalMarkdownLinks(document.body)
      .map(href => href.split('#')[0] ?? href)
      .filter(href => href !== '' && !validPaths.has(href))
      .map(href => `${document.filePath}: ${href}`),
  )
  if (broken.length > 0) {
    throw new Error(`Broken internal documentation links:\n${broken.join('\n')}`)
  }
}

const plainText = (html: string): string => html
  .replace(/<style[\s\S]*?<\/style>/g, ' ')
  .replace(/<[^>]+>/g, ' ')
  .replaceAll('&amp;', '&')
  .replaceAll('&lt;', '<')
  .replaceAll('&gt;', '>')
  .replaceAll('&quot;', '"')
  .replaceAll('&#39;', "'")
  .replace(/\s+/g, ' ')
  .trim()

const renderDocument = async (
  document: SourceDocument,
  highlighter: Awaited<ReturnType<typeof createHighlighter>>,
): Promise<Readonly<{ headings: ReadonlyArray<DocsHeading>; html: string }>> => {
  const slugger = new GithubSlugger()
  const headings: Array<DocsHeading> = []
  const renderer = new Renderer()

  renderer.heading = function ({ tokens, depth, text }: Tokens.Heading): string {
    const id = slugger.slug(text)
    if (depth === 2 || depth === 3) {
      headings.push({ depth, id, text })
    }
    const content = this.parser.parseInline(tokens)
    return `<h${depth} id="${escapeHtml(id)}"><a class="docs-heading-anchor" href="#${escapeHtml(id)}">${content}</a></h${depth}>\n`
  }

  renderer.code = ({ text, lang }: Tokens.Code): string => {
    const requestedLanguage = (lang ?? 'text').split(/\s+/)[0] ?? 'text'
    const language = highlighter.getLoadedLanguages().includes(requestedLanguage)
      ? requestedLanguage
      : 'text'
    const highlighted = highlighter.codeToHtml(text, {
      lang: language,
      theme: 'vesper',
    })
    return `<div class="docs-code"><div class="docs-code-toolbar"><span>${escapeHtml(language)}</span><button aria-label="Copy code" class="docs-code-copy" data-docs-copy-code type="button">Copy</button></div>${highlighted}</div>\n`
  }

  renderer.link = function ({ href, title, tokens }: Tokens.Link): string {
    const label = this.parser.parseInline(tokens)
    const titleAttribute = title === null || title === undefined
      ? ''
      : ` title="${escapeHtml(title)}"`
    const external = /^https?:\/\//.test(href)
    const externalAttributes = external ? ' rel="noreferrer" target="_blank"' : ''
    return `<a href="${escapeHtml(href)}"${titleAttribute}${externalAttributes}>${label}</a>`
  }

  const marked = new Marked({
    gfm: true,
    renderer,
    walkTokens: (token: Token) => {
      if (token.type === 'html') {
        throw new Error(`Raw HTML is not allowed in public docs: ${document.filePath}`)
      }
    },
  })
  const html = await marked.parse(document.body)
  return { headings, html }
}

const pageLink = (
  document: SourceDocument | undefined,
): Readonly<{ path: string; title: string }> | undefined => document === undefined
  ? undefined
  : { path: routePath(document.slug), title: document.title }

const serializeModule = (page: DocsPage): string =>
  `// Generated by scripts/generate-docs.ts. Do not edit.\n\nimport type { DocsPage } from '../../content-schema'\n\nconst page: DocsPage = ${JSON.stringify(page, null, 2)}\n\nexport default page\n`

const serializeManifestModule = (
  manifest: ReadonlyArray<DocsPageManifestEntry>,
): string => {
  const pageImports = manifest.map((entry, index) =>
    `import docsPage${index} from './pages/${generatedPageName(entry.slug).replace(/\.ts$/, '')}'`,
  ).join('\n')
  const pages = manifest.map((entry, index) =>
    `  ${JSON.stringify(entry.slug)}: docsPage${index},`,
  ).join('\n')
  return `// Generated by scripts/generate-docs.ts. Do not edit.\n\nimport type { DocsPage, DocsPageManifestEntry } from '../content-schema'\n${pageImports}\n\nexport const docsManifest: ReadonlyArray<DocsPageManifestEntry> = ${JSON.stringify(manifest, null, 2)}\n\nconst docsPages: Readonly<Record<string, DocsPage>> = {\n${pages}\n}\n\nexport const loadDocsPage = async (slug: string): Promise<DocsPage | undefined> => docsPages[slug]\n`
}

const llmsIndex = (documents: ReadonlyArray<SourceDocument>): string => {
  const sections = docsNavigationDefinition.map(group => {
    const links = group.slugs.map(slug => {
      const document = documents.find(candidate => candidate.slug === slug)
      if (document === undefined) {
        throw new Error(`Missing docs source for ${slug}`)
      }
      return `- [${document.title}](${siteUrl}${routePath(slug)}): ${document.description}`
    }).join('\n')
    return `## ${group.label}\n\n${links}`
  }).join('\n\n')
  return `# OpenAgents Docs\n\n> ${docsDescription}\n\n${sections}\n`
}

const llmsFull = (documents: ReadonlyArray<SourceDocument>): string => {
  const body = documents.map(document =>
    `# ${document.title}\nSource: ${siteUrl}${routePath(document.slug)}\n\n${document.body}`,
  ).join('\n\n---\n\n')
  return `# OpenAgents Docs\n\n> ${docsDescription}\n\n${body}\n`
}

const sitemap = (documents: ReadonlyArray<SourceDocument>): string => {
  const urls = [...documents]
    .sort((left, right) => routePath(left.slug).localeCompare(routePath(right.slug)))
    .map(document =>
      `  <url><loc>${siteUrl}${routePath(document.slug)}</loc><lastmod>${document.lastModified}</lastmod></url>`,
    )
    .join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`
}

const readabilityManifest = (): string => `${JSON.stringify({
  artifacts: {
    markdown: {
      contentNegotiation: 'text/markdown',
      pattern: `${siteUrl}/docs/{route}.md`,
    },
    llmsFullTxt: `${siteUrl}/docs/llms-full.txt`,
    llmsTxt: `${siteUrl}/docs/llms.txt`,
    sitemap: `${siteUrl}/docs/sitemap.xml`,
  },
  description: docsDescription,
  generator: 'openagents-tanstack-start',
  name: 'OpenAgents Docs',
  site: siteUrl,
  contentUsage: {
    search: true,
    'ai-input': true,
    'ai-train': false,
  },
  repository: 'https://github.com/OpenAgentsInc/openagents',
}, null, 2)}\n`

const writeGeneratedFile = async (
  filePath: string,
  content: string,
  check: boolean,
): Promise<void> => {
  if (check) {
    const existing = await readFile(filePath, 'utf8').catch(() => '')
    if (existing !== content) {
      throw new Error(`Generated documentation artifact is stale: ${filePath}`)
    }
    return
  }
  await mkdir(path.dirname(filePath), { recursive: true })
  await writeFile(filePath, content)
}

const main = async (): Promise<void> => {
  const check = process.argv.includes('--check')
  const files = await collectMarkdownFiles(contentRoot)
  const sourceDocuments = await Promise.all(files.map(readSourceDocument))
  assertContentGraph(sourceDocuments)
  assertInternalLinks(sourceDocuments)

  const orderedSlugs = docsNavigationDefinition.flatMap(group => group.slugs)
  const orderedDocuments = orderedSlugs.map(slug => {
    const document = sourceDocuments.find(candidate => candidate.slug === slug)
    if (document === undefined) {
      throw new Error(`Missing public documentation source: ${slug}`)
    }
    return document
  })

  const highlighter = await createHighlighter({
    langs: ['bash', 'javascript', 'json', 'markdown', 'text', 'tsx', 'typescript'],
    themes: ['vesper'],
  })
  const rendered = await Promise.all(orderedDocuments.map(async (document, index) => {
    const output = await renderDocument(document, highlighter)
    const next = pageLink(orderedDocuments[index + 1])
    const previous = pageLink(orderedDocuments[index - 1])
    const page: DocsPage = {
      description: document.description,
      editUrl: `https://github.com/OpenAgentsInc/openagents/edit/main/apps/openagents.com/apps/start/content/docs/${normalizeSlash(path.relative(contentRoot, document.filePath))}`,
      group: document.group,
      headings: output.headings,
      html: output.html,
      lastModified: document.lastModified,
      ...(next === undefined ? {} : { next }),
      path: routePath(document.slug),
      ...(previous === undefined ? {} : { previous }),
      rawMarkdownUrl: rawMarkdownPath(document.slug),
      sidebarLabel: document.sidebarLabel,
      slug: document.slug,
      title: document.title,
    }
    return { document, page }
  }))
  highlighter.dispose()

  const manifest = rendered.map(({ page: { html: _html, ...entry } }) => entry)
  const searchRecords: ReadonlyArray<DocsSearchRecord> = rendered.map(({ page }) => ({
    body: plainText(page.html),
    description: page.description,
    headings: page.headings.map(heading => heading.text).join(' '),
    id: createHash('sha256').update(page.path).digest('hex').slice(0, 16),
    path: page.path,
    title: page.title,
  }))

  if (!check) {
    await rm(generatedPagesRoot, { force: true, recursive: true })
    await rm(publicDocsRoot, { force: true, recursive: true })
  }

  await Promise.all([
    ...rendered.map(({ page }) =>
      writeGeneratedFile(
        path.join(generatedPagesRoot, generatedPageName(page.slug)),
        serializeModule(page),
        check,
      )),
    ...orderedDocuments.map(document =>
      writeGeneratedFile(
        path.join(publicDocsRoot, rawMarkdownPath(document.slug).replace(/^\/docs\//, '')),
        document.rawSource,
        check,
      )),
    writeGeneratedFile(
      path.join(generatedRoot, 'docs-manifest.generated.ts'),
      serializeManifestModule(manifest),
      check,
    ),
    writeGeneratedFile(
      path.join(publicDocsRoot, 'search.json'),
      `${JSON.stringify(searchRecords)}\n`,
      check,
    ),
    writeGeneratedFile(path.join(publicDocsRoot, 'llms.txt'), llmsIndex(orderedDocuments), check),
    writeGeneratedFile(path.join(publicDocsRoot, 'llms-full.txt'), llmsFull(orderedDocuments), check),
    writeGeneratedFile(
      path.join(publicDocsRoot, 'agent-readability.json'),
      readabilityManifest(),
      check,
    ),
    writeGeneratedFile(path.join(publicDocsRoot, 'sitemap.xml'), sitemap(orderedDocuments), check),
  ])

  console.log(
    `[docs] ${check ? 'verified' : 'generated'} ${orderedDocuments.length} pages and agent artifacts`,
  )
}

await main()
