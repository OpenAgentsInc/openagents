// Firm reason: 6 (Metal's own passes for the place, the model and the realms; this file holds no product logic).
// Everglade for desktop on the shared Mac host (everglade/docs/desktop-app-spec.md; apps/shared/host/apple/
// ui-host.m, the ui-host.h contract OpenAgents uses, GitHub #13156): the host owns the window, the
// event loop, the input and the offscreen pictures; this file is the app's callbacks and its Metal
// layer (ui-host-apple.h ui_layer). Everything drawn comes from scenes.coax (the list of worlds, the
// Workshop) through face.h. Escape is the list; --scene N opens on world N (1 Everglade, 2 the
// Workshop, 3 the Archery Range, 4 Experimental). Everglade itself is the web's everglade2 realm
// (Character Create, then the glade), and the Archery Range and Experimental are the web's
// archery-range and fivelab realms, each run natively by Aethyr's page host (aethyr/host/apple/
// page-host.h; the rows are Realms.swift's, shared with the iOS host): while one is open this app
// hands it the frames, keys and presses, draws its picture under the Worlds button, and plays its
// sound through the Apple audio host (aethyr/host/apple/PageAudio.swift).
//   Everglade [--chat]             opens the window (--chat: with the chat open in the Workshop)
// The chat (Chat in the Everglade menu, Command-J) is the OpenAgents chat (apps/openagents-desktop/linux/
// chat.h), a panel on the Workshop's left drawn with the shared components; its sessions are kept in
// ~/Library/Application Support/Everglade/chat/. A shot's --chat-demo opens it on the demo transcript, and
// --do's tap, text, key and wait reach the list, the Workshop and the chat too, in points.
//   Everglade --shot OUT.png [--size WxH] [--at SECONDS] [--hold KEYS] [--scene N]
//       draws one frame offscreen after SECONDS with KEYS held (letters, as "wd"), and exits;
//       without --scene it is the list of worlds. In a realm (--scene 1, 3 or 4) nothing leaves this
//       Mac during a shot unless --online is given (the room is then the site's, as a guest's
//       browser would join it), nothing of yours is kept, and --do STEPS plays input first: `key:w:2` holds a key two seconds,
//       `tap:640,400` clicks a point, `drag:600,400,700,400` drags from one point to another, `text:Ada` types, `wait:1` waits, `look` holds the pointer as
//       mouselook asks, `press:2:1` holds a mouse button a second at the middle, `turn:40,0` moves
//       the held mouse, `down:2` and `up:2` hold and let go a button around other steps, `drive:walk-to guard` asks the realm's agent driver, `near:guard:20:60` waits until the driver lists ID within METRES (SECONDS at most); steps are split by `;`.
//       A realm's sound is rendered offline in the shot's time, never played: stderr says what it
//       did (the packets, the voices, the names played, the loudest sample), --audio OUT.wav writes it.
//       --quality TIER starts the realm with that graphics tier kept (auto when none is).
//       --scale S draws S pixels a point (2 unless given); --realtime gives each frame the time it took, and stderr says the frame rate the realm ran at (the whole shot, and its last 5 seconds). --frames DIR writes every sixth frame after `record` (--every N: every Nth)
//       A shot never shows in the Dock or the menu bar.
//   Everglade --shot OUT.png --next WAIT [--reply TEXT]
//       presses Next issue first and waits up to WAIT seconds for the agent (a real Haiku turn on
//       your login); --reply then types TEXT as your answer and waits for the agent again. A reply
//       that starts an issue starts the coding agent: set EVERGLADE_CODER_MODE=rehearsal (read-only)
//       and EVERGLADE_CODER_MODEL=claude-haiku-5-5 to try the flow without changing anything.
//       Autopilot is on, so the loop goes round by itself: --runs N stops waiting after N coding turns
import AppKit
import Metal

// The menu bar's commands (ui_menus ids).
let commandWorlds: Int32 = 1, commandQuit: Int32 = 2, commandChat: Int32 = 3

