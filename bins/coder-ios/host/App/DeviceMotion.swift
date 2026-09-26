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
    private var lastDelivery: TimeInterval?
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
            lastDelivery = nil
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
        lastDelivery = nil
        lastTimestamp = nil
        lastFreshAt = nil
        return true
    }

    func poll(now: TimeInterval) throws -> DeviceMotionSample? {
        guard running else { return nil }
        guard now.isFinite, now >= startedAt else { throw DeviceMotionFailure.invalidSample }
        if let lastDelivery, now - lastDelivery < 1.0 / 30.0 { return nil }
        if let sample = source.latest(now: now) {
            guard sample.quaternion.count == 4, sample.quaternion.allSatisfy(\.isFinite),
                  sample.timestamp.isFinite else { throw DeviceMotionFailure.invalidSample }
            // A cached pre-start value is not a new baseline. Duplicates and old
            // values cannot keep a stopped or denied sensor looking healthy.
            if sample.timestamp >= startedAt,
               sample.timestamp <= now + 0.25, now - sample.timestamp <= 1.0,
               lastTimestamp.map({ sample.timestamp > $0 }) ?? true {
                lastDelivery = now
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
    private var sample: [Double] = [-sqrt(0.5), 0, 0, sqrt(0.5)]
    func start() { running = true }
    func stop() { running = false }
    func latest(now: TimeInterval) -> DeviceMotionSample? {
        running ? DeviceMotionSample(quaternion: sample, timestamp: now) : nil
    }
    func setQuaternion(_ quaternion: [Double]) { sample = quaternion }
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
        manager.deviceMotionUpdateInterval = 1.0 / 30.0
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
