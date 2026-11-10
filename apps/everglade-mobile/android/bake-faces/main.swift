// Firm reason: 6 (CoreText has no C API; this holds no product logic).
// Writes the faces' atlas (apps/everglade-desktop/Sources/Faces.swift) to a file for the Android
// app, which has no CoreText: "EGF1", the count of mips, then each mip's width, height and bytes
// (two a texel: Spectral, Cinzel).   bake-faces FONTS_DIR OUT
import Foundation

let args = CommandLine.arguments
guard args.count == 3, let levels = faceLevels(fonts: URL(fileURLWithPath: args[1])) else {
  FileHandle.standardError.write(Data("bake-faces: usage: bake-faces FONTS_DIR OUT (the directory holds Spectral-Medium.ttf and Cinzel.ttf)\n".utf8))
  exit(1)
}
var out = Data("EGF1".utf8)
func word(_ v: Int) { var x = UInt32(v).littleEndian; out.append(Data(bytes: &x, count: 4)) }
word(levels.count)
for l in levels { word(l.width); word(l.height); out.append(contentsOf: l.bytes) }
do { try out.write(to: URL(fileURLWithPath: args[2])) } catch {
  FileHandle.standardError.write(Data("bake-faces: could not write \(args[2])\n".utf8)); exit(1)
}
