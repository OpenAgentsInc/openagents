# Land IPC Patterns: Type-Safe Cross-Boundary Communication

This document details the IPC (Inter-Process Communication) patterns used in Land for type-safe communication between Rust (Mountain) and TypeScript (Wind/Cocoon).

## Architecture Overview

Land uses multiple IPC mechanisms:
1. **Tauri Commands**: Wind ↔ Mountain communication
2. **gRPC**: Cocoon ↔ Mountain communication
3. **Effect Wrappers**: Type-safe abstractions over both

## 1. Tauri Command Wrapping Pattern

### Basic Effect Wrapper

```typescript
// Wind/Integration/Tauri/Wrap/ReadFile.ts
import { Effect } from 'effect'
import { invoke } from '@tauri-apps/api/core'

// Tagged error for file operations
export class FileSystemError extends Data.TaggedError('FileSystemError')<{
  operation: 'read' | 'write' | 'delete' | 'stat'
  path: string
  cause?: unknown
}> {}

// Effect wrapper for Tauri command
export const ReadFile = (path: string) =>
  Effect.tryPromise({
    try: () => invoke<Uint8Array>('plugin:fs|read_file', { path }),
    catch: (error) => new FileSystemError({ 
      operation: 'read', 
      path, 
      cause: error 
    })
  })
```

### Service Layer Abstraction

```typescript
// Wind/Source/Application/FileSystem/Service.ts
export class FileSystemService extends Effect.Service<FileSystemService>()(
  'FileSystemService',
  {
    sync: () => ({
      readFile: (path: string) => 
        ReadFile(path).pipe(
          Effect.tap(() => Effect.logDebug(`Read file: ${path}`)),
          Effect.withSpan('FileSystemService.readFile')
        ),
      
      writeFile: (path: string, content: Uint8Array) =>
        WriteFile(path, content).pipe(
          Effect.tap(() => Effect.logDebug(`Wrote file: ${path}`)),
          Effect.withSpan('FileSystemService.writeFile')
        ),
      
      deleteFile: (path: string) =>
        DeleteFile(path).pipe(
          Effect.tap(() => Effect.logDebug(`Deleted file: ${path}`)),
          Effect.withSpan('FileSystemService.deleteFile')
        )
    })
  }
) {}
```

### Provider Pattern for URI Schemes

```typescript
// Wind/Source/Application/FileSystem/Provider.ts
export interface FileSystemProvider {
  readonly scheme: string
  readonly capabilities: FileSystemProviderCapabilities
  readonly readFile: (uri: URI) => Effect.Effect<Uint8Array, FileSystemError>
  readonly writeFile: (uri: URI, content: Uint8Array) => Effect.Effect<void, FileSystemError>
  readonly delete: (uri: URI) => Effect.Effect<void, FileSystemError>
  readonly stat: (uri: URI) => Effect.Effect<FileStat, FileSystemError>
}

export class TauriDiskFileSystemProvider implements FileSystemProvider {
  readonly scheme = 'file'
  readonly capabilities = FileSystemProviderCapabilities.FileReadWrite
  
  readFile(uri: URI) {
    return Effect.gen(function* () {
      const path = uri.fsPath
      const content = yield* ReadFile(path)
      return content
    }).pipe(
      Effect.annotateSpan('provider', 'TauriDisk'),
      Effect.annotateSpan('uri', uri.toString())
    )
  }
  
  // ... other methods
}
```

## 2. gRPC Pattern for Extension Host

### Protocol Buffer Definition

```protobuf
// Vine/protocol/language_features.proto
syntax = "proto3";

service LanguageFeatures {
  rpc RegisterHoverProvider(RegisterProviderRequest) returns (RegisterProviderResponse);
  rpc ProvideHover(ProvideHoverRequest) returns (ProvideHoverResponse);
}

message RegisterProviderRequest {
  string language_id = 1;
  string provider_id = 2;
}

message ProvideHoverRequest {
  string document_uri = 1;
  Position position = 2;
  string provider_id = 3;
}

message ProvideHoverResponse {
  optional Hover hover = 1;
}
```

### Effect-Wrapped gRPC Client

