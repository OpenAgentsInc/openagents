import { describe, expect, test } from "vite-plus/test";

import { projectMobileUpdate } from "../src/mobile-update";
import {
  SARAH_BETA_VOICE_BASE_URL,
  SARAH_BETA_VOICE_ENVIRONMENT,
  sarahVoiceApiHost,
} from "../src/sarah-voice/environment";

const input = {
  appVersion: "0.5.2",
  buildNumber: "127",
  updateId: "36c6c979-28d1-40ed-b17d-f66d96f085af",
  runtimeVersion: "4d325a423edf99c9fcdadf67ed6b8b0bcb743dfc",
  isEmbeddedLaunch: false,
  isChecking: false,
  isDownloading: false,
  isUpdatePending: false,
  hasError: false,
  voiceEnvironment: SARAH_BETA_VOICE_ENVIRONMENT,
  voiceHost: sarahVoiceApiHost(),
} as const;

describe("mobile update diagnostic", () => {
  test("shows the applied OTA and the scoped staging voice environment", () => {
    expect(projectMobileUpdate(input)).toEqual({
      appLabel: "0.5.2 (127)",
      copyText: [
        "OpenAgents 0.5.2 (127)",
        "Update 36c6c979",
        "Runtime 4d325a423e",
        "State Current update applied",
        "Sarah voice Staging beta openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app",
      ].join("\n"),
      phase: "current",
      releaseFingerprint: "36c6c979",
      runtimeFingerprint: "4d325a423e",
      statusLabel: "Current update applied",
      voiceEnvironment: "Staging beta",
      voiceHost: "openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app",
    });
    expect(SARAH_BETA_VOICE_BASE_URL).toBe(
      "https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app",
    );
  });

  test("shows checking and downloaded restart states from Expo Updates", () => {
    expect(projectMobileUpdate({ ...input, isChecking: true }).statusLabel).toBe(
      "Checking for update",
    );
    expect(projectMobileUpdate({ ...input, isUpdatePending: true })).toMatchObject({
      phase: "downloaded",
      statusLabel: "Update downloaded—restart to apply",
    });
  });

  test("uses a stable embedded fingerprint when no OTA identifier exists", () => {
    expect(
      projectMobileUpdate({ ...input, updateId: null, isEmbeddedLaunch: true }),
    ).toMatchObject({
      releaseFingerprint: "embedded-4d325a423e",
      statusLabel: "Embedded update running",
    });
  });
});
