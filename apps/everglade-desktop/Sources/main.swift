// Firm reason: 6 (AppKit and Metal have no C API; this host holds no product logic).
// Everglade for desktop, the Mac host (everglade/docs/desktop-app-spec.md): a window,
// a Metal view and input. Everything drawn comes from workshop.coax through face.h.
//   Everglade                      opens the window
//   Everglade --shot OUT.png [--size WxH] [--at SECONDS] [--hold KEYS]
//       draws one frame offscreen after SECONDS with KEYS held (letters, as "wd"), and exits
//   Everglade --shot OUT.png --next WAIT [--reply TEXT]
//       presses Next issue first and waits up to WAIT seconds for the agent (a real Haiku turn on
//       your login); --reply then types TEXT as your answer and waits for the agent again. A reply
//       that starts an issue starts the coding agent: set EVERGLADE_CODER_MODE=rehearsal (read-only)
//       and EVERGLADE_CODER_MODEL=claude-haiku-5-5 to try the flow without changing anything.
//       Autopilot is on, so the loop goes round by itself: --runs N stops waiting after N coding turns
import AppKit
import Metal
import MetalKit
import ImageIO
import CoreText

// The faces' atlas (text.coax): ASCII 32..126 of Spectral Medium in red and of Cinzel at weight 600
// in green, 16 columns of 96-texel cells, the pen at x 16 and the baseline at y 72 at 64 px, as the
// sans atlas (aethyr/render/sans-profile.coax). Each mip is drawn at its own size.
func faceAtlas(_ device: MTLDevice) -> MTLTexture? {
  func face(_ file: String, weight: CGFloat?) -> CTFontDescriptor? {
    guard let url = Bundle.main.url(forResource: file, withExtension: "ttf", subdirectory: "fonts"),
          let all = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor], let d = all.first else { return nil }
    guard let weight else { return d }
    // Cinzel is variable; 'wght' is its weight axis.
    return CTFontDescriptorCreateCopyWithAttributes(d, [kCTFontVariationAttribute: [NSNumber(value: 0x77676874): NSNumber(value: Double(weight))]] as CFDictionary)
  }
  guard let body = face("Spectral-Medium", weight: nil), let title = face("Cinzel", weight: 600) else { return nil }
  let cols = 16, rows = 6, cell = 96, levels = 5
  let td = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rg8Unorm, width: cols * cell, height: rows * cell, mipmapped: true)
  td.mipmapLevelCount = levels
  guard let texture = device.makeTexture(descriptor: td) else { return nil }
  for level in 0..<levels {
    let c = cell >> level, w = cols * c, h = rows * c, scale = CGFloat(1 << level)
    var both = [UInt8](repeating: 0, count: w * h * 2)
    for (channel, descriptor) in [body, title].enumerated() {
      guard let g = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue) else { return nil }
      g.setFillColor(gray: 0, alpha: 1); g.fill(CGRect(x: 0, y: 0, width: w, height: h))
      g.setShouldSmoothFonts(false); g.setAllowsAntialiasing(true); g.textMatrix = .identity
      let font = CTFontCreateWithFontDescriptor(descriptor, 64 / scale, nil)
      let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: CGColor(gray: 1, alpha: 1)]
      for i in 0..<95 {
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: String(Character(Unicode.Scalar(UInt8(32 + i)))), attributes: attrs))
        g.textPosition = CGPoint(x: CGFloat((i % cols) * c) + 16 / scale, y: CGFloat(h) - (CGFloat((i / cols) * c) + 72 / scale))
        CTLineDraw(line, g)
      }
      let gray = g.data!.assumingMemoryBound(to: UInt8.self)
      for p in 0..<w * h { both[p * 2 + channel] = gray[p] }
    }
    texture.replace(region: MTLRegionMake2D(0, 0, w, h), mipmapLevel: level, withBytes: both, bytesPerRow: w * 2)
  }
  return texture
}

