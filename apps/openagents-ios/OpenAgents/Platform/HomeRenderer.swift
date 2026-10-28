import UIKit
import Metal
import CoreText

private struct ViewUniform { var size: SIMD2<Float>; var time: Float; var scale: Float }
private struct BarUniform { var rect: SIMD4<Float>; var state: SIMD4<Float> }
private struct HauntUniform { var rows: (SIMD4<Float>, SIMD4<Float>, SIMD4<Float>, SIMD4<Float>); var eyeTime: SIMD4<Float>; var options: SIMD4<Float> }
private struct CastUniform { var rows: (SIMD4<Float>, SIMD4<Float>, SIMD4<Float>, SIMD4<Float>); var size: SIMD2<Float>; var scale: Float; var pad: Float }

/// The Grid's cast for one frame: the view matrix by rows
/// (metareal_grid_view) and a call that writes every joint's palette entry
/// into the buffer it is given (metareal_cast_pose, metareal_cast_robots_pose).
struct CastFrame {
    var view: [Float]
    var pose: (coax_v_t_Joint) -> Bool
}

/// Cast.bin's buffers on the GPU, at the offsets metareal_cast_open admitted,
/// and the character's texture layers.
final class CastAssets {
    let cast: coax_t_Cast
    fileprivate let bodyVertices: MTLBuffer, bodyIndices: MTLBuffer
    fileprivate let robotVertices: MTLBuffer, robotFaces: MTLBuffer, robotLines: MTLBuffer
    fileprivate let sheets: MTLTexture
    fileprivate var palettes: [MTLBuffer] = []
    /// The terminal ahead of where you arrive (metareal_grid_props): one
    /// rigid mesh on an identity joint, drawn like a robot.
    fileprivate let propVertices: MTLBuffer, propFaces: MTLBuffer, propLines: MTLBuffer, still: MTLBuffer
    fileprivate let propFaceCount: Int, propLineCount: Int
    init(device: MTLDevice, queue: MTLCommandQueue, bytes: Data, cast: coax_t_Cast, layers: [URL]) throws {
        guard cast.ok, cast.size == bytes.count else { throw HomeError.resource("Cast.bin") }
        self.cast = cast
        func buffer(_ b: coax_t_Buffer, _ stride: Int) throws -> MTLBuffer {
            let made = bytes.withUnsafeBytes { device.makeBuffer(bytes: $0.baseAddress! + Int(b.offset), length: Int(b.count) * stride, options: .storageModeShared) }
            guard let made else { throw HomeError.resource("Cast buffers") }
            return made
        }
        bodyVertices = try buffer(cast.body_vertices, 64); bodyIndices = try buffer(cast.body_indices, 4)
        robotVertices = try buffer(cast.robot_vertices, 64); robotFaces = try buffer(cast.robot_faces, 4)
        robotLines = try buffer(cast.robot_lines, 4)
        let side = 1024
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm_srgb, width: side, height: side, mipmapped: true)
        d.textureType = .type2DArray; d.arrayLength = layers.count; d.usage = .shaderRead; d.storageMode = .shared
        guard let sheets = device.makeTexture(descriptor: d) else { throw HomeError.resource("Cast textures") }
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        for (slice, url) in layers.enumerated() {
            guard let image = UIImage(contentsOfFile: url.path)?.cgImage else { throw HomeError.resource(url.lastPathComponent) }
            let drawn = pixels.withUnsafeMutableBytes { data -> Bool in
                guard let c = CGContext(data: data.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                        space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return false }
                c.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side)); return true
            }
            guard drawn else { throw HomeError.resource(url.lastPathComponent) }
            pixels.withUnsafeBytes { sheets.replace(region: MTLRegionMake2D(0, 0, side, side), mipmapLevel: 0, slice: slice, withBytes: $0.baseAddress!, bytesPerRow: side * 4, bytesPerImage: side * side * 4) }
        }
        guard let command = queue.makeCommandBuffer(), let blit = command.makeBlitCommandEncoder() else { throw HomeError.resource("Cast textures") }
        blit.generateMipmaps(for: sheets); blit.endEncoding(); command.commit()
        self.sheets = sheets
        var vertices = [UInt8](repeating: 0, count: Int(metareal_grid_prop_vertices()) * 64)
        var faces = [UInt32](repeating: 0, count: Int(metareal_grid_prop_faces()))
        var lines = [UInt32](repeating: 0, count: Int(metareal_grid_prop_lines()))
        let built = vertices.withUnsafeMutableBufferPointer { v in faces.withUnsafeMutableBufferPointer { f in lines.withUnsafeMutableBufferPointer { l in
            metareal_grid_props(coax_v_u8(data: v.baseAddress, count: Int64(v.count)), coax_v_u32(data: f.baseAddress, count: Int64(f.count)), coax_v_u32(data: l.baseAddress, count: Int64(l.count)))
        } } }
        var identity: [Float] = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0]
        guard built, let pv = device.makeBuffer(bytes: vertices, length: vertices.count, options: .storageModeShared),
              let pf = device.makeBuffer(bytes: faces, length: faces.count * 4, options: .storageModeShared),
              let pl = device.makeBuffer(bytes: lines, length: lines.count * 4, options: .storageModeShared),
              let still = device.makeBuffer(bytes: &identity, length: identity.count * 4, options: .storageModeShared) else { throw HomeError.resource("Grid props") }
        propVertices = pv; propFaces = pf; propLines = pl; self.still = still
        propFaceCount = faces.count; propLineCount = lines.count / 2
        let length = Int(metareal_cast_palette(cast)) * MemoryLayout<coax_t_Joint>.stride
        for _ in 0..<3 {
            guard let palette = device.makeBuffer(length: length, options: .storageModeShared) else { throw HomeError.resource("Cast palette") }
            palettes.append(palette)
        }
    }
}

