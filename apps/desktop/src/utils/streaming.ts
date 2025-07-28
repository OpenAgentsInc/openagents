import { Stream, Effect, Queue, Schedule, Duration, Chunk } from "effect"

/**
 * Streaming patterns based on Land architecture
 * Replaces polling with reactive streams for better performance and resource usage
 */

// Create an auto-refreshing stream
export const createAutoRefreshStream = <T>(
  fetch: () => Effect.Effect<T>,
  interval: Duration.DurationInput
) =>
  Stream.repeatEffect(fetch()).pipe(
    Stream.schedule(Schedule.fixed(interval)),
    Stream.tap(() => Effect.logDebug(`Auto-refresh data fetched`))
  )

// Create a stream that debounces rapid events
export const createDebouncedStream = <T>(
  source: Stream.Stream<T>,
  duration: Duration.DurationInput
) =>
  source.pipe(
    Stream.debounce(duration),
    Stream.tap(() => Effect.logDebug(`Event debounced after ${duration}`))
  )

// Create a throttled stream
export const createThrottledStream = <T>(
  source: Stream.Stream<T>,
  maxPerSecond: number
) =>
  source.pipe(
    Stream.throttle({
      cost: () => 1,
      units: maxPerSecond,
      duration: Duration.seconds(1)
    })
  )

// Batch events for performance
export const createBatchedStream = <T>(
  source: Stream.Stream<T>,
  batchSize: number,
  timeout: Duration.DurationInput
) =>
  source.pipe(
    Stream.groupedWithin(batchSize, timeout),
    Stream.map((chunk) => Chunk.toReadonlyArray(chunk))
  )

// Create a stream with backpressure handling
export const createBackpressureStream = <T>(
  queueSize: number = 100
) => {
  const create = Effect.gen(function* () {
    const queue = yield* Queue.bounded<T>(queueSize)
    
    const offer = (item: T) =>
      Queue.offer(queue, item).pipe(
        Effect.catchAll(() => {
          // Handle backpressure - drop oldest
          return Effect.gen(function* () {
            yield* Queue.take(queue)
            return yield* Queue.offer(queue, item)
          })
        })
      )
    
    const stream = Stream.fromQueue(queue)
    
    return { offer, stream, shutdown: () => Queue.shutdown(queue) }
  })
  
  return create
}

// Create a reconnecting stream (for WebSockets, SSE, etc.)
export const createReconnectingStream = <T>(
  connect: () => Effect.Effect<Stream.Stream<T>>,
  reconnectSchedule = Schedule.exponential(Duration.seconds(1)).pipe(
    Schedule.union(Schedule.spaced(Duration.seconds(30))),
    Schedule.jittered
  )
) =>
  Stream.repeatEffect(
    connect().pipe(
      Effect.map((stream) =>
        stream.pipe(
          Stream.onError((cause) =>
            Effect.logError("Stream error, will reconnect", cause)
          )
        )
      ),
      Effect.retry(reconnectSchedule)
    )
  ).pipe(Stream.flatten)

// Transform polling to streaming
export const pollToStream = <T>(
  poll: () => Effect.Effect<T>,
  interval: Duration.DurationInput,
  shouldContinue: (value: T) => boolean = () => true
) =>
  Stream.repeatEffect(poll()).pipe(
    Stream.schedule(Schedule.fixed(interval)),
    Stream.takeWhile(shouldContinue),
    Stream.tap(() => Effect.logDebug("Polled value"))
  )

// Create a stream that merges multiple sources
export const createMergedStream = <T>(
  streams: Stream.Stream<T>[],
  strategy: "concurrent" | "sequential" = "concurrent"
) =>
  strategy === "concurrent"
    ? Stream.mergeAll(streams, { concurrency: streams.length })
    : streams.reduce((acc, stream) => Stream.concat(acc, stream), Stream.empty as Stream.Stream<T>)

// Create a stream with state accumulation
export const createStatefulStream = <T, S>(
  source: Stream.Stream<T>,
  initialState: S,
  reducer: (state: S, value: T) => S
) =>
  source.pipe(
    Stream.scan(initialState, reducer)
  )

// Create a stream that filters duplicates
export const createDeduplicatedStream = <T>(
  source: Stream.Stream<T>,
  equals: (a: T, b: T) => boolean = (a, b) => a === b
) => {
  let last: T | undefined
  
  return source.pipe(
    Stream.filter((value) => {
      if (last === undefined || !equals(last, value)) {
        last = value
        return true
      }
      return false
    })
  )
}

// Create a stream with timeout handling
export const createTimeoutStream = <T>(
  source: Stream.Stream<T>,
  timeout: Duration.DurationInput,
  onTimeout: () => T
) =>
  source.pipe(
    Stream.timeoutTo(timeout, Stream.make(onTimeout()))
  )

// Stats streaming specific helper
export const createStatsStream = <T>(
  fetchStats: () => Effect.Effect<T>,
  refreshInterval: Duration.DurationInput = Duration.seconds(10)
) =>
  createAutoRefreshStream(fetchStats, refreshInterval).pipe(
    Stream.catchAll((error) => {
      Effect.logError("Failed to fetch stats", error)
      return Stream.empty
    })
  )

// Message streaming with queue
export const createMessageQueueStream = <T extends { id: string }>(
  bufferSize = 100
) =>
  Effect.gen(function* () {
    const queue = yield* Queue.bounded<T>(bufferSize)
    const processedIds = new Set<string>()
    
    const offer = (message: T) =>
      Effect.gen(function* () {
        if (!processedIds.has(message.id)) {
          processedIds.add(message.id)
          yield* Queue.offer(queue, message)
        }
      })
    
    const stream = Stream.fromQueue(queue).pipe(
      Stream.tap((msg) => Effect.logDebug(`Processing message: ${msg.id}`))
    )
    
    return { offer, stream, shutdown: () => Queue.shutdown(queue) }
  })

// Stream transformation utilities
export const StreamTransformers = {
  // Add timestamps to stream items
  withTimestamp: <T>(stream: Stream.Stream<T>) =>
    stream.pipe(
      Stream.map((value) => ({
        value,
        timestamp: Date.now()
      }))
    ),
  
  // Add sequence numbers
  withSequence: <T>(stream: Stream.Stream<T>) => {
    let sequence = 0
    return stream.pipe(
      Stream.map((value) => ({
        value,
        sequence: sequence++
      }))
    )
  },
  
  // Buffer recent items
  withBuffer: <T>(stream: Stream.Stream<T>, bufferSize: number) => {
    const buffer: T[] = []
    return stream.pipe(
      Stream.map((value) => {
        buffer.push(value)
        if (buffer.length > bufferSize) {
          buffer.shift()
        }
        return {
          current: value,
          buffer: [...buffer]
        }
      })
    )
  }
}

// Stream metrics
export const withStreamMetrics = <T>(name: string) =>
  (stream: Stream.Stream<T>) => {
    let itemCount = 0
    let errorCount = 0
    const startTime = Date.now()
    
    return stream.pipe(
      Stream.tap(() => Effect.sync(() => {
        itemCount++
      })),
      Stream.catchAll((error) => {
        errorCount++
        Effect.logError(`Stream ${name} error:`, error)
        return Stream.fail(error)
      }),
      Stream.ensuring(
        Effect.sync(() => {
          const duration = Date.now() - startTime
          const throughput = itemCount / (duration / 1000)
          console.log(`Stream '${name}' metrics:`, {
            itemCount,
            errorCount,
            duration: `${duration}ms`,
            throughput: `${throughput.toFixed(2)} items/sec`
          })
        })
      )
    )
  }