// Firm reason: 6 (UIKit: the clipboard, opening a link, the app's lifecycle and the text size).
// The device, for a compiled Coax app (GitHub #13645): the two effects that need UIKit (write the
// clipboard, open a link outside the app; each only when the app's Coax asks after an explicit
// action) and the facts the app is told (it came to the front, it left, memory is low, the
// person's text size changed). It never reads the clipboard.
// Main thread only.
import UIKit

@MainActor final class AppDevice: AppPlatform {
    /// What happened to the app, as the Coax is told.
    enum Fact: Int32 { case foreground = 1, background = 2, memoryWarning = 3, textSize = 4 }
    /// Called on the main thread, outside any dispatch into Coax.
    var fact: ((Fact) -> Void)?
    private var watches: [NSObjectProtocol] = []

    init() {
        let center = NotificationCenter.default
        func watch(_ name: Notification.Name, _ fact: Fact) {
            watches.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.fact?(fact) } })
        }
        watch(UIApplication.didBecomeActiveNotification, .foreground)
        watch(UIApplication.didEnterBackgroundNotification, .background)
        watch(UIApplication.didReceiveMemoryWarningNotification, .memoryWarning)
        watch(UIContentSizeCategory.didChangeNotification, .textSize)
    }
    deinit { for w in watches { NotificationCenter.default.removeObserver(w) } }

    func copy(_ text: String) -> Bool { UIPasteboard.general.string = text; return true }

    /// Only an explicit Coax effect presents this system chooser. No automatic recipient.
    var presentation: (() -> UIViewController?)?
    func share(_ text: String, done: @escaping (AppCompletion.State) -> Void) {
        guard let presenter=presentation?(), presenter.presentedViewController == nil else {done(.failed);return}
        let sheet=UIActivityViewController(activityItems:[text], applicationActivities:nil)
        sheet.popoverPresentationController?.sourceView=presenter.view
        sheet.popoverPresentationController?.sourceRect=CGRect(x:presenter.view.bounds.midX,y:presenter.view.bounds.maxY-44,width:1,height:1)
        sheet.completionWithItemsHandler={_,sent,_,error in DispatchQueue.main.async {done(error != nil ? .failed : sent ? .done : .cancelled)}}
        presenter.present(sheet,animated:true)
    }

    /// Opens a web link in the person's browser. Anything that is not http or https is refused
    /// here too, whatever the app's own link rule admitted.
    func open(_ link: String, done: @escaping (Bool) -> Void) {
        guard let url = URL(string: link), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http", url.host?.isEmpty == false else { done(false); return }
        UIApplication.shared.open(url, options: [:]) { opened in DispatchQueue.main.async { done(opened) } }
    }

    /// The person's text size as a multiple of the default (Dynamic Type): 1 at Large.
    static var textScale: Float { Float(UIFontMetrics(forTextStyle: .body).scaledValue(for: 100) / 100) }
    static var boldText: Bool { UIAccessibility.isBoldTextEnabled }
    static var reduceMotion: Bool { UIAccessibility.isReduceMotionEnabled }
    static var increaseContrast: Bool { UIAccessibility.isDarkerSystemColorsEnabled }
}
