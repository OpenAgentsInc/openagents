// Deterministic checks for the native sensor lifecycle. No app, Core Motion
// hardware, model, or network connection is started by this executable.
import Foundation
import CoreMotion
import simd

@MainActor
private final class FixtureSource: DeviceMotionSource {
    var available = true
    var starts = 0
    var stops = 0
    var sample: DeviceMotionSample?
    func start() { starts += 1 }
    func stop() { stops += 1 }
    func latest(now: TimeInterval) -> DeviceMotionSample? { sample }
}

// Feed Core Motion's public decoding initializer its quaternion archive fields.
// This exercises Apple's quaternion-to-DCM implementation without a sensor.
private final class AttitudeFixtureDecoder: NSCoder {
    private let quaternion: [Double]
    init(_ quaternion: [Double]) { self.quaternion = quaternion }
    override var allowsKeyedCoding: Bool { true }
    override func decodeDouble(forKey key: String) -> Double {
        switch key {
        case "kCMAttitudeCodingKeyQX": return quaternion[0]
        case "kCMAttitudeCodingKeyQY": return quaternion[1]
        case "kCMAttitudeCodingKeyQZ": return quaternion[2]
        case "kCMAttitudeCodingKeyQW": return quaternion[3]
        default: preconditionFailure("The Core Motion fixture archive format changed: \(key)")
        }
    }
}

