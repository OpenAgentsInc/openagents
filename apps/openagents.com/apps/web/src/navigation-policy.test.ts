import { readFileSync, readdirSync, statSync } from 'node:fs'
import { extname, join, relative } from 'node:path'
import { describe, expect, test } from 'vitest'

const sourceRoot = join(process.cwd(), 'src')

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

const forbiddenHistoryCalls = [
  'window.history.pushState(',
  'window.history.replaceState(',
  'history.pushState(',
  'history.replaceState(',
  'pushState(',
  'replaceState(',
]

describe('navigation policy', () => {
  test('production app code uses Foldkit navigation instead of raw history mutation', () => {
    const violations = sourceFilesUnder(sourceRoot)
      .filter(productionSourceFile)
      .flatMap(path => {
        const source = readFileSync(path, 'utf8')

        return forbiddenHistoryCalls
          .filter(call => source.includes(call))
          .map(call => `${relative(sourceRoot, path)} contains ${call}`)
      })

    expect(violations).toEqual([])
  })
})
