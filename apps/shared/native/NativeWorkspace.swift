// Firm reason: 6 (UIKit child lifecycle, interactive gesture observations and accessibility).
// Coax owns the rail/history geometry, drag projection and settling rules.
import UIKit

@MainActor final class NativeWorkspace:UIViewController,UIGestureRecognizerDelegate {
    let detail:NativeControls,sidebar=NativeControls()
    private var available=false,open=false,split=false,history=false,dragging=false,transitioning=false
    private var progress:CGFloat=0,start:CGFloat=0
    private var embedded:UIViewController?,embeddedChrome:NativeControls?
    private let backdrop=UIControl(),sidebarClip=UIView(),edge=UIScreenEdgePanGestureRecognizer(),close=UIPanGestureRecognizer()
    var press:((String)->Void)?
    init(detail:NativeControls) {self.detail=detail;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder) {fatalError()}
    func content(_ controller:UIViewController?,chrome:NativeControls?=nil) {
        guard controller !== embedded else {return}
        embedded?.willMove(toParent:nil);embedded?.view.removeFromSuperview();embedded?.removeFromParent();embedded=controller;embeddedChrome=chrome
        if let controller {addChild(controller);view.insertSubview(controller.view,aboveSubview:detail.view);controller.didMove(toParent:self)}
        detail.view.isHidden=controller != nil;view.setNeedsLayout()
    }
    private var rtl:Bool {view.effectiveUserInterfaceLayoutDirection == .rightToLeft}
    private var geometry:coax_t_WorkspaceArcade {workspace_arcade(Double(view.bounds.width),traitCollection.horizontalSizeClass == .regular,history,Double(progress),rtl)}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor=NativeSkin.ground;view.clipsToBounds=true
        addChild(detail);view.addSubview(detail.view);detail.didMove(toParent:self)
        backdrop.backgroundColor=NativeSkin.drawerShade;backdrop.accessibilityIdentifier="workspace.backdrop";backdrop.accessibilityLabel="Close navigation";backdrop.addAction(UIAction { [weak self] _ in self?.press?("nav.close") },for:.touchUpInside);view.addSubview(backdrop)
        sidebarClip.clipsToBounds=true;view.addSubview(sidebarClip)
        addChild(sidebar);sidebar.view.autoresizingMask=[];sidebarClip.addSubview(sidebar.view);sidebar.didMove(toParent:self);sidebar.view.accessibilityIdentifier="workspace.history"
        edge.addTarget(self,action:#selector(pan(_:)));edge.edges = .left;view.addGestureRecognizer(edge)
        close.addTarget(self,action:#selector(pan(_:)));close.delegate=self;sidebar.view.addGestureRecognizer(close)
    }
    func render(_ elements:[NativeElement],history:[NativeElement]) {
        loadViewIfNeeded();let nav=elements.first {$0.role=="navigation"};available=nav != nil
        let next=nav?.attrs["data-state"]=="open",wide=nav?.attrs["type"]=="history",changed=next != open || wide != self.history
        if next && !open {detail.view.endEditing(true);embedded?.view.endEditing(true)}
        open=next;self.history=wide;sidebar.railMode = !wide
        detail.render(elements.filter {$0.role != "navigation"});sidebar.render(history)
        guard changed,!dragging else {view.setNeedsLayout();return}
        settle()
        UIAccessibility.post(notification:.screenChanged,argument:open ? sidebar.view:embedded?.view ?? detail.view)
    }
    private func settle() {
        transitioning=true;sidebar.view.isHidden=false;progress=open ? 1:0;view.setNeedsLayout()
        // The rail/history mode may change its width. Commit that size before
        // starting a translation, so text never lays out at intermediate widths.
        UIView.performWithoutAnimation {let g=self.geometry;self.sidebar.view.bounds=CGRect(x:0,y:0,width:g.list,height:self.view.bounds.height);self.sidebar.view.layoutIfNeeded()}
        UIView.animate(withDuration:NativeSkin.reducedMotion ? 0:geometry.duration,delay:0,options:[.beginFromCurrentState,.allowUserInteraction],animations:{self.view.layoutIfNeeded()},completion:{_ in self.transitioning=false;self.view.setNeedsLayout()})
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews();edge.edges=rtl ? .right:.left
        let g=geometry;split=available && g.split
        let show=available && (split || progress>0 || dragging || transitioning)
        sidebarClip.isHidden = !show;sidebar.view.isHidden = !show
        UIView.performWithoutAnimation {sidebar.view.frame=CGRect(x:0,y:0,width:g.list,height:view.bounds.height)}
        sidebarClip.frame=CGRect(x:g.sidebar_x,y:0,width:g.list,height:view.bounds.height)
        detail.view.frame=CGRect(x:available ? g.detail_x:0,y:0,width:available ? g.content:Double(view.bounds.width),height:Double(view.bounds.height))
        embedded?.view.frame=detail.view.frame
        backdrop.frame=view.bounds;backdrop.isHidden = !show || split;backdrop.alpha=CGFloat(g.shade)
        detail.view.accessibilityElementsHidden=show && !split;detail.view.isUserInteractionEnabled = !show || split;embedded?.view.accessibilityElementsHidden=show && !split;embedded?.view.isUserInteractionEnabled = !show || split;sidebar.view.accessibilityViewIsModal=show && !split
        for button in [detail.controls["nav.toggle"],embeddedChrome?.controls["nav.toggle"],sidebar.controls["nav.close"]].compactMap({$0 as? NativeButton}) {button.drawerProgress=Float(progress);button.drawerInteractive=dragging;button.drawerRTL=rtl}
        if split && !history {Task { [weak self] in guard let self,self.split,!self.history else{return};self.press?("nav.recent") }}
    }
    @objc private func pan(_ gesture:UIPanGestureRecognizer) {
        guard available,!split else{return}
        switch gesture.state {
        case .began:dragging=true;start=progress;detail.view.endEditing(true);embedded?.view.endEditing(true)
        case .changed:progress=CGFloat(workspace_drag(Double(start),Double(gesture.translation(in:view).x),geometry.list,rtl));view.setNeedsLayout();view.layoutIfNeeded()
        case .ended:
            dragging=false;let next=workspace_settle(Double(progress),Double(gesture.velocity(in:view).x),rtl)
            if next != open {press?(next ? "nav.toggle":"nav.close")}else {settle()}
        case .cancelled,.failed:dragging=false;settle()
        default:break
        }
    }
    func gestureRecognizerShouldBegin(_ gestureRecognizer:UIGestureRecognizer)->Bool {
        guard let pan=gestureRecognizer as? UIPanGestureRecognizer,!split,open else{return false};let v=pan.velocity(in:view)
        return abs(v.x)>abs(v.y) && (rtl ? v.x>0:v.x<0)
    }
    override func accessibilityPerformEscape()->Bool {guard open && !split else{return false};press?("nav.close");return true}
}
