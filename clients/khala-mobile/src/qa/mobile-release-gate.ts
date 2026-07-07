export type KhalaMobileGateScreenWaiver = Readonly<{
  blockerRef: string
  issueRef: string
  reason: string
  screenFile: string
  targetArtifact: string
}>

export type KhalaMobileGateScreenBundle = Readonly<{
  mountTest: string | null
  screenFile: string
}>

export const khalaMobileGateScreenMountBundles: readonly KhalaMobileGateScreenBundle[] = [
  {
    mountTest: "tests/repo-picker-screen.test.tsx",
    screenFile: "src/screens/repo-picker-screen.tsx",
  },
  {
    mountTest: "tests/onboarding-welcome-cta.test.tsx",
    screenFile: "src/screens/onboarding-flow.tsx",
  },
]

export const khalaMobileGateScreenMountWaivers: readonly KhalaMobileGateScreenWaiver[] = [
  {
    blockerRef: "blocker.qam_2.thread_list_mount_fixture",
    issueRef: "#8537",
    reason:
      "ThreadListScreen depends on the live Khala sync runtime/provider graph; QAM-2 owns the typed fixture mount for loading, empty, error, and populated states.",
    screenFile: "src/screens/thread-list-screen.tsx",
    targetArtifact: "tests/thread-list-screen.test.tsx",
  },
  {
    blockerRef: "blocker.qam_2.thread_messages_stream_fixture",
    issueRef: "#8537",
    reason:
      "ThreadMessagesScreen needs the runtime-event stream fixture suite so the mount can prove transcript ordering, refusals, interruption, and writeback cards without live cloud.",
    screenFile: "src/screens/thread-messages-screen.tsx",
    targetArtifact: "tests/thread-messages-screen.test.tsx",
  },
  {
    blockerRef: "blocker.qam_2.credits_history_mount_fixture",
    issueRef: "#8537",
    reason:
      "CreditsHistoryScreen needs a typed credits transaction fixture and auth/fetch harness to cover loading, unavailable, error, empty, and populated states.",
    screenFile: "src/screens/credits-history-screen.tsx",
    targetArtifact: "tests/credits-history-screen.test.tsx",
  },
  {
    blockerRef: "blocker.qam_2.settings_native_module_mount",
    issueRef: "#8537",
    reason:
      "SettingsScreen currently has source-composition and sign-out mount coverage; full-screen mount needs expo-notifications, expo-constants, Modal, auth, model, credits, and readiness fixtures.",
    screenFile: "src/screens/settings-screen.tsx",
    targetArtifact: "tests/settings-screen.test.tsx",
  },
]

export const khalaMobileGateGeneratorConformanceStatus = {
  blockerRef: "blocker.qam_3.generator_bundle_upgrade",
  issueRef: "#8538",
  state: "stubbed_until_qam_3",
  statement:
    "QAM-1 enforces that every existing screen has a mount artifact or typed waiver; QAM-3 upgrades templates so new generated screens emit mount tests, stories, contract stubs, Maestro stubs, and visual registration.",
} as const

export const khalaMobileGateFixtureTierStatus = {
  enforcedArtifacts: [
    "tests/khala-mobile-sync-runtime.test.ts",
    "tests/khala-mobile-sync-runtime-registry.test.ts",
    "tests/khala-runtime-compose-core.test.ts",
    "tests/khala-runtime-transcript-core.test.ts",
  ],
  pendingArtifacts: [
    {
      blockerRef: "blocker.qam_2.agent_computer_streaming_fixture_suite",
      issueRef: "#8537",
      targetArtifact: "tests/thread-messages-streaming-fixtures.test.tsx",
    },
  ],
  state: "partial_existing_runtime_fixture_tier",
} as const
