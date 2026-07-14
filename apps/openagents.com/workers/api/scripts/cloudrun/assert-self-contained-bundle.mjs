#!/usr/bin/env node
import { builtinModules } from 'node:module'
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const builtins = new Set([
  ...builtinModules,
  ...builtinModules.map(name => `node:${name}`),
])
const runtimeProvided = new Set([
  'cloudflare:workers',
  '@cloudflare/playwright',
])

export const externalRuntimeSpecifiers = source => {
  const specifiers = new Set()
  const patterns = [
    /^[\t ]*import\s.+?\sfrom\s*['"]([^'"]+)['"]/gm,
    /^[\t ]*import\s*['"]([^'"]+)['"]/gm,
  ]

  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      const specifier = match[1]
      if (
        specifier !== undefined &&
        !specifier.startsWith('.') &&
        !specifier.startsWith('/') &&
        !builtins.has(specifier) &&
        !runtimeProvided.has(specifier)
      ) {
        specifiers.add(specifier)
      }
    }
  }

  return [...specifiers].sort()
}

const packageNameForSpecifier = specifier =>
  specifier.startsWith('@')
    ? specifier.split('/').slice(0, 2).join('/')
    : specifier.split('/')[0]

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const bundleDirectory = resolve(process.argv[2] ?? 'dist-cloudrun')
  const failures = readdirSync(bundleDirectory)
    .filter(name => name.endsWith('.mjs'))
    .flatMap(name => {
      const modulePath = resolve(bundleDirectory, name)
      return externalRuntimeSpecifiers(readFileSync(modulePath, 'utf8')).flatMap(
        specifier =>
          existsSync(
            resolve(
              bundleDirectory,
              'node_modules',
              packageNameForSpecifier(specifier),
            ),
          )
            ? []
            : [`${name}: ${specifier}`],
      )
    })

  if (failures.length > 0) {
    console.error(
      'FATAL: Cloud Run bundle has unresolved runtime dependencies:',
    )
    for (const failure of failures) console.error(`  ${failure}`)
    process.exit(1)
  }
}
