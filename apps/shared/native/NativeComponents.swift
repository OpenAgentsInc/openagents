// Firm reason: 6 (UIKit lifecycle/semantics and the shared Metal carrier).
import UIKit

@MainActor final class NativeComponents: UIViewController {
    private var owner: UnsafeMutableRawPointer?
    private var display: CADisplayLink?
    private var canvas: UIView?
    private let header=UIView()
    private var chrome:NativeArcadeSurface?
    private let closed: () -> Void
    private var finished=false
    init(closed: @escaping () -> Void) {self.closed=closed;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor=NativeSkin.ground;navigationController?.setNavigationBarHidden(true,animated:false)
        header.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(header)
        let close=NativeButton(.icon),options=NativeButton(.icon),title=UILabel()
        close.glass=true;options.glass=true
        close.show("Close components",symbol:"xmark");close.accessibilityIdentifier="components.close";close.addAction(UIAction { [weak self] _ in self?.finish() },for:.primaryActionTriggered)
        options.show("Gallery options",symbol:"slider.horizontal.3");options.accessibilityIdentifier="components.options";options.addAction(UIAction { [weak self,weak options] _ in guard let self,let options else{return};self.showOptions(from:options) },for:.primaryActionTriggered)
        title.attributedText=NativeSkin.label("Components",size:12,ink:NativeSkin.tokens.ink);title.accessibilityLabel="Components";title.accessibilityTraits.insert(.header)
        for child in [close,title,options] {child.translatesAutoresizingMaskIntoConstraints=false;header.addSubview(child)}
        NSLayoutConstraint.activate([header.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),header.leadingAnchor.constraint(equalTo:view.leadingAnchor),header.trailingAnchor.constraint(equalTo:view.trailingAnchor),header.heightAnchor.constraint(equalToConstant:44),close.leadingAnchor.constraint(equalTo:header.leadingAnchor,constant:12),close.centerYAnchor.constraint(equalTo:header.centerYAnchor),close.widthAnchor.constraint(equalToConstant:44),close.heightAnchor.constraint(equalToConstant:44),options.trailingAnchor.constraint(equalTo:header.trailingAnchor,constant:-12),options.centerYAnchor.constraint(equalTo:header.centerYAnchor),options.widthAnchor.constraint(equalToConstant:44),options.heightAnchor.constraint(equalToConstant:44),title.centerXAnchor.constraint(equalTo:header.centerXAnchor),title.centerYAnchor.constraint(equalTo:header.centerYAnchor)])
        chrome=NativeArcadeSurface(in:header)
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({cg_open($0)}),let raw=cg_view(owner) else {
            let label=UILabel();label.text="Components unavailable";label.textColor=NativeSkin.color(NativeSkin.tokens.ink);label.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(label);NSLayoutConstraint.activate([label.centerXAnchor.constraint(equalTo:view.centerXAnchor),label.centerYAnchor.constraint(equalTo:view.centerYAnchor)]);return
        }
        self.owner=owner;cg_monochrome(owner,1)
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();self.canvas=canvas
        canvas.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(canvas)
        NSLayoutConstraint.activate([canvas.leadingAnchor.constraint(equalTo:view.leadingAnchor),canvas.trailingAnchor.constraint(equalTo:view.trailingAnchor),canvas.topAnchor.constraint(equalTo:header.bottomAnchor),canvas.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor)])
        canvas.accessibilityLabel="Coax GPU component playground";canvas.accessibilityHint="Gallery options selects sections and animation controls. Swipe up or down to scroll."
        canvas.isAccessibilityElement=true
        canvas.accessibilityCustomActions=[UIAccessibilityCustomAction(name:"Scroll down",actionHandler:{ [weak self] _ in self?.scroll(400);return true }),UIAccessibilityCustomAction(name:"Scroll up",actionHandler:{ [weak self] _ in self?.scroll(-400);return true })]
        if UIAccessibility.isReduceMotionEnabled {cg_motion(owner,2)}
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(active),name:UIApplication.didBecomeActiveNotification,object:nil)
    }
    private func showOptions(from anchor:UIView) {
        var items:[NativeMenu.Item]=[]
        for (index,name) in ["All","Motion","Controls","HUD","ARWES","Reference"].enumerated() {items.append(.init(id:"components.section.\(index)",title:name) { [weak self] in guard let owner=self?.owner else{return};cg_section(owner,Int32(index)) })}
        for (index,name) in [(0,"Replay"),(1,"Pause / play"),(2,"Reduce motion"),(3,"Toggle specimen"),(4,"Zeron"),(5,"Everglade"),(6,"Verse")] {items.append(.init(id:"components.motion.\(index)",title:name) { [weak self] in guard let owner=self?.owner else{return};cg_motion(owner,Int32(index)) })}
        NativeMenu.show(from:anchor,items:items)
    }
    private func scroll(_ amount:Float) {if let owner {cg_scroll(owner,amount)}}
    override func viewDidAppear(_ animated:Bool) {super.viewDidAppear(animated);start()}
    override func viewWillDisappear(_ animated:Bool) {super.viewWillDisappear(animated);display?.invalidate();display=nil}
    private func start() {
        guard owner != nil,display==nil else{return}
        let link=CADisplayLink(target:self,selector:#selector(frame));link.preferredFrameRateRange=CAFrameRateRange(minimum:30,maximum:60,preferred:60);link.add(to:.main,forMode:.common);display=link
    }
    @objc private func frame() {if let owner {cg_frame(owner)}}
    @objc private func background() {display?.invalidate();display=nil}
    @objc private func active() {if viewIfLoaded?.window != nil {start()}}
    private func finish() {
        guard !finished else{return};finished=true;display?.invalidate();display=nil
        NotificationCenter.default.removeObserver(self)
        canvas?.removeFromSuperview();canvas=nil
        if let owner {cg_close(owner);self.owner=nil}
        dismiss(animated:true) { [closed] in closed() }
    }
    deinit {display?.invalidate();NotificationCenter.default.removeObserver(self);if let owner {cg_close(owner)}}
}
