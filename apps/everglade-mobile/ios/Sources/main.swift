// Firm reason: 6 (UIKit and Metal have no C API; this host holds no product logic).
// Everglade for iOS, the host (apps/everglade-mobile/README.md): a full-screen Metal view and
// touch. Everything drawn comes from the same Coax as the desktop app: the scene (workshop.coax),
// the temple's parts and Metaverse Engine shaders, and your model. What the hosts share on the GPU is
// apps/everglade-desktop/Sources/Engine.swift. The web's realms (Everglade itself, the Archery
// Range, Experimental) run through the Metaverse Engine's page host, as on the Mac (Pages.swift): while one is
// open its picture fills the screen inside the notch and the home bar, every finger is the realm's
// as a browser's touch is, and only a touch that begins on the Worlds button is this app's.
//
// Touch: a drag that starts on the left side is a stick that moves you; one that starts on the
// right side looks round; a tap on the right side jumps; a second finger on the right side
// pinches to zoom. A touch that begins on the list of worlds or on one of a world's buttons is a
// press wherever it is (face.h eg_claims). Both thumbs work at once: the fingers are followed here, not by a gesture
// recognizer, which would take them away from each other. Turning the phone looks round too
// (PhoneTurn.swift) while the scene's Gyro switch is on. The Coax decides what each does: the
// host only reports where fingers are and how the phone turned.
import UIKit
import Metal
import MetalKit
import AVFoundation


final class WorkshopView: MTKView, MTKViewDelegate, UIKeyInput {
  let state = eg_new()!
  var pages: RealmPages!
  var script = Script(arguments: CommandLine.arguments)
  var safe = UIEdgeInsets.zero
  // The finger that went down on the Worlds button over a realm.
  var worlds: AnyObject?
  var temple: Place!
  // The place behind the world that is open (face.h eg_place).
  var place: Place { temple }
  var model: Model?
  var strokes: MTLRenderPipelineState!
  var vertices: MTLBuffer!
  var flat: MTLDepthStencilState!
  var faces: MTLTexture!
  var faceSampler: MTLSamplerState!
  var queue: MTLCommandQueue!
  var last = CACurrentMediaTime()
  var time: Float = 0
  // The finger that is the stick and where it went down; the finger that looks, and how far it has gone.
  var stick: UITouch?, stickFrom = CGPoint.zero
  var look: UITouch?, lookMoved: CGFloat = 0, lookDown = 0.0
  // A second finger on the look side, and how far apart the two were last.
  var pinch: UITouch?, pinchSpan: CGFloat = 0
  let turn = PhoneTurn()
  var jumpUp = 0.0

  init(frame: CGRect, deep: Deep) {
    super.init(frame: frame, device: MTLCreateSystemDefaultDevice())
    guard let device else { fatalError("Everglade needs a Metal device.") }
    colorPixelFormat = .bgra8Unorm
    depthStencilPixelFormat = .depth32Float; clearDepth = 1
    clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    preferredFramesPerSecond = UIScreen.main.maximumFramesPerSecond
    isMultipleTouchEnabled = true
    queue = device.makeCommandQueue()
    let lib = try! device.makeLibrary(URL: Bundle.main.url(forResource: "default", withExtension: "metallib")!)
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
    let fd = MTLDepthStencilDescriptor(); fd.depthCompareFunction = .always; fd.isDepthWriteEnabled = false
    flat = device.makeDepthStencilState(descriptor: fd)!
    // The app's own parts. Without one of them there is nothing to show, so say which and stop.
    guard let pl = Place(device: device, color: colorPixelFormat) else { fatalError("Everglade: the temple did not build or its shaders are missing. Rebuild with apps/everglade-mobile/build.sh.") }
    temple = pl
    guard let atlas = faceAtlas(device) else { fatalError("Everglade: the app's fonts are missing. Rebuild with apps/everglade-mobile/build.sh.") }
    faces = atlas
    let fsd = MTLSamplerDescriptor(); fsd.minFilter = .linear; fsd.magFilter = .linear; fsd.mipFilter = .linear
    faceSampler = device.makeSamplerState(descriptor: fsd)!
    Model.deep = { deep.run($0) }
    guard let url = Bundle.main.url(forResource: "everglade2", withExtension: "pack"), let pack = Pack(url, device: device),
          let m = Model(device: device, library: lib, color: colorPixelFormat, pack: pack) else { fatalError("Everglade: the app's everglade2.pack is missing or doesn't hold the player's parts. Rebuild with apps/everglade-mobile/build.sh.") }
    model = m
    eg_pad(state, 0, 0)   // a touch screen: the scene shows the touch hint
    // "--scene N" opens on world N (face.h eg_scene), for a look at one without a tap.
    if let w = CommandLine.arguments.firstIndex(of: "--scene"), w + 1 < CommandLine.arguments.count, let v = Int32(CommandLine.arguments[w + 1]) { eg_go(state, v) }
    pages = RealmPages(state: state, deep: deep, device: device, queue: queue)
    turn.view = self
    // The simulator has no gyro; "--offer-gyro" shows the switch there, to see it drawn.
    if turn.available || CommandLine.arguments.contains("--offer-gyro") { eg_turn_offer(state) }
    if turn.available || CommandLine.arguments.contains("--offer-gyro") { pages.motion = 1 }
    delegate = self
  }
  required init(coder: NSCoder) { fatalError() }

