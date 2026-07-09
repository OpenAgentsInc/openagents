#!/usr/bin/env bun

import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const OMEGA_EFFECT_VERSION = '4.0.0-beta.70'
const EFFECT_NATIVE_EFFECT_VERSION = '4.0.0-beta.94'
const FOLDKIT_EFFECT_EXCEPTION_VERSION = '4.0.0-beta.66'
const ISOLATED_NOSTR_RELAY_EFFECT_VERSION = '3.19.8'
const EFFECT_CF_VERSION = '0.13.1'
const EFFECT_VITEST_VERSION = OMEGA_EFFECT_VERSION
const EFFECT_VITEST_DEFERRED_NOTE =
  '@effect/vitest is allowed only on the repo-aligned 4.0.0-beta.70 line; latest stable 0.29.0 still peers on effect ^3.21.0.'

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url))
const APP_ROOT = resolve(SCRIPT_DIR, '..')
const REPO_ROOT = resolve(APP_ROOT, '..', '..')
const ROOT_PACKAGE_JSON_PATH = join(REPO_ROOT, 'package.json')
const BUN_LOCK_PATH = join(REPO_ROOT, 'bun.lock')
const EFFECT_NATIVE_VENDOR_PATH = join(
  APP_ROOT,
  'packages',
  'effect-native-vendor.json',
)
const BUN_PACKAGE_STORE = join(REPO_ROOT, 'node_modules', '.bun')

const EXPECTED_EFFECT_NATIVE_PACKAGES = new Map([
  [
    'apps/openagents.com/packages/effect-native-core',
    '@effect-native/core',
  ],
  [
    'apps/openagents.com/packages/effect-native-render-dom',
    '@effect-native/render-dom',
  ],
  [
    'apps/openagents.com/packages/effect-native-render-rn',
    '@effect-native/render-rn',
  ],
  [
    'apps/openagents.com/packages/effect-native-tokens',
    '@effect-native/tokens',
  ],
])

const SKIPPED_PACKAGE_SCAN_DIRECTORIES = new Set([
  '.git',
  '.sarah',
  '.turbo',
  'coverage',
  'dist',
  'node_modules',
])

const readJson = path => JSON.parse(readFileSync(path, 'utf8'))
const repoRelative = path => relative(REPO_ROOT, path)

const collectPackageJsonPaths = root => {
  const packageJsonPaths = []

  const visit = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIPPED_PACKAGE_SCAN_DIRECTORIES.has(entry.name)) {
          visit(join(directory, entry.name))
        }
        continue
      }

      if (entry.isFile() && entry.name === 'package.json') {
        packageJsonPaths.push(join(directory, entry.name))
      }
    }
  }

  visit(root)
  return packageJsonPaths.sort()
}

const packageJsonPaths = collectPackageJsonPaths(REPO_ROOT)
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
      packageName: json.name ?? repoRelative(path),
      path,
      section,
      version,
    })),
  ),
)

const effectNativeVendor = readJson(EFFECT_NATIVE_VENDOR_PATH)
const declaredEffectNativePackages = new Set(
  effectNativeVendor.vendoredPackages ?? [],
)
const expectedEffectNativePackagePaths = new Set(
  EXPECTED_EFFECT_NATIVE_PACKAGES.keys(),
)

const effectNativeManifestProblems = [
  ...[...expectedEffectNativePackagePaths]
    .filter(path => !declaredEffectNativePackages.has(path))
    .map(path => `Effect Native vendor manifest is missing exact package ${path}`),
  ...[...declaredEffectNativePackages]
    .filter(path => !expectedEffectNativePackagePaths.has(path))
    .map(
      path =>
        `Effect Native vendor manifest has unexpected runtime package ${path}; review its Effect isolation before adding it`,
    ),
]

const effectNativePackageJsonPaths = new Set(
  [...expectedEffectNativePackagePaths].map(path =>
    join(REPO_ROOT, path, 'package.json'),
  ),
)

const expectedEffectVersionForManifest = path =>
  effectNativePackageJsonPaths.has(path)
    ? EFFECT_NATIVE_EFFECT_VERSION
    : OMEGA_EFFECT_VERSION

