// Firm reason: 6 (AppKit and Metal have no C API; this host holds no product logic).
// Everglade for desktop, the Mac host (everglade/docs/desktop-app-spec.md): a window,
// a Metal view and input. Everything drawn comes from scenes.coax (the list of worlds, the
// Workshop, the Archery Range) through face.h. Escape is the list; --scene N opens on world N
// (1 Everglade, 2 the Workshop, 3 the Archery Range, 4 Experimental).
//   Everglade                      opens the window
//   Everglade --shot OUT.png [--size WxH] [--at SECONDS] [--hold KEYS] [--scene N]
//       draws one frame offscreen after SECONDS with KEYS held (letters, as "wd"), and exits;
//       without --scene it is the list of worlds
//   Everglade --shot OUT.png --next WAIT [--reply TEXT]
//       presses Next issue first and waits up to WAIT seconds for the agent (a real Haiku turn on
//       your login); --reply then types TEXT as your answer and waits for the agent again. A reply
//       that starts an issue starts the coding agent: set EVERGLADE_CODER_MODE=rehearsal (read-only)
//       and EVERGLADE_CODER_MODEL=claude-haiku-5-5 to try the flow without changing anything.
//       Autopilot is on, so the loop goes round by itself: --runs N stops waiting after N coding turns
import AppKit
import Metal
import MetalKit

final class WorkshopView: MTKView, MTKViewDelegate {
  let state = eg_new()!
  var temple: Place!, range: Place!
  // The place behind the world that is open (face.h eg_place).
  var place: Place { eg_place(state) == 2 ? range : temple }
  var strokes: MTLRenderPipelineState!
  var vertices: MTLBuffer!
  var model: Model?
  let next = nxp_new()!
  var fetching = false
  var overlay: MTLBuffer!
  var panel: MTLRenderPipelineState!
  var flat: MTLDepthStencilState!
  var faces: MTLTexture!
  var faceSampler: MTLSamplerState!
  var queue: MTLCommandQueue!     // one for the life of the view: Metal means a queue to be kept
  var last = CACurrentMediaTime()
  var time: Float = 0

