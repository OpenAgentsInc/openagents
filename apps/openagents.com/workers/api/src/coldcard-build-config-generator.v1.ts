import { forensicCanonicalJson } from '@openagentsinc/forensic-contract'

const OUTPUT_PATH = 'generated/coldcard-build-config.json'
const TOOLCHAIN_DIGEST =
  'sha256:e1900ca9116bcd97ae95d51c189a5003a22022a16f2aae389096f2a3200eef46'
const INPUT_KEYS = ['board', 'commit', 'toolchain'] as const

export const generateColdcardBuildConfigV1 = (
  input: Readonly<{
    sourceCommitSha: string
    path: string
    parameters: Readonly<Record<string, string>>
  }>,
): Uint8Array | undefined => {
  const parameterKeys = Object.keys(input.parameters).sort()
  if (
    input.path !== OUTPUT_PATH ||
    forensicCanonicalJson(parameterKeys) !==
      forensicCanonicalJson(INPUT_KEYS) ||
    input.parameters.board !== 'COLDCARD' ||
    input.parameters.commit !== input.sourceCommitSha ||
    input.parameters.toolchain !== `stm32/dockerfile.build@${TOOLCHAIN_DIGEST}`
  )
    return undefined
  return new TextEncoder().encode(
    `${forensicCanonicalJson(input.parameters)}\n`,
  )
}
