import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const fixtureDir = import.meta.dir
const script = readFileSync(join(fixtureDir, 'run-khala-tb2.sh'), 'utf8')
const readme = readFileSync(join(fixtureDir, 'README.md'), 'utf8')
const replication = readFileSync(
  join(fixtureDir, 'replication-and-path-to-beat.md'),
  'utf8',
)
const measuredSummary = readFileSync(
  join(fixtureDir, 'measured-run-summary.json'),
  'utf8',
)

describe('run-khala-tb2 public-safe reporting guard', () => {
  test('does not print API key-derived material', () => {
    const publicOutputStatements = script
      .split('\n')
      .filter(line => /^\s*(echo|printf|print\()/.test(line))
      .join('\n')

    expect(publicOutputStatements).not.toContain('${KEY')
    expect(publicOutputStatements).not.toMatch(/key acquired \(prefix/i)
  })

  test('does not bake current backing-lane claims into runner summary output', () => {
    const summaryBlock = script.slice(script.indexOf('summary={'))

    expect(summaryBlock).toContain(
      'backing model/lane intentionally not asserted',
    )
    expect(summaryBlock).not.toMatch(
      /served_model|supply_lane|fireworks|deepseek|non-GLM tool-caller/i,
    )
    expect(measuredSummary).toContain(
      'backing model/lane intentionally not asserted',
    )
    expect(measuredSummary).not.toMatch(
      /served_model|supply_lane|fireworks|deepseek|non-GLM tool-caller/i,
    )
  })

  test('docs keep dated measurements separate from current serving claims', () => {
    expect(readme).toMatch(/public\s+`openagents\/khala`\s+is a\s+router/)
    expect(readme).toContain('Historical snapshot')
    expect(readme).not.toContain(
      'We serve this exact checkpoint on Hydralisk behind `openagents/khala`',
    )

    expect(replication).toContain('2026-06-26 public route snapshot')
    expect(replication).toContain('does not assert the current backing lane')
  })
})
