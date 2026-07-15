import { Window } from "happy-dom"
import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  acquireComposerImages,
  installComposerImageAcquisition,
  type ComposerImageAcquisitionHost,
} from "./composer-image-acquisition.ts"
import { COMPOSER_IMAGE_BYTES_LIMIT, COMPOSER_IMAGE_COUNT_LIMIT } from "./composer-images.ts"

const cleanups: Array<() => void> = []
afterEach(() => { while (cleanups.length > 0) cleanups.pop()?.() })

const fixture = () => {
  const window = new Window({ url: "http://localhost" })
  const composer = window.document.createElement("section")
  composer.dataset.enKey = "shell-composer"
  const input = window.document.createElement("textarea")
  composer.appendChild(input)
  window.document.body.appendChild(composer)
  const added: Array<{ name: string; data: string }> = []
  const rejected: string[] = []
  let pending = false
  let imageCount = 0
  const host: ComposerImageAcquisitionHost = {
    readSnapshot: async () => ({ pending, imageCount }),
    add: async attachment => { added.push(attachment); imageCount += 1 },
    reject: async message => { rejected.push(message) },
  }
  cleanups.push(installComposerImageAcquisition(window as unknown as globalThis.Window, host))
  return { window, composer, input, added, rejected, host, setPending: (value: boolean) => { pending = value }, setCount: (value: number) => { imageCount = value } }
}

const file = (window: Window, name: string, type: string, bytes: Uint8Array) =>
  new window.File([bytes], name, { type }) as unknown as File

const dispatchWith = (target: { dispatchEvent: (event: never) => boolean }, window: Window, type: string, property: string, value: unknown): Event => {
  const event = new window.Event(type, { bubbles: true, cancelable: true })
  Object.defineProperty(event, property, { configurable: true, value })
  target.dispatchEvent(event as never)
  return event as unknown as Event
}

const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 10))

describe("composer image DOM acquisition", () => {
  test("paste and drop converge on the same typed add path while text paste falls through", async () => {
    const f = fixture()
    const png = file(f.window, "paste.png", "image/png", new Uint8Array([1, 2, 3]))
    const paste = dispatchWith(f.input, f.window, "paste", "clipboardData", {
      items: [{ kind: "file", getAsFile: () => png }],
    })
    await settle()
    expect(paste.defaultPrevented).toBe(true)
    expect(f.added.map(item => item.name)).toEqual(["paste.png"])

    const webp = file(f.window, "drop.webp", "image/webp", new Uint8Array([4, 5]))
    const drop = dispatchWith(f.composer, f.window, "drop", "dataTransfer", {
      files: [webp], dropEffect: "none",
    })
    await settle()
    expect(drop.defaultPrevented).toBe(true)
    expect(f.added.map(item => item.name)).toEqual(["paste.png", "drop.webp"])

    const textPaste = dispatchWith(f.input, f.window, "paste", "clipboardData", { items: [] })
    expect(textPaste.defaultPrevented).toBe(false)
  })

  test("wrong type, oversize, count limit, and pending state fail honestly", async () => {
    const f = fixture()
    const wrongType = dispatchWith(f.composer, f.window, "drop", "dataTransfer", {
      files: [file(f.window, "vector.svg", "image/svg+xml", new Uint8Array([1]))],
      dropEffect: "none",
    })
    await settle()
    expect(wrongType.defaultPrevented).toBe(true)
    expect(f.rejected.at(-1)).toContain("supported image")

    await acquireComposerImages([{
      name: "huge.png",
      type: "image/png",
      size: COMPOSER_IMAGE_BYTES_LIMIT + 1,
      arrayBuffer: async () => new ArrayBuffer(0),
    } as File], f.host)
    expect(f.rejected.at(-1)).toContain("10 MB")

    f.setCount(COMPOSER_IMAGE_COUNT_LIMIT)
    await acquireComposerImages([file(f.window, "extra.png", "image/png", new Uint8Array([1]))], f.host)
    expect(f.rejected.at(-1)).toContain(String(COMPOSER_IMAGE_COUNT_LIMIT))

    f.setCount(0)
    f.setPending(true)
    const before = f.added.length
    await acquireComposerImages([file(f.window, "pending.png", "image/png", new Uint8Array([1]))], f.host)
    expect(f.added).toHaveLength(before)
  })

  test("serializes rapid batches so the count bound cannot race", async () => {
    const f = fixture()
    f.setCount(COMPOSER_IMAGE_COUNT_LIMIT - 1)
    dispatchWith(f.composer, f.window, "drop", "dataTransfer", {
      files: [file(f.window, "last.png", "image/png", new Uint8Array([1]))],
      dropEffect: "none",
    })
    dispatchWith(f.input, f.window, "paste", "clipboardData", {
      items: [{
        kind: "file",
        getAsFile: () => file(f.window, "overflow.png", "image/png", new Uint8Array([2])),
      }],
    })
    await settle()
    expect(f.added.map(item => item.name)).toEqual(["last.png"])
    expect(f.rejected.at(-1)).toContain(String(COMPOSER_IMAGE_COUNT_LIMIT))
  })
})