/// Metal resource binding and bulk transfers only. Product visual functions are
/// emitted from metareal/light.coax; scene packets come from home.coax or
/// grid.coax.
final class HomeRenderer {
    private let layer: CAMetalLayer
    private let queue: MTLCommandQueue
    private let sky: MTLRenderPipelineState
    private let dots: MTLRenderPipelineState
    private let lines: MTLRenderPipelineState
    private let bloom: MTLRenderPipelineState
    private let composite: MTLRenderPipelineState
    private let bar: MTLRenderPipelineState
    private let slot: MTLRenderPipelineState
    private let hauntFaces: MTLRenderPipelineState
    private let hauntSky: MTLRenderPipelineState
    private let castFaces: MTLRenderPipelineState
    private let castLines: MTLRenderPipelineState
    private let flat: MTLDepthStencilState
    private let solid: MTLDepthStencilState
    private let behind: MTLDepthStencilState
    private var depth: MTLTexture?
    private let available = DispatchSemaphore(value: 3)
    private var ring = 0
    private var dotBuffers: [MTLBuffer] = []
    private var lineBuffers: [MTLBuffer] = []
    private var scene: MTLTexture?
    private var blurA: MTLTexture?
    private var blurB: MTLTexture?
    private var labelTexture: MTLTexture?
    private var labelKey = ""
    private var font: CTFont
    private var epoch: Double?
    var device: MTLDevice { gpu }
    var commandQueue: MTLCommandQueue { queue }
    private var gpu: MTLDevice