// The realms' sound (PageAudio.swift) is the main actor's; so is all of this, run on the main thread by the host.
@MainActor
final class Workshop {
  let state = eg_new()!
  var host: OpaquePointer!
  var device: MTLDevice!
  var queue: MTLCommandQueue!     // the host's: the realms, the place and the frame share one queue
  let format = MTLPixelFormat.bgra8Unorm
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
  var time: Float = 0
  // The realms, each once its row is first opened, by its world's number. `pageOffline` is a
  // shot's: no socket and no request leaves this Mac.
  var pages: [Int32: AEPageHost] = [:]
  var sounds: [Int32: AEPageAudio] = [:]   // each open realm's sound
  var pageTried: Set<Int32> = []
  var pageOffline = false, pageShot = false
  var shownWorld: Int32 = 0           // the world whose realm is on screen; 0 when none is
  var lookHeld = false, worldsPress = false, leftDown = false
  weak var lookPage: AEPageHost?      // the realm the pointer is held for
  var shotStore = NSTemporaryDirectory() + "everglade-shot-\(getpid())"
  // The view in points, and where the pointer last was, from the host's frames and moves.
  var viewW: Float = 1320, viewH: Float = 880, pointerX: Float = 0, pointerY: Float = 0
  var cursorShown: Int32 = -1
  // What this frame draws, kept from the frame callback for the layer's prepare and encode.
  var drawn: Int32 = 0, panelDrawn: Int32 = 0, seen = false
  var box = [Float](repeating: 0, count: 8)
  var camera = [Float](repeating: 0, count: 24)
  var pu = [Float](repeating: 0, count: 24)
  var frameRealm: AEPageHost?
  // The OpenAgents chat (apps/openagents-desktop/linux/chat.h), made the first time it is shown: a panel on the
  // Workshop's left, drawn with the shared components. It has the keys after a press on it, until Escape or a
  // press on the scene. `chatDemo` is a shot's: the demo transcript, nothing kept.
  var chat: OpaquePointer?
  var chatShown = false, chatFocus = false, chatDown = false, chatDemo = false
  // The panel's width in points: under half the window, and at least the 600 the chat's sidebar and
  // transcript lay out in (narrower, its lines would run past the edge the host clips them at).
  var chatWidth: Float { min(max(viewW * 0.45, 600), min(720, viewW - 240)) }
  var chatOn: Bool { chatShown && chat != nil && eg_scene(state) == 2 && !inRealm }
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

  init() {
    // A realm's files are in the app: its row opens a world, not a card.
    for world in Realms.table.keys where Workshop.realmFiles(world) != nil { eg_runs(state, world, 1) }
  }

