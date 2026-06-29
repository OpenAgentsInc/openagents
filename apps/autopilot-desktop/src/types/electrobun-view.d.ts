import type {
  ElectrobunRPCConfig,
  ElectrobunRPCInstance,
  ElectrobunRPCSchema,
  RPCWithTransport,
} from "./electrobun-rpc.js"

export class Electroview<T extends RPCWithTransport = RPCWithTransport> {
  constructor(config: Readonly<{ rpc: T }>)
  static defineRPC<Schema extends ElectrobunRPCSchema>(
    config: ElectrobunRPCConfig<Schema, "webview">,
  ): ElectrobunRPCInstance<Schema, "webview">
}

export default Electroview