```typescript
// Cocoon/Source/Integration/Vine/Client.ts
export class VineGRPCClient extends Effect.Service<VineGRPCClient>()(
  'VineGRPCClient',
  {
    sync: () => ({
      registerHoverProvider: (request: RegisterProviderRequest) =>
        Effect.tryPromise({
          try: () => grpcClient.registerHoverProvider(request),
          catch: (error) => new ExtensionHostError({
            operation: 'registerHoverProvider',
            request,
            cause: error
          })
        }).pipe(
          Effect.retry(retryPolicy),
          Effect.timeout('5 seconds')
        ),
      
      provideHover: (request: ProvideHoverRequest) =>
        Effect.tryPromise({
          try: () => grpcClient.provideHover(request),
          catch: (error) => new ExtensionHostError({
            operation: 'provideHover',
            request,
            cause: error
          })
        }).pipe(
          Effect.timeout('2 seconds')
        )
    })
  }
) {}
```

## 3. Bidirectional Communication Pattern

### Event Streaming from Backend

```typescript
// Wind/Integration/Tauri/Events.ts
export const createEventStream = <T>(eventName: string) =>
  Stream.async<T>((emit) => {
    const setupListener = Effect.gen(function* () {
      const unlisten = yield* Effect.promise(() =>
        listen<T>(eventName, (event) => {
          emit(Effect.succeed(event.payload))
        })
      )
      return unlisten
    })
    
    return setupListener.pipe(
      Effect.map(unlisten => 
        Effect.sync(() => {
          unlisten()
        })
      )
    )
  })

// Usage for terminal output
export const terminalOutputStream = (terminalId: string) =>
  createEventStream<TerminalOutput>(`terminal:${terminalId}:output`).pipe(
    Stream.tap(output => 
      Effect.logDebug(`Terminal ${terminalId} output: ${output.data.length} bytes`)
    )
  )
```

### Command Routing Pattern

```typescript
// Mountain/Track Dispatcher equivalent in TypeScript
export class CommandDispatcher extends Effect.Service<CommandDispatcher>()(
  'CommandDispatcher',
  {
    sync: () => ({
      dispatch: (command: string, args: unknown) =>
        Effect.gen(function* () {
          const registry = yield* CommandRegistry
          const handler = yield* registry.getHandler(command)
          
          if (!handler) {
            return yield* Effect.fail(new CommandNotFoundError({ command }))
          }
          
          // Route to appropriate handler
          if (handler.location === 'backend') {
            return yield* invokeBackendCommand(command, args)
          } else {
            return yield* invokeExtensionCommand(command, args)
          }
        })
    })
  }
) {}
```

## 4. Type Generation Pattern

### Rust to TypeScript Types

```rust
// Mountain/src/dto/file_system.rs
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub modified: i64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReadFileRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReadFileResponse {
    pub content: Vec<u8>,
    pub info: FileInfo,
}
```

### Generated TypeScript Types

```typescript
// Generated by ts-rs
export interface FileInfo {
  path: string
  size: number
  modified: number
  is_directory: boolean
}

export interface ReadFileRequest {
  path: string
}

export interface ReadFileResponse {
  content: Uint8Array
  info: FileInfo
}
```

## 5. Error Propagation Pattern

### Cross-Boundary Error Handling

```typescript
// Wind/Integration/Tauri/Errors.ts
export const parseTauriError = (error: unknown): Effect.Effect<never, TauriError> => {
  if (typeof error === 'string') {
    // Parse Rust error messages
    if (error.includes('Permission denied')) {
      return Effect.fail(new PermissionError({ message: error }))
    }
    if (error.includes('File not found')) {
      return Effect.fail(new FileNotFoundError({ message: error }))
    }
  }
  
  return Effect.fail(new TauriError({ 
    message: 'Unknown Tauri error',
    cause: error 
  }))
}

// Usage in command wrapper
export const SafeReadFile = (path: string) =>
  ReadFile(path).pipe(
    Effect.catchAll(error => parseTauriError(error))
  )
```

## 6. Performance Optimization Patterns

### Batched Commands

