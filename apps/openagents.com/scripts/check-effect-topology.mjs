#!/usr/bin/env bun

import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const OMEGA_EFFECT_VERSION = '4.0.0-beta.70'
const FOLDKIT_EFFECT_EXCEPTION_VERSION = '4.0.0-beta.66'
const ISOLATED_NOSTR_RELAY_EFFECT_VERSION = '3.19.8'
const EFFECT_CF_VERSION = '0.13.1'
const EFFECT_VITEST_VERSION = OMEGA_EFFECT_VERSION
const EFFECT_VITEST_DEFERRED_NOTE =
  '@effect/vitest is allowed only on the repo-aligned 4.0.0-beta.70 line; latest stable 0.29.0 still peers on effect ^3.21.0.'

const workspaceRoots = ['apps', 'workers', 'packages']

const packageJsonPaths = [
  'package.json',
  ...workspaceRoots.flatMap(root =>
    readdirSync(root, { withFileTypes: true })
      .filter(entry => entry.isDirectory())
      .map(entry => join(root, entry.name, 'package.json'))
      .filter(path => existsSync(path)),
  ),
]

const readJson = path => JSON.parse(readFileSync(path, 'utf8'))

const packageJsons = packageJsonPaths.map(path => ({
  json: readJson(path),
  path,
}))

const dependencySections = json => [
  ['dependencies', json.dependencies ?? {}],
  ['devDependencies', json.devDependencies ?? {}],
  ['peerDependencies', json.peerDependencies ?? {}],
  ['optionalDependencies', json.optionalDependencies ?? {}],
]

const dependencyEntries = packageJsons.flatMap(({ json, path }) =>
  dependencySections(json).flatMap(([section, dependencies]) =>
    Object.entries(dependencies).map(([name, version]) => ({
      name,
      packageName: json.name ?? path,
      path,
      section,
      version,
    })),
  ),
)

const expectedExactVersions = new Map([
  ['effect', OMEGA_EFFECT_VERSION],
  ['@effect/platform-browser', OMEGA_EFFECT_VERSION],
  ['@effect/sql-d1', OMEGA_EFFECT_VERSION],
  ['@effect/sql-sqlite-do', OMEGA_EFFECT_VERSION],
  ['effect-cf', EFFECT_CF_VERSION],
])

const directVersionProblems = dependencyEntries
  .filter(entry => expectedExactVersions.has(entry.name))
  .filter(entry => entry.version !== expectedExactVersions.get(entry.name))
  .map(
    entry =>
      `${entry.path} ${entry.section}.${entry.name} is ${entry.version}; expected ${expectedExactVersions.get(entry.name)}`,
  )

const effectVitestReferences = dependencyEntries.filter(
  entry => entry.name === '@effect/vitest',
)

const unexpectedEffectVitestReferences = effectVitestReferences.filter(
  entry => entry.version !== EFFECT_VITEST_VERSION,
)

const runBunWhyEffect = () => {
  const result = spawnSync('bun', ['pm', 'why', 'effect'], {
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'bun pm why effect failed')
  }

  return result.stdout.trim()
}

const effectWhyOutput = runBunWhyEffect()

const effectSections = effectWhyOutput
  .split(/\n(?=effect@)/)
  .map(block => {
    const [header = '', ...bodyLines] = block.split('\n')

    return {
      body: bodyLines.join('\n'),
      version: header.replace(/^effect@/, ''),
    }
  })
  .filter(section => section.version !== '')

const observedVersions = effectSections.map(section => section.version)

const requiredIsolatedNostrRelayPullers = [
  '@openagentsinc/nostr-relay@workspace',
  'nostr-effect@0.0.12',
]

const allowedEffectSection = section => {
  if (
    section.version === OMEGA_EFFECT_VERSION ||
    section.version === FOLDKIT_EFFECT_EXCEPTION_VERSION
  ) {
    return true
  }

  if (section.version !== ISOLATED_NOSTR_RELAY_EFFECT_VERSION) {
    return false
  }

  return requiredIsolatedNostrRelayPullers.every(puller =>
    section.body.includes(puller),
  )
}

const unexpectedEffectSections = effectSections.filter(
  section => !allowedEffectSection(section),
)

const omegaSection = effectSections.find(
  section => section.version === OMEGA_EFFECT_VERSION,
)

const foldkitExceptionSection = effectSections.find(
  section => section.version === FOLDKIT_EFFECT_EXCEPTION_VERSION,
)
const foldkitEffectAligned = foldkitExceptionSection === undefined

const trackedInstalledVersions = [
  ['effect', OMEGA_EFFECT_VERSION],
  ['effect-cf', EFFECT_CF_VERSION],
  ['foldkit', '0.102.1'],
  ['@foldkit/devtools-mcp', '0.9.0'],
  ['@foldkit/vite-plugin', '0.7.0'],
  [
    'isolated @openagentsinc/nostr-relay effect line',
    `${ISOLATED_NOSTR_RELAY_EFFECT_VERSION} via nostr-effect@0.0.12 only`,
  ],
  [
    '@effect/platform-browser',
    foldkitEffectAligned
      ? `${OMEGA_EFFECT_VERSION} direct and via resolved Foldkit peer tree`
      : `${OMEGA_EFFECT_VERSION} direct, ${FOLDKIT_EFFECT_EXCEPTION_VERSION} via Foldkit`,
  ],
  ['@effect/vitest', `${EFFECT_VITEST_VERSION} where Effect service tests use it.effect`],
]

const requiredOmegaPullers = [
  '@openagentsinc/api-worker@workspace',
  '@openagentsinc/autopilot-web@workspace',
  '@openagentsinc/provider-account-schema@workspace',
  '@openagentsinc/sync-client@workspace',
  '@openagentsinc/sync-schema@workspace',
  '@openagentsinc/sync-worker@workspace',
  'effect-cf@0.13.1',
]

