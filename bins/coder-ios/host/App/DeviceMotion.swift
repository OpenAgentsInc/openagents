// Core Motion supplies raw portrait-device attitude. Rust owns its camera
// interpretation; this adapter only bounds delivery and owns sensor lifetime.
import Foundation
#if os(iOS)
import CoreMotion
#endif

struct DeviceMotionSample {
    let quaternion: [Double]
    let timestamp: TimeInterval
}

@MainActor
protocol DeviceMotionSource: AnyObject {
    var available: Bool { get }
    func start()
    func stop()
    func latest(now: TimeInterval) -> DeviceMotionSample?
}

@MainActor
final class DeviceMotionDriver {
    private let source: DeviceMotionSource
    private(set) var running = false
    private var startedAt: TimeInterval = 0
    private var lastTimestamp: TimeInterval?
    private var lastFreshAt: TimeInterval?
    var available: Bool { source.available }

    init(source: DeviceMotionSource) { self.source = source }

    // A true return means Rust must discard its previous sensor baseline.
    @discardableResult
    func setNeeded(_ needed: Bool, now: TimeInterval) throws -> Bool {
        if !needed {
            let changed = running
            if running { source.stop() }
            running = false
            lastTimestamp = nil
            lastFreshAt = nil
            return changed
        }
        guard !running else { return false }
        guard available else { throw DeviceMotionFailure.unavailable }
        guard now.isFinite else { throw DeviceMotionFailure.invalidSample }
        source.start()
        running = true
        startedAt = now
        lastTimestamp = nil
        lastFreshAt = nil
        return true
    }

    func poll(now: TimeInterval) throws -> DeviceMotionSample? {
        guard running else { return nil }
        guard now.isFinite, now >= startedAt else { throw DeviceMotionFailure.invalidSample }
        // The display link bounds polling to 60 Hz. A second interval gate
        // would drop fresh samples when display callbacks arrive unevenly.
        if let sample = source.latest(now: now) {
            guard sample.quaternion.count == 4, sample.quaternion.allSatisfy(\.isFinite),
                  sample.timestamp.isFinite else { throw DeviceMotionFailure.invalidSample }
            // A cached pre-start value is not a new baseline. Duplicates and old
            // values cannot keep a stopped or denied sensor looking healthy.
            if sample.timestamp >= startedAt,
               sample.timestamp <= now + 0.005, now - sample.timestamp <= 0.25,
               lastTimestamp.map({ sample.timestamp > $0 }) ?? true {
                lastTimestamp = sample.timestamp
                lastFreshAt = now
                return sample
            }
        }
        let allowance = lastFreshAt == nil ? 2.0 : 1.0
        if now - (lastFreshAt ?? startedAt) > allowance { throw DeviceMotionFailure.noUpdates }
        return nil
    }
}

enum DeviceMotionFailure: LocalizedError {
    case unavailable, invalidSample, noUpdates
    var errorDescription: String? {
        switch self {
        case .unavailable: return "Motion look is unavailable on this device. Touch look is still available."
        case .invalidSample: return "Motion data could not be read. Touch look is active."
        case .noUpdates: return "Motion updates stopped or were denied. Touch look is active."
        }
    }
}

// Explicit UI fixtures provide no evidence that a physical sensor is present.
@MainActor
final class PreviewDeviceMotionSource: DeviceMotionSource {
    var available: Bool { true }
    private var running = false
    private var index = 0
    // Raw Core Motion quaternions: upright portrait, body left/right by 0.4
    // radians about gravity, then look up by 0.8 radians. Native checks verify
    // their gravity vectors through CMAttitude rather than the camera mapping.
    private let samples: [[Double]] = [
        [sqrt(0.5), 0, 0, sqrt(0.5)],
        [0.6930117232058353, 0.14048043101898117, 0.1404804310189812, 0.6930117232058354],
        [0.6930117232058353, -0.14048043101898117, -0.1404804310189812, 0.6930117232058354],
        [0.926648825310733, 0, 0, 0.37592812418099114],
    ]
    func start() { running = true }
    func stop() { running = false }
    func latest(now: TimeInterval) -> DeviceMotionSample? {
        running ? DeviceMotionSample(quaternion: samples[index], timestamp: now) : nil
    }
    func advance() { index = (index + 1) % samples.count }
}

#if os(iOS)
@MainActor
final class CoreMotionSource: DeviceMotionSource {
    private let manager = CMMotionManager()
    var available: Bool {
        manager.isDeviceMotionAvailable &&
        CMMotionManager.availableAttitudeReferenceFrames().contains(.xArbitraryZVertical)
    }
    func start() {
        manager.deviceMotionUpdateInterval = 1.0 / 60.0
        manager.showsDeviceMovementDisplay = false
        manager.startDeviceMotionUpdates(using: .xArbitraryZVertical)
    }
    func stop() { manager.stopDeviceMotionUpdates() }
    func latest(now: TimeInterval) -> DeviceMotionSample? {
        guard let data = manager.deviceMotion else { return nil }
        let q = data.attitude.quaternion
        return DeviceMotionSample(quaternion: [q.x, q.y, q.z, q.w], timestamp: data.timestamp)
    }
}
#endif
