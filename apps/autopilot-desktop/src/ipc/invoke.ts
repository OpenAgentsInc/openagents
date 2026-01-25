import { Effect, Schema } from "effect"
import { invoke } from "@tauri-apps/api/core"

export const invokeWithSchema = <A>(
  command: string,
  payload: unknown,
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
