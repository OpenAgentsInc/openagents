declare module "electrobun/view" {
  export type RpcDefinition<Requests extends Record<string, (...args: any[]) => any>> = {
    readonly request: Requests
  }

  export class Electroview {
    constructor(options: { readonly rpc?: unknown })
    static defineRPC<Schema extends { requests: Record<string, (...args: any[]) => any> }>(
      options: {
        readonly maxRequestTime?: number
        readonly handlers: {
          readonly requests: Partial<Schema["requests"]>
          readonly messages?: Record<string, (...args: any[]) => void>
        }
      },
    ): RpcDefinition<Schema["requests"]>
  }
}
