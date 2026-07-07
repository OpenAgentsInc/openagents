export type KhalaMobileGeneratedScreenBundle = Readonly<{
  contractTest: string
  maestroFlow: string
  mountTest: string
  screenFile: string
  storyFile: string
  visualRegistration: string
}>

export const khalaMobileGeneratedScreenBundles: readonly KhalaMobileGeneratedScreenBundle[] = [
  {
    contractTest: "tests/mobile-testing-lab-contract.test.ts",
    maestroFlow: ".maestro/generated/mobile-testing-lab-screen.yaml",
    mountTest: "tests/mobile-testing-lab-screen.test.tsx",
    screenFile: "src/screens/mobile-testing-lab-screen.tsx",
    storyFile: "src/screens/mobile-testing-lab-screen.stories.tsx",
    visualRegistration: "src/qa/visual-baselines/mobile-testing-lab-screen.ts",
  },
]
