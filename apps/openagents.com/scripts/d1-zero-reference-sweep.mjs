#!/usr/bin/env bun

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const appRoot = resolve(dirname(scriptPath), '..')

export const defaultMigrationRoot = 'workers/api/migrations'
export const defaultScanRoots = [
  'workers/api/src',
  'packages',
  'apps/web/src',
  'scripts',
  '../../apps',
  '../../clients',
  '../../packages',
  '../../scripts',
]

export const confirmedZeroReferenceTables = new Map([
  [
    'forum_actor_forum_trust',
    {
      issueRef: '#8379',
      reason:
        'Wave 1 audit found no production reader/writer; fixture references do not keep the table live.',
    },
  ],
  [
    'forum_trust_edges',
    {
      issueRef: '#8379',
      reason:
        'Wave 1 audit found no production reader/writer; fixture references do not keep the table live.',
    },
  ],
  [
    'gym_agentcl_eval_gain_metrics',
    {
      issueRef: '#8380',
      reason:
        'Wave 1 audit found no production reader/writer after the AgentCL Vertex runner and eval registry removal.',
    },
  ],
  [
    'gym_agentcl_eval_phase_metrics',
    {
      issueRef: '#8380',
      reason:
        'Wave 1 audit found no production reader/writer after the AgentCL Vertex runner and eval registry removal.',
    },
  ],
  [
    'gym_agentcl_eval_prompt_mutations',
    {
      issueRef: '#8380',
      reason:
        'Wave 1 audit found no production reader/writer after the AgentCL Vertex runner and eval registry removal.',
    },
  ],
  [
    'gym_agentcl_eval_run_state_events',
    {
      issueRef: '#8380',
      reason:
        'Wave 1 audit found no production reader/writer after the AgentCL Vertex runner and eval registry removal.',
    },
  ],
  [
    'gym_agentcl_eval_runs',
    {
      issueRef: '#8380',
      reason:
        'Wave 1 audit found no production reader/writer after the AgentCL Vertex runner and eval registry removal.',
    },
  ],
])

export const manualRetainedTables = new Map()

const ignoredDirs = new Set([
  '.git',
  '.next',
  '.turbo',
  '.wrangler',
  'build',
  'coverage',
  'dist',
  'node_modules',
  'target',
])

const sourceFilePattern = /\.(?:cjs|js|json|mjs|ts|tsx)$/
const createTablePattern =
  /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[`"[]?([A-Za-z_][A-Za-z0-9_]*)[`"\]]?/gi

const escapeRegExp = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const read = path => readFileSync(path, 'utf8')

const lineNumberAt = (lineStarts, index) => {
  let low = 0
  let high = lineStarts.length - 1

  while (low <= high) {
    const middle = Math.floor((low + high) / 2)
    const start = lineStarts[middle]
    const next = lineStarts[middle + 1] ?? Number.POSITIVE_INFINITY

    if (index < start) {
      high = middle - 1
    } else if (index >= next) {
      low = middle + 1
    } else {
      return middle + 1
    }
  }

  return 1
}

const lineStartsFor = text => {
  const starts = [0]

  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === '\n') {
      starts.push(index + 1)
    }
  }

  return starts
}

export const stripJavaScriptComments = text => {
  let output = ''
  let state = 'code'
  let escaped = false

  for (let index = 0; index < text.length; index += 1) {
    const char = text[index]
    const next = text[index + 1]

    if (state === 'line-comment') {
      if (char === '\n') {
        output += '\n'
        state = 'code'
      } else {
        output += ' '
      }
      continue
    }

    if (state === 'block-comment') {
      if (char === '*' && next === '/') {
        output += '  '
        index += 1
        state = 'code'
      } else {
        output += char === '\n' ? '\n' : ' '
      }
      continue
    }

    if (state === 'single-quote') {
      output += char
      if (escaped) {
        escaped = false
      } else if (char === '\\') {
        escaped = true
      } else if (char === "'") {
        state = 'code'
      }
      continue
    }

    if (state === 'double-quote') {
      output += char
      if (escaped) {
        escaped = false
      } else if (char === '\\') {
        escaped = true
      } else if (char === '"') {
        state = 'code'
      }
      continue
    }

    if (state === 'template') {
      output += char
      if (escaped) {
        escaped = false
      } else if (char === '\\') {
        escaped = true
      } else if (char === '`') {
        state = 'code'
      }
      continue
    }

    if (char === '/' && next === '/') {
      output += '  '
      index += 1
      state = 'line-comment'
      continue
    }

    if (char === '/' && next === '*') {
      output += '  '
      index += 1
      state = 'block-comment'
      continue
    }

    if (char === "'") {
      state = 'single-quote'
      escaped = false
    } else if (char === '"') {
      state = 'double-quote'
      escaped = false
    } else if (char === '`') {
      state = 'template'
      escaped = false
    }

    output += char
  }

  return output
}

