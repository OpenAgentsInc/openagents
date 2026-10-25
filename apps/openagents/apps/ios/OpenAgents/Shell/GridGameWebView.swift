import UIKit
import WebKit

/// Game input stays with the world. WebKit text tools and page zoom do not
/// compete with sticks, holds, or repeated control taps.
final class GridGameWebView: WKWebView {
    static func configuration() -> WKWebViewConfiguration {
        let config = WKWebViewConfiguration()
        config.preferences.isTextInteractionEnabled = false
        config.dataDetectorTypes = []
        return config
    }
    override init(frame: CGRect, configuration: WKWebViewConfiguration) {
        super.init(frame:frame,configuration:configuration)
        allowsLinkPreview = false
        isFindInteractionEnabled = false
        isOpaque = false
        backgroundColor = .black
        scrollView.backgroundColor = .black
        scrollView.isScrollEnabled = false
        scrollView.bounces = false
        scrollView.pinchGestureRecognizer?.isEnabled = false
        scrollView.contentInsetAdjustmentBehavior = .never
    }
    func enforceGameInput() {
        if let pinch = scrollView.pinchGestureRecognizer, pinch.isEnabled { pinch.isEnabled = false }
    }
    required init?(coder: NSCoder) { fatalError() }
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool { false }
}
