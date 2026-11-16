// Firm reason: 6 (UIKit presentation and the display link for a second Metal view).
// The dev menu (GitHub #13645): the app set's gallery (components/app/gallery/gallery.coax), every
// GPU component on example data, over the app. Debug builds only. It opens when:
//   - the app is launched with the argument `-OADevGallery YES` (or the environment variable
//     OA_DEV_GALLERY=1), for example `xcrun simctl launch booted com.openagents.app -OADevGallery YES`;
//     add `-OADevGallerySection g.sidebar` to open it at a section;
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

@MainActor private final class DevGalleryScreen:UIViewController,KitAccessSource {
    private var owner:UnsafeMutableRawPointer?
    private var display:CADisplayLink?
    private var access:KitAccess?

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
        // VoiceOver reads the gallery's committed rows through the one bridge (KitAccess.swift)
        canvas.isAccessibilityElement=false;canvas.accessibilityElementsHidden=true
        let access=KitAccess(container:view,source:self);self.access=access
        access.scroll={ [weak self] direction in
            guard let self,let owner=self.owner else {return false}
            let dir:Int32=direction == .up ? 1:direction == .down ? -1:0
            guard dir != 0,og_scroll_page(owner,dir) != 0 else {return false}
            self.wake();UIAccessibility.post(notification:.pageScrolled,argument:nil);return true
        }
        access.escape={ [weak self] in self?.dismiss(animated:true);return true }
    }
    var accessStamp:Int32 {owner.map {og_rows_stamp($0)} ?? 0}
    var accessLayer:Int {0}
    var accessCount:Int {owner.map {Int(og_rows($0))} ?? 0}
    func accessRow(_ i:Int)->KitRow? {
        guard let owner else {return nil}
        var row=[Float](repeating:0,count:8),id=[CChar](repeating:0,count:64),label=[CChar](repeating:0,count:4100),value=[CChar](repeating:0,count:256)
        guard og_row(owner,Int32(i),&row,&id,Int32(id.count),&label,Int32(label.count)) != 0 else {return nil}
        _ = og_row_value(owner,Int32(i),&value,Int32(value.count))
        return KitRow(id:String(cString:id),label:String(cString:label),value:String(cString:value),role:Int(row[4]),state:Int(row[5]),parent:Int(row[6]),focus:Int(row[7]),
            frame:CGRect(x:CGFloat(row[0]),y:CGFloat(row[1]),width:CGFloat(row[2]),height:CGFloat(row[3])))
    }
    func accessActivate(_ id:String)->Bool {guard let owner else {return false};let done=og_activate(owner,id) != 0;wake();return done}
    override func viewDidAppear(_ animated:Bool) {
        super.viewDidAppear(animated);wake()
        // `-OADevGallerySection g.sidebar` opens the gallery at that section, once it has drawn
        if let section=UserDefaults.standard.string(forKey:"OADevGallerySection"),let owner {
            DispatchQueue.main.asyncAfter(deadline:.now()+0.5) { [weak self] in _ = og_reveal(owner,section);self?.wake() }
        }
    }
    override func viewDidLayoutSubviews() {super.viewDidLayoutSubviews();wake()}
    override func viewWillDisappear(_ animated:Bool) {super.viewWillDisappear(animated);display?.invalidate();display=nil}
    override var preferredStatusBarStyle:UIStatusBarStyle {.lightContent}
    deinit {if let owner {og_close(owner)}}
    private var dumped=""


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
        access?.refresh()
        #if DEBUG
        if UserDefaults.standard.bool(forKey:"OAAccessDump"),let now=access?.dump(),now != dumped {dumped=now;print("OA-ACCESS-BEGIN gallery\n\(now)\nOA-ACCESS-END")}
        #endif
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