const listFiles = root => {
  let entries

  try {
    entries = readdirSync(root, { withFileTypes: true })
  } catch {
    return []
  }

  return entries.flatMap(entry => {
    const path = join(root, entry.name)

    if (entry.isDirectory()) {
      return ignoredDirs.has(entry.name) ? [] : listFiles(path)
    }

    return entry.isFile() ? [path] : []
  })
}

const existingRoots = (root, roots) =>
  roots.map(scanRoot => resolve(root, scanRoot)).filter(path => existsSync(path))

export const isTestOrFixturePath = path =>
  /(?:^|\/)(?:__fixtures__|fixtures|test-fixtures)(?:\/|$)/.test(path) ||
  /(?:^|\/)tests?\//.test(path) ||
  /\.(?:spec|test)\.[cm]?[jt]sx?$/.test(path)

export const extractTableDeclarations = (sql, sourcePath) => {
  const lineStarts = lineStartsFor(sql)

  return [...sql.matchAll(createTablePattern)].map(match => ({
    line: lineNumberAt(lineStarts, match.index ?? 0),
    sourcePath,
    table: match[1],
  }))
}

const migrationDeclarations = ({ root, migrationRoot }) =>
  listFiles(resolve(root, migrationRoot))
    .filter(path => path.endsWith('.sql'))
    .sort((left, right) => left.localeCompare(right))
    .flatMap(path =>
      extractTableDeclarations(read(path), relative(root, path)).map(
        declaration => ({
          ...declaration,
          migration: relative(resolve(root, migrationRoot), path),
        }),
      ),
    )

const referencePatternFor = tableNames =>
  new RegExp(
    `\\b(${tableNames
      .slice()
      .sort((left, right) => right.length - left.length || left.localeCompare(right))
      .map(escapeRegExp)
      .join('|')})\\b`,
    'g',
  )

const addReference = (references, table, reference) => {
  const existing = references.get(table) ?? []
  const previous = existing.find(item => item.path === reference.path)

  if (previous) {
    previous.lines = [...new Set([...previous.lines, ...reference.lines])].slice(
      0,
      8,
    )
    references.set(table, existing)
    return
  }

  references.set(table, [...existing, reference])
}

const scanReferenceFiles = ({ root, scanRoots, tableNames }) => {
  const productionReferences = new Map()
  const testReferences = new Map()

  if (tableNames.length === 0) {
    return { productionReferences, scannedFiles: [], testReferences }
  }

  const pattern = referencePatternFor(tableNames)
  const scannedFiles = existingRoots(root, scanRoots)
    .flatMap(listFiles)
    .filter((path, index, files) => files.indexOf(path) === index)
    .filter(path => sourceFilePattern.test(path))
    .filter(path => !path.endsWith('d1-zero-reference-sweep.mjs'))
    .filter(path => !path.endsWith('d1-zero-reference-sweep.test.ts'))
    .sort((left, right) => left.localeCompare(right))

  for (const path of scannedFiles) {
    const relativePath = relative(root, path)
    const text = stripJavaScriptComments(read(path))
    const lineStarts = lineStartsFor(text)
    const byTable = new Map()

    pattern.lastIndex = 0
    for (const match of text.matchAll(pattern)) {
      const table = match[1]
      const lines = byTable.get(table) ?? []
      lines.push(lineNumberAt(lineStarts, match.index ?? 0))
      byTable.set(table, lines)
    }

    const target = isTestOrFixturePath(relativePath)
      ? testReferences
      : productionReferences

    for (const [table, lines] of byTable.entries()) {
      addReference(target, table, {
        lines: [...new Set(lines)].slice(0, 8),
        path: relativePath,
      })
    }
  }

  return { productionReferences, scannedFiles, testReferences }
}

const tableStatus = ({
  confirmedZeroReferences,
  manualRetentions,
  productionRefs,
  table,
}) => {
  if (productionRefs.length > 0) {
    return 'referenced'
  }

  if (manualRetentions.has(table)) {
    return 'manually_retained'
  }

  if (confirmedZeroReferences.has(table)) {
    return 'confirmed_zero_reference'
  }

  return 'migration_only'
}

