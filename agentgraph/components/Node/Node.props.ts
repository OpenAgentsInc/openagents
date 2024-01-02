import { Agent, Brain, Step } from "@/types/agents";

export interface NodeProps {
  agent?: Agent
  brain?: Brain
  position?: { x: number; y: number }
  step?: Step
}
