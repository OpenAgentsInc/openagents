// Firm reason: 6 (UIKit controls, text input, keyboard layout and accessibility SDK).
import UIKit

/// The shared UIKit projection: keyed controls survive updates, including their
/// caret/selection. Coax owns what exists, its value and the meaning of input.
final class NativeControls: UIViewController,UIScrollViewDelegate,UIGestureRecognizerDelegate {
    let stack = UIStackView()
    private(set) var controls: [String:UIView] = [:]
    var press: ((String)->Void)?
    var edit: ((String,String)->Void)?
    private let dock=UIStackView()
    private var composer:NativeComposer?
    private let scroll=UIScrollView()
    private var conversation:String?
    private var following=true
    private let header=UIStackView(),headerSpacer=UIView()
    private lazy var headerHeight=header.heightAnchor.constraint(equalToConstant:0)
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        scroll.translatesAutoresizingMaskIntoConstraints = false;scroll.delegate=self
        scroll.keyboardDismissMode = .interactive
        // A fixed native form consumes pinches. Otherwise the two touches can
        // reach different buttons even though the scroll view has no zoom view.
        scroll.addGestureRecognizer(UIPinchGestureRecognizer(target:nil,action:nil))
        view.addSubview(scroll)
        header.axis = .horizontal;header.alignment = .center;header.distribution = .equalSpacing;header.spacing=8;header.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(header);headerHeight.isActive=true
        let outside=UITapGestureRecognizer(target:self,action:#selector(dismissKeyboard));outside.cancelsTouchesInView=false;outside.delegate=self;view.addGestureRecognizer(outside)
        dock.axis = .vertical;dock.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(dock)
        stack.axis = .vertical; stack.spacing = 16; stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),header.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:20),header.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-20),
            scroll.topAnchor.constraint(equalTo:header.bottomAnchor),
            scroll.bottomAnchor.constraint(equalTo:dock.topAnchor,constant:-6),
            dock.bottomAnchor.constraint(equalTo:view.keyboardLayoutGuide.topAnchor,constant:-6),
            dock.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:12),
            dock.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-12),
            scroll.leadingAnchor.constraint(equalTo:view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo:view.trailingAnchor),
            stack.topAnchor.constraint(equalTo:scroll.contentLayoutGuide.topAnchor,constant:24),
            stack.bottomAnchor.constraint(equalTo:scroll.contentLayoutGuide.bottomAnchor,constant:-24),
            stack.leadingAnchor.constraint(equalTo:scroll.frameLayoutGuide.leadingAnchor,constant:20),
            stack.trailingAnchor.constraint(equalTo:scroll.frameLayoutGuide.trailingAnchor,constant:-20),
        ])
    }
    func render(_ elements:[NativeElement]) {
        loadViewIfNeeded()
        var kept:[String:UIView] = [:]
        let thread=elements.first {$0.role=="transcript"}?.value
        if thread != conversation {conversation=thread;following=true}
        let ordinary=elements.filter {$0.role != "composer" && $0.role != "transcript" && $0.role != "toolbar"}
        var bar:[NativeElement]=[]
        func toolbar(_ e:NativeElement) {
            if e.role=="button" {bar.append(NativeElement(id:e.id,role:"button",label:e.textContent,disabled:e.attrs["disabled"] != nil))}
            else if !e.label.isEmpty {bar.append(NativeElement(id:e.id,role:"text",label:e.label))}
            else {for child in e.children {toolbar(child)}}
        }
        for e in elements where e.role=="toolbar" {toolbar(e)}
        headerHeight.constant=bar.isEmpty ? 0:44
        if bar.count==1 {if headerSpacer.superview==nil {header.insertArrangedSubview(headerSpacer,at:0)}}else {headerSpacer.removeFromSuperview()}
        for e in bar {
            let old=controls[e.id],matching=e.role=="button" ? old is UIButton:old is UILabel
            let control=matching ? old!:make(e)
            if let button=control as? UIButton {button.configuration = .plain();button.configuration?.title=e.label;button.isEnabled = !e.disabled}
            else if let label=control as? UILabel {label.text=e.label;label.font=NativeTypography.font(16,strong:true)}
            kept[e.id]=control;if control.superview !== header {header.addArrangedSubview(control)}
        }
        let ordered=(bar.count==1 ? [headerSpacer]:[])+bar.compactMap {kept[$0.id]}
        for (i,control) in ordered.enumerated() {
            if i<header.arrangedSubviews.count && header.arrangedSubviews[i] === control {continue}
            header.insertArrangedSubview(control,at:i)
        }
        if let e=elements.first(where:{$0.role=="composer"}) {
            if composer==nil {let c=NativeComposer();c.changed={ [weak self,weak c] text in self?.edit?(c?.fieldID ?? "",text) };c.press={ [weak self,weak c] id in if id==c?.sendID {self?.following=true};self?.press?(id) };composer=c;dock.addArrangedSubview(c)}
            composer?.render(e)
            if let c=composer {kept[e.id]=c;kept[c.fieldID]=c.editor}
        } else {composer?.removeFromSuperview();composer=nil}
        for e in ordinary {
            let old=controls[e.id]
            let matches = e.role == "message" ? old is NativeMessage : e.role == "button" ? old is UIButton : e.role == "field" ? old is UITextField : old is UILabel
            let control = matches ? old! : make(e)
            if let message=control as? NativeMessage {message.press={ [weak self] id in self?.press?(id) };message.render(e)}
            else if let b=control as? UIButton { b.configuration?.title=e.label; b.isEnabled = !e.disabled }
            else if let f=control as? UITextField {
                f.placeholder=e.label; f.accessibilityLabel=e.label; f.isEnabled = !e.disabled
                if f.text != e.value { f.text=e.value }
                f.keyboardType = e.type == "email" ? .emailAddress : e.type == "tel" ? .numberPad : .default
                f.textContentType = e.complete == "one-time-code" ? .oneTimeCode : e.complete == "email" ? .emailAddress : nil
            } else if let l=control as? UILabel {
                l.text=e.label
                l.font = .preferredFont(forTextStyle:e.role == "heading" ? (e.type=="h2" ? .title2:.largeTitle) : e.role == "notice" ? .footnote : .body)
                l.textColor = e.role == "notice" ? .secondaryLabel : .label
            }
            kept[e.id]=control
        }
        for (id,old) in controls where kept[id] !== old { old.removeFromSuperview() }
        controls=kept
        for (i,e) in ordinary.enumerated() {
            guard let control=kept[e.id] else {continue}
            if i<stack.arrangedSubviews.count && stack.arrangedSubviews[i] === control {continue}
            stack.insertArrangedSubview(control,at:i)
        }
    }
    private func make(_ e:NativeElement)->UIView {
        let control:UIView
        if e.role=="message" {control=NativeMessage()}
        else if e.role == "button" {
            let b=UIButton(configuration:.bordered())
            b.addAction(UIAction { [weak self] _ in self?.press?(e.id) },for:.primaryActionTriggered)
            b.heightAnchor.constraint(greaterThanOrEqualToConstant:44).isActive=true
            control=b
        } else if e.role == "field" {
            let f=UITextField(); f.borderStyle = .roundedRect
            f.autocapitalizationType = .none; f.autocorrectionType = .no
            f.font = .preferredFont(forTextStyle:.body); f.adjustsFontForContentSizeCategory=true
            f.heightAnchor.constraint(greaterThanOrEqualToConstant:44).isActive=true
            f.addAction(UIAction { [weak self,weak f] _ in self?.edit?(e.id,f?.text ?? "") },for:.editingChanged)
            control=f
        } else {
            let l=UILabel(); l.numberOfLines=0; l.adjustsFontForContentSizeCategory=true
            if e.role == "heading" {l.accessibilityTraits.insert(.header)}
            control=l
        }
        control.accessibilityIdentifier=e.id
        return control
    }
    @objc private func dismissKeyboard() {view.endEditing(true)}
    func gestureRecognizer(_ gestureRecognizer:UIGestureRecognizer,shouldReceive touch:UITouch)->Bool {
        if let composer,touch.view?.isDescendant(of:composer)==true {return false}
        var target=touch.view
        while let v=target {if v is UITextField || (v as? UITextView)?.isEditable==true {return false};target=v.superview}
        return true
    }
    func gestureRecognizer(_ gestureRecognizer:UIGestureRecognizer,shouldRecognizeSimultaneouslyWith other:UIGestureRecognizer)->Bool {true}
    func scrollViewDidScroll(_ scrollView:UIScrollView) {
        if scrollView.isDragging || scrollView.isDecelerating {following=scroll_at_end(Float(scrollView.contentSize.height),Float(scrollView.bounds.height),Float(scrollView.contentOffset.y),18)}
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if conversation != nil && following && !scroll.isDragging && !scroll.isDecelerating {
            let end=CGFloat(scroll_max(Float(scroll.contentSize.height),Float(scroll.bounds.height)))
            if abs(scroll.contentOffset.y-end)>0.5 {scroll.setContentOffset(CGPoint(x:0,y:end),animated:false)}
        }
    }
}
