import { Effect, Queue, Stream } from "effect"
import { Subscription } from "foldkit"

import { FleetCockpitMounted, FleetCockpitReceivedHostPort } from "./message.js"
import type { KhalaCodeFleetCockpitMessage } from "./message.js"
import type { KhalaCodeFleetCockpitModel } from "./model.js"
import type { KhalaCodeFleetCockpitHostPort } from "./ports.js"

const mountedStream = Stream.make(FleetCockpitMounted())

const hostPortStream = (
  port: KhalaCodeFleetCockpitHostPort,
): Stream.Stream<KhalaCodeFleetCockpitMessage> =>
  Stream.callback<KhalaCodeFleetCockpitMessage>((queue) =>
    Effect.acquireRelease(
      Effect.sync(() =>
        port.subscribe((message) => {
          Queue.offerUnsafe(queue, FleetCockpitReceivedHostPort({ message }))
        }),
      ),
      (unsubscribe) => Effect.sync(unsubscribe),
    ),
  )

export const makeKhalaCodeFleetCockpitSubscriptions = (
  port: KhalaCodeFleetCockpitHostPort,
) =>
  Subscription.make<KhalaCodeFleetCockpitModel, KhalaCodeFleetCockpitMessage>()(() => ({
    mounted: Subscription.persistent(mountedStream),
    hostPort: Subscription.persistent(hostPortStream(port)),
  }))
