import Foundation

#if os(iOS)
import UIKit
import CoreHaptics

enum PerformanceWarmup {
    /// Preload Berkeley Mono and prime CoreText glyph caches.
    static func preloadMonoFont() {
        let name = BerkeleyFont.defaultName()
        // Create and render a tiny offscreen label to force layout and glyph rasterization
        let label = UILabel(frame: CGRect(x: 0, y: 0, width: 1, height: 1))
        label.font = UIFont(name: name, size: 16) ?? UIFont.monospacedSystemFont(ofSize: 16, weight: .regular)
        label.text = "Warmup"
        label.sizeToFit()
        UIGraphicsBeginImageContextWithOptions(CGSize(width: 1, height: 1), false, 0)
        if let ctx = UIGraphicsGetCurrentContext() {
            label.layer.render(in: ctx)
        }
        UIGraphicsEndImageContext()
    }

    /// Proactively initialize haptic/feedback engines so the first keyboard focus doesn't pay the cost.
    static func prewarmHaptics() {
        // Keep strong references so generators remain prepared for a while
        HapticsHolder.shared.prepare()
    }
}

private final class HapticsHolder {
    static let shared = HapticsHolder()
    private var selection: UISelectionFeedbackGenerator?
    private var impact: UIImpactFeedbackGenerator?
    private var engine: CHHapticEngine?

    func prepare() {
        // Prepare UIFeedback generators (lightweight)
        let s = UISelectionFeedbackGenerator(); s.prepare(); self.selection = s
        let i = UIImpactFeedbackGenerator(style: .light); i.prepare(); self.impact = i

        // Spin up and stop a CHHapticEngine to front‑load its initialization costs (if supported)
        if CHHapticEngine.capabilitiesForHardware().supportsHaptics {
            do {
                let eng = try CHHapticEngine()
                try? eng.start()
                eng.notifyWhenPlayersFinished { _ in .stopEngine }
                self.engine = eng
            } catch {
                // Ignore errors on simulator / unsupported devices
            }
        }
    }
}
#endif