const rootPackageJson = readJson(ROOT_PACKAGE_JSON_PATH)
const rootCatalogEffectVersion = rootPackageJson.workspaces?.catalog?.effect
const rootCatalogProblems =
  rootCatalogEffectVersion === OMEGA_EFFECT_VERSION
    ? []
    : [
        `package.json workspaces.catalog.effect is ${String(rootCatalogEffectVersion)}; expected ${OMEGA_EFFECT_VERSION}`,
      ]

const expectedExactVersions = new Map([
  ['@effect/platform-browser', OMEGA_EFFECT_VERSION],
  ['@effect/sql-d1', OMEGA_EFFECT_VERSION],
  ['@effect/sql-sqlite-do', OMEGA_EFFECT_VERSION],
  ['effect-cf', EFFECT_CF_VERSION],
])

const directEffectVersionProblems = dependencyEntries
  .filter(entry => entry.name === 'effect')
  .filter(entry => {
    const expected = expectedEffectVersionForManifest(entry.path)
    return entry.version !== expected && entry.version !== 'catalog:'
  })
  .map(entry => {
    const expected = expectedEffectVersionForManifest(entry.path)
    return `${repoRelative(entry.path)} ${entry.section}.effect is ${entry.version}; expected exact ${expected}${expected === OMEGA_EFFECT_VERSION ? ' or catalog:' : ''}`
  })

const directCompanionVersionProblems = dependencyEntries
  .filter(entry => expectedExactVersions.has(entry.name))
  .filter(entry => entry.version !== expectedExactVersions.get(entry.name))
  .map(
    entry =>
      `${repoRelative(entry.path)} ${entry.section}.${entry.name} is ${entry.version}; expected ${expectedExactVersions.get(entry.name)}`,
  )

const isolatedNostrRelayPackageJsonPath = join(
  REPO_ROOT,
  'apps',
  'nostr-relay',
  'package.json',
)
const isolatedNostrRelayPullerProblems = dependencyEntries
  .filter(entry => entry.name === 'nostr-effect' && entry.version === '0.0.12')
  .filter(entry => entry.path !== isolatedNostrRelayPackageJsonPath)
  .map(
    entry =>
      `${repoRelative(entry.path)} unexpectedly pulls isolated nostr-effect@0.0.12; only apps/nostr-relay/package.json may pull that Effect 3 line`,
  )

const effectVitestReferences = dependencyEntries.filter(
  entry => entry.name === '@effect/vitest',
)

const unexpectedEffectVitestReferences = effectVitestReferences.filter(
  entry => entry.version !== EFFECT_VITEST_VERSION,
)

const findOwningPackageJson = (entryPath, expectedPackageName) => {
  let directory = dirname(entryPath)

  while (directory.startsWith(REPO_ROOT) || directory.includes('node_modules')) {
    const candidate = join(directory, 'package.json')
    if (existsSync(candidate)) {
      const candidateJson = readJson(candidate)
      if (candidateJson.name === expectedPackageName) {
        return candidate
      }
    }

    const parent = dirname(directory)
    if (parent === directory) {
      break
    }
    directory = parent
  }

  throw new Error(
    `Could not locate package.json for ${expectedPackageName} from ${entryPath}`,
  )
}

const resolveDependencyPackageJson = (contextPackageJsonPath, dependencyName) => {
  const contextRequire = createRequire(contextPackageJsonPath)

  try {
    return contextRequire.resolve(`${dependencyName}/package.json`)
  } catch (packageJsonError) {
    try {
      return findOwningPackageJson(
        contextRequire.resolve(dependencyName),
        dependencyName,
      )
    } catch {
      throw packageJsonError
    }
  }
}

const resolveEffectFromPackage = contextPackageJsonPath => {
  const effectPackageJsonPath = resolveDependencyPackageJson(
    contextPackageJsonPath,
    'effect',
  )

  return {
    packageJsonPath: effectPackageJsonPath,
    version: readJson(effectPackageJsonPath).version,
  }
}

const directEffectManifests = packageJsons.filter(({ json }) =>
  dependencySections(json).some(([, dependencies]) =>
    Object.hasOwn(dependencies, 'effect'),
  ),
)

const resolvedEffectPullers = []
const resolvedEffectProblems = []

