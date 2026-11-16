// Firm reason: 6 (platform timers, clocks and the system's random bytes).
// Wakes, clocks and random identities for a compiled Coax app (GitHub #13645). The app asks to be
// woken after a delay; nothing runs and no frame is presented until then. A wake answers once on
// the main thread, or never when it is cancelled.
// Main thread only.
//   apps/shared/native/app-services-check.sh
import Foundation
import Security

@MainActor final class AppWake {
    private struct Entry { var generation: UInt64; var timer: DispatchSourceTimer }
    private var entries: [UInt64: Entry] = [:]

    /// How many wakes are waiting.
    var waiting: Int { entries.count }

    /// Answers once after `milliseconds`. A second wake with a live op replaces the first.
    func after(op: UInt64, generation: UInt64, milliseconds: Int, fire: @escaping () -> Void) {
        cancel(op: op)
        let ms = max(0, min(milliseconds, 24 * 3600 * 1000))
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + .milliseconds(ms), leeway: .milliseconds(max(1, ms / 10)))
        timer.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self, let e = self.entries.removeValue(forKey: op), e.generation == generation else { return }
                e.timer.cancel(); fire()
            }
        }
        entries[op] = Entry(generation: generation, timer: timer)
        timer.resume()
    }
    func cancel(op: UInt64) { entries.removeValue(forKey: op)?.timer.cancel() }
    /// Ends every wake of an owner generation (0: every wake).
    func cancel(generation: UInt64) {
        for (op, e) in entries where generation == 0 || e.generation == generation { _ = e; cancel(op: op) }
    }

    /// Milliseconds since 1970 by the wall clock.
    nonisolated static func wallMs() -> Int64 { Int64((Date().timeIntervalSince1970 * 1000).rounded()) }
    /// Milliseconds that only go forward, counted while the device sleeps too.
    nonisolated static func steadyMs() -> Int64 { Int64(clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW) / 1_000_000) }
    /// `count` random bytes from the system as lower-case hex, nil when the system has none.
    nonisolated static func randomHex(_ count: Int) -> String? {
        var bytes = [UInt8](repeating: 0, count: max(0, min(count, 64)))
        guard !bytes.isEmpty, SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else { return nil }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
