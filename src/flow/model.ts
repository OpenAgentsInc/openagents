export type NodeId = string

export type Direction = "horizontal" | "vertical"

export interface FlowNode {
  readonly id: NodeId
  readonly type: string
  readonly label: string
  readonly direction?: Direction
  readonly children?: readonly FlowNode[]
  readonly metadata?: Record<string, unknown>
}

export interface NodeSize {
  readonly width: number
  readonly height: number
}

export interface PositionedNode extends FlowNode {
  readonly x: number
  readonly y: number
  readonly size: NodeSize
}

export interface Connection {
  readonly parentId: NodeId
  readonly childId: NodeId
  readonly waypoints: readonly Point[]
}

export interface Point {
  readonly x: number
  readonly y: number
}

export type Status = "idle" | "busy" | "error" | "blocked" | "completed"
