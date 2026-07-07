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
  {
    mountTest: "tests/mobile-testing-lab-screen.test.tsx",
    screenFile: "src/screens/mobile-testing-lab-screen.tsx",
  },
]

export const khalaMobileGateScreenMountWaivers: readonly KhalaMobileGateScreenWaiver[] = []

export const khalaMobileGateGeneratorConformanceStatus = {
  enforcedArtifacts: [
    "scripts/generate.ts",
    "tests/generated/generator-conformance.test.ts",
    "src/qa/screen-bundles/generated-screen-bundles.ts",
  ],
  issueRef: "#8538",
  state: "qam_3_generator_bundle_enforced",
  statement:
    "Generated screens must be created through the local generator and keep screen, mount tests, stories, pending contract stub, Maestro flow stub, visual registration, and visual-baseline registration bundle members.",
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
