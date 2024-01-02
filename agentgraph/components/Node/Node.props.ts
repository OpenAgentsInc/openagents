import { Agent, Step } from "@/types/agents";

export interface NodeProps {
  agent?: Agent
  position?: { x: number; y: number }
  step?: Step
}
