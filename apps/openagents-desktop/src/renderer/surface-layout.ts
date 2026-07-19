export const desktopSurfaceLayoutStorageKey = "openagents.desktop.surface-layout.v1"
export const defaultDesktopSurfacePanelWidth = 440

export const desktopSurfaceKinds = ["files", "review", "terminal", "browser"] as const
export type DesktopSurfaceKind = (typeof desktopSurfaceKinds)[number]

export type DesktopSurfaceLayout = Readonly<{
  version: 1
  surfaces: ReadonlyArray<DesktopSurfaceKind>
  active: DesktopSurfaceKind | null
  maximized: boolean
  width: number
}>

export type DesktopSurfaceLayoutAction =
  | Readonly<{ type: "open"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "toggle"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "activate"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "close"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "close_others"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "close_right"; surface: DesktopSurfaceKind }>
  | Readonly<{ type: "close_all" }>
  | Readonly<{ type: "toggle_maximized" }>
  | Readonly<{ type: "resize"; width: number }>

export const defaultDesktopSurfaceLayout = (): DesktopSurfaceLayout => ({
  version: 1,
  surfaces: [],
  active: null,
  maximized: false,
  width: defaultDesktopSurfacePanelWidth,
})

const isSurface = (value: unknown): value is DesktopSurfaceKind =>
  typeof value === "string" && desktopSurfaceKinds.includes(value as DesktopSurfaceKind)

export const clampDesktopSurfaceWidth = (width: number): number =>
  Math.min(960, Math.max(320, Math.round(Number.isFinite(width) ? width : defaultDesktopSurfacePanelWidth)))

/** Total, bounded decoder for renderer-local presentation state. */
export const decodeDesktopSurfaceLayout = (value: unknown): DesktopSurfaceLayout => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return defaultDesktopSurfaceLayout()
  const record = value as Record<string, unknown>
  const surfaces = Array.isArray(record.surfaces)
    ? [...new Set(record.surfaces.filter(isSurface))].slice(0, desktopSurfaceKinds.length)
    : []
  const active = isSurface(record.active) && surfaces.includes(record.active) ? record.active : surfaces.at(-1) ?? null
  return {
    version: 1,
    surfaces,
    active,
    maximized: record.maximized === true && active !== null,
    width: clampDesktopSurfaceWidth(typeof record.width === "number" ? record.width : defaultDesktopSurfacePanelWidth),
  }
}

export const reduceDesktopSurfaceLayout = (
  state: DesktopSurfaceLayout,
  action: DesktopSurfaceLayoutAction,
): DesktopSurfaceLayout => {
  if (action.type === "toggle") {
    return state.active === action.surface
      ? reduceDesktopSurfaceLayout(state, { type: "close", surface: action.surface })
      : reduceDesktopSurfaceLayout(state, { type: "open", surface: action.surface })
  }
  if (action.type === "open") {
    const surfaces = state.surfaces.includes(action.surface) ? state.surfaces : [...state.surfaces, action.surface]
    return { ...state, surfaces, active: action.surface }
  }
  if (action.type === "activate") {
    return state.surfaces.includes(action.surface) ? { ...state, active: action.surface } : state
  }
  if (action.type === "close") {
    const index = state.surfaces.indexOf(action.surface)
    if (index < 0) return state
    const surfaces = state.surfaces.filter(surface => surface !== action.surface)
    const active = state.active === action.surface ? surfaces[Math.min(index, surfaces.length - 1)] ?? null : state.active
    return { ...state, surfaces, active, maximized: active === null ? false : state.maximized }
  }
  if (action.type === "close_others") {
    return state.surfaces.includes(action.surface)
      ? { ...state, surfaces: [action.surface], active: action.surface }
      : state
  }
  if (action.type === "close_right") {
    const index = state.surfaces.indexOf(action.surface)
    if (index < 0) return state
    const surfaces = state.surfaces.slice(0, index + 1)
    return { ...state, surfaces, active: surfaces.includes(state.active as DesktopSurfaceKind) ? state.active : action.surface }
  }
  if (action.type === "close_all") return { ...state, surfaces: [], active: null, maximized: false }
  if (action.type === "toggle_maximized") return state.active === null ? state : { ...state, maximized: !state.maximized }
  return { ...state, width: clampDesktopSurfaceWidth(action.width) }
}
