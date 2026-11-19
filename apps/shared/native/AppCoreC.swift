// Firm reason: 6 (the shared compiled core ABI, used by Apple native hosts).
import Foundation

@MainActor final class AppCoreC: AppCoreABI {
    let raw: UnsafeMutableRawPointer
    /// Runs after every call into the core: the page's login flow follows the core (chat_step).
    var after: (() -> Void)?
    init?() { guard let raw = oa_core_open() else { return nil }; self.raw = raw }
    deinit { oa_core_close(raw) }
    private var mono: Int64 { AppWake.steadyMs() }
    private var wall: Int64 { AppWake.wallMs() }
    private func bytes(_ read: (UnsafeMutablePointer<UInt8>, Int64) -> Int64) -> Data? {
        var out = [UInt8](repeating: 0, count: 1 << 18)
        let n = read(&out, Int64(out.count))
        return n < 0 ? nil : Data(out[0..<Int(n)])
    }
    func next() -> Int64 { oa_core_next(raw) }
    func request(_ op: Int64) -> [Int64]? {
        var f = [Int64](repeating: 0, count: AppCoreField.count)
        return oa_core_request(raw, op, &f) != 0 ? f : nil
    }
    func path(_ op: Int64) -> Data? { bytes { oa_core_path(raw, op, $0, $1) } }
    func text(_ op: Int64) -> Data? { bytes { oa_core_text(raw, op, $0, $1) } }
    func body(_ op: Int64) -> Data? { bytes { oa_core_body(raw, op, $0, $1) } }
    func taken(_ op: Int64, accepted: Bool) { _ = oa_core_taken(raw, mono, wall, op, accepted ? 1 : 0); after?() }
    func cancelNext() -> Int64 { oa_core_cancel_next(raw) }
    func done(op: Int64, generation: Int64, outcome: Int32, status: Int32, flag: Bool, text: Data, more: Data, value: Int64) {
        let t = [UInt8](text), m = [UInt8](more)
        _ = oa_core_done(raw, mono, wall, op, generation, Int64(outcome), Int64(status), flag ? 1 : 0, t, Int64(t.count), m, Int64(m.count), value); after?()
    }
    func wake() -> Int64 { oa_core_wake(raw) }
    func clock() { _ = oa_core_clock(raw, mono, wall); after?() }
    func lifecycle(_ code: Int32) { _ = oa_core_lifecycle(raw, mono, wall, Int64(code)); after?() }
    @discardableResult func setProfile(_ profile: Int64) -> Bool { oa_core_set_profile(raw, profile) != 0 }
    func memoryWarning() { _ = oa_core_memory_warning(raw, mono, wall); after?() }
    func launch(rejoin: String) { _ = oa_core_launch(raw, mono, wall, rejoin); after?() }
    /// A person's action the platform owns (the editor's text, a hardware Return).
    @discardableResult func action(_ code: Int64, flag: Bool = false, text: String = "", more: String = "") -> Int64 {
        let result = oa_core_action(raw, mono, wall, code, flag ? 1 : 0, text, more); after?(); return result
    }
    func dirty(_ mask: Int64) -> Int64 { oa_core_dirty_take(raw, mask) }
    /// The conversation to rejoin at the next launch (app_host_keep): an ID, at most 193 bytes.
    func keep() -> String {
        var out = [UInt8](repeating: 0, count: 256)
        let n = oa_core_keep(raw, &out, Int64(out.count))
        return n > 0 ? String(decoding: out[0..<Int(n)], as: UTF8.self) : ""
    }
}
