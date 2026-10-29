// The Zeron-fork chat screen (docs/desktop/coax-desktop.md): zeron/screen.coax
// compiled to C writes GPUI-style primitives (zeron/gpui.coax), drawn here by
// Zeron.metal. This file is GPUI's platform layer for it: the window's Metal
// view, input, and the atlas of Zeron's font (Geist, rasterized as
// gpui_macos/text_system.rs does) and Zeron's SVG icons. Based on Zeron by Wing, MIT.
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
final class ZeronAtlas {
  let mono: MTLTexture, color: MTLTexture
  let fonts: [CTFont], dpr: CGFloat, cell: Int, penX: Int, penY: Int
  init(mono: MTLTexture, color: MTLTexture, fonts: [CTFont], dpr: CGFloat, cell: Int, penX: Int, penY: Int) {
    self.mono = mono; self.color = color; self.fonts = fonts; self.dpr = dpr; self.cell = cell; self.penX = penX; self.penY = penY
  }

  // What the last frame asked for: each new grapheme's advance and whether
  // CoreText draws it in color (a color-glyph font, Apple Color Emoji), then
  // each wanted cell drawn as rasterize_glyph does.
  func serve(_ state: UnsafeMutableRawPointer) {
    var bytes = [UInt8](repeating: 0, count: 64)
    var guardCount = 0
    while case let slot = zr_next_cluster(state), slot >= 0, guardCount < 600 {
      guardCount += 1
      let (line, _) = shape(state, slot, &bytes)
      let width = Float(CTLineGetTypographicBounds(line, nil, nil, nil))
      let runs = CTLineGetGlyphRuns(line) as! [CTRun]
      let isColor = runs.contains { run in
        let attrs = CTRunGetAttributes(run) as NSDictionary
        guard let f = attrs[kCTFontAttributeName] else { return false }
        return CTFontGetSymbolicTraits(f as! CTFont).contains(.traitColorGlyphs)
      }
      zr_measured(state, slot, width, isColor ? 1 : 0)
    }
    var info = [Float](repeating: 0, count: 6)
    guardCount = 0
    while case let index = zr_next_cell(state), index >= 0, guardCount < 3000 {
      guardCount += 1
      _ = zr_cell(state, index, &info)
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
      zr_cell_ready(state, index)
    }
  }
  var side: Int { cell }
  func shape(_ state: UnsafeMutableRawPointer, _ slot: Int32, _ bytes: inout [UInt8]) -> (CTLine, Int) {
    let code = Int(zr_cluster(state, slot, &bytes, Int32(bytes.count)))
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

func zeronAtlas(_ device: MTLDevice, state: UnsafeMutableRawPointer, dpr: CGFloat) -> ZeronAtlas {
  let glyphs = Int(zr_glyph_count()), fonts = Int(zr_font_count()), icons = Int(zr_icon_count())
  let width = 8192
  var y0 = 0
  struct Slot { var font: CTFont; var cell: Int; var penX: Int; var penY: Int; var y0: Int; var cols: Int; var size: CGFloat; var mono: Bool }
  var slots: [Slot] = []
  for f in 0..<fonts {
    let size = CGFloat(zr_font_size(Int32(f)))
    let weight = zr_font_weight(Int32(f))
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
    let px = Int(ceil(CGFloat(zr_icon_size(Int32(i))) * dpr * 2))
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
    var codes = (0..<glyphs).map { UniChar(truncatingIfNeeded: zr_glyph_code(Int32($0))) }
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
    zr_set_icon(state, Int32(i), Float(t.0), Float(t.1), Float(t.2), Float(t.2))
  }
  zr_set_atlas(state, Float(width), Float(height))
  let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: width, height: height, mipmapped: false)
  let texture = device.makeTexture(descriptor: d)!
  texture.replace(region: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0, withBytes: g.data!, bytesPerRow: width)
  let colorCols = width / dynCell, colorHeight = ((256 + colorCols - 1) / colorCols) * dynCell
  let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: width, height: colorHeight, mipmapped: false)
  let color = device.makeTexture(descriptor: cd)!
  let penX = Int(round(Double(dynCell) * 0.2)), penY = Int(round(Double(dynCell) * 0.7))
  zr_set_dynamic(state, Float(dynCell), Float(penX), Float(penY), Float(dynY0), Int32(dynCols), Int32(colorCols), Float(width), Float(colorHeight))
  return ZeronAtlas(mono: texture, color: color, fonts: slots.map { $0.font }, dpr: dpr, cell: dynCell, penX: penX, penY: penY)
}

