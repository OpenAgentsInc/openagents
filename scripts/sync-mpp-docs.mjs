#!/usr/bin/env node

import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const outputRoot = path.join(repoRoot, 'docs/reference/mpp')
const mppDevRoot = path.join(outputRoot, 'mpp-dev')
const paymentAuthRoot = path.join(outputRoot, 'paymentauth')
const localSpecRoot = path.resolve(repoRoot, '../projects/repos/mpp-specs')

const mppBase = 'https://mpp.dev'
const paymentAuthBase = 'https://paymentauth.org'

const fetchText = async (url, accept = 'text/markdown') => {
  const response = await fetch(url, {
    headers: {
      accept,
      'user-agent': 'OpenAgents-MPP-docs-mirror/1.0',
    },
  })
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status} ${response.statusText}`)
  }
  return await response.text()
}

const normalizeText = text => {
  const withoutCarriageReturns = text.replace(/\r\n?/g, '\n')
  return `${withoutCarriageReturns.replace(/[ \t]+$/gm, '').replace(/\n*$/, '')}\n`
}

const writeText = async (filePath, text) => {
  await mkdir(path.dirname(filePath), { recursive: true })
  await writeFile(filePath, normalizeText(text))
}

const copyTextFile = async (sourcePath, targetPath) => {
  await writeText(targetPath, await readFile(sourcePath, 'utf8'))
}

const copyTextTree = async (sourceDir, targetDir) => {
  const entries = await readdir(sourceDir, { withFileTypes: true })
  for (const entry of entries) {
    const sourcePath = path.join(sourceDir, entry.name)
    const targetPath = path.join(targetDir, entry.name)
    if (entry.isDirectory()) {
      await copyTextTree(sourcePath, targetPath)
    } else if (entry.isFile()) {
      await copyTextFile(sourcePath, targetPath)
    }
  }
}

const pagePathFor = route => {
  const trimmed = route.replace(/^\/+/, '').replace(/\/+$/, '')
  if (trimmed === '') {
    return 'index.md'
  }
  return `${trimmed}/index.md`
}

const extractSitemapRoutes = llmsFull => {
  const routes = new Set()
  const re = /^- \[[^\]]+\]\((\/[^)]+)\):/gm
  let match
  while ((match = re.exec(llmsFull)) !== null) {
    const route = match[1]
    if (!route.includes('://') && !route.startsWith('/api/')) {
      routes.add(route)
    }
  }
  return [...routes].sort()
}

const listSpecBasenames = async () => {
  const specsDir = path.join(localSpecRoot, 'specs')
  const specs = []
  const walk = async dir => {
    const entries = await readdir(dir, { withFileTypes: true })
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        await walk(fullPath)
      } else if (entry.isFile() && entry.name.endsWith('.md')) {
        specs.push(path.basename(entry.name, '.md'))
      }
    }
  }
  await walk(specsDir)
  return specs.sort()
}

const main = async () => {
  await rm(outputRoot, { recursive: true, force: true })
  await mkdir(outputRoot, { recursive: true })

  const llms = await fetchText(`${mppBase}/llms.txt`, 'text/plain')
  const llmsFull = await fetchText(`${mppBase}/llms-full.txt`, 'text/plain')
  await writeText(path.join(mppDevRoot, 'llms.txt'), llms)
  await writeText(path.join(mppDevRoot, 'llms-full.txt'), llmsFull)

  const mcpCard = await fetchText(`${mppBase}/.well-known/mcp.json`, 'application/json')
  const skillsIndex = await fetchText(
    `${mppBase}/.well-known/agent-skills/index.json`,
    'application/json',
  )
  await writeText(path.join(mppDevRoot, '.well-known/mcp.json'), mcpCard)
  await writeText(path.join(mppDevRoot, '.well-known/agent-skills/index.json'), skillsIndex)

  const parsedSkills = JSON.parse(skillsIndex)
  const skillFiles = []
  const skillFailures = []
  for (const skill of parsedSkills.skills ?? []) {
    if (typeof skill.url !== 'string') {
      continue
    }
    try {
      const skillText = await fetchText(`${mppBase}${skill.url}`, 'text/markdown')
      const skillPath = path.join(mppDevRoot, skill.url.replace(/^\/+/, ''))
      await writeText(skillPath, skillText)
      skillFiles.push(skill.url)
    } catch (error) {
      skillFailures.push({
        url: skill.url,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  const routes = extractSitemapRoutes(llmsFull)
  const pageFailures = []
  for (const route of routes) {
    try {
      const page = await fetchText(`${mppBase}${route}`, 'text/markdown')
      await writeText(path.join(mppDevRoot, 'pages', pagePathFor(route)), page)
    } catch (error) {
      pageFailures.push({ route, error: error instanceof Error ? error.message : String(error) })
    }
  }

  await copyTextTree(path.join(localSpecRoot, 'specs'), path.join(paymentAuthRoot, 'specs'))
  for (const sourceFile of ['README.md', 'STYLE.md', 'CONTRIBUTING.md', 'LICENSE.md']) {
    await copyTextFile(path.join(localSpecRoot, sourceFile), path.join(paymentAuthRoot, sourceFile))
  }

  const paymentAuthIndex = await fetchText(`${paymentAuthBase}/`, 'text/html')
  await writeText(path.join(paymentAuthRoot, 'rendered/index.html'), paymentAuthIndex)

  const specBasenames = await listSpecBasenames()
  const specTxtFailures = []
  for (const basename of specBasenames) {
    try {
      const specText = await fetchText(`${paymentAuthBase}/${basename}.txt`, 'text/plain')
      await writeText(path.join(paymentAuthRoot, 'rendered/txt', `${basename}.txt`), specText)
    } catch (error) {
      specTxtFailures.push({
        basename,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  const mppxReadme = await readFile(path.resolve(repoRoot, '../projects/repos/mppx/README.md'), 'utf8')
  const mppxAgent = await readFile(path.resolve(repoRoot, '../projects/repos/mppx/AGENTS.md'), 'utf8')
  await writeText(path.join(outputRoot, 'mppx/README.md'), mppxReadme)
  await writeText(path.join(outputRoot, 'mppx/AGENTS.md'), mppxAgent)

  const manifest = {
    schema: 'openagents.mpp_docs_mirror.v1',
    generatedAt: new Date().toISOString(),
    sources: {
      mppDev: mppBase,
      mppDevMcp: `${mppBase}/.well-known/mcp.json`,
      paymentauth: paymentAuthBase,
      localSpecRepo: path.relative(repoRoot, localSpecRoot),
      localMppxRepo: '../projects/repos/mppx',
    },
    mirrored: {
      mppDevPages: routes.length - pageFailures.length,
      mppDevPageFailures: pageFailures,
      mppDevSkillFiles: skillFiles,
      mppDevSkillFailures: skillFailures,
      paymentAuthSpecs: specBasenames.length - specTxtFailures.length,
      paymentAuthSpecFailures: specTxtFailures,
    },
  }
  await writeText(path.join(outputRoot, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)

  const readme = `# MPP Docs Mirror

Generated by \`node scripts/sync-mpp-docs.mjs\`.

This mirror keeps the OpenAgents MPP integration work independent of transient
web access while preserving source pointers.

## Sources

- \`mpp-dev/\`: Markdown docs, llms files, MCP discovery card, and agent skill
  files from \`${mppBase}\`.
- \`paymentauth/specs/\`: canonical Markdown specs copied from
  \`${path.relative(repoRoot, path.join(localSpecRoot, 'specs'))}\`.
- \`paymentauth/rendered/\`: rendered \`${paymentAuthBase}\` index and TXT specs.
- \`mppx/\`: SDK README and agent contract from \`../projects/repos/mppx\`.

Do not edit mirrored files by hand. Update the source repositories or rerun the
sync script.
`
  await writeText(path.join(outputRoot, 'README.md'), readme)

  if (pageFailures.length > 0 || specTxtFailures.length > 0 || skillFailures.length > 0) {
    console.warn(JSON.stringify({ pageFailures, skillFailures, specTxtFailures }, null, 2))
  }
  console.log(`Mirrored MPP docs to ${path.relative(repoRoot, outputRoot)}`)
}

main().catch(error => {
  console.error(error)
  process.exitCode = 1
})
