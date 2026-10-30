import CoreMotion
import UIKit

/// The phone's turn (the gyro) as camera look, in radians, the same way a
/// right-thumb drag looks: yaw about the world's up (a turn to the right is
/// negative, as a drag to the right), pitch about the screen's across
/// (tilting the top back to look up is positive, as a drag up).
final class PhoneTurn {
    private let motion = CMMotionManager()
    private var yaw = 0.0, pitch = 0.0
    weak var view: UIView?
    var use = "camera"   // what the turn moves, for the app's analytics (once each time on screen)

    func start() {
        guard motion.isDeviceMotionAvailable, !motion.isDeviceMotionActive else { return }
        motion.deviceMotionUpdateInterval = 1.0 / 60
        motion.startDeviceMotionUpdates(to: .main) { [weak self] m, _ in if let m { self?.add(m) } }
    }
    func stop() { motion.stopDeviceMotionUpdates(); yaw = 0; pitch = 0 }

    /// The turn since the last call.
    func take() -> (yaw: Double, pitch: Double) {
        defer { yaw = 0; pitch = 0 }
        return (yaw, pitch)
    }

    private func add(_ m: CMDeviceMotion) {
        let r = m.rotationRate, g = m.gravity, dt = motion.deviceMotionUpdateInterval
        let len = (g.x * g.x + g.y * g.y + g.z * g.z).squareRoot()
        guard len > 0.1 else { return }
        if abs(r.x) + abs(r.y) + abs(r.z) > 0.6 { AppAnalytics.shared.gyro(use) }
        let up = (x: -g.x / len, y: -g.y / len, z: -g.z / len)
        yaw += (r.x * up.x + r.y * up.y + r.z * up.z) * dt
        // The screen's across, in the phone's own axes, for how it's held.
        let across: (Double, Double)
        switch view?.window?.windowScene?.effectiveGeometry.interfaceOrientation ?? .portrait {
        case .landscapeLeft: across = (0, 1)
        case .landscapeRight: across = (0, -1)
        case .portraitUpsideDown: across = (-1, 0)
        default: across = (1, 0)
        }
        pitch -= (r.x * across.0 + r.y * across.1) * dt
    }
}
