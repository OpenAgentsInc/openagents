declare module "electrobun/bun" {
  export type RpcDefinition<Requests extends Record<string, (...args: any[]) => any>> = {
    readonly request: Requests
  }

  export class BrowserView {
    static defineRPC<Schema extends { requests: Record<string, (...args: any[]) => any> }>(
      options: {
        readonly maxRequestTime?: number
        readonly handlers: {
          readonly requests: Schema["requests"]
          readonly messages?: Record<string, (...args: any[]) => void>
        }
      },
    ): RpcDefinition<Schema["requests"]>
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

  export const ApplicationMenu: Readonly<{
    setApplicationMenu: (menu: unknown) => void
  }>
}
