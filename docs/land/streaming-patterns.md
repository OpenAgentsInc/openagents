# Land Streaming Patterns

This document details Land's comprehensive streaming patterns using Effect's Stream API, replacing polling architectures with reactive, backpressure-aware streams.

## Core Streaming Architecture

### From Polling to Streaming

```typescript
// BEFORE: Polling approach (inefficient)
const pollMessages = (sessionId: string) =>
  Effect.gen(function* () {
    while (true) {
      const messages = yield* getMessages(sessionId)
      yield* updateUI(messages)
      yield* Effect.sleep('50 millis')
    }
  })

// AFTER: Stream-based approach (efficient)
const streamMessages = (sessionId: string) =>
  createMessageStream(sessionId).pipe(
    Stream.tap(message => updateUI([message])),
    Stream.runDrain
  )
```

## Tauri Event Streaming

### Basic Event Stream

```typescript
// Wind/Integration/Tauri/Events/Stream.ts
export const createTauriEventStream = <T>(eventName: string) =>
  Stream.async<T>((emit) => {
    const setup = Effect.gen(function* () {
      const unlisten = yield* Effect.promise(() =>
        listen<T>(eventName, (event) => {
          // Emit each event to the stream
          emit(Effect.succeed(event.payload))
        })
      )
      
      // Return cleanup function
      return Effect.sync(() => {
        unlisten()
      })
    })
    
    return setup
  })

// Usage
export const fileChangeStream = createTauriEventStream<FileChangeEvent>('file:change').pipe(
  Stream.filter(event => event.type === 'modified'),
  Stream.debounce('100 millis')
)
```

### Typed Event Streams

```typescript
// Wind/Integration/Tauri/Events/Typed.ts
export class TauriEventStreams extends Effect.Service<TauriEventStreams>()(
  'TauriEventStreams',
  {
    sync: () => ({
      // Terminal output stream
      terminalOutput: (terminalId: string) =>
        createTauriEventStream<TerminalData>(`terminal:${terminalId}:output`).pipe(
          Stream.map(data => ({
            terminalId,
            data: new TextDecoder().decode(data),
            timestamp: Date.now()
          }))
        ),
      
      // Extension host messages
      extensionMessages: () =>
        createTauriEventStream<ExtensionMessage>('extension:message').pipe(
          Stream.filter(msg => msg.type !== 'heartbeat'),
          Stream.tap(msg => Effect.logDebug(`Extension message: ${msg.type}`))
        ),
      
      // File system events
      fileSystemEvents: (path: string) =>
        createTauriEventStream<FSEvent>(`fs:watch:${path}`).pipe(
          Stream.groupByKey(event => event.path, {
            bufferSize: 100
          })
        ),
      
      // Language server protocol events
      lspEvents: (serverId: string) =>
        createTauriEventStream<LSPMessage>(`lsp:${serverId}`).pipe(
          Stream.filter(msg => msg.method !== '$/cancelRequest')
        )
    })
  }
) {}
```

## Message Queue Patterns

### Bounded Queue with Backpressure

```typescript
// Wind/Core/MessageQueue.ts
export const createMessageQueue = <T>(options: QueueOptions = {}) =>
  Effect.gen(function* () {
    const queue = yield* options.bounded
      ? Queue.bounded<T>(options.size || 100)
      : Queue.unbounded<T>()
    
    // Monitor queue metrics
    const metrics = yield* Ref.make({
      enqueued: 0,
      dequeued: 0,
      dropped: 0,
      maxSize: 0
    })
    
    const updateMetrics = (type: 'enqueue' | 'dequeue' | 'drop') =>
      Ref.update(metrics, m => ({
        ...m,
        [type === 'enqueue' ? 'enqueued' : type === 'dequeue' ? 'dequeued' : 'dropped']: m[type] + 1,
        maxSize: Math.max(m.maxSize, m.enqueued - m.dequeued)
      }))
    
    return {
      offer: (item: T) =>
        Queue.offer(queue, item).pipe(
          Effect.tap(() => updateMetrics('enqueue')),
          Effect.catchTag('QueueFullError', () =>
            options.strategy === 'drop-newest'
              ? Effect.succeed(false)
              : options.strategy === 'drop-oldest'
              ? Effect.gen(function* () {
                  yield* Queue.take(queue)
                  yield* updateMetrics('drop')
                  return yield* Queue.offer(queue, item)
                })
              : Effect.fail(new QueueFullError())
          )
        ),
      
      stream: Stream.fromQueue(queue),
      
      metrics: () => Ref.get(metrics),
      
      shutdown: () => Queue.shutdown(queue)
    }
  })
```

### Priority Queue Pattern

