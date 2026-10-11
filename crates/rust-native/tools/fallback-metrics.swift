// CoreText's widths for the characters a drawing face lacks, measured on
// this Mac and written as Rust tables (`src/layout/shape/fallback.rs`):
//
//   swift crates/rust-native/tools/fallback-metrics.swift \
//       crates/paper-mono/fonts crates/rust-native/src/layout/shape/fallback.rs
//
// CoreText draws a character the face lacks from a fallback face it picks
// by the face: Menlo under a monospace face (SF Mono, and Paper Mono at
// most weights), and the system's default list (Helvetica, Lucida Grande,
// Zapf Dingbats, Hiragino, ...) under a proportional one (SF Pro, and Paper
// Mono's Regular instance, wght 400, which CoreText does not treat as
// monospace). Paper Mono lacks nearly every character outside Latin, so it
// stands for both: at wght 500 for the monospace list and 400 for the
// default one. For every character in the blocks below that it lacks, the
// line CoreText lays out for that character alone is measured at 100 pt.
// Runs of characters with close widths share the run's first width, and
// unassigned characters take their neighbours'. A character CoreText draws
// from Apple Color Emoji is marked as an emoji; emoji have one width, whole
// points that depend on the size, recorded as the sizes where it steps up.
// Only these numbers are written, never the fonts.
import CoreText
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 3 else {
    FileHandle.standardError.write("usage: fallback-metrics.swift PAPER_MONO_FONTS_DIR OUT.rs\n".data(using: .utf8)!)
    exit(64)
}
let paperMono = URL(fileURLWithPath: arguments[1]).appendingPathComponent("PaperMono-Variable.ttf").path

func tag(_ name: String) -> Int {
    name.unicodeScalars.reduce(0) { $0 << 8 | Int($1.value) }
}

func font(_ path: String, size: Double, weight: Double = 400) -> CTFont {
    let data = try! Data(contentsOf: URL(fileURLWithPath: path))
    guard let descriptors = CTFontManagerCreateFontDescriptorsFromData(data as CFData) as? [CTFontDescriptor],
          let first = descriptors.first else { fatalError("unreadable face \(path)") }
    let attributes: [CFString: Any] = [kCTFontVariationAttribute: [tag("wght"): weight]]
    let descriptor = CTFontDescriptorCreateCopyWithAttributes(first, attributes as CFDictionary)
    return CTFontCreateWithFontDescriptor(descriptor, CGFloat(size), nil)
}

func has(_ font: CTFont, _ text: String) -> Bool {
    var units = Array(text.utf16)
    var glyphs = [CGGlyph](repeating: 0, count: units.count)
    return CTFontGetGlyphsForCharacters(font, &units, &glyphs, units.count)
}

/// The width of `text` laid out alone in `font`, in points, and the face
/// CoreText drew its first run with.
func measure(_ font: CTFont, _ text: String) -> (Double, String) {
    let attributed = NSAttributedString(string: text, attributes: [.init(kCTFontAttributeName as String): font])
    let line = CTLineCreateWithAttributedString(attributed)
    let runs = CTLineGetGlyphRuns(line) as! [CTRun]
    let name = runs.first.map {
        CTFontCopyPostScriptName((CTRunGetAttributes($0) as NSDictionary)[kCTFontAttributeName] as! CTFont) as String
    } ?? ""
    return (CTLineGetTypographicBounds(line, nil, nil, nil), name)
}

/// Latin, Greek and Cyrillic extensions, Hangul, punctuation and symbols,
/// CJK, compatibility and full-width forms, and the emoji planes. Other
/// scripts keep the shaper's default width.
/// Each block with how far, in thousandths of an em, a width may stray
/// from its run's: letters closely, symbols loosely.
let blocks: [(ClosedRange<UInt32>, Int)] = [
    (0x0080...0x02FF, 5), (0x0370...0x052F, 5), (0x1100...0x11FF, 25), (0x1E00...0x1FFF, 5),
    (0x2000...0x2BFF, 60), (0x2E80...0x9FFF, 25), (0xA960...0xA97F, 25), (0xAC00...0xD7FF, 25),
    (0xF900...0xFAFF, 25), (0xFE10...0xFE6F, 25), (0xFF00...0xFFEF, 25), (0x1F000...0x1FAFF, 60),
    (0x20000...0x3FFFF, 25),
]
let emoji = UInt32(UInt16.max)

