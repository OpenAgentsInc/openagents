// Firm reason: 6 (UIKit measured bounds, clipping, lifecycle and the shared Metal carrier).
import UIKit

struct NativeArcadePaint {
    var kind:Int32=0,role:Int64=0
    var background:UInt32=0,line:UInt32=0,ink:UInt32=0
    var value:Float=0,flags:Int32=0
}
protocol NativeArcadeNode:AnyObject {var arcadePaint:NativeArcadePaint {get}}
private final class WeakArcadeView {weak var view:UIView?;init(_ view:UIView) {self.view=view}}
@MainActor private final class ArcadeDisplayTarget:NSObject {
    weak var surface:NativeArcadeSurface?
    @objc func frame() {surface?.draw()}
}

/// One GPU scene under the native text/IME/accessibility adapters. Every paint
/// record uses the adapter's actual presented rectangle and ancestor clips.
/// The surface receives no touches and defines no application actions.
@MainActor final class NativeArcadeSurface:UIView {
    private weak var root:UIView?
    private var owner:UnsafeMutableRawPointer?
    private var metal:UIView?
    private var display:CADisplayLink?
    private let target=ArcadeDisplayTarget()
    private var candidates:[WeakArcadeView]=[]
    private var dirty=true
    var active=true
    private(set) var painted=0,refused=0
    static let changed=Notification.Name("CoaxArcadeTreeChanged")
    init(in root:UIView) {
        self.root=root;super.init(frame:root.bounds);autoresizingMask=[.flexibleWidth,.flexibleHeight]
        isUserInteractionEnabled=false;isOpaque=false;accessibilityIdentifier="coax.arcade.canvas";accessibilityElementsHidden=true
        if let shader=Bundle.main.path(forResource:"components",ofType:"metal",inDirectory:"Components"),let owner=shader.withCString({cnv_open($0)}) {
            self.owner=owner
            if let raw=cnv_view(owner) {
                let view=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();view.frame=bounds;view.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.isUserInteractionEnabled=false;addSubview(view);metal=view
            }
        }
        target.surface=self
        let center=NotificationCenter.default
        center.addObserver(self,selector:#selector(treeChanged),name:Self.changed,object:nil)
        center.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(foreground),name:UIApplication.didBecomeActiveNotification,object:nil)
        root.insertSubview(self,at:0)
    }
    required init?(coder:NSCoder) {fatalError()}
    static func invalidateTree() {NotificationCenter.default.post(name:changed,object:nil)}
    override func didMoveToWindow() {super.didMoveToWindow();if window==nil {stop()}else {start()}}
    private func start() {
        guard owner != nil,display==nil,window != nil else{return}
        let link=CADisplayLink(target:target,selector:#selector(ArcadeDisplayTarget.frame));link.preferredFrameRateRange=CAFrameRateRange(minimum:30,maximum:60,preferred:60);link.add(to:.main,forMode:.common);display=link
    }
    private func stop() {display?.invalidate();display=nil}
    func refresh() {dirty=true;setNeedsLayout()}
    override func layoutSubviews() {super.layoutSubviews();metal?.frame=bounds;dirty=true}
    @objc private func treeChanged() {dirty=true}
    @objc private func background() {stop()}
    @objc private func foreground() {start()}
    func draw() {
        guard active,let root,let owner,root.window != nil,!root.isHidden else{return}
        if dirty {
            candidates=[]
            func collect(_ view:UIView) {
                if view is NativeArcadeSurface {return}
                if view is NativeArcadeNode {candidates.append(WeakArcadeView(view))}
                for child in view.subviews {collect(child)}
            }
            collect(root);dirty=false
        }
        cnv_begin(owner,UIAccessibility.isReduceMotionEnabled ? 1:0)
        for item in candidates {
            guard let node=item.view,let paint=(node as? NativeArcadeNode)?.arcadePaint,node.window != nil,!node.isHidden,node.bounds.width>0,node.bounds.height>0 else {continue}
            var alpha=node.alpha,clip=root.bounds,parent=node.superview,visible=true,reached=false
            while let view=parent {
                if view.isHidden {visible=false;break};alpha*=view.alpha
                if view.clipsToBounds || view is UIScrollView {clip=clip.intersection(view.convert(view.bounds,to:root))}
                if view === root {reached=true;break};parent=view.superview
            }
            guard visible,reached,alpha>0.001,!clip.isNull else{continue}
            let rect=node.convert(node.bounds,to:root)
            guard rect.intersects(clip) else{continue}
            let key=Int64(UInt(bitPattern:Unmanaged.passUnretained(node).toOpaque()))
            _=cnv_add(owner,key,paint.kind,Int32(paint.role),Float(rect.minX),Float(rect.minY),Float(rect.width),Float(rect.height),Float(clip.minX),Float(clip.minY),Float(clip.maxX),Float(clip.maxY),paint.background,paint.line,paint.ink,Float(alpha),paint.value,paint.flags)
        }
        cnv_frame(owner);painted=Int(cnv_count(owner));refused=Int(cnv_refused(owner))
    }
    deinit {display?.invalidate();NotificationCenter.default.removeObserver(self);if let owner {cnv_close(owner)}}
}
