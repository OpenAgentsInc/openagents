// Firm reason: 6 (UIKit lifecycle/semantics and the shared Metal carrier).
import UIKit

@MainActor final class NativeComponents: UIViewController {
    private var owner: UnsafeMutableRawPointer?
    private var display: CADisplayLink?
    private var canvas: UIView?
    private let closed: () -> Void
    private var finished=false
    init(closed: @escaping () -> Void) {self.closed=closed;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black;title="Components"
        navigationItem.leftBarButtonItem=UIBarButtonItem(systemItem:.close,primaryAction:UIAction { [weak self] _ in self?.finish() })
        navigationItem.rightBarButtonItem=UIBarButtonItem(image:UIImage(systemName:"slider.horizontal.3"),menu:menu())
        navigationItem.rightBarButtonItem?.accessibilityLabel="Gallery options"
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({cg_open($0)}),let raw=cg_view(owner) else {
            let label=UILabel();label.text="Components unavailable";label.textColor = .white;label.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(label);NSLayoutConstraint.activate([label.centerXAnchor.constraint(equalTo:view.centerXAnchor),label.centerYAnchor.constraint(equalTo:view.centerYAnchor)]);return
        }
        self.owner=owner
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();self.canvas=canvas
        canvas.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(canvas)
        NSLayoutConstraint.activate([canvas.leadingAnchor.constraint(equalTo:view.leadingAnchor),canvas.trailingAnchor.constraint(equalTo:view.trailingAnchor),canvas.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),canvas.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor)])
        canvas.accessibilityLabel="Coax GPU component playground";canvas.accessibilityHint="Gallery options selects sections and animation controls. Swipe up or down to scroll."
        canvas.isAccessibilityElement=true
        canvas.accessibilityCustomActions=[UIAccessibilityCustomAction(name:"Scroll down",actionHandler:{ [weak self] _ in self?.scroll(400);return true }),UIAccessibilityCustomAction(name:"Scroll up",actionHandler:{ [weak self] _ in self?.scroll(-400);return true })]
        if UIAccessibility.isReduceMotionEnabled {cg_motion(owner,2)}
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(active),name:UIApplication.didBecomeActiveNotification,object:nil)
    }
    private func menu()->UIMenu {
        let sections=["All","Motion","Controls","HUD","ARWES"].enumerated().map {index,name in UIAction(title:name) { [weak self] _ in guard let owner=self?.owner else{return};cg_section(owner,Int32(index)) }}
        let controls=[(0,"Replay","arrow.counterclockwise"),(1,"Pause / play","pause"),(2,"Reduce motion","figure.stand"),(3,"Toggle specimen","switch.2"),(4,"Zeron","circle"),(5,"Everglade","leaf"),(6,"Verse","sparkles")].map {index,name,icon in UIAction(title:name,image:UIImage(systemName:icon)) { [weak self] _ in guard let owner=self?.owner else{return};cg_motion(owner,Int32(index)) }}
        return UIMenu(children:[UIMenu(title:"Sections",options:.displayInline,children:sections),UIMenu(title:"Motion",children:controls)])
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
