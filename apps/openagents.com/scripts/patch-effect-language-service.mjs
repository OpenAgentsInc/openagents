#!/usr/bin/env node
import { access } from 'node:fs/promises'
import { constants } from 'node:fs'
import { dirname, join } from 'node:path'
import { spawnSync } from 'node:child_process'

const exists = async path => {
  try {
    await access(path, constants.R_OK)
    return true
  } catch {
    return false
  }
}

const findInstallRoot = async start => {
  let current = start
  for (;;) {
    const typescriptPackage = join(
      current,
      'node_modules',
      'typescript',
      'package.json',
    )
    const languageServiceBin = join(
      current,
      'node_modules',
      '.bin',
      process.platform === 'win32'
        ? 'effect-language-service.cmd'
        : 'effect-language-service',
    )

    if ((await exists(typescriptPackage)) && (await exists(languageServiceBin))) {
      return { bin: languageServiceBin, root: current }
    }

    const parent = dirname(current)
    if (parent === current) {
      throw new Error(
        'Unable to find installed typescript and effect-language-service packages',
      )
    }
    current = parent
  }
}

const { bin, root } = await findInstallRoot(process.cwd())
const result = spawnSync(bin, ['patch'], {
  cwd: root,
  env: process.env,
  stdio: 'inherit',
})

if (result.error !== undefined) {
  throw result.error
}

process.exit(result.status ?? 1)
