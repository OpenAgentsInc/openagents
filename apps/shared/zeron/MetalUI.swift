// Firm reason: 6 (AppKit/CoreText glyph rasterization and Metal effects shared by desktop apps).
// Extracted from the OpenAgents desktop Zeron host; based on Zeron by Wing,
// MIT, and GPUI's platform rasterization. See NOTICE beside this file.
import AppKit
import CoreText
import MetalKit

// Geist and Geist Mono from the bundle (fonts/, SIL OFL 1.1, as Zeron bundles
// them): 0 Geist, 1 Geist Medium, 2 Geist SemiBold, 3 Geist Mono (zr_font_weight).
func geist(_ weight: Int32) -> CGFont? {
  let name = ["Geist", "Geist-Medium", "Geist-SemiBold", "GeistMono"][Int(min(max(weight, 0), 3))]
  guard let url = Bundle.main.url(forResource: name, withExtension: "ttf", subdirectory: "fonts"),
        let data = try? Data(contentsOf: url) as CFData, let provider = CGDataProvider(data: data) else { return nil }
  return CGFont(provider)
}

// The atlas: per font, every glyph at four subpixel x offsets and five dilations
// (gpui_macos rasterize_glyph: alpha-only, smoothing with fill luminance
// level/4 when dilated), then every icon at twice its device size (gpui svg_renderer).
// Then room for GPUI's glyph cache (gpui.coax): every other grapheme, shaped
// and rasterized when the text first uses it (ZeronAtlas.serve), mono ones
// below the icons, emoji in a second, color atlas.
struct ZeronAtlasAPI {
  let next_cluster: (UnsafeMutableRawPointer?) -> Int32
  let measured: (UnsafeMutableRawPointer?, Int32, Float, Int32) -> Void
  let next_cell: (UnsafeMutableRawPointer?) -> Int32
  let cell: (UnsafeMutableRawPointer?, Int32, UnsafeMutablePointer<Float>?) -> Int32
  let cell_ready: (UnsafeMutableRawPointer?, Int32) -> Void
  let cluster: (UnsafeMutableRawPointer?, Int32, UnsafeMutablePointer<UInt8>?, Int32) -> Int32
  let glyph_count: () -> Int32
  let font_count: () -> Int32
  let icon_count: () -> Int32
  let font_size: (Int32) -> Float
  let font_weight: (Int32) -> Int32
  let icon_size: (Int32) -> Float
  let glyph_code: (Int32) -> UInt32
  let set_font: (UnsafeMutableRawPointer?, Int32, UnsafePointer<Float>?, UnsafePointer<Float>?, UnsafePointer<Float>?) -> Void
  let icon_name: (Int32, UnsafeMutablePointer<UInt8>?, Int32) -> Int32
  let set_icon: (UnsafeMutableRawPointer?, Int32, Float, Float, Float, Float) -> Void
  let set_atlas: (UnsafeMutableRawPointer?, Float, Float) -> Void
  let set_dynamic: (UnsafeMutableRawPointer?, Float, Float, Float, Float, Int32, Int32, Float, Float) -> Void
}

final class ZeronAtlas {
  let api: ZeronAtlasAPI
  let mono: MTLTexture, color: MTLTexture
  let fonts: [CTFont], dpr: CGFloat, cell: Int, penX: Int, penY: Int
  init(api: ZeronAtlasAPI, mono: MTLTexture, color: MTLTexture, fonts: [CTFont], dpr: CGFloat, cell: Int, penX: Int, penY: Int) {
    self.api = api; self.mono = mono; self.color = color; self.fonts = fonts; self.dpr = dpr; self.cell = cell; self.penX = penX; self.penY = penY
  }

