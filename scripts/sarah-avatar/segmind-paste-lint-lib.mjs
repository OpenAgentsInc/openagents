const BRITISH_MARKERS = [
  /\bbritish\b/i,
  /\benglish\s*\(\s*(?:uk|united kingdom|great britain)\s*\)/i,
  /\ben[-_ ]?gb\b/i,
  /\buk english\b/i,
];

export function isBritishVoiceSelection({ voice, voiceLanguage }) {
  const selection = [voice, voiceLanguage].filter((value) => typeof value === "string").join(" ");

  return BRITISH_MARKERS.some((marker) => marker.test(selection));
}

export function findBritishZedPronunciationRisks({ script, voice, voiceLanguage }) {
  if (typeof script !== "string" || !isBritishVoiceSelection({ voice, voiceLanguage })) {
    return [];
  }

  return Array.from(script.matchAll(/\bZed\b/g), (match) => ({
    index: match.index,
    value: match[0],
  }));
}

export function assertBritishZedPronunciationSafe({
  script,
  voice,
  voiceLanguage,
  allowPronunciationRisk = false,
}) {
  const risks = findBritishZedPronunciationRisks({
    script,
    voice,
    voiceLanguage,
  });

  if (risks.length === 0 || allowPronunciationRisk) return risks;

  const occurrences = risks.length === 1 ? "occurrence" : "occurrences";
  throw new Error(
    `spoken paste contains ${risks.length} uppercase "Zed" ${occurrences} with a British voice; ` +
      'write lowercase "zed" for the intended pronunciation, or pass ' +
      "--allow-pronunciation-risk to override this spend gate",
  );
}
