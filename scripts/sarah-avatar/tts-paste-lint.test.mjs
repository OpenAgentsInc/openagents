import assert from 'node:assert/strict'
import test from 'node:test'
import { lintTtsPaste, usesBritishVoice } from './tts-paste-lint.mjs'

test('usesBritishVoice recognizes supported British voice settings', () => {
  assert.equal(usesBritishVoice({ voiceLanguage: 'English (UK)' }), true)
  assert.equal(usesBritishVoice({ voiceLanguage: 'en-GB' }), true)
  assert.equal(usesBritishVoice({ voice: 'British Female' }), true)
  assert.equal(usesBritishVoice({ voiceLanguage: 'English (US)' }), false)
})

test('lintTtsPaste rejects uppercase Zed for British TTS', () => {
  const result = lintTtsPaste({
    script: 'We fork Zed. Zed is the product name.',
    voiceLanguage: 'English (UK)',
  })

  assert.equal(result.ok, false)
  assert.equal(result.risks.length, 2)
  assert.equal(result.risks[0].code, 'british_voice_uppercase_zed')
  assert.match(result.risks[0].message, /lowercase "zed"/)
})

test('lintTtsPaste accepts the intended lowercase spoken form', () => {
  const result = lintTtsPaste({
    script: 'We fork zed.',
    voiceLanguage: 'English (UK)',
  })

  assert.deepEqual(result, { ok: true, risks: [] })
})

test('lintTtsPaste does not apply the British risk to another locale', () => {
  const result = lintTtsPaste({
    script: 'We fork Zed.',
    voiceLanguage: 'English (US)',
  })

  assert.deepEqual(result, { ok: true, risks: [] })
})

test('lintTtsPaste matches Zed as a word and preserves similar names', () => {
  const result = lintTtsPaste({
    script: 'Zedless and Zed.',
    voicePrompt: 'Use a British accent.',
  })

  assert.equal(result.risks.length, 1)
  assert.equal(result.risks[0].index, 12)
})