  // The app's pipelines on the host's device, once the host is open.
  func attach(_ h: OpaquePointer) {
    host = h
    device = (Unmanaged<AnyObject>.fromOpaque(ui_device(h)!).takeUnretainedValue() as! MTLDevice)
    queue = (Unmanaged<AnyObject>.fromOpaque(ui_queue(h)!).takeUnretainedValue() as! MTLCommandQueue)
    let url = Bundle.main.url(forResource: "default", withExtension: "metallib")!
    let lib = try! device.makeLibrary(URL: url)
    // The place: the temple's parts and Aethyr's shaders. Without them there is nowhere to stand.
    guard let pl = Place(device: device, color: format) else {
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
    a.pixelFormat = format; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .max
    a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = .one; a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .one
    strokes = try! device.makeRenderPipelineState(descriptor: f)
    vertices = device.makeBuffer(length: Int(eg_max_vertices()) * Int(eg_vertex_floats()) * 4, options: .storageModeShared)!
    overlay = device.makeBuffer(length: Int(nxp_max_vertices()) * Int(nxp_vertex_floats()) * 4, options: .storageModeShared)!
    let r = MTLRenderPipelineDescriptor()
    r.vertexFunction = lib.makeFunction(name: "rect_vs"); r.fragmentFunction = lib.makeFunction(name: "rect_fs")
    r.depthAttachmentPixelFormat = .depth32Float
    let ra = r.colorAttachments[0]!
    ra.pixelFormat = format; ra.isBlendingEnabled = true
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
          let m = Model(device: device, library: lib, color: format, pack: pack) else {
      FileHandle.standardError.write(Data("Everglade: the app's everglade2.pack is missing or doesn't hold the player's parts. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    model = m
    var layer = UiLayer()
    layer.user = Unmanaged.passUnretained(self).toOpaque()
    layer.depth = 1
    layer.prepare = { u, cmd, w, h, _ in MainActor.assumeIsolated { workshop(u).prepare(Unmanaged<AnyObject>.fromOpaque(cmd!).takeUnretainedValue() as! MTLCommandBuffer, Int(w), Int(h)) } }
    layer.encode = { u, enc, _, _, dpr in MainActor.assumeIsolated { workshop(u).encode(Unmanaged<AnyObject>.fromOpaque(enc!).takeUnretainedValue() as! MTLRenderCommandEncoder, scale: dpr) } }
    ui_layer(h, &layer)
  }

  // The chat, first: its sessions are kept beside Everglade's own (a shot keeps none), in the folder you
  // started the app from. It has no microphone here (dictation-none.c).
  func openChat() {
    if chat == nil {
      let res = (Bundle.main.resourceURL?.appendingPathComponent("chat").path ?? ".")
      let support = (FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.appendingPathComponent("Everglade/chat/settings.json").path) ?? ""
      let settings = pageShot ? "" : support
      chat = res.withCString { r in settings.withCString { st in
        var o = OaChatOptions()
        o.resources = r; o.settings = st; o.dpr = pageShot ? shotScale : ui_dpr(host); o.offscreen = pageShot ? 1 : 0; o.demo = chatDemo ? 1 : 0; o.still_clock = pageShot ? 1 : 0
        return oa_chat_new(&o)
      } }
      if let c = chat, oa_chat_start(c, host) == 0 { oa_chat_close(c); chat = nil }
      if chat == nil { FileHandle.standardError.write(Data("Everglade: the chat did not open; its fonts and icons are in the app's Resources/chat. Rebuild with apps/everglade-desktop/build.sh.\n".utf8)) }
    }
    chatShown = chat != nil
    if chatShown { eg_go(state, 2) }   // the chat is the Workshop's
  }

  // Opens a world's realm: its library, shaders, pack, fonts and pictures from the app, and what a
  // browser would keep in its storage under ~/.openagents/everglade/realm/ (one folder for every
  // realm, as one site's pages share a browser's storage: your character is the same in each).
  func openPage(_ world: Int32) {
    pageTried.insert(world)
    // A module runs one page at a time (Realms.sharing): opening a world closes any other page of its module.
    for other in Realms.sharing(world) where pages[other] != nil { closePage(other) }
    guard let device, let r = Realms.table[world], let dir = Workshop.realmFiles(world), let res = Bundle.main.resourceURL else { eg_runs(state, world, 0); return }
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
    if host != nil && !pageShot { ui_hold_pointer(host, on ? 1 : 0) }
    if on { lookPage = page }
    lookPage?.pointerLock(on ? 1 : 0)
    if !on { lookPage = nil }
  }
  func syncRealm() {
    guard let page, inRealm else { hold(false); setCursor(0); return }
    let want = page.mouselook()
    if want == 2 && !lookHeld && ui_focused(host) == 1 { hold(true) } else if want != 2 && lookHeld { hold(false) }
    // The realm's cursor (1 hand, 2 crosshair, 3 text) as the host's kinds.
    if !lookHeld { switch page.cursor() { case 1: setCursor(2); case 2: setCursor(4); case 3: setCursor(1); default: setCursor(0) } }
  }
  func setCursor(_ kind: Int32) { if kind != cursorShown { cursorShown = kind; ui_cursor(host, kind) } }

  // One frame's work before it is drawn (UiApp.frame): the realm's own picture, the scene's and
  // the Next issue panel's strokes, and what the panel asked of the Mac. w and h are device pixels.
  func frame(_ out: UnsafeMutablePointer<Float>?, width: Float, height: Float, scale: Float, dt: Float) -> Int32 {
    viewW = width / scale; viewH = height / scale
    time += dt
    // A realm's row, first opened: the realm starts. Leaving it lets its keys go; it keeps its state.
    let world = eg_scene(state)
    if Realms.table[world] != nil && pages[world] == nil && !pageTried.contains(world) { openPage(world) }
    let on: Int32 = pages[world] != nil ? world : 0
    if on != shownWorld { hold(false); pages[shownWorld]?.shown(false); sounds[shownWorld]?.show(false); pages[on]?.shown(true); sounds[on]?.show(true); shownWorld = on }
    let realm = page
    frameRealm = realm
    for (w, p) in pages where w != on { p.idle(); sounds[w]?.step(dt) }   // out of sight a realm draws nothing, and still hears its room
    // A shot waits for the realm's picture; the window doesn't.
    if let realm { _ = realm.frame(CGSize(width: CGFloat(viewW), height: CGFloat(viewH)), scale: CGFloat(scale), dt: dt, wait: pageShot); sounds[on]?.step(dt) }
    drawn = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), viewW, viewH, scale, dt)
    model?.step(state, dt: dt)
    // The view, the picture's size, the clock and where you stand: the place's uniforms, and the
    // first twenty floats Model.metal's MU.
    for k in 0..<24 { camera[k] = 0; pu[k] = 0 }
    seen = eg_camera(state, viewW, viewH, scale, &camera) == 1
    camera[16] = width; camera[17] = height; camera[18] = time; camera[19] = Float(eg_place(state))
    var you = [Float](repeating: 0, count: 5)
    if eg_you(state, &you) == 1 { camera[20] = you[0]; camera[21] = you[1]; camera[22] = you[2] }
    // The place's uniforms (everglade/workshop/gpu/temple.coax): size, clock, mirror flag, the eye
    // and its right, up and forward with the focal length, and where you stand.
    pu[0] = width; pu[1] = height; pu[2] = time
    for k in 0..<3 { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = camera[20 + k] }
    pu[19] = camera[15]
    // Next issue, over everything: its panel's dark box, then its strokes.
    for k in 0..<8 { box[k] = 0 }
    panelDrawn = nxp_frame(next, overlay.contents().assumingMemoryBound(to: Float.self), &box, viewW, viewH, scale, dt, Int64(Date().timeIntervalSince1970))
    serve()
    if !pageShot { syncRealm() }
    // The chat's panel over the scene's left, in the components' pass; the cursor is its own over it.
    guard chatOn, let chat, let out else { chatFocus = false; return 0 }
    ui_clip(host, 0, 0, (chatWidth * scale).rounded(), height)   // a long line stops at the panel's edge
    let n = oa_chat_frame(chat, out, chatWidth * scale, height, scale, dt)
    if pointerX >= chatWidth { cursorShown = -1; setCursor(0) }
    return n
  }
  // Passes of the place's own before the frame's: its reflection.
  func prepare(_ cmd: MTLCommandBuffer, _ width: Int, _ height: Int) {
    if seen, frameRealm == nil, let device { place.reflect(cmd, device: device, uniforms: pu, width: width, height: height) }
  }
  // The realm's picture; or the place, you, what is only light. Then the scene's text over them, as the phones draw it.
  func encode(_ enc: MTLRenderCommandEncoder, scale: Float) {
    var u: [Float] = [pu[0], pu[1], time, 0]
    if let realm = frameRealm { enc.setDepthStencilState(flat); realm.present(enc, format: format) }
    else if seen { place.draw(enc, uniforms: pu); model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
    enc.setDepthStencilState(flat)
    if drawn > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(drawn))
    }
    // Over the realm the agent's buttons and panels are not drawn: the realm's own HUD is there.
    // The agent keeps working; its buttons are back on the list and in the other worlds.
    for k in [0, 4] where box[k + 2] > 0 && frameRealm == nil {
      var ru: [Float] = [box[k] * scale, box[k + 1] * scale, box[k + 2] * scale, box[k + 3] * scale, pu[0], pu[1], 0, 0]
      enc.setRenderPipelineState(panel)
      enc.setVertexBytes(&ru, length: 32, index: 0)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }
    if panelDrawn > 0 && frameRealm == nil {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(overlay, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(panelDrawn))
    }
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
      if verb == "open", let url = URL(string: rest), url.scheme == "https" { ui_open_url(rest) }
      if verb == "copy" { _ = rest.withCString { ui_copy(host, $0, strlen($0)) } }
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
    // One frame of the host's, offscreen, dt seconds after the last; the GPU has drawn it when this returns.
    func render(_ dt: Float) { _ = ui_shot_frame(host, UInt32(width), UInt32(height), dt) }
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
          render(0.05)
          RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while CACurrentMediaTime() < until && (nxp_state(next) == 1 || nxp_state(next) == 2) && (nextRuns == 0 || nxp_runs(next) < nextRuns)
      }
      render(0)
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
      repeat { render(0.05); RunLoop.current.run(until: Date().addingTimeInterval(0.05)) } while CACurrentMediaTime() < until + 24 && refreshingGolems
      let read = CACurrentMediaTime()
      repeat { render(0.05); RunLoop.current.run(until: Date().addingTimeInterval(0.05)) } while CACurrentMediaTime() < read + golemsWait && readingGolems || CACurrentMediaTime() < read + 4.5
      if golemsView != 0 { nxp_golems_view(next, golemsView) }
    }
    // In a shot nobody holds the pointer, so the harness answers the realm's mouselook as a held
    // pointer would (--do look), and writes a numbered frame every few when --frames DIR is given.
    var frameCount = 0
    var frameStamps: [Double] = []   // --realtime: when each frame in a realm ended, for the frame rate said at the end
    func run(_ time: Float) {
      var left = time
      while left > 0 {
        // --realtime: a frame takes the time it really took, so the realm keeps pace with the site's room
        let since = Float(CACurrentMediaTime() - lastShotFrame); lastShotFrame = CACurrentMediaTime()
        let step = shotRealtime && inRealm ? min(max(since, 1.0 / 240), 0.1) : min(left, inRealm ? 1.0 / 60 : 0.05); render(step); left -= step
        if let page, inRealm, shotLooks { let want = page.mouselook(); if want == 2 && !lookHeld { lookHeld = true; page.pointerLock(1) } else if want != 2 && lookHeld { lookHeld = false; page.pointerLock(0) } }
        frameCount += 1
        if shotRealtime && inRealm { frameStamps.append(CACurrentMediaTime()) }
        if !framesDir.isEmpty && recording && frameCount % framesEvery == 0 { _ = ui_shot_write(host, framesDir + String(format: "/f%04d.png", frameCount)) }
      }
    }
    run(seconds)
    // --do: input for the realm, a step at a time, each followed by the frames it takes.
    for step in doSteps.split(separator: ";") {
      let part = step.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
      guard let verb = part.first else { continue }
      // Outside a realm (the list, the Workshop and its chat) a step goes in as the window's input does, in points.
      guard let page, inRealm else {
        let s = Float(shotScale)
        if verb == "wait", part.count > 1, let t = Float(part[1]) { run(t) }
        else if verb == "tap", part.count > 1, case let xy = part[1].split(separator: ",").compactMap({ Float($0) }), xy.count == 2 {
          pointer(xy[0] * s, xy[1] * s, 1); run(1.0 / 60); pointer(xy[0] * s, xy[1] * s, 0); run(1.0 / 60); pointer(xy[0] * s, xy[1] * s, 2); run(0.1)
        }
        else if verb == "text", part.count > 1 { text(part[1]); run(0.1) }
        else if verb == "key", part.count > 1 {
          let named: [String: Int32] = ["escape": -1, "enter": -2, "backspace": -3, "left": -4, "right": -5, "up": -6, "down": -7, "tab": -15]
          let code = named[part[1]] ?? Int32(part[1].unicodeScalars.first?.value ?? 0)
          _ = key(code, 0, true); run(part.count > 2 ? Float(part[2]) ?? 0.1 : 0.1); _ = key(code, 0, false); run(1.0 / 60)
        }
        continue
      }
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
      else if verb == "drag", part.count > 1 {   // `drag:x0,y0,x1,y1` presses at one point and drags to the other in ten moves (Character Create's turn)
        let v = part[1].split(separator: ",").compactMap { Double($0) }
        if v.count == 4 {
          page.pointer(CGPoint(x: v[0], y: v[1]), phase: 0, button: 0); run(1.0 / 60)
          for i in 1...10 { let t = Double(i) / 10; page.pointer(CGPoint(x: v[0] + (v[2] - v[0]) * t, y: v[1] + (v[3] - v[1]) * t), phase: 1, button: 0); run(1.0 / 60) }
          page.pointer(CGPoint(x: v[2], y: v[3]), phase: 2, button: 0); run(0.1)
        }
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
    render(0)
    nxp_close(next)
    // --realtime: the frame rate the realm really ran at, over the whole shot and over its last 5 seconds (the
    // HUD's own number is the last half second's), so a measurement doesn't hang on one sample.
    if shotRealtime, frameStamps.count > 2, let first = frameStamps.first, let last = frameStamps.last, last > first {
      let tail = frameStamps.filter { $0 >= last - 5 }
      var gaps: [Double] = []; for i in 1..<tail.count { gaps.append(tail[i] - tail[i - 1]) }
      gaps.sort()
      let late = tail.count > 1 && tail.last! > tail.first! ? Double(tail.count - 1) / (tail.last! - tail.first!) : 0
      let line = String(format: "Everglade: frames %d in %.1f s, %.1f FPS; the last 5 s %.1f FPS, median frame %.1f ms\n", frameStamps.count, last - first, Double(frameStamps.count - 1) / (last - first), late, gaps.isEmpty ? 0 : gaps[gaps.count / 2] * 1000)
      FileHandle.standardError.write(Data(line.utf8))
    }
    if !shotAudio.isEmpty, let sound = sounds[eg_scene(state)], !sound.write(shotAudio) { FileHandle.standardError.write(Data("Everglade: couldn't write \(shotAudio)\n".utf8)) }
    for world in Array(pages.keys) { closePage(world) }; try? FileManager.default.removeItem(atPath: shotStore)
    return ui_shot_write(host, path) == 1
  }

  // ---- input, from the host (device pixels; this app's worlds take points)

  // The Next issue button and panel take a press first; the world gets what they leave.
  func press(_ x: Float, _ y: Float, _ phase: Int32, button: Int32 = 0) {
    guard let page, inRealm else {
      if button == 0 && nxp_pointer(next, x, y, phase, viewW, viewH) == 0 { eg_pointer(state, x, y, phase) }
      return
    }
    // In the realm: a press that begins on the Worlds button is the list's; every other is the realm's.
    if button == 0 && !lookHeld {
      if phase == 0 { worldsPress = eg_on_worlds(state, x, y) == 1 }
      if worldsPress { eg_pointer(state, x, y, phase); if phase == 2 { worldsPress = false }; return }
    }
    if phase == 0 { heard() }   // a press is the gesture the realm's sound starts on, before the realm hears it
    // With the pointer held a move only turns the camera (motion), and a press is at the middle of the view.
    if lookHeld {
      if phase == 1 { return }
      page.pointer(CGPoint(x: CGFloat(viewW) / 2, y: CGFloat(viewH) / 2), phase: phase, button: button)
    } else { page.pointer(CGPoint(x: CGFloat(x), y: CGFloat(y)), phase: phase, button: button) }
    syncRealm()
  }
  func pointer(_ x: Float, _ y: Float, _ phase: Int32) {
    let s = ui_dpr(host), px = x / s, py = y / s
    pointerX = px; pointerY = py
    // On the chat's panel, or a press that began there: the chat's, in the host's pixels as its own app has them.
    if chatOn, let chat, chatDown || px < chatWidth {
      if phase == 0 { chatDown = true; chatFocus = true; _ = eg_key(state, -1, 1) }
      if phase == 2 { chatDown = false }
      oa_chat_pointer(chat, x, y, phase, 1, 0)
      return
    }
    if phase == 0 { chatFocus = false }
    if phase == 0 { leftDown = true }
    // A move with no button down is the realm's alone (its hover); the list and the panel take drags.
    if phase == 1 && !leftDown && !inRealm { return }
    if phase == 2 { leftDown = false }
    press(px, py, phase)
  }
  func button(_ x: Float, _ y: Float, _ phase: Int32, _ b: Int32) {
    let s = ui_dpr(host)
    if inRealm { press(x / s, y / s, phase, button: b) }
  }
  func motion(_ dx: Float, _ dy: Float) { if lookHeld, let page, inRealm { page.look(CGPoint(x: CGFloat(dx), y: CGFloat(dy))) } }
  func scroll(_ dy: Float, _ precise: Bool) {
    // GPUI's pixel delta, as the host gives the chat's own app: a mouse's lines times the line height (22.75)
    if chatOn, let chat, pointerX < chatWidth { oa_chat_wheel(chat, -(precise ? dy : dy * 22.75) * ui_dpr(host)); return }
    if let page, inRealm { page.wheel(hmi_appkit_wheel(dy, precise)); return }
    let d = precise ? dy : dy * 10
    if nxp_wheel(next, pointerX, pointerY, d, viewW, viewH) == 1 { return }
    eg_wheel(state, d)
  }
  // The host's named keys as the list's and Workshop's codes (the characters AppKit gave them).
  func listKey(_ k: Int32) -> Int32? {
    switch k {
    case -1: return 27; case -2: return 13; case -3: return 127; case -15: return 9
    case -4, -5, -6, -7, -13: return k
    case ..<0: return nil
    default: return k < 128 ? Int32(Character(Unicode.Scalar(UInt8(k))).lowercased().unicodeScalars.first!.value) : nil
    }
  }
  func key(_ k: Int32, _ mods: Int32, _ down: Bool) -> Bool {
    // The chat has the keys after a press on it: what it takes is its own, and Escape it leaves gives the keys back.
    if chatFocus, chatOn, let chat {
      if oa_chat_key(chat, k, mods, down ? 1 : 0) != 0 { return true }
      if down && k == -1 { chatFocus = false; return true }
    }
    if mods & 8 != 0 { return false }   // Command chords stay with the menu
    // While the reply has the keys they are typing, not walking: its text comes as text.
    if down && nxp_wants_keys(next) == 1 && !inRealm {
      switch k { case -2: nxp_key(next, 13); case -3: nxp_key(next, 8); case -1: nxp_key(next, 27); default: break }
      _ = eg_key(state, -1, 1)
      return true
    }
    if let page, inRealm {
      // A control character or a function key's private-use character is no key of the realm's.
      if k >= 0 && (k < 32 || k == 127 || (0xF700...0xF8FF).contains(k)) { return true }
      // Escape with the pointer held lets it go, as a browser does, and is not the realm's key.
      if down && k == -1 && lookHeld { hold(false); return true }
      if down && k != -13 && k != -14 && k != -16 { heard() }
      _ = page.key(k, mods: mods, down: down)
      if down { syncRealm() }
      return true
    }
    if k == -14 || k == -16 { return true }
    if let c = listKey(k) { _ = eg_key(state, c, down ? 1 : 0) }
    return true
  }
  func text(_ typed: String) {
    if chatFocus, chatOn, let chat, oa_chat_wants_text(chat) != 0 { typed.withCString { oa_chat_text(chat, $0, strlen($0)) }; return }
    if nxp_wants_keys(next) == 1 && !inRealm {
      for c in typed.unicodeScalars where c.isASCII && c.value >= 32 && c.value <= 126 { nxp_key(next, Int32(c.value)) }
      _ = eg_key(state, -1, 1)
      return
    }
    if let page, inRealm { heard(); page.text(typed); syncRealm() }
  }
  // While the reply or the realm's text box has the keys, AppKit's typing (and its input method) reaches text.
  func wantsText() -> Bool { (chatFocus && chatOn && oa_chat_wants_text(chat) != 0) || (nxp_wants_keys(next) == 1 && !inRealm) || (inRealm && page?.wantsText() == true) }
  func focus(_ on: Bool) {
    if !on { _ = eg_key(state, -1, 1); hold(false) }
    if inRealm { page?.focus(on) }
  }
  func command(_ id: Int32) {
    if id == commandWorlds { eg_go(state, 0) }   // the list of worlds from anywhere: in the realm Escape is the realm's own key
    if id == commandQuit { ui_quit(host) }
    if id == commandChat { if chatOn { chatShown = false; chatFocus = false } else { openChat() } }
  }
  // A file chosen in the paperclip's panel, dropped or pasted: the chat's.
  func file(_ request: Int32, _ path: String) { if let chat { path.withCString { oa_chat_file(chat, request, $0) } } }
  func close() { hold(false); for world in Array(pages.keys) { closePage(world) }; nxp_close(next); if let chat { oa_chat_close(chat) }; chat = nil }
}

@MainActor func workshop(_ u: UnsafeMutableRawPointer?) -> Workshop { Unmanaged<Workshop>.fromOpaque(u!).takeUnretainedValue() }

@main
enum Everglade {
  @MainActor static func main() {
    // Opened from the Finder the app gets a bare PATH; the agent's `gh` and a Homebrew or user install
    // of `claude` live in these, so they are added once, for this process and its children.
    let home = NSHomeDirectory(), have = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin"
    setenv("PATH", have + ":/opt/homebrew/bin:/usr/local/bin:\(home)/.local/bin", 1)
    let args = CommandLine.arguments
    let shotAt = args.firstIndex(of: "--shot").flatMap { $0 + 1 < args.count ? args[$0 + 1] : nil }
    // A headless run (a shot, an Autopilot run) never shows in the Dock or the menu bar.
    if shotAt != nil { NSApplication.shared.setActivationPolicy(.prohibited) }
    let w = Workshop()
    var app = UiApp()
    app.title = UnsafePointer(strdup("Everglade")); app.app_id = UnsafePointer(strdup("com.openagents.everglade"))
    app.width = 1320; app.height = 880; app.min_width = 900; app.min_height = 600
    app.max_vertices = oa_chat_max_vertices()   // the chat's panel (chat.h), the one thing drawn with the components
    // The component shader (components/backend/gpu-shader.coax as Metal), for what the app draws with the shared components.
    app.shader = UnsafePointer(strdup(Bundle.main.url(forResource: "zeron", withExtension: "metallib")?.path ?? ""))
    app.user = Unmanaged.passUnretained(w).toOpaque()
    app.frame = { u, out, width, height, dpr, dt in MainActor.assumeIsolated { workshop(u).frame(out, width: width, height: height, scale: dpr, dt: dt) } }
    app.pointer = { u, x, y, phase, _, _ in MainActor.assumeIsolated { workshop(u).pointer(x, y, phase) } }
    app.button = { u, x, y, phase, b, _ in MainActor.assumeIsolated { workshop(u).button(x, y, phase, b) } }
    app.motion = { u, dx, dy in MainActor.assumeIsolated { workshop(u).motion(dx, dy) } }
    app.scroll = { u, dy, precise in MainActor.assumeIsolated { workshop(u).scroll(dy, precise != 0) } }
    app.key = { u, k, mods, down in MainActor.assumeIsolated { workshop(u).key(k, mods, down != 0) ? 1 : 0 } }
    app.text = { u, bytes, n in
      guard let bytes, let s = String(bytes: UnsafeRawBufferPointer(start: bytes, count: n), encoding: .utf8) else { return }
      MainActor.assumeIsolated { workshop(u).text(s) }
    }
    app.wants_text = { u in MainActor.assumeIsolated { workshop(u).wantsText() ? 1 : 0 } }
    app.focus = { u, on in MainActor.assumeIsolated { workshop(u).focus(on != 0) } }
    app.command = { u, id in MainActor.assumeIsolated { workshop(u).command(id) } }
    app.file = { u, request, path in
      guard let path else { return }
      let p = String(cString: path)
      MainActor.assumeIsolated { workshop(u).file(request, p) }
    }
    app.modifier_keys = 1
    guard let h = ui_open(&app, shotAt != nil ? 1 : 0) else {
      FileHandle.standardError.write(Data("Everglade: the window did not open. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    w.attach(h)
    if let path = shotAt {
      var width = 2640, height = 1760, seconds: Float = 2
      if let s = args.firstIndex(of: "--size"), s + 1 < args.count {
        let parts = args[s + 1].split(separator: "x").compactMap { Int($0) }
        if parts.count == 2, parts[0] > 0, parts[1] > 0, parts[0] <= 8192, parts[1] <= 8192 { width = parts[0]; height = parts[1] }
      }
      if let t = args.firstIndex(of: "--at"), t + 1 < args.count, let v = Float(args[t + 1]), v >= 0, v <= 600 { seconds = v }
      var shotScale: Float = 2
      if let c = args.firstIndex(of: "--scale"), c + 1 < args.count, let v = Float(args[c + 1]), v >= 1, v <= 3 { shotScale = v }
      ui_set_dpr(h, shotScale)
      if let r = args.firstIndex(of: "--room"), r + 1 < args.count, args[r + 1].allSatisfy({ $0.isLetter || $0.isNumber || $0 == "-" }) { w.shotRoom = args[r + 1] }
      if let r = args.firstIndex(of: "--every"), r + 1 < args.count, let v = Int(args[r + 1]), v >= 1, v <= 60 { w.framesEvery = v }
      w.shotScale = shotScale; w.shotRealtime = args.contains("--realtime")
      if let n = args.firstIndex(of: "--next"), n + 1 < args.count, let v = Float(args[n + 1]), v > 0, v <= 86400 { w.nextWait = v }
      if let r = args.firstIndex(of: "--reply"), r + 1 < args.count { w.nextReply = args[r + 1] }
      if let r = args.firstIndex(of: "--runs"), r + 1 < args.count, let v = Int32(args[r + 1]), v > 0, v <= 16 { w.nextRuns = v }
      w.pageShot = true; w.pageOffline = !args.contains("--online")
      if let d = args.firstIndex(of: "--do"), d + 1 < args.count { w.doSteps = args[d + 1] }
      if let d = args.firstIndex(of: "--frames"), d + 1 < args.count { w.framesDir = args[d + 1] }
      if let d = args.firstIndex(of: "--audio"), d + 1 < args.count { w.shotAudio = args[d + 1] }
      // --quality high|medium|low|auto: the graphics tier the shot's realm finds kept, as a browser that picked it would
      if let d = args.firstIndex(of: "--quality"), d + 1 < args.count, ["high", "medium", "low", "auto"].contains(args[d + 1]) {
        try? FileManager.default.createDirectory(atPath: w.shotStore, withIntermediateDirectories: true)
        try? args[d + 1].write(toFile: w.shotStore + "/verse.quality", atomically: true, encoding: .utf8)
      }
      if let s = args.firstIndex(of: "--scene"), s + 1 < args.count, let v = Int32(args[s + 1]) { eg_go(w.state, v) }
      if let k = args.firstIndex(of: "--hold"), k + 1 < args.count {
        for c in args[k + 1].lowercased().unicodeScalars where c.isASCII { _ = eg_key(w.state, Int32(c.value), 1) }
      }
      if args.contains("--golems") { nxp_golems_open(w.next); w.golemsWait = 6 }
      // --chat: the chat's panel in the Workshop (--chat-demo: its demo transcript)
      if args.contains("--chat") || args.contains("--chat-demo") { w.chatDemo = args.contains("--chat-demo"); w.openChat() }
      if let g = args.firstIndex(of: "--golems-view"), g + 1 < args.count, let v = Int32(args[g + 1]) { w.golemsView = v }
      exit(w.shot(to: path, width: width, height: height, seconds: seconds) ? 0 : 1)
    }
    if let s = args.firstIndex(of: "--scene"), s + 1 < args.count, let v = Int32(args[s + 1]) { eg_go(w.state, v) }
    if args.contains("--golems") { nxp_golems_open(w.next) }
    if args.contains("--chat") { w.openChat() }
    let menus: [(String?, String, Int32, Int32, String?)] = [
      ("Worlds", "0", 8, commandWorlds, nil), ("Chat", "j", 8, commandChat, nil), (nil, "", 0, -1, nil), ("Quit Everglade", "q", 8, commandQuit, nil),
    ]
    var items: [UiMenuItem] = menus.map { m in
      UiMenuItem(menu: UnsafePointer(strdup("Everglade")), title: m.0.map { UnsafePointer(strdup($0)) }, key: UnsafePointer(strdup(m.1)), mods: m.2, id: m.3, selector: m.4.map { UnsafePointer(strdup($0)) })
    }
    ui_menus(h, &items, Int32(items.count))
    while ui_frame(h) != 0 {}
    w.close()
    ui_close(h)
    exit(0)
  }
}
