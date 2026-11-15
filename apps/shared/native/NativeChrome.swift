// Firm reason: 6 (UIKit/AppKit layers and controls that project the mono skin's exports).
// The platform's text/input/accessibility projection of the shared kit. On
// iOS, frames and motion are painted by components/native/scene.coax through
// NativeArcadeSurface. AppKit retains its frame-point adapter. Skin exports
// own the tokens, variants and states. This file holds no private palette.
// coax/native/stock-style.coax fails any other host file that styles a
// control on its own (#13580).
import Foundation
import QuartzCore
import CoreText
#if canImport(UIKit)
import UIKit
typealias NativePlatformColor=UIColor
typealias NativePlatformFont=UIFont
#else
import AppKit
typealias NativePlatformColor=NSColor
typealias NativePlatformFont=NSFont
#endif

enum NativeSkin {
    // SDK glyph binding for the shared kit's icon names. Paint and geometry
    // still come from the mono skin, as with the existing native composer.
    static func symbol(_ element:NativeElement)->String? {
        func href(_ e:NativeElement)->String? {if let h=e.attrs["href"],h.contains("/icons/") {return h};return e.children.compactMap(href).first}
        guard let h=href(element),let raw=h.split(separator:"/").last else{return nil}
        let name=String(raw).components(separatedBy:".svg")[0]
        return ["sidebar":"line.3.horizontal","menu":"line.3.horizontal","plus":"plus","x":"xmark","settings":"gearshape","arrow-rotate-cw":"arrow.clockwise","chat":"bubble.left.and.bubble.right","folder":"folder","globe":"globe","history":"clock.arrow.circlepath","arrow-left":"arrow.left","avatar-profile":"person.crop.circle","bell":"bell","key":"key","shield-lock":"lock.shield","filter":"line.3.horizontal.decrease","news-paper":"newspaper"][name]
    }
    static var drawerShade:NativePlatformColor {color(mono_drawer_shade())}
    static let tokens=mono_native()
    static let names=mono_names()
    static func cg(_ c:UInt32)->CGColor {
        CGColor(srgbRed:CGFloat((c>>24)&255)/255,green:CGFloat((c>>16)&255)/255,blue:CGFloat((c>>8)&255)/255,alpha:CGFloat(c&255)/255)
    }
    static func color(_ c:UInt32)->NativePlatformColor {NativePlatformColor(cgColor:cg(c)) ?? NativePlatformColor.clear}
    static var reducedMotion:Bool {
        #if canImport(UIKit)
        return UIAccessibility.isReduceMotionEnabled
        #else
        return NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        #endif
    }
    /// The code face the web skin sets its labels in, from the bundled resources; the system's fixed face without it.
    private static let labelFace:String? = {
        for directory in ["Starter/build/zeron","build/zeron"] {
            guard let url=Bundle.main.url(forResource:"GeistMono",withExtension:"ttf",subdirectory:directory) else {continue}
            CTFontManagerRegisterFontsForURL(url as CFURL,.process,nil)
            if let d=(CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor])?.first,let name=CTFontDescriptorCopyAttribute(d,kCTFontNameAttribute) as? String {return name}
        }
        return nil
    }()
    /// A chrome label's font. On a phone it follows Dynamic Type, to a limit that keeps a header on one line.
    static func labelFont(_ size:CGFloat)->NativePlatformFont {
        let base=labelFace.flatMap {NativePlatformFont(name:$0,size:size)} ?? NativePlatformFont.monospacedSystemFont(ofSize:size,weight:.medium)
        #if canImport(UIKit)
        return UIFontMetrics(forTextStyle:.caption1).scaledFont(for:base,maximumPointSize:size*1.7)
        #else
        return base
        #endif
    }
    /// Chrome text: small, upper case and tracked. What a screen reader says stays the caller's own words.
    static func label(_ text:String,size:Double?=nil,track:Double?=nil,ink:UInt32?=nil,upper:Bool=true)->NSAttributedString {
        let font=labelFont(CGFloat(size ?? tokens.label))
        return NSAttributedString(string:upper ? text.uppercased():text,attributes:[.font:font,.kern:font.pointSize*CGFloat(track ?? tokens.track),.foregroundColor:color(ink ?? tokens.ink_muted)])
    }
    /// The root ground: plain black on iOS, the skin's faint grid on desktop.
    static let ground:NativePlatformColor = {
        #if canImport(UIKit)
        return color(tokens.ground)
        #else
        let step=CGFloat(tokens.grid_step),size=CGSize(width:step,height:step)
        func parts(_ c:CGContext) {
            c.setFillColor(cg(tokens.ground));c.fill(CGRect(origin:.zero,size:size))
            for i in 0..<tokens.grid_parts {let p=mono_native_grid(i);c.setFillColor(cg(p.color));c.fill(CGRect(x:p.x,y:p.y,width:p.w,height:p.h))}
        }
        #if canImport(UIKit)
        return UIColor(patternImage:UIGraphicsImageRenderer(size:size).image {parts($0.cgContext)})
        #else
        return NSColor(patternImage:NSImage(size:size,flipped:true) {_ in if let c=NSGraphicsContext.current?.cgContext {parts(c)};return true})
        #endif
        #endif
    }()
}

