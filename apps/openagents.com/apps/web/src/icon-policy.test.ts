import { Option, Schema as S } from 'effect'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { extname, join, relative } from 'node:path'
import { describe, expect, test } from 'vitest'

import { IconName } from './icon'

const sourceRoot = join(process.cwd(), 'src')
const generatedIconFile = 'icon.ts'
const iconLiteralPattern = /\bicon:\s*'([^']+)'/g
const forbiddenRawIconSources = [
  {
    label: 'raw inline SVG',
    pattern: /<svg\b/,
  },
  {
    label: 'raw InnerHTML icon rendering',
    pattern: /\bh\.InnerHTML\(/,
  },
  {
    label: 'lucide icon dependency',
    pattern: /from ['"]lucide-react['"]/,
  },
  {
    label: 'react-icons dependency',
    pattern: /from ['"]react-icons\//,
  },
  {
    label: 'iconify dependency',
    pattern: /from ['"]@iconify\//,
  },
] as const

const sourceFilesUnder = (dir: string): ReadonlyArray<string> =>
  readdirSync(dir).flatMap(entry => {
    const path = join(dir, entry)

    return statSync(path).isDirectory() ? sourceFilesUnder(path) : [path]
  })

const productionSourceFile = (path: string): boolean =>
  ['.ts', '.tsx'].includes(extname(path)) &&
  !path.endsWith('.test.ts') &&
  !path.endsWith('.story.test.ts') &&
  !path.endsWith('.scene.test.ts')

const productionFiles = (): ReadonlyArray<string> =>
  sourceFilesUnder(sourceRoot).filter(productionSourceFile)

const relativeSourcePath = (path: string): string => relative(sourceRoot, path)

describe('icon policy', () => {
  test('production web code renders icons only through the generated catalog', () => {
    const violations = productionFiles()
      .filter(path => relativeSourcePath(path) !== generatedIconFile)
      .flatMap(path => {
        const source = readFileSync(path, 'utf8')

        return forbiddenRawIconSources
          .filter(rule => rule.pattern.test(source))
          .map(rule => `${relativeSourcePath(path)} contains ${rule.label}`)
      })

    expect(violations).toEqual([])
  })

  test('icon literals in production web code are catalog icon names', () => {
    const decodeIconName = S.decodeUnknownOption(IconName)
    const violations = productionFiles()
      .filter(path => relativeSourcePath(path) !== generatedIconFile)
      .flatMap(path => {
        const source = readFileSync(path, 'utf8')
        const iconNames = globalThis.Array.from(
          source.matchAll(iconLiteralPattern),
          match => match[1],
        )

        return iconNames
          .filter(name => Option.isNone(decodeIconName(name)))
          .map(name => `${relativeSourcePath(path)} contains icon: '${name}'`)
      })

    expect(violations).toEqual([])
  })
})
