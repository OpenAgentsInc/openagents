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
    })
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
  }>

  export const Screen: Readonly<{
    getPrimaryDisplay: () => Display
    getAllDisplays: () => readonly Display[]
    getCursorScreenPoint: () => Readonly<{ x: number; y: number }>
    getMouseButtons: () => bigint
  }>
}
