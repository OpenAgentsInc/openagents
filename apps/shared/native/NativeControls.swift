// Firm reason: 6 (UIKit controls, text input, keyboard layout and accessibility SDK).
import UIKit

/// The shared UIKit projection: keyed controls survive updates, including their
/// caret/selection. Coax owns what exists, its value and the meaning of input.
final class NativeControls: UIViewController,UIScrollViewDelegate,UIGestureRecognizerDelegate,UITextViewDelegate {
    let stack = UIStackView()
    private(set) var controls: [String:UIView] = [:]
    var press: ((String)->Void)?
    var edit: ((String,String)->Void)?
    var submit: ((String)->Void)?
    private let dock=UIStackView()
    private let topDock=UIStackView(),options=UIStackView()
    private var composer:NativeComposer?
    private let scroll=UIScrollView()
    private var arcade:NativeArcadeSurface?
    private lazy var stackLeading=stack.leadingAnchor.constraint(equalTo:scroll.frameLayoutGuide.leadingAnchor,constant:20)
    private lazy var stackTrailing=stack.trailingAnchor.constraint(equalTo:scroll.frameLayoutGuide.trailingAnchor,constant:-20)
    private lazy var topLeading=topDock.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:20)
    private lazy var topTrailing=topDock.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-20)
    private lazy var headerLeading=header.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:14)
    private lazy var headerTrailing=header.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-14)
    var railMode=false {didSet {if oldValue != railMode {loadViewIfNeeded();stackLeading.constant=railMode ? 10:20;stackTrailing.constant=railMode ? -10:-20;topLeading.constant=railMode ? 10:20;topTrailing.constant=railMode ? -10:-20;headerLeading.constant=railMode ? 10:14;headerTrailing.constant=railMode ? -10:-14;view.setNeedsLayout()}}}
    var sidebarMode=false
    private lazy var emptyTop=topDock.heightAnchor.constraint(equalToConstant:0)
    private lazy var emptyOptions=options.heightAnchor.constraint(equalToConstant:0)
    private lazy var ordinaryTop=stack.topAnchor.constraint(equalTo:scroll.contentLayoutGuide.topAnchor,constant:24)
    private lazy var centeredTop=stack.topAnchor.constraint(greaterThanOrEqualTo:scroll.contentLayoutGuide.topAnchor,constant:24)
    private lazy var ordinaryBottom=stack.bottomAnchor.constraint(equalTo:scroll.contentLayoutGuide.bottomAnchor,constant:-24)
    private lazy var centeredBottom=stack.bottomAnchor.constraint(lessThanOrEqualTo:scroll.contentLayoutGuide.bottomAnchor,constant:-24)
    private lazy var centeredY=stack.centerYAnchor.constraint(equalTo:scroll.contentLayoutGuide.centerYAnchor)
    private lazy var centeredHeight:NSLayoutConstraint={let c=scroll.contentLayoutGuide.heightAnchor.constraint(equalTo:scroll.frameLayoutGuide.heightAnchor);c.priority = .defaultLow;return c}()
    private var conversation:String?
    private var readingScope:String?
    private var following=true
    private let header=UIView(),headerLeft=UIStackView(),headerRight=UIStackView(),headerTitle=UIView(),headerRule=NativeHeaderRule()
    private lazy var titleWidth=headerTitle.widthAnchor.constraint(lessThanOrEqualTo:header.widthAnchor,constant:-104)
    private lazy var headerHeight=header.heightAnchor.constraint(equalToConstant:0)
    private let hostBar=UIStackView(),hostTitle=UILabel(),hostClose=NativeButton(.icon)
    private lazy var hostHeight=hostBar.heightAnchor.constraint(equalToConstant:0)
    private var hostAction:(()->Void)?
    private var flows:[String:NativeFlow]=[:]
    /// A sheet over something else: the skin's panel, with its corner brackets, stands behind what it holds.
    var sheet=false
    var secretEdit:((String,String)->Void)?
    var secretBind:((UITextField,String)->Void)?
    /// A host's own header above what Coax renders: a title and one icon that closes what the host opened.
    func hostHeader(title:String,close:String,action:@escaping()->Void) {
        loadViewIfNeeded();hostAction=action;hostTitle.attributedText=NativeSkin.label(title,size:12,ink:NativeSkin.tokens.ink);hostTitle.accessibilityLabel=title
        hostClose.show(close,symbol:"xmark");hostClose.glass=true;hostHeight.constant=CGFloat(NativeSkin.tokens.header);hostBar.isHidden=false;showHeaderRule()
    }
    private func showHeaderRule() {headerRule.isHidden=sidebarMode || headerHeight.constant==0 && hostHeight.constant==0}
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor=NativeSkin.ground;view.tintColor=NativeSkin.color(NativeSkin.tokens.ink)
        arcade=NativeArcadeSurface(in:view)
        if sheet {
            let panel=NativeFrameView(role:NativeSkin.names.panel);panel.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(panel)
            NSLayoutConstraint.activate([panel.topAnchor.constraint(equalTo:view.topAnchor,constant:14),panel.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor,constant:-2),panel.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:10),panel.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-10)])
        }
        scroll.translatesAutoresizingMaskIntoConstraints = false;scroll.delegate=self
        scroll.keyboardDismissMode = .interactive
        // A fixed native form consumes pinches. Otherwise the two touches can
        // reach different buttons even though the scroll view has no zoom view.
        scroll.addGestureRecognizer(UIPinchGestureRecognizer(target:nil,action:nil))
        view.addSubview(scroll)
        header.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(header);headerHeight.isActive=true
        for group in [headerLeft,headerRight] {group.axis = .horizontal;group.alignment = .center;group.spacing=0;group.translatesAutoresizingMaskIntoConstraints=false;header.addSubview(group)}
        headerTitle.translatesAutoresizingMaskIntoConstraints=false;header.addSubview(headerTitle)
        NSLayoutConstraint.activate([headerLeft.leadingAnchor.constraint(equalTo:header.leadingAnchor),headerLeft.centerYAnchor.constraint(equalTo:header.centerYAnchor),headerRight.trailingAnchor.constraint(equalTo:header.trailingAnchor),headerRight.centerYAnchor.constraint(equalTo:header.centerYAnchor),sidebarMode ? headerTitle.leadingAnchor.constraint(equalTo:header.leadingAnchor):headerTitle.centerXAnchor.constraint(equalTo:header.centerXAnchor),headerTitle.centerYAnchor.constraint(equalTo:header.centerYAnchor),titleWidth])
        hostBar.axis = .horizontal;hostBar.alignment = .center;hostBar.spacing=8;hostBar.translatesAutoresizingMaskIntoConstraints=false;hostBar.isHidden=true;view.addSubview(hostBar);hostHeight.isActive=true
        hostTitle.accessibilityTraits.insert(.header);hostTitle.setContentHuggingPriority(.defaultLow,for:.horizontal)
        hostClose.addAction(UIAction { [weak self] _ in self?.hostAction?() },for:.primaryActionTriggered)
        hostBar.addArrangedSubview(hostClose);hostBar.addArrangedSubview(hostTitle)
        headerRule.translatesAutoresizingMaskIntoConstraints=false;headerRule.isHidden=true;view.addSubview(headerRule)
        let outside=UITapGestureRecognizer(target:self,action:#selector(dismissKeyboard));outside.cancelsTouchesInView=false;outside.delegate=self;view.addGestureRecognizer(outside)
        dock.axis = .vertical;dock.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(dock)
        topDock.axis = .vertical;topDock.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(topDock)
        options.axis = .vertical;dock.addArrangedSubview(options)
        stack.axis = .vertical; stack.spacing = 16; stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            hostBar.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor,constant:sheet ? 10:0),hostBar.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:8),hostBar.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-20),
            header.topAnchor.constraint(equalTo:hostBar.bottomAnchor),headerLeading,headerTrailing,
            headerRule.topAnchor.constraint(equalTo:header.bottomAnchor),headerRule.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:14),headerRule.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-14),
            topDock.topAnchor.constraint(equalTo:header.bottomAnchor),topLeading,topTrailing,
            scroll.topAnchor.constraint(equalTo:topDock.bottomAnchor),
            scroll.bottomAnchor.constraint(equalTo:dock.topAnchor,constant:-6),
            dock.bottomAnchor.constraint(equalTo:view.keyboardLayoutGuide.topAnchor,constant:-6),
            dock.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:12),
            dock.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-12),
            scroll.leadingAnchor.constraint(equalTo:view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo:view.trailingAnchor),
            ordinaryTop,
            ordinaryBottom,
            stackLeading,stackTrailing,
        ])
    }
    override func viewDidAppear(_ animated:Bool) {super.viewDidAppear(animated);arcade?.active=true;arcade?.refresh()}
    override func viewDidDisappear(_ animated:Bool) {super.viewDidDisappear(animated);arcade?.active=false}
    func render(_ elements:[NativeElement]) {
        loadViewIfNeeded()
        let reading=elements.first {$0.role=="reading-list"}?.value
        let anchor:(String,CGFloat)? = reading != nil && reading==readingScope ? controls.compactMap {id,control -> (String,CGFloat)? in
            guard control.isDescendant(of:stack) else{return nil}
            let rect=control.convert(control.bounds,to:scroll)
            return rect.maxY>scroll.contentOffset.y+8 ? (id,rect.minY-scroll.contentOffset.y):nil
        }.min(by:{$0.1<$1.1}):nil
        let oldReading=readingScope;readingScope=reading
        let centered=elements.contains {$0.role=="welcome"};ordinaryTop.isActive = !centered;ordinaryBottom.isActive = !centered;centeredTop.isActive=centered;centeredBottom.isActive=centered;centeredY.isActive=centered;centeredHeight.isActive=centered
        var kept:[String:UIView] = [:]
        let thread=elements.first {$0.role=="transcript"}?.value
        if thread != conversation {conversation=thread;following=true}
        let ordinary=elements.filter {$0.role != "welcome" && $0.role != "composer" && $0.role != "transcript" && $0.role != "toolbar" && $0.role != "reading-list"}
        var bar:[NativeElement]=[]
        func toolbar(_ e:NativeElement) {
            if e.role=="button" {var button=e;button.label=e.textContent;button.disabled=e.attrs["disabled"] != nil;bar.append(button)}
            else if !e.label.isEmpty {bar.append(NativeElement(id:e.id,role:"text",label:e.label))}
            else {for child in e.children {toolbar(child)}}
        }
        for e in elements where e.role=="toolbar" {toolbar(e)}
        headerHeight.constant=bar.isEmpty ? 0:CGFloat(NativeSkin.tokens.header);showHeaderRule()
        for e in bar {
            let old=controls[e.id],matching=e.role=="button" ? old is NativeButton:old is UILabel
            let control=matching ? old!:make(e)
            if let button=control as? NativeButton {
                button.reference=sidebarMode
                button.isEnabled = !e.disabled
                if let state=e.attrs["data-state"],["loading","ready","offline","idle"].contains(state) {
                    // A status: how it stands is shown by the icon and its brightness, and said by its label.
                    button.variant = .icon;button.busy=state=="loading";button.current=state=="ready"
                    button.show(e.attrs["aria-label"] ?? "Refresh conversation",symbol:state=="ready" ? "checkmark.circle":state=="offline" ? "exclamationmark.arrow.triangle.2.circlepath":"arrow.clockwise")
                } else {button.variant=e.hasClass("ib") ? .icon:.quiet;button.busy=false;button.current=false;button.show(e.label.isEmpty ? e.attrs["aria-label"] ?? "":e.label,symbol:NativeSkin.symbol(e))}
                // The shared activity ring is painted beneath UIKit effects;
                // keep it unobscured until it has a native glass foreground.
                button.glass=button.variant == .icon && !button.busy
            }
            else if let label=control as? UILabel {if sidebarMode {label.attributedText=nil;label.text=e.label;label.font=NativeTypography.font(20,strong:true);label.textColor=NativeSkin.color(NativeSkin.tokens.ink)}else {label.attributedText=NativeSkin.label(e.label,size:12,ink:NativeSkin.tokens.ink)};label.accessibilityLabel=e.label;label.numberOfLines=1;label.lineBreakMode = .byTruncatingTail;label.setContentCompressionResistancePriority(.defaultLow,for:.horizontal)}
            kept[e.id]=control
        }
        let titleIndex=bar.firstIndex {$0.role != "button"}
        var leading:[UIView]=[],trailing:[UIView]=[]
        for (index,e) in bar.enumerated() {
            guard let control=kept[e.id] else {continue}
            if index==titleIndex {
                if control.superview !== headerTitle {headerTitle.subviews.forEach {$0.removeFromSuperview()};control.translatesAutoresizingMaskIntoConstraints=false;headerTitle.addSubview(control);NSLayoutConstraint.activate([control.leadingAnchor.constraint(equalTo:headerTitle.leadingAnchor),control.trailingAnchor.constraint(equalTo:headerTitle.trailingAnchor),control.topAnchor.constraint(equalTo:headerTitle.topAnchor),control.bottomAnchor.constraint(equalTo:headerTitle.bottomAnchor)])}
                (control as? UILabel)?.textAlignment = sidebarMode ? .natural:.center
            } else if (titleIndex.map {index<$0} ?? railMode) {leading.append(control)}else {trailing.append(control)}
        }
        for (group,items) in [(headerLeft,leading),(headerRight,trailing)] {
            for old in group.arrangedSubviews where !items.contains(where:{$0 === old}) {group.removeArrangedSubview(old);old.removeFromSuperview()}
            for (i,item) in items.enumerated() {if i>=group.arrangedSubviews.count || group.arrangedSubviews[i] !== item {group.insertArrangedSubview(item,at:i)}}
        }
        if titleIndex==nil {headerTitle.subviews.forEach {$0.removeFromSuperview()}}
        titleWidth.constant = -2*max(leading.reduce(0){$0+$1.intrinsicContentSize.width},trailing.reduce(0){$0+$1.intrinsicContentSize.width})-16
        if let e=elements.first(where:{$0.role=="composer"}) {
            if composer==nil {let c=NativeComposer();c.changed={ [weak self,weak c] text in self?.edit?(c?.fieldID ?? "",text) };c.press={ [weak self,weak c] id in if id==c?.sendID {self?.following=true};self?.press?(id) };composer=c;dock.addArrangedSubview(c)}
            composer?.render(e)
            if let c=composer {kept[e.id]=c;kept[c.fieldID]=c.editor}
        } else {composer?.removeFromSuperview();composer=nil}
        for e in ordinary {
            let old=controls[e.id]
            let matches = e.role == "image" ? old is NativeRemoteImage : e.role == "tool" ? old is NativeTool : e.role == "message" ? old is NativeMessage : e.role == "button" ? old is NativeButton : e.role == "field" ? (e.type=="multiline" ? old is UITextView:old is UITextField) : old is UILabel
            let control = matches ? old! : make(e)
            if let image=control as? NativeRemoteImage {image.render(e)}
            else if let tool=control as? NativeTool {tool.press={ [weak self] id in self?.following=false;self?.press?(id) };tool.render(e)}
            else if let message=control as? NativeMessage {message.press={ [weak self] id in self?.press?(id) };message.render(e)}
            else if let b=control as? NativeButton {
                b.reference=sidebarMode;b.ruled = !sidebarMode
                // The kit says which button this is (its class and state); the skin says how each looks.
                let state=e.attrs["data-state"],select=e.hasClass("sel")
                b.variant=e.hasClass("sw") ? .toggle:state=="rail" ? .rail:e.hasClass("ib") ? .icon:e.hasClass("lk") || select || e.label.isEmpty ? .quiet:e.hasClass("row") ? .list:state=="primary" ? .primary:state=="destructive" ? .destructive:.plain
                b.busy=state=="busy";b.current=e.attrs["aria-current"]=="true" || e.attrs["aria-pressed"]=="true";b.isEnabled = !e.disabled
                b.show(e.hasClass("sw") ? e.attrs["aria-label"] ?? e.label:e.label.isEmpty ? e.attrs["aria-label"] ?? "":e.label,symbol:select ? (e.attrs["aria-expanded"]=="true" ? "chevron.up":"chevron.down"):NativeSkin.symbol(e))
            }
            else if let text=control as? UITextView {
                text.accessibilityLabel=e.label;text.isEditable = !e.disabled
                if text.text != e.value {let selection=text.selectedRange;text.text=e.value;text.selectedRange=NSRange(location:min(selection.location,(e.value as NSString).length),length:0)}
                text.font=e.attrs["data-lang"] != nil ? NativeTypography.font(15,code:true):NativeTypography.font(16)
            } else if let f=control as? UITextField {
                (f as? NativeField)?.reference=sidebarMode
                f.placeholder=e.label; f.accessibilityLabel=e.label; f.isEnabled = !e.disabled
                let sensitive=e.type=="secret" || e.type=="secret-output"
                (f as? NativeField)?.sensitive=sensitive
                f.isUserInteractionEnabled=e.type != "secret-output";f.isSecureTextEntry=sensitive
                if sensitive {secretBind?(f,e.id);f.accessibilityValue="Private key"}else if f.text != e.value { f.text=e.value }
                f.keyboardType = e.type == "email" ? .emailAddress : e.type == "tel" ? .numberPad : e.type=="number" ? .decimalPad:.default
                f.textContentType = e.complete == "one-time-code" ? .oneTimeCode : e.complete == "email" ? .emailAddress : nil
            } else if let l=control as? UILabel {
                // A page's title reads as text. A section's name, a small fact and a figure are chrome: the skin's label type.
                let t=NativeSkin.tokens
                if e.role=="status",let glyph=NativeSkin.status(e) {l.attributedText=glyph}
                else if sidebarMode && (e.role=="heading" || e.hasClass("meta")) {l.attributedText=nil;l.text=e.label;l.font=NativeTypography.font(14);l.textColor=NativeSkin.color(t.ink_muted)}
                else if e.role=="heading" && e.type != "h1" {l.attributedText=NativeSkin.label(e.label,size:12,ink:t.ink_muted)}
                else if e.hasClass("meta") {l.attributedText=NativeSkin.label(e.label,ink:t.ink_muted)}
                else if e.hasClass("amount") {l.attributedText=NativeSkin.label(e.label,size:24,track:0.02,ink:t.ink_bright,upper:false)}
                else {
                    l.attributedText=nil;l.text=e.label
                    l.font=e.role=="heading" ? NativeTypography.font(26,strong:true):e.role=="notice" || e.hasClass("note") ? NativeTypography.font(14):NativeTypography.font(16)
                    l.textColor=NativeSkin.color(e.role=="heading" ? t.ink_bright:e.role=="notice" || e.hasClass("note") ? t.ink_muted:t.ink)
                }
                l.textAlignment=centered ? .center:.natural;l.accessibilityLabel=e.label
            }
            kept[e.id]=control
        }
        // The composer reuses its native editor across domain IDs. Remove a
        // view only when no current key retains it, including the editor key.
        let retained=Set(kept.values.map(ObjectIdentifier.init))
        for old in controls.values where !retained.contains(ObjectIdentifier(old)) {old.removeFromSuperview()}
        controls=kept
        // Quiet buttons that follow one another share a line, each as wide as its words; everything else has its own.
        var rows:[(view:UIView,first:NativeElement,last:NativeElement)]=[];var used:[String:NativeFlow]=[:];var at=0
        // On a sheet every button but a list's row does: a sheet's tools are many and its room is small.
        func quiet(_ e:NativeElement)->Bool {guard e.role=="button",let variant=(kept[e.id] as? NativeButton)?.variant else {return false};return variant == .quiet || variant == .icon || sheet && variant != .list || sidebarMode && e.attrs["_nativePlacement"]=="dock" && variant == .primary}
        while at<ordinary.count {
            let e=ordinary[at];guard let control=kept[e.id] else {at+=1;continue}
            guard quiet(e) else {rows.append((control,e,e));at+=1;continue}
            var end=at;var items:[UIView]=[]
            while end<ordinary.count,quiet(ordinary[end]),ordinary[end].attrs["_nativePlacement"]==e.attrs["_nativePlacement"],let c=kept[ordinary[end].id] {items.append(c);end+=1}
            let flow=flows[e.id] ?? NativeFlow();if flow.items.count != items.count || !zip(flow.items,items).allSatisfy({$0 === $1}) {flow.items=items}
            flow.centered=centered;flow.edgeAligned=sidebarMode && e.attrs["_nativePlacement"]=="dock";used[e.id]=flow;rows.append((flow,e,ordinary[end-1]));at=end
        }
        for (id,flow) in flows where used[id] !== flow {flow.items=[];flow.removeFromSuperview()}
        flows=used
        var normal=0,above=0,below=0
        for row in rows {
            let place=row.first.attrs["_nativePlacement"],target=place=="dock" ? options:place=="top" ? topDock:stack
            let i=place=="dock" ? below:place=="top" ? above:normal
            if place=="dock" {below+=1}else if place=="top" {above+=1}else {normal+=1}
            if i<target.arrangedSubviews.count && target.arrangedSubviews[i] === row.view {continue}
            if row.view.superview !== target {row.view.removeFromSuperview()};target.insertArrangedSubview(row.view,at:i)
        }
        emptyTop.isActive=above==0;emptyOptions.isActive=below==0
        // Rows of one list stand together under a hairline each; tool rows stay as close as the work log sets them.
        func row(_ e:NativeElement)->Bool {e.role=="button" && e.hasClass("row")}
        for (i,r) in rows.enumerated() {let next=i+1<rows.count ? rows[i+1].first:nil;(r.first.attrs["_nativePlacement"]=="dock" ? options:r.first.attrs["_nativePlacement"]=="top" ? topDock:stack).setCustomSpacing(r.last.role=="tool" && next?.role=="tool" ? 1:row(r.last) && next.map(row)==true ? 0:CGFloat(NativeSkin.tokens.gap),after:r.view)}
        arcade?.refresh();view.setNeedsLayout()
        if reading != nil {
            view.layoutIfNeeded()
            if let (id,y)=anchor,let control=controls[id] {
                let target=control.convert(control.bounds,to:scroll).minY-y
                let maximum=max(0,scroll.contentSize.height-scroll.bounds.height)
                scroll.setContentOffset(CGPoint(x:0,y:min(maximum,max(0,target))),animated:false)
            }else if reading != oldReading {scroll.setContentOffset(.zero,animated:false)}
        }
    }
    private func make(_ e:NativeElement)->UIView {
        let control:UIView
        if e.role=="image" {control=NativeRemoteImage()}
        else if e.role=="tool" {control=NativeTool()}
        else if e.role=="message" {control=NativeMessage()}
        else if e.role == "button" {
            let b=NativeButton()
            b.addAction(UIAction { [weak self] _ in self?.press?(e.id) },for:.primaryActionTriggered)
            control=b
        } else if e.role == "field" && e.type=="multiline" {
            let text=NativeTextArea();text.delegate=self;text.font=NativeTypography.font(16);text.adjustsFontForContentSizeCategory=true
            text.fixedHeight=e.attrs["data-lang"] != nil ? 240:nil;control=text
        } else if e.role == "field" {
            let f=NativeField()
            f.autocapitalizationType = .none; f.autocorrectionType = .no
            f.addAction(UIAction { [weak self,weak f] _ in if e.type=="secret" {self?.secretEdit?(e.id,f?.text ?? "")}else {self?.edit?(e.id,f?.text ?? "")} },for:.editingChanged)
            f.addAction(UIAction { [weak self] _ in self?.submit?(e.id) },for:e.type=="number" ? .editingDidEnd:.editingDidEndOnExit)
            control=f
        } else {
            let l:UILabel=e.role=="notice" ? NativeNotice():UILabel(); l.numberOfLines=0; l.adjustsFontForContentSizeCategory=true
            if e.role == "heading" {l.accessibilityTraits.insert(.header)}
            control=l
        }
        control.accessibilityIdentifier=e.id
        return control
    }
    @objc private func dismissKeyboard() {view.endEditing(true)}
    func textViewDidChange(_ textView:UITextView) {if let id=textView.accessibilityIdentifier {edit?(id,textView.text)}}
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
        super.viewDidLayoutSubviews();arcade?.frame=view.bounds
        if conversation != nil && following && !scroll.isDragging && !scroll.isDecelerating {
            let end=CGFloat(scroll_max(Float(scroll.contentSize.height),Float(scroll.bounds.height)))
            if abs(scroll.contentOffset.y-end)>0.5 {scroll.setContentOffset(CGPoint(x:0,y:end),animated:false)}
        }
    }
}
