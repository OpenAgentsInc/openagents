// CoreText's line breaks for the shaping corpus, with the bundled fonts.
//
//   swift crates/rust-native/tools/coretext-lines.swift CORPUS.json \
//       crates/paper-mono/fonts crates/rust-native/fixtures/coretext-lines.json
//
// CORPUS.json comes from the rust-native test `shape::tests::write_corpus`.
// Each case is a paragraph, its runs as [face, size, wght, opsz, calt,
// start16, end16], and a wrap width (0 breaks only at hard breaks). Lines
// break exactly as the iOS measurer does (NativeTranscriptPainter.swift,
// nativeLayoutMeasure): CTTypesetterSuggestLineBreak, then a cluster break
// when no word fits. The output ties itself to the corpus by an FNV-1a
// digest of CORPUS.json's bytes. macOS's CoreText is the iOS engine.
import CoreText
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 4 else {
    FileHandle.standardError.write("usage: coretext-lines.swift CORPUS.json FONTS_DIR OUT.json\n".data(using: .utf8)!)
    exit(64)
}
let corpusData = try Data(contentsOf: URL(fileURLWithPath: arguments[1]))
let fontsDir = URL(fileURLWithPath: arguments[2])
let faceFiles = ["PaperMono-Variable.ttf"]
let faces: [CTFontDescriptor] = try faceFiles.map { name in
    let data = try Data(contentsOf: fontsDir.appendingPathComponent(name))
    guard let descriptors = CTFontManagerCreateFontDescriptorsFromData(data as CFData) as? [CTFontDescriptor],
          let first = descriptors.first else { fatalError("unreadable face \(name)") }
    return first
}

func tag(_ name: String) -> Int {
    name.unicodeScalars.reduce(0) { $0 << 8 | Int($1.value) }
}

var fonts: [String: CTFont] = [:]
func font(face: Int, size: Double, weight: Double, optical: Double, calt: Bool) -> CTFont {
    let key = "\(face)/\(size)/\(weight)/\(optical)/\(calt)"
    if let found = fonts[key] { return found }
    var variation: [Int: Double] = [tag("wght"): weight]
    if optical > 0 { variation[tag("opsz")] = optical }
    var attributes: [CFString: Any] = [kCTFontVariationAttribute: variation]
    if !calt {
        attributes[kCTFontFeatureSettingsAttribute] = [[kCTFontOpenTypeFeatureTag: "calt",
                                                       kCTFontOpenTypeFeatureValue: 0]]
    }
    let descriptor = CTFontDescriptorCreateCopyWithAttributes(faces[face], attributes as CFDictionary)
    let made = CTFontCreateWithFontDescriptor(descriptor, CGFloat(size), nil)
    fonts[key] = made
    return made
}

var digest: UInt64 = 0xcbf2_9ce4_8422_2325
for byte in corpusData {
    digest ^= UInt64(byte)
    digest = digest &* 0x0100_0000_01b3
}

let corpus = try JSONSerialization.jsonObject(with: corpusData) as! [String: Any]
let cases = corpus["cases"] as! [[String: Any]]
var out: [[String: Any]] = []
for item in cases {
    let text = item["text"] as! String
    let runs = item["runs"] as! [[Any]]
    let width = (item["width"] as! NSNumber).doubleValue
    let attributed = NSMutableAttributedString(string: text)
    for run in runs {
        let face = (run[0] as! NSNumber).intValue
        let size = (run[1] as! NSNumber).doubleValue
        let weight = (run[2] as! NSNumber).doubleValue
        let optical = (run[3] as! NSNumber).doubleValue
        let calt = run[4] as! Bool
        let start = (run[5] as! NSNumber).intValue
        let end = (run[6] as! NSNumber).intValue
        attributed.addAttribute(.init(kCTFontAttributeName as String),
                                value: font(face: face, size: size, weight: weight, optical: optical, calt: calt),
                                range: NSRange(location: start, length: end - start))
    }
    let length = attributed.length
    let typesetter = CTTypesetterCreateWithAttributedString(attributed)
    let limit = width > 0 ? width : 1.0e7
    var ends: [Int] = []
    var widths: [Double] = []
    var start = 0
    while start < length {
        var count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
        if count <= 0 { count = max(1, CTTypesetterSuggestClusterBreak(typesetter, start, limit)) }
        count = min(count, length - start)
        let line = CTTypesetterCreateLine(typesetter, CFRange(location: start, length: count))
        let typographic = CTLineGetTypographicBounds(line, nil, nil, nil)
        let trailing = CTLineGetTrailingWhitespaceWidth(line)
        start += count
        ends.append(start)
        widths.append((max(0, typographic - trailing) * 1000).rounded() / 1000)
    }
    out.append(["ends": ends, "widths": widths])
}
let result: [String: Any] = ["corpus": String(format: "%016llx", digest), "cases": out]
let data = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
try data.write(to: URL(fileURLWithPath: arguments[3]))
print("wrote \(out.count) cases")
