// Firm reason: 6 (UIKit child views, platform gestures, transitions and accessibility).
// Coax chooses the pane state, geometry and content; controls remain NativeControls.
import UIKit

@MainActor final class NativeWorkspace:UIViewController,UIGestureRecognizerDelegate {
    let detail:NativeControls,sidebar=NativeControls()
    private var available=false,open=false,laidOpen=false,split=false,transitioning=false
    var press:((String)->Void)?
    init(detail:NativeControls) {self.detail=detail;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor=NativeSkin.ground
        for child in [detail,sidebar] {addChild(child);view.addSubview(child.view);child.didMove(toParent:self)}
        sidebar.view.accessibilityIdentifier="workspace.history";sidebar.view.isHidden=true
        let edge=UIScreenEdgePanGestureRecognizer(target:self,action:#selector(reveal(_:)));edge.edges = .left;view.addGestureRecognizer(edge)
        let close=UIPanGestureRecognizer(target:self,action:#selector(conceal(_:)));close.delegate=self;sidebar.view.addGestureRecognizer(close)
    }
    func render(_ elements:[NativeElement],history:[NativeElement]) {
        loadViewIfNeeded();let nav=elements.first {$0.role=="navigation"};available=nav != nil
        let next=nav?.attrs["data-state"]=="open"
        if next && !open {detail.view.endEditing(true)}
        open=next;detail.render(elements.filter {$0.role != "navigation"});sidebar.render(history)
        view.setNeedsLayout()
        if laidOpen != open {
            let g=workspace_layout(Double(view.bounds.width),traitCollection.horizontalSizeClass == .regular,open)
            transitioning=true;sidebar.view.isHidden=false
            UIView.animate(withDuration:UIAccessibility.isReduceMotionEnabled ? 0:g.duration,animations:{self.view.layoutIfNeeded()},completion:{_ in self.transitioning=false;self.view.setNeedsLayout()})
            laidOpen=open;UIAccessibility.post(notification:.screenChanged,argument:open ? sidebar.view:detail.view)
        }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews();let g=workspace_layout(Double(view.bounds.width),traitCollection.horizontalSizeClass == .regular,open);split=available && g.split
        let show=available && (open || split);sidebar.view.isHidden = !show && !transitioning
        sidebar.view.frame=CGRect(x:g.offset,y:0,width:g.list,height:view.bounds.height)
        detail.view.frame=CGRect(x:split ? g.list:0,y:0,width:split ? g.content:Double(view.bounds.width),height:Double(view.bounds.height))
        detail.view.accessibilityElementsHidden=show && !split;detail.view.isUserInteractionEnabled = !show || split
        sidebar.view.accessibilityViewIsModal=show && !split
    }
    @objc private func reveal(_ gesture:UIScreenEdgePanGestureRecognizer) {
        let d=gesture.translation(in:view)
        if available && !open && !split && gesture.state == .ended && workspace_swipe(d.x,d.y,true) {press?("nav.toggle")}
    }
    @objc private func conceal(_ gesture:UIPanGestureRecognizer) {
        let d=gesture.translation(in:view)
        if open && !split && gesture.state == .ended && workspace_swipe(d.x,d.y,false) {press?("nav.close")}
    }
    func gestureRecognizerShouldBegin(_ gestureRecognizer:UIGestureRecognizer)->Bool {
        guard let pan=gestureRecognizer as? UIPanGestureRecognizer,!split else{return false};let v=pan.velocity(in:view)
        return v.x<0 && abs(v.x)>abs(v.y)
    }
    override func accessibilityPerformEscape()->Bool {guard open && !split else{return false};press?("nav.close");return true}
}
