import type { VerseLocalPose } from "./model.js"

let latestPose: VerseLocalPose | null = null

export const recordLatestVerseLocalPose = (pose: VerseLocalPose): void => {
  latestPose = pose
}

export const latestVerseLocalPose = (): VerseLocalPose | null => latestPose

export const clearLatestVerseLocalPoseForTest = (): void => {
  latestPose = null
}
