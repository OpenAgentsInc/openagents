import Foundation

#if os(macOS)
import AppKit

enum MacWindowUtils {
    static func fitToScreen(margin: CGFloat = 24) {
        guard let window = NSApp.windows.first else { return }
        let screen = window.screen ?? NSScreen.main
        guard let frame = screen?.visibleFrame.insetBy(dx: margin, dy: margin) else { return }
        window.setFrame(frame, display: true, animate: true)
    }
}
#endif