const tableNote = ({
  confirmedZeroReferences,
  manualRetentions,
  productionRefs,
  status,
  table,
  testRefs,
}) => {
  if (status === 'referenced' && confirmedZeroReferences.has(table)) {
    return `audit seed has current production reference; ${confirmedZeroReferences.get(table).issueRef} must remove it before drop`
  }

  if (status === 'confirmed_zero_reference') {
    const detail =
      testRefs.length > 0 ? '; only test/fixture references remain' : ''
    return `${confirmedZeroReferences
      .get(table)
      .reason.replace(/[.]\s*$/, '')}${detail}`
  }

  if (status === 'manually_retained') {
    const retention = manualRetentions.get(table)
    return `${retention.issueRef}: ${retention.reason}`
  }

  if (productionRefs.length === 0 && testRefs.length > 0) {
    return 'no production references; test/fixture references only'
  }

  if (productionRefs.length === 0) {
    return 'no references outside migrations found by the lexical sweep'
  }

  return ''
}

export const buildD1TableSweep = ({
  confirmedZeroReferences = confirmedZeroReferenceTables,
  manualRetentions = manualRetainedTables,
  migrationRoot = defaultMigrationRoot,
  root = appRoot,
  scanRoots = defaultScanRoots,
} = {}) => {
  const declarations = migrationDeclarations({ migrationRoot, root })
  const declarationsByTable = declarations.reduce((groups, declaration) => {
    groups.set(declaration.table, [
      ...(groups.get(declaration.table) ?? []),
      declaration,
    ])
    return groups
  }, new Map())
  const tableNames = [...declarationsByTable.keys()].sort((left, right) =>
    left.localeCompare(right),
  )
  const { productionReferences, scannedFiles, testReferences } =
    scanReferenceFiles({
      root,
      scanRoots,
      tableNames,
    })

  const tables = tableNames.map(table => {
    const productionRefs = productionReferences.get(table) ?? []
    const testRefs = testReferences.get(table) ?? []
    const status = tableStatus({
      confirmedZeroReferences,
      manualRetentions,
      productionRefs,
      table,
    })

    const normalizedStatus =
      status === 'migration_only' && testRefs.length > 0
        ? 'test_only'
        : status

    return {
      declarations: declarationsByTable.get(table) ?? [],
      note: tableNote({
        confirmedZeroReferences,
        manualRetentions,
        productionRefs,
        status: normalizedStatus,
        table,
        testRefs,
      }),
      productionRefs,
      status: normalizedStatus,
      table,
      testRefs,
    }
  })

  const statusCounts = Object.fromEntries(
    [
      'referenced',
      'confirmed_zero_reference',
      'test_only',
      'migration_only',
      'manually_retained',
    ].map(status => [
      status,
      tables.filter(table => table.status === status).length,
    ]),
  )

  return {
    declarations,
    scanRoots,
    scannedFiles: scannedFiles.map(path => relative(root, path)),
    statusCounts,
    summary: {
      createTableStatements: declarations.length,
      scannedProductionFiles: scannedFiles
        .map(path => relative(root, path))
        .filter(path => !isTestOrFixturePath(path)).length,
      scannedTestFiles: scannedFiles
        .map(path => relative(root, path))
        .filter(isTestOrFixturePath).length,
      uniqueTableNames: tableNames.length,
    },
    tables,
  }
}

const formatReferenceList = references => {
  if (references.length === 0) {
    return '-'
  }

  const shown = references
    .slice(0, 3)
    .map(ref => `${ref.path}:${ref.lines.join(',')}`)
  const extra = references.length - shown.length

  return extra > 0 ? `${shown.join('<br>')}<br>+${extra} more` : shown.join('<br>')
}

const markdownEscape = value =>
  String(value)
    .replaceAll('|', '\\|')
    .replaceAll('\n', '<br>')

