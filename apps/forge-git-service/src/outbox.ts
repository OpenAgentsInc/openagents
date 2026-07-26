import { Context, Effect, Layer, Schema } from "effect";
import { Relay } from "nostr-effect/wrappers/relay";
import type { Event as NostrEvent } from "nostr-effect/pure";

import { ForgeGitConfiguration } from "./config.js";
import { ForgeGitDatabase } from "./database.js";

export class ForgeGitRelayOutboxError extends Schema.TaggedErrorClass<ForgeGitRelayOutboxError>()(
  "ForgeGitRelayOutboxError",
  { operation: Schema.String },
) {}

export interface ForgeGitRelayOutboxShape {
  /** Publishes a bounded batch; one failed row remains retryable. */
  readonly drain: () => Effect.Effect<number, ForgeGitRelayOutboxError>;
}

export class ForgeGitRelayOutbox extends Context.Service<
  ForgeGitRelayOutbox,
  ForgeGitRelayOutboxShape
>()("@openagentsinc/forge-git-service/RelayOutbox") {}

const parseSignedEvent = (value: string): NostrEvent | undefined => {
  try {
    const parsed = JSON.parse(value) as Partial<NostrEvent>;
    return typeof parsed.id === "string" && typeof parsed.sig === "string" &&
      typeof parsed.pubkey === "string" && typeof parsed.kind === "number" &&
      typeof parsed.created_at === "number" && typeof parsed.content === "string" &&
      Array.isArray(parsed.tags)
      ? (parsed as NostrEvent)
      : undefined;
  } catch {
    return undefined;
  }
};

export const layerRelayOutbox = Layer.effect(
  ForgeGitRelayOutbox,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    const database = yield* ForgeGitDatabase;
    const relayUrl = configuration.relayUrl;

    return ForgeGitRelayOutbox.of({
      drain: Effect.fn("ForgeGitRelayOutbox.drain")(function* () {
        if (relayUrl === undefined) return 0;
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<Readonly<{ event_id: string; event_json: string; outbox_ref: string }>[]>
            `SELECT outbox.outbox_ref, outbox.event_id, state.event_json
               FROM forge_git_relay_outbox AS outbox
               JOIN forge_git_signed_ref_states AS state
                 ON state.tenant_ref = outbox.tenant_ref AND state.event_id = outbox.event_id
              WHERE outbox.state IN ('pending', 'failed')
                AND outbox.available_at <= now()
              ORDER BY outbox.created_at
              LIMIT 20`,
          catch: () => new ForgeGitRelayOutboxError({ operation: "select" }),
        });
        if (rows.length === 0) return 0;
        const relay = new Relay(relayUrl);
        let published = 0;
        try {
          yield* Effect.tryPromise({
            try: () => relay.connect(),
            catch: () => new ForgeGitRelayOutboxError({ operation: "connect" }),
          });
          for (const row of rows) {
            const event = parseSignedEvent(row.event_json);
            if (event === undefined || event.id !== row.event_id) {
              yield* Effect.tryPromise({
                try: () => database.sql`
                  UPDATE forge_git_relay_outbox
                     SET state = 'failed'
                   WHERE outbox_ref = ${row.outbox_ref}`,
                catch: () => new ForgeGitRelayOutboxError({ operation: "record_invalid" }),
              });
              continue;
            }
            const outcome = yield* Effect.result(
              Effect.tryPromise({
                try: () => relay.publish(event),
                catch: () => new ForgeGitRelayOutboxError({ operation: "publish" }),
              }),
            );
            if (outcome._tag === "Failure") {
              yield* Effect.tryPromise({
                try: () => database.sql`
                  UPDATE forge_git_relay_outbox
                     SET state = 'failed'
                   WHERE outbox_ref = ${row.outbox_ref}`,
                catch: () => new ForgeGitRelayOutboxError({ operation: "record_failure" }),
              });
              continue;
            }
            yield* Effect.tryPromise({
              try: () => database.sql`
                UPDATE forge_git_relay_outbox
                   SET state = 'published', published_at = now()
                 WHERE outbox_ref = ${row.outbox_ref}`,
              catch: () => new ForgeGitRelayOutboxError({ operation: "record_success" }),
            });
            published += 1;
          }
        } finally {
          relay.close();
        }
        return published;
      }),
    });
  }),
);

export const layerNoopRelayOutbox = Layer.succeed(
  ForgeGitRelayOutbox,
  ForgeGitRelayOutbox.of({ drain: () => Effect.succeed(0) }),
);
