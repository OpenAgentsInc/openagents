import { Effect, Scope } from "effect"

/**
 * Resource management utilities following Land patterns
 * These utilities ensure proper cleanup of resources using Effect's Scope API
 */

// Generic resource creation with automatic cleanup
export const createResource = <T>(
  acquire: Effect.Effect<T>,
  release: (resource: T) => Effect.Effect<void>
) =>
  Effect.acquireRelease(acquire, release)

// Event listener management
export const addEventListener = <K extends keyof WindowEventMap>(
  target: Window | HTMLElement,
  event: K,
  handler: (event: WindowEventMap[K]) => void,
  options?: AddEventListenerOptions
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      target.addEventListener(event, handler as EventListener, options)
      return { target, event, handler: handler as EventListener, options }
    }),
    ({ target, event, handler, options }) =>
      Effect.sync(() => {
        target.removeEventListener(event, handler, options)
      })
  )

// Document event listener helper
export const addDocumentEventListener = <K extends keyof DocumentEventMap>(
  event: K,
  handler: (event: DocumentEventMap[K]) => void,
  options?: AddEventListenerOptions
) =>
  document.addEventListener(event, handler, options)

// Window event listener helper
export const addWindowEventListener = <K extends keyof WindowEventMap>(
  event: K,
  handler: (event: WindowEventMap[K]) => void,
  options?: AddEventListenerOptions
) =>
  addEventListener(window, event, handler, options)

// Timer management
export const setTimer = (
  callback: () => void,
  delay: number,
  type: "timeout" | "interval" = "timeout"
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const timerId = type === "timeout"
        ? setTimeout(callback, delay)
        : setInterval(callback, delay)
      return { timerId, type }
    }),
    ({ timerId, type }) =>
      Effect.sync(() => {
        if (type === "timeout") {
          clearTimeout(timerId)
        } else {
          clearInterval(timerId)
        }
      })
  )

// Timeout helper
export const setTimeoutEffect = (callback: () => void, delay: number) =>
  setTimer(callback, delay, "timeout")

// Interval helper
export const setIntervalEffect = (callback: () => void, delay: number) =>
  setTimer(callback, delay, "interval")

// ResizeObserver management
export const observeResize = (
  element: Element,
  callback: ResizeObserverCallback,
  options?: ResizeObserverOptions
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const observer = new ResizeObserver(callback)
      observer.observe(element, options)
      return observer
    }),
    (observer) =>
      Effect.sync(() => {
        observer.disconnect()
      })
  )

// IntersectionObserver management
export const observeIntersection = (
  elements: Element[],
  callback: IntersectionObserverCallback,
  options?: IntersectionObserverInit
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const observer = new IntersectionObserver(callback, options)
      elements.forEach((el) => observer.observe(el))
      return observer
    }),
    (observer) =>
      Effect.sync(() => {
        observer.disconnect()
      })
  )

// MutationObserver management
export const observeMutation = (
  target: Node,
  callback: MutationCallback,
  options: MutationObserverInit
) =>
  Effect.acquireRelease(
    Effect.sync(() => {
      const observer = new MutationObserver(callback)
      observer.observe(target, options)
      return observer
    }),
    (observer) =>
      Effect.sync(() => {
        observer.disconnect()
      })
  )

// WebSocket management
export const createWebSocket = (
  url: string,
  protocols?: string | string[]
) =>
  Effect.acquireRelease(
    Effect.sync(() => new WebSocket(url, protocols)),
    (ws) =>
      Effect.sync(() => {
        if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
          ws.close()
        }
      })
  )

// AbortController for fetch requests
export const createAbortController = () =>
  Effect.acquireRelease(
    Effect.sync(() => new AbortController()),
    (controller) =>
      Effect.sync(() => {
        controller.abort()
      })
  )

// MediaStream management (for webcam, microphone, etc.)
export const getMediaStream = (constraints: MediaStreamConstraints) =>
  Effect.acquireRelease(
    Effect.tryPromise({
      try: () => navigator.mediaDevices.getUserMedia(constraints),
      catch: (error) => new Error(`Failed to get media stream: ${error}`)
    }),
    (stream) =>
      Effect.sync(() => {
        stream.getTracks().forEach((track) => track.stop())
      })
  )

