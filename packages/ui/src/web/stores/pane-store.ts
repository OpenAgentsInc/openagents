import { create } from "zustand"
import type { Pane, PaneInput } from "../../core/types/pane.js"

interface PaneStore {
  panes: Array<Pane>
  activePane: string | null
  nextZIndex: number

  // Actions
  addPane: (paneInput: PaneInput) => string
  removePane: (id: string) => void
  updatePane: (id: string, updates: Partial<Pane>) => void
  movePane: (id: string, x: number, y: number) => void
  resizePane: (id: string, width: number, height: number) => void
  activatePane: (id: string) => void
  minimizePane: (id: string) => void
  maximizePane: (id: string) => void
  restorePane: (id: string) => void
}

export const usePaneStore = create<PaneStore>((set, get) => ({
  panes: [],
  activePane: null,
  nextZIndex: 1,

  addPane: (paneInput) => {
    const id = paneInput.id || `pane-${Date.now()}`
    const { nextZIndex, panes } = get()

    // Calculate default position if not provided
    const x = paneInput.x ?? 50 + panes.length * 20
    const y = paneInput.y ?? 50 + panes.length * 20
    const width = paneInput.width ?? 400
    const height = paneInput.height ?? 300

    const newPane: Pane = {
      ...paneInput,
      id,
      x,
      y,
      width,
      height,
      isActive: true,
      zIndex: nextZIndex,
      minimized: false,
      maximized: false
    }

    set({
      panes: [...panes, newPane],
      activePane: id,
      nextZIndex: nextZIndex + 1
    })

    // Deactivate other panes
    get().panes.forEach((pane) => {
      if (pane.id !== id) {
        get().updatePane(pane.id, { isActive: false })
      }
    })

    return id
  },

  removePane: (id) => {
    set((state) => ({
      panes: state.panes.filter((pane) => pane.id !== id),
      activePane: state.activePane === id ? null : state.activePane
    }))
  },

  updatePane: (id, updates) => {
    set((state) => ({
      panes: state.panes.map((pane) => pane.id === id ? { ...pane, ...updates } : pane)
    }))
  },

  movePane: (id, x, y) => {
    get().updatePane(id, { x, y })
  },

  resizePane: (id, width, height) => {
    get().updatePane(id, { width, height })
  },

  activatePane: (id) => {
    const { nextZIndex, panes } = get()

    // Deactivate all panes and activate the target one
    set({
      panes: panes.map((pane) => ({
        ...pane,
        isActive: pane.id === id,
        zIndex: pane.id === id ? nextZIndex : pane.zIndex
      })),
      activePane: id,
      nextZIndex: nextZIndex + 1
    })
  },

  minimizePane: (id) => {
    get().updatePane(id, { minimized: true })
  },

  maximizePane: (id) => {
    get().updatePane(id, { maximized: true })
  },

  restorePane: (id) => {
    get().updatePane(id, { minimized: false, maximized: false })
  }
}))
