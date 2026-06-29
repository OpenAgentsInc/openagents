import type {
  ElectrobunRPCConfig,
  ElectrobunRPCInstance,
  ElectrobunRPCSchema,
  RPCWithTransport,
} from "./electrobun-rpc.js"

export type WindowOptionsType<T = RPCWithTransport> = Readonly<{
  activate?: boolean
  frame: Readonly<{ x: number; y: number; width: number; height: number }>
  hidden?: boolean
  html: string | null
  navigationRules?: string | null
  passthrough?: boolean
  preload: string | null
  renderer?: "native" | "cef"
  rpc?: T
  sandbox?: boolean
  styleMask?: Record<string, boolean>
  title: string
  titleBarStyle?: "hidden" | "hiddenInset" | "default"
  trafficLightOffset?: Readonly<{ x: number; y: number }>
  transparent?: boolean
  url: string | null
  viewsRoot: string | null
}>

export type BrowserViewOptions<T = RPCWithTransport> = Readonly<{
  autoResize?: boolean
  frame?: Readonly<{ x: number; y: number; width: number; height: number }>
  hostWebviewId?: number
  html?: string | null
  navigationRules?: string | null
  partition?: string | null
  preload?: string | null
  renderer?: "native" | "cef"
  rpc?: T
  sandbox?: boolean
  startPassthrough?: boolean
  startTransparent?: boolean
  url?: string | null
  viewsRoot?: string | null
  windowId?: number
}>

export class BrowserView<T extends RPCWithTransport = RPCWithTransport> {
  readonly id: number
  constructor(options?: Partial<BrowserViewOptions<T>>)
  on(name: string, handler: (event: unknown) => void): void
  executeJavascript(js: string): void
  loadHTML(html: string): void
  loadURL(url: string): void
  remove(): void
  static defineRPC<Schema extends ElectrobunRPCSchema>(
    config: ElectrobunRPCConfig<Schema, "bun">,
  ): ElectrobunRPCInstance<Schema, "bun">
}

export class BrowserWindow<T extends RPCWithTransport = RPCWithTransport> {
  readonly id: number
  readonly webview: BrowserView<T>
  constructor(options?: Partial<WindowOptionsType<T>>)
  on(name: string, handler: (event: unknown) => void): void
}

export const ApplicationMenu: Readonly<{
  setApplicationMenu: (menu: unknown) => void
}>

export const PATHS: Readonly<{
  RESOURCES_FOLDER: string
}>

export const Updater: unknown
