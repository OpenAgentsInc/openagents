#!/usr/bin/env bun
// CLI for the headless Khala acceptance runner (EPIC #6017).
//
// Runs OUT of the CF Worker. Given an HTML artifact file, it derives the crossy-road
// acceptance spec, runs the real headless suite, prints the verdict JSON, and exits
// non-zero when not verified — so a coder worker / CI loop can gate on it.
//
// Usage:
//   bun src/inference/acceptance-runner/cli.ts <artifact.html>
//   cat artifact.html | bun src/inference/acceptance-runner/cli.ts -
//
// Prereq: `bunx playwright install chromium` (chromium must be installed).

import { readFile } from 'node:fs/promises'
import process from 'node:process'

import { crossyRoadAcceptanceSpec } from '../acceptance-spec'
import { runAcceptanceSuite } from './runner'

const readStdin = async (): Promise<string> => {
  const chunks: Buffer[] = []
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk))
  }
  return Buffer.concat(chunks).toString('utf8')
}

const readArtifact = async (path: string): Promise<string> =>
  path === '-' ? readStdin() : readFile(path, 'utf8')

const main = async (): Promise<void> => {
  const path = process.argv[2]
  if (path === undefined) {
    console.error(
      'Usage: bun acceptance-runner/cli.ts <artifact.html|->\n' +
        'Reads a single-file HTML artifact and runs the crossy-road acceptance suite.',
    )
    process.exit(2)
  }

  const artifactHtml = await readArtifact(path)
  const spec = crossyRoadAcceptanceSpec()
  const verdict = await runAcceptanceSuite({ artifactHtml, spec })

  console.log(JSON.stringify(verdict, null, 2))
  process.exit(verdict.verified ? 0 : 1)
}

await main()