for (const { json, path } of directEffectManifests) {
  const expectedVersion = expectedEffectVersionForManifest(path)

  try {
    const resolved = resolveEffectFromPackage(path)
    resolvedEffectPullers.push({
      expectedVersion,
      packageName: json.name ?? repoRelative(path),
      packagePath: repoRelative(path),
      resolvedVersion: resolved.version,
    })

    if (resolved.version !== expectedVersion) {
      resolvedEffectProblems.push(
        `${repoRelative(path)} resolves effect@${resolved.version}; expected effect@${expectedVersion}`,
      )
    }
  } catch (error) {
    resolvedEffectProblems.push(
      `${repoRelative(path)} could not resolve effect/package.json: ${error instanceof Error ? error.message : String(error)}`,
    )
  }
}

const collectInstalledPackageJsonPaths = root => {
  if (!existsSync(root)) {
    return []
  }

  const packageJsonPaths = []
  for (const storeEntry of readdirSync(root, { withFileTypes: true })) {
    if (!storeEntry.isDirectory()) {
      continue
    }

    const nodeModulesPath = join(root, storeEntry.name, 'node_modules')
    if (!existsSync(nodeModulesPath)) {
      continue
    }

    for (const packageEntry of readdirSync(nodeModulesPath, {
      withFileTypes: true,
    })) {
      if (!packageEntry.isDirectory()) {
        continue
      }

      if (packageEntry.name.startsWith('@')) {
        const scopePath = join(nodeModulesPath, packageEntry.name)
        for (const scopedPackageEntry of readdirSync(scopePath, {
          withFileTypes: true,
        })) {
          if (!scopedPackageEntry.isDirectory()) {
            continue
          }
          const packageJsonPath = join(
            scopePath,
            scopedPackageEntry.name,
            'package.json',
          )
          if (existsSync(packageJsonPath)) {
            packageJsonPaths.push(packageJsonPath)
          }
        }
        continue
      }

      const packageJsonPath = join(
        nodeModulesPath,
        packageEntry.name,
        'package.json',
      )
      if (existsSync(packageJsonPath)) {
        packageJsonPaths.push(packageJsonPath)
      }
    }
  }

  return packageJsonPaths
}

const allowedInstalledEffectVersions = new Set([
  OMEGA_EFFECT_VERSION,
  ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
])
const installedExternalEffectPullers = []
const installedExternalEffectProblems = []

for (const packageJsonPath of collectInstalledPackageJsonPaths(BUN_PACKAGE_STORE)) {
  const packageJson = readJson(packageJsonPath)
  // Published devDependencies are build metadata, not an installed runtime
  // edge. Only installed dependency and peer declarations are pullers.
  const declaresEffect = [
    packageJson.dependencies ?? {},
    packageJson.peerDependencies ?? {},
    packageJson.optionalDependencies ?? {},
  ].some(dependencies => Object.hasOwn(dependencies, 'effect'))
  if (!declaresEffect || packageJson.name === 'effect') {
    continue
  }

  try {
    const resolvedEffect = resolveEffectFromPackage(packageJsonPath)
    installedExternalEffectPullers.push({
      effectVersion: resolvedEffect.version,
      packageName: packageJson.name ?? repoRelative(packageJsonPath),
      packageVersion: packageJson.version,
    })

    if (resolvedEffect.version === EFFECT_NATIVE_EFFECT_VERSION) {
      installedExternalEffectProblems.push(
        `${packageJson.name}@${String(packageJson.version)} unexpectedly resolves vendored Effect Native's effect@${EFFECT_NATIVE_EFFECT_VERSION}; only the exact four source-vendored @effect-native/* workspaces may pull that line`,
      )
    } else if (!allowedInstalledEffectVersions.has(resolvedEffect.version)) {
      installedExternalEffectProblems.push(
        `${packageJson.name}@${String(packageJson.version)} resolves unexpected installed Effect runtime line ${resolvedEffect.version}`,
      )
    }
  } catch (error) {
    installedExternalEffectProblems.push(
      `${packageJson.name ?? repoRelative(packageJsonPath)} declares Effect but its installed package context could not resolve effect/package.json: ${error instanceof Error ? error.message : String(error)}`,
    )
  }
}