    init(layer: CAMetalLayer) throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw HomeError.resource("Metal unavailable") }
        self.layer = layer; self.gpu = device; self.queue = queue
        layer.device = device; layer.pixelFormat = .bgra8Unorm; layer.framebufferOnly = true; layer.isOpaque = true
        guard let source = Bundle.main.url(forResource: "HomeShaders", withExtension: "txt") else { throw HomeError.resource("HomeShaders.txt") }
        let library = try device.makeLibrary(source: String(contentsOf: source, encoding: .utf8), options: nil)
        // The scene pass has depth, for the cast; the screen passes don't.
        func pipeline(_ vertex: String, _ fragment: String, _ format: MTLPixelFormat, additive: Bool = false, alpha: Bool = false, brightest: Bool = false, scene: Bool = false) throws -> MTLRenderPipelineState {
            let d = MTLRenderPipelineDescriptor(); d.vertexFunction = library.makeFunction(name: vertex); d.fragmentFunction = library.makeFunction(name: fragment)
            d.colorAttachments[0].pixelFormat = format
            if scene { d.depthAttachmentPixelFormat = .depth32Float }
            if additive || alpha || brightest {
                let a = d.colorAttachments[0]!
                a.isBlendingEnabled = true; a.rgbBlendOperation = .add; a.alphaBlendOperation = .add
                a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = additive ? .one : .oneMinusSourceAlpha
                a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .oneMinusSourceAlpha
                // Where many lines meet, the brightest shows, not their sum.
                if brightest { a.rgbBlendOperation = .max; a.alphaBlendOperation = .max }
            }
            return try device.makeRenderPipelineState(descriptor: d)
        }
        sky = try pipeline("screen_vertex", "sky_fragment", .rgba16Float, scene: true)
        dots = try pipeline("dot_vertex", "dot_fragment", .rgba16Float, additive: true, scene: true)
        lines = try pipeline("line_vertex", "line_fragment", .rgba16Float, additive: true, scene: true)
        bloom = try pipeline("screen_vertex", "bloom_fragment", .rgba16Float)
        composite = try pipeline("screen_vertex", "composite_fragment", .bgra8Unorm)
        bar = try pipeline("bar_vertex", "bar_fragment", .bgra8Unorm, alpha: true)
        slot = try pipeline("slot_vertex", "slot_fragment", .bgra8Unorm, alpha: true)
        hauntFaces = try pipeline("haunt_vertex", "haunt_fragment", .rgba16Float, scene: true)
        hauntSky = try pipeline("screen_vertex", "haunt_sky_fragment", .rgba16Float, scene: true)
        castFaces = try pipeline("cast_vertex", "cast_fragment", .rgba16Float, scene: true)
        castLines = try pipeline("cast_line_vertex", "line_fragment", .rgba16Float, brightest: true, scene: true)
        func depthState(_ compare: MTLCompareFunction, write: Bool) throws -> MTLDepthStencilState {
            let d = MTLDepthStencilDescriptor(); d.depthCompareFunction = compare; d.isDepthWriteEnabled = write
            guard let state = device.makeDepthStencilState(descriptor: d) else { throw HomeError.resource("Metal depth") }
            return state
        }
        flat = try depthState(.always, write: false)
        solid = try depthState(.less, write: true)
        behind = try depthState(.lessEqual, write: false)
        for _ in 0..<3 {
            guard let a = device.makeBuffer(length: 8192 * MemoryLayout<MRDot>.stride, options: .storageModeShared), let b = device.makeBuffer(length: 8192 * MemoryLayout<MRLine>.stride, options: .storageModeShared) else { throw HomeError.resource("Metal buffers") }
            dotBuffers.append(a); lineBuffers.append(b)
        }
        guard let fontURL = Bundle.main.url(forResource: "Michroma-Regular", withExtension: "ttf") else { throw HomeError.resource("Michroma") }
        CTFontManagerRegisterFontsForURL(fontURL as CFURL, .process, nil)
        font = CTFontCreateWithName("Michroma" as CFString, 17, nil)
    }
    func resize(_ size: CGSize) {
        guard size.width > 0, size.height > 0 else { return }
        func texture(_ w: Int, _ h: Int) -> MTLTexture? {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba16Float, width: max(1,w), height: max(1,h), mipmapped: false)
            d.usage = [.renderTarget, .shaderRead]; d.storageMode = .private
            return device.makeTexture(descriptor: d)
        }
        scene = texture(Int(size.width), Int(size.height))
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .depth32Float, width: max(1, Int(size.width)), height: max(1, Int(size.height)), mipmapped: false)
        d.usage = .renderTarget; d.storageMode = .memoryless
        depth = gpu.makeTexture(descriptor: d)
        blurA = texture(Int(size.width)/4, Int(size.height)/4)
        blurB = texture(Int(size.width)/4, Int(size.height)/4)
    }
    private func rasterize(_ text: String, scale: Float) -> MTLTexture? {
        let f = CTFontCreateCopyWithAttributes(font, CGFloat(17*scale), nil, nil)
        let attributes: [NSAttributedString.Key: Any] = [.font: f, .kern: CGFloat(4*scale), .foregroundColor: UIColor.white]
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text.uppercased(), attributes: attributes))
        let bounds = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
        let w = max(1,Int(ceil(bounds.width))+16), h = max(1,Int(ceil(bounds.height))+16)
        var bytes = [UInt8](repeating: 0, count: w*h)
        let made = bytes.withUnsafeMutableBytes { data -> Bool in
            guard let c = CGContext(data: data.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue) else { return false }
            c.textPosition = CGPoint(x: 8-bounds.minX, y: 8-bounds.minY)
            CTLineDraw(line, c); return true
        }
        guard made else { return nil }
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: w, height: h, mipmapped: false)
        desc.usage = .shaderRead
        guard let texture = gpu.makeTexture(descriptor: desc) else { return nil }
        bytes.withUnsafeBytes { texture.replace(region: MTLRegionMake2D(0,0,w,h), mipmapLevel: 0, withBytes: $0.baseAddress!, bytesPerRow: w) }
        return texture
    }
    /// Draws `handle`'s scene, with the HUD bar from `bar` (the home's Start
    /// bar over the Grid's overview) or else from `handle`.
    func draw(handle: Int64, time: Double, cast assets: CastAssets? = nil, frame castFrame: CastFrame? = nil, bar barHandle: Int64? = nil, haunt: HauntAssets? = nil, houseView: [Float]? = nil) {
        #if DEBUG
        let capture = R0FrameCapture.shared
        let captureFrame = capture?.currentFrame ?? -1
        let encodeClock = capture == nil ? 0 : R0FrameCapture.clock()
        #endif
        guard available.wait(timeout: .now()) == .success else { return }
        guard let scene, let depth, let blurA, let blurB, let drawable = layer.nextDrawable(), let command = queue.makeCommandBuffer(), let rawBar = mr_bar(barHandle ?? handle)?.pointee else { available.signal(); return }
        let scale = Float(layer.contentsScale)
        let text: String = withUnsafePointer(to: rawBar.label) { pointer in pointer.withMemoryRebound(to: CChar.self, capacity: 33) { String(cString: $0) } }
        let key = "\(scale):\(text)"
        if key != labelKey { labelTexture = rasterize(text, scale: scale); labelKey = key }
        guard let labelTexture else { available.signal(); return }
        if epoch == nil { epoch = time }
        var view = ViewUniform(size: SIMD2(Float(scene.width),Float(scene.height)), time: Float(time - epoch!), scale: scale)
        let dotCount = mr_dot_count(handle), lineCount = mr_line_count(handle)
        let dotBuffer = dotBuffers[ring], lineBuffer = lineBuffers[ring]; ring = (ring+1)%3
        #if DEBUG
        let copyClock = capture == nil ? 0 : R0FrameCapture.clock()
        #endif
        if dotCount > 0, let source = mr_dots(handle) { memcpy(dotBuffer.contents(),source,dotCount*MemoryLayout<MRDot>.stride) }
        if lineCount > 0, let source = mr_lines(handle) { memcpy(lineBuffer.contents(),source,lineCount*MemoryLayout<MRLine>.stride) }
        #if DEBUG
        capture?.elapsed("buffer_copy", frame: captureFrame, since: copyClock)
        #endif
        func encoder(_ texture: MTLTexture, depth: MTLTexture? = nil) -> MTLRenderCommandEncoder? {
            let pass = MTLRenderPassDescriptor(); pass.colorAttachments[0].texture = texture
            pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
            if let depth {
                pass.depthAttachment.texture = depth; pass.depthAttachment.loadAction = .clear
                pass.depthAttachment.clearDepth = 1; pass.depthAttachment.storeAction = .dontCare
            }
            return command.makeRenderCommandEncoder(descriptor: pass)
        }
        // Coax writes this frame's palette before the GPU reads it.
        var palette: MTLBuffer?
        if let assets, let castFrame, castFrame.view.count == 16 {
            let buffer = assets.palettes[ring]
            let joints = coax_v_t_Joint(data: buffer.contents().bindMemory(to: coax_t_Joint.self, capacity: buffer.length / MemoryLayout<coax_t_Joint>.stride),
                                        count: Int64(buffer.length / MemoryLayout<coax_t_Joint>.stride))
            #if DEBUG
            let poseClock = capture == nil ? 0 : R0FrameCapture.clock()
            #endif
            if castFrame.pose(joints) { palette = buffer }
            #if DEBUG
            capture?.elapsed("pose_and_palette_copy", frame: captureFrame, since: poseClock)
            #endif
        }
        guard let e = encoder(scene, depth: depth) else { available.signal(); return }
        e.setDepthStencilState(flat)
        e.setRenderPipelineState(haunt == nil ? sky : hauntSky); e.setFragmentBytes(&view,length: MemoryLayout<ViewUniform>.stride,index:0)
        e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6)
        e.setRenderPipelineState(lines); e.setVertexBuffer(lineBuffer,offset:0,index:0); e.setVertexBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:1)
        if lineCount > 0 { e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6,instanceCount:lineCount) }
        e.setRenderPipelineState(dots); e.setVertexBuffer(dotBuffer,offset:0,index:0)
        if dotCount > 0 { e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6,instanceCount:dotCount) }
        if let haunt, let houseView { drawHaunt(e, haunt, houseView, ring, time) }
        else if let assets, let castFrame, let palette { drawCast(e, assets, castFrame.view, palette, view) }
        e.endEncoding()
        func blurPass(_ input: MTLTexture, _ output: MTLTexture, _ direction: SIMD2<Float>) {
            guard let e = encoder(output) else { return }; var direction = direction
            e.setRenderPipelineState(bloom); e.setFragmentTexture(input,index:0); e.setFragmentBytes(&direction,length:MemoryLayout<SIMD2<Float>>.stride,index:0)
            e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6); e.endEncoding()
        }
        blurPass(scene,blurA,SIMD2(1/Float(blurA.width),0)); blurPass(blurA,blurB,SIMD2(0,1/Float(blurB.height)))
        guard let final = encoder(drawable.texture) else { available.signal(); return }
        final.setRenderPipelineState(composite); final.setFragmentTexture(scene,index:0); final.setFragmentTexture(blurB,index:1)
        final.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6)
        // A scene without a HUD bar (the Grid) draws none.
        if rawBar.width > 0 {
            var button = BarUniform(rect: SIMD4(rawBar.x,rawBar.y,rawBar.width,rawBar.height),state: SIMD4(rawBar.press,rawBar.flash,0,0))
            final.setRenderPipelineState(bar); final.setVertexBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0); final.setVertexBytes(&button,length:MemoryLayout<BarUniform>.stride,index:1)
            final.setFragmentBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0); final.setFragmentBytes(&button,length:MemoryLayout<BarUniform>.stride,index:1); final.setFragmentTexture(labelTexture,index:0)
            final.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6)
        }
        // The hotbar's slots (the Grid's dance button).
        if let slots = mr_slots(handle) {
            final.setRenderPipelineState(slot); final.setVertexBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0); final.setFragmentBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0)
            for i in 0..<mr_slot_count(handle) {
                let s = slots[i]
                var u = BarUniform(rect: SIMD4(s.x,s.y,s.width,s.height),state: SIMD4(s.press,s.flash,s.selected,s.icon))
                final.setVertexBytes(&u,length:MemoryLayout<BarUniform>.stride,index:1); final.setFragmentBytes(&u,length:MemoryLayout<BarUniform>.stride,index:1)
                final.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6)
            }
        }
        final.endEncoding()
        let semaphore = available
        command.addCompletedHandler { buffer in
            if let error = buffer.error { NSLog("Metareal GPU: %@",error.localizedDescription) }
            #if DEBUG
            capture?.gpu(buffer, frame: captureFrame)
            #endif
            semaphore.signal()
        }
        command.present(drawable); command.commit()
        #if DEBUG
        capture?.elapsed("render_encode", frame: captureFrame, since: encodeClock)
        #endif
    }
    private func drawHaunt(_ e: MTLRenderCommandEncoder, _ assets: HauntAssets, _ m: [Float], _ frame: Int, _ time: Double) {
        guard m.count == 16 else { return }
        let instances = assets.instances[frame], lights = assets.lightBuffers[frame]
        let count = metareal_haunt_instances(assets.state, time, coax_v_t_HauntInstance(data: instances.contents().assumingMemoryBound(to: coax_t_HauntInstance.self), count: 4096), coax_v_t_HauntDraw(data: assets.drawPackets, count: 256))
        let lightCount = metareal_haunt_lights(assets.state, time, coax_v_t_HauntGPUlight(data: lights.contents().assumingMemoryBound(to: coax_t_HauntGPUlight.self), count: 64))
        let eye = assets.state.eye
        var u = HauntUniform(rows: (SIMD4(m[0],m[1],m[2],m[3]), SIMD4(m[4],m[5],m[6],m[7]), SIMD4(m[8],m[9],m[10],m[11]), SIMD4(m[12],m[13],m[14],m[15])), eyeTime: SIMD4(eye.x,eye.y,eye.z,Float(time)), options: SIMD4(Float(lightCount),time < assets.state.flash_until ? 1 : 0,0,0))
        e.setRenderPipelineState(hauntFaces); e.setDepthStencilState(solid); e.setCullMode(.none)
        e.setVertexBuffer(assets.vertices, offset: 0, index: 0); e.setVertexBuffer(instances, offset: 0, index: 1)
        e.setVertexBytes(&u, length: MemoryLayout<HauntUniform>.stride, index: 2)
        e.setFragmentBytes(&u, length: MemoryLayout<HauntUniform>.stride, index: 0)
        e.setFragmentBuffer(lights, offset: 0, index: 1); e.setFragmentTexture(assets.sheets, index: 0)
        for i in 0..<Int(count) { let d = assets.drawPackets[i]
            e.drawIndexedPrimitives(type: .triangle, indexCount: Int(d.count), indexType: .uint32, indexBuffer: assets.indices, indexBufferOffset: Int(d.first)*4, instanceCount: Int(d.instances), baseVertex: 0, baseInstance: Int(d.base))
        }
    }
    /// You, then each robot's faces and line art, each with its own run of
    /// the palette: yours first, then `bones` joints per robot.
    private func drawCast(_ e: MTLRenderCommandEncoder, _ assets: CastAssets, _ m: [Float], _ palette: MTLBuffer, _ screen: ViewUniform) {
        let joint = MemoryLayout<coax_t_Joint>.stride
        var u = CastUniform(rows: (SIMD4(m[0], m[1], m[2], m[3]), SIMD4(m[4], m[5], m[6], m[7]), SIMD4(m[8], m[9], m[10], m[11]), SIMD4(m[12], m[13], m[14], m[15])),
                            size: screen.size, scale: screen.scale, pad: 0)
        let c = assets.cast
        e.setRenderPipelineState(castFaces); e.setDepthStencilState(solid)
        e.setVertexBytes(&u, length: MemoryLayout<CastUniform>.stride, index: 2)
        e.setFragmentTexture(assets.sheets, index: 0)
        e.setVertexBuffer(assets.propVertices, offset: 0, index: 0); e.setVertexBuffer(assets.still, offset: 0, index: 1)
        e.drawIndexedPrimitives(type: .triangle, indexCount: assets.propFaceCount, indexType: .uint32, indexBuffer: assets.propFaces, indexBufferOffset: 0)
        e.setVertexBuffer(assets.bodyVertices, offset: 0, index: 0); e.setVertexBuffer(palette, offset: 0, index: 1)
        e.drawIndexedPrimitives(type: .triangle, indexCount: Int(c.body_indices.count), indexType: .uint32, indexBuffer: assets.bodyIndices, indexBufferOffset: 0)
        let robots = Int(metareal_cast_robots(c))
        e.setVertexBuffer(assets.robotVertices, offset: 0, index: 0)
        for r in 0..<robots {
            e.setVertexBuffer(palette, offset: (Int(c.joints) + r * Int(c.bones)) * joint, index: 1)
            e.drawIndexedPrimitives(type: .triangle, indexCount: Int(c.robot_faces.count), indexType: .uint32, indexBuffer: assets.robotFaces, indexBufferOffset: 0)
        }
        e.setRenderPipelineState(castLines); e.setDepthStencilState(behind)
        e.setVertexBuffer(assets.propVertices, offset: 0, index: 0); e.setVertexBuffer(assets.still, offset: 0, index: 1)
        e.setVertexBuffer(assets.propLines, offset: 0, index: 3)
        e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: assets.propLineCount)
        e.setVertexBuffer(assets.robotVertices, offset: 0, index: 0)
        e.setVertexBuffer(assets.robotLines, offset: 0, index: 3)
        for r in 0..<robots {
            e.setVertexBuffer(palette, offset: (Int(c.joints) + r * Int(c.bones)) * joint, index: 1)
            e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: Int(c.robot_lines.count) / 2)
        }
    }
}
