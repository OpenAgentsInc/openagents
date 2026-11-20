// Firm reason: 6 (Swift-to-C ABI glue calling coax/lib/agent/desktop/ via desktop-core-mac.h; holds no logic).
import Foundation

enum DesktopCore {
    static func withCoaxString<R>(_ s: String, _ body: (coax_string) -> R) -> R {
        let bytes = Array(s.utf8)
        if bytes.isEmpty {
            var z: CChar = 0
            return withUnsafePointer(to: &z) { p in
                body(coax_string(data: p, count: 0))
            }
        }
        return bytes.withUnsafeBytes { raw in
            let ptr = raw.bindMemory(to: CChar.self).baseAddress
            return body(coax_string(data: ptr, count: Int64(bytes.count)))
        }
    }

    static func withCoaxData<R>(_ d: Data, _ body: (coax_string) -> R) -> R {
        if d.isEmpty {
            var z: CChar = 0
            return withUnsafePointer(to: &z) { p in
                body(coax_string(data: p, count: 0))
            }
        }
        return d.withUnsafeBytes { raw in
            let ptr = raw.bindMemory(to: CChar.self).baseAddress
            return body(coax_string(data: ptr, count: Int64(d.count)))
        }
    }

    static func withByteBuf<R>(_ buf: inout [UInt8], _ body: (coax_v_u8) -> R) -> R {
        buf.withUnsafeMutableBufferPointer { bp in
            body(coax_v_u8(data: bp.baseAddress, count: Int64(bp.count)))
        }
    }

    static func withMutableBytes<R>(_ buf: inout [UInt8], _ body: (coax_v_u8) -> R) -> R {
        withByteBuf(&buf, body)
    }

    static func withS64Buf<R>(_ buf: inout [Int64], _ body: (coax_v_s64) -> R) -> R {
        buf.withUnsafeMutableBufferPointer { bp in
            body(coax_v_s64(data: bp.baseAddress, count: Int64(bp.count)))
        }
    }

    static func withMutableS64<R>(_ buf: inout [Int64], _ body: (coax_v_s64) -> R) -> R {
        withS64Buf(&buf, body)
    }

    static func callString(_ cap: Int, _ body: (coax_v_u8) -> Int64) -> String {
        var buf = [UInt8](repeating: 0, count: cap)
        let n = withByteBuf(&buf, body)
        if n <= 0 { return "" }
        return String(decoding: buf[0..<min(Int(n), cap)], as: UTF8.self)
    }

    static func stringFromBuf(_ cap: Int = 4096, _ body: (coax_v_u8) -> Int64) -> String {
        callString(cap, body)
    }

    static func cString(from buf: [UInt8]) -> String {
        let n = buf.firstIndex(of: 0) ?? buf.count
        return String(decoding: buf[0..<n], as: UTF8.self)
    }
}