// Everglade II's pack (verse/tools/pack_realm.py VPK1): a catalog, the vertices, the indices and
// the texture layers. The host only finds the entries; model.coax reads the catalog.
struct Pack {
  var catalog = Data(), vertices = Data(), indices = Data(), layers: [Data] = []
  init?(_ url: URL) {
    guard let d = try? Data(contentsOf: url, options: .mappedIfSafe), d.count >= 32, d.prefix(4) == Data("VPK1".utf8) else { return nil }
    func word(_ at: Int) -> Int { at + 4 <= d.count ? Int(d.subdata(in: at..<at + 4).withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }) : -1 }
    let count = word(4), table = word(16)
    guard count > 0, table >= 32, table + count * 48 <= d.count else { return nil }
    for k in 0..<count {
      let e = table + k * 48, off = word(e + 4), len = word(e + 8)
      guard off >= 0, len >= 0, off + len <= d.count else { return nil }
      let bytes = d.subdata(in: off..<off + len)
      switch d[e] { case 1: catalog = bytes; case 2: vertices = bytes; case 3: indices = bytes; case 4: layers.append(bytes); default: break }
    }
    if catalog.isEmpty || vertices.isEmpty || indices.isEmpty || layers.isEmpty { return nil }
  }
}

// The place on the GPU: the temple's parts (everglade/workshop/temple.coax, built from Aethyr's
// architecture kit) and the Aethyr shaders that draw them (everglade/workshop/gpu/temple.coax,
// emitted for Metal by `coax metal` at build time). The host uploads three buffers once and
// issues the draws; it decides nothing about what is where or how it looks.
//
// An emitted shader takes its resources at fixed places (its .bind file lists them): the uniform
// floats at buffer 0, the mesh at 1, the records at 2, a lengths buffer at 29, and for the scene
// the mirror image at texture 0 with its sampler.
final class Place {
  let mesh: MTLBuffer, records: MTLBuffer, lengths: MTLBuffer
  var draws: [UInt32]
  var groups: [(first: Int, count: Int)] = []
  let sky: MTLRenderPipelineState, skyMirror: MTLRenderPipelineState
  let scene: MTLRenderPipelineState, sceneMirror: MTLRenderPipelineState, light: MTLRenderPipelineState
  let solid: MTLDepthStencilState, behind: MTLDepthStencilState, over: MTLDepthStencilState
  let smooth: MTLSamplerState, blank: MTLTexture
  var mirror: MTLTexture?, mirrorDepth: MTLTexture?

