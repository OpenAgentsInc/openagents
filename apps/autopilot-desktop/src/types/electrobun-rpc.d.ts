export type RPCRequestSpec = Readonly<{
  params?: unknown
  response?: unknown
}>

export type RPCRequestsSchema = Readonly<Record<string, RPCRequestSpec>>

export type RPCMessagesSchema = Readonly<Record<string, unknown>>

export type RPCSchema = Readonly<{
  requests: RPCRequestsSchema
  messages: RPCMessagesSchema
}>

export type ElectrobunRPCSchema = Readonly<{
  bun: RPCSchema
  webview: RPCSchema
}>

export type RPCRequestParams<
  Requests extends RPCRequestsSchema,
  Method extends keyof Requests,
> = "params" extends keyof Requests[Method]
  ? Requests[Method]["params"]
  : never

export type RPCRequestResponse<
  Requests extends RPCRequestsSchema,
  Method extends keyof Requests,
> = "response" extends keyof Requests[Method]
  ? Requests[Method]["response"]
  : void

export type RPCRequestHandlerObject<Requests extends RPCRequestsSchema> = {
  [Method in keyof Requests]?: (
    ...args: "params" extends keyof Requests[Method]
      ? undefined extends Requests[Method]["params"]
        ? [params?: Requests[Method]["params"]]
        : [params: Requests[Method]["params"]]
      : []
  ) =>
    | Awaited<RPCRequestResponse<Requests, Method>>
    | Promise<Awaited<RPCRequestResponse<Requests, Method>>>
} & {
  _?: (
    method: keyof Requests,
    params: Requests[keyof Requests] extends RPCRequestSpec
      ? RPCRequestParams<Requests, keyof Requests>
      : never,
  ) => unknown
}

export type RPCMessageHandlerObject<Messages extends RPCMessagesSchema> = {
  [Message in keyof Messages]?: (payload: Messages[Message]) => void
} & {
  "*"?: (
    messageName: keyof Messages,
    payload: Messages[keyof Messages],
  ) => void
}

export type ElectrobunRPCConfig<
  Schema extends ElectrobunRPCSchema,
  Side extends keyof Schema,
> = Readonly<{
  maxRequestTime?: number
  handlers: Readonly<{
    requests?: RPCRequestHandlerObject<Schema[Side]["requests"]>
    messages?: RPCMessageHandlerObject<Schema[Side]["messages"]>
  }>
}>

type OtherSide<Side extends "bun" | "webview"> =
  Side extends "bun" ? "webview" : "bun"

export type RPCRequestsProxy<Requests extends RPCRequestsSchema> = {
  [Method in keyof Requests]: (
    ...args: "params" extends keyof Requests[Method]
      ? undefined extends Requests[Method]["params"]
        ? [params?: Requests[Method]["params"]]
        : [params: Requests[Method]["params"]]
      : []
  ) => Promise<RPCRequestResponse<Requests, Method>>
}

export type RPCMessagesProxy<Messages extends RPCMessagesSchema> = {
  [Message in keyof Messages]: (
    ...args: Messages[Message] extends void
      ? []
      : undefined extends Messages[Message]
        ? [payload?: Messages[Message]]
        : [payload: Messages[Message]]
  ) => void
}

export type RPCTransport = Readonly<{
  send?: (data: unknown) => void
  registerHandler?: (handler: (data: unknown) => void) => void
  unregisterHandler?: () => void
}>

export type RPCWithTransport = Readonly<{
  setTransport: (transport: RPCTransport) => void
}>

export type ElectrobunRPCInstance<
  Schema extends ElectrobunRPCSchema,
  Side extends "bun" | "webview",
> = RPCWithTransport &
  Readonly<{
    request: RPCRequestsProxy<Schema[OtherSide<Side>]["requests"]>
    send: RPCMessagesProxy<Schema[OtherSide<Side>]["messages"]>
    addMessageListener: <
      Message extends keyof Schema[Side]["messages"],
    >(
      message: Message,
      listener: (payload: Schema[Side]["messages"][Message]) => void,
    ) => void
    removeMessageListener: <
      Message extends keyof Schema[Side]["messages"],
    >(
      message: Message,
      listener: (payload: Schema[Side]["messages"][Message]) => void,
    ) => void
  }>
