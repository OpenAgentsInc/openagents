// Firm reason: 6 (Metal, CoreText and ImageIO have no C API; this holds no product logic).
// Everglade's hosts, what they share: the faces' atlas, the pack, the place and your model on the
// GPU. It is Metal and CoreText only, so the Mac host (main.swift here) and the iOS host
// (apps/everglade-mobile/ios/Sources/main.swift) both compile it.
import Foundation
import Metal
import CoreGraphics
import CoreText
import ImageIO

// The faces' atlas on the GPU: Faces.swift draws each mip; this uploads them.
func faceAtlas(_ device: MTLDevice) -> MTLTexture? {
  guard let dir = Bundle.main.url(forResource: "fonts", withExtension: nil), let levels = faceLevels(fonts: dir), let top = levels.first else { return nil }
  let td = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rg8Unorm, width: top.width, height: top.height, mipmapped: true)
  td.mipmapLevelCount = levels.count
  guard let texture = device.makeTexture(descriptor: td) else { return nil }
  for (level, l) in levels.enumerated() {
    texture.replace(region: MTLRegionMake2D(0, 0, l.width, l.height), mipmapLevel: level, withBytes: l.bytes, bytesPerRow: l.width * 2)
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
  // The web's character code (model.coax) passes megabytes by value, so every call into it needs
  // a deep stack. The Mac app links its main thread with one, and runs the calls where they are.
  // A host whose main thread has a small stack (iOS) sets this to run them on a thread that has one.
  static var deep: (() -> Void) -> Void = { $0() }

  init?(device: MTLDevice, library: MTLLibrary, color: MTLPixelFormat, pack: Pack) {
    var made: UnsafeMutableRawPointer?, named: Int32 = 0
    Model.deep {
      made = egm_new()
      if let m = made { named = pack.catalog.withUnsafeBytes { egm_catalog(m, $0.bindMemory(to: UInt8.self).baseAddress, Int32($0.count)) } }
    }
    guard let st = made else { return nil }
    state = st
    guard named == 1,
          let v = pack.vertices.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }),
          let i = pack.indices.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }),
          let b = device.makeBuffer(length: Int(egm_bake_floats()) * 4, options: .storageModeShared),
          let r = device.makeBuffer(length: Int(egm_records()) * Int(egm_record_floats()) * 4, options: .storageModeShared) else { return nil }
    var baked: Int32 = 0
    Model.deep { baked = egm_bake(st, b.contents().assumingMemoryBound(to: Float.self)) }
    guard baked == 1 else { return nil }
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
    guard eg_you(scene, &you) == 1 else { count = 0; return }
    Model.deep { self.count = Int(egm_frame(self.state, you, dt, self.records.contents().assumingMemoryBound(to: Float.self), &self.draws)) }
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