```typescript
// Wind/Core/PriorityQueue.ts
export interface PriorityItem<T> {
  priority: number
  item: T
  timestamp: number
}

export const createPriorityQueue = <T>() =>
  Effect.gen(function* () {
    const queues = {
      high: yield* Queue.bounded<PriorityItem<T>>(50),
      normal: yield* Queue.bounded<PriorityItem<T>>(100),
      low: yield* Queue.bounded<PriorityItem<T>>(200)
    }
    
    const offer = (item: T, priority: 'high' | 'normal' | 'low' = 'normal') =>
      Queue.offer(queues[priority], {
        priority: priority === 'high' ? 0 : priority === 'normal' ? 1 : 2,
        item,
        timestamp: Date.now()
      })
    
    // Merge streams with priority
    const stream = Stream.mergeAll([
      Stream.fromQueue(queues.high),
      Stream.fromQueue(queues.normal),
      Stream.fromQueue(queues.low)
    ], {
      concurrency: 3,
      bufferSize: 10
    }).pipe(
      Stream.map(item => item.item)
    )
    
    return { offer, stream }
  })
```

## Real-time Data Processing

### Stream Transformation Pipeline

```typescript
// Wind/Streaming/Pipeline.ts
export const createDataPipeline = <T, U>(
  source: Stream.Stream<T>,
  transform: (item: T) => Effect.Effect<U>
) =>
  source.pipe(
    // Add timestamp
    Stream.map(item => ({
      item,
      timestamp: Date.now()
    })),
    
    // Buffer for performance
    Stream.buffer({
      capacity: 100,
      strategy: 'sliding'
    }),
    
    // Transform with concurrency control
    Stream.mapEffect(
      ({ item, timestamp }) =>
        transform(item).pipe(
          Effect.map(result => ({ result, timestamp, latency: Date.now() - timestamp })),
          Effect.catchAll(error => 
            Effect.succeed({ 
              result: null as U | null, 
              timestamp, 
              error 
            })
          )
        ),
      { concurrency: 5 }
    ),
    
    // Monitor performance
    Stream.tap(({ latency }) =>
      latency > 100 
        ? Effect.logWarning(`High latency: ${latency}ms`)
        : Effect.unit
    ),
    
    // Filter out errors if needed
    Stream.filter(({ result }) => result !== null),
    
    // Extract result
    Stream.map(({ result }) => result!)
  )
```

### Debouncing and Throttling

```typescript
// Wind/Streaming/FlowControl.ts
export class StreamFlowControl extends Effect.Service<StreamFlowControl>()(
  'StreamFlowControl',
  {
    sync: () => ({
      debounce: <T>(duration: Duration.Duration) => 
        (stream: Stream.Stream<T>) =>
          stream.pipe(
            Stream.debounce(duration),
            Stream.tap(() => Effect.logDebug(`Debounced after ${duration}`))
          ),
      
      throttle: <T>(duration: Duration.Duration) =>
        (stream: Stream.Stream<T>) =>
          stream.pipe(
            Stream.throttle({
              cost: () => 1,
              duration,
              units: 1
            })
          ),
      
      batch: <T>(size: number, timeout: Duration.Duration) =>
        (stream: Stream.Stream<T>) =>
          stream.pipe(
            Stream.groupedWithin(size, timeout),
            Stream.map(chunk => Chunk.toReadonlyArray(chunk))
          ),
      
      rateLimit: <T>(maxPerSecond: number) =>
        (stream: Stream.Stream<T>) =>
          stream.pipe(
            Stream.schedule(
              Schedule.fixed('1 second').pipe(
                Schedule.map(() => Math.floor(1000 / maxPerSecond))
              )
            )
          )
    })
  }
) {}
```

## WebSocket Streaming

### WebSocket Stream Integration

```typescript
// Wind/Integration/WebSocket/Stream.ts
export const createWebSocketStream = (url: string, options?: WebSocketOptions) =>
  Stream.async<WebSocketMessage>((emit) => {
    const connect = Effect.gen(function* () {
      const ws = yield* Effect.acquireRelease(
        Effect.sync(() => new WebSocket(url)),
        (ws) => Effect.sync(() => {
          ws.close()
        })
      )
      
      // Setup event handlers
      ws.onmessage = (event) => {
        emit(Effect.succeed({
          type: 'message',
          data: event.data,
          timestamp: Date.now()
        }))
      }
      
      ws.onerror = (error) => {
        emit(Effect.fail(new WebSocketError({ cause: error })))
      }
      
      ws.onclose = (event) => {
        if (!event.wasClean) {
          emit(Effect.fail(new WebSocketClosedError({ 
            code: event.code,
            reason: event.reason 
          })))
        }
        emit(Effect.fail(new StreamEndError()))
      }
      
      // Wait for connection
      yield* Effect.promise(() => new Promise<void>((resolve, reject) => {
        ws.onopen = () => resolve()
        setTimeout(() => reject(new Error('Connection timeout')), options?.timeout || 5000)
      }))
      
      return ws
    })
    
    return connect.pipe(
      Effect.map(ws => Effect.sync(() => ws.close()))
    )
  })
```

### Reconnecting WebSocket

