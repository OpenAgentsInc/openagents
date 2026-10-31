// OpenAgents for Mac (docs/desktop/coax-desktop.md): an AppKit window and a
// Metal view drawing chat.coax compiled to C, as host.js draws chat.wasm in
// the browser. The host holds no product logic: it forwards input, draws
// the vertices, and answers chat.coax's requests from the local backend.
import AppKit
import CoreText
import MetalKit

let backend = Backend()
var chatState: UnsafeMutableRawPointer!
var nextId: Int32 = 0

// chat.coax's HTTP asks, answered on the next turn of the run loop (message 512 + id).
let http: dk_http_fn = { url, urlLen, body, bodyLen, post in
  let u = String(decoding: UnsafeBufferPointer(start: url, count: Int(urlLen)), as: UTF8.self)
  let b = bodyLen > 0 && body != nil ? Data(bytes: body!, count: Int(bodyLen)) : nil
  let id = nextId; nextId = (nextId + 1) % 8
  DispatchQueue.global().async {
    let answer = Array(backend.handle(url: u, body: b, post: post != 0).utf8)
    DispatchQueue.main.async { answer.withUnsafeBufferPointer { dk_message(chatState, 512 + id, $0.baseAddress, Int32($0.count)) } }
  }
  return id
}

// The sans atlas (host.js sansAtlas, font.coax): ASCII 32..126 then the extras, 16 columns of
// 96-texel cells, the pen at x 16 and the baseline at y 72, Helvetica at 64 px, each mip drawn at its size.
let sansExtra: [UInt32] = [183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800]
func sansAtlas(_ device: MTLDevice) -> MTLTexture {
  let codes = (32...126).map { UInt32($0) } + sansExtra
  let cols = 16, rows = (codes.count + 15) / 16, cell = 96, levels = 5
  let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: cols * cell, height: rows * cell, mipmapped: true)
  d.mipmapLevelCount = levels
  let texture = device.makeTexture(descriptor: d)!
  for level in 0..<levels {
    let c = cell >> level, w = cols * c, h = rows * c, scale = CGFloat(1 << level)
    let g = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    g.setFillColor(gray: 0, alpha: 1); g.fill(CGRect(x: 0, y: 0, width: w, height: h))
    g.setShouldSmoothFonts(false); g.setAllowsAntialiasing(true); g.textMatrix = .identity
    let font = CTFontCreateWithName("Helvetica" as CFString, 64 / scale, nil)
    let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: CGColor(gray: 1, alpha: 1)]
    for (i, code) in codes.enumerated() {
      let line = CTLineCreateWithAttributedString(NSAttributedString(string: String(Character(Unicode.Scalar(code)!)), attributes: attrs))
      g.textPosition = CGPoint(x: CGFloat((i % cols) * c) + 16 / scale, y: CGFloat(h) - (CGFloat((i / cols) * c) + 72 / scale))
      CTLineDraw(line, g)
    }
    texture.replace(region: MTLRegionMake2D(0, 0, w, h), mipmapLevel: level, withBytes: g.data!, bytesPerRow: w)
  }
  return texture
}

final class ChatView: MTKView, MTKViewDelegate {
  var queue: MTLCommandQueue!
  var bg: MTLRenderPipelineState!, fx: MTLRenderPipelineState!
  var sans: MTLTexture!, sampler: MTLSamplerState!
  var vertices: [MTLBuffer] = [], slot = 0
  let inflight = DispatchSemaphore(value: 2)
  var uniforms = [Float](repeating: 0, count: Int(dk_uniform_floats()))
  var last = CACurrentMediaTime()
  var hand = false