#if !canImport(UIKit)
/// One of the skin's frames at this layer's size: a filled ground, a line and a decoration.
final class NativeFrame:CALayer {
    var role:Int64=0 {didSet {if role != oldValue {drawn = .zero;setNeedsLayout()}}}
    private let ground=CAShapeLayer(),rule=CAShapeLayer(),mark=CAShapeLayer(),solid=CAShapeLayer()
    private var drawn=CGSize.zero,marching=false
    override init() {
        super.init()
        #if !canImport(UIKit)
        isGeometryFlipped=true
        #endif
        for l in [ground,rule,mark,solid] {l.actions=["path":NSNull(),"fillColor":NSNull(),"strokeColor":NSNull(),"lineDashPattern":NSNull(),"bounds":NSNull(),"position":NSNull()];addSublayer(l)}
        ground.strokeColor=nil;solid.strokeColor=nil;rule.fillColor=nil;mark.fillColor=nil
    }
    override init(layer:Any) {super.init(layer:layer)}
    required init?(coder:NSCoder) {fatalError()}
    func paint(bg:UInt32,line:UInt32,deco:UInt32,dashed:Bool=false,march:Bool=false) {
        let t=NativeSkin.tokens
        ground.fillColor=NativeSkin.cg(bg);rule.strokeColor=NativeSkin.cg(line);mark.strokeColor=NativeSkin.cg(deco);solid.fillColor=NativeSkin.cg(deco)
        rule.lineDashPattern=dashed ? [NSNumber(value:t.dash),NSNumber(value:t.dash)]:nil
        let moves=dashed && march && !NativeSkin.reducedMotion
        if moves != marching {
            marching=moves;rule.removeAnimation(forKey:"march")
            if moves {let a=CABasicAnimation(keyPath:"lineDashPhase");a.fromValue=0;a.toValue = -(t.dash*2);a.duration=t.march;a.repeatCount = .infinity;a.isRemovedOnCompletion=false;rule.add(a,forKey:"march")}
        }
    }
    override func layoutSublayers() {
        super.layoutSublayers()
        guard bounds.size != drawn else {return}
        drawn=bounds.size
        var points=[coax_t_AfPoint](repeating:coax_t_AfPoint(),count:256)
        let n=points.withUnsafeMutableBufferPointer {Int(mono_native_frame(role,Double(bounds.width),Double(bounds.height),coax_v_t_AfPoint(data:$0.baseAddress,count:256)))}
        let layers=[ground,rule,mark,solid],paths=layers.map {_ in CGMutablePath()}
        for p in points.prefix(n) {
            let k=p.layer==0 ? 0:p.layer==1 ? 1:p.fill ? 3:2,at=CGPoint(x:CGFloat(p.x),y:CGFloat(p.y))
            if p.move {paths[k].move(to:at)}else {paths[k].addLine(to:at)}
            if p.stroke>0 {layers[k].lineWidth=CGFloat(p.stroke)}
        }
        for (layer,path) in zip(layers,paths) {layer.frame=bounds;layer.path=path}
    }
}

/// A ring of dashes that turns while something runs, and stands still under reduced motion.
final class NativeRing:CALayer {
    private let ring=CAShapeLayer()
    override init() {super.init();ring.fillColor=nil;addSublayer(ring)}
    override init(layer:Any) {super.init(layer:layer)}
    required init?(coder:NSCoder) {fatalError()}
    func paint(_ ink:UInt32,running:Bool) {
        let t=NativeSkin.tokens
        ring.strokeColor=NativeSkin.cg(ink);ring.lineWidth=CGFloat(t.hairline)*1.5;ring.lineDashPattern=[NSNumber(value:t.dash),NSNumber(value:t.dash)]
        ring.removeAnimation(forKey:"turn")
        if running && !NativeSkin.reducedMotion {let a=CABasicAnimation(keyPath:"transform.rotation.z");a.fromValue=0;a.toValue=Double.pi*2;a.duration=t.turn;a.repeatCount = .infinity;a.isRemovedOnCompletion=false;ring.add(a,forKey:"turn")}
    }
    override func layoutSublayers() {super.layoutSublayers();ring.bounds=bounds;ring.position=CGPoint(x:bounds.midX,y:bounds.midY);ring.path=CGPath(ellipseIn:bounds.insetBy(dx:1,dy:1),transform:nil)}
}

#endif