const requiredFoldkitExceptionPullers = [
  '@foldkit/devtools-mcp@0.9.0',
  '@foldkit/vite-plugin@0.7.0',
  'foldkit@0.102.1',
  '@effect/platform-browser@4.0.0-beta.66',
]

const missingOmegaPullers =
  omegaSection === undefined
    ? requiredOmegaPullers
    : requiredOmegaPullers.filter(puller => !omegaSection.body.includes(puller))

const missingFoldkitExceptionPullers =
  foldkitEffectAligned
    ? []
    : foldkitExceptionSection === undefined
    ? requiredFoldkitExceptionPullers
    : requiredFoldkitExceptionPullers.filter(
        puller => !foldkitExceptionSection.body.includes(puller),
      )

const bunLock = readFileSync('bun.lock', 'utf8')

const lockExpectations = [
  {
    description: `effect-cf@${EFFECT_CF_VERSION} peers on effect ^${OMEGA_EFFECT_VERSION}`,
    pattern: `"effect-cf": ["effect-cf@${EFFECT_CF_VERSION}"`,
  },
  {
    description: `effect-cf peerDependencies.effect is ^${OMEGA_EFFECT_VERSION}`,
    pattern: `"effect": "^${OMEGA_EFFECT_VERSION}"`,
  },
  {
    description: `foldkit@0.102.1 peers on effect ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
    pattern: `"foldkit": ["foldkit@0.102.1"`,
  },
  {
    description: `Foldkit peerDependencies.effect is ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
    pattern: `"effect": "${FOLDKIT_EFFECT_EXCEPTION_VERSION}"`,
  },
  ...(foldkitEffectAligned
    ? []
    : [
        {
          description: `Foldkit nested platform browser is ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
          pattern: `"foldkit/@effect/platform-browser": ["@effect/platform-browser@${FOLDKIT_EFFECT_EXCEPTION_VERSION}"`,
        },
        {
          description: `@foldkit/devtools-mcp uses ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
          pattern: `"@foldkit/devtools-mcp/effect": ["effect@${FOLDKIT_EFFECT_EXCEPTION_VERSION}"`,
        },
        {
          description: `@foldkit/vite-plugin uses ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
          pattern: `"@foldkit/vite-plugin/effect": ["effect@${FOLDKIT_EFFECT_EXCEPTION_VERSION}"`,
        },
      ]),
]

const lockProblems = lockExpectations
  .filter(expectation => !bunLock.includes(expectation.pattern))
  .map(expectation => `bun.lock is missing ${expectation.description}`)

const problems = [
  ...directVersionProblems,
  ...unexpectedEffectSections.map(
    section =>
      `Unexpected installed Effect runtime line ${section.version}; allowed lines are ${OMEGA_EFFECT_VERSION}, the temporary Foldkit exception ${FOLDKIT_EFFECT_EXCEPTION_VERSION}, and isolated @openagentsinc/nostr-relay via nostr-effect@0.0.12 on ${ISOLATED_NOSTR_RELAY_EFFECT_VERSION}`,
  ),
  ...(omegaSection === undefined
    ? [
        `Missing required Omega/effect-cf Effect runtime line ${OMEGA_EFFECT_VERSION}`,
      ]
    : []),
  ...missingOmegaPullers.map(
    puller =>
      `Omega/effect-cf Effect line ${OMEGA_EFFECT_VERSION} is missing puller ${puller}`,
  ),
  ...missingFoldkitExceptionPullers.map(
    puller =>
      `Foldkit Effect exception ${FOLDKIT_EFFECT_EXCEPTION_VERSION} is missing expected puller ${puller}`,
  ),
  ...lockProblems,
  ...effectSections
    .filter(section => section.version === ISOLATED_NOSTR_RELAY_EFFECT_VERSION)
    .flatMap(section =>
      requiredIsolatedNostrRelayPullers
        .filter(puller => !section.body.includes(puller))
        .map(
          puller =>
            `Isolated Nostr relay Effect line ${ISOLATED_NOSTR_RELAY_EFFECT_VERSION} is missing expected puller ${puller}`,
        ),
    ),
  ...unexpectedEffectVitestReferences.map(
    entry =>
      `${entry.path} ${entry.section}.${entry.name} is ${entry.version}; ${EFFECT_VITEST_DEFERRED_NOTE}`,
  ),
]

console.log('Effect topology report')
console.log('')
console.log(`Omega/effect-cf line: effect@${OMEGA_EFFECT_VERSION}`)
console.log(
  foldkitEffectAligned
    ? `Foldkit Effect line: aligned to effect@${OMEGA_EFFECT_VERSION}`
    : `Temporary Foldkit exception: effect@${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
)
console.log(`effect-cf line: effect-cf@${EFFECT_CF_VERSION}`)
console.log(
  `Isolated Nostr relay line: effect@${ISOLATED_NOSTR_RELAY_EFFECT_VERSION} via nostr-effect@0.0.12 only`,
)
console.log(`@effect/vitest deferred: ${EFFECT_VITEST_DEFERRED_NOTE}`)
console.log('')
console.log('Tracked installed package versions:')
trackedInstalledVersions.forEach(([name, version]) =>
  console.log(`- ${name}: ${version}`),
)
console.log('')
console.log('Installed effect dependency tree from `bun pm why effect`:')
console.log(effectWhyOutput)
console.log('')

if (problems.length > 0) {
  console.error('Effect topology check failed:')
  problems.forEach(problem => console.error(`- ${problem}`))
  process.exit(1)
}

console.log('Effect topology check passed.')
