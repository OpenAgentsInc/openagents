import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

const appSrcRoot = join(process.cwd(), 'src')

const sourceFiles = (root: string): ReadonlyArray<string> => {
  const visit = (dir: string): ReadonlyArray<string> =>
    readdirSync(dir).flatMap(entry => {
      const path = join(dir, entry)
      const stat = statSync(path)

      if (stat.isDirectory()) {
        return visit(path)
      }

      return path.endsWith('.ts') && !path.endsWith('.test.ts') ? [path] : []
    })

  return visit(root)
}

describe('Foldkit UI composition', () => {
  it('keeps app page code composed through the Foldkit UI system', () => {
    const appFiles = sourceFiles(appSrcRoot).filter(file => {
      const localPath = relative(appSrcRoot, file)

      return !localPath.startsWith('ui/') && localPath !== 'icon.ts'
    })

    const filesWithRawClasses = appFiles
      .filter(file => /\bh\.Class\(/.test(readFileSync(file, 'utf8')))
      .map(file => relative(appSrcRoot, file))

    expect(filesWithRawClasses).toEqual([])
  })
})