#if canImport(UIKit)
/// A view with one of the skin's frames behind what it holds.
class NativeFrameView:UIView,NativeArcadeNode {
    var arcadePaint=NativeArcadePaint()
    init(role:Int64=NativeSkin.names.panel,lit:Bool=false) {super.init(frame:.zero);isUserInteractionEnabled=false;isOpaque=false;backgroundColor = .clear;surface(role,lit:lit)}
    required init?(coder:NSCoder) {fatalError()}
    func surface(_ role:Int64,lit:Bool=false) {let s=mono_surface(role,lit);paint(role:s.role,bg:s.bg,line:s.line,deco:s.deco);arcadePaint.flags=lit ? 1:0}
    func paint(role:Int64,bg:UInt32,line:UInt32,deco:UInt32,dashed:Bool=false,march:Bool=false) {arcadePaint.role=role;arcadePaint.background=bg;arcadePaint.line=line;arcadePaint.ink=deco}
    override func didMoveToSuperview() {super.didMoveToSuperview();NativeArcadeSurface.invalidateTree()}
}

/// The shared two-arc activity component, drawn by the containing GPU scene.
final class NativeRingView:UIView,NativeArcadeNode {
    var arcadePaint=NativeArcadePaint(kind:1,ink:NativeSkin.tokens.ink_muted)
    override init(frame:CGRect) {super.init(frame:frame);isUserInteractionEnabled=false;backgroundColor = .clear}
    required init?(coder:NSCoder) {fatalError()}
    func paint(_ ink:UInt32) {arcadePaint.ink=ink}
    override func didMoveToSuperview() {super.didMoveToSuperview();NativeArcadeSurface.invalidateTree()}
    override var intrinsicContentSize:CGSize {CGSize(width:20,height:20)}
}

/// A download's determinate value is animated by the shared Coax spring.
final class NativeBar:UIView,NativeArcadeNode {
    var progress:Float=0 {didSet {setNeedsLayout()}}
    var arcadePaint:NativeArcadePaint {NativeArcadePaint(kind:2,line:NativeSkin.tokens.line_dim,ink:NativeSkin.tokens.ink,value:progress)}
    override init(frame:CGRect) {super.init(frame:frame);isAccessibilityElement=true;accessibilityTraits = .updatesFrequently}
    required init?(coder:NSCoder) {fatalError()}
    override var intrinsicContentSize:CGSize {CGSize(width:UIView.noIntrinsicMetric,height:2)}
    override func layoutSubviews() {super.layoutSubviews();accessibilityValue="\(Int(max(0,min(1,progress))*100))%"}
    override func didMoveToSuperview() {super.didMoveToSuperview();NativeArcadeSurface.invalidateTree()}
}
/// A rule or table cell is a kit primitive in the same composited scene.
final class NativeRuleView:NativeFrameView {
    init(_ ink:UInt32=NativeSkin.tokens.line_dim,box:Bool=false,fill:UInt32=0) {super.init(role:NativeSkin.names.box);arcadePaint.kind=box ? 4:3;arcadePaint.line=ink;arcadePaint.background=fill}
    required init?(coder:NSCoder) {fatalError()}
}

final class NativeNotice:UILabel {
    private let surface=NativeFrameView(role:NativeSkin.names.notice)
    override init(frame:CGRect) {super.init(frame:frame);addSubview(surface)}
    required init?(coder:NSCoder) {fatalError()}
    override func layoutSubviews() {super.layoutSubviews();surface.frame=bounds}
    override func drawText(in rect:CGRect) {super.drawText(in:rect.insetBy(dx:12,dy:10))}
    override func textRect(forBounds bounds:CGRect,limitedToNumberOfLines lines:Int)->CGRect {
        let text=super.textRect(forBounds:bounds.insetBy(dx:12,dy:10),limitedToNumberOfLines:lines)
        return text.insetBy(dx:-12,dy:-10)
    }
}

/// A world HUD layer passes empty space through to the scene below it.
final class NativeOverlayView:UIView {
    override func hitTest(_ point:CGPoint,with event:UIEvent?)->UIView? {let found=super.hitTest(point,with:event);return found === self ? nil:found}
}

