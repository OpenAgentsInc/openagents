# Land Resource Management Patterns

This document details Land's comprehensive resource management patterns using Effect's Scope, acquireRelease, and finalizer APIs.

## Core Resource Management Pattern

### AcquireRelease for Automatic Cleanup

```typescript
// Basic pattern for any resource
export const createResource = <T>(
  acquire: Effect.Effect<T>,
  release: (resource: T) => Effect.Effect<void>
) =>
  Effect.acquireRelease(acquire, release)

// Example: Event listener management
export const addEventListener = (
  element: HTMLElement,
  event: string,
  handler: EventListener
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      element.addEventListener(event, handler)
      return { element, event, handler }
    }),
    ({ element, event, handler }) =>
      Effect.sync(() => {
        element.removeEventListener(event, handler)
      })
  )
```

## File System Resource Management

### File Handle Management

```typescript
// Wind/Source/Application/FileSystem/FileHandle.ts
export class FileHandle extends Effect.Service<FileHandle>()(
  'FileHandle',
  {
    sync: () => ({
      open: (path: string, mode: 'read' | 'write') =>
        Effect.acquireRelease(
          Effect.gen(function* () {
            const handle = yield* Effect.tryPromise({
              try: () => fs.promises.open(path, mode === 'read' ? 'r' : 'w'),
              catch: (error) => new FileSystemError({
                operation: 'open',
                path,
                cause: error
              })
            })
            
            yield* Effect.logDebug(`Opened file handle: ${path} (mode: ${mode})`)
            return handle
          }),
          (handle) =>
            Effect.gen(function* () {
              yield* Effect.tryPromise({
                try: () => handle.close(),
                catch: (error) => Effect.logError('Failed to close file handle', error)
              })
              yield* Effect.logDebug(`Closed file handle`)
            })
        )
    })
  }
) {}

// Usage with automatic cleanup
export const readLargeFile = (path: string) =>
  Effect.scoped(
    Effect.gen(function* () {
      const handle = yield* FileHandle.open(path, 'read')
      const stream = handle.createReadStream()
      
      // Process stream...
      // Handle is automatically closed when scope ends
    })
  )
```

### File Watcher Resource

```typescript
// Wind/Source/Application/FileSystem/Watcher.ts
export const createFileWatcher = (path: string, options?: WatchOptions) =>
  Effect.gen(function* () {
    const eventQueue = yield* Queue.bounded<FileChangeEvent>(100)
    
    const watcher = yield* Effect.acquireRelease(
      Effect.tryPromise({
        try: () => chokidar.watch(path, options),
        catch: (error) => new FileWatchError({ path, cause: error })
      }),
      (watcher) =>
        Effect.promise(() => watcher.close()).pipe(
          Effect.tap(() => Effect.logDebug(`Closed file watcher for: ${path}`)),
          Effect.orElse(() => Effect.unit)
        )
    )
    
    // Setup event handlers
    yield* Effect.sync(() => {
      watcher.on('add', (filePath) => 
        Queue.offer(eventQueue, { type: 'created', path: filePath })
          .pipe(Effect.runPromise)
      )
      watcher.on('change', (filePath) =>
        Queue.offer(eventQueue, { type: 'changed', path: filePath })
          .pipe(Effect.runPromise)
      )
      watcher.on('unlink', (filePath) =>
        Queue.offer(eventQueue, { type: 'deleted', path: filePath })
          .pipe(Effect.runPromise)
      )
    })
    
    return {
      events: Stream.fromQueue(eventQueue),
      close: () => Queue.shutdown(eventQueue)
    }
  })
```

## Process Management

Based on Cocoon's repository documentation, the extension host implements "process hardening" with automatic termination if the parent process exits. This ensures no orphaned processes.

### Extension Host Process

