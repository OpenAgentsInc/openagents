// Adapts the live pylon-network scene onto the existing three-effect bezier
// graph (trainingRunView / TrainingRunVisualizationOptions) — "adapt that bezier
// graph we have elsewhere in desktop". The center node is the network; pylons
// ring around it, connected by bezier edges; node status carries the tone and
// pulseSpeed carries the activity glow.
//
// Visual language: docs/autopilot-coder/2026-06-15-autopilot-home-network-visual-language.md
import type {
  TrainingRunNodeDefinition,
  TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import type {
  PylonNetworkNode,
  PylonNetworkScene,
} from "../shared/pylon-network-scene.js"

const BACKGROUND = 0x0c0f13 // matches the homepage pylon scene
const CENTER_ID = "network"

// working -> "active" (lit + animated), online -> "queued" (steady),
// offline -> "planned" (dim). These three statuses already have distinct tones
// in the three-effect scene.
const toneToStatus = (
  tone: PylonNetworkNode["tone"],
): TrainingRunNodeDefinition["status"] =>
  tone === "working" ? "active" : tone === "online" ? "queued" : "planned"

// Deterministic ring layout so the graph is stable between polls (no jitter).
// Radius grows gently with node count so a busy network stays legible.
const ringPosition = (
  index: number,
  count: number,
): TrainingRunNodeDefinition["position"] => {
  const radius = 2.4 + Math.min(1.6, count / 40)
  const angle = count <= 0 ? 0 : (2 * Math.PI * index) / count
  const height = 0.35 + Math.sin(index * 1.324717957244746 + count * 0.19) * 0.72
  return [
    Number((Math.cos(angle) * radius).toFixed(3)),
    Number((Math.sin(angle) * radius * 0.62).toFixed(3)),
    Number(height.toFixed(3)),
  ]
}

export function pylonNetworkVisualizationOptions(
  scene: PylonNetworkScene,
): TrainingRunVisualizationOptions {
  const centerStatus: TrainingRunNodeDefinition["status"] = scene.dormant
    ? "planned"
    : scene.activityIntensity > 0.05
      ? "active"
      : "sealed"

  const centerNode: TrainingRunNodeDefinition = {
    id: CENTER_ID,
    label: "network",
    detail: scene.dormant
      ? "no pylons online"
      : `${scene.onlineNow} online · ${scene.sessionsOnlineNow} working`,
    role: "run",
    status: centerStatus,
    position: [0, 0, 0.95],
    // edges flow from each pylon into the center (set on the pylon side below)
  }

  const count = scene.nodes.length
  const pylonNodes: TrainingRunNodeDefinition[] = scene.nodes.map((node, index) => ({
    id: node.id,
    label: node.label,
    detail: node.tone === "working" ? "working" : node.tone === "online" ? "online" : "seen",
    role: node.tone === "working" ? "proof" : "lifecycle",
    status: toneToStatus(node.tone),
    position: ringPosition(index, count),
    connectedTo: [CENTER_ID],
  }))

  // Activity drives the pulse: idle breathes slowly, a busy network pulses fast.
  const pulseSpeed = Number((0.55 + scene.activityIntensity * 1.9).toFixed(3))

  return {
    backgroundColor: BACKGROUND,
    pulseSpeed,
    nodes: [centerNode, ...pylonNodes],
    worldLabelDensity: "compact",
  }
}