/// The library's button. Its variant picks the frame and inks; its state is rest, pressed, disabled, busy or current.
/// The label is drawn in the skin's chrome type; what VoiceOver and UI tests read is the label as it was given.
final class NativeButton:UIButton {
    enum Variant {
        case plain,primary,quiet,icon,destructive,list,action,rail,toggle
        var number:Int64 {let n=NativeSkin.names;switch self {case .plain:return n.plain;case .primary:return n.primary;case .quiet,.toggle:return n.quiet;case .icon:return n.icon;case .destructive:return n.destructive;case .list:return n.list;case .action:return n.action;case .rail:return n.rail}}
        /// Only an icon, in a small square.
        var square:Bool {self == .icon || self == .action || self == .rail}
    }
    var variant:Variant {didSet {if variant != oldValue {restyle()}}}
    /// It is doing what it was pressed for: it can't be pressed, and its line moves.
    var busy=false {didSet {if busy != oldValue {restyle()}}}
    /// The row that is open now.
    var current=false {didSet {if current != oldValue {restyle()}}}
    /// Over a picture (a world): a dim ground stays behind the icon so it reads on anything.
    var floating=false {didSet {restyle()}}
    /// Native Liquid Glass for floating controls; older iOS keeps the shared frame.
    var glass=false {didSet {if glass != oldValue {restyle()}}}
    /// A list row's hairline. A row that sets its own spacing (a tool call) goes without.
    var ruled=true {didSet {restyle()}}
    var drawerProgress:Float? {didSet {if (oldValue==nil) != (drawerProgress==nil) {restyle()};drawerPaint()}}
    var drawerInteractive=false {didSet {drawerPaint()}}
    var drawerRTL=false {didSet {drawerPaint()}}
    private func drawerPaint() {if let drawerProgress {chrome.arcadePaint.kind=7;chrome.arcadePaint.ink=style.ink;chrome.arcadePaint.value=drawerProgress;chrome.arcadePaint.flags=(drawerInteractive ? 4:0)|(drawerRTL ? 8:0)}}
    private let chrome=NativeFrameView(role:NativeSkin.names.button),caption=UILabel(),glyph=UIImageView(),ring=NativeRingView(),rule=NativeRuleView()
    private var glassView:UIVisualEffectView?
    private var words=""
    private(set) var symbol:String?
    init(_ variant:Variant = .plain) {
        self.variant=variant;super.init(frame:.zero)
        for v in [chrome,rule,caption,glyph,ring] as [UIView] {v.isUserInteractionEnabled=false;addSubview(v)}
        caption.numberOfLines=2;caption.lineBreakMode = .byTruncatingTail;glyph.contentMode = .center;ring.isHidden=true
        NotificationCenter.default.addObserver(self,selector:#selector(typeChanged),name:UIContentSizeCategory.didChangeNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(typeChanged),name:UIAccessibility.reduceMotionStatusDidChangeNotification,object:nil)
        restyle()
    }
    required init?(coder:NSCoder) {fatalError()}
    @objc private func typeChanged() {restyle()}
    /// What it says, and for an icon button the SF Symbol it shows. The words are also its accessibility label.
    func show(_ label:String,symbol:String?=nil) {
        guard label != words || symbol != self.symbol else {return}
        words=label;self.symbol=symbol;accessibilityLabel=label;restyle()
    }
    override var isHighlighted:Bool {didSet {if isHighlighted != oldValue {restyle()}}}
    override var isEnabled:Bool {didSet {if isEnabled != oldValue {restyle()}}}
    private var state_:Int64 {let n=NativeSkin.names;return busy ? n.busy:!isEnabled ? n.disabled:isHighlighted ? n.pressed:current ? n.current:n.rest}
    private var style:coax_t_MonoButton {mono_button(variant.number,state_)}
    private func restyle() {
        let s=style,t=NativeSkin.tokens
        #if compiler(>=6.2)
        if #available(iOS 26.0, *),glass {
            if glassView == nil {
                let effect=UIGlassEffect(style:.regular);effect.isInteractive=true
                let view=UIVisualEffectView(effect:effect);view.isUserInteractionEnabled=false
                view.clipsToBounds=true;insertSubview(view,aboveSubview:chrome);glassView=view
            }
        } else {glassView?.removeFromSuperview();glassView=nil}
        #endif
        chrome.isHidden=glassView != nil
        var bg=s.bg,line=s.line
        if floating && state_==NativeSkin.names.rest {bg=t.ground & 0xffffff8c;line=t.line_dim}
        chrome.paint(role:s.role,bg:bg,line:line,deco:line,dashed:s.dashed,march:s.march)
        chrome.arcadePaint.flags=isHighlighted ? 1:0
        chrome.arcadePaint.kind=variant == .action || variant == .rail ? 5:variant == .toggle ? 6:0
        chrome.arcadePaint.value=variant == .toggle && current ? 1:0
        if variant == .toggle {chrome.arcadePaint.ink=s.ink}
        let font:UIFont=s.upper ? NativeSkin.labelFont(CGFloat(s.type)):NativeTypography.font(CGFloat(s.type))
        caption.attributedText=NSAttributedString(string:s.upper ? words.uppercased():words,attributes:[.font:font,.kern:font.pointSize*CGFloat(s.track),.foregroundColor:NativeSkin.color(s.ink)])
        caption.numberOfLines=variant == .list ? 1:2;caption.textAlignment=variant == .list ? .natural:.center;caption.isHidden=variant.square
        // A row of a list stands over a hairline, unless it is boxed as the current one.
        rule.isHidden=variant != .list || !ruled || current || isHighlighted
        let spinning=variant.square && busy
        glyph.isHidden=symbol==nil || spinning || drawerProgress != nil;ring.isHidden = !spinning;ring.paint(s.ink)
        glyph.image=symbol.flatMap {UIImage(systemName:$0,withConfiguration:UIImage.SymbolConfiguration(pointSize:CGFloat(variant.square ? s.icon:s.type),weight:.medium))};glyph.tintColor=NativeSkin.color(s.ink)
        drawerPaint();if variant == .toggle {accessibilityValue=current ? "On":"Off"};invalidateIntrinsicContentSize();setNeedsLayout()
    }
    /// Room at a label's end for a mark beside it (a select's chevron).
    private var mark:CGFloat {!variant.square && symbol != nil ? CGFloat(style.icon)+6:0}
    private func textHeight(_ width:CGFloat)->CGFloat {ceil(caption.sizeThatFits(CGSize(width:max(1,width),height:.greatestFiniteMagnitude)).height)}
    override var intrinsicContentSize:CGSize {
        let s=style,hit=CGFloat(NativeSkin.tokens.hit)
        if variant.square {return CGSize(width:max(hit,CGFloat(s.side)),height:max(hit,CGFloat(s.height)))}
        let natural=ceil(caption.sizeThatFits(CGSize(width:CGFloat.greatestFiniteMagnitude,height:.greatestFiniteMagnitude)).width)+CGFloat(s.pad)*2+mark
        let width=bounds.width>0 ? bounds.width:natural
        return CGSize(width:natural,height:max(hit,max(CGFloat(s.height),textHeight(width-CGFloat(s.pad)*2-mark)+16)))
    }
    override func layoutSubviews() {
        super.layoutSubviews();let s=style
        glassView?.frame=bounds;glassView?.layer.cornerRadius=min(bounds.width,bounds.height)/2
        if variant.square {
            let side=min(CGFloat(s.side),min(bounds.width,bounds.height))
            chrome.frame=CGRect(x:(bounds.width-side)/2,y:(bounds.height-side)/2,width:side,height:side).integral
            glyph.frame=bounds;ring.frame=CGRect(x:(bounds.width-18)/2,y:(bounds.height-18)/2,width:18,height:18);return
        }
        if variant == .toggle {caption.frame=CGRect(x:0,y:0,width:max(1,bounds.width-64),height:bounds.height);chrome.frame=CGRect(x:bounds.width-52,y:(bounds.height-28)/2,width:52,height:28);return}
        let pad=CGFloat(s.pad),inner=max(1,bounds.width-pad*2-mark),text=textHeight(inner)
        let tall=min(bounds.height,max(CGFloat(s.height),text+16))
        chrome.frame=CGRect(x:0,y:(bounds.height-tall)/2,width:bounds.width,height:tall).integral
        caption.frame=CGRect(x:pad,y:(bounds.height-text)/2,width:inner,height:text)
        glyph.frame=CGRect(x:bounds.width-pad-mark+6,y:0,width:max(0,mark-6),height:bounds.height)
        rule.frame=CGRect(x:0,y:bounds.height-CGFloat(NativeSkin.tokens.hairline),width:bounds.width,height:CGFloat(NativeSkin.tokens.hairline))
        if abs(intrinsicContentSize.height-bounds.height)>0.5 {invalidateIntrinsicContentSize()}
    }
    /// A finger's target is never smaller than the skin's least, whatever the frame's size.
    override func point(inside point:CGPoint,with event:UIEvent?)->Bool {
        let hit=CGFloat(NativeSkin.tokens.hit)
        return bounds.insetBy(dx:min(0,(bounds.width-hit)/2),dy:min(0,(bounds.height-hit)/2)).contains(point)
    }
}