```typescript
// Mountain/Cocoon process management with hardening
export const spawnExtensionHost = Effect.gen(function* () {
  const logger = yield* Logger
  const config = yield* ExtensionHostConfig
  
  const process = yield* Effect.acquireRelease(
    Effect.gen(function* () {
      yield* logger.info('Spawning extension host process...')
      
      const childProcess = yield* Effect.tryPromise({
        try: () => spawn('node', [
          config.entryPoint,
          '--port', String(config.port),
          '--parent-pid', String(process.pid)
        ], {
          stdio: ['pipe', 'pipe', 'pipe', 'ipc'],
          env: {
            ...process.env,
            VSCODE_EXTENSION_HOST: 'true'
          }
        }),
        catch: (error) => new ExtensionHostSpawnError({ cause: error })
      })
      
      // Monitor process health
      const healthCheckFiber = yield* Effect.fork(
        monitorProcessHealth(childProcess)
      )
      
      return { process: childProcess, healthCheckFiber }
    }),
    ({ process, healthCheckFiber }) =>
      Effect.gen(function* () {
        yield* logger.info('Shutting down extension host...')
        
        // Cancel health monitoring
        yield* Fiber.interrupt(healthCheckFiber)
        
        // Try graceful shutdown first
        const gracefulShutdown = Effect.gen(function* () {
          process.send({ type: 'shutdown' })
          yield* Effect.sleep('5 seconds')
          
          if (!process.killed) {
            process.kill('SIGTERM')
            yield* Effect.sleep('5 seconds')
          }
          
          if (!process.killed) {
            process.kill('SIGKILL')
          }
        })
        
        yield* gracefulShutdown.pipe(
          Effect.timeout('15 seconds'),
          Effect.catchAll(() => 
            Effect.sync(() => process.kill('SIGKILL'))
          )
        )
        
        yield* logger.info('Extension host shut down')
      })
  )
  
  return process
})
```

### Terminal Process Management

```typescript
// Wind/Source/Application/Terminal/Process.ts
export const createTerminal = (options: TerminalOptions) =>
  Effect.gen(function* () {
    const ptyProcess = yield* Effect.acquireRelease(
      Effect.gen(function* () {
        const pty = yield* Effect.tryPromise({
          try: () => nodePty.spawn(
            options.shell || getDefaultShell(),
            options.args || [],
            {
              name: 'xterm-256color',
              cols: options.cols || 80,
              rows: options.rows || 30,
              cwd: options.cwd || process.cwd(),
              env: options.env || process.env
            }
          ),
          catch: (error) => new TerminalSpawnError({ cause: error })
        })
        
        yield* Effect.logDebug(`Spawned terminal process: PID ${pty.pid}`)
        return pty
      }),
      (pty) =>
        Effect.gen(function* () {
          yield* Effect.logDebug(`Killing terminal process: PID ${pty.pid}`)
          
          try {
            if (!pty.pid) return
            
            // Try graceful shutdown
            pty.kill('SIGTERM')
            yield* Effect.sleep('2 seconds')
            
            // Force kill if still alive
            if (isProcessAlive(pty.pid)) {
              pty.kill('SIGKILL')
            }
          } catch (error) {
            yield* Effect.logError('Failed to kill terminal process', error)
          }
        })
    )
    
    // Create streams for I/O
    const outputStream = Stream.async<string>((emit) => {
      ptyProcess.onData((data) => {
        emit(Effect.succeed(data))
      })
      
      ptyProcess.onExit(() => {
        emit(Effect.fail(new TerminalExitedError()))
      })
      
      return Effect.unit
    })
    
    return {
      process: ptyProcess,
      output: outputStream,
      input: (data: string) => Effect.sync(() => ptyProcess.write(data)),
      resize: (cols: number, rows: number) => 
        Effect.sync(() => ptyProcess.resize(cols, rows))
    }
  })
```

## WebView Resource Management

### WebView Panel Lifecycle

```typescript
// Wind/Source/Application/WebView/Panel.ts
export const createWebViewPanel = (options: WebViewPanelOptions) =>
  Effect.gen(function* () {
    const panelId = yield* generatePanelId()
    const eventBus = yield* EventBus
    
    const panel = yield* Effect.acquireRelease(
      Effect.gen(function* () {
        // Create WebView element
        const webview = document.createElement('webview')
        webview.src = options.html
        webview.preload = options.preload
        webview.nodeintegration = false
        webview.contextIsolation = true
        
        // Setup message passing
        const messageQueue = yield* Queue.bounded<WebViewMessage>(100)
        
        webview.addEventListener('ipc-message', (event) => {
          Queue.offer(messageQueue, {
            command: event.channel,
            args: event.args
          }).pipe(Effect.runPromise)
        })
        
        // Add to DOM
        const container = yield* getWebViewContainer()
        container.appendChild(webview)
        
        // Register with panel manager
        yield* registerPanel(panelId, webview)
        
        return {
          id: panelId,
          webview,
          messageQueue
        }
      }),
      ({ id, webview }) =>
        Effect.gen(function* () {
          yield* Effect.logDebug(`Disposing WebView panel: ${id}`)
          
          // Unregister from manager
          yield* unregisterPanel(id)
          
          // Clean up event listeners
          webview.removeAllListeners()
          
          // Remove from DOM
          webview.remove()
          
          // Fire disposal event
          yield* eventBus.emit('webview:disposed', { panelId: id })
        })
    )
    
    return {
      id: panel.id,
      postMessage: (message: any) =>
        Effect.sync(() => panel.webview.send('message', message)),
      onMessage: Stream.fromQueue(panel.messageQueue),
      dispose: () => Effect.unit // Handled by scope
    }
  })
```

