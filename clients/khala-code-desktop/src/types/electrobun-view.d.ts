declare module "electrobun/view" {
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

  export class Electroview {
    constructor(options: { readonly rpc?: unknown })
    static defineRPC<Schema extends {
      requests: Record<string, (...args: any[]) => any>
      messages?: Record<string, (...args: any[]) => any>
    }>(
      options: {
        readonly maxRequestTime?: number
        readonly handlers: {
          readonly requests: Partial<Schema["requests"]>
          readonly messages?: RpcMessageHandlers<NonNullable<Schema["messages"]>>
        }
      },
    ): RpcDefinition<Schema["requests"], NonNullable<Schema["messages"]>>
  }
}