  init(frame: CGRect) {
    let device = MTLCreateSystemDefaultDevice()!
    super.init(frame: frame, device: device)
    colorPixelFormat = .bgra8Unorm
    preferredFramesPerSecond = 60
    delegate = self
    queue = device.makeCommandQueue()
    let library = try! device.makeDefaultLibrary(bundle: .main)
    let b = MTLRenderPipelineDescriptor()
    b.vertexFunction = library.makeFunction(name: "bg_vs"); b.fragmentFunction = library.makeFunction(name: "bg_fs")
    b.colorAttachments[0].pixelFormat = colorPixelFormat
    bg = try! device.makeRenderPipelineState(descriptor: b)
    let f = MTLRenderPipelineDescriptor()
    f.vertexFunction = library.makeFunction(name: "fx_vs"); f.fragmentFunction = library.makeFunction(name: "fx_fs")
    let v = MTLVertexDescriptor()
    for i in 0..<3 { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = Int(dk_vertex_floats()) * 4
    f.vertexDescriptor = v
    let a = f.colorAttachments[0]!
    a.pixelFormat = colorPixelFormat; a.isBlendingEnabled = true
    a.rgbBlendOperation = .max; a.alphaBlendOperation = .max
    a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = .one; a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .one
    fx = try! device.makeRenderPipelineState(descriptor: f)
    sans = sansAtlas(device)
    let s = MTLSamplerDescriptor()
    s.minFilter = .linear; s.magFilter = .linear; s.mipFilter = .linear
    sampler = device.makeSamplerState(descriptor: s)
    let bytes = Int(dk_max_vertices()) * Int(dk_vertex_floats()) * 4
    vertices = (0..<2).map { _ in device.makeBuffer(length: bytes, options: .storageModeShared)! }
    addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect, .cursorUpdate], owner: self))
  }
  required init(coder: NSCoder) { fatalError() }

  override var acceptsFirstResponder: Bool { true }
  override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
  func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

  func draw(in view: MTKView) {
    guard let pass = currentRenderPassDescriptor, let drawable = currentDrawable else { return }
    render(pass, size: drawableSize, dpr: Float(window?.backingScaleFactor ?? 2)) { $0.present(drawable) }
  }

  // One frame: chat.coax writes the vertices and uniforms, then the two passes.
  func render(_ pass: MTLRenderPassDescriptor, size: CGSize, dpr: Float, done: ((MTLCommandBuffer) -> Void)? = nil) {
    let now = CACurrentMediaTime(), dt = Float(now - last); last = now
    inflight.wait()
    slot = (slot + 1) % 2
    let buf = vertices[slot]
    let n = uniforms.withUnsafeMutableBufferPointer { dk_frame(chatState, buf.contents().assumingMemoryBound(to: Float.self), $0.baseAddress, Float(size.width), Float(size.height), dpr, dt) }
    act()
    let want = dk_cursor(chatState) != 0
    if want != hand { hand = want; (want ? NSCursor.pointingHand : NSCursor.arrow).set() }
    guard let cmd = queue.makeCommandBuffer(), let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { inflight.signal(); return }
    let ub = uniforms.count * 4
    enc.setRenderPipelineState(bg)
    enc.setFragmentBytes(uniforms, length: ub, index: 0)
    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    if n > 0 {
      enc.setRenderPipelineState(fx)
      enc.setVertexBuffer(buf, offset: 0, index: 0)
      enc.setVertexBytes(uniforms, length: ub, index: 1)
      enc.setFragmentBytes(uniforms, length: ub, index: 0)
      enc.setFragmentTexture(sans, index: 0); enc.setFragmentSamplerState(sampler, index: 0)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    enc.endEncoding()
    cmd.addCompletedHandler { [inflight] _ in inflight.signal() }
    done?(cmd)
    cmd.commit()
  }

  // --shot PATH [--click X,Y] [--type TEXT] [--frames N]: draw offscreen (clicking at device pixel X,Y, then typing TEXT and Enter) for a second and a half, write the last frame as a PNG, quit.
  func shot(_ path: String, width: Int, height: Int) {
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: colorPixelFormat, width: width, height: height, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    let target = device!.makeTexture(descriptor: d)!
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = target; pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
    var frames = 0
    let last = CommandLine.arguments.firstIndex(of: "--frames").flatMap { Int(CommandLine.arguments[$0 + 1]) } ?? 90
    Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { timer in
      frames += 1
      if frames == 30, let i = CommandLine.arguments.firstIndex(of: "--click"), i + 1 < CommandLine.arguments.count {
        let xy = CommandLine.arguments[i + 1].split(separator: ",").compactMap { Float($0) }
        if xy.count == 2 { _ = dk_pointer(chatState, xy[0], xy[1], 0); _ = dk_pointer(chatState, xy[0], xy[1], 2) }
      }
      if frames == 45, let i = CommandLine.arguments.firstIndex(of: "--type"), i + 1 < CommandLine.arguments.count {
        self.typed(CommandLine.arguments[i + 1]); _ = dk_key(chatState, -2)
      }
      self.render(pass, size: CGSize(width: width, height: height), dpr: 2) { cmd in
        guard frames == last else { return }
        timer.invalidate()
        cmd.addCompletedHandler { _ in
          DispatchQueue.main.async {
            var px = [UInt8](repeating: 0, count: width * height * 4)
            target.getBytes(&px, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
            let g = CGContext(data: &px, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.noneSkipFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)!
            let rep = NSBitmapImageRep(cgImage: g.makeImage()!)
            try? rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
            exit(0)
          }
        }
      }
    }
  }

  // A navigation chat.coax asked for: openagents.com's own pages open in the browser.
  func act() {
    var url = [UInt8](repeating: 0, count: 256)
    let n = dk_take_action(chatState, &url, 256)
    if n > 0, let path = String(bytes: url[0..<Int(n)], encoding: .utf8), let u = URL(string: "https://openagents.com" + path) { NSWorkspace.shared.open(u) }
  }

  func place(_ e: NSEvent) -> (Float, Float) {
    let p = convert(e.locationInWindow, from: nil), d = window?.backingScaleFactor ?? 2
    return (Float(p.x * d), Float((bounds.height - p.y) * d))
  }
  func pointer(_ e: NSEvent, _ phase: Int32) { let (x, y) = place(e); _ = dk_pointer(chatState, x, y, phase) }
  override func mouseDown(with e: NSEvent) { window?.makeFirstResponder(self); pointer(e, 0) }
  override func mouseDragged(with e: NSEvent) { pointer(e, 1) }
  override func mouseMoved(with e: NSEvent) { pointer(e, 1) }
  override func mouseUp(with e: NSEvent) { pointer(e, 2) }
  override func scrollWheel(with e: NSEvent) { dk_wheel(chatState, Float(-e.scrollingDeltaY * (e.hasPreciseScrollingDeltas ? 1 : 16))) }
  override func cursorUpdate(with event: NSEvent) { (hand ? NSCursor.pointingHand : NSCursor.arrow).set() }

  func typed(_ s: String) {
    let b = Array(s.utf8)
    b.withUnsafeBufferPointer { dk_message(chatState, 1, $0.baseAddress, Int32($0.count)) }
  }
  override func performKeyEquivalent(with e: NSEvent) -> Bool {
    if e.modifierFlags.contains(.command), e.charactersIgnoringModifiers == "v", dk_wants_text(chatState) != 0,
       let s = NSPasteboard.general.string(forType: .string) { typed(s); return true }
    return super.performKeyEquivalent(with: e)
  }
  static let named: [UInt16: Int32] = [53: -1, 36: -2, 76: -2, 51: -3, 123: -4, 124: -5, 126: -6, 125: -7, 115: -8, 119: -9, 116: -10, 121: -11, 117: -12]
  override func keyDown(with e: NSEvent) {
    let text = dk_wants_text(chatState) != 0
    if e.keyCode == 36 && e.modifierFlags.contains(.shift) && text { typed("\n"); return }
    if let k = ChatView.named[e.keyCode] { _ = dk_key(chatState, k); return }
    guard let s = e.characters, !s.isEmpty, !e.modifierFlags.contains(.command) else { return }
    if text { typed(s) } else { for u in s.unicodeScalars { _ = dk_key(chatState, Int32(u.value)) } }
  }
}

final class App: NSObject, NSApplicationDelegate {
  var window: NSWindow!
  func applicationDidFinishLaunching(_ n: Notification) {
    Router.loadKey()
    chatState = dk_new(http)
    Timer.scheduledTimer(withTimeInterval: 0.12, repeats: true) { _ in
      guard let lines = backend.pushed() else { return }
      Array(lines.utf8).withUnsafeBufferPointer { dk_message(chatState, 603, $0.baseAddress, Int32($0.count)) }
    }
    NSApp.mainMenu = App.menus()
    window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1320, height: 880),
                      styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
    window.title = "OpenAgents"
    window.titlebarAppearsTransparent = true; window.titleVisibility = .hidden
    window.minSize = NSSize(width: 900, height: 600)
    // The Zeron-fork chat screen (Zeron.swift); the HUD's ChatView stays in the tree, unshown.
    window.backgroundColor = NSColor(red: 13 / 255, green: 13 / 255, blue: 13 / 255, alpha: 1)
    let zeron = ZeronView(frame: window.contentRect(forFrameRect: window.frame))
    window.contentView = zeron
    window.center()
    let args = CommandLine.arguments
    zeron.configure(args)
    // --shot draws offscreen; the window never shows.
    if !args.contains("--shot") { window.makeKeyAndOrderFront(nil) }
    window.makeFirstResponder(window.contentView)
    if let i = args.firstIndex(of: "--shot"), i + 1 < args.count {
      let view = window.contentView as! ZeronView
      view.isPaused = true
      let size = args.firstIndex(of: "--size").flatMap { j in j + 1 < args.count ? args[j + 1].split(separator: ",").compactMap { Int($0) } : nil } ?? [2640, 1760]
      view.shot(args[i + 1], width: size.count == 2 ? size[0] : 2640, height: size.count == 2 ? size[1] : 1760)
      return
    }
    NSApp.activate(ignoringOtherApps: true)
    // --fps SECS: the window as you see it, its presented frames per second after a second's warm-up, printed; then quit.
    if let i = args.firstIndex(of: "--fps"), i + 1 < args.count, let secs = Double(args[i + 1]) {
      DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
        zeron.presented = 0; zeron.slowest = 0
        let start = CACurrentMediaTime()
        DispatchQueue.main.asyncAfter(deadline: .now() + secs) {
          let rate = Double(zeron.presented) / (CACurrentMediaTime() - start)
          print(String(format: "%.1f fps over %.0f s at %.0fx%.0f (scale %.0f), slowest frame %.1f ms on the CPU", rate, secs,
                       zeron.drawableSize.width, zeron.drawableSize.height, self.window.backingScaleFactor, zeron.slowest * 1000))
          NSApp.terminate(nil)
        }
      }
    }
  }
  func applicationShouldTerminateAfterLastWindowClosed(_ s: NSApplication) -> Bool { true }
  // Claude Code's process group goes with the app.
  func applicationWillTerminate(_ n: Notification) { if let v = window?.contentView as? ZeronView { dsk_quit(v.desk) } }

  // Zeron's menu bar (crates/ui/src/app_menus.rs) for what this app has: the
  // app menu, File (New Session Cmd+N, New Session in Folder… Shift+Cmd+N, Close Tab Cmd+W), View (Toggle Sidebar Cmd+S), Edit (the composer's Undo, Redo, Cut, Copy, Paste, Select All,
  // by their native selectors through the responder chain; AppKit adds
  // Dictation and Emoji & Symbols to a menu titled Edit), and Window.
  static func menus() -> NSMenu {
    let bar = NSMenu()
    func menu(_ title: String, _ items: [NSMenuItem]) {
      let top = NSMenuItem(), sub = NSMenu(title: title)
      for i in items { sub.addItem(i) }
      top.submenu = sub; bar.addItem(top)
    }
    func item(_ title: String, _ action: Selector?, _ key: String = "", _ mods: NSEvent.ModifierFlags = .command) -> NSMenuItem {
      let i = NSMenuItem(title: title, action: action, keyEquivalent: key)
      i.keyEquivalentModifierMask = mods
      return i
    }
    let services = NSMenuItem(title: "Services", action: nil, keyEquivalent: ""), servicesMenu = NSMenu(title: "Services")
    services.submenu = servicesMenu; NSApp.servicesMenu = servicesMenu
    menu("OpenAgents", [
      item("About OpenAgents", #selector(NSApplication.orderFrontStandardAboutPanel(_:))), .separator(),
      services, .separator(),
      item("Hide OpenAgents", #selector(NSApplication.hide(_:)), "h"),
      item("Hide Others", #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]),
      item("Show All", #selector(NSApplication.unhideAllApplications(_:))), .separator(),
      item("Quit OpenAgents", #selector(NSApplication.terminate(_:)), "q"),
    ])
    menu("File", [item("New Session", #selector(ZeronView.newChat(_:)), "n"),
                  item("New Session in Folder…", #selector(ZeronView.newSessionInFolder(_:)), "n", [.command, .shift]), .separator(),
                  item("Close Tab", #selector(ZeronView.closeTab(_:)), "w")])
    menu("Edit", [
      item("Undo", #selector(ZeronView.undo(_:)), "z"),
      item("Redo", #selector(ZeronView.redo(_:)), "z", [.command, .shift]), .separator(),
      item("Cut", #selector(ZeronView.cut(_:)), "x"),
      item("Copy", #selector(ZeronView.copy(_:)), "c"),
      item("Paste", #selector(ZeronView.paste(_:)), "v"), .separator(),
      item("Select All", #selector(ZeronView.selectAll(_:)), "a"),
    ])
    menu("View", [item("Toggle Sidebar", #selector(ZeronView.toggleSidebar(_:)), "s")])
    let window = [item("Minimize", #selector(NSWindow.performMiniaturize(_:)), "m"),
                  item("Zoom", #selector(NSWindow.performZoom(_:))), .separator(),
                  item("Close Window", #selector(NSWindow.performClose(_:)), "w", [.command, .shift])]
    menu("Window", window)
    NSApp.windowsMenu = bar.items.last?.submenu
    return bar
  }
}

// --compare SHOT GOLDEN [PERCENT]: how many pixels of SHOT differ from GOLDEN by more than 48 of 255 in a
// channel, printed; exit 1 when the sizes differ or more than PERCENT (default 0.1) do (check.sh's goldens).
if let i = CommandLine.arguments.firstIndex(of: "--compare"), i + 2 < CommandLine.arguments.count {
  let args = CommandLine.arguments
  func pixels(_ path: String) -> (Int, Int, [UInt8])? {
    guard let image = NSImage(contentsOfFile: path)?.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
    let w = image.width, h = image.height
    var px = [UInt8](repeating: 0, count: w * h * 4)
    let g = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                      bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
    g.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
    return (w, h, px)
  }
  guard let a = pixels(args[i + 1]), let b = pixels(args[i + 2]) else { print("compare: cannot read \(args[i + 1]) or \(args[i + 2])"); exit(1) }
  guard a.0 == b.0, a.1 == b.1 else { print("compare: \(a.0)x\(a.1) against \(b.0)x\(b.1)"); exit(1) }
  var over = 0, most = 0
  for p in 0..<(a.0 * a.1) {
    var worst = 0
    for c in 0..<3 { worst = max(worst, abs(Int(a.2[p * 4 + c]) - Int(b.2[p * 4 + c]))) }
    most = max(most, worst)
    if worst > 48 { over += 1 }
  }
  let percent = 100 * Double(over) / Double(a.0 * a.1)
  let limit = i + 3 < args.count ? Double(args[i + 3]) ?? 0.1 : 0.1
  print(String(format: "%d of %d pixels differ by more than 48 (%.4f%%, at most %d)", over, a.0 * a.1, percent, most))
  exit(percent > limit ? 1 : 0)
}

// --route TEXT: Mammoth's decision for TEXT on this Mac, printed; no window (a live check of the Lab's router).
if let i = CommandLine.arguments.firstIndex(of: "--route"), i + 1 < CommandLine.arguments.count {
  Router.loadKey()
  let done = DispatchSemaphore(value: 0)
  var code: Int32 = 0
  Router.decide(CommandLine.arguments[i + 1]) { json, ok in print(json); code = ok ? 0 : 1; done.signal() }
  done.wait()
  exit(code)
}

let app = NSApplication.shared
let delegate = App()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