  init(frame: CGRect) {
    super.init(frame: frame, device: MTLCreateSystemDefaultDevice())
    guard let device else { fatalError("Everglade needs a Metal device.") }
    colorPixelFormat = .bgra8Unorm
    preferredFramesPerSecond = NSScreen.main?.maximumFramesPerSecond ?? 60
    clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    depthStencilPixelFormat = .depth32Float; clearDepth = 1
    queue = device.makeCommandQueue()
    let url = Bundle.main.url(forResource: "default", withExtension: "metallib")!
    let lib = try! device.makeLibrary(URL: url)
    let b = MTLRenderPipelineDescriptor()
    // The place: the temple's parts and Aethyr's shaders. Without them there is nowhere to stand.
    guard let pl = Place(device: device, color: colorPixelFormat), let rg = Place(device: device, color: colorPixelFormat, range: true) else {
      FileHandle.standardError.write(Data("Everglade: the temple or the range did not build, or their shaders are missing. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    temple = pl; range = rg
    let f = MTLRenderPipelineDescriptor()
    f.vertexFunction = lib.makeFunction(name: "fx_vs"); f.fragmentFunction = lib.makeFunction(name: "fx_fs")
    let v = MTLVertexDescriptor()
    for i in 0..<3 { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = Int(eg_vertex_floats()) * 4
    f.vertexDescriptor = v; f.depthAttachmentPixelFormat = .depth32Float
    let a = f.colorAttachments[0]!
    a.pixelFormat = colorPixelFormat; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .max
    a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = .one; a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .one
    strokes = try! device.makeRenderPipelineState(descriptor: f)
    vertices = device.makeBuffer(length: Int(eg_max_vertices()) * Int(eg_vertex_floats()) * 4, options: .storageModeShared)!
    overlay = device.makeBuffer(length: Int(nxp_max_vertices()) * Int(nxp_vertex_floats()) * 4, options: .storageModeShared)!
    let r = MTLRenderPipelineDescriptor()
    r.vertexFunction = lib.makeFunction(name: "rect_vs"); r.fragmentFunction = lib.makeFunction(name: "rect_fs")
    r.depthAttachmentPixelFormat = .depth32Float
    let ra = r.colorAttachments[0]!
    ra.pixelFormat = colorPixelFormat; ra.isBlendingEnabled = true
    ra.sourceRGBBlendFactor = .sourceAlpha; ra.destinationRGBBlendFactor = .oneMinusSourceAlpha; ra.sourceAlphaBlendFactor = .one; ra.destinationAlphaBlendFactor = .one
    panel = try! device.makeRenderPipelineState(descriptor: r)
    let fd = MTLDepthStencilDescriptor(); fd.depthCompareFunction = .always; fd.isDepthWriteEnabled = false
    flat = device.makeDepthStencilState(descriptor: fd)!
    // Everglade's faces, from the font files in the app. Without them there is no text to read.
    guard let atlas = faceAtlas(device) else {
      FileHandle.standardError.write(Data("Everglade: the app's fonts (Spectral-Medium.ttf, Cinzel.ttf) are missing. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    faces = atlas
    let fsd = MTLSamplerDescriptor(); fsd.minFilter = .linear; fsd.magFilter = .linear; fsd.mipFilter = .linear
    faceSampler = device.makeSamplerState(descriptor: fsd)!
    // Your model, from the pack in the app. Without it there is no you to draw, so say so and stop.
    guard let url = Bundle.main.url(forResource: "everglade2", withExtension: "pack"), let pack = Pack(url),
          let m = Model(device: device, library: lib, color: colorPixelFormat, pack: pack) else {
      FileHandle.standardError.write(Data("Everglade: the app's everglade2.pack is missing or doesn't hold the player's parts. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    model = m
    delegate = self
  }
  required init(coder: NSCoder) { fatalError() }

  func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}
  func draw(in view: MTKView) {
    guard let pass = currentRenderPassDescriptor, let drawable = currentDrawable else { return }
    let now = CACurrentMediaTime(); let dt = Float(now - last); last = now
    render(pass, size: drawableSize, scale: Float(window?.backingScaleFactor ?? 2), dt: dt, wait: false) { $0.present(drawable) }
  }

  // `wait` holds the caller until the GPU has drawn: a shot reads the picture back, the window doesn't.
  func render(_ pass: MTLRenderPassDescriptor, size: CGSize, scale: Float, dt: Float, wait: Bool = true, then: (MTLCommandBuffer) -> Void) {
    time += dt
    let n = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), Float(size.width) / scale, Float(size.height) / scale, scale, dt)
    var u: [Float] = [Float(size.width), Float(size.height), time, 0]
    model?.step(state, dt: dt)
    // The view, the picture's size, the clock and where you stand: the place's uniforms, and the
    // first twenty floats Model.metal's MU.
    var camera = [Float](repeating: 0, count: 24)
    let seen = eg_camera(state, Float(size.width) / scale, Float(size.height) / scale, scale, &camera) == 1
    camera[16] = Float(size.width); camera[17] = Float(size.height); camera[18] = time; camera[19] = Float(eg_place(state))
    let place = self.place
    var you = [Float](repeating: 0, count: 5)
    if eg_you(state, &you) == 1 { camera[20] = you[0]; camera[21] = you[1]; camera[22] = you[2] }
    guard let cmd = queue.makeCommandBuffer() else { return }
    // The place's uniforms (everglade/workshop/gpu/temple.coax): size, clock, mirror flag, the eye
    // and its right, up and forward with the focal length, and where you stand.
    var pu = [Float](repeating: 0, count: 24)
    pu[0] = Float(size.width); pu[1] = Float(size.height); pu[2] = time
    for k in 0..<3 { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = camera[20 + k] }
    pu[19] = camera[15]
    if seen, let device { place.reflect(cmd, device: device, uniforms: pu, width: Int(size.width), height: Int(size.height)) }
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    // The place, you, what is only light, then the scene's text over them, as the phones draw it.
    if seen { place.draw(enc, uniforms: pu); model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
    enc.setDepthStencilState(flat)
    if n > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    // Next issue, over everything: its panel's dark box, then its strokes.
    var box = [Float](repeating: 0, count: 4)
    let tn = nxp_frame(next, overlay.contents().assumingMemoryBound(to: Float.self), &box, Float(size.width) / scale, Float(size.height) / scale, scale, dt, Int64(Date().timeIntervalSince1970))
    serve()
    enc.setDepthStencilState(flat)
    if box[2] > 0 {
      var ru: [Float] = [box[0] * scale, box[1] * scale, box[2] * scale, box[3] * scale, Float(size.width), Float(size.height), 0, 0]
      enc.setRenderPipelineState(panel)
      enc.setVertexBytes(&ru, length: 32, index: 0)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }
    if tn > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(overlay, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(tn))
    }
    enc.endEncoding()
    then(cmd)
    cmd.commit()
    if wait { cmd.waitUntilCompleted() }
  }

  // What the panel asked of the host this frame. The host holds no logic here: it runs the
  // context program the panel asks for and appends the lines the panel hands it.
  func serve() {
    // A request is taken only when the last one's program has answered, so none is dropped.
    let request = fetching ? 0 : nxp_take_request(next)
    if request != 0 {
      fetching = true
      let tool = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/everglade-next")
      let process = Process(), out = Pipe(), err = Pipe()
      process.executableURL = tool
      // The checkout the app was built from is where a worktree for an issue branches off.
      let checkout = (Bundle.main.url(forResource: "repo", withExtension: "txt").flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
      let working = request == 3
      process.arguments = working ? ["work", String(nxp_chosen(next)), "--repo-dir", checkout, "--owner", String(getpid())] : request == 2 ? ["context", "--refresh"] : ["context"]
      // EVERGLADE_NEXT_PROMPT is what you say now, for a run with no one at the window (--shot).
      if !working, let said = ProcessInfo.processInfo.environment["EVERGLADE_NEXT_PROMPT"], !said.isEmpty { process.arguments! += ["--prompt", said] }
      process.standardOutput = out; process.standardError = err
      DispatchQueue.global().async { [weak self] in
        var text = Data(), report = Data(), ok = false
        do {
          try process.run()
          // stderr is a few lines; stdout is read to its end first so a full pipe never stalls the tool.
          text = out.fileHandleForReading.readDataToEndOfFile()
          report = err.fileHandleForReading.readDataToEndOfFile()
          process.waitUntilExit()
          ok = process.terminationStatus == 0
        } catch { report = Data("the app's everglade-next is missing; rebuild with apps/everglade-desktop/build.sh".utf8) }
        DispatchQueue.main.async {
          guard let self else { return }
          self.fetching = false
          text.withUnsafeBytes { t in report.withUnsafeBytes { r in
            let tb = t.bindMemory(to: UInt8.self).baseAddress, rb = r.bindMemory(to: UInt8.self).baseAddress
            if working { nxp_work(self.next, tb, Int32(t.count), rb, Int32(r.count), ok ? 1 : 0) }
            else { nxp_context(self.next, tb, Int32(t.count), rb, Int32(r.count), ok ? 1 : 0) }
          } }
        }
      }
    }
    var line = [UInt8](repeating: 0, count: 4096)
    let home = FileManager.default.homeDirectoryForCurrentUser
    var n = Int(nxp_take_usage(next, &line, Int32(line.count)))
    if n > 0 { append(Data(line[0..<n]), to: home.appendingPathComponent(".openagents/usage/ai-calls.jsonl")) }
    n = Int(nxp_take_decision(next, &line, Int32(line.count)))
    if n > 0 { append(Data(line[0..<n]), to: home.appendingPathComponent(".openagents/everglade/next-issue/decisions.jsonl")) }
  }
  // One line onto the end of a file only its owner can read, its folder made if need be.
  func append(_ line: Data, to url: URL) {
    try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    if !FileManager.default.fileExists(atPath: url.path) { FileManager.default.createFile(atPath: url.path, contents: nil, attributes: [.posixPermissions: 0o600]) }
    guard let h = try? FileHandle(forWritingTo: url) else { return }
    defer { try? h.close() }
    _ = try? h.seekToEnd()
    try? h.write(contentsOf: line + Data([10]))
  }

  // One frame offscreen, after `seconds` of the scene's own time, as a PNG.
  var nextWait: Float = 0
  var nextReply = ""
  var nextRuns: Int32 = 0
  func shot(to path: String, width: Int, height: Int, seconds: Float) -> Bool {
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: colorPixelFormat, width: width, height: height, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    guard let texture = device?.makeTexture(descriptor: d) else { return false }
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = texture; pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
    let zd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
    zd.usage = [.renderTarget]; zd.storageMode = .private
    guard let z = device?.makeTexture(descriptor: zd) else { return false }
    pass.depthAttachment.texture = z; pass.depthAttachment.loadAction = .clear; pass.depthAttachment.clearDepth = 1; pass.depthAttachment.storeAction = .dontCare
    // --next: press the button, then draw in real time until it is your turn (or it failed) or the
    // wait is up; --reply types a reply and waits for the answer to it. The run loop turns so the
    // context program's answer, which arrives on the main queue, is delivered.
    if nextWait > 0 {
      let w = Float(width) / 2, h = Float(height) / 2, bx = w - 24 - 66, by: Float = 24 + 17
      // --autopilot-off presses the Autopilot button first, for the flow where you answer.
      if CommandLine.arguments.contains("--autopilot-off") { _ = nxp_pointer(next, bx - 150, by, 0, w, h); _ = nxp_pointer(next, bx - 150, by, 2, w, h) }
      func wait() {
        let until = CACurrentMediaTime() + Double(nextWait)
        repeat {
          render(pass, size: CGSize(width: width, height: height), scale: 2, dt: 0.05) { _ in }
          RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while CACurrentMediaTime() < until && (nxp_state(next) == 1 || nxp_state(next) == 2) && (nextRuns == 0 || nxp_runs(next) < nextRuns)
      }
      render(pass, size: CGSize(width: width, height: height), scale: 2, dt: 0) { _ in }
      _ = nxp_pointer(next, bx, by, 0, w, h); _ = nxp_pointer(next, bx, by, 2, w, h)
      wait()
      if !nextReply.isEmpty && nxp_state(next) == 3 {
        for c in nextReply.unicodeScalars where c.isASCII && c.value >= 32 && c.value <= 126 { nxp_key(next, Int32(c.value)) }
        nxp_key(next, 13)
        wait()
      }
      FileHandle.standardError.write(Data("next issue state \(nxp_state(next))\n".utf8))
    }
    var left = seconds
    while left > 0 { let step = min(left, 0.05); render(pass, size: CGSize(width: width, height: height), scale: 2, dt: step) { _ in }; left -= step }
    render(pass, size: CGSize(width: width, height: height), scale: 2, dt: 0) { _ in }
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    texture.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
    let info = CGImageAlphaInfo.noneSkipFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
    guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info),
          let image = ctx.makeImage(), let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { return false }
    nxp_close(next)
    return (try? png.write(to: URL(fileURLWithPath: path))) != nil
  }

  override var acceptsFirstResponder: Bool { true }
  func point(_ e: NSEvent) -> (Float, Float) { let p = convert(e.locationInWindow, from: nil); return (Float(p.x), Float(bounds.height - p.y)) }
  // The Next issue button and panel take a press first; the world gets what they leave.
  func press(_ e: NSEvent, _ phase: Int32) {
    let p = point(e)
    if nxp_pointer(next, p.0, p.1, phase, Float(bounds.width), Float(bounds.height)) == 0 { eg_pointer(state, p.0, p.1, phase) }
  }
  override func mouseDown(with e: NSEvent) { press(e, 0) }
  override func mouseDragged(with e: NSEvent) { press(e, 1) }
  override func mouseUp(with e: NSEvent) { press(e, 2) }
  // Keys go to Coax in the web realms' codes; Cmd chords stay with the menu.
  func code(_ e: NSEvent) -> Int32? {
    switch e.keyCode { case 123: return -4; case 124: return -5; case 126: return -6; case 125: return -7; case 49: return 32; default: break }
    guard let c = e.charactersIgnoringModifiers?.lowercased().unicodeScalars.first, c.isASCII else { return nil }
    return Int32(c.value)
  }
  override func keyDown(with e: NSEvent) {
    if e.modifierFlags.contains(.command) { super.keyDown(with: e); return }
    // While the reply has the keys they are typing, not walking.
    if nxp_wants_keys(next) == 1 {
      switch e.keyCode {
      case 36, 76: nxp_key(next, 13)
      case 51: nxp_key(next, 8)
      case 53: nxp_key(next, 27)
      default: for c in (e.characters ?? "").unicodeScalars where c.isASCII && c.value >= 32 && c.value <= 126 { nxp_key(next, Int32(c.value)) }
      }
      _ = eg_key(state, -1, 1)
      return
    }
    if let k = code(e) { _ = eg_key(state, k, 1) }
  }
  override func keyUp(with e: NSEvent) { if let k = code(e) { _ = eg_key(state, k, 0) } }
  override func flagsChanged(with e: NSEvent) { _ = eg_key(state, -13, e.modifierFlags.contains(.shift) ? 1 : 0) }
  override func resignFirstResponder() -> Bool { _ = eg_key(state, -1, 1); return super.resignFirstResponder() }
  override func scrollWheel(with e: NSEvent) {
    let p = point(e), dy = Float(e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 10)
    if nxp_wheel(next, p.0, p.1, dy, Float(bounds.width), Float(bounds.height)) == 1 { return }
    eg_wheel(state, dy)
  }
}

final class App: NSObject, NSApplicationDelegate {
  var window: NSWindow!
  func applicationDidFinishLaunching(_ note: Notification) {
    let frame = NSRect(x: 0, y: 0, width: 1320, height: 880)
    window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
    window.title = "Everglade"
    window.minSize = NSSize(width: 900, height: 600)
    let view = WorkshopView(frame: frame)
    if let w = CommandLine.arguments.firstIndex(of: "--scene"), w + 1 < CommandLine.arguments.count, let v = Int32(CommandLine.arguments[w + 1]) { eg_go(view.state, v) }
    window.contentView = view
    window.center()
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(window.contentView)
    NSApp.activate(ignoringOtherApps: true)
  }
  func applicationDidResignActive(_ note: Notification) { if let v = window.contentView as? WorkshopView { _ = eg_key(v.state, -1, 1) } }
  func applicationWillTerminate(_ note: Notification) { if let v = window.contentView as? WorkshopView { nxp_close(v.next) } }
  func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}

func menus() {
  let bar = NSMenu(), item = NSMenuItem(), menu = NSMenu()
  menu.addItem(withTitle: "Quit Everglade", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
  item.submenu = menu; bar.addItem(item); NSApp.mainMenu = bar
}

// Opened from the Finder the app gets a bare PATH; the agent's `gh` and a Homebrew or user install
// of `claude` live in these, so they are added once, for this process and its children.
do {
  let home = NSHomeDirectory(), have = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin"
  setenv("PATH", have + ":/opt/homebrew/bin:/usr/local/bin:\(home)/.local/bin", 1)
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
  if let w = args.firstIndex(of: "--next"), w + 1 < args.count, let v = Float(args[w + 1]), v > 0, v <= 86400 { view.nextWait = v }
  if let r = args.firstIndex(of: "--reply"), r + 1 < args.count { view.nextReply = args[r + 1] }
  if let r = args.firstIndex(of: "--runs"), r + 1 < args.count, let v = Int32(args[r + 1]), v > 0, v <= 16 { view.nextRuns = v }
  if let w = args.firstIndex(of: "--scene"), w + 1 < args.count, let v = Int32(args[w + 1]) { eg_go(view.state, v) }
  if let h = args.firstIndex(of: "--hold"), h + 1 < args.count {
    for c in args[h + 1].lowercased().unicodeScalars where c.isASCII { _ = eg_key(view.state, Int32(c.value), 1) }
  }
  exit(view.shot(to: args[i + 1], width: width, height: height, seconds: seconds) ? 0 : 1)
}
let app = NSApplication.shared
let delegate = App()
app.delegate = delegate
app.setActivationPolicy(.regular)
menus()
app.run()
