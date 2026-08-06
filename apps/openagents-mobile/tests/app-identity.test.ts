import { describe, expect, test } from "vite-plus/test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * OpenAgents mobile (#8597) identity oracle — the issue's identity locks,
 * mechanically enforced so they cannot drift:
 *
 * 1. Display/product name: `OpenAgents`.
 * 2. iOS bundle identifier: `com.openagents.app`.
 * 3. Android package/application ID: `com.openagents.app`.
 * 4. The checked-in canonical OpenAgents mobile icon is pinned by SHA-256.
 */

const appRoot = join(import.meta.dirname, "..");
const sarahVoiceScreenSource = readFileSync(
  join(appRoot, "src/screens/sarah-voice-screen.tsx"),
  "utf8",
);
const realtimeAudioModuleSource = readFileSync(
  join(appRoot, "modules/expo-realtime-audio/ios/ExpoRealtimeAudioModule.swift"),
  "utf8",
);
const realtimeAudioAndroidModuleSource = readFileSync(
  join(
    appRoot,
    "modules/expo-realtime-audio/android/src/main/java/expo/modules/realtimeaudio/ExpoRealtimeAudioModule.kt",
  ),
  "utf8",
);

const appConfig = JSON.parse(readFileSync(join(appRoot, "app.json"), "utf8")) as {
  expo: {
    name: string;
    icon: string;
    ios: {
      bundleIdentifier: string;
      entitlements?: Record<string, ReadonlyArray<string>>;
    };
    android: { package: string };
    extra?: { openagents?: { pushProjectId?: string } };
    runtimeVersion?: { policy?: string };
    updates?: {
      enabled?: boolean;
      url?: string;
      requestHeaders?: Record<string, string>;
    };
    plugins?: ReadonlyArray<string | readonly [string, Readonly<Record<string, string | boolean>>]>;
  };
};

const PINNED_ICON_SHA256 = "0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce";

describe("contract openagents_mobile.identity.v1", () => {
  test("display name is exactly OpenAgents", () => {
    expect(appConfig.expo.name).toBe("OpenAgents");
  });

  test("iOS bundle identifier and Android application ID are exactly com.openagents.app", () => {
    expect(appConfig.expo.ios.bundleIdentifier).toBe("com.openagents.app");
    expect(appConfig.expo.ios.entitlements).toEqual({
      "keychain-access-groups": ["$(AppIdentifierPrefix)com.openagents.app"],
    });
    expect(appConfig.expo.android.package).toBe("com.openagents.app");
  });

  test("checked-in icon is the exact pinned OpenAgents icon (SHA-256)", () => {
    expect(appConfig.expo.icon).toBe("./assets/images/icon.png");
    const digest = createHash("sha256")
      .update(readFileSync(join(appRoot, "assets/images/icon.png")))
      .digest("hex");
    expect(digest).toBe(PINNED_ICON_SHA256);
  });

  test("OTA is explicitly disabled, not left pointing at a retired endpoint", () => {
    // The mobile OTA surface on updates.openagents.com was retired on
    // 2026-08-05 (#9325) at owner direction — there are no installed mobile
    // users, and the `/<owner>/manifest` route now 404s. expo-updates is still
    // linked into the binary and `updates.enabled` DEFAULTS TO TRUE, so the
    // key must be present and false: deleting it would leave the client
    // enabled with no configured URL, which is the misconfiguration rather
    // than the fix. JS changes now ship only in a new store build.
    expect(appConfig.expo.updates?.enabled).toBe(false);
    expect(appConfig.expo.updates?.url).toBeUndefined();
    expect(appConfig.expo.updates?.requestHeaders).toBeUndefined();
  });

  test("no update endpoint of any kind is configured — owned, legacy, or Expo CDN", () => {
    const serialized = JSON.stringify(appConfig.expo.updates ?? {}).toLowerCase();
    expect(serialized).not.toContain("http");
    expect(serialized).not.toContain("updates.openagents.com");
    expect(serialized).not.toContain("khala");
    expect(serialized).not.toContain("autopilot");
    expect(serialized).not.toContain("u.expo.dev");
  });

  test("microphone permission is exact and background audio stays disabled", () => {
    const audioPlugin = appConfig.expo.plugins?.find(
      (plugin) => Array.isArray(plugin) && plugin[0] === "expo-audio",
    );
    expect(audioPlugin).toEqual([
      "expo-audio",
      {
        microphonePermission:
          "Allow OpenAgents to use the microphone for live conversations with Sarah.",
        recordAudioAndroid: true,
        enableBackgroundPlayback: false,
        enableBackgroundRecording: false,
      },
    ]);
  });

  test("ambient native SDKs and the stable push project are configured in the store build", () => {
    expect(appConfig.expo.extra?.openagents?.pushProjectId).toBe(
      "9b2d1e62-fd84-4fa4-928f-fce07b733014",
    );
    expect(appConfig.expo.plugins).toContain("expo-notifications");
    expect(appConfig.expo.plugins).toContain("expo-live-activity");
    expect(
      appConfig.expo.plugins?.some(
        (plugin) => Array.isArray(plugin) && plugin[0] === "expo-share-intent",
      ),
    ).toBe(true);
  });

  test("native SHA-256 receives a typed array", () => {
    expect(sarahVoiceScreenSource).toContain(
      "digest(CryptoDigestAlgorithm.SHA256, Uint8Array.from(bytes))",
    );
    expect(sarahVoiceScreenSource).not.toContain(
      "digest(CryptoDigestAlgorithm.SHA256, Uint8Array.from(bytes).buffer)",
    );
  });

  test("Sarah screen defers microphone construction to the owned native module", () => {
    expect(sarahVoiceScreenSource).not.toContain("useAudioStream");
    expect(sarahVoiceScreenSource).toContain("RealtimeAudio.startMicrophone(24_000)");
    expect(realtimeAudioModuleSource).toContain('Events("onMicrophoneBuffer")');
    expect(realtimeAudioModuleSource).toContain('Function("startMicrophone")');
  });

  test("playback status never queries an unattached AVAudioPlayerNode", () => {
    expect(realtimeAudioModuleSource).toContain(
      "currentStarted,\n      player.engine != nil,\n      let renderTime = player.lastRenderTime",
    );
  });

  test("native playback queues a complete normal voice response", () => {
    expect(realtimeAudioModuleSource).toContain("maximumQueuedSeconds: Int64 = 120");
    expect(realtimeAudioAndroidModuleSource).toContain("MAXIMUM_QUEUED_SECONDS = 120");
  });
});
