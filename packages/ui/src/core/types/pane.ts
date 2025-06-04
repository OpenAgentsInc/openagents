// Platform-agnostic pane types
import type { PaneHeaderMenu } from "./pane-menu.js"

export interface PanePosition {
  x: number
  y: number
}

export interface PaneSize {
  width: number
  height: number
}

export interface PaneContent {
  [key: string]: unknown
}

export interface Pane extends PanePosition, PaneSize {
  id: string
  type: string
  title: string
  isActive?: boolean
  dismissable?: boolean
  headerMenus?: PaneHeaderMenu[]
  content?: PaneContent
  zIndex?: number
  minimized?: boolean
  maximized?: boolean
}

export type PaneInput = Omit<Pane, "x" | "y" | "width" | "height" | "id" | "isActive"> & {
  id?: string
  x?: number
  y?: number
  width?: number
  height?: number
}

export interface PaneState {
  id: string
  title: string
  position: PanePosition
  size: PaneSize
  isActive: boolean
  zIndex: number
  minimized?: boolean
  maximized?: boolean
}

export interface PaneAction {
  type: "move" | "resize" | "activate" | "close" | "minimize" | "maximize"
  paneId: string
  payload?: Partial<PaneState>
}