for (const [packagePath, expectedPackageName] of EXPECTED_EFFECT_NATIVE_PACKAGES) {
  const packageJsonPath = join(REPO_ROOT, packagePath, 'package.json')
  if (!existsSync(packageJsonPath)) {
    effectNativeManifestProblems.push(
      `Missing vendored Effect Native package manifest ${packagePath}/package.json`,
    )
    continue
  }

  const packageJson = readJson(packageJsonPath)
  if (packageJson.name !== expectedPackageName) {
    effectNativeManifestProblems.push(
      `${packagePath}/package.json is named ${String(packageJson.name)}; expected ${expectedPackageName}`,
    )
  }
  if (packageJson.dependencies?.effect !== EFFECT_NATIVE_EFFECT_VERSION) {
    effectNativeManifestProblems.push(
      `${packagePath}/package.json must pin dependencies.effect exactly to ${EFFECT_NATIVE_EFFECT_VERSION}`,
    )
  }
}

const criticalPackageExpectations = [
  {
    context: 'apps/openagents.com/workers/api/package.json',
    dependency: 'effect-cf',
    expectedDependencyVersion: EFFECT_CF_VERSION,
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com Worker -> effect-cf',
  },
  {
    context: 'apps/openagents.com/workers/api/package.json',
    dependency: '@effect/sql-d1',
    expectedDependencyVersion: OMEGA_EFFECT_VERSION,
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com Worker -> @effect/sql-d1',
  },
  {
    context: 'apps/openagents.com/workers/api/package.json',
    dependency: '@effect/sql-sqlite-do',
    expectedDependencyVersion: OMEGA_EFFECT_VERSION,
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com Worker -> @effect/sql-sqlite-do',
  },
  {
    context: 'apps/openagents.com/apps/web/package.json',
    dependency: '@effect/platform-browser',
    expectedDependencyVersion: OMEGA_EFFECT_VERSION,
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com web -> @effect/platform-browser',
  },
  {
    context: 'apps/openagents.com/apps/web/package.json',
    dependency: 'foldkit',
    expectedDependencyVersion: '0.102.1',
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com web -> Foldkit',
  },
  {
    context: 'apps/openagents.com/apps/web/package.json',
    dependency: '@foldkit/devtools-mcp',
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com web -> Foldkit devtools',
  },
  {
    context: 'apps/openagents.com/apps/web/package.json',
    dependency: '@foldkit/vite-plugin',
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com web -> Foldkit Vite plugin',
  },
  {
    context: 'apps/nostr-relay/package.json',
    dependency: 'nostr-effect',
    expectedDependencyVersion: '0.0.12',
    expectedEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
    label: 'isolated Nostr relay -> nostr-effect',
  },
]

const criticalResolutionRows = []
const criticalResolutionProblems = []

for (const expectation of criticalPackageExpectations) {
  const contextPackageJsonPath = join(REPO_ROOT, expectation.context)

  try {
    const dependencyPackageJsonPath = resolveDependencyPackageJson(
      contextPackageJsonPath,
      expectation.dependency,
    )
    const dependencyPackageJson = readJson(dependencyPackageJsonPath)
    const resolvedEffect = resolveEffectFromPackage(dependencyPackageJsonPath)

    criticalResolutionRows.push({
      dependencyVersion: dependencyPackageJson.version,
      effectVersion: resolvedEffect.version,
      label: expectation.label,
    })

    if (
      expectation.expectedDependencyVersion !== undefined &&
      dependencyPackageJson.version !== expectation.expectedDependencyVersion
    ) {
      criticalResolutionProblems.push(
        `${expectation.label} resolves ${expectation.dependency}@${dependencyPackageJson.version}; expected ${expectation.dependency}@${expectation.expectedDependencyVersion}`,
      )
    }

    if (resolvedEffect.version !== expectation.expectedEffectVersion) {
      criticalResolutionProblems.push(
        `${expectation.label} resolves its internal effect to ${resolvedEffect.version}; expected ${expectation.expectedEffectVersion}`,
      )
    }
  } catch (error) {
    criticalResolutionProblems.push(
      `${expectation.label} could not be resolved: ${error instanceof Error ? error.message : String(error)}`,
    )
  }
}

