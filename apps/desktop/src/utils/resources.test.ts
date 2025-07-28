import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { Effect, Scope, Fiber, Exit } from 'effect'
import {
  createResource,
  addEventListener,
  addDocumentEventListener,
  addWindowEventListener,
  setTimer,
  setTimeoutEffect,
  setIntervalEffect,
  observeResize,
  observeIntersection,
  observeMutation,
  createWebSocket,
  createAbortController,
  getMediaStream,
  openFileHandle,
  openIndexedDB,
  createWorker,
  withResources,
  addEventListeners,
  createResourcePool
} from './resources'
import { expectEffect, expectEffectError } from '@/test/effect-test-utils'

// Mock DOM APIs
global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn()
}))

global.IntersectionObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn()
}))

global.MutationObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  disconnect: vi.fn()
}))

global.WebSocket = vi.fn().mockImplementation((url) => ({
  url,
  readyState: WebSocket.CONNECTING,
  close: vi.fn(),
  send: vi.fn(),
  addEventListener: vi.fn(),
  removeEventListener: vi.fn()
}))

global.AbortController = vi.fn().mockImplementation(() => ({
  signal: { aborted: false },
  abort: vi.fn()
}))

global.Worker = vi.fn().mockImplementation((scriptURL) => ({
  scriptURL,
  terminate: vi.fn(),
  postMessage: vi.fn(),
  addEventListener: vi.fn(),
  removeEventListener: vi.fn()
}))

// Mock navigator.mediaDevices
Object.defineProperty(navigator, 'mediaDevices', {
  value: {
    getUserMedia: vi.fn()
  },
  configurable: true
})

// Mock indexedDB
global.indexedDB = {
  open: vi.fn()
} as any

// Mock window.showOpenFilePicker
;(window as any).showOpenFilePicker = vi.fn()

