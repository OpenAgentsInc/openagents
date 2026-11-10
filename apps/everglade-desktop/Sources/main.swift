// Firm reason: 6 (AppKit and Metal have no C API; this host holds no product logic).
// Everglade for desktop, the Mac host (everglade/docs/desktop-app-spec.md): a window,
// a Metal view and input. Everything drawn comes from workshop.coax through face.h.
//   Everglade                      opens the window
//   Everglade --shot OUT.png [--size WxH] [--at SECONDS]   draws one frame offscreen and exits
import AppKit
import Metal
import MetalKit

final class WorkshopView: MTKView, MTKViewDelegate {
  let state = eg_new()!
  var background: MTLRenderPipelineState!
  var strokes: MTLRenderPipelineState!
  var vertices: MTLBuffer!
  var last = CACurrentMediaTime()
  var time: Float = 0

  init(frame: CGRect) {
    super.init(frame: frame, device: MTLCreateSystemDefaultDevice())
    guard let device else { fatalError("Everglade needs a Metal device.") }
    colorPixelFormat = .bgra8Unorm
    preferredFramesPerSecond = NSScreen.main?.maximumFramesPerSecond ?? 60
    clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    let url = Bundle.main.url(forResource: "default", withExtension: "metallib")!
    let lib = try! device.makeLibrary(URL: url)
    let b = MTLRenderPipelineDescriptor()
    b.vertexFunction = lib.makeFunction(name: "bg_vs"); b.fragmentFunction = lib.makeFunction(name: "bg_fs")
    b.colorAttachments[0].pixelFormat = colorPixelFormat
    background = try! device.makeRenderPipelineState(descriptor: b)
    let f = MTLRenderPipelineDescriptor()
    f.vertexFunction = lib.makeFunction(name: "fx_vs"); f.fragmentFunction = lib.makeFunction(name: "fx_fs")
    let v = MTLVertexDescriptor()
    for i in 0..<3 { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = Int(eg_vertex_floats()) * 4
    f.vertexDescriptor = v
    let a = f.colorAttachments[0]!
    a.pixelFormat = colorPixelFormat; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .max
    a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = .one; a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .one
    strokes = try! device.makeRenderPipelineState(descriptor: f)
    vertices = device.makeBuffer(length: Int(eg_max_vertices()) * Int(eg_vertex_floats()) * 4, options: .storageModeShared)!
    delegate = self
  }
  required init(coder: NSCoder) { fatalError() }

  func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}
  func draw(in view: MTKView) {
    guard let pass = currentRenderPassDescriptor, let drawable = currentDrawable else { return }
    let now = CACurrentMediaTime(); let dt = Float(now - last); last = now
    render(pass, size: drawableSize, scale: Float(window?.backingScaleFactor ?? 2), dt: dt) { $0.present(drawable) }
  }

  func render(_ pass: MTLRenderPassDescriptor, size: CGSize, scale: Float, dt: Float, then: (MTLCommandBuffer) -> Void) {
    time += dt
    let n = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), Float(size.width) / scale, Float(size.height) / scale, scale, dt)
    var u: [Float] = [Float(size.width), Float(size.height), time, 0]
    guard let queue = device?.makeCommandQueue(), let cmd = queue.makeCommandBuffer(), let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    enc.setRenderPipelineState(background)
    enc.setFragmentBytes(&u, length: 16, index: 0)
    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    if n > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    enc.endEncoding()
    then(cmd)
    cmd.commit()
    cmd.waitUntilCompleted()
  }

  // One frame offscreen, after `seconds` of the scene's own time, as a PNG.
  func shot(to path: String, width: Int, height: Int, seconds: Float) -> Bool {
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: colorPixelFormat, width: width, height: height, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    guard let texture = device?.makeTexture(descriptor: d) else { return false }
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = texture; pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
    var left = seconds
    while left > 0 { let step = min(left, 0.05); render(pass, size: CGSize(width: width, height: height), scale: 2, dt: step) { _ in }; left -= step }
    render(pass, size: CGSize(width: width, height: height), scale: 2, dt: 0) { _ in }
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    texture.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
    let info = CGImageAlphaInfo.noneSkipFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
    guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info),
          let image = ctx.makeImage(), let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { return false }
    return (try? png.write(to: URL(fileURLWithPath: path))) != nil
  }

  override var acceptsFirstResponder: Bool { true }
  func point(_ e: NSEvent) -> (Float, Float) { let p = convert(e.locationInWindow, from: nil); return (Float(p.x), Float(bounds.height - p.y)) }
  override func mouseDown(with e: NSEvent) { let p = point(e); eg_pointer(state, p.0, p.1, 0) }
  override func mouseDragged(with e: NSEvent) { let p = point(e); eg_pointer(state, p.0, p.1, 1) }
  override func mouseUp(with e: NSEvent) { let p = point(e); eg_pointer(state, p.0, p.1, 2) }
  override func scrollWheel(with e: NSEvent) { eg_wheel(state, Float(e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 10)) }
}

final class App: NSObject, NSApplicationDelegate {
  var window: NSWindow!
  func applicationDidFinishLaunching(_ note: Notification) {
    let frame = NSRect(x: 0, y: 0, width: 1320, height: 880)
    window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
    window.title = "Everglade"
    window.minSize = NSSize(width: 900, height: 600)
    window.contentView = WorkshopView(frame: frame)
    window.center()
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(window.contentView)
    NSApp.activate(ignoringOtherApps: true)
  }
  func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}

func menus() {
  let bar = NSMenu(), item = NSMenuItem(), menu = NSMenu()
  menu.addItem(withTitle: "Quit Everglade", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
  item.submenu = menu; bar.addItem(item); NSApp.mainMenu = bar
}

let args = CommandLine.arguments
if let i = args.firstIndex(of: "--shot"), i + 1 < args.count {
  var width = 2640, height = 1760, seconds: Float = 2
  if let s = args.firstIndex(of: "--size"), s + 1 < args.count {
    let parts = args[s + 1].split(separator: "x").compactMap { Int($0) }
    if parts.count == 2, parts[0] > 0, parts[1] > 0, parts[0] <= 8192, parts[1] <= 8192 { width = parts[0]; height = parts[1] }
  }
  if let t = args.firstIndex(of: "--at"), t + 1 < args.count, let v = Float(args[t + 1]), v >= 0, v <= 600 { seconds = v }
  let view = WorkshopView(frame: NSRect(x: 0, y: 0, width: width / 2, height: height / 2))
  exit(view.shot(to: args[i + 1], width: width, height: height, seconds: seconds) ? 0 : 1)
}
let app = NSApplication.shared
let delegate = App()
app.delegate = delegate
app.setActivationPolicy(.regular)
menus()
app.run()
