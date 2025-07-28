# Land Service Layer Architecture

This document details Land's Effect-based service layer architecture, showing how VS Code's workbench services were reimplemented using Effect-TS patterns.

Based on analysis of the CodeEditorLand repositories, Wind provides a "highly reliable, composable, and maintainable" service architecture that serves as the core application logic layer. The project emphasizes declarative dependency management with a master "AppLayer" for composing services.

## Core Service Pattern

### Wind's Service Usage Example

From the Wind repository documentation, here's how services are used with Effect generators:

```typescript
const openFileEffect = Effect.gen(function* (_) {
  const dialogService = yield* _(DialogServiceTag);
  
  const uris = yield* _(
    Effect.tryPromise(() => 
      dialogService.showOpenDialog({
        canSelectFiles: true,
        title: "Open a File"
      })
    )
  );
});
```

This pattern emphasizes:
- Generator functions for effect composition
- Context.Tag for service registration
- Explicit error handling with `Effect.tryPromise()`
- Clean abstraction over asynchronous operations

### Service Definition

```typescript
// Base pattern for all Land services
export class EditorService extends Effect.Service<EditorService>()(
  'EditorService',
  {
    sync: () => ({
      // Service methods return Effects
      openEditor: (input: IEditorInput) => 
        Effect.Effect<IEditor, EditorError>,
      
      closeEditor: (editor: IEditor) => 
        Effect.Effect<void, never>,
      
      getActiveEditor: () => 
        Effect.Effect<IEditor | null, never>
    }),
    
    // Dependencies are declared explicitly
    dependencies: [FileService, TextEditorService, EditorGroupsService]
  }
) {}
```

## Service Composition Pattern

### Layer-Based Composition

```typescript
// Wind/Source/Application/Editor/Live.ts
export const EditorServiceLive = Layer.effect(
  EditorService,
  Effect.gen(function* () {
    // Inject dependencies
    const fileService = yield* FileService
    const textEditorService = yield* TextEditorService
    const editorGroupsService = yield* EditorGroupsService
    const logger = yield* Logger
    
    // Internal state
    const activeEditorRef = yield* Ref.make<IEditor | null>(null)
    const openEditorsRef = yield* Ref.make<Map<string, IEditor>>(new Map())
    
    return EditorService.of({
      openEditor: (input) =>
        Effect.gen(function* () {
          yield* logger.debug(`Opening editor for: ${input.resource}`)
          
          // Resolve input to concrete editor
          const resolved = yield* textEditorService.resolve(input)
          
          // Find or create editor group
          const group = yield* editorGroupsService.findGroup(resolved.options)
          
          // Load file content if needed
          if (resolved.resource) {
            const content = yield* fileService.readFile(resolved.resource)
            yield* resolved.setContent(content)
          }
          
          // Open in group
          const editor = yield* group.openEditor(resolved)
          
          // Update internal state
          yield* Ref.update(openEditorsRef, map => 
            new Map(map).set(editor.id, editor)
          )
          yield* Ref.set(activeEditorRef, editor)
          
          return editor
        }).pipe(
          Effect.catchTag('FileNotFoundError', (error) =>
            Effect.gen(function* () {
              // Create new file
              yield* logger.info(`Creating new file: ${error.path}`)
              return yield* createNewFileEditor(error.path)
            })
          )
        ),
      
      closeEditor: (editor) =>
        Effect.gen(function* () {
          // Check for unsaved changes
          if (editor.isDirty) {
            const saved = yield* promptToSave(editor)
            if (!saved) {
              return yield* Effect.fail(new EditorCloseAbortedError())
            }
          }
          
          // Close in group
          yield* editorGroupsService.closeEditor(editor)
          
          // Update state
          yield* Ref.update(openEditorsRef, map => {
            const newMap = new Map(map)
            newMap.delete(editor.id)
            return newMap
          })
          
          // Update active editor
          const isActive = yield* Ref.get(activeEditorRef).pipe(
            Effect.map(active => active?.id === editor.id)
          )
          
          if (isActive) {
            const remaining = yield* Ref.get(openEditorsRef)
            const next = remaining.values().next().value || null
            yield* Ref.set(activeEditorRef, next)
          }
        }),
      
      getActiveEditor: () => Ref.get(activeEditorRef)
    })
  })
)
```

## Workbench Services Reimplementation

### File Service Pattern

