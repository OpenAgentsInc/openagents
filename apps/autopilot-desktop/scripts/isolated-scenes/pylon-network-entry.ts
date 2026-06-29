// Isolated scene entry for #6033: a synthetic Pylon network scene.

import type { TrainingRunVisualizationOptions } from "@openagentsinc/three-effect/core"

import type {
  PylonNetworkNode,
  PylonNetworkScene,
} from "../../src/shared/pylon-network-scene.js"
import { pylonNetworkVisualizationOptions } from "../../src/ui/pylon-network-visualization.js"

import { mountTrainingRunIsolatedScene } from "./mount-training-run-scene.js"

const tones: PylonNetworkNode["tone"][] = [
  "working",
  "online",
  "working",
  "offline",
  "online",
  "working",
  "online",
  "offline",
  "working",
  "online",
  "working",
  "offline",
  "online",
  "working",
]

const nodes: PylonNetworkNode[] = tones.map((tone, index) => ({
  id: `pylon.${index + 1}`,
  label: `pylon-${index + 1}`,
  tone,
  flowing: tone === "working",
}))

const scene: PylonNetworkScene = {
  activityIntensity: 0.72,
  dormant: false,
  onlineNow: nodes.filter((node) => node.tone !== "offline").length,
  sessionsOnlineNow: nodes.filter((node) => node.tone === "working").length,
  sellableOnlineNow: 6,
  walletReadyNow: 5,
  assignmentReadyNow: 5,
  seen24h: 14,
  registeredTotal: 20,
  satsSettled24h: 1234,
  satsSettledTotal: 56789,
  trainingAssignedContributors: 6,
  trainingAcceptedContributors: 5,
  trainingProgressContributors: 4,
  nodes,
  asOfLabel: "isolated-scene.pylon-network",
}

const base = pylonNetworkVisualizationOptions(scene)
const noBloom =
  new URLSearchParams(globalThis.location?.search ?? "").get("nobloom") === "1"

const visualization: TrainingRunVisualizationOptions = noBloom
  ? { ...base, bloom: { enabled: false } }
  : base

mountTrainingRunIsolatedScene("pylon-network", visualization)
