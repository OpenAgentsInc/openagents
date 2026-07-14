#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync, realpathSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, isAbsolute, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// 2026-07-14 (`build: unify completion checks on Effect beta.94`,
// 1da7bbc2af): the whole repo — root catalog, Omega worker/web, companion
// @effect/* packages, nostr-effect (github pin), and the vendored Effect
// Native workspaces — now runs ONE effect line. The previous three-line
// topology (Omega beta.70 / vendored beta.94 / isolated Nostr Effect 3.19.8)
// and its isolation rules are retired; this guard now enforces the single
// unified line plus the named external peer exceptions below.
const OMEGA_EFFECT_VERSION = '4.0.0-beta.94'
const EFFECT_NATIVE_EFFECT_VERSION = '4.0.0-beta.94'
const FOLDKIT_EFFECT_EXCEPTION_VERSION = '4.0.0-beta.66'
// effect-cf@0.13.1 still PEERS on ^4.0.0-beta.70 upstream; pnpm resolves that
// range to the unified installed beta.94.
const EFFECT_CF_PEER_RANGE = '^4.0.0-beta.70'
const NOSTR_EFFECT_PINNED_SPEC =
  'https://github.com/OpenAgentsInc/nostr-effect/archive/2bb57870eeeb214ed80ca8a275292f5e4dd89863.tar.gz'
const EFFECT_CF_VERSION = '0.13.1'
const EFFECT_VITEST_VERSION = OMEGA_EFFECT_VERSION
const EFFECT_VITEST_DEFERRED_NOTE =
  `@effect/vitest is allowed only on the unified effect@${OMEGA_EFFECT_VERSION} line; latest stable 0.29.0 still peers on effect ^3.21.0.`

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url))
const APP_ROOT = resolve(SCRIPT_DIR, '..')
const REPO_ROOT = resolve(APP_ROOT, '..', '..')
const REPO_ROOT_REAL_PATH = realpathSync(REPO_ROOT)
const PNPM_WORKSPACE_PATH = join(REPO_ROOT, 'pnpm-workspace.yaml')
const PNPM_LOCK_PATH = join(REPO_ROOT, 'pnpm-lock.yaml')
const EFFECT_NATIVE_VENDOR_PATH = join(
  APP_ROOT,
  'packages',
  'effect-native-vendor.json',
)
const REPO_NODE_MODULES = join(REPO_ROOT, 'node_modules')
const EXPECTED_REPO_NODE_MODULES_REAL_PATH = join(
  REPO_ROOT_REAL_PATH,
  'node_modules',
)
const PNPM_PACKAGE_STORE = join(REPO_NODE_MODULES, '.pnpm')

