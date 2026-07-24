const BRITISH_VOICE_PATTERN =
  /(?:\bbritish\b|english\s*\((?:uk|gb)\)|\ben[-_](?:gb|uk)\b|\bunited kingdom\b)/i

const UPPERCASE_ZED_PATTERN = /\bZed\b/g

export function usesBritishVoice({ voice = '', voiceLanguage = '', voicePrompt = '' } = {}) {
  return BRITISH_VOICE_PATTERN.test(
    `${String(voice)} ${String(voiceLanguage)} ${String(voicePrompt)}`,
  )
}

export function lintTtsPaste({
  script = '',
  voice = '',
  voiceLanguage = '',
  voicePrompt = '',
} = {}) {
  if (!usesBritishVoice({ voice, voiceLanguage, voicePrompt })) {
    return { ok: true, risks: [] }
  }

  const risks = [...String(script).matchAll(UPPERCASE_ZED_PATTERN)].map((match) => ({
    code: 'british_voice_uppercase_zed',
    index: match.index,
    message:
      'British TTS can pronounce uppercase "Zed" as "Zeed". Use lowercase "zed" in the spoken paste.',
  }))

  return { ok: risks.length === 0, risks }
}