/// A menu: rows in the skin's panel, over everything, beside the control that opened it. A tap outside puts it away.
final class NativeMenu:UIControl {
    struct Item {var id:String;var title:String;var symbol:String?=nil;var current=false;var action:()->Void}
    private let panel=NativeFrameView(role:NativeSkin.names.panel),column=UIStackView(),box=UIView(),scroll=UIScrollView()
    private weak var anchor:UIView?
    private var arcade:NativeArcadeSurface?
    static func show(from anchor:UIView,in host:UIView?=nil,title:String?=nil,items:[Item]) {
        guard let host=host ?? anchor.window,!items.isEmpty,!host.subviews.contains(where:{$0 is NativeMenu}) else {return}
        let menu=NativeMenu(frame:host.bounds);menu.anchor=anchor;menu.autoresizingMask=[.flexibleWidth,.flexibleHeight]
        if let title,!title.isEmpty {
            let name=UILabel();name.attributedText=NativeSkin.label(title);name.accessibilityLabel=title;name.accessibilityTraits.insert(.header)
            let inset=UIView();inset.addSubview(name);name.translatesAutoresizingMaskIntoConstraints=false
            NSLayoutConstraint.activate([name.leadingAnchor.constraint(equalTo:inset.leadingAnchor,constant:12),name.trailingAnchor.constraint(equalTo:inset.trailingAnchor,constant:-12),name.topAnchor.constraint(equalTo:inset.topAnchor,constant:10),name.bottomAnchor.constraint(equalTo:inset.bottomAnchor,constant:-6)])
            menu.column.addArrangedSubview(inset)
        }
        for item in items {
            let row=NativeButton(.list);row.show(item.title,symbol:item.symbol);row.current=item.current;row.accessibilityIdentifier=item.id
            row.addAction(UIAction { [weak menu] _ in menu?.removeFromSuperview();item.action() },for:.primaryActionTriggered);menu.column.addArrangedSubview(row)
        }
        (menu.column.arrangedSubviews.last as? NativeButton)?.ruled=false
        host.addSubview(menu);UIAccessibility.post(notification:.screenChanged,argument:menu.column)
    }
    override init(frame:CGRect) {
        super.init(frame:frame);accessibilityViewIsModal=true
        column.axis = .vertical;addSubview(box);box.backgroundColor=NativeSkin.color(NativeSkin.tokens.ground);box.addSubview(panel);box.addSubview(scroll);scroll.addSubview(column);arcade=NativeArcadeSurface(in:box)
        addAction(UIAction { [weak self] _ in self?.removeFromSuperview() },for:.touchUpInside)
    }
    required init?(coder:NSCoder) {fatalError()}
    override func accessibilityPerformEscape()->Bool {removeFromSuperview();return true}
    override func layoutSubviews() {
        super.layoutSubviews();guard let anchor else {return}
        let at=anchor.convert(anchor.bounds,to:self),safe=bounds.inset(by:safeAreaInsets).insetBy(dx:8,dy:8)
        let width=min(260,safe.width),contentHeight=column.systemLayoutSizeFitting(CGSize(width:width-12,height:0),withHorizontalFittingPriority:.required,verticalFittingPriority:.fittingSizeLevel).height+12
        let height=min(contentHeight,safe.height)
        let x=at.midX>bounds.midX ? min(safe.maxX,at.maxX)-width:max(safe.minX,at.minX)
        let below=at.maxY+6,y=below+height<=safe.maxY ? below:max(safe.minY,at.minY-6-height)
        box.frame=CGRect(x:x,y:y,width:width,height:height).integral;panel.frame=box.bounds;scroll.frame=box.bounds.insetBy(dx:6,dy:6);column.frame=CGRect(x:0,y:0,width:scroll.bounds.width,height:contentHeight-12);scroll.contentSize=column.bounds.size;arcade?.frame=box.bounds;arcade?.refresh()
    }
}