const EXPECTED_EFFECT_NATIVE_PACKAGES = new Map([
  ['apps/openagents.com/packages/effect-native-core', '@effect-native/core'],
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
const isPathWithin = (parent, candidate) => {
  const pathFromParent = relative(parent, candidate)
  return (
    pathFromParent !== '' &&
    pathFromParent !== '..' &&
    !pathFromParent.startsWith(
      `..${process.platform === 'win32' ? '\\' : '/'}`,
    ) &&
    !isAbsolute(pathFromParent)
  )
}

const pathContainmentPolicyCases = [
  {
    candidate: join(REPO_NODE_MODULES, '.pnpm', 'effect', 'package.json'),
    shouldAllow: true,
  },
  {
    candidate: join(
      dirname(REPO_ROOT),
      'node_modules',
      'effect',
      'package.json',
    ),
    shouldAllow: false,
  },
]
const pathContainmentPolicyTestProblems = pathContainmentPolicyCases.flatMap(
  ({ candidate, shouldAllow }) => {
    const allowed = isPathWithin(REPO_NODE_MODULES, candidate)
    return allowed === shouldAllow
      ? []
      : [
          `Internal local-install containment policy regression for ${candidate}: expected ${shouldAllow ? 'allow' : 'deny'}`,
        ]
  },
)

const localInstallProblems = []
let localNodeModulesRealPath = null

if (!existsSync(REPO_NODE_MODULES)) {
  localInstallProblems.push(
    `Missing local ${repoRelative(REPO_NODE_MODULES)}; run pnpm install --frozen-lockfile in this checkout before checking Effect topology`,
  )
} else {
  localNodeModulesRealPath = realpathSync(REPO_NODE_MODULES)
  if (localNodeModulesRealPath !== EXPECTED_REPO_NODE_MODULES_REAL_PATH) {
    localInstallProblems.push(
      `${repoRelative(REPO_NODE_MODULES)} resolves outside this checkout to ${localNodeModulesRealPath}; ancestor or shared installs are not topology evidence`,
    )
  }
}

if (!existsSync(PNPM_PACKAGE_STORE)) {
  localInstallProblems.push(
    `Missing local ${repoRelative(PNPM_PACKAGE_STORE)} package store; a guard run without this checkout's frozen install is invalid`,
  )
} else if (localNodeModulesRealPath !== null) {
  const packageStoreRealPath = realpathSync(PNPM_PACKAGE_STORE)
  if (
    !isPathWithin(EXPECTED_REPO_NODE_MODULES_REAL_PATH, packageStoreRealPath)
  ) {
    localInstallProblems.push(
      `${repoRelative(PNPM_PACKAGE_STORE)} resolves outside this checkout to ${packageStoreRealPath}`,
    )
  }
}

const requireLocalInstalledPackagePath = (path, dependencyName) => {
  if (localNodeModulesRealPath === null) {
    throw new Error(
      `Cannot validate ${dependencyName}: this checkout has no local node_modules`,
    )
  }

  const installedRealPath = realpathSync(path)
  if (!isPathWithin(EXPECTED_REPO_NODE_MODULES_REAL_PATH, installedRealPath)) {
    throw new Error(
      `${dependencyName} resolved outside this checkout's node_modules to ${installedRealPath}`,
    )
  }
  return path
}

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
    .map(
      path => `Effect Native vendor manifest is missing exact package ${path}`,
    ),
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

const rootCatalogEffectVersion = readFileSync(PNPM_WORKSPACE_PATH, 'utf8').match(
  /^  effect:\s*([^\s#]+)\s*$/m,
)?.[1]
const rootCatalogProblems =
  rootCatalogEffectVersion === OMEGA_EFFECT_VERSION
    ? []
    : [
        `pnpm-workspace.yaml catalog.effect is ${String(rootCatalogEffectVersion)}; expected ${OMEGA_EFFECT_VERSION}`,
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

const effectVitestReferences = dependencyEntries.filter(
  entry => entry.name === '@effect/vitest',
)

const unexpectedEffectVitestReferences = effectVitestReferences.filter(
  entry => entry.version !== EFFECT_VITEST_VERSION,
)

const findOwningPackageJson = (entryPath, expectedPackageName) => {
  let directory = dirname(entryPath)

  while (
    directory.startsWith(REPO_ROOT) ||
    directory.includes('node_modules')
  ) {
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

const resolveDependencyPackageJson = (
  contextPackageJsonPath,
  dependencyName,
) => {
  const contextRequire = createRequire(contextPackageJsonPath)
  const directPackageJson = join(
    dirname(contextPackageJsonPath),
    'node_modules',
    ...dependencyName.split('/'),
    'package.json',
  )

  if (existsSync(directPackageJson)) {
    return requireLocalInstalledPackagePath(directPackageJson, dependencyName)
  }

  try {
    return requireLocalInstalledPackagePath(
      contextRequire.resolve(`${dependencyName}/package.json`),
      dependencyName,
    )
  } catch (packageJsonError) {
    try {
      return requireLocalInstalledPackagePath(
        findOwningPackageJson(
          contextRequire.resolve(dependencyName),
          dependencyName,
        ),
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

// The Nostr relay rides the unified effect line via a reviewed github pin of
// nostr-effect. Guard the exact pin so a drive-by respec (registry range,
// different commit) cannot silently change the relay's protocol/runtime line.
const nostrRelayPackageJson = readJson(
  join(REPO_ROOT, 'apps', 'nostr-relay', 'package.json'),
)
const nostrRelayPinProblems =
  nostrRelayPackageJson.dependencies?.['nostr-effect'] ===
  NOSTR_EFFECT_PINNED_SPEC
    ? []
    : [
        `apps/nostr-relay/package.json dependencies.nostr-effect is ${String(nostrRelayPackageJson.dependencies?.['nostr-effect'])}; expected the reviewed pin ${NOSTR_EFFECT_PINNED_SPEC}`,
      ]

const installedExternalPullerPolicyProblem = ({
  effectVersion,
  packageName,
  packageVersion,
}) => {
  const packageRef = `${packageName}@${String(packageVersion)}`

  if (effectVersion === OMEGA_EFFECT_VERSION) {
    return null
  }

  return `${packageRef} resolves unexpected installed Effect runtime line ${effectVersion}; the repo runs one unified effect@${OMEGA_EFFECT_VERSION} line`
}

const installedExternalPullerPolicyCases = [
  {
    input: {
      effectVersion: OMEGA_EFFECT_VERSION,
      packageName: 'effect-cf',
      packageVersion: EFFECT_CF_VERSION,
    },
    shouldAllow: true,
  },
  {
    input: {
      effectVersion: '3.19.8',
      packageName: 'unrelated-effect3-consumer',
      packageVersion: '1.0.0',
    },
    shouldAllow: false,
  },
  {
    input: {
      effectVersion: '4.0.0-beta.70',
      packageName: 'stale-previous-line-consumer',
      packageVersion: '1.0.0',
    },
    shouldAllow: false,
  },
]

const installedExternalPullerPolicyTestProblems =
  installedExternalPullerPolicyCases.flatMap(({ input, shouldAllow }) => {
    const allowed = installedExternalPullerPolicyProblem(input) === null
    return allowed === shouldAllow
      ? []
      : [
          `Internal installed-puller policy regression for ${input.packageName}@${input.packageVersion} on effect@${input.effectVersion}: expected ${shouldAllow ? 'allow' : 'deny'}`,
        ]
  })

const installedExternalEffectPullers = []
const installedExternalEffectProblems = []
const installedPackageJsonPaths =
  collectInstalledPackageJsonPaths(PNPM_PACKAGE_STORE)

if (installedPackageJsonPaths.length === 0) {
  localInstallProblems.push(
    `Local ${repoRelative(PNPM_PACKAGE_STORE)} contains no installed package manifests; ancestor resolution is not accepted`,
  )
}

for (const packageJsonPath of installedPackageJsonPaths) {
  const packageJson = readJson(packageJsonPath)
  // Published devDependencies are build metadata, not an installed runtime
  // edge. Only installed dependency and peer declarations are pullers.
  const runtimeDependencySections = [
    packageJson.dependencies ?? {},
    packageJson.peerDependencies ?? {},
    packageJson.optionalDependencies ?? {},
  ]
  const declaresEffect = runtimeDependencySections.some(dependencies =>
    Object.hasOwn(dependencies, 'effect'),
  )

  if (declaresEffect && packageJson.name !== 'effect') {
    try {
      const resolvedEffect = resolveEffectFromPackage(packageJsonPath)
      installedExternalEffectPullers.push({
        effectVersion: resolvedEffect.version,
        packageName: packageJson.name ?? repoRelative(packageJsonPath),
        packageVersion: packageJson.version,
      })

      const policyProblem = installedExternalPullerPolicyProblem({
        effectVersion: resolvedEffect.version,
        packageName: packageJson.name ?? repoRelative(packageJsonPath),
        packageVersion: packageJson.version,
      })
      if (policyProblem !== null) {
        installedExternalEffectProblems.push(policyProblem)
      }
    } catch (error) {
      installedExternalEffectProblems.push(
        `${packageJson.name ?? repoRelative(packageJsonPath)} declares Effect but its installed package context could not resolve effect/package.json: ${error instanceof Error ? error.message : String(error)}`,
      )
    }
  }
}

for (const [
  packagePath,
  expectedPackageName,
] of EXPECTED_EFFECT_NATIVE_PACKAGES) {
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
    context: 'apps/openagents.com/apps/start/package.json',
    dependency: 'effect',
    expectedDependencyVersion: OMEGA_EFFECT_VERSION,
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'openagents.com Start -> Effect',
  },
  {
    context: 'apps/nostr-relay/package.json',
    dependency: 'nostr-effect',
    expectedDependencyVersion: '0.0.12',
    expectedEffectVersion: OMEGA_EFFECT_VERSION,
    label: 'Nostr relay -> reviewed nostr-effect pin',
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

const runPnpmWhyEffect = () => {
  const result = spawnSync('pnpm', ['why', 'effect', '--recursive', '--depth', '0'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'pnpm why effect failed')
  }

  return result.stdout.trim()
}

// `pnpm why` is deliberately report-only. The authoritative checks above
// resolve `effect/package.json` from each package's own installed context.
const effectWhyOutput = runPnpmWhyEffect()

const trackedInstalledVersions = [
  ['unified repo effect line', OMEGA_EFFECT_VERSION],
  ['vendored Effect Native runtime', EFFECT_NATIVE_EFFECT_VERSION],
  ['effect-cf', EFFECT_CF_VERSION],
  ['foldkit', '0.102.1'],
  [
    '@openagentsinc/nostr-relay nostr-effect pin',
    `${NOSTR_EFFECT_PINNED_SPEC} on the unified line`,
  ],
  [
    '@effect/vitest',
    `${EFFECT_VITEST_VERSION} where Effect service tests use it.effect`,
  ],
]

const pnpmLock = readFileSync(PNPM_LOCK_PATH, 'utf8')

const lockExpectations = [
  {
    description: `effect-cf@${EFFECT_CF_VERSION}`,
    pattern: `effect-cf@${EFFECT_CF_VERSION}:`,
  },
  {
    description: `the unified installed effect@${OMEGA_EFFECT_VERSION}`,
    pattern: `effect@${OMEGA_EFFECT_VERSION}:`,
  },
  {
    description: `foldkit@0.102.1 peers on effect ${FOLDKIT_EFFECT_EXCEPTION_VERSION}`,
    pattern: 'foldkit@0.102.1:',
  },
]

const lockProblems = lockExpectations
  .filter(expectation => !pnpmLock.includes(expectation.pattern))
  .map(expectation => `pnpm-lock.yaml is missing ${expectation.description}`)

const problems = [
  ...pathContainmentPolicyTestProblems,
  ...localInstallProblems,
  ...effectNativeManifestProblems,
  ...rootCatalogProblems,
  ...directEffectVersionProblems,
  ...directCompanionVersionProblems,
  ...resolvedEffectProblems,
  ...nostrRelayPinProblems,
  ...installedExternalPullerPolicyTestProblems,
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
console.log(`Unified repository line: effect@${OMEGA_EFFECT_VERSION}`)
console.log('Vendored Effect Native workspaces: unified on the repository line')
console.log(`effect-cf line: effect-cf@${EFFECT_CF_VERSION}`)
console.log(
  `Nostr relay: ${NOSTR_EFFECT_PINNED_SPEC} on effect@${OMEGA_EFFECT_VERSION}`,
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
  `- installed external Effect pullers checked: ${installedExternalEffectPullers.length}; every one resolves effect@${OMEGA_EFFECT_VERSION}`,
)
console.log('')
console.log(
  'Installed effect dependency tree from `pnpm why effect --recursive --depth 0` (diagnostic only; peer placement is not resolution authority):',
)
console.log(effectWhyOutput)
console.log('')

if (problems.length > 0) {
  console.error('Effect topology check failed:')
  problems.forEach(problem => console.error(`- ${problem}`))
  process.exit(1)
}

console.log('Effect topology check passed.')
