// Firm reason: 6 (UIKit presentation and the display link for a second Metal view).
// The dev menu (GitHub #13645): the app set's gallery (components/app/gallery/gallery.coax), every
// GPU component on example data, over the app. Debug builds only. It opens when:
//   - the app is launched with the argument `-OADevGallery YES` (or the environment variable
//     OA_DEV_GALLERY=1), for example `xcrun simctl launch booted com.openagents.app -OADevGallery YES`;
//   - three fingers double-tap anywhere on the screen.
// Close at the top right of the gallery dismisses it.
//
// The one call the app makes, at the end of its root screen's viewDidLoad:
//   DevGallery.install(on: self)
// It needs the gallery's surface in the build: app.coax loads
// ../../components/app/gallery/gallery.coax, host.c includes
// ../../components/app/gallery/host.c, Bridge.h includes ../../components/app/gallery/host.h, and
// project.yml lists this file (components/app/INTEGRATION.md).
import UIKit

@MainActor enum DevGallery {
    static func install(on screen:UIViewController) {
        #if DEBUG
        let opener=DevGalleryOpener(screen)
        let tap=UITapGestureRecognizer(target:opener,action:#selector(DevGalleryOpener.open));tap.numberOfTouchesRequired=3;tap.numberOfTapsRequired=2
        tap.cancelsTouchesInView=false;screen.view.addGestureRecognizer(tap);openers.append(opener)
        if UserDefaults.standard.bool(forKey:"OADevGallery") || ProcessInfo.processInfo.environment["OA_DEV_GALLERY"]=="1" {DispatchQueue.main.async {opener.open()}}
        #endif
    }
    #if DEBUG
    private static var openers:[DevGalleryOpener]=[]
    #endif
}

#if DEBUG
@MainActor private final class DevGalleryOpener:NSObject {
    weak var screen:UIViewController?
    init(_ screen:UIViewController) {self.screen=screen}
    @objc func open() {
        guard let screen,screen.presentedViewController==nil else {return}
        // At launch viewDidLoad runs before the screen is in a window, and a present then does nothing.
        if screen.view.window==nil {DispatchQueue.main.asyncAfter(deadline:.now()+0.1) { [weak self] in self?.open() };return}
        let gallery=DevGalleryScreen();gallery.modalPresentationStyle = .fullScreen;screen.present(gallery,animated:true)
    }
}

@MainActor private final class DevGalleryScreen:UIViewController {
    private var owner:UnsafeMutableRawPointer?
    private var display:CADisplayLink?

    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black;view.accessibilityIdentifier="dev.gallery"
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({og_open($0)}),let raw=og_view(owner) else {
            let label=UILabel();label.text="The component gallery could not start.";label.textColor = .label;label.textAlignment = .center
            label.frame=view.bounds;label.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(label);return
        }
        self.owner=owner
        og_on_change(owner,{ context in
            guard let context else {return}
            let screen=Unmanaged<DevGalleryScreen>.fromOpaque(context).takeUnretainedValue()
            MainActor.assumeIsolated {screen.wake()}
        },Unmanaged.passUnretained(self).toOpaque())
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue()
        canvas.frame=view.bounds;canvas.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(canvas)
    }
    override func viewDidAppear(_ animated:Bool) {super.viewDidAppear(animated);wake()}
    override func viewDidLayoutSubviews() {super.viewDidLayoutSubviews();wake()}
    override func viewWillDisappear(_ animated:Bool) {super.viewWillDisappear(animated);display?.invalidate();display=nil}
    override var preferredStatusBarStyle:UIStatusBarStyle {.lightContent}
    deinit {if let owner {og_close(owner)}}

    func wake() {
        guard owner != nil,viewIfLoaded?.window != nil else {return}
        if display==nil {
            let link=CADisplayLink(target:DevGalleryTick(self),selector:#selector(DevGalleryTick.tick));link.add(to:.main,forMode:.common);display=link
        }
        display?.isPaused=false
    }
    fileprivate func step() {
        guard let owner else {return}
        let insets=view.safeAreaInsets
        og_env(owner,Float(insets.top),Float(insets.bottom),UIAccessibility.isReduceMotionEnabled ? 1:0)
        og_frame(owner)
        if og_action(owner)==1 {display?.invalidate();display=nil;dismiss(animated:true);return}
        if og_moving(owner)==0 {display?.isPaused=true}
    }
}

/// The display link's target, so the link does not keep the screen alive.
@MainActor private final class DevGalleryTick:NSObject {
    weak var screen:DevGalleryScreen?
    init(_ screen:DevGalleryScreen) {self.screen=screen}
    @objc func tick() {screen?.step()}
}
#endif