/// Quiet buttons that follow one another share a line, each as wide as its words, and wrap to the next.
final class NativeFlow:UIView {
    var centered=false {didSet {setNeedsLayout()}}
    var items:[UIView]=[] {
        didSet {
            for v in oldValue where v.superview === self && !items.contains(where:{$0 === v}) {v.removeFromSuperview()}
            for v in items where v.superview !== self {addSubview(v)}
            laid=0;invalidateIntrinsicContentSize();setNeedsLayout()
        }
    }
    private var laid:CGFloat=0
    private func arrange(_ width:CGFloat,apply:Bool)->CGFloat {
        let gap=CGFloat(NativeSkin.tokens.gap)*2/3;var x:CGFloat=0,y:CGFloat=0,row:CGFloat=0,start=0
        func centerRow(_ end:Int) {guard apply,centered,end>start else{return};let shift=max(0,(width-(x-gap))/2);for i in start..<end {items[i].frame.origin.x += shift}}
        for (index,v) in items.enumerated() {
            let size=v.intrinsicContentSize,w=min(size.width,width)
            if x>0 && x+w>width {centerRow(index);start=index;x=0;y+=row;row=0}
            if apply {v.frame=CGRect(x:x,y:y,width:w,height:size.height)}
            x+=w+gap;row=max(row,size.height)
        }
        centerRow(items.count);return y+row
    }
    override func layoutSubviews() {
        super.layoutSubviews();guard bounds.width>0 else {return}
        let h=arrange(bounds.width,apply:true);if abs(h-laid)>0.5 {laid=h;invalidateIntrinsicContentSize()}
    }
    override var intrinsicContentSize:CGSize {CGSize(width:UIView.noIntrinsicMetric,height:laid>0 ? laid:CGFloat(NativeSkin.tokens.hit))}
}

