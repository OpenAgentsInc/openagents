import { setup } from 'foldkit/test/vitest'

const processEnv = (
  globalThis as { process?: { env?: Record<string, string | undefined> } }
).process?.env

if (processEnv !== undefined) {
  processEnv.OA_STYLEX_RUNTIME_FALLBACK = '1'
}

setup()