@main
struct DeviceMotionChecks {
    @MainActor
    static func main() throws {
        let source = FixtureSource()
        let driver = DeviceMotionDriver(source: source)
        try check(try driver.poll(now: 0) == nil, "Inactive adapter does not read a sensor.")
        try check(try driver.setNeeded(true, now: 10), "Starting requires a new Rust baseline.")
        try check(!(try driver.setNeeded(true, now: 10.01)), "Repeated interest does not restart sensors.")
        try check(source.starts == 1, "Exactly one start occurred.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 9)
        try check(try driver.poll(now: 10.1) == nil, "Pre-start samples cannot establish a baseline.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.1)
        try check(try driver.poll(now: 10.1)?.timestamp == 10.1, "Fresh samples retain the hardware timestamp.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.116)
        try check(try driver.poll(now: 10.116)?.timestamp == 10.116,
                  "A fresh sample survives a display callback after 16 ms.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.133333)
        try check(try driver.poll(now: 10.133333)?.timestamp == 10.133333,
                  "A fresh sample survives the next callback after 17.33 ms.")
        try check(try driver.poll(now: 10.2) == nil, "Duplicate samples are not delivered twice.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.6)
        try check(try driver.poll(now: 10.61)?.timestamp == 10.6, "A delayed display callback still delivers a current sample.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.63)
        try check(try driver.poll(now: 11.0) == nil, "A sample older than 250 ms is not delivered.")
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 11.21)
        try check(try driver.poll(now: 11.2) == nil, "A sample more than 5 ms into the future is not delivered.")
        try expectFailure(.noUpdates) { _ = try driver.poll(now: 11.7) }
        try check(try driver.setNeeded(false, now: 11.7), "Stopping invalidates the baseline.")
        try check(!driver.running && source.stops == 1, "Stopping releases the sensor immediately.")
        try check(!(try driver.setNeeded(false, now: 11.8)), "Repeated stop is inert.")
        try check(try driver.setNeeded(true, now: 12), "Resume needs a new baseline.")
        try check(try driver.poll(now: 12.1) == nil, "Resume rejects the old cached attitude.")
        source.sample = nil
        try expectFailure(.noUpdates) { _ = try driver.poll(now: 14.1) }
        _ = try driver.setNeeded(false, now: 14.1)
        source.available = false
        try expectFailure(.unavailable) { _ = try driver.setNeeded(true, now: 15) }
        try check(source.starts == 2 && source.stops == 2, "Unavailable hardware never starts.")
        source.available = true
        _ = try driver.setNeeded(true, now: 16)
        source.sample = DeviceMotionSample(quaternion: [Double.nan, 0, 0, 1], timestamp: 16.1)
        try expectFailure(.invalidSample) { _ = try driver.poll(now: 16.1) }
        _ = try driver.setNeeded(false, now: 16.1)
        let preview = PreviewDeviceMotionSource()
        let fixtureDriver = DeviceMotionDriver(source: preview)
        _ = try fixtureDriver.setNeeded(true, now: 20)
        try check(try fixtureDriver.poll(now: 20.1)?.quaternion == [sqrt(0.5), 0, 0, sqrt(0.5)],
                  "The explicit fixture starts in upright portrait orientation.")
        let expectedPoses: [(String, [Double], [Double])] = [
            ("upright", [0, -1, 0], [0, 1, 0]),
            ("left", [0, -1, 0], [-sin(0.4), cos(0.4), 0]),
            ("right", [0, -1, 0], [sin(0.4), cos(0.4), 0]),
            ("up", [0, -cos(0.8), sin(0.8)], [0, cos(0.8), sin(0.8)]),
        ]
        for (index, pose) in expectedPoses.enumerated() {
            let sample = try fixtureDriver.poll(now: 20.2 + Double(index) * 0.1)
            guard let sample else { throw Failure(message: "The motion fixture did not deliver a sample.") }
            try checkNativeAttitude(sample.quaternion, name: pose.0, gravity: pose.1, forward: pose.2)
            preview.advance()
        }
        try check(try fixtureDriver.poll(now: 20.7)?.quaternion == [sqrt(0.5), 0, 0, sqrt(0.5)],
                  "The fixture returns to its upright pose after both turns and looking up.")
        _ = try fixtureDriver.setNeeded(false, now: 20.8)
        print("Native motion checks passed: uneven 60 Hz callbacks, fresh receipt timestamps, pause/resume, unavailable hardware, invalid samples, and Core Motion gravity and direction fixtures.")
    }

    private static func checkNativeAttitude(_ quaternion: [Double], name: String,
                                            gravity: [Double], forward: [Double]) throws {
        guard let attitude = CMAttitude(coder: AttitudeFixtureDecoder(quaternion)) else {
            throw Failure(message: "Core Motion refused the attitude fixture.")
        }
        let matrix = attitude.rotationMatrix
        // Apple defines device gravity as DCM * reference gravity (0, 0, -1).
        // Its transposed DCM maps the device's back (-Z) into the reference.
        let nativeGravity = [-matrix.m13, -matrix.m23, -matrix.m33]
        let nativeForward = [-matrix.m31, -matrix.m32, -matrix.m33]
        let raw = attitude.quaternion
        let hamilton = simd_quatd(ix: raw.x, iy: raw.y, iz: raw.z, r: raw.w)
        let projected = hamilton.act(SIMD3<Double>(0, 0, -1))
        for index in 0..<3 {
            try check(abs(nativeGravity[index] - gravity[index]) < 0.000001,
                      "Core Motion confirms gravity for the \(name) fixture.")
            try check(abs(nativeForward[index] - forward[index]) < 0.000001,
                      "Core Motion confirms the viewing direction for the \(name) fixture.")
            try check(abs(projected[index] - nativeForward[index]) < 0.000001,
                      "The raw Hamilton quaternion maps device coordinates into the reference.")
        }
        print("Core Motion \(name): raw quaternion \(quaternion), gravity \(nativeGravity), forward \(nativeForward)")
    }

    private static func check(_ condition: Bool, _ message: String) throws {
        if !condition { throw Failure(message: message) }
    }
    private static func expectFailure(_ expected: DeviceMotionFailure, action: () throws -> Void) throws {
        do { try action(); throw Failure(message: "Expected motion refusal.") }
        catch let error as DeviceMotionFailure {
            try check(String(describing: error) == String(describing: expected), "Unexpected motion refusal.")
        }
    }
    private struct Failure: Error { let message: String }
}
