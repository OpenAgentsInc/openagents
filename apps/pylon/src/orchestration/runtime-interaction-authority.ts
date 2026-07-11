import {
  decodeRuntimeInteraction,
  type RuntimeInteraction,
} from "@openagentsinc/agent-runtime-schema"
import {
  type PylonRuntimeInteractionAuthority,
} from "@openagentsinc/pylon-core/executor/runtime-interaction-bridge"
import { Effect } from "effect"

export const RUNTIME_INTERACTION_ROUTE_PATH =
  "/api/internal/khala-sync/runtime-interaction"

export const createRuntimeInteractionHttpAuthority = (input: Readonly<{
  baseUrl: string
  adminToken: string
  ownerUserId: string
  fetchImpl?: typeof globalThis.fetch
  pollIntervalMs?: number
}>): PylonRuntimeInteractionAuthority => {
  const fetchImpl = input.fetchImpl ?? globalThis.fetch
  const baseUrl = input.baseUrl.replace(/\/+$/, "")
  const requestJson = (
    url: string,
    init: RequestInit,
  ): Effect.Effect<unknown, Error> => Effect.tryPromise({
    try: signal => fetchImpl(url, { ...init, signal }).then(async response => {
      if (!response.ok) throw new Error(`runtime interaction authority returned ${response.status}`)
      return response.json()
    }),
    catch: error => error instanceof Error ? error : new Error("runtime interaction authority failed"),
  })
  const headers = {
    authorization: `Bearer ${input.adminToken}`,
    "content-type": "application/json",
  }

  return {
    request: interaction => Effect.asVoid(requestJson(
      `${baseUrl}${RUNTIME_INTERACTION_ROUTE_PATH}`,
      {
        method: "POST",
        headers,
        body: JSON.stringify({ ownerUserId: input.ownerUserId, interaction }),
      },
    ).pipe(Effect.flatMap(body =>
      (body as { ok?: unknown }).ok === true
        ? Effect.void
        : Effect.fail(new Error("runtime interaction request was not applied"))))),
    awaitTerminal: interactionRef => {
      const read = (): Effect.Effect<RuntimeInteraction, Error> => requestJson(
        `${baseUrl}${RUNTIME_INTERACTION_ROUTE_PATH}?ownerUserId=${encodeURIComponent(input.ownerUserId)}&interactionRef=${encodeURIComponent(interactionRef)}`,
        { method: "GET", headers: { authorization: `Bearer ${input.adminToken}` } },
      ).pipe(Effect.flatMap(body => Effect.try({
        try: () => decodeRuntimeInteraction((body as { interaction?: unknown }).interaction),
        catch: () => new Error("runtime interaction response failed canonical decode"),
      })))

      const expire = (interaction: RuntimeInteraction): Effect.Effect<void, Error> =>
        Effect.asVoid(requestJson(`${baseUrl}${RUNTIME_INTERACTION_ROUTE_PATH}`, {
          method: "POST",
          headers,
          body: JSON.stringify({
            action: "expire",
            ownerUserId: input.ownerUserId,
            interactionRef: interaction.interactionRef,
            threadId: interaction.threadId,
            turnId: interaction.turnId,
          }),
        }).pipe(Effect.flatMap(body =>
          (body as { ok?: unknown }).ok === true
            ? Effect.void
            : Effect.fail(new Error("runtime interaction expiry was not applied")))))

      const loop = (): Effect.Effect<RuntimeInteraction, Error> => Effect.gen(function*() {
        let interaction = yield* read()
        if (interaction.lifecycle.status !== "pending") return interaction
        if (Date.now() >= Date.parse(interaction.expiresAt)) {
          yield* expire(interaction)
          interaction = yield* read()
          if (interaction.lifecycle.status !== "pending") return interaction
        }
        yield* Effect.sleep(`${input.pollIntervalMs ?? 500} millis`)
        return yield* loop()
      })
      return loop()
    },
  }
}
