#!/usr/bin/env bun

import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync, realpathSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, isAbsolute, join, relative, resolve } from 'node:path'
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
const REPO_ROOT_REAL_PATH = realpathSync(REPO_ROOT)
const ROOT_PACKAGE_JSON_PATH = join(REPO_ROOT, 'package.json')
const BUN_LOCK_PATH = join(REPO_ROOT, 'bun.lock')
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
const BUN_PACKAGE_STORE = join(REPO_NODE_MODULES, '.bun')

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
    candidate: join(REPO_NODE_MODULES, '.bun', 'effect', 'package.json'),
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
    `Missing local ${repoRelative(REPO_NODE_MODULES)}; run bun install --frozen-lockfile in this checkout before checking Effect topology`,
  )
} else {
  localNodeModulesRealPath = realpathSync(REPO_NODE_MODULES)
  if (localNodeModulesRealPath !== EXPECTED_REPO_NODE_MODULES_REAL_PATH) {
    localInstallProblems.push(
      `${repoRelative(REPO_NODE_MODULES)} resolves outside this checkout to ${localNodeModulesRealPath}; ancestor or shared installs are not topology evidence`,
    )
  }
}

if (!existsSync(BUN_PACKAGE_STORE)) {
  localInstallProblems.push(
    `Missing local ${repoRelative(BUN_PACKAGE_STORE)} package store; a guard run without this checkout's frozen install is invalid`,
  )
} else if (localNodeModulesRealPath !== null) {
  const packageStoreRealPath = realpathSync(BUN_PACKAGE_STORE)
  if (
    !isPathWithin(EXPECTED_REPO_NODE_MODULES_REAL_PATH, packageStoreRealPath)
  ) {
    localInstallProblems.push(
      `${repoRelative(BUN_PACKAGE_STORE)} resolves outside this checkout to ${packageStoreRealPath}`,
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

const ISOLATED_NOSTR_EXTERNAL_EFFECT_PULLERS = new Set([
  'nostr-effect@0.0.12',
  '@effect/platform@0.93.5',
  '@effect/schema@0.75.5',
])
const ISOLATED_NOSTR_DEPENDENCY_NAMES = new Set([
  'nostr-effect',
  '@effect/platform',
  '@effect/schema',
])
const ISOLATED_NOSTR_SOURCE_EDGE =
  'apps/nostr-relay/package.json -> nostr-effect@0.0.12'
const REQUIRED_ISOLATED_NOSTR_EXTERNAL_EDGES = new Map([
  ['@effect/platform@0.93.5', 'nostr-effect@0.0.12'],
  ['@effect/schema@0.75.5', 'nostr-effect@0.0.12'],
])

const isIsolatedNostrDependency = ({
  dependencyEffectVersion,
  dependencyPackageRef,
}) =>
  dependencyEffectVersion === ISOLATED_NOSTR_RELAY_EFFECT_VERSION &&
  ISOLATED_NOSTR_EXTERNAL_EFFECT_PULLERS.has(dependencyPackageRef)

const sourceIsolatedNostrEdgePolicyProblem = ({
  declaredDependencyVersion,
  dependencyEffectVersion,
  dependencyPackageRef,
  parentPackagePath,
}) => {
  if (
    !isIsolatedNostrDependency({
      dependencyEffectVersion,
      dependencyPackageRef,
    })
  ) {
    return null
  }

  if (
    parentPackagePath === 'apps/nostr-relay/package.json' &&
    dependencyPackageRef === 'nostr-effect@0.0.12' &&
    declaredDependencyVersion === '0.0.12'
  ) {
    return null
  }

  return `${parentPackagePath} unexpectedly pulls isolated ${dependencyPackageRef} on effect@${dependencyEffectVersion} via manifest spec ${String(declaredDependencyVersion)}; the only source edge into Effect 3 is exact ${ISOLATED_NOSTR_SOURCE_EDGE}`
}

const sourceIsolatedNostrEdgePolicyCases = [
  {
    input: {
      declaredDependencyVersion: '0.0.12',
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: 'nostr-effect@0.0.12',
      parentPackagePath: 'apps/nostr-relay/package.json',
    },
    shouldAllow: true,
  },
  {
    input: {
      declaredDependencyVersion: '^0.0.12',
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: 'nostr-effect@0.0.12',
      parentPackagePath: 'apps/nostr-relay/package.json',
    },
    shouldAllow: false,
  },
  {
    input: {
      declaredDependencyVersion: '0.0.12',
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: 'nostr-effect@0.0.12',
      parentPackagePath: 'apps/openagents.com/workers/api/package.json',
    },
    shouldAllow: false,
  },
  {
    input: {
      declaredDependencyVersion: '0.93.5',
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: '@effect/platform@0.93.5',
      parentPackagePath: 'apps/openagents.com/workers/api/package.json',
    },
    shouldAllow: false,
  },
  {
    input: {
      declaredDependencyVersion: 'github:OpenAgentsInc/nostr-effect#4c52847',
      dependencyEffectVersion: OMEGA_EFFECT_VERSION,
      dependencyPackageRef: 'nostr-effect@0.0.12',
      parentPackagePath: 'apps/openagents.com/workers/api/package.json',
    },
    shouldAllow: true,
  },
]

const sourceIsolatedNostrEdgePolicyTestProblems =
  sourceIsolatedNostrEdgePolicyCases.flatMap(({ input, shouldAllow }) => {
    const allowed = sourceIsolatedNostrEdgePolicyProblem(input) === null
    return allowed === shouldAllow
      ? []
      : [
          `Internal source dependency-edge policy regression for ${input.parentPackagePath} -> ${input.dependencyPackageRef} on effect@${input.dependencyEffectVersion}: expected ${shouldAllow ? 'allow' : 'deny'}`,
        ]
  })

const sourceIsolatedNostrEdgeProblems = []
const observedIsolatedNostrSourceEdges = new Set()

for (const { json, path } of packageJsons) {
  const isolatedDependencyEntries = dependencySections(json).flatMap(
    ([section, dependencies]) =>
      Object.entries(dependencies)
        .filter(([dependencyName]) =>
          ISOLATED_NOSTR_DEPENDENCY_NAMES.has(dependencyName),
        )
        .map(([dependencyName, declaredDependencyVersion]) => ({
          declaredDependencyVersion,
          dependencyName,
          section,
        })),
  )

  for (const {
    declaredDependencyVersion,
    dependencyName,
    section,
  } of isolatedDependencyEntries) {
    try {
      const dependencyPackageJsonPath = resolveDependencyPackageJson(
        path,
        dependencyName,
      )
      const dependencyPackageJson = readJson(dependencyPackageJsonPath)
      const dependencyEffect = resolveEffectFromPackage(
        dependencyPackageJsonPath,
      )
      const dependencyPackageRef = `${dependencyPackageJson.name}@${String(dependencyPackageJson.version)}`
      const parentPackagePath = repoRelative(path)
      const policyProblem = sourceIsolatedNostrEdgePolicyProblem({
        declaredDependencyVersion,
        dependencyEffectVersion: dependencyEffect.version,
        dependencyPackageRef,
        parentPackagePath,
      })

      if (policyProblem !== null) {
        sourceIsolatedNostrEdgeProblems.push(policyProblem)
      } else if (
        parentPackagePath === 'apps/nostr-relay/package.json' &&
        declaredDependencyVersion === '0.0.12' &&
        dependencyPackageRef === 'nostr-effect@0.0.12' &&
        dependencyEffect.version === ISOLATED_NOSTR_RELAY_EFFECT_VERSION
      ) {
        observedIsolatedNostrSourceEdges.add(
          `${parentPackagePath} -> ${dependencyPackageRef}`,
        )
      }
    } catch (error) {
      sourceIsolatedNostrEdgeProblems.push(
        `${repoRelative(path)} ${section}.${dependencyName} could not resolve its installed dependency edge: ${error instanceof Error ? error.message : String(error)}`,
      )
    }
  }
}

if (!observedIsolatedNostrSourceEdges.has(ISOLATED_NOSTR_SOURCE_EDGE)) {
  sourceIsolatedNostrEdgeProblems.push(
    `Isolated Nostr Effect 3 graph is missing required source edge ${ISOLATED_NOSTR_SOURCE_EDGE}`,
  )
}

const installedIsolatedNostrEdgePolicyProblem = ({
  dependencyEffectVersion,
  dependencyPackageRef,
  parentEffectVersion,
  parentPackageRef,
}) => {
  if (
    !isIsolatedNostrDependency({
      dependencyEffectVersion,
      dependencyPackageRef,
    })
  ) {
    return null
  }

  const expectedParent =
    REQUIRED_ISOLATED_NOSTR_EXTERNAL_EDGES.get(dependencyPackageRef)
  if (
    expectedParent === parentPackageRef &&
    parentEffectVersion === ISOLATED_NOSTR_RELAY_EFFECT_VERSION
  ) {
    return null
  }

  return `${parentPackageRef} unexpectedly pulls isolated ${dependencyPackageRef} on effect@${dependencyEffectVersion}; expected only ${expectedParent ?? 'the apps/nostr-relay source workspace'} on the isolated effect@${ISOLATED_NOSTR_RELAY_EFFECT_VERSION} chain`
}

const installedIsolatedNostrEdgePolicyCases = [
  {
    input: {
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: '@effect/platform@0.93.5',
      parentEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      parentPackageRef: 'nostr-effect@0.0.12',
    },
    shouldAllow: true,
  },
  {
    input: {
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: '@effect/platform@0.93.5',
      parentEffectVersion: null,
      parentPackageRef: 'external-main-app-helper@1.0.0',
    },
    shouldAllow: false,
  },
  {
    input: {
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: '@effect/schema@0.75.5',
      parentEffectVersion: OMEGA_EFFECT_VERSION,
      parentPackageRef: 'nostr-effect@0.0.12',
    },
    shouldAllow: false,
  },
  {
    input: {
      dependencyEffectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      dependencyPackageRef: 'nostr-effect@0.0.12',
      parentEffectVersion: OMEGA_EFFECT_VERSION,
      parentPackageRef: 'external-main-app-helper@1.0.0',
    },
    shouldAllow: false,
  },
]

const installedIsolatedNostrEdgePolicyTestProblems =
  installedIsolatedNostrEdgePolicyCases.flatMap(({ input, shouldAllow }) => {
    const allowed = installedIsolatedNostrEdgePolicyProblem(input) === null
    return allowed === shouldAllow
      ? []
      : [
          `Internal installed dependency-edge policy regression for ${input.parentPackageRef} -> ${input.dependencyPackageRef} on effect@${input.dependencyEffectVersion}: expected ${shouldAllow ? 'allow' : 'deny'}`,
        ]
  })

const installedExternalPullerPolicyProblem = ({
  effectVersion,
  packageName,
  packageVersion,
}) => {
  const packageRef = `${packageName}@${String(packageVersion)}`

  if (effectVersion === OMEGA_EFFECT_VERSION) {
    return null
  }

  if (effectVersion === EFFECT_NATIVE_EFFECT_VERSION) {
    return `${packageRef} unexpectedly resolves vendored Effect Native's effect@${EFFECT_NATIVE_EFFECT_VERSION}; only the exact four source-vendored @effect-native/* workspaces may pull that line`
  }

  if (effectVersion === ISOLATED_NOSTR_RELAY_EFFECT_VERSION) {
    return ISOLATED_NOSTR_EXTERNAL_EFFECT_PULLERS.has(packageRef)
      ? null
      : `${packageRef} unexpectedly resolves isolated effect@${ISOLATED_NOSTR_RELAY_EFFECT_VERSION}; only the exact nostr-effect@0.0.12 dependency chain may pull that line`
  }

  return `${packageRef} resolves unexpected installed Effect runtime line ${effectVersion}`
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
      effectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      packageName: 'nostr-effect',
      packageVersion: '0.0.12',
    },
    shouldAllow: true,
  },
  {
    input: {
      effectVersion: ISOLATED_NOSTR_RELAY_EFFECT_VERSION,
      packageName: 'unrelated-effect3-consumer',
      packageVersion: '1.0.0',
    },
    shouldAllow: false,
  },
  {
    input: {
      effectVersion: EFFECT_NATIVE_EFFECT_VERSION,
      packageName: 'external-effect-native-consumer',
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
const installedIsolatedNostrEdgeProblems = []
const observedIsolatedNostrExternalPullers = new Set()
const observedIsolatedNostrExternalEdges = new Set()
const installedPackageJsonPaths =
  collectInstalledPackageJsonPaths(BUN_PACKAGE_STORE)

if (installedPackageJsonPaths.length === 0) {
  localInstallProblems.push(
    `Local ${repoRelative(BUN_PACKAGE_STORE)} contains no installed package manifests; ancestor resolution is not accepted`,
  )
}

for (const packageJsonPath of installedPackageJsonPaths) {
  const packageJson = readJson(packageJsonPath)
  const packageRef = `${packageJson.name ?? repoRelative(packageJsonPath)}@${String(packageJson.version)}`
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
  let parentEffectVersion = null

  if (declaresEffect && packageJson.name !== 'effect') {
    try {
      const resolvedEffect = resolveEffectFromPackage(packageJsonPath)
      parentEffectVersion = resolvedEffect.version
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

      if (resolvedEffect.version === ISOLATED_NOSTR_RELAY_EFFECT_VERSION) {
        observedIsolatedNostrExternalPullers.add(packageRef)
      }
    } catch (error) {
      installedExternalEffectProblems.push(
        `${packageJson.name ?? repoRelative(packageJsonPath)} declares Effect but its installed package context could not resolve effect/package.json: ${error instanceof Error ? error.message : String(error)}`,
      )
    }
  }

  const isolatedDependencyNames = new Set(
    runtimeDependencySections
      .flatMap(dependencies => Object.keys(dependencies))
      .filter(dependencyName =>
        ISOLATED_NOSTR_DEPENDENCY_NAMES.has(dependencyName),
      ),
  )

  for (const dependencyName of isolatedDependencyNames) {
    try {
      const dependencyPackageJsonPath = resolveDependencyPackageJson(
        packageJsonPath,
        dependencyName,
      )
      const dependencyPackageJson = readJson(dependencyPackageJsonPath)
      const dependencyEffect = resolveEffectFromPackage(
        dependencyPackageJsonPath,
      )
      const dependencyPackageRef = `${dependencyPackageJson.name}@${String(dependencyPackageJson.version)}`
      const policyProblem = installedIsolatedNostrEdgePolicyProblem({
        dependencyEffectVersion: dependencyEffect.version,
        dependencyPackageRef,
        parentEffectVersion,
        parentPackageRef: packageRef,
      })

      if (policyProblem !== null) {
        installedIsolatedNostrEdgeProblems.push(policyProblem)
      } else if (
        dependencyEffect.version === ISOLATED_NOSTR_RELAY_EFFECT_VERSION &&
        REQUIRED_ISOLATED_NOSTR_EXTERNAL_EDGES.get(dependencyPackageRef) ===
          packageRef &&
        parentEffectVersion === ISOLATED_NOSTR_RELAY_EFFECT_VERSION
      ) {
        observedIsolatedNostrExternalEdges.add(
          `${packageRef} -> ${dependencyPackageRef}`,
        )
      }
    } catch (error) {
      installedIsolatedNostrEdgeProblems.push(
        `${packageRef} declares ${dependencyName} but its installed dependency edge could not be resolved: ${error instanceof Error ? error.message : String(error)}`,
      )
    }
  }
}

const missingIsolatedNostrExternalEdgeProblems = [
  ...REQUIRED_ISOLATED_NOSTR_EXTERNAL_EDGES,
]
  .map(
    ([dependencyPackageRef, parentPackageRef]) =>
      `${parentPackageRef} -> ${dependencyPackageRef}`,
  )
  .filter(edge => !observedIsolatedNostrExternalEdges.has(edge))
  .map(
    edge =>
      `Isolated Nostr Effect 3 graph is missing required installed dependency edge ${edge}`,
  )

const missingIsolatedNostrExternalPullerProblems = [
  ...ISOLATED_NOSTR_EXTERNAL_EFFECT_PULLERS,
]
  .filter(puller => !observedIsolatedNostrExternalPullers.has(puller))
  .map(
    puller =>
      `Isolated Nostr Effect 3 chain is missing expected installed puller ${puller}`,
  )

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
  [
    '@effect/vitest',
    `${EFFECT_VITEST_VERSION} where Effect service tests use it.effect`,
  ],
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
  ...pathContainmentPolicyTestProblems,
  ...localInstallProblems,
  ...effectNativeManifestProblems,
  ...rootCatalogProblems,
  ...directEffectVersionProblems,
  ...directCompanionVersionProblems,
  ...resolvedEffectProblems,
  ...sourceIsolatedNostrEdgePolicyTestProblems,
  ...sourceIsolatedNostrEdgeProblems,
  ...installedExternalPullerPolicyTestProblems,
  ...installedExternalEffectProblems,
  ...installedIsolatedNostrEdgePolicyTestProblems,
  ...installedIsolatedNostrEdgeProblems,
  ...missingIsolatedNostrExternalEdgeProblems,
  ...missingIsolatedNostrExternalPullerProblems,
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
  `- installed external Effect pullers checked: ${installedExternalEffectPullers.length} (beta.70 or the exact three-package Nostr beta3 chain; none may resolve vendored beta.94)`,
)
console.log(
  `- isolated Nostr source edge checked: ${ISOLATED_NOSTR_SOURCE_EDGE}`,
)
console.log(
  `- isolated Nostr installed edges checked: ${[...observedIsolatedNostrExternalEdges].sort().join(', ')}`,
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
