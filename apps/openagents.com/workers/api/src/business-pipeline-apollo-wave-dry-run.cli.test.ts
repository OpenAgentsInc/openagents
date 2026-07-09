/**
 * CLI entry for `bun run apollo-wave:dry-run`.
 * Prints the dry-run receipt when OB2_APOLLO_WAVE_DRY_RUN_PRINT=1.
 */

import { expect, test } from 'vitest'

import { runOb2ApolloWaveDryRun } from './business-pipeline-apollo-wave-dry-run'

test('OB-2 Apollo wave dry-run CLI receipt', async () => {
  const count = Number(process.env.OB2_APOLLO_WAVE_DRY_RUN_COUNT ?? '100')
  const receipt = await runOb2ApolloWaveDryRun(count)
  expect(receipt.ok).toBe(true)
  expect(receipt.actualTotalRows).toBe(receipt.expectedTotalRows)
  expect(receipt.segments).toHaveLength(2)
  if (process.env.OB2_APOLLO_WAVE_DRY_RUN_PRINT === '1') {
    // eslint-disable-next-line no-console
    console.log(JSON.stringify(receipt, null, 2))
  }
})
