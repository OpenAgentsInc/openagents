#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const repoRoot = resolve(import.meta.dirname, '..')
const companionFiles = ['AGENTS.md', 'AGENTS-CORE.md', 'INSTALL.md', 'SURFACES.md', 'HEARTBEAT.md', 'RULES.md', 'skill.json']

const main = async () => {
  await Promise.all(
    companionFiles.map(async fileName => {
      const sourcePath = resolve(repoRoot, 'docs/live', fileName)
      const outputPath = resolve(repoRoot, 'apps/web/public', fileName)
      const contents = await readFile(sourcePath, 'utf8')

      await mkdir(dirname(outputPath), { recursive: true })
      await writeFile(outputPath, contents)
      console.log(`Synced ${sourcePath} -> ${outputPath}`)
    }),
  )
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
