import { Effect, Queue, Stream } from "effect"
import { Subscription } from "foldkit"

import { FoldkitDemoMounted, FoldkitDemoReceivedHostPort } from "./message.js"
import type { KhalaCodeFoldkitMessage } from "./message.js"
import type { KhalaCodeFoldkitModel } from "./model.js"
import type { KhalaCodeFoldkitHostPort } from "./ports.js"

const mountedStream = Stream.make(FoldkitDemoMounted())

const hostPortStream = (
  port: KhalaCodeFoldkitHostPort,
): Stream.Stream<KhalaCodeFoldkitMessage> =>
  Stream.callback<KhalaCodeFoldkitMessage>((queue) =>
    Effect.acquireRelease(
      Effect.sync(() =>
        port.subscribe((message) => {
          Queue.offerUnsafe(queue, FoldkitDemoReceivedHostPort({ message }))
        }),
      ),
      (unsubscribe) => Effect.sync(unsubscribe),
    ),
  )

export const makeKhalaCodeFoldkitSubscriptions = (
  port: KhalaCodeFoldkitHostPort,
) =>
  Subscription.make<KhalaCodeFoldkitModel, KhalaCodeFoldkitMessage>()(() => ({
    mounted: Subscription.persistent(mountedStream),
    hostPort: Subscription.persistent(hostPortStream(port)),
  }))