// File picker options interface
interface FilePickerOptions {
  types?: Array<{
    description?: string
    accept: Record<string, string[]>
  }>
  excludeAcceptAllOption?: boolean
  multiple?: boolean
}

// File System Access API type definitions
interface FileSystemFileHandle {
  readonly kind: 'file'
  readonly name: string
  getFile(): Promise<File>
  createWritable(): Promise<FileSystemWritableFileStream>
}

interface FileSystemWritableFileStream extends WritableStream {
  write(data: FileSystemWriteChunkType): Promise<void>
  seek(position: number): Promise<void>
  truncate(size: number): Promise<void>
}

type FileSystemWriteChunkType = BufferSource | Blob | string

// Extend window interface for File System Access API
declare global {
  interface Window {
    showOpenFilePicker?(options?: FilePickerOptions): Promise<FileSystemFileHandle[]>
  }
}

// File handle management
export const openFileHandle = (
  options?: FilePickerOptions
) =>
  Effect.acquireRelease(
    Effect.tryPromise({
      try: async () => {
        const [handle] = await window.showOpenFilePicker!(options)
        return handle
      },
      catch: (error) => new Error(`Failed to open file: ${error}`)
    }),
    // File handles don't need explicit cleanup
    () => Effect.void
  )

// IndexedDB connection management
export const openIndexedDB = (
  name: string,
  version?: number,
  onupgradeneeded?: (event: IDBVersionChangeEvent) => void
) =>
  Effect.acquireRelease(
    Effect.async<IDBDatabase, Error>((resume) => {
      const request = indexedDB.open(name, version)
      
      request.onsuccess = () => resume(Effect.succeed(request.result))
      request.onerror = () => resume(Effect.fail(new Error("Failed to open IndexedDB")))
      
      if (onupgradeneeded) {
        request.onupgradeneeded = onupgradeneeded
      }
    }),
    (db) =>
      Effect.sync(() => {
        db.close()
      })
  )

// Worker management
export const createWorker = (scriptURL: string | URL, options?: WorkerOptions) =>
  Effect.acquireRelease(
    Effect.sync(() => new Worker(scriptURL, options)),
    (worker) =>
      Effect.sync(() => {
        worker.terminate()
      })
  )

// Utility to run multiple resources in a scope
export const withResources = <R, E, A>(
  effect: Effect.Effect<A, E, R | Scope.Scope>
) =>
  Effect.scoped(effect)

// Helper to manage multiple event listeners
export const addEventListeners = <T extends Record<string, EventListener>>(
  target: EventTarget,
  listeners: T,
  options?: AddEventListenerOptions
) =>
  Effect.all(
    Object.entries(listeners).map(([event, handler]) =>
      Effect.acquireRelease(
        Effect.sync(() => {
          target.addEventListener(event, handler, options)
          return { event, handler }
        }),
        ({ event, handler }) =>
          Effect.sync(() => {
            target.removeEventListener(event, handler, options)
          })
      )
    )
  )

// Resource pool for reusable resources
export const createResourcePool = <T>(
  create: () => Effect.Effect<T>,
  destroy: (resource: T) => Effect.Effect<void>,
  maxSize = 10
) => {
  const pool: T[] = []
  const inUse = new Set<T>()
  
  const acquire = (): Effect.Effect<T> =>
    Effect.gen(function* () {
      // Try to get from pool
      const resource = pool.pop()
      if (resource) {
        inUse.add(resource)
        return resource
      }
      
      // Create new if pool is empty and under limit
      if (inUse.size < maxSize) {
        const newResource = yield* create()
        inUse.add(newResource)
        return newResource
      }
      
      // Wait and retry if at capacity
      yield* Effect.sleep("100 millis")
      return yield* acquire()
    })
  
  const release = (resource: T): Effect.Effect<void> =>
    Effect.sync(() => {
      inUse.delete(resource)
      pool.push(resource)
    })
  
  const destroyAll = () =>
    Effect.all([
      ...Array.from(inUse).map(destroy),
      ...pool.map(destroy)
    ])
  
  return { acquire, release, destroyAll }
}