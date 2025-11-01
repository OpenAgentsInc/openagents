import { parseBridgeCode, normalizeBridgeCodeInput } from './pairing'

function b64url(obj: any): string {
  return Buffer.from(JSON.stringify(obj)).toString('base64url')
}

describe('parseBridgeCode', () => {
  test('parses deep link with base64url payload', () => {
    const code = b64url({ v: 1, type: 'bridge', hosts: ['127.0.0.1:8787'], token: 'abc' })
    const url = `openagents://connect?j=${code}`
    const res = parseBridgeCode(url)
    expect(res).not.toBeNull()
    expect(res!.bridgeHost).toBe('127.0.0.1:8787')
    expect(res!.token).toBe('abc')
  })

  test('parses raw base64url code', () => {
    const code = b64url({ v: 1, type: 'bridge', hosts: ['10.0.0.2:9999'], token: null })
    const res = parseBridgeCode(code)
    expect(res).not.toBeNull()
    expect(res!.bridgeHost).toBe('10.0.0.2:9999')
    expect(res!.token).toBeNull()
  })

  test('falls back to bridge URL when hosts missing', () => {
    const payload = { v: 1, type: 'bridge', bridge: 'wss://bridge.example.com/ws', token: 't' }
    const res = parseBridgeCode(b64url(payload))
    expect(res).not.toBeNull()
    expect(res!.bridgeHost).toBe('bridge.example.com:443')
    expect(res!.token).toBe('t')
  })

  test('returns null on malformed input', () => {
    expect(parseBridgeCode('')).toBeNull()
    expect(parseBridgeCode('not-base64')).toBeNull()
  })
})

describe('normalizeBridgeCodeInput', () => {
  test('extracts code from deep link', () => {
    const code = b64url({ v: 1, type: 'bridge', hosts: ['h:1'], token: 'x' })
    const url = `openagents://connect?j=${code}`
    expect(normalizeBridgeCodeInput(url)).toBe(code)
  })

  test('passes through plain code', () => {
    expect(normalizeBridgeCodeInput('abc')).toBe('abc')
  })
})