  init?(device: MTLDevice, color: MTLPixelFormat) {
    guard let state = plc_new() else { return nil }
    defer { free(state) }
    let vf = Int(plc_vertex_floats()) * Int(plc_vertices(state)), rf = Int(plc_record_floats()) * Int(plc_records(state)), dn = Int(plc_draws(state)) * 4
    draws = [UInt32](repeating: 0, count: dn)
    guard vf > 0, rf > 0, dn > 0,
          let m = device.makeBuffer(length: vf * 4, options: .storageModeShared),
          let r = device.makeBuffer(length: rf * 4, options: .storageModeShared),
          let l = device.makeBuffer(length: 64, options: .storageModeShared),
          plc_copy_mesh(state, m.contents().assumingMemoryBound(to: Float.self), Int32(vf)) == 1,
          plc_copy_records(state, r.contents().assumingMemoryBound(to: Float.self), Int32(rf)) == 1,
          plc_copy_draws(state, &draws, Int32(dn)) == 1 else { return nil }
    mesh = m; records = r; lengths = l
    for g in 0..<3 { groups.append((Int(plc_group_first(state, Int32(g))), Int(plc_group_count(state, Int32(g))))) }
    func library(_ name: String) -> MTLLibrary? { Bundle.main.url(forResource: name, withExtension: "metallib").flatMap { try? device.makeLibrary(URL: $0) } }
    guard let skyLib = library("temple-sky"), let sceneLib = library("temple-scene"), let lightLib = library("temple-light") else { return nil }
    // Each pair twice: once for the picture, once for the mirror image (linear colour, sixteen-bit).
    func pipeline(_ lib: MTLLibrary, _ pair: String, _ format: MTLPixelFormat, add: Bool) -> MTLRenderPipelineState? {
      let d = MTLRenderPipelineDescriptor()
      d.vertexFunction = lib.makeFunction(name: "temple_\(pair)_vs"); d.fragmentFunction = lib.makeFunction(name: "temple_\(pair)_fs")
      d.colorAttachments[0].pixelFormat = format; d.depthAttachmentPixelFormat = .depth32Float
      if add {
        let a = d.colorAttachments[0]!
        a.isBlendingEnabled = true; a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = .one; a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .one
      }
      return try? device.makeRenderPipelineState(descriptor: d)
    }
    guard let a = pipeline(skyLib, "sky", color, add: false), let b = pipeline(skyLib, "sky", .rgba16Float, add: false),
          let c = pipeline(sceneLib, "scene", color, add: false), let e = pipeline(sceneLib, "scene", .rgba16Float, add: false),
          let f = pipeline(lightLib, "light", color, add: true) else { return nil }
    sky = a; skyMirror = b; scene = c; sceneMirror = e; light = f
    func depth(_ compare: MTLCompareFunction, _ write: Bool) -> MTLDepthStencilState? {
      let d = MTLDepthStencilDescriptor(); d.depthCompareFunction = compare; d.isDepthWriteEnabled = write
      return device.makeDepthStencilState(descriptor: d)
    }
    let sd = MTLSamplerDescriptor(); sd.minFilter = .linear; sd.magFilter = .linear
    let bd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba16Float, width: 1, height: 1, mipmapped: false)
    guard let s1 = depth(.less, true), let s2 = depth(.always, false), let s3 = depth(.less, false),
          let sm = device.makeSamplerState(descriptor: sd), let bl = device.makeTexture(descriptor: bd) else { return nil }
    solid = s1; behind = s2; over = s3; smooth = sm; blank = bl
  }

  func bind(_ enc: MTLRenderCommandEncoder, _ u: inout [Float], image: MTLTexture) {
    enc.setVertexBytes(&u, length: u.count * 4, index: 0); enc.setFragmentBytes(&u, length: u.count * 4, index: 0)
    enc.setVertexBuffer(mesh, offset: 0, index: 1); enc.setFragmentBuffer(mesh, offset: 0, index: 1)
    enc.setVertexBuffer(records, offset: 0, index: 2); enc.setFragmentBuffer(records, offset: 0, index: 2)
    enc.setVertexBuffer(lengths, offset: 0, index: 29); enc.setFragmentBuffer(lengths, offset: 0, index: 29)
    enc.setVertexTexture(image, index: 0); enc.setFragmentTexture(image, index: 0)
    enc.setVertexSamplerState(smooth, index: 0); enc.setFragmentSamplerState(smooth, index: 0)
    // The kit's triangles are counter-clockwise seen from outside; the shader turns a normal
    // round for the side seen from behind.
    enc.setCullMode(.none); enc.setFrontFacing(.counterClockwise)
  }
  func group(_ enc: MTLRenderCommandEncoder, _ g: Int) {
    for k in groups[g].first..<groups[g].first + groups[g].count {
      let first = Int(draws[k * 4]), count = Int(draws[k * 4 + 1]), record = Int(draws[k * 4 + 2]), n = Int(draws[k * 4 + 3])
      if count > 0 && n > 0 { enc.drawPrimitives(type: .triangle, vertexStart: first, vertexCount: count, instanceCount: n, baseInstance: record) }
    }
  }
  // What stands on the floor, upside down, into the mirror image.
  func reflect(_ cmd: MTLCommandBuffer, device: MTLDevice, uniforms: [Float], width: Int, height: Int) {
    if mirror == nil || mirror!.width != width || mirror!.height != height {
      let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba16Float, width: width, height: height, mipmapped: false)
      cd.usage = [.renderTarget, .shaderRead]; cd.storageMode = .private
      let dd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: width, height: height, mipmapped: false)
      dd.usage = [.renderTarget]; dd.storageMode = .private
      mirror = device.makeTexture(descriptor: cd); mirrorDepth = device.makeTexture(descriptor: dd)
    }
    guard let image = mirror, let z = mirrorDepth else { return }
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = image; pass.colorAttachments[0].loadAction = .dontCare; pass.colorAttachments[0].storeAction = .store
    pass.depthAttachment.texture = z; pass.depthAttachment.loadAction = .clear; pass.depthAttachment.clearDepth = 1; pass.depthAttachment.storeAction = .dontCare
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    var u = uniforms; u[3] = 1
    bind(enc, &u, image: blank)
    // Upside down, a triangle that faced you turns the other way round.
    enc.setFrontFacing(.clockwise)
    enc.setRenderPipelineState(skyMirror); enc.setDepthStencilState(behind)
    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    enc.setRenderPipelineState(sceneMirror); enc.setDepthStencilState(solid)
    group(enc, 0)
    enc.endEncoding()
  }
  // The night, what stands, and the floor over the mirror image.
  func draw(_ enc: MTLRenderCommandEncoder, uniforms: [Float]) {
    var u = uniforms; u[3] = 0
    bind(enc, &u, image: mirror ?? blank)
    enc.setRenderPipelineState(sky); enc.setDepthStencilState(behind)
    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    enc.setRenderPipelineState(scene); enc.setDepthStencilState(solid)
    group(enc, 0); group(enc, 1)
  }
  // What is only light, added over everything that stands.
  func glow(_ enc: MTLRenderCommandEncoder, uniforms: [Float]) {
    var u = uniforms; u[3] = 0
    bind(enc, &u, image: blank)
    enc.setRenderPipelineState(light); enc.setDepthStencilState(over)
    group(enc, 2)
  }
}