```typescript
// Wind/Source/Application/File/Live.ts
export const FileServiceLive = Layer.effect(
  FileService,
  Effect.gen(function* () {
    const providers = yield* Ref.make<Map<string, FileSystemProvider>>(new Map())
    const fileSystemService = yield* FileSystemService
    
    // Register default providers
    yield* registerProvider(new TauriDiskFileSystemProvider())
    yield* registerProvider(new MemoryFileSystemProvider())
    
    return FileService.of({
      readFile: (uri) =>
        Effect.gen(function* () {
          const provider = yield* getProvider(uri.scheme)
          const content = yield* provider.readFile(uri)
          
          // Fire events
          yield* fireFileEvent('read', uri)
          
          return content
        }).pipe(
          Effect.withSpan('FileService.readFile', {
            attributes: { uri: uri.toString() }
          })
        ),
      
      writeFile: (uri, content, options) =>
        Effect.gen(function* () {
          const provider = yield* getProvider(uri.scheme)
          
          // Check if file exists for create vs update
          const exists = yield* provider.exists(uri).pipe(
            Effect.orElseSucceed(() => false)
          )
          
          // Write through provider
          yield* provider.writeFile(uri, content, options)
          
          // Fire appropriate event
          yield* fireFileEvent(exists ? 'update' : 'create', uri)
        }),
      
      watch: (uri, options) =>
        Effect.gen(function* () {
          const provider = yield* getProvider(uri.scheme)
          
          if (!provider.watch) {
            return yield* Effect.fail(new FileSystemError({
              operation: 'watch',
              message: 'Provider does not support watching'
            }))
          }
          
          const watcher = yield* provider.watch(uri, options)
          
          // Convert to Effect stream
          return Stream.async<FileChangeEvent>((emit) => {
            watcher.onDidChange(event => {
              emit(Effect.succeed(event))
            })
            
            return Effect.sync(() => watcher.dispose())
          })
        })
    })
  })
)
```

### Configuration Service Pattern

```typescript
// Wind/Source/Application/Configuration/Live.ts
export const ConfigurationServiceLive = Layer.effect(
  ConfigurationService,
  Effect.gen(function* () {
    const fileService = yield* FileService
    const userDataPath = yield* UserDataPath
    
    // Configuration sources in priority order
    const defaultConfig = yield* Ref.make(defaultConfiguration)
    const userConfig = yield* Ref.make<Record<string, any>>({})
    const workspaceConfig = yield* Ref.make<Record<string, any>>({})
    
    // Load configurations
    yield* loadUserConfiguration()
    yield* loadWorkspaceConfiguration()
    
    // Watch for changes
    yield* watchConfigurationFiles()
    
    return ConfigurationService.of({
      getValue: <T>(key: string, defaultValue?: T) =>
        Effect.gen(function* () {
          // Check workspace config first
          const workspaceValue = yield* getValueFromConfig(workspaceConfig, key)
          if (workspaceValue !== undefined) return workspaceValue as T
          
          // Then user config
          const userValue = yield* getValueFromConfig(userConfig, key)
          if (userValue !== undefined) return userValue as T
          
          // Then default config
          const defaultConfigValue = yield* getValueFromConfig(defaultConfig, key)
          if (defaultConfigValue !== undefined) return defaultConfigValue as T
          
          // Finally, provided default
          if (defaultValue !== undefined) return defaultValue
          
          return yield* Effect.fail(new ConfigurationKeyNotFoundError({ key }))
        }),
      
      updateValue: (key: string, value: any, target: ConfigurationTarget) =>
        Effect.gen(function* () {
          const configRef = target === ConfigurationTarget.User 
            ? userConfig 
            : workspaceConfig
          
          // Update in memory
          yield* Ref.update(configRef, config => ({
            ...config,
            [key]: value
          }))
          
          // Persist to disk
          yield* persistConfiguration(target)
          
          // Fire change event
          yield* fireConfigurationChangeEvent([key])
        }),
      
      inspect: (key: string) =>
        Effect.gen(function* () {
          const defaultValue = yield* getValueFromConfig(defaultConfig, key)
          const userValue = yield* getValueFromConfig(userConfig, key)
          const workspaceValue = yield* getValueFromConfig(workspaceConfig, key)
          
          return {
            key,
            defaultValue,
            userValue,
            workspaceValue,
            value: workspaceValue ?? userValue ?? defaultValue
          }
        })
    })
  })
)
```

## Dependency Injection Pattern

### Wind's Runtime Pattern

From the Wind repository, the application runtime is created through Layer composition:

```typescript
// Wind's actual runtime pattern
const AppRuntime = Layer.toRuntime(AppLayer).pipe(
  Effect.scoped,
  Effect.runSync
);
```

