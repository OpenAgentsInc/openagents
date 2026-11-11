// Firm reason: 6 (AppKit and Metal have no C API; this host holds no product logic).
// Everglade for desktop, the Mac host (everglade/docs/desktop-app-spec.md): a window,
// a Metal view and input. Everything drawn comes from scenes.coax (the list of worlds, the
// Workshop) through face.h. Escape is the list; --scene N opens on world N (1 Everglade, 2 the
// Workshop, 3 the Archery Range, 4 Experimental). Everglade itself is the web's everglade2 realm
// (Character Create, then the glade), and the Archery Range and Experimental are the web's fivelab
// realm on their own pages, each run natively by Aethyr's page host (aethyr/host/apple/
// page-host.h; the rows are Realms.swift's, shared with the iOS host): while one is open this host
// hands it the frames, keys and presses, draws its picture under the Worlds button, and plays its
// sound through the Apple audio host (aethyr/host/apple/PageAudio.swift).
//   Everglade                      opens the window
//   Everglade --shot OUT.png [--size WxH] [--at SECONDS] [--hold KEYS] [--scene N]
//       draws one frame offscreen after SECONDS with KEYS held (letters, as "wd"), and exits;
//       without --scene it is the list of worlds. In a realm (--scene 1, 3 or 4) nothing leaves this
//       Mac during a shot unless --online is given (the room is then the site's, as a guest's
//       browser would join it), nothing of yours is kept, and --do STEPS plays input first: `key:w:2` holds a key two seconds,
//       `tap:640,400` clicks a point, `text:Ada` types, `wait:1` waits, `look` holds the pointer as
//       mouselook asks, `press:2:1` holds a mouse button a second at the middle, `turn:40,0` moves
//       the held mouse, `down:2` and `up:2` hold and let go a button around other steps, `drive:walk-to guard` asks the realm's agent driver, `near:guard:20:60` waits until the driver lists ID within METRES (SECONDS at most); steps are split by `;`.
//       A realm's sound is rendered offline in the shot's time, never played: stderr says what it
//       did (the packets, the voices, the names played, the loudest sample), --audio OUT.wav writes it.
//       --quality TIER starts the realm with that graphics tier kept (auto when none is).
//       --scale S draws S pixels a point (2 unless given); --realtime gives each frame the time it took. --frames DIR writes every sixth frame after `record` (--every N: every Nth)
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
  var temple: Place!
  // The place behind the world that is open (face.h eg_place).
  var place: Place { temple }
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
  // The realms, each once its row is first opened, by its world's number. `pageOffline` is a
  // shot's: no socket and no request leaves this Mac.
  var pages: [Int32: AEPageHost] = [:]
  var sounds: [Int32: AEPageAudio] = [:]   // each open realm's sound
  var pageTried: Set<Int32> = []
  var pageOffline = false, pageShot = false
  var shownWorld: Int32 = 0           // the world whose realm is on screen; 0 when none is
  var lookHeld = false, worldsPress = false
  weak var lookPage: AEPageHost?      // the realm the pointer is held for
  var shotStore = NSTemporaryDirectory() + "everglade-shot-\(getpid())"
  // The realm of the world that is open, when it has one.
  var page: AEPageHost? { pages[eg_scene(state)] }
  var inRealm: Bool { page != nil }
  // A realm's folder in the app (its library, shaders and bindings), when the app was built with it.
  static func realmFiles(_ world: Int32) -> URL? {
    guard let r = Realms.table[world], let dir = Bundle.main.url(forResource: r.module, withExtension: nil), let res = Bundle.main.resourceURL,
          FileManager.default.fileExists(atPath: dir.appendingPathComponent("lib\(r.module).dylib").path),
          FileManager.default.fileExists(atPath: res.appendingPathComponent(r.pack).path) else { return nil }
    return dir
  }

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
    guard let pl = Place(device: device, color: colorPixelFormat) else {
      FileHandle.standardError.write(Data("Everglade: the temple did not build or its shaders are missing. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    temple = pl
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
    // A realm's files are in the app: its row opens a world, not a card.
    for world in Realms.table.keys where WorkshopView.realmFiles(world) != nil { eg_runs(state, world, 1) }
    delegate = self
  }

  // Opens a world's realm: its library, shaders, pack, fonts and pictures from the app, and what a
  // browser would keep in its storage under ~/.openagents/everglade/realm/ (one folder for every
  // realm, as one site's pages share a browser's storage: your character is the same in each).
  func openPage(_ world: Int32) {
    pageTried.insert(world)
    // A module runs one page at a time (Realms.sharing): the Archery Range and Experimental are
    // both fivelab, so opening one closes the other, which opens afresh from its save slot.
    for other in Realms.sharing(world) where pages[other] != nil { closePage(other) }
    guard let device, let r = Realms.table[world], let dir = WorkshopView.realmFiles(world), let res = Bundle.main.resourceURL else { eg_runs(state, world, 0); return }
    let c = AEPageConfig()
    c.producer = dir.appendingPathComponent("lib\(r.module).dylib").path; c.shaders = dir.path
    c.pack = res.appendingPathComponent(r.pack).path
    c.fonts = res.appendingPathComponent("fonts").path; c.reading = "Spectral-Medium"; c.title = "Cinzel"; c.titleWeight = 600
    c.web = res.appendingPathComponent("web").path
    // A shot keeps nothing of yours: its storage is a folder of its own, gone when it ends.
    c.store = pageShot ? shotStore : FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/everglade/realm").path
    // The page the web opens this world on. Everglade's room is the site's (wss://openagents.com/api/room/);
    // Experimental has no room on the web either.
    c.origin = "https://openagents.com"; c.path = r.path + (shotRoom.isEmpty ? "" : "?room=" + shotRoom)
    c.realm = r.realm; c.slot = r.slot; c.style = "painterly"; c.clock = "fixed:0.5"
    c.offline = pageOffline
    // This app is built from a checkout, so who runs it is the team: its sessions are marked as a
    // driven browser's are, and never count as visitors.
    c.automated = true
    var why: NSString?
    pages[world] = AEPageHost.open(c, device: device, queue: queue, why: &why)
    if let page = pages[world] { sounds[world] = AEPageAudio(page: page, resources: res, offline: pageShot) }
    if pages[world] == nil {
      FileHandle.standardError.write(Data("Everglade: the realm did not open: \(why ?? "unknown"). Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      eg_runs(state, world, 0)
    }
  }
  func closePage(_ world: Int32) {
    if shownWorld == world { hold(false); shownWorld = 0 }
    if pageShot, let sound = sounds[world] { FileHandle.standardError.write(Data("Everglade: world \(world) \(sound.summary())\n".utf8)) }
    sounds[world]?.close(); sounds[world] = nil
    pages[world]?.close(); pages[world] = nil; pageTried.remove(world)
  }
  // A key or a press in the realm on screen: the gesture its sound starts on.
  func heard() { sounds[eg_scene(state)]?.gesture() }
  // Mouselook: the realm says when it wants the pointer held; the host holds it and says what it did.
  func hold(_ on: Bool) {
    if on == lookHeld { return }
    lookHeld = on
    CGAssociateMouseAndMouseCursorPosition(on ? 0 : 1)
    if on { NSCursor.hide() } else { NSCursor.unhide() }
    if on { lookPage = page }
    lookPage?.pointerLock(on ? 1 : 0)
    if !on { lookPage = nil }
  }
  func syncRealm() {
    guard let page, inRealm else { hold(false); return }
    let want = page.mouselook()
    if want == 2 && !lookHeld && window?.isKeyWindow == true { hold(true) } else if want != 2 && lookHeld { hold(false) }
    if !lookHeld {
      switch page.cursor() { case 1: NSCursor.pointingHand.set(); case 2: NSCursor.crosshair.set(); case 3: NSCursor.iBeam.set(); default: NSCursor.arrow.set() }
    }
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
    // A realm's row, first opened: the realm starts. Leaving it lets its keys go; it keeps its state.
    let world = eg_scene(state)
    if Realms.table[world] != nil && pages[world] == nil && !pageTried.contains(world) { openPage(world) }
    let on: Int32 = pages[world] != nil ? world : 0
    if on != shownWorld { hold(false); pages[shownWorld]?.shown(false); sounds[shownWorld]?.show(false); pages[on]?.shown(true); sounds[on]?.show(true); shownWorld = on }
    let realm = page
    for (w, p) in pages where w != on { p.idle(); sounds[w]?.step(dt) }   // out of sight a realm draws nothing, and still hears its room
    if let realm { _ = realm.frame(CGSize(width: size.width / CGFloat(scale), height: size.height / CGFloat(scale)), scale: CGFloat(scale), dt: dt, wait: wait); sounds[on]?.step(dt) }
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
    if seen, realm == nil, let device { place.reflect(cmd, device: device, uniforms: pu, width: Int(size.width), height: Int(size.height)) }
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    // The realm's picture; or the place, you, what is only light. Then the scene's text over them, as the phones draw it.
    if let realm { enc.setDepthStencilState(flat); realm.present(enc, format: colorPixelFormat) }
    else if seen { place.draw(enc, uniforms: pu); model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
    enc.setDepthStencilState(flat)
    if n > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    // Next issue, over everything: its panel's dark box, then its strokes.
    var box = [Float](repeating: 0, count: 8)
    let tn = nxp_frame(next, overlay.contents().assumingMemoryBound(to: Float.self), &box, Float(size.width) / scale, Float(size.height) / scale, scale, dt, Int64(Date().timeIntervalSince1970))
    serve()
    enc.setDepthStencilState(flat)
    // Over the realm the agent's buttons and panels are not drawn: the realm's own HUD is there.
    // The agent keeps working; its buttons are back on the list and in the other worlds.
    for k in [0, 4] where box[k + 2] > 0 && realm == nil {
      var ru: [Float] = [box[k] * scale, box[k + 1] * scale, box[k + 2] * scale, box[k + 3] * scale, Float(size.width), Float(size.height), 0, 0]
      enc.setRenderPipelineState(panel)
      enc.setVertexBytes(&ru, length: 32, index: 0)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }
    if tn > 0 && realm == nil {
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
    if !wait { syncRealm() }
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
    // The Golems board: every agent's transcript, read by everglade-golems off the main thread.
    let now = Int64(Date().timeIntervalSince1970)
    if !readingGolems && nxp_golems_due(next, now) != 0 {
      readingGolems = true
      let tool = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/everglade-golems")
      DispatchQueue.global().async { [weak self] in
        let process = Process(), out = Pipe()
        process.executableURL = tool; process.arguments = ["snapshot", "--now", String(now), "--tz", String(TimeZone.current.secondsFromGMT())]
        process.standardOutput = out; process.standardError = FileHandle.nullDevice
        var text = Data()
        if (try? process.run()) != nil { text = out.fileHandleForReading.readDataToEndOfFile(); process.waitUntilExit() }
        DispatchQueue.main.async {
          guard let self else { return }
          self.readingGolems = false
          text.withUnsafeBytes { t in nxp_golems(self.next, t.bindMemory(to: UInt8.self).baseAddress, Int32(t.count), Int64(Date().timeIntervalSince1970)) }
        }
      }
    }
    // The slow things the board shows (GitHub's issues, the build hosts) are fetched into files by
    // the same program, at most every two minutes while the board is open.
    if !refreshingGolems && nxp_golems_refresh_due(next, now) != 0 {
      refreshingGolems = true
      let tool = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/everglade-golems")
      let checkout = (Bundle.main.url(forResource: "repo", withExtension: "txt").flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
      DispatchQueue.global().async { [weak self] in
        let process = Process()
        process.executableURL = tool
        process.arguments = ["refresh", "--now", String(now), "--tz", String(TimeZone.current.secondsFromGMT()), "--repo-dir", checkout]
        process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
        if (try? process.run()) != nil { process.waitUntilExit() }
        DispatchQueue.main.async { self?.refreshingGolems = false }
      }
    }
    var line = [UInt8](repeating: 0, count: 4096)
    // What a press on the board asked of the Mac: a page in the browser, or text on the clipboard.
    let asked = Int(nxp_golems_action(next, &line, Int32(line.count)))
    if asked > 0, let text = String(bytes: line[0..<asked], encoding: .utf8), let tab = text.firstIndex(of: "\t") {
      let verb = text[..<tab], rest = String(text[text.index(after: tab)...])
      if verb == "open", let url = URL(string: rest), url.scheme == "https" { NSWorkspace.shared.open(url) }
      if verb == "copy" { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(rest, forType: .string) }
    }
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
  var readingGolems = false
  var refreshingGolems = false
  var golemsView: Int32 = 0    // --golems-view in a shot: 1 the first agent, 2 narrow, 3 narrow with the queue
  var golemsWait: Double = 0   // --golems in a shot: seconds to let the board read the agents first
  var nextReply = ""
  var nextRuns: Int32 = 0
  var framesEvery = 6
  var shotRoom = ""   // --room CODE: a shot's own room on the site (a check's pair-e2-NAME), apart from everyone's
  var shotScale: Float = 2, shotRealtime = false, lastShotFrame = CACurrentMediaTime()
  var doSteps = "", framesDir = "", shotAudio = "", shotLooks = false, recording = false
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
          render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: 0.05) { _ in }
          RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while CACurrentMediaTime() < until && (nxp_state(next) == 1 || nxp_state(next) == 2) && (nextRuns == 0 || nxp_runs(next) < nextRuns)
      }
      render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: 0) { _ in }
      _ = nxp_pointer(next, bx, by, 0, w, h); _ = nxp_pointer(next, bx, by, 2, w, h)
      wait()
      if !nextReply.isEmpty && nxp_state(next) == 3 {
        for c in nextReply.unicodeScalars where c.isASCII && c.value >= 32 && c.value <= 126 { nxp_key(next, Int32(c.value)) }
        nxp_key(next, 13)
        wait()
      }
      FileHandle.standardError.write(Data("next issue state \(nxp_state(next))\n".utf8))
    }
    if golemsWait > 0 {
      let until = CACurrentMediaTime() + golemsWait
      // The issues and hosts first (a fresh copy is a few seconds; an old one is left), then the agents.
      repeat { render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: 0.05) { _ in }; RunLoop.current.run(until: Date().addingTimeInterval(0.05)) } while CACurrentMediaTime() < until + 24 && refreshingGolems
      let read = CACurrentMediaTime()
      repeat { render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: 0.05) { _ in }; RunLoop.current.run(until: Date().addingTimeInterval(0.05)) } while CACurrentMediaTime() < read + golemsWait && readingGolems || CACurrentMediaTime() < read + 4.5
      if golemsView != 0 { nxp_golems_view(next, golemsView) }
    }
    // In a shot nobody holds the pointer, so the harness answers the realm's mouselook as a held
    // pointer would (--do look), and writes a numbered frame every few when --frames DIR is given.
    var frameCount = 0
    func run(_ time: Float) {
      var left = time
      while left > 0 {
        // --realtime: a frame takes the time it really took, so the realm keeps pace with the site's room
        let since = Float(CACurrentMediaTime() - lastShotFrame); lastShotFrame = CACurrentMediaTime()
        let step = shotRealtime && inRealm ? min(max(since, 1.0 / 240), 0.1) : min(left, inRealm ? 1.0 / 60 : 0.05); render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: step) { _ in }; left -= step
        if let page, inRealm, shotLooks { let want = page.mouselook(); if want == 2 && !lookHeld { lookHeld = true; page.pointerLock(1) } else if want != 2 && lookHeld { lookHeld = false; page.pointerLock(0) } }
        frameCount += 1
        if !framesDir.isEmpty && recording && frameCount % framesEvery == 0 { _ = save(texture, width, height, framesDir + String(format: "/f%04d.png", frameCount)) }
      }
    }
    run(seconds)
    // --do: input for the realm, a step at a time, each followed by the frames it takes.
    for step in doSteps.split(separator: ";") {
      let part = step.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
      guard let page, inRealm, let verb = part.first else { continue }
      if verb == "wait", part.count > 1, let t = Float(part[1]) { run(t) }
      else if verb == "key", part.count > 1 {
        let named: [String: Int32] = ["escape": -1, "enter": -2, "backspace": -3, "left": -4, "right": -5, "up": -6, "down": -7, "shift": -13, "tab": -15, "space": 32]
        let code = named[part[1]] ?? Int32(part[1].unicodeScalars.first?.value ?? 0)
        _ = page.key(code, mods: 0, down: true); run(part.count > 2 ? Float(part[2]) ?? 0.1 : 0.1); _ = page.key(code, mods: 0, down: false); run(1.0 / 60)
      }
      else if verb == "tap", part.count > 1 {
        let xy = part[1].split(separator: ",").compactMap { Double($0) }
        if xy.count == 2 { let at = CGPoint(x: xy[0], y: xy[1]); page.pointer(at, phase: 1, button: 0); run(1.0 / 60); page.pointer(at, phase: 0, button: 0); run(1.0 / 60); page.pointer(at, phase: 2, button: 0); run(0.1) }
      }
      else if verb == "text", part.count > 1 { page.text(part[1]); run(0.1) }
      else if verb == "look" { shotLooks = true; run(0.2) }
      else if verb == "drive", part.count > 1 {   // the realm's agent driver; its answer goes to stderr
        let said = page.drive(part[1...].joined(separator: ":")) ?? "this realm has no driver"
        FileHandle.standardError.write(Data("drive \(part[1]): \(said)\n".utf8)); run(1.0 / 60)
      }
      else if verb == "near", part.count > 2, case let rest = part[2].split(separator: ":"), rest.count == 2, let metres = Double(rest[0]), let most = Float(rest[1]) {   // waits until the driver's tree has ID within METRES, SECONDS at most
        var waited: Float = 0, found = false
        while waited < most && !found {
          for line in (page.drive("tree") ?? "").split(separator: "\n") where line.hasPrefix(part[1] + " ") {
            if let m = line.range(of: #", ([0-9.]+) m"#, options: .regularExpression), let d = Double(line[m].dropFirst(2).dropLast(2)), d <= metres { found = true; FileHandle.standardError.write(Data("near: \(line)\n".utf8)) }
          }
          if !found { run(0.25); waited += 0.25 }
        }
        if !found { FileHandle.standardError.write(Data("near: \(part[1]) never came within \(metres) m\n".utf8)) }
      }
      else if verb == "turn", part.count > 1 { let xy = part[1].split(separator: ",").compactMap { Double($0) }; if xy.count == 2 { page.look(CGPoint(x: xy[0], y: xy[1])); run(0.1) } }
      else if verb == "record" { recording = true }
      else if (verb == "down" || verb == "up"), part.count > 1, let b = Int32(part[1]) {   // a mouse button at the middle, held until its `up`: `down:2;turn:40,0;up:2` aims a draw
        page.pointer(CGPoint(x: CGFloat(width) / CGFloat(shotScale * 2), y: CGFloat(height) / CGFloat(shotScale * 2)), phase: verb == "down" ? 0 : 2, button: b); run(1.0 / 60)
      }
      else if verb == "press", part.count > 1, let b = Int32(part[1]) {   // a mouse button at the middle of the view, held SECONDS
        let mid = CGPoint(x: CGFloat(width) / CGFloat(shotScale * 2), y: CGFloat(height) / CGFloat(shotScale * 2))
        page.pointer(mid, phase: 0, button: b); run(part.count > 2 ? Float(part[2]) ?? 0.05 : 0.05); page.pointer(mid, phase: 2, button: b); run(1.0 / 60)
      }
    }
    render(pass, size: CGSize(width: width, height: height), scale: shotScale, dt: 0) { _ in }
    nxp_close(next)
    if !shotAudio.isEmpty, let sound = sounds[eg_scene(state)], !sound.write(shotAudio) { FileHandle.standardError.write(Data("Everglade: couldn't write \(shotAudio)\n".utf8)) }
    for world in Array(pages.keys) { closePage(world) }; try? FileManager.default.removeItem(atPath: shotStore)
    return save(texture, width, height, path)
  }
  func save(_ texture: MTLTexture, _ width: Int, _ height: Int, _ path: String) -> Bool {
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    texture.getBytes(&bytes, bytesPerRow: width * 4, from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
    let info = CGImageAlphaInfo.noneSkipFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
    guard let ctx = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info),
          let image = ctx.makeImage(), let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { return false }
    return (try? png.write(to: URL(fileURLWithPath: path))) != nil
  }

  override var acceptsFirstResponder: Bool { true }
  func point(_ e: NSEvent) -> (Float, Float) { let p = convert(e.locationInWindow, from: nil); return (Float(p.x), Float(bounds.height - p.y)) }
  // The Next issue button and panel take a press first; the world gets what they leave.
  func press(_ e: NSEvent, _ phase: Int32, button: Int32 = 0) {
    let p = point(e)
    guard let page, inRealm else {
      if button == 0 && nxp_pointer(next, p.0, p.1, phase, Float(bounds.width), Float(bounds.height)) == 0 { eg_pointer(state, p.0, p.1, phase) }
      return
    }
    // In the realm: a press that begins on the Worlds button is the list's; every other is the realm's.
    if button == 0 && !lookHeld {
      if phase == 0 { worldsPress = eg_on_worlds(state, p.0, p.1) == 1 }
      if worldsPress { eg_pointer(state, p.0, p.1, phase); if phase == 2 { worldsPress = false }; return }
    }
    if phase == 0 { heard() }   // a press is the gesture the realm's sound starts on, before the realm hears it
    // With the pointer held, a move only turns the camera and a press is at the middle of the view.
    if lookHeld {
      if phase == 1 { page.look(CGPoint(x: e.deltaX, y: e.deltaY)); return }
      page.pointer(CGPoint(x: bounds.width / 2, y: bounds.height / 2), phase: phase, button: button)
    } else { page.pointer(CGPoint(x: CGFloat(p.0), y: CGFloat(p.1)), phase: phase, button: button) }
    syncRealm()
  }
  override func mouseDown(with e: NSEvent) { press(e, 0) }
  override func mouseDragged(with e: NSEvent) { press(e, 1) }
  override func mouseUp(with e: NSEvent) { press(e, 2) }
  override func mouseMoved(with e: NSEvent) { if inRealm { press(e, 1) } }
  override func rightMouseDown(with e: NSEvent) { if inRealm { press(e, 0, button: 2) } }
  override func rightMouseDragged(with e: NSEvent) { if inRealm { press(e, 1, button: 2) } }
  override func rightMouseUp(with e: NSEvent) { if inRealm { press(e, 2, button: 2) } }
  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    for a in trackingAreas { removeTrackingArea(a) }
    addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: self, userInfo: nil))
  }
  // A key for the realm, in the host contract's codes (aethyr/host/README.md): a named key's
  // negative number, else the character it types.
  func realmKey(_ e: NSEvent) -> Int32? {
    switch e.keyCode {
    case 53: return -1; case 36, 76: return -2; case 51: return -3; case 123: return -4; case 124: return -5; case 126: return -6; case 125: return -7
    case 115: return -8; case 119: return -9; case 116: return -10; case 121: return -11; case 117: return -12; case 48: return -15
    default: break
    }
    guard let c = e.charactersIgnoringModifiers?.unicodeScalars.first, c.value >= 32, c.value != 127, !(0xF700...0xF8FF).contains(c.value) else { return nil }
    return Int32(c.value)
  }
  func mods(_ e: NSEvent) -> Int32 {
    let f = e.modifierFlags
    return (f.contains(.shift) ? 1 : 0) | (f.contains(.control) ? 2 : 0) | (f.contains(.option) ? 4 : 0) | (f.contains(.command) ? 8 : 0)
  }
  var heldMods: NSEvent.ModifierFlags = []
  // Keys go to Coax in the web realms' codes; Cmd chords stay with the menu.
  func code(_ e: NSEvent) -> Int32? {
    switch e.keyCode { case 123: return -4; case 124: return -5; case 126: return -6; case 125: return -7; case 49: return 32; default: break }
    guard let c = e.charactersIgnoringModifiers?.lowercased().unicodeScalars.first, c.isASCII else { return nil }
    return Int32(c.value)
  }
  override func keyDown(with e: NSEvent) {
    if e.modifierFlags.contains(.command) { super.keyDown(with: e); return }
    // While the reply has the keys they are typing, not walking.
    if nxp_wants_keys(next) == 1 && !inRealm {
      switch e.keyCode {
      case 36, 76: nxp_key(next, 13)
      case 51: nxp_key(next, 8)
      case 53: nxp_key(next, 27)
      default: for c in (e.characters ?? "").unicodeScalars where c.isASCII && c.value >= 32 && c.value <= 126 { nxp_key(next, Int32(c.value)) }
      }
      _ = eg_key(state, -1, 1)
      if inRealm { page?.focus(false); page?.focus(true) }
      return
    }
    if let page, inRealm {
      // Escape with the pointer held lets it go, as a browser does, and is not the realm's key.
      if e.keyCode == 53 && lookHeld { hold(false); return }
      guard let k = realmKey(e) else { return }
      heard()
      // While the realm takes text, what you type arrives as text; a named key is still a key.
      if k >= 32 && page.wantsText() && !e.modifierFlags.contains(.control) { if let t = e.characters, !t.isEmpty { page.text(t) } }
      else { _ = page.key(k, mods: mods(e), down: true) }
      syncRealm()
      return
    }
    if let k = code(e) { _ = eg_key(state, k, 1) }
  }
  override func keyUp(with e: NSEvent) {
    if let page, inRealm { if let k = realmKey(e) { _ = page.key(k, mods: mods(e), down: false) }; return }
    if let k = code(e) { _ = eg_key(state, k, 0) }
  }
  override func flagsChanged(with e: NSEvent) {
    if let page, inRealm {
      let now = e.modifierFlags.intersection([.shift, .control, .option])
      for (flag, key) in [(NSEvent.ModifierFlags.shift, Int32(-13)), (.control, -14), (.option, -16)] where now.contains(flag) != heldMods.contains(flag) { _ = page.key(key, mods: mods(e), down: now.contains(flag)) }
      heldMods = now; return
    }
    _ = eg_key(state, -13, e.modifierFlags.contains(.shift) ? 1 : 0)
  }
  override func resignFirstResponder() -> Bool { _ = eg_key(state, -1, 1); hold(false); if inRealm { page?.focus(false) }; return super.resignFirstResponder() }
  override func becomeFirstResponder() -> Bool { if inRealm { page?.focus(true) }; return super.becomeFirstResponder() }
  override func scrollWheel(with e: NSEvent) {
    let p = point(e), dy = Float(e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 10)
    if let page, inRealm { page.wheel(hmi_appkit_wheel(Float(e.scrollingDeltaY), e.hasPreciseScrollingDeltas)); return }
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
    if CommandLine.arguments.contains("--golems") { nxp_golems_open(view.next) }
    window.contentView = view
    window.center()
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(window.contentView)
    NSApp.activate(ignoringOtherApps: true)
  }
  func applicationDidResignActive(_ note: Notification) { if let v = window.contentView as? WorkshopView { _ = eg_key(v.state, -1, 1); v.hold(false); if v.inRealm { v.page?.focus(false) } } }
  func applicationDidBecomeActive(_ note: Notification) { if let v = window?.contentView as? WorkshopView, v.inRealm { v.page?.focus(true) } }
  func applicationWillTerminate(_ note: Notification) { if let v = window.contentView as? WorkshopView { v.hold(false); for world in Array(v.pages.keys) { v.closePage(world) }; nxp_close(v.next) } }
  // The list of worlds from anywhere: in the realm Escape is the realm's own key.
  @objc func worlds(_ sender: Any?) { if let v = window.contentView as? WorkshopView { eg_go(v.state, 0) } }
  func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}

