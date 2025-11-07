import Foundation

#if os(iOS)
import UIKit
import CoreHaptics

enum PerformanceWarmup {
    /// Preload Berkeley Mono and prime CoreText glyph caches.
    static func preloadMonoFont() {
        let name = BerkeleyFont.defaultName()
        let font = UIFont(name: name, size: 16) ?? UIFont.monospacedSystemFont(ofSize: 16, weight: .regular)
        // Render a broad ASCII sample to precreate glyphs and layout caches
        let ascii = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789" +
                    " ~`!@#$%^&*()-_=+[{]}\\|;:'\",<.>/?\n \t"
        let attr = NSAttributedString(string: ascii, attributes: [.font: font, .foregroundColor: UIColor.white])
        let size = CGSize(width: 600, height: 200)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        attr.draw(in: CGRect(origin: .zero, size: size))
        UIGraphicsEndImageContext()
        // Also render using a UILabel's layer to warm TextKit path
        let label = UILabel(frame: CGRect(x: 0, y: 0, width: size.width, height: size.height))
        label.numberOfLines = 0
        label.text = ascii
        label.font = font
        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        if let ctx = UIGraphicsGetCurrentContext() { label.layer.render(in: ctx) }
        UIGraphicsEndImageContext()
    }

    /// Proactively initialize haptic/feedback engines so the first keyboard focus doesn't pay the cost.
    static func prewarmHaptics() {
        // Keep strong references so generators remain prepared for a while
        HapticsHolder.shared.prepare()
    }

    /// Create an offscreen text input, focus and resign it to load the keyboard
    /// and associated input subsystems ahead of time.
    static func prewarmKeyboardAndTextInput() {
        // Soft warmup: create and configure a text view, perform layout and trait access
        // without becoming first responder (avoids showing the keyboard to the user).
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            guard let window = UIApplication.shared.connectedScenes
                .compactMap({ $0 as? UIWindowScene })
                .flatMap({ $0.windows })
                .first(where: { $0.isKeyWindow }) ?? UIApplication.shared.windows.first else { return }

            let tv = UITextView(frame: CGRect(x: -1000, y: -1000, width: 10, height: 10))
            tv.isScrollEnabled = false
            tv.autocorrectionType = .no
            tv.spellCheckingType = .no
            tv.autocapitalizationType = .none
            tv.smartDashesType = .no
            tv.smartQuotesType = .no
            tv.smartInsertDeleteType = .no
            tv.dataDetectorTypes = []
            tv.keyboardType = .asciiCapable
            tv.keyboardAppearance = .dark
            tv.text = "Warm"
            tv.alpha = 0.001
            window.addSubview(tv)
            tv.setNeedsLayout(); tv.layoutIfNeeded()
            // Touch active input modes and text checker to warm internal caches
            _ = UITextInputMode.activeInputModes
            let langs = UITextChecker.availableLanguages
            if let lang = langs.first {
                let checker = UITextChecker()
                let text = "Warm up autocorrect"
                let ns = text as NSString
                _ = checker.rangeOfMisspelledWord(in: text, range: NSRange(location: 0, length: ns.length), startingAt: 0, wrap: false, language: lang)
            }
            // Remove immediately without showing keyboard
            tv.removeFromSuperview()
        }
    }

    /// Exercise first-responder path without showing the system keyboard by
    /// attaching a custom empty inputView. This primes input managers while
    /// remaining invisible to the user.
    private static var didResponderWarm = false
    static func prewarmResponderSilently() {
        guard !didResponderWarm else { return }
        didResponderWarm = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            guard let window = UIApplication.shared.connectedScenes
                .compactMap({ $0 as? UIWindowScene })
                .flatMap({ $0.windows })
                .first(where: { $0.isKeyWindow }) ?? UIApplication.shared.windows.first else { return }
            let tv = UITextView(frame: CGRect(x: -1000, y: -1000, width: 10, height: 10))
            tv.inputView = UIView(frame: CGRect(x: 0, y: 0, width: 1, height: 1)) // suppress system keyboard
            tv.autocorrectionType = .no
            tv.spellCheckingType = .no
            tv.autocapitalizationType = .none
            window.addSubview(tv)
            tv.becomeFirstResponder()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                tv.resignFirstResponder()
                tv.removeFromSuperview()
            }
        }
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