const runBunWhyEffect = () => {
  const result = spawnSync('bun', ['pm', 'why', 'effect'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'bun pm why effect failed')
  }

  return result.stdout.trim()
}

// `bun pm why` is deliberately report-only. Bun 1.3.11 displays compatible
// peer ranges beneath every satisfying Effect line, so effect-cf's
// `^4.0.0-beta.70` peer appears under beta.94 even though its installed
// package-local `effect` symlink resolves to beta.70. The authoritative checks
// above resolve `effect/package.json` from each package's own context.
const effectWhyOutput = runBunWhyEffect()

const trackedInstalledVersions = [
  ['OpenAgents/Omega runtime', OMEGA_EFFECT_VERSION],
  ['vendored Effect Native runtime', EFFECT_NATIVE_EFFECT_VERSION],
  ['effect-cf', EFFECT_CF_VERSION],
  ['foldkit', '0.102.1'],
  [
    'isolated @openagentsinc/nostr-relay effect line',
    `${ISOLATED_NOSTR_RELAY_EFFECT_VERSION} via nostr-effect@0.0.12 only`,
  ],
  ['@effect/vitest', `${EFFECT_VITEST_VERSION} where Effect service tests use it.effect`],
]

const bunLock = readFileSync(BUN_LOCK_PATH, 'utf8')

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
    description: `openagents.com Worker resolves a package-local effect@${OMEGA_EFFECT_VERSION}`,
    pattern: `"@openagentsinc/api-worker/effect": ["effect@${OMEGA_EFFECT_VERSION}"`,
  },
  {
    description: `foldkit@0.102.1 peers on effect ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
    pattern: '"foldkit": ["foldkit@0.102.1"',
  },
  {
    description: `Foldkit peerDependencies.effect is ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
    pattern: `"effect": "${FOLDKIT_EFFECT_EXCEPTION_VERSION}"`,
  },
]

const lockProblems = lockExpectations
  .filter(expectation => !bunLock.includes(expectation.pattern))
  .map(expectation => `bun.lock is missing ${expectation.description}`)

const problems = [
  ...effectNativeManifestProblems,
  ...rootCatalogProblems,
  ...directEffectVersionProblems,
  ...directCompanionVersionProblems,
  ...isolatedNostrRelayPullerProblems,
  ...resolvedEffectProblems,
  ...installedExternalEffectProblems,
  ...criticalResolutionProblems,
  ...lockProblems,
  ...unexpectedEffectVitestReferences.map(
    entry =>
      `${repoRelative(entry.path)} ${entry.section}.${entry.name} is ${entry.version}; ${EFFECT_VITEST_DEFERRED_NOTE}`,
  ),
]

console.log('Effect topology report')
console.log('')
console.log(`OpenAgents/Omega line: effect@${OMEGA_EFFECT_VERSION}`)
console.log(
  `Vendored Effect Native line: effect@${EFFECT_NATIVE_EFFECT_VERSION} (exact four-package source boundary)`,
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
console.log('Resolved Effect boundaries (authoritative):')
console.log(
  `- repo-aligned direct workspaces: ${resolvedEffectPullers.filter(row => row.expectedVersion === OMEGA_EFFECT_VERSION).length} on effect@${OMEGA_EFFECT_VERSION}`,
)
resolvedEffectPullers
  .filter(row => row.expectedVersion === EFFECT_NATIVE_EFFECT_VERSION)
  .forEach(row =>
    console.log(
      `- ${row.packageName}: effect@${row.resolvedVersion} (${row.packagePath})`,
    ),
  )
criticalResolutionRows.forEach(row =>
  console.log(
    `- ${row.label}: dependency@${row.dependencyVersion}, effect@${row.effectVersion}`,
  ),
)
console.log(
  `- installed external Effect pullers checked: ${installedExternalEffectPullers.length} (none may resolve the vendored beta.94 line)`,
)
console.log('')
console.log(
  'Installed effect dependency tree from `bun pm why effect` (diagnostic only; peer placement is not resolution authority):',
)
console.log(effectWhyOutput)
console.log('')

if (problems.length > 0) {
  console.error('Effect topology check failed:')
  problems.forEach(problem => console.error(`- ${problem}`))
  process.exit(1)
}

console.log('Effect topology check passed.')