  func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}
  func draw(in view: MTKView) {
    guard let pass = currentRenderPassDescriptor, let drawable = currentDrawable, let cmd = queue.makeCommandBuffer(), let device else { return }
    let now = CACurrentMediaTime(); let dt = Float(now - last); last = now
    if jumpUp > 0 && now >= jumpUp { jumpUp = 0; _ = eg_key(state, 32, 0) }
    let size = drawableSize, scale = Float(contentScaleFactor)
    time += dt
    script?.step(dt, pages: pages) { [unowned self] key, at, phase in self.finger(key, at: at, phase: phase) }
    // The realm of the world that is open, drawn into its own picture: the screen inside the notch,
    // the corners and the home bar. Its keyboard comes up while it takes text.
    pages.box = bounds.inset(by: safe)
    let realm = pages.frame(scale: CGFloat(scale), dt: dt)
    // A realm's governor holds 60 (metaverse-engine/core/quality.coax): drawn at a ProMotion screen's 120 it does
    // twice the GPU's work for nothing, the phone heats and slows (#13190).
    let fps = realm != nil ? min(60, UIScreen.main.maximumFramesPerSecond) : UIScreen.main.maximumFramesPerSecond
    if preferredFramesPerSecond != fps { preferredFramesPerSecond = fps }
    let typing = realm != nil && pages.wantsText
    if typing != isFirstResponder { if typing { _ = becomeFirstResponder() } else { _ = resignFirstResponder() } }
    // The phone's turn: the Workshop's while its Gyro switch is on, a realm's while it wants it
    // (metaverse-engine/cam/gyro.coax: its GYRO button, the bow drawn).
    let realmTurns = realm != nil && pages.wantsMotion
    if eg_turns(state) == 1 || realmTurns {
      if !turn.running { turn.start() }
      let t = turn.take()
      if realmTurns { pages.tilt(Float(t.yaw), Float(t.pitch)) } else { eg_turn(state, Float(t.yaw), Float(t.pitch)) }
    } else if turn.running { turn.stop() }
    let n = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), Float(size.width) / scale, Float(size.height) / scale, scale, dt)
    var u: [Float] = [Float(size.width), Float(size.height), time, 0]
    if !pages.opening { model?.step(state, dt: dt) }   // the deep thread is a realm's while it opens
    var camera = [Float](repeating: 0, count: 24)
    let seen = eg_camera(state, Float(size.width) / scale, Float(size.height) / scale, scale, &camera) == 1
    camera[16] = Float(size.width); camera[17] = Float(size.height); camera[18] = time; camera[19] = Float(eg_place(state))
    let place = self.place
    var you = [Float](repeating: 0, count: 5)
    if eg_you(state, &you) == 1 { camera[20] = you[0]; camera[21] = you[1]; camera[22] = you[2] }
    // The place's uniforms (everglade/workshop/gpu/temple.coax).
    var pu = [Float](repeating: 0, count: 24)
    pu[0] = Float(size.width); pu[1] = Float(size.height); pu[2] = time
    for k in 0..<3 { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = camera[20 + k] }
    pu[19] = camera[15]
    if seen, realm == nil { place.reflect(cmd, device: device, uniforms: pu, width: Int(size.width), height: Int(size.height)) }
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    // The realm's picture in its box; or the place, you, what is only light. Then the scene's text over them.
    if let realm {
      let b = pages.box, k = Double(scale)
      enc.setDepthStencilState(flat)
      enc.setViewport(MTLViewport(originX: b.minX * k, originY: b.minY * k, width: b.width * k, height: b.height * k, znear: 0, zfar: 1))
      realm.present(enc, format: colorPixelFormat)
      enc.setViewport(MTLViewport(originX: 0, originY: 0, width: size.width, height: size.height, znear: 0, zfar: 1))
    } else if seen { place.draw(enc, uniforms: pu); model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
    enc.setDepthStencilState(flat)
    if n > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    enc.endEncoding()
    cmd.present(drawable)
    cmd.commit()
  }

  // A finger over a realm (a real touch, or a shot's script): one that begins on the Worlds button
  // is the list's, every other is the realm's. False when no realm is open.
  @discardableResult func finger(_ key: AnyObject, at p: CGPoint, phase: Int32) -> Bool {
    if worlds === key {
      eg_pointer(state, Float(p.x), Float(p.y), min(phase, 2))
      if phase >= 2 { worlds = nil }
      return true
    }
    if pages.owns(key) { pages.finger(key, at: p, phase: phase); return true }
    guard phase == 0, pages.page != nil else { return false }
    if worlds == nil && eg_on_worlds(state, Float(p.x), Float(p.y)) == 1 { worlds = key; eg_pointer(state, Float(p.x), Float(p.y), 0) }
    else { pages.finger(key, at: p, phase: 0) }
    return true
  }
  // The realm's text field, typed on the phone's keyboard.
  override var canBecomeFirstResponder: Bool { true }
  var hasText: Bool { true }
  func insertText(_ text: String) { pages.typed(text) }
  func deleteBackward() { pages.deleted() }
  var autocorrectionType: UITextAutocorrectionType { get { .no } set {} }
  var autocapitalizationType: UITextAutocapitalizationType { get { .words } set {} }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    for t in touches {
      let p = t.location(in: self)
      if finger(t, at: p, phase: 0) { continue }
      // A touch on the list of worlds or on a button is a press, never the stick.
      if stick == nil && p.x < bounds.width * 0.45 && eg_claims(state, Float(p.x), Float(p.y)) == 0 { stick = t; stickFrom = p }
      else if look == nil { look = t; lookMoved = 0; lookDown = CACurrentMediaTime(); eg_pointer(state, Float(p.x), Float(p.y), 0) }
      else if pinch == nil { pinch = t; pinchSpan = 0; lookMoved = 1000 }
    }
  }
  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    for t in touches {
      let p = t.location(in: self)
      if finger(t, at: p, phase: 1) { continue }
      if t === stick {
        // The stick's whole travel is 70 points; forward is up the screen.
        eg_pad(state, Float((p.x - stickFrom.x) / 70), Float((stickFrom.y - p.y) / 70))
      } else if t === look && pinch == nil {
        let q = t.previousLocation(in: self)
        lookMoved += abs(p.x - q.x) + abs(p.y - q.y)
        eg_pointer(state, Float(p.x), Float(p.y), 1)
      }
    }
    // Two fingers on the look side: how far apart they are zooms, and neither looks.
    if let a = look, let b = pinch {
      let p = a.location(in: self), q = b.location(in: self), span = hypot(p.x - q.x, p.y - q.y)
      if pinchSpan > 1 && span > 1 { eg_wheel(state, Float(1 - span / pinchSpan) * 220) }
      pinchSpan = span
    }
  }
  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { lift(touches, tapped: true) }
  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { lift(touches, tapped: false) }
  func lift(_ touches: Set<UITouch>, tapped: Bool) {
    for t in touches {
      let p = t.location(in: self)
      if finger(t, at: p, phase: tapped ? 2 : 3) { continue }
      if t === stick { stick = nil; eg_pad(state, 0, 0) }
      else if t === pinch {
        pinch = nil
        // The looking finger moved while it pinched and none of that was a look: it starts again
        // from where it is now, or the camera would jump by all of it.
        if let l = look, !touches.contains(l) { let q = l.location(in: self); eg_pointer(state, Float(q.x), Float(q.y), 0) }
      }
      else if t === look {
        look = nil; pinch = nil
        eg_pointer(state, Float(p.x), Float(p.y), 2)
        // A finger down and up in one place, quickly, is a tap: on the Gyro switch it flips it,
        // anywhere else it is the jump key, held a moment.
        if tapped && lookMoved < 12 && CACurrentMediaTime() - lookDown < 0.35 && eg_tap(state, Float(p.x), Float(p.y)) == 0 { _ = eg_key(state, 32, 1); jumpUp = CACurrentMediaTime() + 0.1 }
      }
    }
  }
}