### Application Layer Composition

```typescript
// Wind/Source/Application/Layer.ts
export const ApplicationLayer = Layer.mergeAll(
  // Core services
  FileSystemServiceLive,
  FileServiceLive,
  ConfigurationServiceLive,
  LoggerServiceLive,
  
  // Editor services
  TextEditorServiceLive,
  EditorGroupsServiceLive,
  EditorServiceLive,
  
  // Extension services
  ExtensionServiceLive,
  LanguageFeaturesServiceLive,
  
  // UI services
  NotificationServiceLive,
  DialogServiceLive,
  StatusBarServiceLive
)

// Bootstrap application
export const runApplication = (program: Effect.Effect<void, never, ApplicationServices>) =>
  program.pipe(
    Effect.provide(ApplicationLayer),
    Effect.runPromise
  )
```

### Service Testing with Layers

```typescript
// Test/Editor/EditorService.test.ts
const TestFileService = Layer.succeed(
  FileService,
  FileService.of({
    readFile: (uri) => 
      uri.path.includes('test') 
        ? Effect.succeed(new TextEncoder().encode('test content'))
        : Effect.fail(new FileNotFoundError({ path: uri.path })),
    
    writeFile: () => Effect.unit,
    watch: () => Effect.fail(new NotImplementedError())
  })
)

const TestLayer = Layer.mergeAll(
  TestFileService,
  TextEditorServiceLive,
  EditorGroupsServiceLive,
  EditorServiceLive
)

test('EditorService opens files', async () => {
  const program = Effect.gen(function* () {
    const editorService = yield* EditorService
    
    const editor = yield* editorService.openEditor({
      resource: URI.parse('file:///test.ts')
    })
    
    expect(editor).toBeDefined()
    expect(editor.getModel()?.getValue()).toBe('test content')
  })
  
  await program.pipe(
    Effect.provide(TestLayer),
    Effect.runPromise
  )
})
```

## Service Lifecycle Management

### Initialization Pattern

```typescript
// Wind/Source/Application/Lifecycle.ts
export class LifecycleService extends Effect.Service<LifecycleService>()(
  'LifecycleService',
  {
    sync: () => ({
      initialize: () => 
        Effect.gen(function* () {
          // Phase 1: Core services
          yield* Effect.logInfo('Initializing core services...')
          yield* initializeCoreServices()
          
          // Phase 2: Extension host
          yield* Effect.logInfo('Starting extension host...')
          yield* startExtensionHost()
          
          // Phase 3: Workspace
          yield* Effect.logInfo('Loading workspace...')
          yield* loadWorkspace()
          
          // Phase 4: Extensions
          yield* Effect.logInfo('Activating extensions...')
          yield* activateExtensions()
          
          yield* Effect.logInfo('Application initialized successfully')
        }).pipe(
          Effect.catchAll(error =>
            Effect.gen(function* () {
              yield* Effect.logError('Initialization failed', error)
              yield* emergencyShutdown()
              return yield* Effect.fail(error)
            })
          )
        ),
      
      shutdown: () =>
        Effect.gen(function* () {
          // Graceful shutdown sequence
          yield* Effect.logInfo('Beginning shutdown...')
          
          // Save all dirty files
          yield* saveAllDirtyFiles()
          
          // Deactivate extensions
          yield* deactivateExtensions()
          
          // Stop extension host
          yield* stopExtensionHost()
          
          // Persist state
          yield* persistApplicationState()
          
          yield* Effect.logInfo('Shutdown complete')
        }).pipe(
          Effect.timeout('30 seconds'),
          Effect.catchAll(() => 
            Effect.logError('Graceful shutdown failed, forcing exit')
          )
        )
    })
  }
) {}
```

## Key Patterns for OpenAgents

1. **Service-First Architecture**: Every feature is a service
2. **Explicit Dependencies**: Services declare what they need
3. **Layer Composition**: Build complex apps from simple services
4. **Testability**: Easy to mock services with test layers
5. **Lifecycle Management**: Proper initialization and shutdown
6. **State Encapsulation**: Services manage their own state
7. **Event-Driven**: Services communicate through events
8. **Error Recovery**: Each service handles its error cases

## Implementation Checklist

- [ ] Define service interfaces with Effect.Service
- [ ] Implement live layers with proper dependency injection
- [ ] Create test layers for unit testing
- [ ] Compose application layer from service layers
- [ ] Add lifecycle management for initialization/shutdown
- [ ] Implement proper error handling with tagged errors
- [ ] Add observability with spans and logs
- [ ] Document service contracts and dependencies