final class ZeronView: MTKView, MTKViewDelegate, NSTextInputClient, NSMenuItemValidation {
  let state = zr_new()!
  var queue: MTLCommandQueue!
  var pipeline: MTLRenderPipelineState!
  var atlas: ZeronAtlas?, atlasDpr: CGFloat = 0
  var vertices: [MTLBuffer] = [], slot = 0
  let inflight = DispatchSemaphore(value: 2)
  var last = CACurrentMediaTime()
  var cursorKind: Int32 = 0
  var copyBuffer = [UInt8](repeating: 0, count: 65536)

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
    let floats = Int(zr_vertex_floats())
    for i in 0..<(floats / 4) { v.attributes[i].format = .float4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 0 }
    v.layouts[0].stride = floats * 4
    p.vertexDescriptor = v
    // GPUI's blending (metal_renderer.rs): straight alpha, source over.
    let a = p.colorAttachments[0]!
    a.pixelFormat = colorPixelFormat; a.isBlendingEnabled = true
    a.rgbBlendOperation = .add; a.alphaBlendOperation = .add
    a.sourceRGBBlendFactor = .sourceAlpha; a.sourceAlphaBlendFactor = .one
    a.destinationRGBBlendFactor = .oneMinusSourceAlpha; a.destinationAlphaBlendFactor = .oneMinusSourceAlpha
    pipeline = try! device.makeRenderPipelineState(descriptor: p)
    let bytes = Int(zr_max_vertices()) * floats * 4
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
    // the graphemes and cells this frame wanted, for the next
    atlas!.serve(state)
    let want = zr_cursor(state)
    if want != cursorKind { cursorKind = want; cursorFor(want).set() }
    // a code block's Copy, pressed
    // a notice's Log in or Install: Terminal runs it (the CLI's own sign-in, the install line)
    switch zr_take_action(state) {
    case 1: terminal("claude /login")
    case 2: terminal("curl -fsSL https://claude.ai/install.sh | bash")
    default: break
    }
    let copied = Int(zr_take_copy(state, &copyBuffer, Int32(copyBuffer.count)))
    if copied > 0 {
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(String(decoding: copyBuffer[0..<copied], as: UTF8.self), forType: .string)
    }
    pass.colorAttachments[0].loadAction = .clear
    pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
    guard let cmd = queue.makeCommandBuffer(), let enc = cmd.makeRenderCommandEncoder(descriptor: pass) else { inflight.signal(); return }
    if n > 0 {
      var viewport = SIMD2<Float>(Float(size.width), Float(size.height))
      enc.setRenderPipelineState(pipeline)
      enc.setVertexBuffer(buf, offset: 0, index: 0)
      enc.setVertexBytes(&viewport, length: 8, index: 1)
      enc.setFragmentTexture(atlas!.mono, index: 0)
      enc.setFragmentTexture(atlas!.color, index: 1)
      enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: Int(n))
    }
    enc.endEncoding()
    cmd.addCompletedHandler { [inflight] _ in inflight.signal() }
    done?(cmd)
    cmd.commit()
  }

  // The chat's engine: the user's own Claude Code in --cwd PATH (default the
  // home folder), or Zeron's demo conversation with --demo; --model
  // opus|sonnet|haiku, --effort low|medium|high|xhigh|max and --access
  // full|edits|supervised preset the composer's chips; --mode claude|psionic
  // the chat's mode (zeron/modes.coax; the effort then counts in its list).
  func configure(_ args: [String]) {
    func value(_ flag: String) -> String? { args.firstIndex(of: flag).flatMap { $0 + 1 < args.count ? args[$0 + 1] : nil } }
    if args.contains("--demo") { zr_demo(state); return }
    let cwd = value("--cwd").map { ($0 as NSString).expandingTildeInPath } ?? NSHomeDirectory()
    Array(cwd.utf8).withUnsafeBufferPointer { zr_attach(state, $0.baseAddress, Int32($0.count)) }
    if let m = value("--mode")?.lowercased(), let i = ["claude", "psionic"].firstIndex(of: m) { zr_choose(state, 4, Int32(i)) }
    for (flag, which, names) in [("--model", 1, ["opus", "sonnet", "haiku"]), ("--effort", 2, ["low", "medium", "high", "xhigh", "max"]),
                                 ("--access", 3, ["full", "edits", "supervised"])] {
      if let v = value(flag)?.lowercased(), let i = names.firstIndex(of: v) { zr_choose(state, Int32(which), Int32(i)) }
    }
  }
  func terminal(_ command: String) {
    let script = "tell application \"Terminal\"\nactivate\ndo script \"\(command.replacingOccurrences(of: "\"", with: "\\\""))\"\nend tell"
    NSAppleScript(source: script)?.executeAndReturnError(nil)
  }

  // --shot PATH [--size W,H] [--click X,Y] [--type TEXT] [--do STEPS] [--frames N]: draw offscreen at scale 2
  // (clicking at device pixel X,Y at frame 30, typing TEXT at frame 45), write the last frame as a PNG, quit.
  // --do runs STEPS, separated by " ; ", one every 4 frames from frame 50, through the same paths
  // as the keyboard, mouse, menus and IME:
  //   click X,Y[,N]   press and release at device pixel X,Y (N clicks: 2 a word, 3 the line)
  //   shiftclick X,Y  drag X,Y,X2,Y2   wheel DY
  //   key CHORD       a key as Zeron's keymap names it: cmd+a, shift+alt+left, backspace, shift+enter
  //   type TEXT       mark TEXT  (IME marked text)   commit TEXT (IME insertText)
  //   menu TITLE      an Edit menu item through the menu bar (Undo, Cut, Copy, Paste, Select All...)
  //   keyevent CHORD  a real NSEvent through keyDown and the input context
  //   menus           print the menu bar
  //   move X,Y        the pointer moving to device pixel X,Y (hover)
  //   fixture open|closed   the transcript as the Zeron capture opens it (transcript-fixture.rs)
  //   scroll PX       the transcript scrolled PX from its top    pasteboard  print the pasteboard
  //   send TEXT       type TEXT into the composer and press Enter    stop  press Escape (stops a running turn)
  //   model NAME      a chip's choice (also effort NAME, access NAME, as the flags take them)
  //   wait SECS       let the frames run    waitidle [SECS]  until the running turn ends (at most SECS, 300)
  // Without --frames the shot is taken 12 frames after the last step.
  func shot(_ path: String, width: Int, height: Int) {
    let args = CommandLine.arguments
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: colorPixelFormat, width: width, height: height, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    let target = device!.makeTexture(descriptor: d)!
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = target; pass.colorAttachments[0].storeAction = .store
    var frames = 0, next = 50, index = 0
    var hold: (until: Double, idle: Bool)? = nil
    var last = args.firstIndex(of: "--frames").flatMap { Int(args[$0 + 1]) } ?? Int.max
    let fixed = last != Int.max
    let steps = args.firstIndex(of: "--do").map { i in i + 1 < args.count ? args[i + 1].components(separatedBy: " ; ") : [] } ?? []
    Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { timer in
      frames += 1
      if frames == 30, let i = args.firstIndex(of: "--click"), i + 1 < args.count {
        let xy = args[i + 1].split(separator: ",").compactMap { Float($0) }
        if xy.count == 2 { _ = zr_pointer(self.state, xy[0], xy[1], 0, 1, 0); _ = zr_pointer(self.state, xy[0], xy[1], 2, 1, 0) }
      }
      if frames == 45, let i = args.firstIndex(of: "--type"), i + 1 < args.count { self.typed(args[i + 1]) }
      if let h = hold {
        if CACurrentMediaTime() >= h.until || (h.idle && zr_live(self.state) == 0) { hold = nil; next = frames + 4 }
      } else if frames >= next && index < steps.count {
        hold = self.step(steps[index]); index += 1; next = frames + 4
      }
      if !fixed && last == Int.max && index >= steps.count && hold == nil { last = max(60, frames + 12) }
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
            zr_quit(self.state); exit(0)
          }
        }
      }
    }
  }
  @discardableResult func step(_ line: String) -> (until: Double, idle: Bool)? {
    let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
    guard let verb = parts.first else { return nil }
    let arg = parts.count > 1 ? parts[1] : ""
    let nums = arg.split(separator: ",").compactMap { Float($0) }
    switch verb {
    case "click" where nums.count >= 2:
      let n = nums.count > 2 ? Int32(nums[2]) : 1
      _ = zr_pointer(state, nums[0], nums[1], 0, n, 0); _ = zr_pointer(state, nums[0], nums[1], 2, n, 0)
    case "shiftclick" where nums.count >= 2:
      _ = zr_pointer(state, nums[0], nums[1], 0, 1, 1); _ = zr_pointer(state, nums[0], nums[1], 2, 1, 1)
    case "drag" where nums.count == 4:
      _ = zr_pointer(state, nums[0], nums[1], 0, 1, 0)
      for k in 1...8 { let t = Float(k) / 8; _ = zr_pointer(state, nums[0] + (nums[2] - nums[0]) * t, nums[1] + (nums[3] - nums[1]) * t, 1, 1, 0) }
      _ = zr_pointer(state, nums[2], nums[3], 2, 1, 0)
    case "wheel" where nums.count == 1: _ = zr_wheel(state, nums[0])
    case "move" where nums.count == 2: _ = zr_pointer(state, nums[0], nums[1], 1, 1, 0)
    case "fixture": zr_fixture(state, arg == "closed" ? 0 : 1)
    case "send": typed(arg); _ = zr_key(state, -2, 0)
    case "stop": _ = zr_key(state, -1, 0)
    case "model", "effort", "access":
      let names = ["model": ["opus", "sonnet", "haiku"], "effort": ["low", "medium", "high", "xhigh", "max"], "access": ["full", "edits", "supervised"]][verb]!
      if let i = names.firstIndex(of: arg.lowercased()) { zr_choose(state, verb == "model" ? 1 : verb == "effort" ? 2 : 3, Int32(i)) }
    case "mode":
      if let i = ["claude", "psionic"].firstIndex(of: arg.lowercased()) { zr_choose(state, 4, Int32(i)) }
    case "wait" where nums.count == 1: return (CACurrentMediaTime() + Double(nums[0]), false)
    case "waitidle": return (CACurrentMediaTime() + Double(nums.first ?? 300), true)
    case "scroll" where nums.count == 1: zr_scroll_to(state, nums[0])
    case "pasteboard": print("pasteboard: \(NSPasteboard.general.string(forType: .string) ?? "")")
    case "key":
      let (code, mods, char) = chord(arg)
      _ = binding(code: code, char: char, mods: mods)
    case "type": typed(arg)
    case "mark": setMarkedText(arg, selectedRange: NSRange(location: (arg as NSString).length, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
    case "commit": insertText(arg, replacementRange: NSRange(location: NSNotFound, length: 0))
    case "menu":
      for top in NSApp.mainMenu?.items ?? [] {
        guard let sub = top.submenu, let i = sub.items.firstIndex(where: { $0.title == arg }) else { continue }
        // offscreen there is no key window to route a nil-targeted item, so it goes to this view, as it would in the window
        let item = sub.items[i]
        if let action = item.action, validateMenuItem(item) { NSApp.sendAction(action, to: self, from: item) }
        else { print("menu \(arg) is disabled") }
      }
    case "menus":   // the menu bar as AppKit has it, printed
      for top in NSApp.mainMenu?.items ?? [] {
        let sub = top.submenu
        print("\(sub?.title ?? "?"): " + (sub?.items ?? []).map { $0.isSeparatorItem ? "|" : $0.title + ($0.keyEquivalent.isEmpty ? "" : " [\($0.keyEquivalentModifierMask.contains(.shift) ? "shift-" : "")\($0.keyEquivalentModifierMask.contains(.option) ? "alt-" : "")\($0.keyEquivalentModifierMask.contains(.control) ? "ctrl-" : "")cmd-\($0.keyEquivalent)]") }.joined(separator: ", "))
      }
    case "keyevent":
      let (code, mods, char) = chord(arg)
      var flags: NSEvent.ModifierFlags = []
      if mods & 1 != 0 { flags.insert(.shift) }; if mods & 2 != 0 { flags.insert(.control) }
      if mods & 4 != 0 { flags.insert(.option) }; if mods & 8 != 0 { flags.insert(.command) }
      let keyCode = ZeronView.keyCodes[char ?? ""].map { UInt16($0) } ?? ZeronView.named.first(where: { $0.value == code })?.key ?? 0
      inputContext?.activate()   // offscreen the window never becomes key, which is what activates it
      for type in [NSEvent.EventType.keyDown, .keyUp] {
        guard let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: window?.windowNumber ?? 0, context: nil, characters: mods & 4 != 0 ? "" : (char ?? ""), charactersIgnoringModifiers: char ?? "",
                                       isARepeat: false, keyCode: keyCode) else { continue }
        if type == .keyDown { keyDown(with: e) } else { keyUp(with: e) }
      }
    default: print("unknown step: \(line)")
    }
    return nil
  }
  static let named: [UInt16: Int32] = [53: -1, 36: -2, 76: -2, 51: -3, 123: -4, 124: -5, 126: -6, 125: -7, 115: -8, 119: -9, 116: -10, 121: -11, 117: -12]
  static let keyCodes: [String: Int] = ["a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14,
                                        "r": 15, "y": 16, "t": 17, "o": 31, "u": 32, "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46]
  // "shift+alt+left" -> (named key or 0, mods, a letter)
  func chord(_ text: String) -> (Int32, Int32, String?) {
    var mods: Int32 = 0, code: Int32 = 0, char: String? = nil
    let names: [String: Int32] = ["escape": -1, "enter": -2, "backspace": -3, "left": -4, "right": -5, "up": -6, "down": -7, "home": -8, "end": -9, "delete": -12]
    for part in text.lowercased().split(separator: "+").map(String.init) {
      switch part {
      case "shift": mods |= 1
      case "ctrl": mods |= 2
      case "alt": mods |= 4
      case "cmd": mods |= 8
      default: if let n = names[part] { code = n } else { char = part }
      }
    }
    return (code, mods, char)
  }

  // ---- the pointer

  func place(_ e: NSEvent) -> (Float, Float) {
    let p = convert(e.locationInWindow, from: nil), d = window?.backingScaleFactor ?? 2
    return (Float(p.x * d), Float((bounds.height - p.y) * d))
  }
  func pointer(_ e: NSEvent, _ phase: Int32) -> Bool {
    let (x, y) = place(e)
    return zr_pointer(state, x, y, phase, Int32(max(e.clickCount, 1)), mods(e)) != 0
  }
  override func mouseDown(with e: NSEvent) {
    window?.makeFirstResponder(self)
    if hasMarkedText() { inputContext?.discardMarkedText() }
    _ = pointer(e, 0)
  }
  override func mouseDragged(with e: NSEvent) { _ = pointer(e, 1) }
  override func mouseMoved(with e: NSEvent) { _ = pointer(e, 1) }
  override func mouseUp(with e: NSEvent) { _ = pointer(e, 2) }
  override func scrollWheel(with e: NSEvent) {
    // GPUI's pixel delta: precise deltas as they come, lines times the line height
    let dy = e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 22.75
    if zr_wheel(state, Float(dy)) == 0 { super.scrollWheel(with: e) }
  }
  func cursorFor(_ kind: Int32) -> NSCursor { kind == 1 ? .iBeam : kind == 2 ? .pointingHand : .arrow }
  override func cursorUpdate(with event: NSEvent) { cursorFor(cursorKind).set() }

  // ---- the keyboard: Zeron's keymap first, then the input context (IME, dead keys, plain typing)

  func typed(_ s: String) { Array(s.utf8).withUnsafeBufferPointer { zr_text(state, $0.baseAddress, Int32($0.count)) } }
  func mods(_ e: NSEvent) -> Int32 {
    var m: Int32 = 0
    if e.modifierFlags.contains(.shift) { m |= 1 }
    if e.modifierFlags.contains(.control) { m |= 2 }
    if e.modifierFlags.contains(.option) { m |= 4 }
    if e.modifierFlags.contains(.command) { m |= 8 }
    return m
  }
  // A keystroke Zeron's composer binds (input_key_bindings): named keys and
  // cmd-/ctrl- chords. Ctrl+C, X and V go to the pasteboard as Cmd's do
  // (through the menu). Whether it was bound.
  func binding(code: Int32, char: String?, mods: Int32) -> Bool {
    if code != 0 { return zr_key(state, code, mods) != 0 }
    guard let c = char?.unicodeScalars.first, mods & (2 | 8) != 0 else { return false }
    let letter = String(c).lowercased()
    if mods & 2 != 0 && mods & (4 | 8) == 0 && mods & 1 == 0 {
      if letter == "c" { copy(nil); return true }
      if letter == "x" { cut(nil); return true }
      if letter == "v" { paste(nil); return true }
    }
    if mods & 8 != 0 && mods & (2 | 4) == 0 {
      if letter == "c" { copy(nil); return true }
      if letter == "x" { cut(nil); return true }
      if letter == "v" { paste(nil); return true }
    }
    return zr_key(state, Int32(c.value), mods) != 0
  }
  override func keyDown(with e: NSEvent) {
    guard zr_wants_text(state) != 0 else {
      // unfocused: Zeron's composer only takes keys when focused; Copy and Select All reach the transcript
      if e.modifierFlags.contains(.command), let c = e.charactersIgnoringModifiers?.lowercased() {
        if c == "c" { copy(nil) } else if c == "a" { selectAll(nil) }
      }
      return
    }
    // while composing, the IME owns every key (Enter commits, arrows pick candidates)
    if hasMarkedText() { inputContext?.handleEvent(e); return }
    if e.keyCode == 48 { return }   // Tab: Zeron's MentionTab, nothing without a mention or a list
    let code = ZeronView.named[e.keyCode] ?? 0
    if binding(code: code, char: e.charactersIgnoringModifiers, mods: mods(e)) { return }
    if code != 0 || e.modifierFlags.contains(.command) || e.modifierFlags.contains(.control) { return }
    inputContext?.handleEvent(e)
  }

  // ---- the Edit menu (and Cmd+C/X/V/A/Z, its key equivalents)

  @objc func undo(_ sender: Any?) { zr_undo(state) }
  @objc func redo(_ sender: Any?) { zr_redo(state) }
  @objc override func selectAll(_ sender: Any?) { zr_select_all(state) }
  @objc func copy(_ sender: Any?) {
    let n = Int(zr_selected_count(state))
    guard n > 0 else { return }
    var bytes = [UInt8](repeating: 0, count: n)
    _ = zr_selected(state, &bytes, Int32(n))
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(String(decoding: bytes, as: UTF8.self), forType: .string)
  }
  @objc func cut(_ sender: Any?) {
    guard zr_can_cut(state) != 0 else { return }
    copy(sender); zr_cut(state)
  }
  @objc func paste(_ sender: Any?) {
    let pb = NSPasteboard.general
    // files and images are attachments in Zeron (staged, not typed); this screen has none
    if pb.canReadObject(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) { return }
    guard let s = pb.string(forType: .string), !s.isEmpty else { return }
    let bytes = Array(s.utf8)
    if hasMarkedText() { inputContext?.discardMarkedText(); zr_unmark(state) }
    bytes.withUnsafeBufferPointer { zr_paste(state, $0.baseAddress, Int32($0.count)) }
  }
  func validateMenuItem(_ item: NSMenuItem) -> Bool {
    let focused = zr_wants_text(state) != 0
    switch item.action {
    case #selector(undo(_:)), #selector(redo(_:)), #selector(paste(_:)): return focused
    case #selector(selectAll(_:)): return true
    case #selector(copy(_:)): return zr_selected_count(state) > 0
    case #selector(cut(_:)): return zr_can_cut(state) != 0
    default: return true
    }
  }

  // ---- NSTextInputClient (GPUI's ElementInputHandler over the composer)

  func string(_ value: Any) -> String { (value as? NSAttributedString)?.string ?? (value as? String) ?? "" }
  func insertText(_ value: Any, replacementRange: NSRange) {
    let bytes = Array(string(value).utf8)
    let from = replacementRange.location == NSNotFound ? -1 : Int32(replacementRange.location)
    let to = replacementRange.location == NSNotFound ? -1 : Int32(replacementRange.location + replacementRange.length)
    bytes.withUnsafeBufferPointer { zr_replace(state, from, to, $0.baseAddress, Int32($0.count)) }
  }
  func setMarkedText(_ value: Any, selectedRange: NSRange, replacementRange: NSRange) {
    let bytes = Array(string(value).utf8)
    let from = replacementRange.location == NSNotFound ? -1 : Int32(replacementRange.location)
    let to = replacementRange.location == NSNotFound ? -1 : Int32(replacementRange.location + replacementRange.length)
    let sf = selectedRange.location == NSNotFound ? -1 : Int32(selectedRange.location)
    let st = selectedRange.location == NSNotFound ? -1 : Int32(selectedRange.location + selectedRange.length)
    bytes.withUnsafeBufferPointer { zr_mark(state, from, to, $0.baseAddress, Int32($0.count), sf, st) }
  }
  func unmarkText() { zr_unmark(state) }
  func selectedRange() -> NSRange {
    let a = Int(zr_range16(state, 0)), b = Int(zr_range16(state, 1))
    return NSRange(location: a, length: b - a)
  }
  func markedRange() -> NSRange {
    let a = Int(zr_range16(state, 2)), b = Int(zr_range16(state, 3))
    return a < 0 ? NSRange(location: NSNotFound, length: 0) : NSRange(location: a, length: b - a)
  }
  func hasMarkedText() -> Bool { zr_range16(state, 2) >= 0 }
  func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
    let total = Int(zr_range16(state, 4))
    let a = min(range.location, total), b = min(range.location + range.length, total)
    guard a <= b else { return nil }
    var bytes = [UInt8](repeating: 0, count: 8192)
    let n = Int(zr_text16(state, Int32(a), Int32(b), &bytes, Int32(bytes.count)))
    actualRange?.pointee = NSRange(location: a, length: b - a)
    return NSAttributedString(string: String(decoding: bytes[0..<n], as: UTF8.self))
  }
  func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
  func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
    var r = [Float](repeating: 0, count: 4)
    guard zr_rect16(state, Int32(range.location == NSNotFound ? 0 : range.location), &r) != 0, let window else { return .zero }
    actualRange?.pointee = range
    let local = NSRect(x: CGFloat(r[0]), y: bounds.height - CGFloat(r[1]) - CGFloat(r[3]), width: CGFloat(r[2]), height: CGFloat(r[3]))
    return window.convertToScreen(convert(local, to: nil))
  }
  func characterIndex(for point: NSPoint) -> Int {
    guard let window else { return NSNotFound }
    let p = convert(window.convertPoint(fromScreen: point), from: nil), d = window.backingScaleFactor
    return Int(zr_index16(state, Float(p.x * d), Float((bounds.height - p.y) * d)))
  }
  // Commands the IME sends for keys Zeron's keymap doesn't bind do nothing, as in GPUI.
  override func doCommand(by selector: Selector) {}
}
