// Firm reason: 6 (CoreText has no C API; this holds no product logic).
// The faces' atlas (text.coax), drawn: ASCII 32..126 of Spectral Medium in the first byte and of
// Cinzel at weight 600 in the second, 16 columns of 96-texel cells, the pen at x 16 and the
// baseline at y 72 at 64 px, as the sans atlas (metaverse-engine/render/sans-profile.coax), each glyph cut at
// its cell. Each mip is drawn at its own size. The Mac and iOS hosts upload these (Engine.swift); the Android build
// writes them to a file (apps/everglade-mobile/android/bake-faces.swift), since CoreText is
// not on a phone that isn't Apple's.
import Foundation
import CoreGraphics
import CoreText

struct FaceLevel { let width: Int, height: Int, bytes: [UInt8] }

func faceLevels(fonts: URL) -> [FaceLevel]? {
  func face(_ file: String, weight: CGFloat?) -> CTFontDescriptor? {
    let url = fonts.appendingPathComponent(file + ".ttf")
    guard let all = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor], let d = all.first else { return nil }
    guard let weight else { return d }
    // Cinzel is variable; 'wght' is its weight axis.
    return CTFontDescriptorCreateCopyWithAttributes(d, [kCTFontVariationAttribute: [NSNumber(value: 0x77676874): NSNumber(value: Double(weight))]] as CFDictionary)
  }
  guard let body = face("Spectral-Medium", weight: nil), let title = face("Cinzel", weight: 600) else { return nil }
  let cols = 16, rows = 6, cell = 96, levels = 5
  var out: [FaceLevel] = []
  for level in 0..<levels {
    let c = cell >> level, w = cols * c, h = rows * c, scale = CGFloat(1 << level)
    var both = [UInt8](repeating: 0, count: w * h * 2)
    for (channel, descriptor) in [body, title].enumerated() {
      guard let g = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue) else { return nil }
      g.setFillColor(gray: 0, alpha: 1); g.fill(CGRect(x: 0, y: 0, width: w, height: h))
      g.setShouldSmoothFonts(false); g.setAllowsAntialiasing(true); g.textMatrix = .identity
      let font = CTFontCreateWithFontDescriptor(descriptor, 64 / scale, nil)
      let attrs: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): font, NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 1, alpha: 1)]
      for i in 0..<95 {
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: String(Character(Unicode.Scalar(UInt8(32 + i)))), attributes: attrs))
        // Cut at its own cell: Cinzel's Q ends 1.33 em after its pen, past the cell's 1.25, and would
        // leave a mark in R's cell (#13116).
        g.saveGState(); g.clip(to: CGRect(x: (i % cols) * c, y: h - (i / cols + 1) * c, width: c, height: c))
        g.textPosition = CGPoint(x: CGFloat((i % cols) * c) + 16 / scale, y: CGFloat(h) - (CGFloat((i / cols) * c) + 72 / scale))
        CTLineDraw(line, g)
        g.restoreGState()
      }
      let gray = g.data!.assumingMemoryBound(to: UInt8.self)
      for p in 0..<w * h { both[p * 2 + channel] = gray[p] }
    }
    out.append(FaceLevel(width: w, height: h, bytes: both))
  }
  return out
}