final class Root: UIViewController {
  let deep = Deep()
  override func loadView() { view = WorkshopView(frame: UIScreen.main.bounds, deep: deep) }
  override var prefersStatusBarHidden: Bool { true }
  override var prefersHomeIndicatorAutoHidden: Bool { true }
  override var supportedInterfaceOrientations: UIInterfaceOrientationMask { .allButUpsideDown }
  // The scene keeps its text out from under the notch and the home bar.
  override func viewSafeAreaInsetsDidChange() {
    super.viewSafeAreaInsetsDidChange()
    if let v = view as? WorkshopView { v.safe = view.safeAreaInsets; eg_safe(v.state, Float(view.safeAreaInsets.left), Float(view.safeAreaInsets.top), Float(view.safeAreaInsets.bottom)) }
  }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?
  func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
    // The realms' sound is a game's: it mixes with what is playing, and the ring switch silences it.
    try? AVAudioSession.sharedInstance().setCategory(.ambient)
    let w = UIWindow(frame: UIScreen.main.bounds)
    w.rootViewController = Root()
    w.makeKeyAndVisible()
    window = w
    return true
  }
  // Losing the screen lets every key and finger go.
  func applicationWillResignActive(_ application: UIApplication) {
    if let v = window?.rootViewController?.view as? WorkshopView { _ = eg_key(v.state, -1, 1); eg_pad(v.state, 0, 0); v.stick = nil; v.look = nil; v.pinch = nil; v.worlds = nil; v.turn.stop(); v.pages.active(false) }
  }
  func applicationDidBecomeActive(_ application: UIApplication) {
    if let v = window?.rootViewController?.view as? WorkshopView { v.pages.active(true) }
  }
}

UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
