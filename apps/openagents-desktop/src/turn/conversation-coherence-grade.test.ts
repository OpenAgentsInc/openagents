import { describe, expect, test } from "vite-plus/test"

import { gradeConversationCoherence } from "./conversation-coherence-grade.ts"

describe("conversation coherence grading", () => {
  test("reproduces the reported thread as a hard-fail score of 9", () => {
    expect(gradeConversationCoherence({
      evidenceComplete: true,
      intentPreserved: false,
      modeStarted: true,
      modeTriggerPresent: false,
      materialActionCount: 18,
      allMaterialActionsAuthorized: false,
      answerPresent: true,
      answerRelevant: false,
      routeChanged: true,
      routeVisibleBeforeAnswer: false,
      presentedProviderMatches: true,
      eventOrderValid: false,
      reloadStateMatches: true,
      outcomeClosed: false,
      nonActionRequest: true,
    })).toMatchObject({
      score: 9,
      grade: "F",
      disposition: "fail",
      failedGates: ["G1", "G2", "G3", "G5"],
    })
  })

  test("gives a direct relevant identity answer an A", () => {
    expect(gradeConversationCoherence({
      evidenceComplete: true,
      intentPreserved: true,
      modeStarted: false,
      modeTriggerPresent: false,
      materialActionCount: 0,
      allMaterialActionsAuthorized: true,
      answerPresent: true,
      answerRelevant: true,
      routeChanged: false,
      routeVisibleBeforeAnswer: true,
      presentedProviderMatches: true,
      eventOrderValid: true,
      reloadStateMatches: true,
      outcomeClosed: true,
      nonActionRequest: true,
    })).toEqual(expect.objectContaining({
      score: 100,
      grade: "A",
      disposition: "pass",
      failedGates: [],
    }))
  })

  test("does not let absent evidence hide a hard failure", () => {
    expect(gradeConversationCoherence({
      evidenceComplete: false,
      intentPreserved: false,
      modeStarted: true,
      modeTriggerPresent: false,
      materialActionCount: 1,
      allMaterialActionsAuthorized: false,
      answerPresent: false,
      answerRelevant: false,
      routeChanged: true,
      routeVisibleBeforeAnswer: false,
      presentedProviderMatches: false,
      eventOrderValid: false,
      reloadStateMatches: false,
      outcomeClosed: false,
      nonActionRequest: true,
    })).toMatchObject({
      grade: "F",
      disposition: "fail",
      failedGates: ["G1", "G2", "G3", "G4", "G5", "G6"],
    })
  })
})
