#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const repoRoot = resolve(import.meta.dirname, '..')
const workspaceRoot = resolve(repoRoot, '../..')
const companionFiles = ['AGENTS.md', 'AGENTS-CORE.md', 'INSTALL.md', 'SURFACES.md', 'PYLON.md', 'SITES.md', 'HEARTBEAT.md', 'RULES.md', 'skill.json']
const canonicalChatSkillPath = resolve(
  workspaceRoot,
  '.agents/skills/public-nostr-chat/SKILL.md',
)
const bootstrapStart = '<!-- public-nostr-chat-bootstrap:start -->'
const bootstrapEnd = '<!-- public-nostr-chat-bootstrap:end -->'

const main = async () => {
  const canonicalChatSkill = await readFile(canonicalChatSkillPath, 'utf8')
  const bootstrapStartIndex = canonicalChatSkill.indexOf(bootstrapStart)
  const bootstrapEndIndex = canonicalChatSkill.indexOf(bootstrapEnd)
  if (
    bootstrapStartIndex < 0 ||
    bootstrapEndIndex < 0 ||
    bootstrapEndIndex <= bootstrapStartIndex
  ) {
    throw new Error('The canonical public chat skill has invalid bootstrap markers')
  }
  const bootstrap = canonicalChatSkill
    .slice(bootstrapStartIndex + bootstrapStart.length, bootstrapEndIndex)
    .trim()
  const bootstrapLines = bootstrap
    .split('\n')
    .map(line => `  ${JSON.stringify(line)},`)
    .join('\n')
  const generatedBootstrap = `// Generated from .agents/skills/public-nostr-chat/SKILL.md.\n// Run apps/openagents.com/scripts/sync-live-agent-doc.mjs after a skill change.\nexport const publicNostrChatAgentBootstrap = [\n${bootstrapLines}\n].join("\\n");\n`

  await Promise.all([
    ...companionFiles.map(async fileName => {
      const sourcePath = resolve(repoRoot, 'docs/live', fileName)
      const outputPath = resolve(repoRoot, 'apps/start/public', fileName)
      const contents = await readFile(sourcePath, 'utf8')

      await mkdir(dirname(outputPath), { recursive: true })
      await writeFile(outputPath, contents)
      console.log(`Synced ${sourcePath} -> ${outputPath}`)
    }),
    ...[
      resolve(repoRoot, 'docs/live/skills/AGENT_CHAT.md'),
      resolve(repoRoot, 'apps/start/public/skills/AGENT_CHAT.md'),
    ].map(async outputPath => {
      await mkdir(dirname(outputPath), { recursive: true })
      await writeFile(outputPath, canonicalChatSkill)
      console.log(`Synced ${canonicalChatSkillPath} -> ${outputPath}`)
    }),
    (async () => {
      const outputPath = resolve(
        repoRoot,
        'apps/start/src/generated/public-nostr-chat-bootstrap.ts',
      )
      await mkdir(dirname(outputPath), { recursive: true })
      await writeFile(outputPath, generatedBootstrap)
      console.log(`Generated ${outputPath} from ${canonicalChatSkillPath}`)
    })(),
  ])
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