// Your model on the GPU: the pack's mesh and layers, the baked clips, and this frame's records.
final class Model {
  static let sheet = 512
  let state: UnsafeMutableRawPointer
  let vertices: MTLBuffer, indices: MTLBuffer, bake: MTLBuffer, records: MTLBuffer, sheets: MTLTexture
  let pipeline: MTLRenderPipelineState, depth: MTLDepthStencilState, sampler: MTLSamplerState
  var draws = [UInt32](repeating: 0, count: Int(egm_records()) * 4)
  var count = 0

  init?(device: MTLDevice, library: MTLLibrary, color: MTLPixelFormat, pack: Pack) {
    let st = egm_new()!
    state = st
    let named = pack.catalog.withUnsafeBytes { egm_catalog(st, $0.bindMemory(to: UInt8.self).baseAddress, Int32($0.count)) }
    guard named == 1,
          let v = pack.vertices.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }),
          let i = pack.indices.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }),
          let b = device.makeBuffer(length: Int(egm_bake_floats()) * 4, options: .storageModeShared),
          let r = device.makeBuffer(length: Int(egm_records()) * Int(egm_record_floats()) * 4, options: .storageModeShared),
          egm_bake(st, b.contents().assumingMemoryBound(to: Float.self)) == 1 else { return nil }
    vertices = v; indices = i; bake = b; records = r
    // Every layer drawn to one size, so they stack as one array; the pack's are 512 a side at most.
    let side = Model.sheet
    let td = MTLTextureDescriptor()
    td.textureType = .type2DArray; td.pixelFormat = .rgba8Unorm_srgb; td.width = side; td.height = side
    td.arrayLength = pack.layers.count; td.mipmapLevelCount = 1 + Int(log2(Double(side))); td.usage = [.shaderRead]
    guard let t = device.makeTexture(descriptor: td) else { return nil }
    var pixels = [UInt8](repeating: 0, count: side * side * 4)
    let info = CGImageAlphaInfo.premultipliedLast.rawValue
    for (layer, data) in pack.layers.enumerated() {
      guard let src = CGImageSourceCreateWithData(data as CFData, nil), let image = CGImageSourceCreateImageAtIndex(src, 0, nil),
            let ctx = CGContext(data: &pixels, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: info) else { return nil }
      ctx.clear(CGRect(x: 0, y: 0, width: side, height: side))
      ctx.interpolationQuality = .high
      ctx.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side))
      // CoreGraphics multiplies colour by alpha; the shader wants it straight.
      for p in stride(from: 0, to: pixels.count, by: 4) {
        let a = Int(pixels[p + 3])
        if a > 0 && a < 255 { for c in 0..<3 { pixels[p + c] = UInt8(min(255, Int(pixels[p + c]) * 255 / a)) } }
      }
      t.replace(region: MTLRegionMake2D(0, 0, side, side), mipmapLevel: 0, slice: layer, withBytes: pixels, bytesPerRow: side * 4, bytesPerImage: 0)
    }
    if let queue = device.makeCommandQueue(), let cmd = queue.makeCommandBuffer(), let blit = cmd.makeBlitCommandEncoder() {
      blit.generateMipmaps(for: t); blit.endEncoding(); cmd.commit(); cmd.waitUntilCompleted()
    }
    sheets = t
    let d = MTLRenderPipelineDescriptor()
    d.vertexFunction = library.makeFunction(name: "model_vs"); d.fragmentFunction = library.makeFunction(name: "model_fs")
    d.colorAttachments[0].pixelFormat = color; d.depthAttachmentPixelFormat = .depth32Float
    let dd = MTLDepthStencilDescriptor(); dd.depthCompareFunction = .less; dd.isDepthWriteEnabled = true
    let sd = MTLSamplerDescriptor(); sd.minFilter = .linear; sd.magFilter = .linear; sd.mipFilter = .linear
    guard let ps = try? device.makeRenderPipelineState(descriptor: d), let ds = device.makeDepthStencilState(descriptor: dd), let ss = device.makeSamplerState(descriptor: sd) else { return nil }
    pipeline = ps; depth = ds; sampler = ss
  }

  // This frame's records from where you stand.
  func step(_ scene: UnsafeMutableRawPointer, dt: Float) {
    var you = [Float](repeating: 0, count: 5)
    count = eg_you(scene, &you) == 1 ? Int(egm_frame(state, you, dt, records.contents().assumingMemoryBound(to: Float.self), &draws)) : 0
  }
  func draw(_ enc: MTLRenderCommandEncoder, camera: [Float]) {
    if count == 0 { return }
    var u = camera
    enc.setRenderPipelineState(pipeline); enc.setDepthStencilState(depth); enc.setCullMode(.none)
    enc.setVertexBytes(&u, length: u.count * 4, index: 0); enc.setFragmentBytes(&u, length: u.count * 4, index: 0)
    enc.setVertexBuffer(vertices, offset: 0, index: 1); enc.setVertexBuffer(records, offset: 0, index: 2); enc.setVertexBuffer(bake, offset: 0, index: 3)
    enc.setFragmentTexture(sheets, index: 0); enc.setFragmentSamplerState(sampler, index: 0)
    for k in 0..<count {
      let first = Int(draws[k * 4]), n = Int(draws[k * 4 + 1]), record = Int(draws[k * 4 + 2])
      if n <= 0 || (first + n) * 4 > indices.length { continue }
      enc.drawIndexedPrimitives(type: .triangle, indexCount: n, indexType: .uint32, indexBuffer: indices, indexBufferOffset: first * 4, instanceCount: 1, baseVertex: 0, baseInstance: record)
    }
  }
}

