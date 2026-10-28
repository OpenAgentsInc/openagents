import UIKit
import Metal
import CoreText

private struct ViewUniform { var size: SIMD2<Float>; var time: Float; var scale: Float }
private struct BarUniform { var rect: SIMD4<Float>; var state: SIMD4<Float> }

/// Metal resource binding and bulk transfers only. Product visual functions are
/// emitted from metareal/light.coax; scene packets come from home.coax.
final class HomeRenderer {
    private let layer: CAMetalLayer
    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let sky: MTLRenderPipelineState
    private let dots: MTLRenderPipelineState
    private let lines: MTLRenderPipelineState
    private let bloom: MTLRenderPipelineState
    private let composite: MTLRenderPipelineState
    private let bar: MTLRenderPipelineState
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

    init(layer: CAMetalLayer) throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw HomeError.resource("Metal unavailable") }
        self.layer = layer; self.device = device; self.queue = queue
        layer.device = device; layer.pixelFormat = .bgra8Unorm; layer.framebufferOnly = true; layer.isOpaque = true
        guard let source = Bundle.main.url(forResource: "HomeShaders", withExtension: "txt") else { throw HomeError.resource("HomeShaders.txt") }
        let library = try device.makeLibrary(source: String(contentsOf: source, encoding: .utf8), options: nil)
        func pipeline(_ vertex: String, _ fragment: String, _ format: MTLPixelFormat, additive: Bool = false, alpha: Bool = false) throws -> MTLRenderPipelineState {
            let d = MTLRenderPipelineDescriptor(); d.vertexFunction = library.makeFunction(name: vertex); d.fragmentFunction = library.makeFunction(name: fragment)
            d.colorAttachments[0].pixelFormat = format
            if additive || alpha {
                let a = d.colorAttachments[0]!
                a.isBlendingEnabled = true; a.rgbBlendOperation = .add; a.alphaBlendOperation = .add
                a.sourceRGBBlendFactor = .one; a.destinationRGBBlendFactor = additive ? .one : .oneMinusSourceAlpha
                a.sourceAlphaBlendFactor = .one; a.destinationAlphaBlendFactor = .oneMinusSourceAlpha
            }
            return try device.makeRenderPipelineState(descriptor: d)
        }
        sky = try pipeline("screen_vertex", "sky_fragment", .rgba16Float)
        dots = try pipeline("dot_vertex", "dot_fragment", .rgba16Float, additive: true)
        lines = try pipeline("line_vertex", "line_fragment", .rgba16Float, additive: true)
        bloom = try pipeline("screen_vertex", "bloom_fragment", .rgba16Float)
        composite = try pipeline("screen_vertex", "composite_fragment", .bgra8Unorm)
        bar = try pipeline("bar_vertex", "bar_fragment", .bgra8Unorm, alpha: true)
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
        guard let texture = device.makeTexture(descriptor: desc) else { return nil }
        bytes.withUnsafeBytes { texture.replace(region: MTLRegionMake2D(0,0,w,h), mipmapLevel: 0, withBytes: $0.baseAddress!, bytesPerRow: w) }
        return texture
    }
    func draw(handle: Int64, time: Double) {
        guard available.wait(timeout: .now()) == .success else { return }
        guard let scene, let blurA, let blurB, let drawable = layer.nextDrawable(), let command = queue.makeCommandBuffer(), let rawBar = mr_bar(handle)?.pointee else { available.signal(); return }
        let scale = Float(layer.contentsScale)
        let text: String = withUnsafePointer(to: rawBar.label) { pointer in pointer.withMemoryRebound(to: CChar.self, capacity: 33) { String(cString: $0) } }
        let key = "\(scale):\(text)"
        if key != labelKey { labelTexture = rasterize(text, scale: scale); labelKey = key }
        guard let labelTexture else { available.signal(); return }
        if epoch == nil { epoch = time }
        var view = ViewUniform(size: SIMD2(Float(scene.width),Float(scene.height)), time: Float(time - epoch!), scale: scale)
        let dotCount = mr_dot_count(handle), lineCount = mr_line_count(handle)
        let dotBuffer = dotBuffers[ring], lineBuffer = lineBuffers[ring]; ring = (ring+1)%3
        if dotCount > 0, let source = mr_dots(handle) { memcpy(dotBuffer.contents(),source,dotCount*MemoryLayout<MRDot>.stride) }
        if lineCount > 0, let source = mr_lines(handle) { memcpy(lineBuffer.contents(),source,lineCount*MemoryLayout<MRLine>.stride) }
        func encoder(_ texture: MTLTexture) -> MTLRenderCommandEncoder? {
            let pass = MTLRenderPassDescriptor(); pass.colorAttachments[0].texture = texture
            pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
            return command.makeRenderCommandEncoder(descriptor: pass)
        }
        guard let e = encoder(scene) else { available.signal(); return }
        e.setRenderPipelineState(sky); e.setFragmentBytes(&view,length: MemoryLayout<ViewUniform>.stride,index:0)
        e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6)
        e.setRenderPipelineState(lines); e.setVertexBuffer(lineBuffer,offset:0,index:0); e.setVertexBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:1)
        if lineCount > 0 { e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6,instanceCount:lineCount) }
        e.setRenderPipelineState(dots); e.setVertexBuffer(dotBuffer,offset:0,index:0)
        if dotCount > 0 { e.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6,instanceCount:dotCount) }
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
        var button = BarUniform(rect: SIMD4(rawBar.x,rawBar.y,rawBar.width,rawBar.height),state: SIMD4(rawBar.press,rawBar.flash,0,0))
        final.setRenderPipelineState(bar); final.setVertexBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0); final.setVertexBytes(&button,length:MemoryLayout<BarUniform>.stride,index:1)
        final.setFragmentBytes(&view,length:MemoryLayout<ViewUniform>.stride,index:0); final.setFragmentBytes(&button,length:MemoryLayout<BarUniform>.stride,index:1); final.setFragmentTexture(labelTexture,index:0)
        final.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:6); final.endEncoding()
        let semaphore = available
        command.addCompletedHandler { buffer in
            if let error = buffer.error { NSLog("Metareal GPU: %@",error.localizedDescription) }
            semaphore.signal()
        }
        command.present(drawable); command.commit()
    }
}
