import type { FlowNode, NodeId, NodeSize } from "./model.js"

export const sampleMechaCoderTree: FlowNode = {
  id: "root" as NodeId,
  type: "root",
  label: "OpenAgents Desktop",
  direction: "horizontal",
  children: [
    {
      id: "mechacoder" as NodeId,
      type: "agent",
      label: "MechaCoder Agent",
      direction: "vertical",
      children: [
        {
          id: "repo-openagents" as NodeId,
          type: "repo",
          label: "Repo: openagents",
          direction: "vertical",
          children: [
            {
              id: "oa-b78d3f" as NodeId,
              type: "task",
              label: "oa-b78d3f: HUD-1 Flow model",
              metadata: { status: "busy" as const },
            },
            {
              id: "oa-138548" as NodeId,
              type: "task",
              label: "oa-138548: HUD-2 layout",
              metadata: { status: "open" as const },
            },
            {
              id: "oa-91a779" as NodeId,
              type: "task",
              label: "oa-91a779: HUD-3 path",
            },
          ],
        },
        {
          id: "repo-nostr-effect" as NodeId,
          type: "repo",
          label: "Repo: nostr-effect",
          direction: "vertical",
          children: [
            {
              id: "ne-task1" as NodeId,
              type: "task",
              label: "nostr-effect task 1",
            },
          ],
        },
        {
          id: "internal-loop" as NodeId,
          type: "workflow",
          label: "Internal Loop",
          direction: "horizontal",
          children: [
            {
              id: "phase-read" as NodeId,
              type: "phase",
              label: "read",
            },
            {
              id: "phase-plan" as NodeId,
              type: "phase",
              label: "plan",
            },
            {
              id: "phase-edit" as NodeId,
              type: "phase",
              label: "edit",
            },
            {
              id: "phase-test" as NodeId,
              type: "phase",
              label: "test",
            },
            {
              id: "phase-commit" as NodeId,
              type: "phase",
              label: "commit/close",
            },
          ],
        },
      ],
    },
  ],
}

export const sampleNodeSizes: Record<NodeId, NodeSize> = {
  root: { width: 160, height: 40 },
  mechacoder: { width: 282, height: 100 },
  "repo-openagents": { width: 240, height: 80 },
  "repo-nostr-effect": { width: 240, height: 80 },
  "oa-b78d3f": { width: 240, height: 60 },
  "oa-138548": { width: 240, height: 60 },
  "oa-91a779": { width: 240, height: 60 },
  "ne-task1": { width: 240, height: 60 },
  "internal-loop": { width: 200, height: 60 },
  "phase-read": { width: 120, height: 40 },
  "phase-plan": { width: 120, height: 40 },
  "phase-edit": { width: 120, height: 40 },
  "phase-test": { width: 120, height: 40 },
  "phase-commit": { width: 140, height: 40 },
}