export const formatMarkdownReport = sweep => {
  const lines = [
    '# D1 Zero-Reference Sweep Report',
    '',
    'Generated by `bun run d1:zero-reference-sweep -- --format markdown` from the local checkout. The report is deterministic: it does not embed wall-clock time.',
    '',
    '## Summary',
    '',
    `- CREATE TABLE statements scanned: ${sweep.summary.createTableStatements}`,
    `- Unique table names classified: ${sweep.summary.uniqueTableNames}`,
    `- Production files scanned: ${sweep.summary.scannedProductionFiles}`,
    `- Test/fixture files scanned: ${sweep.summary.scannedTestFiles}`,
    '',
    '| Status | Count | Meaning |',
    '|---|---:|---|',
    `| referenced | ${sweep.statusCounts.referenced} | At least one non-test code reference remains. |`,
    `| confirmed_zero_reference | ${sweep.statusCounts.confirmed_zero_reference} | Wave 1 seed table with no production reference in the current scan. Test fixtures may still mention it. |`,
    `| test_only | ${sweep.statusCounts.test_only} | No production reference; at least one test or fixture reference remains. |`,
    `| migration_only | ${sweep.statusCounts.migration_only} | No code reference outside migrations was found. Requires domain review before a drop migration. |`,
    `| manually_retained | ${sweep.statusCounts.manually_retained} | No production reference, but explicitly retained with an issue-backed reason. |`,
    '',
    '## Sweep Rules',
    '',
    '- Migration inventory is extracted from `CREATE TABLE` statements under `workers/api/migrations`.',
    '- Production references are lexical table-name hits in non-test code under `apps/openagents.com` plus root `apps/`, `clients/`, `packages/`, and `scripts/`.',
    '- Test references are lexical hits in `.test.*`, `.spec.*`, `test/`, `tests/`, `fixtures/`, `__fixtures__/`, or `test-fixtures/` paths.',
    '- JavaScript and TypeScript comments are ignored before scanning so explanatory comments do not keep a table live.',
    '- The sweep is intentionally conservative: dynamic SQL/table-name construction can be a false negative, and table names mentioned in runtime strings can be a false positive. Before dropping any table, rerun this script, inspect the row, and grep the table name directly.',
    '',
    '## Wave 1 Seed Results',
    '',
    '| Table | Status | Production refs | Test refs | Notes |',
    '|---|---|---|---|---|',
    ...sweep.tables
      .filter(table => confirmedZeroReferenceTables.has(table.table))
      .map(table =>
        [
          markdownEscape(table.table),
          table.status,
          formatReferenceList(table.productionRefs),
          formatReferenceList(table.testRefs),
          markdownEscape(table.note),
        ].join(' | '),
      )
      .map(row => `| ${row} |`),
    '',
    '## Full Table Inventory',
    '',
    '| Table | Status | Declarations | Production refs | Test refs | Notes |',
    '|---|---|---|---|---|---|',
    ...sweep.tables.map(table =>
      [
        markdownEscape(table.table),
        table.status,
        table.declarations
          .map(declaration => `${declaration.sourcePath}:${declaration.line}`)
          .join('<br>'),
        formatReferenceList(table.productionRefs),
        formatReferenceList(table.testRefs),
        markdownEscape(table.note),
      ]
        .map(markdownEscape)
        .join(' | '),
    ).map(row => `| ${row} |`),
    '',
  ]

  return `${lines.join('\n')}`
}

const formatSummary = sweep =>
  [
    'D1 zero-reference sweep',
    `CREATE TABLE statements scanned: ${sweep.summary.createTableStatements}`,
    `Unique table names classified: ${sweep.summary.uniqueTableNames}`,
    `Production files scanned: ${sweep.summary.scannedProductionFiles}`,
    `Test/fixture files scanned: ${sweep.summary.scannedTestFiles}`,
    '',
    'Status counts:',
    ...Object.entries(sweep.statusCounts).map(
      ([status, count]) => `  ${status}: ${count}`,
    ),
    '',
    'Wave 1 seed results:',
    ...sweep.tables
      .filter(table => confirmedZeroReferenceTables.has(table.table))
      .map(
        table =>
          `  ${table.table}: ${table.status} ` +
          `(prod=${table.productionRefs.length}, test=${table.testRefs.length})`,
      ),
  ].join('\n')

const parseArgs = argv => {
  const options = {
    format: 'summary',
    output: null,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]

    if (arg === '--format') {
      options.format = argv[index + 1]
      index += 1
    } else if (arg.startsWith('--format=')) {
      options.format = arg.slice('--format='.length)
    } else if (arg === '--output') {
      options.output = argv[index + 1]
      index += 1
    } else if (arg.startsWith('--output=')) {
      options.output = arg.slice('--output='.length)
    } else if (arg === '--help' || arg === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${arg}`)
    }
  }

  return options
}

const printHelp = () => {
  console.log(`Usage: bun scripts/d1-zero-reference-sweep.mjs [--format summary|json|markdown] [--output path]

Classifies D1 tables declared in Worker migrations by current non-test code
references. Default output is a concise summary; use --format markdown for the
committed KS-8.19 evidence report.`)
}

const isMain =
  import.meta.url === `file://${process.argv[1]}` ||
  process.argv[1]?.endsWith('d1-zero-reference-sweep.mjs')

if (isMain) {
  try {
    const options = parseArgs(process.argv.slice(2))

    if (options.help) {
      printHelp()
      process.exit(0)
    }

    if (!['json', 'markdown', 'summary'].includes(options.format)) {
      throw new Error(`Unsupported format: ${options.format}`)
    }

    const sweep = buildD1TableSweep()
    const body =
      options.format === 'json'
        ? `${JSON.stringify(sweep, null, 2)}\n`
        : options.format === 'markdown'
          ? formatMarkdownReport(sweep)
          : `${formatSummary(sweep)}\n`

    if (options.output) {
      const outputPath = resolve(appRoot, options.output)
      mkdirSync(dirname(outputPath), { recursive: true })
      writeFileSync(outputPath, body)
    } else {
      process.stdout.write(body)
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }
}