/// Each run as its first character and width in thousandths of an em (0
/// for none, `emoji` for an emoji), covering the blocks.
func table(_ path: String, weight: Double) -> [(UInt32, UInt32)] {
    let base = font(path, size: 100, weight: weight)
    var runs: [(UInt32, UInt32)] = []
    for (block, tolerance) in blocks {
        // A block starts a run of no width until a character is measured.
        runs.append((block.lowerBound, 0))
        for value in block {
            guard let scalar = Unicode.Scalar(value) else { continue }
            switch scalar.properties.generalCategory {
            case .nonspacingMark, .enclosingMark, .unassigned, .control, .format, .surrogate, .privateUse:
                continue
            default: break
            }
            let text = String(scalar)
            if has(base, text) { continue }
            let (width, face) = measure(base, text)
            if face.isEmpty || face == "LastResort" { continue }
            let milli = face == "AppleColorEmoji" ? emoji : UInt32((width * 10).rounded())
            let last = runs[runs.count - 1].1
            let same = last == milli
                || (last != 0 && last != emoji && milli != emoji && abs(Int(last) - Int(milli)) <= tolerance)
            if same { continue }
            if last == 0, runs[runs.count - 1].0 == block.lowerBound {
                runs[runs.count - 1].1 = milli
            } else {
                runs.append((value, milli))
            }
        }
        runs.append((block.upperBound + 1, 0))
    }
    // Drop empty runs that start where another starts.
    var merged: [(UInt32, UInt32)] = []
    for run in runs {
        if let last = merged.last, last.0 == run.0 {
            merged[merged.count - 1] = run
        } else if let last = merged.last, last.1 == run.1 {
            continue
        } else {
            merged.append(run)
        }
    }
    return merged
}

/// Apple Color Emoji's width in whole points: the sizes from 4 pt to 64 pt
/// where it grows by a point, and the width below the first.
func emojiSteps(_ path: String) -> (Int, [Double]) {
    func advance(_ size: Double) -> Int {
        let (width, face) = measure(font(path, size: size), "\u{1F389}")
        precondition(face == "AppleColorEmoji", "emoji drew in \(face)")
        precondition(abs(width - width.rounded()) < 1e-6, "emoji widths are whole points")
        return Int(width.rounded())
    }
    let low = advance(4)
    let high = advance(64)
    var steps: [Double] = []
    for points in (low + 1)...high {
        var lower = 4.0
        var upper = 64.0
        while upper - lower > 1e-4 {
            let middle = (lower + upper) / 2
            if advance(middle) >= points { upper = middle } else { lower = middle }
        }
        steps.append((upper * 1e4).rounded(.up) / 1e4)
    }
    precondition(advance(64) == 64 && advance(100.4) == 100, "emoji are a whole em above 64 pt")
    // Where every later step is a half point below its width, the width is
    // the size rounded, so those steps are left out.
    while let last = steps.last, steps.count >= 2,
          last == Double(low + steps.count) - 0.5 {
        steps.removeLast()
    }
    return (low, steps)
}

func rust(_ name: String, _ doc: String, _ runs: [(UInt32, UInt32)]) -> String {
    var out = doc + "/// Each run's first character and its width in thousandths of an em,\n"
    out += "/// [`EMOJI`] for emoji, or 0 for the shaper's default.\n"
    out += "#[rustfmt::skip]\npub(super) const \(name): &[(u32, u16)] = &[\n"
    for chunk in stride(from: 0, to: runs.count, by: 6) {
        out += "    " + runs[chunk..<min(chunk + 6, runs.count)]
            .map { String(format: "(0x%05X, %u),", $0.0, $0.1) }
            .joined(separator: " ") + "\n"
    }
    return out + "];\n"
}

let monospace = table(paperMono, weight: 500)
let proportional = table(paperMono, weight: 400)
let (emojiLow, steps) = emojiSteps(paperMono)
var source = """
//! CoreText's widths for characters the drawing faces lack, measured by
//! `tools/fallback-metrics.swift` on macOS (Menlo, PingFang, Hiragino,
//! Apple SD Gothic Neo, Apple Color Emoji, ...). Generated; do not edit.

/// A run of emoji.
pub(super) const EMOJI: u16 = u16::MAX;

/// The smallest size measured, in points.
pub(super) const EMOJI_FROM: f32 = 4.0;

/// Apple Color Emoji's width, in points, from [`EMOJI_FROM`] up to the
/// first of [`EMOJI_STEPS`].
pub(super) const EMOJI_LOW: f32 = \(emojiLow).0;

/// The sizes, in points, where an emoji's width grows by a whole point.
/// Above the last, an emoji is as wide as the size, rounded to whole
/// points, and never narrower than at the last step.
#[rustfmt::skip]
pub(super) const EMOJI_STEPS: &[f32] = &[

"""
for chunk in stride(from: 0, to: steps.count, by: 8) {
    source += "    " + steps[chunk..<min(chunk + 8, steps.count)].map { String(format: "%.4f,", $0) }.joined(separator: " ") + "\n"
}
source += "];\n\n"
source += rust("MONOSPACE", "/// Widths under a monospace face (Menlo first).\n", monospace) + "\n"
source += rust("PROPORTIONAL", "/// Widths under a proportional face (the system's default list).\n", proportional)
try source.write(toFile: arguments[2], atomically: true, encoding: .utf8)
print("monospace \(monospace.count) runs, proportional \(proportional.count) runs, \(steps.count) emoji steps")