  // What the last frame asked for: each new grapheme's advance and whether
  // CoreText draws it in color (a color-glyph font, Apple Color Emoji), then
  // each wanted cell drawn as rasterize_glyph does.
  func serve(_ state: UnsafeMutableRawPointer) {
    var bytes = [UInt8](repeating: 0, count: 64)
    var guardCount = 0
    while case let slot = api.next_cluster(state), slot >= 0, guardCount < 600 {
      guardCount += 1
      let (line, _) = shape(state, slot, &bytes)
      let width = Float(CTLineGetTypographicBounds(line, nil, nil, nil))
      let runs = CTLineGetGlyphRuns(line) as! [CTRun]
      let isColor = runs.contains { run in
        let attrs = CTRunGetAttributes(run) as NSDictionary
        guard let f = attrs[kCTFontAttributeName] else { return false }
        return CTFontGetSymbolicTraits(f as! CTFont).contains(.traitColorGlyphs)
      }
      api.measured(state, slot, width, isColor ? 1 : 0)
    }
    var info = [Float](repeating: 0, count: 6)
    guardCount = 0
    while case let index = api.next_cell(state), index >= 0, guardCount < 3000 {
      guardCount += 1
      _ = api.cell(state, index, &info)
      let (line, _) = shape(state, Int32(info[0]), &bytes)
      let level = Int(info[1]), variant = Int(info[2]), isColor = info[3] > 0.5
      let tx = Int(info[4]), ty = Int(info[5])
      if isColor {
        let g = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4, space: CGColorSpaceCreateDeviceRGB(),
                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        draw(line, in: g, level: 0, variant: 0)
        // RGBA with premultiplied alpha to straight alpha (swap_rgba_pa_to_bgra, in RGBA order here)
        let px = g.data!.assumingMemoryBound(to: UInt8.self)
        for i in 0..<(side * side) {
          let a = Int(px[i * 4 + 3])
          if a > 0 && a < 255 { for k in 0..<3 { px[i * 4 + k] = UInt8(min(255, Int(px[i * 4 + k]) * 255 / a)) } }
        }
        color.replace(region: MTLRegionMake2D(tx, ty, side, side), mipmapLevel: 0, withBytes: g.data!, bytesPerRow: side * 4)
      } else {
        let g = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side, space: CGColorSpaceCreateDeviceGray(),
                          bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue)!
        draw(line, in: g, level: level, variant: variant)
        mono.replace(region: MTLRegionMake2D(tx, ty, side, side), mipmapLevel: 0, withBytes: g.data!, bytesPerRow: side)
      }
      api.cell_ready(state, index)
    }
  }
  var side: Int { cell }
  func shape(_ state: UnsafeMutableRawPointer, _ slot: Int32, _ bytes: inout [UInt8]) -> (CTLine, Int) {
    let code = Int(api.cluster(state, slot, &bytes, Int32(bytes.count)))
    let font = fonts[min(max(code / 256, 0), fonts.count - 1)], n = code % 256
    let text = String(decoding: bytes[0..<n], as: UTF8.self)
    let attrs: [NSAttributedString.Key: Any] = [.font: font, NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true]
    return (CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attrs)), n)
  }
  func draw(_ line: CTLine, in g: CGContext, level: Int, variant: Int) {
    g.setAllowsAntialiasing(true); g.setShouldAntialias(true)
    g.setAllowsFontSubpixelPositioning(true); g.setShouldSubpixelPositionFonts(true)
    g.setAllowsFontSubpixelQuantization(false); g.setShouldSubpixelQuantizeFonts(false)
    g.setTextDrawingMode(.fill)
    if level > 0 { g.setShouldSmoothFonts(true); g.setFillColor(gray: CGFloat(level) * 0.25, alpha: 1) }
    else { g.setShouldSmoothFonts(false); g.setFillColor(gray: 0, alpha: 1) }
    g.translateBy(x: CGFloat(penX), y: CGFloat(cell - penY))
    g.scaleBy(x: dpr, y: dpr)
    g.textPosition = CGPoint(x: CGFloat(variant) / 4 / dpr, y: 0)
    CTLineDraw(line, g)
  }
}

