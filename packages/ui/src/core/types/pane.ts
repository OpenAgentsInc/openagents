// Platform-agnostic pane types
export interface PanePosition {
  x: number
  y: number
}

export interface PaneSize {
  width: number
  height: number
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
  type: 'move' | 'resize' | 'activate' | 'close' | 'minimize' | 'maximize'
  paneId: string
  payload?: Partial<PaneState>
}