```typescript
// Wind/Integration/WebSocket/Reconnecting.ts
export const createReconnectingWebSocketStream = (
  url: string,
  options: ReconnectOptions = {}
) =>
  Effect.gen(function* () {
    const reconnectSchedule = Schedule.exponential('1 second').pipe(
      Schedule.union(Schedule.spaced('30 seconds')),
      Schedule.jittered
    )
    
    const stream = Stream.repeatEffect(
      createWebSocketStream(url).pipe(
        Stream.retry(reconnectSchedule),
        Stream.tap(() => Effect.logInfo(`WebSocket connected: ${url}`)),
        Stream.onError((cause) => 
          Effect.logError(`WebSocket error: ${cause}`)
        )
      )
    ).pipe(
      Stream.flatten,
      Stream.tap(msg => Effect.logDebug(`WS message: ${msg.type}`))
    )
    
    return stream
  })
```

## File Streaming

### Large File Processing

```typescript
// Wind/Streaming/FileStream.ts
export const streamLargeFile = (path: string, options?: FileStreamOptions) =>
  Stream.gen(function* () {
    const handle = yield* FileHandle.open(path, 'read')
    
    const readStream = yield* Effect.sync(() =>
      handle.createReadStream({
        highWaterMark: options?.chunkSize || 64 * 1024,
        encoding: options?.encoding
      })
    )
    
    return Stream.fromAsyncIterable(
      readStream,
      (error) => new FileReadError({ path, cause: error })
    )
  }).pipe(
    Stream.flatten,
    Stream.tap(chunk => 
      Effect.logDebug(`Read ${chunk.length} bytes from ${path}`)
    )
  )

// Process file in chunks
export const processLargeFile = (
  path: string,
  processor: (chunk: Buffer) => Effect.Effect<void>
) =>
  streamLargeFile(path).pipe(
    Stream.mapEffect(processor, { concurrency: 3 }),
    Stream.runDrain
  )
```

## Terminal Output Streaming

### PTY Stream Management

```typescript
// Wind/Terminal/Stream.ts
export const createTerminalStream = (command: string, args: string[] = []) =>
  Effect.gen(function* () {
    const pty = yield* spawnPTY(command, args)
    
    const outputStream = Stream.async<TerminalOutput>((emit) => {
      pty.onData((data) => {
        emit(Effect.succeed({
          type: 'stdout',
          data,
          timestamp: Date.now()
        }))
      })
      
      pty.onExit(({ exitCode, signal }) => {
        emit(Effect.succeed({
          type: 'exit',
          exitCode,
          signal,
          timestamp: Date.now()
        }))
        emit(Effect.fail(new StreamEndError()))
      })
      
      return Effect.sync(() => pty.kill())
    })
    
    const inputSink = Sink.forEach<string>(
      (input) => Effect.sync(() => pty.write(input))
    )
    
    return {
      output: outputStream,
      input: inputSink,
      resize: (cols: number, rows: number) => 
        Effect.sync(() => pty.resize(cols, rows))
    }
  })
```

## Performance Monitoring

### Stream Metrics

```typescript
// Wind/Streaming/Metrics.ts
export const withStreamMetrics = <T>(name: string) =>
  (stream: Stream.Stream<T>) => {
    const metrics = {
      itemsProcessed: 0,
      bytesProcessed: 0,
      errors: 0,
      startTime: Date.now()
    }
    
    return stream.pipe(
      Stream.tap(() => {
        metrics.itemsProcessed++
      }),
      Stream.tap((item) => {
        if (typeof item === 'string' || item instanceof Buffer) {
          metrics.bytesProcessed += item.length
        }
      }),
      Stream.catchAll((error) => {
        metrics.errors++
        return Stream.fail(error)
      }),
      Stream.ensuring(
        Effect.sync(() => {
          const duration = Date.now() - metrics.startTime
          const throughput = metrics.itemsProcessed / (duration / 1000)
          
          console.log(`Stream '${name}' metrics:`, {
            ...metrics,
            duration: `${duration}ms`,
            throughput: `${throughput.toFixed(2)} items/sec`
          })
        })
      )
    )
  }
```

## Key Patterns for OpenAgents

1. **Replace All Polling**: Use streams for real-time data
2. **Implement Backpressure**: Use bounded queues and flow control
3. **Handle Reconnection**: Build resilient stream sources
4. **Monitor Performance**: Add metrics to all streams
5. **Use Typed Events**: Create type-safe event streams
6. **Batch Processing**: Group items for efficiency
7. **Error Recovery**: Implement retry strategies for streams
8. **Resource Cleanup**: Ensure streams are properly terminated

## Implementation Checklist

- [ ] Convert all polling loops to Effect streams
- [ ] Implement typed Tauri event streams
- [ ] Add backpressure handling with queues
- [ ] Create reconnecting WebSocket streams
- [ ] Implement file streaming for large files
- [ ] Add flow control (debounce, throttle, batch)
- [ ] Monitor stream performance metrics
- [ ] Test stream error recovery scenarios