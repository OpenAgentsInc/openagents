// The Zeron-fork chat screen (docs/desktop/coax-desktop.md): zeron/screen.coax
// compiled to C writes GPUI-style primitives (zeron/gpui.coax), drawn here by
// Zeron.metal. This file is GPUI's platform layer for it: the window's Metal
// view, input, and the atlas of Zeron's font (Geist, rasterized as
// gpui_macos/text_system.rs does) and Zeron's SVG icons. Based on Zeron by Wing, MIT.
import AppKit
import CoreText
import MetalKit

// Geist from the bundle (fonts/, SIL OFL 1.1, as Zeron bundles it).
func geist(_ weight: Int32) -> CGFont? {
  let name = weight == 1 ? "Geist-Medium" : "Geist"
  guard let url = Bundle.main.url(forResource: name, withExtension: "ttf", subdirectory: "fonts"),
        let data = try? Data(contentsOf: url) as CFData, let provider = CGDataProvider(data: data) else { return nil }
  return CGFont(provider)
}

// The atlas: per font, every glyph at four subpixel x offsets and five dilations
// (gpui_macos rasterize_glyph: alpha-only, smoothing with fill luminance
// level/4 when dilated), then every icon at twice its device size (gpui svg_renderer).
func zeronAtlas(_ device: MTLDevice, state: UnsafeMutableRawPointer, dpr: CGFloat) -> MTLTexture {
  let glyphs = Int(zr_glyph_count()), fonts = Int(zr_font_count()), icons = Int(zr_icon_count())
  let width = 4096
  var y0 = 0
  struct Slot { var font: CTFont; var cell: Int; var penX: Int; var penY: Int; var y0: Int; var cols: Int; var size: CGFloat }
  var slots: [Slot] = []
  for f in 0..<fonts {
    let size = CGFloat(zr_font_size(Int32(f)))
    let cg = geist(zr_font_weight(Int32(f)))!
    let font = CTFontCreateWithGraphicsFont(cg, size, nil, nil)
    let cell = Int(ceil(size * dpr * 2.4))
    let cols = width / cell
    let cells = glyphs * 4 * 5
    let slot = Slot(font: font, cell: cell, penX: Int(round(Double(cell) * 0.2)), penY: Int(round(Double(cell) * 0.7)), y0: y0, cols: cols, size: size)
    slots.append(slot)
    y0 += ((cells + cols - 1) / cols) * cell
  }
  var iconY = y0, iconX = 0, rowH = 0
  var tiles: [(Int, Int, Int)] = []
  for i in 0..<icons {
    let px = Int(ceil(CGFloat(zr_icon_size(Int32(i))) * dpr * 2))
    if iconX + px > width { iconX = 0; iconY += rowH; rowH = 0 }
    tiles.append((iconX, iconY, px)); iconX += px + 2; rowH = max(rowH, px + 2)
  }
  let height = iconY + rowH + 1
  let g = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width,
                    space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue)!
  for (f, s) in slots.enumerated() {
    var header: [Float] = [Float(CTFontGetAscent(s.font)), Float(CTFontGetDescent(s.font)), Float(s.cell), Float(s.penX), Float(s.penY), Float(s.y0), Float(s.cols)]
    var codes = (0..<glyphs).map { UniChar(truncatingIfNeeded: zr_glyph_code(Int32($0))) }
    var ids = [CGGlyph](repeating: 0, count: glyphs)
    CTFontGetGlyphsForCharacters(s.font, &codes, &ids, glyphs)
    var adv = [CGSize](repeating: .zero, count: glyphs)
    CTFontGetAdvancesForGlyphs(s.font, .horizontal, ids, &adv, glyphs)
    var advance = adv.map { Float($0.width) }
    // Kerning as CoreText shapes it (what GPUI's line layout gets from CTLine): a pair's width less its advances.
    var kern = [Float](repeating: 0, count: glyphs * glyphs)
    let attrs: [NSAttributedString.Key: Any] = [.font: s.font]
    for a in 1..<glyphs where a < 95 {
      for b in 1..<glyphs where b < 95 {
        let pair = String(UnicodeScalar(UInt8(32 + a))) + String(UnicodeScalar(UInt8(32 + b)))
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: pair, attributes: attrs))
        let w = Float(CTLineGetTypographicBounds(line, nil, nil, nil))
        let k = w - advance[a] - advance[b]
        if abs(k) > 0.001 { kern[a * glyphs + b] = k }
      }
    }
    header.withUnsafeBufferPointer { h in advance.withUnsafeMutableBufferPointer { ad in kern.withUnsafeBufferPointer { k in
      zr_set_font(state, Int32(f), h.baseAddress, ad.baseAddress, k.baseAddress) } } }
    g.setAllowsAntialiasing(true); g.setShouldAntialias(true)
    g.setAllowsFontSubpixelPositioning(true); g.setShouldSubpixelPositionFonts(true)
    g.setAllowsFontSubpixelQuantization(false); g.setShouldSubpixelQuantizeFonts(false)
    g.setTextDrawingMode(.fill)
    let font = CTFontCreateCopyWithAttributes(s.font, s.size, nil, nil)
    for dilation in 0..<5 {
      if dilation > 0 { g.setShouldSmoothFonts(true); g.setFillColor(gray: CGFloat(dilation) * 0.25, alpha: 1) }
      else { g.setShouldSmoothFonts(false); g.setFillColor(gray: 0, alpha: 1) }
      for variant in 0..<4 {
        for i in 0..<glyphs {
          let cell = (dilation * 4 + variant) * glyphs + i
          let cx = (cell % s.cols) * s.cell, cy = s.y0 + (cell / s.cols) * s.cell
          g.saveGState()
          g.clip(to: CGRect(x: cx, y: height - cy - s.cell, width: s.cell, height: s.cell))
          g.translateBy(x: CGFloat(cx + s.penX), y: CGFloat(height - cy - s.penY))
          g.scaleBy(x: dpr, y: dpr)
          var glyph = ids[i]
          var at = CGPoint(x: CGFloat(variant) / 4 / dpr, y: 0)
          CTFontDrawGlyphs(font, &glyph, &at, 1, g)
          g.restoreGState()
        }
      }
    }
  }
  // Icons: Zeron's SVGs, their 1em size replaced by the tile's, the alpha kept.
  for (i, t) in tiles.enumerated() {
    var name = [UInt8](repeating: 0, count: 64)
    let n = Int(zr_icon_name(Int32(i), &name, 64))
    let file = String(decoding: name[0..<n], as: UTF8.self)
    guard let url = Bundle.main.url(forResource: file, withExtension: "svg", subdirectory: "icons"),
          var svg = try? String(contentsOf: url, encoding: .utf8) else { continue }
    // Sized to the tile (a viewBox that isn't square fills it, which lands closest to usvg's pixmap).
    svg = svg.replacingOccurrences(of: " width=\"1em\" height=\"1em\"", with: "")
    svg = svg.replacingOccurrences(of: "<svg ", with: "<svg width=\"\(t.2)\" height=\"\(t.2)\" ")
    guard let image = NSImage(data: Data(svg.utf8)) else { continue }
    let rgba = CGContext(data: nil, width: t.2, height: t.2, bitsPerComponent: 8, bytesPerRow: t.2 * 4, space: CGColorSpaceCreateDeviceRGB(),
                         bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(cgContext: rgba, flipped: false)
    image.draw(in: NSRect(x: 0, y: 0, width: t.2, height: t.2))
    NSGraphicsContext.restoreGraphicsState()
    let src = rgba.data!.assumingMemoryBound(to: UInt8.self), dst = g.data!.assumingMemoryBound(to: UInt8.self)
    for y in 0..<t.2 { for x in 0..<t.2 { dst[(t.1 + y) * width + t.0 + x] = src[(y * t.2 + x) * 4 + 3] } }
    zr_set_icon(state, Int32(i), Float(t.0), Float(t.1), Float(t.2), Float(t.2))
  }
  zr_set_atlas(state, Float(width), Float(height))
  let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: width, height: height, mipmapped: false)
  let texture = device.makeTexture(descriptor: d)!
  texture.replace(region: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0, withBytes: g.data!, bytesPerRow: width)
  return texture
}

