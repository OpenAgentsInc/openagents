// Firm reason: 6 (UIKit/TextKit projection of the shared Coax tool-line component).
// T3 Code work-log-layout.tsx/thread-work-log.tsx; MIT notice in NativeComposer-T3-LICENSE.txt.
import UIKit

@MainActor final class NativeTool:UIView {
    var press:((String)->Void)?
    // A list row: nothing behind it at rest, the skin's raised row while pressed.
    let header=NativeButton(.list)
    private let icon=UIImageView(),label=UILabel(),chevron=UIImageView(),scroll=UIScrollView()
    private let body=UIView(),copy=NativeButton(.icon),backing=NativeFrameView(role:NativeSkin.names.panel)
    private var texts:[NativeRichText]=[]
    private var record:NativeElement?,detail:NativeElement?
    private lazy var heightConstraint=heightAnchor.constraint(equalToConstant:28)
    override init(frame:CGRect) {
        super.init(frame:frame);heightConstraint.isActive=true;header.ruled=false
        addSubview(header);for v in [icon,label,chevron] {header.addSubview(v);v.isUserInteractionEnabled=false}
        addSubview(backing);addSubview(scroll);scroll.addSubview(body);body.addSubview(copy)
        label.numberOfLines=1;label.lineBreakMode = .byTruncatingTail;label.adjustsFontForContentSizeCategory=true
        scroll.backgroundColor = .clear
        scroll.keyboardDismissMode = .interactive;scroll.showsVerticalScrollIndicator=true
        header.addAction(UIAction { [weak self] _ in guard let self,let id=self.header.accessibilityIdentifier else{return};self.press?(id) },for:.primaryActionTriggered)
        copy.addAction(UIAction { [weak self] _ in guard let self,let id=self.copy.accessibilityIdentifier else{return};self.press?(id) },for:.primaryActionTriggered)
    }
    required init?(coder:NSCoder) {fatalError()}
    func render(_ e:NativeElement) {
        guard record != e else{return};record=e
        let h=e.children.first {$0.role=="button"}
        let what=h?.children.first {$0.hasClass("tool-k")}?.textContent ?? "Tool"
        let sub=h?.children.first {$0.hasClass("tool-d")}?.textContent ?? ""
        label.text=what+(sub.isEmpty ? "":" · "+sub);label.textColor=NativeSkin.color(nt_skin().muted)
        header.accessibilityIdentifier=h?.id;header.accessibilityLabel=label.text
        let open=h?.attrs["aria-expanded"]=="true"
        header.accessibilityValue=open ? "Expanded":"Collapsed";header.accessibilityHint="Double tap to \(open ? "hide":"show") tool details"
        let names=["Bash":"terminal","Read":"doc.text","Edit":"pencil","MultiEdit":"pencil","Write":"square.and.pencil","Grep":"text.magnifyingglass","Glob":"folder","WebFetch":"globe","WebSearch":"magnifyingglass","Agent":"person.2","Task":"person.2","TodoWrite":"checklist"]
        icon.image=UIImage(systemName:names[what] ?? "wrench.and.screwdriver",withConfiguration:UIImage.SymbolConfiguration(pointSize:nt_work().icon,weight:.medium))
        icon.tintColor=label.textColor
        chevron.image=UIImage(systemName:open ? "chevron.up":"chevron.down",withConfiguration:UIImage.SymbolConfiguration(pointSize:nt_work().chevron,weight:.medium));chevron.tintColor=label.textColor
        let next=e.children.first {$0.hasClass("tool-b")};scroll.isHidden = !open;backing.isHidden = !open
        if detail != next {
            detail=next;for t in texts {t.removeFromSuperview()};texts=[]
            for pre in next?.children ?? [] where pre.role=="pre" && !pre.textContent.isEmpty {let t=NativeRichText(pre,code:true);body.addSubview(t);texts.append(t)}
        }
        let action=next?.children.first {$0.role=="button"}
        let said=action?.attrs["aria-label"] ?? "Copy tool output"
        copy.isHidden=action==nil;copy.accessibilityIdentifier=action?.id;copy.show(said,symbol:said=="Copied" ? "checkmark":"doc.on.doc")
        arrange();setNeedsLayout();superview?.setNeedsLayout()
    }
    override func layoutSubviews() {
        super.layoutSubviews();arrange()
    }
    private func arrange() {
        guard bounds.width>0 else{return};let s=nt_work(),font=NativeTypography.font(s.text)
        label.font=font;let row=max(s.row,ceil(font.lineHeight+8)),x=s.slot+s.gap
        header.frame=CGRect(x:0,y:0,width:bounds.width,height:row)
        icon.frame=CGRect(x:(s.slot-s.icon)/2,y:(row-s.icon)/2,width:s.icon,height:s.icon)
        label.frame=CGRect(x:x,y:0,width:max(1,bounds.width-x-24),height:row)
        chevron.frame=CGRect(x:bounds.width-18,y:(row-s.chevron)/2,width:s.chevron,height:s.chevron)
        var height=row
        if !scroll.isHidden {
            let width=max(1,bounds.width-24);var y:CGFloat=copy.isHidden ? 12:40
            copy.frame=CGRect(x:bounds.width-44,y:0,width:44,height:40)
            for t in texts {let h=t.height(width);t.frame=CGRect(x:12,y:y,width:width,height:h);y+=h+8}
            let h=min(s.detail_max,y+4);body.frame=CGRect(x:0,y:0,width:bounds.width,height:y+4);scroll.frame=CGRect(x:0,y:row+4,width:bounds.width,height:h);backing.frame=scroll.frame;scroll.contentSize=body.bounds.size;height+=h+8
        }
        if abs(heightConstraint.constant-height)>0.5 {heightConstraint.constant=height;superview?.setNeedsLayout()}
    }
}
