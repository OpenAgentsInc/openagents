# Land Pattern Implementation Guide for OpenAgents

This guide provides a practical roadmap for implementing Land's proven patterns in the OpenAgents codebase, organized by priority and dependencies.

## Implementation Phases

### Phase 1: Foundation (Week 1)
Focus on core patterns that everything else builds upon.

#### 1.1 Type-Safe IPC Layer
```typescript
// apps/desktop/src/services/ipc/commands.ts
export const Commands = {
  // File system commands
  readFile: createCommand<{ path: string }, Uint8Array>('fs:read_file'),
  writeFile: createCommand<{ path: string; content: Uint8Array }, void>('fs:write_file'),
  
  // Session commands
  createSession: createCommand<{ projectPath: string }, string>('session:create'),
  sendMessage: createCommand<{ sessionId: string; message: string }, void>('session:send_message'),
  
  // Agent commands
  startAgent: createCommand<{ agentId: string; config: AgentConfig }, void>('agent:start'),
  stopAgent: createCommand<{ agentId: string }, void>('agent:stop')
}

// Effect wrapper
export const createCommand = <TArgs, TResult>(name: string) => ({
  name,
  invoke: (args: TArgs) =>
    Effect.tryPromise({
      try: () => invoke<TResult>(name, args),
      catch: (error) => new IPCError({ command: name, args, cause: error })
    })
})
```

#### 1.2 Service Layer Foundation
```typescript
// apps/desktop/src/services/core/Service.ts
export abstract class BaseService<T> extends Effect.Service<T>()<T> {
  static create<T>(tag: string, implementation: T): Layer.Layer<T> {
    return Layer.succeed(this.Tag(tag), implementation)
  }
}

// apps/desktop/src/services/file/FileService.ts
export class FileService extends BaseService<FileService>()('FileService', {
  sync: () => ({
    readFile: (path: string) => Commands.readFile.invoke({ path }),
    writeFile: (path: string, content: Uint8Array) => 
      Commands.writeFile.invoke({ path, content }),
    // ... other methods
  })
}) {}
```

#### 1.3 Error Hierarchy
```typescript
// packages/shared/src/errors/index.ts
export class OpenAgentsError extends Data.TaggedError('OpenAgentsError')<{
  message: string
  context?: Record<string, unknown>
  timestamp: Date
}> {}

export class SessionError extends OpenAgentsError {}
export class SyncError extends OpenAgentsError {}
export class AgentError extends OpenAgentsError {}
// ... domain-specific errors
```

### Phase 2: Resource Management (Week 1-2)
Implement proper cleanup for all resources.

#### 2.1 Event Listener Management
```typescript
// apps/desktop/src/utils/events.ts
export const listenToTauriEvent = <T>(event: string, handler: (data: T) => void) =>
  Effect.acquireRelease(
    Effect.promise(() => listen(event, handler)),
    (unlisten) => Effect.promise(unlisten)
  )

// Usage
export const monitorFileChanges = (path: string) =>
  Effect.scoped(
    listenToTauriEvent<FileChangeEvent>(`file:change:${path}`, (event) => {
      console.log('File changed:', event)
    })
  )
```

#### 2.2 Process Lifecycle
```typescript
// apps/desktop/src/services/agent/ProcessManager.ts
export const spawnAgentProcess = (config: AgentConfig) =>
  Effect.acquireRelease(
    Effect.gen(function* () {
      const process = yield* Effect.tryPromise(() => 
        spawn(config.command, config.args)
      )
      yield* monitorProcessHealth(process)
      return process
    }),
    (process) => gracefulShutdown(process)
  )
```

### Phase 3: Streaming Architecture (Week 2)
Replace polling with reactive streams.

#### 3.1 Message Streaming
```typescript
// apps/desktop/src/services/session/MessageStream.ts
export const createMessageStream = (sessionId: string) =>
  Stream.async<Message>((emit) => {
    const unlisten = listen(`session:${sessionId}:message`, (event) => {
      emit(Effect.succeed(event.payload))
    })
    
    return Effect.promise(unlisten)
  })

// Replace polling in App.tsx
const messageStream = createMessageStream(sessionId).pipe(
  Stream.tap(message => updateMessages(prev => [...prev, message])),
  Stream.runDrain
)
```

#### 3.2 Real-time Sync
```typescript
// apps/mobile/src/services/sync/SyncStream.ts
export const createSyncStream = () =>
  createWebSocketStream(SYNC_URL).pipe(
    Stream.retry(reconnectPolicy),
    Stream.map(msg => JSON.parse(msg.data) as SyncEvent),
    Stream.tap(event => applySyncEvent(event))
  )
```

### Phase 4: Advanced Patterns (Week 2-3)
Implement sophisticated error handling and state management.