```typescript
// Wind/Integration/Tauri/Batch.ts
export class BatchedFileReader extends Effect.Service<BatchedFileReader>()(
  'BatchedFileReader',
  {
    sync: () => {
      const queue = Queue.unbounded<{ path: string; deferred: Deferred<Uint8Array, FileSystemError> }>()
      
      // Process queue in batches
      const processor = Stream.fromQueue(queue).pipe(
        Stream.groupedWithin(50, '100 millis'),
        Stream.mapEffect(batch =>
          Effect.gen(function* () {
            const paths = batch.map(item => item.path)
            const results = yield* BatchReadFiles(paths)
            
            // Resolve deferreds
            yield* Effect.forEach(batch, (item, index) =>
              Deferred.complete(item.deferred, results[index])
            )
          })
        ),
        Stream.runDrain
      ).pipe(Effect.forkDaemon)
      
      return {
        readFile: (path: string) =>
          Effect.gen(function* () {
            const deferred = yield* Deferred.make<Uint8Array, FileSystemError>()
            yield* Queue.offer(queue, { path, deferred })
            return yield* Deferred.await(deferred)
          })
      }
    }
  }
) {}
```

## Implementation Guidelines for OpenAgents

1. **Wrap All Tauri Commands**: Every invoke should be wrapped in Effect.tryPromise
2. **Use Tagged Errors**: Define domain-specific error types for each operation
3. **Service Layer**: Abstract IPC details behind Effect services
4. **Type Generation**: Use ts-rs or similar for Rust→TypeScript types
5. **Retry Logic**: Add appropriate retry strategies for transient failures
6. **Batching**: Consider batching for high-frequency operations
7. **Event Streams**: Use Effect streams for real-time data
8. **Performance Monitoring**: Add spans and metrics to all IPC calls

## Additional Insights from CodeEditorLand GitHub Repositories

### Wind's Context Tag Pattern
From the Wind repository, services use Context.Tag for registration:

```typescript
// Effect-based command registration pattern
export const registerCommands = Effect.gen(function* () {
  const commandRegistry = yield* CommandRegistry
  
  // Register all commands in one declarative block
  yield* Effect.all([
    commandRegistry.register('file.open', openFileCommand),
    commandRegistry.register('file.save', saveFileCommand),
    commandRegistry.register('session.create', createSessionCommand),
    commandRegistry.register('agent.start', startAgentCommand)
  ], { concurrency: 'unbounded' })
})
```

### Cocoon's Bidirectional gRPC Pattern
From Cocoon's architecture, the extension host implements both client and server:

```typescript
// Cocoon gRPC service that both calls and receives calls from Mountain
export const IPCService = Layer.effect(
  IPCServiceTag,
  Effect.gen(function* () {
    // Client for calling Mountain
    const client = yield* createGRPCClient(MOUNTAIN_URL)
    
    // Server for receiving calls from Mountain
    const server = yield* createGRPCServer(COCOON_PORT)
    
    // Bidirectional communication handling
    yield* server.handle('executeCommand', (request) =>
      Effect.gen(function* () {
        const extension = yield* findExtension(request.extensionId)
        return yield* extension.executeCommand(request.command, request.args)
      })
    )
    
    return {
      callMountain: (method: string, args: any) => 
        client.request(method, args),
      
      onMountainCall: (method: string, handler: Handler) =>
        server.handle(method, handler)
    }
  })
)
```

### Wind's Dialog Service Pattern
From Wind's architecture, services use context tags for injection:

```typescript
// Context tag pattern for service injection
export const DialogServiceTag = Context.Tag<DialogService>('DialogService')

// Service implementation with Effect
export const showOpenDialog = Effect.gen(function* () {
  const dialogService = yield* DialogServiceTag
  
  const result = yield* Effect.tryPromise(() =>
    dialogService.showOpenDialog({
      canSelectFiles: true,
      canSelectMany: true,
      filters: [
        { name: 'TypeScript', extensions: ['ts', 'tsx'] },
        { name: 'All Files', extensions: ['*'] }
      ]
    })
  ).pipe(
    Effect.catchTag('CancelledError', () => Effect.succeed([]))
  )
  
  return result
})
```

## Key Takeaways

- Land achieves complete type safety across the Rust-TypeScript boundary
- Effect wrappers provide consistent error handling and retry logic
- Service abstraction hides IPC complexity from business logic
- gRPC enables structured bidirectional communication
- Event streams replace polling for real-time updates
- Declarative patterns reduce boilerplate and improve maintainability
- Context tags enable clean dependency injection