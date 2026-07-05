declare module "electrobun/bun" {
  type RpcMessageHandlers<Messages extends Record<string, (...args: any[]) => any>> = {
    [Message in keyof Messages]?: Messages[Message]
  }

  type RpcMessageProxy<Messages extends Record<string, (...args: any[]) => any>> = {
    [Message in keyof Messages]: (...args: Parameters<Messages[Message]>) => void
  }

  export type RpcDefinition<
    Requests extends Record<string, (...args: any[]) => any>,
    Messages extends Record<string, (...args: any[]) => any> = Record<string, never>,
  > = {
    readonly request: Requests
    readonly send: RpcMessageProxy<Messages>
  }

  export class BrowserView {
    static defineRPC<Schema extends {
      requests: Record<string, (...args: any[]) => any>
      messages?: Record<string, (...args: any[]) => any>
    }>(
      options: {
        readonly maxRequestTime?: number
        readonly handlers: {
          readonly requests: Schema["requests"]
          readonly messages?: RpcMessageHandlers<NonNullable<Schema["messages"]>>
        }
      },
    ): RpcDefinition<Schema["requests"], NonNullable<Schema["messages"]>>
  }

  export class BrowserWindowWebview {
    on(
      name:
        | "will-navigate"
        | "did-navigate"
        | "did-navigate-in-page"
        | "did-commit-navigation"
        | "dom-ready"
        | "download-started"
        | "download-progress"
        | "download-completed"
        | "download-failed",
      handler: (event: unknown) => void,
    ): void
  }

  export class BrowserWindow {
    constructor(options: {
      readonly title: string
      readonly url: string
      readonly frame?: {
        readonly x?: number
        readonly y?: number
        readonly width?: number
        readonly height?: number
      }
      readonly rpc?: unknown
      readonly titleBarStyle?: "hidden" | "hiddenInset" | "default"
      readonly trafficLightOffset?: Readonly<{ x: number; y: number }>
    })
    readonly webview: BrowserWindowWebview
  }

  export type Rectangle = {
    readonly x: number
    readonly y: number
    readonly width: number
    readonly height: number
  }

  export type Display = {
    readonly id: number
    readonly bounds: Rectangle
    readonly workArea: Rectangle
    readonly scaleFactor: number
    readonly isPrimary: boolean
  }

  export const ApplicationMenu: Readonly<{
    setApplicationMenu: (menu: unknown) => void
    on: (
      name: "application-menu-clicked",
      handler: (event: { readonly data?: { readonly action?: string; readonly data?: unknown; readonly id?: number } }) => void,
    ) => void
  }>

  export const Screen: Readonly<{
    getPrimaryDisplay: () => Display
    getAllDisplays: () => readonly Display[]
    getCursorScreenPoint: () => Readonly<{ x: number; y: number }>
    getMouseButtons: () => bigint
  }>

  export type UpdaterLocalInfo = {
    readonly baseUrl: string
    readonly channel: string
    readonly hash: string
    readonly identifier: string
    readonly name: string
    readonly version: string
  }

  // Only the subset of Electrobun's real `Updater` namespace
  // (`electrobun/bun`'s `src/bun/core/Updater.ts`) that #8440's in-app
  // updater plumbing calls is declared here.
  export const Updater: Readonly<{
    applyUpdate: () => Promise<void>
    downloadUpdate: () => Promise<void>
    getLocalInfo: () => Promise<UpdaterLocalInfo>
  }>

  export type MessageBoxOptions = {
    readonly type?: "info" | "warning" | "error" | "question"
    readonly title?: string
    readonly message?: string
    readonly detail?: string
    readonly buttons?: readonly string[]
    readonly defaultId?: number
    readonly cancelId?: number
  }

  export type MessageBoxResponse = {
    readonly response: number
  }

  export const Utils: Readonly<{
    showMessageBox: (options?: MessageBoxOptions) => Promise<MessageBoxResponse>
  }>
}