describe('Resource Management Utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('createResource', () => {
    it('should acquire and release resource', async () => {
      let resourceAcquired = false
      let resourceReleased = false

      const acquire = Effect.sync(() => {
        resourceAcquired = true
        return 'resource'
      })

      const release = () => Effect.sync(() => {
        resourceReleased = true
      })

      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const resource = yield* createResource(acquire, release)
            expect(resource).toBe('resource')
            expect(resourceAcquired).toBe(true)
            expect(resourceReleased).toBe(false)
          })
        ),
        () => {
          expect(resourceReleased).toBe(true)
        }
      )
    })

    it('should release resource on error', async () => {
      let released = false
      
      const acquire = Effect.succeed('resource')
      const release = () => Effect.sync(() => { released = true })
      
      await Effect.runPromiseExit(
        Effect.scoped(
          Effect.gen(function* () {
            yield* createResource(acquire, release)
            return yield* Effect.fail(new Error('Test error'))
          })
        )
      )
      
      expect(released).toBe(true)
    })
  })

  describe('addEventListener', () => {
    it('should add and remove event listener', async () => {
      const element = document.createElement('div')
      const handler = vi.fn()
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addEventListener(element, 'click', handler)
            
            // Simulate click
            element.click()
            expect(handler).toHaveBeenCalledTimes(1)
          })
        ),
        () => {
          // After scope, handler should be removed
          element.click()
          expect(handler).toHaveBeenCalledTimes(1) // Still 1, not 2
        }
      )
    })

    it('should support event listener options', async () => {
      const element = document.createElement('div')
      const handler = vi.fn()
      const addEventListenerSpy = vi.spyOn(element, 'addEventListener')
      const removeEventListenerSpy = vi.spyOn(element, 'removeEventListener')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addEventListener(element, 'click', handler, { capture: true, passive: true })
            
            expect(addEventListenerSpy).toHaveBeenCalledWith(
              'click',
              handler,
              { capture: true, passive: true }
            )
          })
        ),
        () => {
          expect(removeEventListenerSpy).toHaveBeenCalledWith(
            'click',
            handler,
            { capture: true, passive: true }
          )
        }
      )
    })
  })

  describe('addDocumentEventListener', () => {
    it('should add event listener to document', async () => {
      const handler = vi.fn()
      const addEventListenerSpy = vi.spyOn(document, 'addEventListener')
      const removeEventListenerSpy = vi.spyOn(document, 'removeEventListener')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addDocumentEventListener('keydown', handler)
            
            expect(addEventListenerSpy).toHaveBeenCalledWith('keydown', handler, undefined)
          })
        ),
        () => {
          expect(removeEventListenerSpy).toHaveBeenCalledWith('keydown', handler, undefined)
        }
      )
    })
  })

  describe('addWindowEventListener', () => {
    it('should add event listener to window', async () => {
      const handler = vi.fn()
      const addEventListenerSpy = vi.spyOn(window, 'addEventListener')
      const removeEventListenerSpy = vi.spyOn(window, 'removeEventListener')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addWindowEventListener('resize', handler)
            
            expect(addEventListenerSpy).toHaveBeenCalledWith('resize', handler, undefined)
          })
        ),
        () => {
          expect(removeEventListenerSpy).toHaveBeenCalledWith('resize', handler, undefined)
        }
      )
    })
  })

  describe('setTimer', () => {
    it('should set and clear timeout', async () => {
      const callback = vi.fn()
      const setTimeoutSpy = vi.spyOn(global, 'setTimeout')
      const clearTimeoutSpy = vi.spyOn(global, 'clearTimeout')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* setTimer(callback, 100, 'timeout')
            
            expect(setTimeoutSpy).toHaveBeenCalledWith(callback, 100)
          })
        ),
        () => {
          expect(clearTimeoutSpy).toHaveBeenCalled()
        }
      )
    })

    it('should set and clear interval', async () => {
      const callback = vi.fn()
      const setIntervalSpy = vi.spyOn(global, 'setInterval')
      const clearIntervalSpy = vi.spyOn(global, 'clearInterval')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* setTimer(callback, 100, 'interval')
            
            expect(setIntervalSpy).toHaveBeenCalledWith(callback, 100)
          })
        ),
        () => {
          expect(clearIntervalSpy).toHaveBeenCalled()
        }
      )
    })
  })

  describe('setTimeoutEffect', () => {
    it('should create timeout resource', async () => {
      const callback = vi.fn()
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* setTimeoutEffect(callback, 50)
          })
        ),
        () => {}
      )
    })
  })

  describe('setIntervalEffect', () => {
    it('should create interval resource', async () => {
      const callback = vi.fn()
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* setIntervalEffect(callback, 50)
          })
        ),
        () => {}
      )
    })
  })

  describe('observeResize', () => {
    it('should create and disconnect ResizeObserver', async () => {
      const element = document.createElement('div')
      const callback = vi.fn()
      let observer: any
      
      vi.mocked(ResizeObserver).mockImplementation(function(this: any, cb) {
        observer = {
          observe: vi.fn(),
          unobserve: vi.fn(),
          disconnect: vi.fn(),
          callback: cb
        }
        return observer
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* observeResize(element, callback)
            
            expect(observer.observe).toHaveBeenCalledWith(element, undefined)
          })
        ),
        () => {
          expect(observer.disconnect).toHaveBeenCalled()
        }
      )
    })

    it('should support observer options', async () => {
      const element = document.createElement('div')
      const callback = vi.fn()
      const options = { box: 'content-box' as const }
      let observer: any
      
      vi.mocked(ResizeObserver).mockImplementation(function(this: any) {
        observer = {
          observe: vi.fn(),
          disconnect: vi.fn()
        }
        return observer
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* observeResize(element, callback, options)
            
            expect(observer.observe).toHaveBeenCalledWith(element, options)
          })
        ),
        () => {}
      )
    })
  })

  describe('observeIntersection', () => {
    it('should observe multiple elements', async () => {
      const elements = [
        document.createElement('div'),
        document.createElement('div'),
        document.createElement('div')
      ]
      const callback = vi.fn()
      let observer: any
      
      vi.mocked(IntersectionObserver).mockImplementation(function(this: any, cb, opts) {
        observer = {
          observe: vi.fn(),
          unobserve: vi.fn(),
          disconnect: vi.fn(),
          callback: cb,
          options: opts
        }
        return observer
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* observeIntersection(elements, callback)
            
            expect(observer.observe).toHaveBeenCalledTimes(3)
            elements.forEach(el => {
              expect(observer.observe).toHaveBeenCalledWith(el)
            })
          })
        ),
        () => {
          expect(observer.disconnect).toHaveBeenCalled()
        }
      )
    })

    it('should support intersection options', async () => {
      const elements = [document.createElement('div')]
      const callback = vi.fn()
      const options = { threshold: [0, 0.5, 1], rootMargin: '10px' }
      
      vi.mocked(IntersectionObserver).mockImplementation(function(this: any, cb, opts) {
        expect(opts).toEqual(options)
        return {
          observe: vi.fn(),
          disconnect: vi.fn()
        }
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* observeIntersection(elements, callback, options)
          })
        ),
        () => {}
      )
    })
  })

  describe('observeMutation', () => {
    it('should observe DOM mutations', async () => {
      const target = document.createElement('div')
      const callback = vi.fn()
      const options: MutationObserverInit = { childList: true, subtree: true }
      let observer: any
      
      vi.mocked(MutationObserver).mockImplementation(function(this: any, cb) {
        observer = {
          observe: vi.fn(),
          disconnect: vi.fn(),
          callback: cb
        }
        return observer
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* observeMutation(target, callback, options)
            
            expect(observer.observe).toHaveBeenCalledWith(target, options)
          })
        ),
        () => {
          expect(observer.disconnect).toHaveBeenCalled()
        }
      )
    })
  })

  describe('createWebSocket', () => {
    it('should create and close WebSocket', async () => {
      const url = 'ws://localhost:8080'
      let ws: any
      
      vi.mocked(WebSocket).mockImplementation(function(this: any, u) {
        ws = {
          url: u,
          readyState: WebSocket.OPEN,
          close: vi.fn()
        }
        return ws
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const socket = yield* createWebSocket(url)
            expect(socket.url).toBe(url)
          })
        ),
        () => {
          expect(ws.close).toHaveBeenCalled()
        }
      )
    })

    it('should close connecting WebSocket', async () => {
      let ws: any
      
      vi.mocked(WebSocket).mockImplementation(function(this: any, url) {
        ws = {
          url,
          readyState: WebSocket.CONNECTING,
          close: vi.fn()
        }
        return ws
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* createWebSocket('ws://test')
          })
        ),
        () => {
          expect(ws.close).toHaveBeenCalled()
        }
      )
    })

    it('should not close already closed WebSocket', async () => {
      let ws: any
      
      vi.mocked(WebSocket).mockImplementation(function(this: any, url) {
        ws = {
          url,
          readyState: WebSocket.CLOSED,
          close: vi.fn()
        }
        return ws
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* createWebSocket('ws://test')
          })
        ),
        () => {
          expect(ws.close).not.toHaveBeenCalled()
        }
      )
    })

    it('should support protocols', async () => {
      const protocols = ['protocol1', 'protocol2']
      
      vi.mocked(WebSocket).mockImplementation(function(this: any, url, protos) {
        expect(protos).toEqual(protocols)
        return {
          readyState: WebSocket.OPEN,
          close: vi.fn()
        }
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* createWebSocket('ws://test', protocols)
          })
        ),
        () => {}
      )
    })
  })

  describe('createAbortController', () => {
    it('should create and abort controller', async () => {
      let controller: any
      
      vi.mocked(AbortController).mockImplementation(function(this: any) {
        controller = {
          signal: { aborted: false },
          abort: vi.fn()
        }
        return controller
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const ctrl = yield* createAbortController()
            expect(ctrl.signal.aborted).toBe(false)
          })
        ),
        () => {
          expect(controller.abort).toHaveBeenCalled()
        }
      )
    })
  })

  describe('getMediaStream', () => {
    it('should get and stop media stream', async () => {
      const mockTrack = {
        stop: vi.fn()
      }
      
      const mockStream = {
        getTracks: () => [mockTrack, mockTrack]
      }
      
      vi.mocked(navigator.mediaDevices.getUserMedia).mockResolvedValue(mockStream as any)
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const stream = yield* getMediaStream({ video: true, audio: true })
            expect(stream).toBe(mockStream)
          })
        ),
        () => {
          expect(mockTrack.stop).toHaveBeenCalledTimes(2)
        }
      )
    })

    it('should handle getUserMedia errors', async () => {
      vi.mocked(navigator.mediaDevices.getUserMedia).mockRejectedValue(
        new Error('Permission denied')
      )
      
      await expectEffectError(
        Effect.scoped(
          Effect.gen(function* () {
            yield* getMediaStream({ video: true })
          })
        ),
        (error) => {
          expect(error.message).toContain('Failed to get media stream')
          expect(error.message).toContain('Permission denied')
        }
      )
    })
  })

  describe('openFileHandle', () => {
    it('should open file handle', async () => {
      const mockHandle = { kind: 'file', name: 'test.txt' }
      
      ;(window as any).showOpenFilePicker.mockResolvedValue([mockHandle])
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const handle = yield* openFileHandle()
            expect(handle).toBe(mockHandle)
          })
        ),
        () => {}
      )
    })

    it('should support file picker options', async () => {
      const options = {
        types: [{
          description: 'Text files',
          accept: { 'text/plain': ['.txt'] }
        }],
        multiple: false
      }
      
      ;(window as any).showOpenFilePicker.mockImplementation((opts: any) => {
        expect(opts).toEqual(options)
        return Promise.resolve([{ kind: 'file' }])
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* openFileHandle(options)
          })
        ),
        () => {}
      )
    })

    it('should handle file picker errors', async () => {
      ;(window as any).showOpenFilePicker.mockRejectedValue(
        new Error('User cancelled')
      )
      
      await expectEffectError(
        Effect.scoped(
          Effect.gen(function* () {
            yield* openFileHandle()
          })
        ),
        (error) => {
          expect(error.message).toContain('Failed to open file')
          expect(error.message).toContain('User cancelled')
        }
      )
    })
  })

  describe('openIndexedDB', () => {
    it('should open and close IndexedDB', async () => {
      const mockDB = {
        name: 'testDB',
        version: 1,
        close: vi.fn()
      }
      
      const mockRequest = {
        onsuccess: null as any,
        onerror: null as any,
        onupgradeneeded: null as any,
        result: mockDB
      }
      
      vi.mocked(indexedDB.open).mockReturnValue(mockRequest as any)
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const dbPromise = openIndexedDB('testDB', 1)
            
            // Trigger success callback
            mockRequest.onsuccess?.({} as any)
            
            const db = yield* dbPromise
            expect(db).toBe(mockDB)
          })
        ),
        () => {
          expect(mockDB.close).toHaveBeenCalled()
        }
      )
    })

    it('should handle upgrade needed', async () => {
      const onupgradeneeded = vi.fn()
      const mockRequest = {
        onsuccess: null as any,
        onerror: null as any,
        onupgradeneeded: null as any,
        result: { close: vi.fn() }
      }
      
      vi.mocked(indexedDB.open).mockReturnValue(mockRequest as any)
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const dbPromise = openIndexedDB('testDB', 2, onupgradeneeded)
            
            // Trigger upgrade callback
            const event = {} as IDBVersionChangeEvent
            mockRequest.onupgradeneeded?.(event)
            
            // Then success
            mockRequest.onsuccess?.({} as any)
            
            yield* dbPromise
            
            expect(onupgradeneeded).toHaveBeenCalledWith(event)
          })
        ),
        () => {}
      )
    })

    it('should handle IndexedDB errors', async () => {
      const mockRequest = {
        onsuccess: null as any,
        onerror: null as any,
        onupgradeneeded: null as any
      }
      
      vi.mocked(indexedDB.open).mockReturnValue(mockRequest as any)
      
      await expectEffectError(
        Effect.scoped(
          Effect.gen(function* () {
            const dbPromise = openIndexedDB('testDB')
            
            // Trigger error callback
            mockRequest.onerror?.({} as any)
            
            yield* dbPromise
          })
        ),
        (error) => {
          expect(error.message).toBe('Failed to open IndexedDB')
        }
      )
    })
  })

  describe('createWorker', () => {
    it('should create and terminate worker', async () => {
      let worker: any
      
      vi.mocked(Worker).mockImplementation(function(this: any, scriptURL, options) {
        worker = {
          scriptURL,
          options,
          terminate: vi.fn()
        }
        return worker
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            const w = yield* createWorker('worker.js')
            expect(w.scriptURL).toBe('worker.js')
          })
        ),
        () => {
          expect(worker.terminate).toHaveBeenCalled()
        }
      )
    })

    it('should support worker options', async () => {
      const options: WorkerOptions = { type: 'module' }
      
      vi.mocked(Worker).mockImplementation(function(this: any, scriptURL, opts) {
        expect(opts).toEqual(options)
        return {
          terminate: vi.fn()
        }
      })
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* createWorker('module-worker.js', options)
          })
        ),
        () => {}
      )
    })
  })

  describe('withResources', () => {
    it('should run effect with scope', async () => {
      let resourceCleaned = false
      
      const effect = Effect.gen(function* () {
        yield* Effect.acquireRelease(
          Effect.succeed('resource'),
          () => Effect.sync(() => { resourceCleaned = true })
        )
        return 'result'
      })
      
      await expectEffect(
        withResources(effect),
        (result) => {
          expect(result).toBe('result')
          expect(resourceCleaned).toBe(true)
        }
      )
    })
  })

  describe('addEventListeners', () => {
    it('should add multiple event listeners', async () => {
      const target = document.createElement('div')
      const listeners = {
        click: vi.fn(),
        mouseover: vi.fn(),
        mouseout: vi.fn()
      }
      
      const addSpy = vi.spyOn(target, 'addEventListener')
      const removeSpy = vi.spyOn(target, 'removeEventListener')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addEventListeners(target, listeners)
            
            expect(addSpy).toHaveBeenCalledTimes(3)
            expect(addSpy).toHaveBeenCalledWith('click', listeners.click, undefined)
            expect(addSpy).toHaveBeenCalledWith('mouseover', listeners.mouseover, undefined)
            expect(addSpy).toHaveBeenCalledWith('mouseout', listeners.mouseout, undefined)
          })
        ),
        () => {
          expect(removeSpy).toHaveBeenCalledTimes(3)
          expect(removeSpy).toHaveBeenCalledWith('click', listeners.click, undefined)
        }
      )
    })

    it('should support options for all listeners', async () => {
      const target = document.createElement('div')
      const listeners = { click: vi.fn() }
      const options = { capture: true }
      
      const addSpy = vi.spyOn(target, 'addEventListener')
      
      await expectEffect(
        Effect.scoped(
          Effect.gen(function* () {
            yield* addEventListeners(target, listeners, options)
            
            expect(addSpy).toHaveBeenCalledWith('click', listeners.click, options)
          })
        ),
        () => {}
      )
    })
  })

  describe('createResourcePool', () => {
    it('should acquire and release resources', async () => {
      let created = 0
      let destroyed = 0
      
      const create = () => Effect.sync(() => {
        created++
        return { id: created }
      })
      
      const destroy = () => Effect.sync(() => {
        destroyed++
      })
      
      const pool = createResourcePool(create, destroy, 3)
      
      // Acquire resources
      const r1 = await Effect.runPromise(pool.acquire())
      const r2 = await Effect.runPromise(pool.acquire())
      
      expect(created).toBe(2)
      expect(r1.id).toBe(1)
      expect(r2.id).toBe(2)
      
      // Release one
      await Effect.runPromise(pool.release(r1))
      
      // Acquire again (should reuse)
      const r3 = await Effect.runPromise(pool.acquire())
      expect(created).toBe(2) // No new creation
      expect(r3).toBe(r1) // Reused
      
      // Cleanup
      await Effect.runPromise(pool.destroyAll())
      expect(destroyed).toBe(2)
    })

    it('should handle pool capacity', async () => {
      const create = () => Effect.succeed({ resource: Math.random() })
      const destroy = () => Effect.void
      
      const pool = createResourcePool(create, destroy, 2)
      
      // Fill pool
      const r1 = await Effect.runPromise(pool.acquire())
      const r2 = await Effect.runPromise(pool.acquire())
      
      // Pool at capacity, should wait
      const acquirePromise = Effect.runPromise(pool.acquire())
      
      // Release one after delay
      setTimeout(() => {
        Effect.runPromise(pool.release(r1))
      }, 50)
      
      const r3 = await acquirePromise
      expect(r3).toBeDefined()
    })

    it('should drop oldest on backpressure', async () => {
      let nextId = 0
      const create = () => Effect.succeed({ id: nextId++ })
      const destroy = () => Effect.void
      
      const pool = createResourcePool(create, destroy, 2)
      
      // Acquire max resources
      const r1 = await Effect.runPromise(pool.acquire())
      const r2 = await Effect.runPromise(pool.acquire())
      
      expect(r1.id).toBe(0)
      expect(r2.id).toBe(1)
      
      // Release both
      await Effect.runPromise(pool.release(r1))
      await Effect.runPromise(pool.release(r2))
      
      // Pool should have both
      const r3 = await Effect.runPromise(pool.acquire())
      const r4 = await Effect.runPromise(pool.acquire())
      
      // Should reuse existing
      expect([r3.id, r4.id].sort()).toEqual([0, 1])
    })

    it('should destroy all resources', async () => {
      const resources: any[] = []
      const create = () => Effect.sync(() => {
        const r = { id: resources.length, destroyed: false }
        resources.push(r)
        return r
      })
      
      const destroy = (r: any) => Effect.sync(() => {
        r.destroyed = true
      })
      
      const pool = createResourcePool(create, destroy, 5)
      
      // Create some resources
      const r1 = await Effect.runPromise(pool.acquire())
      const r2 = await Effect.runPromise(pool.acquire())
      await Effect.runPromise(pool.release(r1))
      
      // Destroy all
      await Effect.runPromise(pool.destroyAll())
      
      // All should be destroyed
      expect(resources.every(r => r.destroyed)).toBe(true)
    })
  })
})