final class ZeronView: MTKView, MTKViewDelegate {
  let state = zr_new()!
  var queue: MTLCommandQueue!
  var pipeline: MTLRenderPipelineState!
  var atlas: MTLTexture?, atlasDpr: CGFloat = 0
  var vertices: [MTLBuffer] = [], slot = 0
  let inflight = DispatchSemaphore(value: 2)
  var last = CACurrentMediaTime()
  var beam = false

  init(frame: CGRect) {
    let device = MTLCreateSystemDefaultDevice()!
    super.init(frame: frame, device: device)
    colorPixelFormat = .bgra8Unorm
    preferredFramesPerSecond = 60
    delegate = self
    queue = device.makeCommandQueue()
    let library = try! device.makeDefaultLibrary(bundle: .main)
    let p = MTLRenderPipelineDescriptor()
    p.vertexFunction = library.makeFunction(name: "zr_vs"); p.fragmentFunction = library.makeFunction(name: "zr_fs")
    let v = MTLVertexDescriptor()
    for i in 0..<5 { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = Int(zr_vertex_floats()) * 4
    p.vertexDescriptor = v
    // GPUI's blending (metal_renderer.rs): straight alpha, source over.
    let a = p.colorAttachments[0]!
    a.pixelFormat = colorPixelFormat; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .add
    a.sourceRGBBlendFactor = .sourceAlpha; a.sourceAlphaBlendFactor = .one
    a.destinationRGBBlendFactor = .oneMinusSourceAlpha; a.destinationAlphaBlendFactor = .oneMinusSourceAlpha
    pipeline = try! device.makeRenderPipelineState(descriptor: p)
    let bytes = Int(zr_max_vertices()) * Int(zr_vertex_floats()) * 4
    vertices = (0..<2).map { _ in device.makeBuffer(length: bytes, options: .storageModeShared)! }
    addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect, .cursorUpdate], owner: self))
  }
  required init(coder: NSCoder) { fatalError() }

  override var acceptsFirstResponder: Bool { true }
  override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
  func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

  func draw(in view: MTKView) {
    guard let pass = currentRenderPassDescriptor, let drawable = currentDrawable else { return }
    render(pass, size: drawableSize, dpr: window?.backingScaleFactor ?? 2) { $0.present(drawable) }
  }

  func render(_ pass: MTLRenderPassDescriptor, size: CGSize, dpr: CGFloat, done: ((MTLCommandBuffer) -> Void)? = nil) {
    if atlas == nil || atlasDpr != dpr { atlas = zeronAtlas(device!, state: state, dpr: dpr); atlasDpr = dpr }
    let now = CACurrentMediaTime(), dt = Float(now - last); last = now
    inflight.wait()
    slot = (slot + 1) % 2
    let buf = vertices[slot]
    let n = zr_frame(state, buf.contents().assumingMemoryBound(to: Float.self), Float(size.width), Float(size.height), Float(dpr), dt)
    let want = zr_cursor(state) != 0
    if want != beam { beam = want; (want ? NSCursor.iBeam : NSCursor.arrow).set() }
    pass.colorAttachments[0].loadAction = .clear
    pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    guard let cmd = queue.makeCommandBuffer(), let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { inflight.signal(); return }
    if n > 0 {
      var viewport = SIMD2<Float>(Float(size.width), Float(size.height))
      enc.setRenderPipelineState(pipeline)
      enc.setVertexBuffer(buf, offset: 0, index: 0)
      enc.setVertexBytes(&viewport, length: 8, index: 1)
      enc.setFragmentTexture(atlas, index: 0)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    enc.endEncoding()
    cmd.addCompletedHandler { [inflight] _ in inflight.signal() }
    done?(cmd)
    cmd.commit()
  }

  // --shot PATH [--size W,H] [--click X,Y] [--type TEXT] [--frames N]: draw offscreen at scale 2
  // (clicking at device pixel X,Y at frame 30, typing TEXT at frame 45), write the last frame as a PNG, quit.
  func shot(_ path: String, width: Int, height: Int) {
    let args = CommandLine.arguments
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: colorPixelFormat, width: width, height: height, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    let target = device!.makeTexture(descriptor: d)!
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = target; pass.colorAttachments[0].storeAction = .store
    var frames = 0
    let last = args.firstIndex(of: "--frames").flatMap { Int(args[$0 + 1]) } ?? 60
    Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { timer in
      frames += 1
      if frames == 30, let i = args.firstIndex(of: "--click"), i + 1 < args.count {
        let xy = args[i + 1].split(separator: ",").compactMap { Float($0) }
        if xy.count == 2 { _ = zr_pointer(self.state, xy[0], xy[1], 0); _ = zr_pointer(self.state, xy[0], xy[1], 2) }
      }
      if frames == 45, let i = args.firstIndex(of: "--type"), i + 1 < args.count { self.typed(args[i + 1]) }
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

  func place(_ e: NSEvent) -> (Float, Float) {
    let p = convert(e.locationInWindow, from: nil), d = window?.backingScaleFactor ?? 2
    return (Float(p.x * d), Float((bounds.height - p.y) * d))
  }
  func pointer(_ e: NSEvent, _ phase: Int32) { let (x, y) = place(e); _ = zr_pointer(state, x, y, phase) }
  override func mouseDown(with e: NSEvent) { window?.makeFirstResponder(self); pointer(e, 0) }
  override func mouseDragged(with e: NSEvent) { pointer(e, 1) }
  override func mouseMoved(with e: NSEvent) { pointer(e, 1) }
  override func mouseUp(with e: NSEvent) { pointer(e, 2) }
  override func cursorUpdate(with event: NSEvent) { (beam ? NSCursor.iBeam : NSCursor.arrow).set() }

  func typed(_ s: String) { Array(s.utf8).withUnsafeBufferPointer { zr_text(state, $0.baseAddress, Int32($0.count)) } }
  func mods(_ e: NSEvent) -> Int32 {
    var m: Int32 = 0
    if e.modifierFlags.contains(.shift) { m |= 1 }
    if e.modifierFlags.contains(.control) { m |= 2 }
    if e.modifierFlags.contains(.option) { m |= 4 }
    if e.modifierFlags.contains(.command) { m |= 8 }
    return m
  }
  override func performKeyEquivalent(with e: NSEvent) -> Bool {
    if e.modifierFlags.contains(.command), e.charactersIgnoringModifiers == "v", zr_wants_text(state) != 0,
       let s = NSPasteboard.general.string(forType: .string) { typed(s); return true }
    return super.performKeyEquivalent(with: e)
  }
  override func keyDown(with e: NSEvent) {
    if e.keyCode == 36 && e.modifierFlags.contains(.shift) && zr_wants_text(state) != 0 { typed("\n"); return }
    if let k = ChatView.named[e.keyCode] { _ = zr_key(state, k, mods(e)); return }
    guard let s = e.characters, !s.isEmpty, !e.modifierFlags.contains(.command), !e.modifierFlags.contains(.control) else {
      if let c = e.charactersIgnoringModifiers?.unicodeScalars.first { _ = zr_key(state, Int32(c.value), mods(e)) }
      return
    }
    if zr_wants_text(state) != 0 { typed(s) }
  }
}
