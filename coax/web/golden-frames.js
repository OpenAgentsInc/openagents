// Firm reason: 1 (browser glue: reads a WebGPU canvas's frame back from the GPU and shows it for golden-frames.coax's screenshot)
// golden-frames.coax injects this into every page it opens, before the page's
// own scripts. On Linux, headless Chromium composites in software when WebGPU
// runs on the GPU, and a screenshot leaves the WebGPU canvas out (#12678), so
// the frame is read where it is drawn: every canvas is configured with
// COPY_SRC as well, and coaxGoldenFrame() copies the next frame's texture into
// a buffer once the frame's commands are queued (a microtask after the page
// takes the frame's texture, before the frame is shown), maps it, and shows it
// on a 2D canvas laid over the page where the WebGPU canvas sits (software
// compositing draws a 2D canvas). It resolves "shown" two frames later, or ""
// when the page has no WebGPU canvas; either way the tool then takes an
// ordinary screenshot, so a golden is the same kind of PNG as the Mac's, and
// coaxGoldenFrameDone() takes the cover away. Comparing, counting lit pixels
// and writing goldens are golden-frames.coax's.
(() => {
  if (!globalThis.GPUCanvasContext || !globalThis.GPUQueue) return
  const P = GPUCanvasContext.prototype
  const configure = P.configure, current = P.getCurrentTexture
  const devices = new Map()   // canvas → its device
  let wanted = null           // { canvas, resolve, reject }
  P.configure = function (d) {
    devices.set(this.canvas, d.device)
    return configure.call(this, { ...d, usage: (d.usage ?? GPUTextureUsage.RENDER_ATTACHMENT) | GPUTextureUsage.COPY_SRC })
  }
  P.getCurrentTexture = function () {
    const texture = current.call(this), w = wanted
    if (w && w.canvas === this.canvas) {
      wanted = null
      const device = devices.get(this.canvas)
      queueMicrotask(() => read(device, texture).then(w.resolve, w.reject))
    }
    return texture
  }
  async function read(device, texture) {
    const fmt = texture.format
    if (fmt !== "bgra8unorm" && fmt !== "rgba8unorm") throw new Error(`golden-frames: canvas format ${fmt}`)
    const w = texture.width, h = texture.height, row = Math.ceil(w * 4 / 256) * 256
    const buffer = device.createBuffer({ size: row * h, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
    const encoder = device.createCommandEncoder()
    encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow: row }, [w, h])
    device.queue.submit([encoder.finish()])
    await buffer.mapAsync(GPUMapMode.READ)
    const px = new Uint8Array(buffer.getMappedRange()).slice()
    buffer.unmap(); buffer.destroy()
    return { px, w, h, row, bgra: fmt === "bgra8unorm" }
  }
  function page(canvas, f) {
    const image = new ImageData(f.w, f.h), d = image.data
    for (let y = 0; y < f.h; y++) for (let x = 0; x < f.w; x++) {
      const i = y * f.row + x * 4, j = (y * f.w + x) * 4
      d[j] = f.px[i + (f.bgra ? 2 : 0)]; d[j + 1] = f.px[i + 1]; d[j + 2] = f.px[i + (f.bgra ? 0 : 2)]; d[j + 3] = 255
    }
    const frame = document.createElement("canvas"); frame.width = f.w; frame.height = f.h
    frame.getContext("2d").putImageData(image, 0, 0)
    cover?.remove()
    cover = frame
    const r = canvas.getBoundingClientRect()
    frame.style.cssText = `position:fixed;left:${r.left}px;top:${r.top}px;width:${r.width}px;height:${r.height}px;z-index:2147483647;image-rendering:pixelated;pointer-events:none`
    document.documentElement.appendChild(frame)
    return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve("shown"))))
  }
  let cover = null
  globalThis.coaxGoldenFrameDone = () => { cover?.remove(); cover = null; return true }
  globalThis.coaxGoldenFrame = (ms = 5000) => {
    const canvas = document.getElementById("coaxial")
    if (!canvas || !devices.has(canvas)) return Promise.resolve("")
    return new Promise((resolve, reject) => {
      wanted = { canvas, resolve, reject }
      setTimeout(() => { if (wanted?.canvas === canvas) { wanted = null; reject(new Error("golden-frames: no frame drawn")) } }, ms)
    }).then((f) => page(canvas, f))
  }
})()
