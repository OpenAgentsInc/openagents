export type ConversationCoherenceFacts = Readonly<{
  evidenceComplete: boolean
  intentPreserved: boolean
  modeStarted: boolean
  modeTriggerPresent: boolean
  materialActionCount: number
  allMaterialActionsAuthorized: boolean
  answerPresent: boolean
  answerRelevant: boolean
  routeChanged: boolean
  routeVisibleBeforeAnswer: boolean
  presentedProviderMatches: boolean
  eventOrderValid: boolean
  reloadStateMatches: boolean
  outcomeClosed: boolean
  nonActionRequest: boolean
}>

export type ConversationCoherenceGrade = Readonly<{
  score: number
  grade: "A" | "B" | "C" | "D" | "F"
  disposition: "pass" | "needs_correction" | "fail" | "inconclusive"
  ratings: Readonly<{
    intentFidelity: number
    causalContinuity: number
    modeAuthorityIntegrity: number
    answerRelevance: number
    provenanceRoleClarity: number
    stateSequenceConsistency: number
    outcomeClosure: number
    informationEconomy: number
  }>
  failedGates: ReadonlyArray<"G1" | "G2" | "G3" | "G4" | "G5" | "G6">
}>

const scoreGrade = (score: number): ConversationCoherenceGrade["grade"] =>
  score >= 90 ? "A" : score >= 80 ? "B" : score >= 70 ? "C" : score >= 50 ? "D" : "F"

/** Deterministic rubric calculation over captured, reviewable thread facts. */
export const gradeConversationCoherence = (
  facts: ConversationCoherenceFacts,
): ConversationCoherenceGrade => {
  const failedGates: Array<ConversationCoherenceGrade["failedGates"][number]> = []
  if (facts.modeStarted && !facts.modeTriggerPresent) failedGates.push("G1")
  if (facts.materialActionCount > 0 && !facts.allMaterialActionsAuthorized) failedGates.push("G2")
  if (!facts.answerRelevant) failedGates.push("G3")
  if (!facts.presentedProviderMatches) failedGates.push("G4")
  if (!facts.eventOrderValid) failedGates.push("G5")
  if (!facts.evidenceComplete) failedGates.push("G6")

  const ratings = {
    intentFidelity: facts.intentPreserved ? 4 : 0,
    causalContinuity: facts.allMaterialActionsAuthorized ? 4 : 0,
    modeAuthorityIntegrity:
      !facts.modeStarted || facts.modeTriggerPresent ? 4 : 0,
    answerRelevance: facts.answerRelevant ? 4 : 0,
    provenanceRoleClarity:
      facts.presentedProviderMatches && (!facts.routeChanged || facts.routeVisibleBeforeAnswer)
        ? 4
        : facts.presentedProviderMatches
          ? 2
          : 0,
    stateSequenceConsistency:
      facts.eventOrderValid && facts.reloadStateMatches
        ? 4
        : facts.eventOrderValid || facts.reloadStateMatches
          ? 1
          : 0,
    outcomeClosure: facts.outcomeClosed ? 4 : facts.answerPresent ? 1 : 0,
    informationEconomy:
      facts.nonActionRequest && facts.materialActionCount > 0
        ? 0
        : facts.allMaterialActionsAuthorized
          ? 4
          : 1,
  } as const
  const score = Math.round(
    ratings.intentFidelity * 5 +
      ratings.causalContinuity * 3.75 +
      ratings.modeAuthorityIntegrity * 5 +
      ratings.answerRelevance * 3.75 +
      ratings.provenanceRoleClarity * 2.5 +
      ratings.stateSequenceConsistency * 2.5 +
      ratings.outcomeClosure * 1.25 +
      ratings.informationEconomy * 1.25,
  )
  const hardFailure = failedGates.some((gate) => gate !== "G6")
  const inconclusive = failedGates.includes("G6") && !hardFailure
  return {
    score,
    grade: hardFailure ? "F" : scoreGrade(score),
    disposition: inconclusive
      ? "inconclusive"
      : hardFailure
        ? "fail"
        : score >= 80
          ? "pass"
          : "needs_correction",
    ratings,
    failedGates,
  }
}