final class WorkshopView: MTKView, MTKViewDelegate {
  let state = eg_new()!
  var place: Place!
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
  var last = CACurrentMediaTime()
  var time: Float = 0

  init(frame: CGRect) {
    super.init(frame: frame, device: MTLCreateSystemDefaultDevice())
    guard let device else { fatalError("Everglade needs a Metal device.") }
    colorPixelFormat = .bgra8Unorm
    preferredFramesPerSecond = NSScreen.main?.maximumFramesPerSecond ?? 60
    clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    depthStencilPixelFormat = .depth32Float; clearDepth = 1
    let url = Bundle.main.url(forResource: "default", withExtension: "metallib")!
    let lib = try! device.makeLibrary(URL: url)
    let b = MTLRenderPipelineDescriptor()
    // The place: the temple's parts and Aethyr's shaders. Without them there is nowhere to stand.
    guard let pl = Place(device: device, color: colorPixelFormat) else {
      FileHandle.standardError.write(Data("Everglade: the temple did not build or its shaders are missing. Rebuild with apps/everglade-desktop/build.sh.\n".utf8))
      exit(1)
    }
    place = pl
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
    render(pass, size: drawableSize, scale: Float(window?.backingScaleFactor ?? 2), dt: dt) { $0.present(drawable) }
  }

