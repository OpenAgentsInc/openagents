/**
 * AFS-12 shared read/compose reader test.
 *
 * The reader is the one compose path Desktop, web, and mobile reuse. This suite
 * proves it decodes the frozen AFS-00 corpus to the canonical facts, that three
 * independent decode calls (standing in for the three surfaces) reach equivalent
 * facts, and that the decoded facts are secret-free.
 */
import { describe, expect, test } from "vite-plus/test";

import {
  readSafeSurfaceScenario,
  summarizeSurfaceFacts,
  surfaceFactsAreSecretFree,
} from "./index.js";
import {
  afsBaselineSurfaceFactSummary,
  afsBaselineSurfaceScenarios,
  readAfsBaselineSurfaceFacts,
  readAfsBaselineSurfaceFactSummary,
} from "./afs-baseline-surface-corpus.js";

describe("AFS-12 shared reader: decode equivalence", () => {
  test("the shared reader decodes the frozen corpus to the canonical facts", () => {
    expect(readAfsBaselineSurfaceFactSummary()).toEqual(afsBaselineSurfaceFactSummary);
  });

  test("three independent decode passes reach equivalent facts (Desktop/web/mobile stand-ins)", () => {
    const desktop = afsBaselineSurfaceScenarios.map(readSafeSurfaceScenario);
    const web = afsBaselineSurfaceScenarios.map(readSafeSurfaceScenario);
    const mobile = afsBaselineSurfaceScenarios.map(readSafeSurfaceScenario);
    expect(desktop).toEqual(web);
    expect(web).toEqual(mobile);
    expect(desktop.map(summarizeSurfaceFacts)).toEqual(afsBaselineSurfaceFactSummary);
  });

  test("the local answer decodes with no dispatched provider turn and an on-device route", () => {
    const local = readAfsBaselineSurfaceFacts()[0];
    expect(local?.scenario).toBe("local_answer");
    expect(local?.card.providerTurnRef).toBeNull();
    expect(local?.card.provider).toBe("apple_fm");
    expect(local?.route?.outcome).toBe("admitted");
    expect(local?.route?.dataDestination).toBe("on_device_local");
    expect(local?.route?.localOnly).toBe(true);
  });

  test("the unavailable-provider scenario decodes fail-closed with a refusal recovery", () => {
    const closed = readAfsBaselineSurfaceFacts()[5];
    expect(closed?.scenario).toBe("unavailable_provider");
    expect(closed?.route?.outcome).toBe("closed");
    expect(closed?.recovery.cardState).toBe("refused");
    expect(closed?.recovery.refusalReason).toBe("route_closed_no_candidate");
  });
});

describe("AFS-12 shared reader: privacy fence", () => {
  test("every decoded scenario's facts are secret-free", () => {
    for (const facts of readAfsBaselineSurfaceFacts()) {
      expect(surfaceFactsAreSecretFree(facts)).toBe(true);
    }
  });

  test("the fence rejects a fact object carrying a raw path or token shape", () => {
    expect(surfaceFactsAreSecretFree({ helperPath: "/Users/x/.codex/auth.json" })).toBe(false);
    expect(surfaceFactsAreSecretFree({ note: "sk-abc123deadbeef" })).toBe(false);
    expect(surfaceFactsAreSecretFree({ scenario: "local_answer", cardState: "done" })).toBe(true);
  });
});
