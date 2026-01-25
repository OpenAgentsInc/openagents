import { Effect, Schema } from "effect"
import { invoke } from "@tauri-apps/api/core"
import type {
  ConnectUnifiedAgentRequest,
  ConnectUnifiedAgentResponse,
  CurrentDirectory,
  DisconnectUnifiedAgentRequest,
  DisconnectUnifiedAgentResponse,
  SendUnifiedMessageRequest,
  SendUnifiedMessageResponse,
  StartUnifiedSessionRequest,
  StartUnifiedSessionResponse,
} from "../../gen/tauri-contracts"
import {
  ConnectUnifiedAgentResponseSchema,
  CurrentDirectorySchema,
  DisconnectUnifiedAgentResponseSchema,
  SendUnifiedMessageResponseSchema,
  StartUnifiedSessionResponseSchema,
} from "../../contracts/tauri"

const invokeWithSchema = <A>(
  command: string,
  payload: any,
  schema: Schema.Schema<A>
): Effect.Effect<A, Error> =>
  Effect.tryPromise({
    try: () => invoke<unknown>(command, payload),
    catch: (error) => new Error(String(error)),
  }).pipe(
    Effect.flatMap((result) =>
      Schema.decodeUnknown(schema)(result).pipe(
        Effect.mapError((error) => new Error(String(error)))
      )
    )
  )

export const getCurrentDirectory = Effect.fn("tauri.getCurrentDirectory")(() =>
  invokeWithSchema<CurrentDirectory>(
    "get_current_directory",
    undefined,
    CurrentDirectorySchema
  )
)

export const connectUnifiedAgent = Effect.fn("tauri.connectUnifiedAgent")(
  (payload: ConnectUnifiedAgentRequest) =>
    invokeWithSchema<ConnectUnifiedAgentResponse>(
      "connect_unified_agent",
      payload,
      ConnectUnifiedAgentResponseSchema
    )
)

export const disconnectUnifiedAgent = Effect.fn("tauri.disconnectUnifiedAgent")(
  (payload: DisconnectUnifiedAgentRequest) =>
    invokeWithSchema<DisconnectUnifiedAgentResponse>(
      "disconnect_unified_agent",
      payload,
      DisconnectUnifiedAgentResponseSchema
    )
)

export const startUnifiedSession = Effect.fn("tauri.startUnifiedSession")(
  (payload: StartUnifiedSessionRequest) =>
    invokeWithSchema<StartUnifiedSessionResponse>(
      "start_unified_session",
      payload,
      StartUnifiedSessionResponseSchema
    )
)

export const sendUnifiedMessage = Effect.fn("tauri.sendUnifiedMessage")(
  (payload: SendUnifiedMessageRequest) =>
    invokeWithSchema<SendUnifiedMessageResponse>(
      "send_unified_message",
      payload,
      SendUnifiedMessageResponseSchema
    )
)