/// A native editor with shared animated focus chrome.
final class NativeField:UITextField {
    var sensitive=false {didSet {if sensitive && !oldValue {NotificationCenter.default.addObserver(self,selector:#selector(clearSensitive),name:UIApplication.willResignActiveNotification,object:nil)}}}
    @objc private func clearSensitive() {if sensitive {text=nil;isSecureTextEntry=true}}
    private let chrome=NativeFrameView(role:NativeSkin.names.field)
    override init(frame:CGRect) {
        super.init(frame:frame);borderStyle = .none;backgroundColor = .clear;keyboardAppearance = .dark
        font=NativeTypography.font(16);adjustsFontForContentSizeCategory=true;tintColor=NativeSkin.color(NativeSkin.tokens.ink)
        addSubview(chrome)
        addAction(UIAction { [weak self] _ in self?.restyle() },for:[.editingDidBegin,.editingDidEnd])
        restyle()
    }
    required init?(coder:NSCoder) {fatalError()}
    override var isEnabled:Bool {didSet {restyle()}}
    override var placeholder:String? {didSet {restyle()}}
    private var style:coax_t_MonoField {mono_field(isFirstResponder,!isEnabled)}
    private func restyle() {
        let s=style;chrome.paint(role:s.role,bg:s.bg,line:s.line,deco:s.line);chrome.arcadePaint.flags=2 | (isFirstResponder ? 1:0);textColor=NativeSkin.color(s.ink)
        if let placeholder {attributedPlaceholder=NSAttributedString(string:placeholder,attributes:[.foregroundColor:NativeSkin.color(s.hint),.font:NativeTypography.font(16)])}
    }
    override var intrinsicContentSize:CGSize {CGSize(width:UIView.noIntrinsicMetric,height:max(CGFloat(style.height),ceil((font?.lineHeight ?? 20)+20)))}
    override func textRect(forBounds bounds:CGRect)->CGRect {bounds.insetBy(dx:CGFloat(style.pad),dy:0)}
    override func editingRect(forBounds bounds:CGRect)->CGRect {textRect(forBounds:bounds)}
    override func placeholderRect(forBounds bounds:CGRect)->CGRect {textRect(forBounds:bounds)}
    override func layoutSubviews() {super.layoutSubviews();chrome.frame=bounds;sendSubviewToBack(chrome)}
}

/// Many lines of typing in the same frame. The frame stays put while the text scrolls. It is as tall as its
/// text, from one target to eight lines (the web skin's MONO_FIELD_LINES), unless a height is set (a code field).
final class NativeTextArea:UITextView {
    private let chrome=NativeFrameView(role:NativeSkin.names.field)
    private lazy var tall=heightAnchor.constraint(equalToConstant:CGFloat(NativeSkin.tokens.hit))
    var fixedHeight:CGFloat? {didSet {setNeedsLayout()}}
    init() {
        super.init(frame:.zero,textContainer:nil);backgroundColor = .clear;keyboardAppearance = .dark
        tintColor=NativeSkin.color(NativeSkin.tokens.ink);textContainerInset=UIEdgeInsets(top:12,left:8,bottom:12,right:8)
        insertSubview(chrome,at:0);restyle()
        NotificationCenter.default.addObserver(self,selector:#selector(focusChanged),name:UITextView.textDidBeginEditingNotification,object:self)
        NotificationCenter.default.addObserver(self,selector:#selector(focusChanged),name:UITextView.textDidEndEditingNotification,object:self)
    }
    required init?(coder:NSCoder) {fatalError()}
    @objc private func focusChanged() {restyle()}
    override var isEditable:Bool {didSet {restyle()}}
    private func restyle() {let s=mono_field(isFirstResponder,!isEditable);chrome.paint(role:s.role,bg:s.bg,line:s.line,deco:s.line);chrome.arcadePaint.flags=2 | (isFirstResponder ? 1:0);textColor=NativeSkin.color(s.ink)}
    override func layoutSubviews() {
        super.layoutSubviews();chrome.frame=bounds;sendSubviewToBack(chrome)
        let line=font?.lineHeight ?? 20,fit=sizeThatFits(CGSize(width:bounds.width,height:.greatestFiniteMagnitude)).height
        let want=fixedHeight ?? min(max(CGFloat(NativeSkin.tokens.hit),ceil(fit)),ceil(line*8+24))
        if !tall.isActive || abs(tall.constant-want)>0.5 {tall.constant=want;tall.isActive=true}
    }
}

/// A header's rule: the skin's header frame along an edge, under a title.
final class NativeHeaderRule:UIView {
    private let chrome=NativeFrameView(role:NativeSkin.names.header)
    override init(frame:CGRect) {super.init(frame:frame);isUserInteractionEnabled=false;addSubview(chrome)}
    required init?(coder:NSCoder) {fatalError()}
    override var intrinsicContentSize:CGSize {CGSize(width:UIView.noIntrinsicMetric,height:8)}
    override func layoutSubviews() {super.layoutSubviews();chrome.arcadePaint.kind=3;chrome.frame=CGRect(x:0,y:bounds.midY,width:bounds.width,height:CGFloat(NativeSkin.tokens.hairline))}
}
#else
/// A view with one of the skin's frames behind what it holds.
final class NativeFrameView:NSView {
    private let frameLayer=NativeFrame()
    init(role:Int64=NativeSkin.names.panel,lit:Bool=false) {super.init(frame:.zero);wantsLayer=true;layer=frameLayer;surface(role,lit:lit)}
    required init?(coder:NSCoder) {fatalError()}
    override func hitTest(_ point:NSPoint)->NSView? {nil}
    func surface(_ role:Int64,lit:Bool=false) {let s=mono_surface(role,lit);frameLayer.role=s.role;frameLayer.paint(bg:s.bg,line:s.line,deco:s.deco)}
    func paint(role:Int64,bg:UInt32,line:UInt32,deco:UInt32,dashed:Bool=false,march:Bool=false) {frameLayer.role=role;frameLayer.paint(bg:bg,line:line,deco:deco,dashed:dashed,march:march)}
}

/// The library's button on the Mac: the same variants, states, frames and inks as the phone's.
final class NativeButton:NSButton {
    enum Variant {
        case plain,primary,quiet,destructive,list
        var number:Int64 {let n=NativeSkin.names;switch self {case .plain:return n.plain;case .primary:return n.primary;case .quiet:return n.quiet;case .destructive:return n.destructive;case .list:return n.list}}
    }
    var variant:Variant = .plain {didSet {restyle()}}
    var busy=false {didSet {restyle()}}
    private let chrome=NativeFrameView(role:NativeSkin.names.button)
    private var words="",down=false
    init(_ variant:Variant = .plain) {
        super.init(frame:.zero);self.variant=variant
        isBordered=false;setButtonType(.momentaryChange);focusRingType = .none;wantsLayer=true
        addSubview(chrome,positioned:.below,relativeTo:nil);restyle()
    }
    required init?(coder:NSCoder) {fatalError()}
    func show(_ label:String) {words=label;setAccessibilityLabel(label);restyle()}
    override var isEnabled:Bool {didSet {restyle()}}
    override func mouseDown(with event:NSEvent) {guard isEnabled else {return};down=true;restyle();super.mouseDown(with:event);down=false;restyle()}
    private var style:coax_t_MonoButton {let n=NativeSkin.names;return mono_button(variant.number,busy ? n.busy:!isEnabled ? n.disabled:down ? n.pressed:n.rest)}
    private func restyle() {
        let s=style;chrome.paint(role:s.role,bg:s.bg,line:s.line,deco:s.line,dashed:s.dashed,march:s.march)
        let font:NSFont=s.upper ? NativeSkin.labelFont(CGFloat(s.type)):NSFont.systemFont(ofSize:CGFloat(s.type))
        let paragraph=NSMutableParagraphStyle();paragraph.alignment=variant == .list ? .natural:.center
        attributedTitle=NSAttributedString(string:s.upper ? words.uppercased():words,attributes:[.font:font,.kern:font.pointSize*CGFloat(s.track),.foregroundColor:NativeSkin.color(s.ink),.paragraphStyle:paragraph])
        invalidateIntrinsicContentSize()
    }
    override var intrinsicContentSize:NSSize {let s=style;return NSSize(width:ceil(attributedTitle.size().width)+CGFloat(s.pad)*2,height:CGFloat(s.height))}
    override func layout() {super.layout();chrome.frame=bounds}
}

/// One line of typing over the skin's underline on the Mac.
final class NativeField:NSTextField {
    private let chrome=NativeFrame()
    override init(frame:NSRect) {
        super.init(frame:frame);isBordered=false;isBezeled=false;drawsBackground=false;focusRingType = .none;wantsLayer=true
        font = .systemFont(ofSize:15);chrome.role=NativeSkin.names.field;layer?.addSublayer(chrome);restyle(false)
    }
    required init?(coder:NSCoder) {fatalError()}
    override var isEnabled:Bool {didSet {restyle(false)}}
    override var placeholderString:String? {didSet {restyle(false)}}
    func restyle(_ focused:Bool) {
        let s=mono_field(focused,!isEnabled);chrome.paint(bg:s.bg,line:s.line,deco:s.line);textColor=NativeSkin.color(s.ink)
        if let placeholderString {placeholderAttributedString=NSAttributedString(string:placeholderString,attributes:[.foregroundColor:NativeSkin.color(s.hint),.font:NSFont.systemFont(ofSize:15)])}
    }
    override func becomeFirstResponder()->Bool {let ok=super.becomeFirstResponder();if ok {restyle(true)};return ok}
    override func textDidEndEditing(_ notification:Notification) {super.textDidEndEditing(notification);restyle(false)}
    override var intrinsicContentSize:NSSize {NSSize(width:NSView.noIntrinsicMetric,height:CGFloat(mono_field(false,false).height)-12)}
    override func layout() {super.layout();chrome.frame=bounds}
}
#endif
