import { describe, expect, test } from 'vitest'

import {
  findPublicScannerUnsafeStrings,
  publicRefTriggersAgentSecretScanner,
  publicScannerSafeRef,
  publicScannerSafeRefs,
} from './public-ref-scanner-safety'

describe('public ref scanner safety', () => {
  test('keeps short dotted public refs readable', () => {
    expect(publicRefTriggersAgentSecretScanner('cap.gepa.retained.v1')).toBe(
      false,
    )
    expect(
      publicScannerSafeRef('capability.public.pylon', 'cap.gepa.retained.v1'),
    ).toBe('cap.gepa.retained.v1')
  })

  test('aliases JWT-shaped and long base64url-shaped refs', () => {
    const jwtShaped =
      'eyJaaaaaaaaaaa.eyJbbbbbbbbbbb.cccccccccccccccccccccccccccccc'
    const longBase64UrlShaped = 'artanis-mdk-bridge-8b378373002501f3e896dcd3'

    expect(publicRefTriggersAgentSecretScanner(jwtShaped)).toBe(true)
    expect(publicRefTriggersAgentSecretScanner(longBase64UrlShaped)).toBe(true)
    expect(
      publicScannerSafeRefs('evidence.public.test', [
        'cap.gepa.retained.v1',
        jwtShaped,
        longBase64UrlShaped,
      ]),
    ).toEqual([
      'cap.gepa.retained.v1',
      expect.stringMatching(
        /^evidence\.public\.test\.scanner_safe\.[0-9a-f]{8}$/,
      ),
      expect.stringMatching(
        /^evidence\.public\.test\.scanner_safe\.[0-9a-f]{8}$/,
      ),
    ])
  })

  test('audits nested public JSON without exposing raw values', () => {
    const findings = findPublicScannerUnsafeStrings({
      pylon: {
        capabilityRefs: [
          'cap.gepa.retained.v1',
          'artanis-mdk-bridge-8b378373002501f3e896dcd3',
        ],
      },
    })

    expect(findings).toEqual([
      {
        length: 43,
        path: '$.pylon.capabilityRefs[1]',
        preview: 'artanis-...dcd3',
        reason: 'long_base64url_shaped',
      },
    ])
  })
})
