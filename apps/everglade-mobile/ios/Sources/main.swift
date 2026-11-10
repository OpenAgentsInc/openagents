// Firm reason: 6 (UIKit and Metal have no C API; this host holds no product logic).
// Everglade for iOS, the host (apps/everglade-mobile/README.md): a full-screen Metal view and
// touch. Everything drawn comes from the same Coax as the desktop app: the scene (workshop.coax),
// the temple's parts and Aethyr shaders, and your model. What the hosts share on the GPU is
// apps/everglade-desktop/Sources/Engine.swift.
//
// Touch: a drag that starts on the left side is a stick that moves you; one that starts on the
// right side looks round; a tap on the right side jumps; a pinch zooms. The Coax decides what
// each does: the host only reports where fingers are.
import UIKit
import Metal
import MetalKit

// A thread with a deep stack, for the web's character code (Engine.swift Model.deep): iOS gives
// the main thread a megabyte, and that code passes megabytes by value.
final class Deep {
  private var work: (() -> Void)?
  private let go = DispatchSemaphore(value: 0), done = DispatchSemaphore(value: 0)
  init() {
    let t = Thread { [unowned self] in
      while true { self.go.wait(); self.work?(); self.work = nil; self.done.signal() }
    }
    t.stackSize = 512 << 20
    t.name = "everglade.deep"
    t.start()
  }
  func run(_ block: () -> Void) {
    withoutActuallyEscaping(block) { b in work = b; go.signal(); done.wait() }
  }
}

final class WorkshopView: MTKView, MTKViewDelegate {
  let state = eg_new()!
  var place: Place!
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
    place = pl
    guard let atlas = faceAtlas(device) else { fatalError("Everglade: the app's fonts are missing. Rebuild with apps/everglade-mobile/build.sh.") }
    faces = atlas
    let fsd = MTLSamplerDescriptor(); fsd.minFilter = .linear; fsd.magFilter = .linear; fsd.mipFilter = .linear
    faceSampler = device.makeSamplerState(descriptor: fsd)!
    Model.deep = { deep.run($0) }
    guard let url = Bundle.main.url(forResource: "everglade2", withExtension: "pack"), let pack = Pack(url),
          let m = Model(device: device, library: lib, color: colorPixelFormat, pack: pack) else { fatalError("Everglade: the app's everglade2.pack is missing or doesn't hold the player's parts. Rebuild with apps/everglade-mobile/build.sh.") }
    model = m
    eg_pad(state, 0, 0)   // a touch screen: the scene shows the touch hint
    addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinch(_:))))
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
    let n = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), Float(size.width) / scale, Float(size.height) / scale, scale, dt)
    var u: [Float] = [Float(size.width), Float(size.height), time, 0]
    model?.step(state, dt: dt)
    var camera = [Float](repeating: 0, count: 24)
    let seen = eg_camera(state, Float(size.width) / scale, Float(size.height) / scale, scale, &camera) == 1
    camera[16] = Float(size.width); camera[17] = Float(size.height); camera[18] = time
    var you = [Float](repeating: 0, count: 5)
    if eg_you(state, &you) == 1 { camera[20] = you[0]; camera[21] = you[1]; camera[22] = you[2] }
    // The place's uniforms (everglade/workshop/gpu/temple.coax).
    var pu = [Float](repeating: 0, count: 24)
    pu[0] = Float(size.width); pu[1] = Float(size.height); pu[2] = time
    for k in 0..<3 { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = camera[20 + k] }
    pu[19] = camera[15]
    if seen { place.reflect(cmd, device: device, uniforms: pu, width: Int(size.width), height: Int(size.height)) }
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    if seen { place.draw(enc, uniforms: pu); model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
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

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    for t in touches {
      let p = t.location(in: self)
      if stick == nil && p.x < bounds.width * 0.45 { stick = t; stickFrom = p }
      else if look == nil { look = t; lookMoved = 0; lookDown = CACurrentMediaTime(); eg_pointer(state, Float(p.x), Float(p.y), 0) }
    }
  }
  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    for t in touches {
      let p = t.location(in: self)
      if t === stick {
        // The stick's whole travel is 70 points; forward is up the screen.
        eg_pad(state, Float((p.x - stickFrom.x) / 70), Float((stickFrom.y - p.y) / 70))
      } else if t === look {
        let q = t.previousLocation(in: self)
        lookMoved += abs(p.x - q.x) + abs(p.y - q.y)
        eg_pointer(state, Float(p.x), Float(p.y), 1)
      }
    }
  }
  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { lift(touches, tapped: true) }
  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { lift(touches, tapped: false) }
  func lift(_ touches: Set<UITouch>, tapped: Bool) {
    for t in touches {
      let p = t.location(in: self)
      if t === stick { stick = nil; eg_pad(state, 0, 0) }
      else if t === look {
        look = nil
        eg_pointer(state, Float(p.x), Float(p.y), 2)
        // A finger down and up in one place, quickly, is a tap: the jump key, held a moment.
        if tapped && lookMoved < 12 && CACurrentMediaTime() - lookDown < 0.35 { _ = eg_key(state, 32, 1); jumpUp = CACurrentMediaTime() + 0.1 }
      }
    }
  }
  @objc func pinch(_ g: UIPinchGestureRecognizer) {
    if g.state == .changed { eg_wheel(state, Float(1 - g.scale) * 220); g.scale = 1 }
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
    if let v = view as? WorkshopView { eg_safe(v.state, Float(view.safeAreaInsets.left), Float(view.safeAreaInsets.top), Float(view.safeAreaInsets.bottom)) }
  }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?
  func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
    let w = UIWindow(frame: UIScreen.main.bounds)
    w.rootViewController = Root()
    w.makeKeyAndVisible()
    window = w
    return true
  }
  // Losing the screen lets every key and finger go.
  func applicationWillResignActive(_ application: UIApplication) {
    if let v = window?.rootViewController?.view as? WorkshopView { _ = eg_key(v.state, -1, 1); eg_pad(v.state, 0, 0); v.stick = nil; v.look = nil }
  }
}

UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
