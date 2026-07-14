import { setup } from 'foldkit/test/vitest'
import { resolve } from 'node:path'

process.chdir(resolve(import.meta.dirname, '..'))

const processEnv = (
  globalThis as { process?: { env?: Record<string, string | undefined> } }
).process?.env

if (processEnv !== undefined) {
  processEnv.OA_STYLEX_RUNTIME_FALLBACK = '1'
}

setup()