func menus() {
  let bar = NSMenu(), item = NSMenuItem(), menu = NSMenu()
  menu.addItem(withTitle: "Worlds", action: #selector(App.worlds(_:)), keyEquivalent: "0")
  menu.addItem(NSMenuItem.separator())
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
  // A headless run (a shot, an Autopilot run) never shows in the Dock or the menu bar.
  NSApplication.shared.setActivationPolicy(.prohibited)
  var width = 2640, height = 1760, seconds: Float = 2
  if let s = args.firstIndex(of: "--size"), s + 1 < args.count {
    let parts = args[s + 1].split(separator: "x").compactMap { Int($0) }
    if parts.count == 2, parts[0] > 0, parts[1] > 0, parts[0] <= 8192, parts[1] <= 8192 { width = parts[0]; height = parts[1] }
  }
  if let t = args.firstIndex(of: "--at"), t + 1 < args.count, let v = Float(args[t + 1]), v >= 0, v <= 600 { seconds = v }
  var shotScale: Float = 2
  if let c = args.firstIndex(of: "--scale"), c + 1 < args.count, let v = Float(args[c + 1]), v >= 1, v <= 3 { shotScale = v }
  let view = WorkshopView(frame: NSRect(x: 0, y: 0, width: CGFloat(width) / CGFloat(shotScale), height: CGFloat(height) / CGFloat(shotScale)))
  if let r = args.firstIndex(of: "--room"), r + 1 < args.count, args[r + 1].allSatisfy({ $0.isLetter || $0.isNumber || $0 == "-" }) { view.shotRoom = args[r + 1] }
  if let r = args.firstIndex(of: "--every"), r + 1 < args.count, let v = Int(args[r + 1]), v >= 1, v <= 60 { view.framesEvery = v }
  view.shotScale = shotScale; view.shotRealtime = args.contains("--realtime")
  if let w = args.firstIndex(of: "--next"), w + 1 < args.count, let v = Float(args[w + 1]), v > 0, v <= 86400 { view.nextWait = v }
  if let r = args.firstIndex(of: "--reply"), r + 1 < args.count { view.nextReply = args[r + 1] }
  if let r = args.firstIndex(of: "--runs"), r + 1 < args.count, let v = Int32(args[r + 1]), v > 0, v <= 16 { view.nextRuns = v }
  view.pageShot = true; view.pageOffline = !args.contains("--online")
  if let d = args.firstIndex(of: "--do"), d + 1 < args.count { view.doSteps = args[d + 1] }
  if let d = args.firstIndex(of: "--frames"), d + 1 < args.count { view.framesDir = args[d + 1] }
  if let d = args.firstIndex(of: "--audio"), d + 1 < args.count { view.shotAudio = args[d + 1] }
  // --quality high|medium|low|auto: the graphics tier the shot's realm finds kept, as a browser that picked it would
  if let d = args.firstIndex(of: "--quality"), d + 1 < args.count, ["high", "medium", "low", "auto"].contains(args[d + 1]) {
    try? FileManager.default.createDirectory(atPath: view.shotStore, withIntermediateDirectories: true)
    try? args[d + 1].write(toFile: view.shotStore + "/verse.quality", atomically: true, encoding: .utf8)
  }
  if let w = args.firstIndex(of: "--scene"), w + 1 < args.count, let v = Int32(args[w + 1]) { eg_go(view.state, v) }
  if let h = args.firstIndex(of: "--hold"), h + 1 < args.count {
    for c in args[h + 1].lowercased().unicodeScalars where c.isASCII { _ = eg_key(view.state, Int32(c.value), 1) }
  }
  if args.contains("--golems") { nxp_golems_open(view.next); view.golemsWait = 6 }
  if let g = args.firstIndex(of: "--golems-view"), g + 1 < args.count, let v = Int32(args[g + 1]) { view.golemsView = v }
  exit(view.shot(to: args[i + 1], width: width, height: height, seconds: seconds) ? 0 : 1)
}
let app = NSApplication.shared
let delegate = App()
app.delegate = delegate
app.setActivationPolicy(.regular)
menus()
app.run()