  func render(_ pass: MTLRenderPassDescriptor, size: CGSize, scale: Float, dt: Float, then: (MTLCommandBuffer) -> Void) {
    time += dt
    let n = eg_frame(state, vertices.contents().assumingMemoryBound(to: Float.self), Float(size.width) / scale, Float(size.height) / scale, scale, dt)
    var u: [Float] = [Float(size.width), Float(size.height), time, 0]
    model?.step(state, dt: dt)
    // The view, the picture's size, the clock and where you stand: the place's uniforms, and the
    // first twenty floats Model.metal's MU.
    var camera = [Float](repeating: 0, count: 24)
    let seen = eg_camera(state, Float(size.width) / scale, Float(size.height) / scale, scale, &camera) == 1
    camera[16] = Float(size.width); camera[17] = Float(size.height); camera[18] = time
    var you = [Float](repeating: 0, count: 5)
    if eg_you(state, &you) == 1 { camera[20] = you[0]; camera[21] = you[1]; camera[22] = you[2] }
    guard let queue = device?.makeCommandQueue(), let cmd = queue.makeCommandBuffer() else { return }
    // The place's uniforms (everglade/workshop/gpu/temple.coax): size, clock, mirror flag, the eye
    // and its right, up and forward with the focal length, and where you stand.
    var pu = [Float](repeating: 0, count: 24)
    pu[0] = Float(size.width); pu[1] = Float(size.height); pu[2] = time
    for k in 0..<3 { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = camera[20 + k] }
    pu[19] = camera[15]
    if seen, let device { place.reflect(cmd, device: device, uniforms: pu, width: Int(size.width), height: Int(size.height)) }
    guard let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { return }
    if seen { place.draw(enc, uniforms: pu) }
    enc.setDepthStencilState(flat)
    if n > 0 {
      enc.setRenderPipelineState(strokes)
      enc.setFragmentTexture(faces, index: 0); enc.setFragmentSamplerState(faceSampler, index: 0)
      enc.setVertexBuffer(vertices, offset: 0, index: 0)
      enc.setVertexBytes(&u, length: 16, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    if seen { model?.draw(enc, camera: camera); place.glow(enc, uniforms: pu) }
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
    cmd.waitUntilCompleted()
  }

  // What the panel asked of the host this frame. The host holds no logic here: it runs the
  // context program the panel asks for and appends the lines the panel hands it.
  func serve() {
    let request = nxp_take_request(next)
    if request != 0 && !fetching {
      fetching = true
      let tool = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/everglade-next")
      let process = Process(), out = Pipe(), err = Pipe()
      process.executableURL = tool
      // The checkout the app was built from is where a worktree for an issue branches off.
      let checkout = (Bundle.main.url(forResource: "repo", withExtension: "txt").flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
      let working = request == 3
      process.arguments = working ? ["work", String(nxp_chosen(next)), "--repo-dir", checkout, "--owner", String(getpid())] : request == 2 ? ["context", "--refresh"] : ["context"]
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
    window.contentView = WorkshopView(frame: frame)
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
  if let w = args.firstIndex(of: "--next"), w + 1 < args.count, let v = Float(args[w + 1]), v > 0, v <= 600 { view.nextWait = v }
  if let r = args.firstIndex(of: "--reply"), r + 1 < args.count { view.nextReply = args[r + 1] }
  if let r = args.firstIndex(of: "--runs"), r + 1 < args.count, let v = Int32(args[r + 1]), v > 0, v <= 16 { view.nextRuns = v }
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
