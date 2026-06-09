#!/usr/bin/env bun

import { spawnSync } from 'node:child_process'

const OMEGA_EFFECT_VERSION = '4.0.0-beta.70'

const npmView = (specifier, ...fields) => {
  const result = spawnSync('npm', ['view', specifier, ...fields, '--json'], {
    encoding: 'utf8',
  })

  if (result.status !== 0) {
    throw new Error(
      result.stderr.trim() ||
        `npm view ${specifier} ${fields.join(' ')} failed`,
    )
  }

  return JSON.parse(result.stdout)
}

const metadata = {
  effectBeta: npmView('effect@beta', 'version'),
  effectCfLatest: npmView('effect-cf@latest', 'version', 'peerDependencies'),
  effectDistTags: npmView('effect', 'dist-tags'),
  effectVitestBeta: npmView(
    '@effect/vitest@beta',
    'version',
    'peerDependencies',
  ),
  effectVitestLatest: npmView(
    '@effect/vitest@latest',
    'version',
    'peerDependencies',
  ),
  foldkitDevtoolsLatest: npmView(
    '@foldkit/devtools-mcp@latest',
    'version',
    'peerDependencies',
  ),
  foldkitLatest: npmView('foldkit@latest', 'version', 'peerDependencies'),
  foldkitVitePluginLatest: npmView(
    '@foldkit/vite-plugin@latest',
    'version',
    'peerDependencies',
  ),
  platformBrowserBeta: npmView(
    '@effect/platform-browser@beta',
    'version',
    'peerDependencies',
  ),
  platformBrowserOmega: npmView(
    `@effect/platform-browser@${OMEGA_EFFECT_VERSION}`,
    'version',
    'peerDependencies',
  ),
}

const peerEffect = entry => entry.peerDependencies?.effect ?? '(none)'

const foldkitFamilyPeersOnOmega =
  peerEffect(metadata.foldkitLatest) === OMEGA_EFFECT_VERSION &&
  metadata.foldkitLatest.peerDependencies?.['@effect/platform-browser'] ===
    OMEGA_EFFECT_VERSION &&
  peerEffect(metadata.foldkitDevtoolsLatest) === OMEGA_EFFECT_VERSION &&
  peerEffect(metadata.foldkitVitePluginLatest) === OMEGA_EFFECT_VERSION

const effectCfPeersOnOmega = peerEffect(metadata.effectCfLatest).includes(
  OMEGA_EFFECT_VERSION,
)

const effectVitestCanInstallOnOmega =
  peerEffect(metadata.effectVitestLatest).includes(OMEGA_EFFECT_VERSION) ||
  peerEffect(metadata.effectVitestBeta).includes(OMEGA_EFFECT_VERSION)

console.log('Effect upgrade metadata report')
console.log('')
console.log(`Omega Effect line: effect@${OMEGA_EFFECT_VERSION}`)
console.log(`effect dist-tags: ${JSON.stringify(metadata.effectDistTags)}`)
console.log(`effect@beta: ${metadata.effectBeta}`)
console.log('')
console.log('Latest package peer metadata:')
console.log(
  `- effect-cf@${metadata.effectCfLatest.version}: effect ${peerEffect(
    metadata.effectCfLatest,
  )}`,
)
console.log(
  `- foldkit@${metadata.foldkitLatest.version}: effect ${peerEffect(
    metadata.foldkitLatest,
  )}; @effect/platform-browser ${
    metadata.foldkitLatest.peerDependencies?.['@effect/platform-browser'] ??
    '(none)'
  }`,
)
console.log(
  `- @foldkit/devtools-mcp@${metadata.foldkitDevtoolsLatest.version}: effect ${peerEffect(
    metadata.foldkitDevtoolsLatest,
  )}`,
)
console.log(
  `- @foldkit/vite-plugin@${metadata.foldkitVitePluginLatest.version}: effect ${peerEffect(
    metadata.foldkitVitePluginLatest,
  )}`,
)
console.log(
  `- @effect/platform-browser@beta ${metadata.platformBrowserBeta.version}: effect ${peerEffect(
    metadata.platformBrowserBeta,
  )}`,
)
console.log(
  `- @effect/vitest@latest ${metadata.effectVitestLatest.version}: effect ${peerEffect(
    metadata.effectVitestLatest,
  )}; vitest ${
    metadata.effectVitestLatest.peerDependencies?.vitest ?? '(none)'
  }`,
)
console.log(
  `- @effect/vitest@beta ${metadata.effectVitestBeta.version}: effect ${peerEffect(
    metadata.effectVitestBeta,
  )}; vitest ${metadata.effectVitestBeta.peerDependencies?.vitest ?? '(none)'}`,
)
console.log('')
console.log('Upgrade gates:')
console.log(
  `- effect-cf still matches Omega beta 70: ${
    effectCfPeersOnOmega ? 'yes' : 'no'
  }`,
)
console.log(
  `- Foldkit/devtools/vite-plugin/platform-browser can align on Omega beta 70: ${
    foldkitFamilyPeersOnOmega ? 'yes' : 'no'
  }`,
)
console.log(
  `- @effect/vitest can be installed on Omega beta 70: ${
    effectVitestCanInstallOnOmega ? 'yes' : 'no'
  }`,
)
console.log('')

if (foldkitFamilyPeersOnOmega && effectVitestCanInstallOnOmega) {
  console.log(
    'Action: metadata is aligned; plan a same-change dependency upgrade and remove topology exceptions after full verification.',
  )
} else {
  console.log(
    'Action: keep the current topology exception and plain Vitest service tests until the package peer lines align.',
  )
}
