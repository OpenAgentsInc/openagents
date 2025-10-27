import test from 'node:test'
import assert from 'node:assert/strict'
import { buildTunnelArgs } from './args.js'

test('buildTunnelArgs uses --local-port and correct ordering', () => {
  const args = buildTunnelArgs(7788, 'bore.pub')
  assert.deepEqual(args, [
    'run',
    '-q',
    '-p',
    'oa-tunnel',
    '--',
    '--to',
    'bore.pub',
    '--local-port',
    '7788',
  ])
})
