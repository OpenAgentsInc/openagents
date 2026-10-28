#if DEBUG
import Foundation
import UIKit
import Metal

/// Development-only clock/file adapter. The workload and statistics are Coax.
/// Enabled only by an explicit -OABenchmark launch argument; no screen or service.
final class R0FrameCapture {
    static let shared: R0FrameCapture? = ProcessInfo.processInfo.arguments.contains("-OABenchmark") ? R0FrameCapture() : nil
    private let lock = NSLock()
    private var nextFrame = 0
    private var samples: [String: [Double]] = [:]
    private var completed = 0
    private var errors = 0
    private var publishScheduled = false
    private(set) var finished = false
    var currentFrame: Int { nextFrame - 1 }
    private let warmup = Int(metareal_bench_plan().warmup)
    private let count = Int(metareal_bench_plan().samples)
    static func clock() -> UInt64 { DispatchTime.now().uptimeNanoseconds }
    private init() {}
    func beginFrame(active: Bool) -> Int {
        guard active else { return -1 }
        let frame = nextFrame; nextFrame += 1; return frame
    }
    func record(_ name: String, frame: Int, nanoseconds: Double) {
        guard frame >= warmup && frame < warmup + count else { return }
        lock.lock(); defer { lock.unlock() }
        samples[name, default: []].append(nanoseconds)
    }
    func elapsed(_ name: String, frame: Int, since start: UInt64) {
        guard start != 0 else { return }
        record(name, frame: frame, nanoseconds: Double(Self.clock() - start))
    }
    func gpu(_ command: MTLCommandBuffer, frame: Int) {
        guard frame >= warmup && frame < warmup + count else { return }
        lock.lock(); completed += 1; if command.error != nil { errors += 1 }; lock.unlock()
        if command.gpuStartTime > 0 && command.gpuEndTime > command.gpuStartTime {
            record("gpu", frame: frame, nanoseconds: (command.gpuEndTime - command.gpuStartTime) * 1e9)
        }
    }
    func endFrame(_ frame: Int, since start: UInt64) {
        elapsed("frame_cpu", frame: frame, since: start)
        guard frame == warmup + count - 1, !publishScheduled else { return }
        publishScheduled = true
        // Let the last submitted commands complete; unavailable GPU clocks stay absent.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in self?.publish() }
    }
    private func publish() {
        lock.lock(); let captured = samples, submitted = completed, failures = errors; lock.unlock()
        var metrics: [String: Any] = [:]
        for (name, raw) in captured {
            var values = raw, scratch = [Double](repeating: 0, count: raw.count)
            let stat = values.withUnsafeMutableBufferPointer { values in scratch.withUnsafeMutableBufferPointer { out in
                metareal_bench_stats(coax_v_f64(data: values.baseAddress, count: Int64(values.count)), coax_v_f64(data: out.baseAddress, count: Int64(out.count)))
            } }
            metrics[name] = ["ok": stat.ok, "count": stat.count, "mean_ns": stat.mean, "median_ns": stat.median, "p95_ns": stat.p95, "cv": stat.cv, "samples_ns": raw]
        }
        #if targetEnvironment(simulator)
        let environment = "iOS Simulator"
        #else
        let environment = "iOS device"
        #endif
        let report: [String: Any] = ["schema_version": 1, "environment": environment, "os": UIDevice.current.systemVersion,
            "warmup_frames": warmup, "requested_samples": count, "completed_commands": submitted, "gpu_errors": failures,
            "gpu_clock_available": captured["gpu"] != nil, "metrics": metrics,
            "workload": "metareal_bench_frame drives normal Grid input; real display timestamps and Metal path"]
        do {
            let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("R0-frame.json")
            try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys]).write(to: url, options: .atomic)
            finished = true
        } catch { NSLog("R0 benchmark report: %@", error.localizedDescription) }
    }
}
#endif
