// Deterministic checks for the native sensor lifecycle. No app, Core Motion
// hardware, model, or network connection is started by this executable.
import Foundation

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
        source.sample = DeviceMotionSample(quaternion: [0, 0, 0, 1], timestamp: 10.11)
        try check(try driver.poll(now: 10.11) == nil, "Delivery is capped at 30 Hz.")
        try check(try driver.poll(now: 10.14)?.timestamp == 10.11, "A later display tick takes the latest sample.")
        try check(try driver.poll(now: 10.2) == nil, "Duplicate samples are not delivered twice.")
        try expectFailure(.noUpdates) { _ = try driver.poll(now: 11.3) }
        try check(try driver.setNeeded(false, now: 11.3), "Stopping invalidates the baseline.")
        try check(!driver.running && source.stops == 1, "Stopping releases the sensor immediately.")
        try check(!(try driver.setNeeded(false, now: 11.4)), "Repeated stop is inert.")
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
        try check(try fixtureDriver.poll(now: 20.1)?.quaternion == [-sqrt(0.5), 0, 0, sqrt(0.5)],
                  "The explicit fixture starts in upright portrait orientation.")
        _ = try fixtureDriver.setNeeded(false, now: 20.2)
        print("Native motion lifecycle checks passed: bounded delivery, fresh timestamps, pause/resume, unavailable hardware, invalid samples, and explicit fixture.")
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