#### 4.1 Circuit Breaker for External Services
```typescript
// packages/shared/src/patterns/CircuitBreaker.ts
export const createCircuitBreaker = <T>(
  operation: Effect.Effect<T>,
  config: CircuitBreakerConfig
) => {
  const breaker = new CircuitBreaker(config)
  return breaker.execute(operation)
}

// Usage in AI service
export const callAIService = (prompt: string) =>
  createCircuitBreaker(
    aiClient.complete(prompt),
    {
      maxFailures: 3,
      resetTimeout: Duration.seconds(30),
      shouldTrip: (error) => error._tag === 'NetworkError'
    }
  )
```

#### 4.2 STM for Complex State
```typescript
// apps/desktop/src/state/SessionState.ts
export const createSessionState = () =>
  Effect.gen(function* () {
    const sessions = yield* TMap.empty<string, Session>()
    const activeSession = yield* TRef.make<string | null>(null)
    
    return {
      addSession: (session: Session) =>
        STM.atomically(
          TMap.set(sessions, session.id, session)
        ),
      
      setActive: (sessionId: string) =>
        STM.atomically(
          STM.gen(function* () {
            const exists = yield* TMap.has(sessions, sessionId)
            if (exists) {
              yield* TRef.set(activeSession, sessionId)
            }
          })
        )
    }
  })
```

## Migration Strategy

### Step 1: Wrapper Approach
Start by wrapping existing code without changing behavior:

```typescript
// Before
async function readFile(path: string): Promise<Buffer> {
  return await invoke('read_file', { path })
}

// After (wrapper)
export const readFile = (path: string) =>
  Effect.tryPromise({
    try: () => invoke<Buffer>('read_file', { path }),
    catch: (error) => new FileSystemError({ operation: 'read', path, error })
  })

// Adapter for existing code
export const readFileAsync = (path: string) =>
  Effect.runPromise(readFile(path))
```

### Step 2: Service Migration
Gradually migrate to service-based architecture:

```typescript
// Transition state
export const FileServiceCompat = {
  // New Effect-based API
  readFile: (path: string) => readFileEffect(path),
  
  // Legacy Promise-based API
  readFileAsync: (path: string) => Effect.runPromise(readFileEffect(path))
}
```

### Step 3: Full Integration
Complete migration with proper error handling and composition:

```typescript
// Final state
export const processFile = (path: string) =>
  Effect.gen(function* () {
    const fileService = yield* FileService
    const content = yield* fileService.readFile(path)
    const processed = yield* transform(content)
    yield* fileService.writeFile(`${path}.processed`, processed)
  }).pipe(
    Effect.catchTags({
      FileNotFoundError: () => createEmptyFile(path),
      PermissionError: () => Effect.fail(new UserError('No permission'))
    })
  )
```

## Testing Strategy

### Unit Tests with Mock Services
```typescript
const TestFileService = Layer.succeed(
  FileService,
  FileService.of({
    readFile: () => Effect.succeed(Buffer.from('test')),
    writeFile: () => Effect.unit
  })
)

test('process file', async () => {
  const result = await processFile('/test').pipe(
    Effect.provide(TestFileService),
    Effect.runPromise
  )
  expect(result).toBeDefined()
})
```

### Integration Tests with Real Services
```typescript
test('real file operations', async () => {
  const result = await Effect.gen(function* () {
    const fileService = yield* FileService
    yield* fileService.writeFile('/tmp/test', Buffer.from('data'))
    return yield* fileService.readFile('/tmp/test')
  }).pipe(
    Effect.provide(FileServiceLive),
    Effect.scoped,
    Effect.runPromise
  )
  expect(result.toString()).toBe('data')
})
```

## Performance Considerations

1. **Bundle Size**: Effect adds ~25KB compressed
2. **Runtime Overhead**: Minimal for most operations
3. **Memory Usage**: Streams are more efficient than arrays
4. **Startup Time**: Layer composition is fast

## Common Pitfalls to Avoid

1. **Don't Mix Paradigms**: Avoid Promise/Effect mixing
2. **Use Scoped Resources**: Always clean up with acquireRelease
3. **Handle All Error Cases**: Use exhaustive error handling
4. **Test Error Paths**: Errors are part of the contract
5. **Monitor Performance**: Add metrics to critical paths

## Verification Checklist

- [ ] All Tauri commands wrapped in Effect
- [ ] Services defined with explicit dependencies
- [ ] Resources managed with acquireRelease
- [ ] Polling replaced with streams
- [ ] Errors are tagged and handled exhaustively
- [ ] STM used for complex state coordination
- [ ] Circuit breakers protect external calls
- [ ] Tests cover both success and error paths

## Next Steps

1. Start with Phase 1 (Foundation)
2. Create proof-of-concept for one feature
3. Measure performance impact
4. Gradually expand to other features
5. Document patterns as you go

This implementation guide provides a practical path to adopting Land's battle-tested patterns while maintaining backward compatibility and ensuring a smooth migration.