## Memory Management Patterns

### Large Data Processing

```typescript
// Wind/Source/Application/Data/LargeFile.ts
export const processLargeFile = (path: string, processor: (chunk: Buffer) => Effect.Effect<void>) =>
  Effect.scoped(
    Effect.gen(function* () {
      // Open file handle - auto-closed on scope exit
      const handle = yield* FileHandle.open(path, 'read')
      
      // Create read stream with backpressure
      const stream = yield* Effect.acquireRelease(
        Effect.sync(() => 
          handle.createReadStream({ 
            highWaterMark: 64 * 1024 // 64KB chunks
          })
        ),
        (stream) => 
          Effect.promise(() => new Promise<void>((resolve) => {
            stream.destroy()
            stream.on('close', resolve)
          }))
      )
      
      // Process chunks with memory management
      yield* Stream.fromAsyncIterable(stream, (error) => 
        new FileReadError({ path, cause: error })
      ).pipe(
        Stream.mapEffect(chunk => processor(chunk), { concurrency: 1 }),
        Stream.runDrain
      )
    })
  )
```

### Cache with Automatic Eviction

```typescript
// Wind/Source/Application/Cache/MemoryCache.ts
export class MemoryCache<K, V> {
  private constructor(
    private cache: Map<K, { value: V; timer: NodeJS.Timeout }>,
    private maxSize: number,
    private ttl: number
  ) {}
  
  static make = <K, V>(options: CacheOptions) =>
    Effect.gen(function* () {
      const cache = new Map<K, { value: V; timer: NodeJS.Timeout }>()
      
      // Cleanup on scope exit
      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          // Clear all timers
          for (const [, entry] of cache) {
            clearTimeout(entry.timer)
          }
          cache.clear()
        })
      )
      
      return new MemoryCache(cache, options.maxSize, options.ttl)
    })
  
  set = (key: K, value: V) =>
    Effect.sync(() => {
      // Remove old entry if exists
      const existing = this.cache.get(key)
      if (existing) {
        clearTimeout(existing.timer)
      }
      
      // Evict if at capacity
      if (this.cache.size >= this.maxSize && !existing) {
        const firstKey = this.cache.keys().next().value
        const firstEntry = this.cache.get(firstKey)!
        clearTimeout(firstEntry.timer)
        this.cache.delete(firstKey)
      }
      
      // Set new entry with TTL
      const timer = setTimeout(() => {
        this.cache.delete(key)
      }, this.ttl)
      
      this.cache.set(key, { value, timer })
    })
}
```

## Scope Composition Patterns

### Nested Resource Management

```typescript
// Complex resource hierarchy
export const runComplexOperation = Effect.scoped(
  Effect.gen(function* () {
    // Level 1: Database connection
    const db = yield* acquireDatabaseConnection()
    
    // Level 2: Transaction (nested scope)
    const result = yield* Effect.scoped(
      Effect.gen(function* () {
        const tx = yield* beginTransaction(db)
        
        // Level 3: Temporary files (nested scope)
        const tempFiles = yield* Effect.scoped(
          Effect.gen(function* () {
            const file1 = yield* createTempFile()
            const file2 = yield* createTempFile()
            
            // Process files...
            yield* processFiles([file1, file2])
            
            // Files are cleaned up here
            return computeResult()
          })
        )
        
        // Commit transaction
        yield* commitTransaction(tx)
        
        // Transaction is closed here
        return tempFiles
      })
    )
    
    // Connection is closed here
    return result
  })
)
```

## Key Patterns for OpenAgents

1. **Always Use AcquireRelease**: For any resource that needs cleanup
2. **Leverage Scopes**: Group related resources together
3. **Graceful Shutdown**: Try soft shutdown before force killing
4. **Monitor Health**: Fork fibers to monitor long-running resources
5. **Handle Cleanup Errors**: Don't let cleanup failures crash the app
6. **Use Finalizers**: For cleanup that must run regardless of success/failure
7. **Memory Limits**: Set bounds on queues and caches
8. **Timeout Cleanup**: Don't wait forever for resources to release

## Implementation Checklist

- [ ] Wrap all event listeners with acquireRelease
- [ ] Manage process lifecycles with proper shutdown sequences
- [ ] Implement file handle management for large files
- [ ] Add memory bounds to all queues and caches
- [ ] Use scoped for operations with multiple resources
- [ ] Add health monitoring for long-running processes
- [ ] Implement graceful degradation when cleanup fails
- [ ] Test resource cleanup in error scenarios