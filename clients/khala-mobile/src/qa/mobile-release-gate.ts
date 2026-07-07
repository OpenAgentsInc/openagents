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
  {
    mountTest: "tests/thread-list-screen.test.tsx",
    screenFile: "src/screens/thread-list-screen.tsx",
  },
  {
    mountTest: "tests/thread-messages-screen.test.tsx",
    screenFile: "src/screens/thread-messages-screen.tsx",
  },
  {
    mountTest: "tests/credits-history-screen.test.tsx",
    screenFile: "src/screens/credits-history-screen.tsx",
  },
  {
    mountTest: "tests/settings-screen.test.tsx",
    screenFile: "src/screens/settings-screen.tsx",
  },
]

export const khalaMobileGateScreenMountWaivers: readonly KhalaMobileGateScreenWaiver[] = []

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
    "tests/thread-messages-screen.test.tsx",
  ],
  pendingArtifacts: [],
  state: "qam_2_streaming_fixture_tier_enforced",
} as const
