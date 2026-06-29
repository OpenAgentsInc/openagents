// Headless render entry: the LIVE pylon network, through the REAL render path,
// for the before/after glow-up proof (graphics audit A1/A2/A3 + nicer pylons).
//
// It mounts the real `oa-training-run` three-effect element with the
// visualization produced by the REAL `pylonNetworkVisualizationOptions` mapper
// (the same one the desktop UI uses). A representative network of working /
// online / offline pylons rings the center node, connected by bezier edges.
//
// Query params:
//   ?nobloom=1  -> force `bloom.enabled = false`. This is the "base frame still
//                  reads with bloom OFF" proof (the bloom skill's acceptance
//                  check + the don't-double-tone-map guard).
//   (default)   -> bloom ON: HDR pylon stations + fat glowing connections +
//                  HDR gateway/arc emitters + active-pylon spark accents.

import type { TrainingRunVisualizationOptions } from "@openagentsinc/three-effect/core"

import { pylonNetworkVisualizationOptions } from "../src/ui/pylon-network-visualization.js"
import type {
  PylonNetworkNode,
  PylonNetworkScene,
} from "../src/shared/pylon-network-scene.js"
import { mountTrainingRunIsolatedScene } from "./isolated-scenes/mount-training-run-scene.js"

// A deterministic, representative live network: a busy mix so every tier of the
// emissive hierarchy is exercised (working -> active+spark, online -> steady,
// offline -> dim). 14 pylons gives the ring some density without crowding.
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
  asOfLabel: "render-harness.pylon-glow",
}

const base = pylonNetworkVisualizationOptions(scene)

const noBloom =
  new URLSearchParams(globalThis.location?.search ?? "").get("nobloom") === "1"

const visualization: TrainingRunVisualizationOptions = noBloom
  ? { ...base, bloom: { enabled: false } }
  : base

mountTrainingRunIsolatedScene("pylon-network", visualization)