func zeronAtlas(_ device: MTLDevice, state: UnsafeMutableRawPointer, dpr: CGFloat, api: ZeronAtlasAPI) -> ZeronAtlas {
  let glyphs = Int(api.glyph_count()), fonts = Int(api.font_count()), icons = Int(api.icon_count())
  let width = 8192
  var y0 = 0
  struct Slot { var font: CTFont; var cell: Int; var penX: Int; var penY: Int; var y0: Int; var cols: Int; var size: CGFloat; var mono: Bool }
  var slots: [Slot] = []
  for f in 0..<fonts {
    let size = CGFloat(api.font_size(Int32(f)))
    let weight = api.font_weight(Int32(f))
    let cg = geist(weight)!
    let font = CTFontCreateWithGraphicsFont(cg, size, nil, nil)
    let cell = Int(ceil(size * dpr * 2.4))
    let cols = width / cell
    let cells = glyphs * 4 * 5
    let slot = Slot(font: font, cell: cell, penX: Int(round(Double(cell) * 0.2)), penY: Int(round(Double(cell) * 0.7)), y0: y0, cols: cols, size: size, mono: weight == 3)
    slots.append(slot)
    y0 += ((cells + cols - 1) / cols) * cell
  }
  var iconY = y0, iconX = 0, rowH = 0
  var tiles: [(Int, Int, Int)] = []
  for i in 0..<icons {
    let px = Int(ceil(CGFloat(api.icon_size(Int32(i))) * dpr * 2))
    if iconX + px > width { iconX = 0; iconY += rowH; rowH = 0 }
    tiles.append((iconX, iconY, px)); iconX += px + 2; rowH = max(rowH, px + 2)
  }
  // the glyph cache's mono cells, sized for the largest font
  let dynCell = slots.map { $0.cell }.max() ?? 64
  let dynY0 = iconY + rowH + 1, dynCols = width / dynCell
  let height = dynY0 + ((2048 + dynCols - 1) / dynCols) * dynCell
  let g = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width,
                    space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue)!
  for (f, s) in slots.enumerated() {
    var header: [Float] = [Float(CTFontGetAscent(s.font)), Float(CTFontGetDescent(s.font)), Float(s.cell), Float(s.penX), Float(s.penY), Float(s.y0), Float(s.cols)]
    var codes = (0..<glyphs).map { UniChar(truncatingIfNeeded: api.glyph_code(Int32($0))) }
    var ids = [CGGlyph](repeating: 0, count: glyphs)
    CTFontGetGlyphsForCharacters(s.font, &codes, &ids, glyphs)
    var adv = [CGSize](repeating: .zero, count: glyphs)
    CTFontGetAdvancesForGlyphs(s.font, .horizontal, ids, &adv, glyphs)
    var advance = adv.map { Float($0.width) }
    // Kerning as CoreText shapes it (what GPUI's line layout gets from CTLine): a pair's width less its advances.
    var kern = [Float](repeating: 0, count: glyphs * glyphs)
    let attrs: [NSAttributedString.Key: Any] = [.font: s.font]
    // (a monospaced font has none)
    for a in 1..<glyphs where a < 95 && !s.mono {
      for b in 1..<glyphs where b < 95 {
        let pair = String(UnicodeScalar(UInt8(32 + a))) + String(UnicodeScalar(UInt8(32 + b)))
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: pair, attributes: attrs))
        let w = Float(CTLineGetTypographicBounds(line, nil, nil, nil))
        let k = w - advance[a] - advance[b]
        if abs(k) > 0.001 { kern[a * glyphs + b] = k }
      }
    }
    header.withUnsafeBufferPointer { h in advance.withUnsafeMutableBufferPointer { ad in kern.withUnsafeBufferPointer { k in
      api.set_font(state, Int32(f), h.baseAddress, ad.baseAddress, k.baseAddress) } } }
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
    let n = Int(api.icon_name(Int32(i), &name, 64))
    let file = String(decoding: name[0..<n], as: UTF8.self)
    guard let url = Bundle.main.url(forResource: file, withExtension: "svg", subdirectory: "icons"),
          var svg = try? String(contentsOf: url, encoding: .utf8) else { continue }
    // Sized to the tile (a viewBox that isn't square fills it, which lands closest to usvg's pixmap).
    if let open = svg.range(of: "<svg"), let close = svg.range(of: ">", range: open.upperBound..<svg.endIndex) {
      let head = String(svg[open.lowerBound..<close.lowerBound])
        .replacingOccurrences(of: #"\s(width|height)="[^"]*""#, with: "", options: .regularExpression)
      svg.replaceSubrange(open.lowerBound..<close.lowerBound, with: head)
    }
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
    api.set_icon(state, Int32(i), Float(t.0), Float(t.1), Float(t.2), Float(t.2))
  }
  api.set_atlas(state, Float(width), Float(height))
  let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: width, height: height, mipmapped: false)
  let texture = device.makeTexture(descriptor: d)!
  texture.replace(region: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0, withBytes: g.data!, bytesPerRow: width)
  let colorCols = width / dynCell, colorHeight = ((256 + colorCols - 1) / colorCols) * dynCell
  let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: width, height: colorHeight, mipmapped: false)
  let color = device.makeTexture(descriptor: cd)!
  let penX = Int(round(Double(dynCell) * 0.2)), penY = Int(round(Double(dynCell) * 0.7))
  api.set_dynamic(state, Float(dynCell), Float(penX), Float(penY), Float(dynY0), Int32(dynCols), Int32(colorCols), Float(width), Float(colorHeight))
  return ZeronAtlas(api: api, mono: texture, color: color, fonts: slots.map { $0.font }, dpr: dpr, cell: dynCell, penX: penX, penY: penY)
}

// Shared GPUI GPU effects. Apps supply only Coax's vertices and the font atlas.
final class ZeronMetalRenderer {
  let queue: MTLCommandQueue
  let pipeline: MTLRenderPipelineState
  init(device: MTLDevice, library: MTLLibrary) throws {
    queue = device.makeCommandQueue()!
    let p = MTLRenderPipelineDescriptor()
    p.vertexFunction = library.makeFunction(name: "zr_vs"); p.fragmentFunction = library.makeFunction(name: "zr_fs")
    let v = MTLVertexDescriptor()
    for i in 0..<7 { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = 28 * 4; p.vertexDescriptor = v
    let a = p.colorAttachments[0]!
    a.pixelFormat = .bgra8Unorm; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .add
    a.sourceRGBBlendFactor = .sourceAlpha; a.sourceAlphaBlendFactor = .one
    a.destinationRGBBlendFactor = .oneMinusSourceAlpha; a.destinationAlphaBlendFactor = .oneMinusSourceAlpha
    pipeline = try device.makeRenderPipelineState(descriptor: p)
  }
  func encode(_ encoder: MTLRenderCommandEncoder, vertices: MTLBuffer, count: Int, size: CGSize, atlas: ZeronAtlas) {
    guard count > 0 else { return }
    var viewport = SIMD2<Float>(Float(size.width), Float(size.height))
    encoder.setRenderPipelineState(pipeline)
    encoder.setVertexBuffer(vertices, offset: 0, index: 0)
    encoder.setVertexBytes(&viewport, length: 8, index: 1)
    encoder.setFragmentTexture(atlas.mono, index: 0); encoder.setFragmentTexture(atlas.color, index: 1)
    encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: count)
  }
}
