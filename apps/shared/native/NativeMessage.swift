// Firm reason: 6 (TextKit selection/layout, UIKit tables, scrolling and links).
// Consumes the shared Coax component tree. It never parses Markdown source.
// Presentation follows T3 Code f313d0a6cb; MIT notice is beside NativeComposer.
import UIKit
import CoreText

enum NativeTypography {
    static let registered:Void = {
        for name in ["DMSans-Regular","DMSans-Medium","DMSans-Bold","DMSans-Italic","DMSans-BoldItalic"] {
            if let url=Bundle.main.url(forResource:name,withExtension:"ttf") {CTFontManagerRegisterFontsForURL(url as CFURL,.process,nil)}
        }
    }()
    static func font(_ size:CGFloat,strong:Bool=false,italic:Bool=false,code:Bool=false)->UIFont {
        _=registered
        let base=code ? UIFont.monospacedSystemFont(ofSize:size,weight:strong ? .semibold:.regular):(UIFont(name:italic ? (strong ? "DMSans-BoldItalic":"DMSans-Italic"):(strong ? "DMSans-Bold":"DMSans-Regular"),size:size) ?? .systemFont(ofSize:size,weight:strong ? .semibold:.regular))
        let face=italic ? base.fontDescriptor.withSymbolicTraits(base.fontDescriptor.symbolicTraits.union(.traitItalic)).map {UIFont(descriptor:$0,size:size)} ?? base:base
        return UIFontMetrics(forTextStyle:.body).scaledFont(for:face)
    }
}
extension NativeElement {
    var textContent:String {label+children.map(\.textContent).joined()}
    func hasClass(_ name:String)->Bool {classes.split(separator:" ").contains(Substring(name))}
    var contentTree:NativeElement {var e=self;e.children=children.filter {$0.role != "button"}.map(\.contentTree);return e}
}

@MainActor final class NativeRichText:UITextView,UITextViewDelegate {
    var tableSurface:NativeRuleView? {didSet {if let tableSurface {insertSubview(tableSurface,at:0)}}}

    var press:((String)->Void)?
    private var quote=false
    private let stripe=NativeRuleView()
    init(_ node:NativeElement,prefix:String="",indent:CGFloat=0,quote:Bool=false,code:Bool=false) {
        super.init(frame:.zero,textContainer:nil)
        let skin=nt_skin();self.quote=quote
        backgroundColor = .clear;isEditable=false;isSelectable=true;isScrollEnabled=false
        textContainerInset=UIEdgeInsets(top:0,left:quote ? 14:0,bottom:0,right:0);textContainer.lineFragmentPadding=0;delegate=self
        accessibilityIdentifier=node.id
        let level=node.role.hasPrefix("h") ? Int(node.role.dropFirst()) ?? 0:0
        let size=CGFloat(level>0 ? nt_heading(Int64(level)):code ? skin.code:skin.body)
        let paragraph=NSMutableParagraphStyle();paragraph.minimumLineHeight=size*(code ? 1.5:level>0 ? 1.35:skin.line/skin.body)
        paragraph.headIndent=indent+(prefix.isEmpty ? 0:20);paragraph.firstLineHeadIndent=indent
        if node.attrs["data-state"]=="right" {paragraph.alignment = .right}else if node.attrs["data-state"]=="center" {paragraph.alignment = .center}
        let out=NSMutableAttributedString(string:prefix,attributes:[.font:NativeTypography.font(size,code:code),.foregroundColor:NativeSkin.color(quote ? skin.muted:skin.ink),.paragraphStyle:paragraph])
        func append(_ e:NativeElement,_ strong:Bool,_ italic:Bool,_ mono:Bool,_ strike:Bool,_ link:String?) {
            let bold=strong || e.role=="strong" || e.role=="th",italics=italic || e.role=="em",fixed=mono || e.role=="code" || e.role=="pre",deleted=strike || e.role=="del"
            let target=e.role=="a" ? e.id:link
            let font=NativeTypography.font(fixed ? CGFloat(skin.code):size,strong:bold,italic:italics,code:fixed)
            var attrs:[NSAttributedString.Key:Any]=[.font:font,.foregroundColor:NativeSkin.color(bold ? skin.strong:quote ? skin.muted:skin.ink),.paragraphStyle:paragraph]
            if deleted {attrs[.strikethroughStyle]=NSUnderlineStyle.single.rawValue}
            if fixed && !code {attrs[.backgroundColor]=NativeSkin.color(skin.code_bg)}
            if let target,let url=URL(string:"coax-component:///"+target) {attrs[.link]=url}
            out.append(NSAttributedString(string:e.label,attributes:attrs))
            for child in e.children {append(child,bold,italics,fixed,deleted,target)}
        }
        append(node,level>0,false,code,false,nil);attributedText=out
        // The skin has no hue to mark a link with: it is the text's ink, underlined.
        linkTextAttributes=[.foregroundColor:NativeSkin.color(skin.link),.underlineStyle:NSUnderlineStyle.single.rawValue,.underlineColor:NativeSkin.color(NativeSkin.tokens.line)]
        if quote {addSubview(stripe)}
    }
    required init?(coder:NSCoder) {fatalError()}
    override func layoutSubviews() {super.layoutSubviews();tableSurface?.frame=bounds;if quote {stripe.frame=CGRect(x:0,y:0,width:2,height:bounds.height)}}
    func height(_ width:CGFloat)->CGFloat {ceil(sizeThatFits(CGSize(width:max(1,width),height:.greatestFiniteMagnitude)).height)}
    func textView(_ textView:UITextView,shouldInteractWith URL:URL,in characterRange:NSRange,interaction:UITextItemInteraction)->Bool {
        if URL.scheme=="coax-component" {press?(String(URL.path.dropFirst()))};return false
    }
}

@MainActor private final class NativeCode:UIView {
    let text:NativeRichText
    let language=UILabel(),copy=NativeButton(.icon),scroll=UIScrollView()
    private let panel=NativeFrameView(role:NativeSkin.names.panel),rule=NativeRuleView()
    init(_ node:NativeElement,press:@escaping(String)->Void) {
        let pre=node.children.first {$0.role=="pre"} ?? NativeElement(id:node.id+".pre",role:"pre",label:"")
        text=NativeRichText(pre,code:true);super.init(frame:.zero)
        let skin=nt_skin();panel.arcadePaint.kind=8;addSubview(panel);addSubview(rule);clipsToBounds=true
        let header=node.children.first {$0.hasClass("code-h")}
        language.text=header?.children.first {$0.role=="span"}?.textContent ?? "text";language.font=NativeTypography.font(13,code:true);language.textColor=NativeSkin.color(skin.muted)
        language.accessibilityIdentifier=node.id+".language"
        if let action=header?.children.first(where:{$0.role=="button"}) {
            copy.accessibilityIdentifier=action.id;copy.show(action.attrs["aria-label"] ?? action.textContent,symbol:action.textContent=="Copied" ? "checkmark":"doc.on.doc")
            copy.addAction(UIAction {_ in press(action.id)},for:.primaryActionTriggered)
        }
        addSubview(language);addSubview(copy);addSubview(scroll);scroll.addSubview(text)
        scroll.showsHorizontalScrollIndicator=true;scroll.alwaysBounceHorizontal=false;scroll.keyboardDismissMode = .interactive
    }
    required init?(coder:NSCoder) {fatalError()}
    func arrange(_ width:CGFloat)->CGFloat {
        language.frame=CGRect(x:14,y:0,width:max(1,width-68),height:40);copy.frame=CGRect(x:width-46,y:0,width:44,height:40);rule.frame=CGRect(x:0,y:39,width:width,height:1)
        let inner=max(1,width-24),natural=ceil(text.attributedText.size().width)+1,w=max(inner,natural),h=text.height(w)
        text.frame=CGRect(x:0,y:0,width:w,height:h);scroll.frame=CGRect(x:12,y:40,width:inner,height:h+8);scroll.contentSize=CGSize(width:w,height:h)
        panel.frame=CGRect(x:0,y:0,width:width,height:48+h)
        return 48+h
    }
}

@MainActor private final class NativeTable:UIView {
    private let scroll=UIScrollView(),grid=UIView()
    private var cells:[[NativeRichText]]=[]
    init(_ node:NativeElement,press:@escaping(String)->Void) {
        super.init(frame:.zero);addSubview(scroll);scroll.addSubview(grid);clipsToBounds=true
        for row in node.children where row.role=="tr" {
            cells.append(row.children.map {cell in let text=NativeRichText(cell);text.press=press;text.textContainerInset=UIEdgeInsets(top:7,left:10,bottom:7,right:10);text.tableSurface=NativeRuleView(nt_skin().border,box:true,fill:cell.role=="th" ? nt_skin().code_bg:0);grid.addSubview(text);return text})
        }
    }
    required init?(coder:NSCoder) {fatalError()}
    func arrange(_ width:CGFloat)->CGFloat {
        let cols=cells.map(\.count).max() ?? 0;guard cols>0 else{return 0}
        var widths=[CGFloat](repeating:80,count:cols)
        for row in cells {for (i,cell) in row.enumerated() {widths[i]=max(widths[i],min(260,ceil(cell.attributedText.size().width)+20))}}
        let spare=max(0,width-widths.reduce(0,+))/CGFloat(cols);widths=widths.map {$0+spare}
        var y:CGFloat=0
        for row in cells {
            let h=(row.enumerated().map {i,cell in cell.height(widths[i])}.max() ?? 0);var x:CGFloat=0
            for (i,cell) in row.enumerated() {cell.textContainerInset=UIEdgeInsets(top:7,left:10,bottom:7,right:10);cell.frame=CGRect(x:x,y:y,width:widths[i],height:h);x+=widths[i]};y+=h
        }
        let w=widths.reduce(0,+);scroll.frame=CGRect(x:0,y:0,width:width,height:y+4);grid.frame=CGRect(x:0,y:0,width:w,height:y);scroll.contentSize=CGSize(width:w,height:y)
        return y+4
    }
}

@MainActor final class NativeMessage:UIView {
    var press:((String)->Void)?
    private var record:NativeElement?
    private let bubble=UIView(),said=NativeFrameView(role:NativeSkin.names.bubble)
    private var blocks:[UIView]=[],copy:NativeButton?,spinner:NativeRingView?
    private var user=false
    private var actions:[String:NativeButton]=[:]
    private lazy var heightConstraint=heightAnchor.constraint(equalToConstant:1)
    override init(frame:CGRect) {super.init(frame:frame);addSubview(bubble);bubble.addSubview(said);heightConstraint.isActive=true;NotificationCenter.default.addObserver(self,selector:#selector(typeChanged),name:UIContentSizeCategory.didChangeNotification,object:nil)}
    required init?(coder:NSCoder) {fatalError()}
    @objc private func typeChanged() {if let record {self.record=nil;render(record)}}
    func render(_ e:NativeElement) {
        guard record != e else{return}
        func updateActions(_ node:NativeElement) {
            if let action=actions[node.id],node.role=="button" {let label=node.attrs["aria-label"] ?? node.textContent;action.show(label,symbol:label=="Copied" ? "checkmark":"doc.on.doc")}
            for child in node.children {updateActions(child)}
        }
        if record?.contentTree==e.contentTree {record=e;updateActions(e);return}
        record=e;user=e.hasClass("msg-user");actions=[:]
        for view in blocks {view.removeFromSuperview()};blocks=[];copy?.removeFromSuperview();copy=nil;spinner?.removeFromSuperview();spinner=nil
        // What the person said stands in the skin's dim bubble frame; the answer is plain on the ground.
        said.isHidden = !user;said.arcadePaint.kind=8
        func add(_ node:NativeElement,quote:Bool=false) {
            if node.hasClass("code") {let code=NativeCode(node) { [weak self] id in self?.press?(id) };bubble.addSubview(code);blocks.append(code);if let id=code.copy.accessibilityIdentifier {actions[id]=code.copy}}
            else if node.role=="table" {let table=NativeTable(node) { [weak self] id in self?.press?(id) };bubble.addSubview(table);blocks.append(table)}
            else if node.role=="hr" {let rule=NativeRuleView(nt_skin().border);bubble.addSubview(rule);blocks.append(rule)}
            else if ["p","h1","h2","h3","h4","h5","h6","li"].contains(node.role) {
                let checked=node.attrs["aria-checked"],level=CGFloat(Int(node.attrs["aria-level"] ?? "1") ?? 1)
                let prefix=node.role != "li" ? "":checked != nil ? (checked=="true" ? "☑\u{FE0E}  ":"☐\u{FE0E}  "):node.attrs["data-state"]=="ordered" ? (node.attrs["value"] ?? "1")+".  ":"•  "
                let text=NativeRichText(node,prefix:prefix,indent:node.role=="li" ? (level-1)*12:0,quote:quote);text.press={ [weak self] id in self?.press?(id) };bubble.addSubview(text);blocks.append(text)
            } else if node.role=="button" {
                let button=NativeButton(.icon);button.accessibilityIdentifier=node.id;button.show(node.textContent,symbol:node.textContent=="Copied" ? "checkmark":"doc.on.doc")
                button.addAction(UIAction { [weak self] _ in self?.press?(node.id) },for:.primaryActionTriggered);addSubview(button);copy=button;actions[node.id]=button
            } else {for child in node.children {add(child,quote:quote || node.role=="blockquote")}}
        }
        for child in e.children {add(child)}
        if e.attrs["data-state"]=="waiting" {let activity=NativeRingView();activity.accessibilityLabel="In progress";activity.isAccessibilityElement=true;bubble.addSubview(activity);spinner=activity}
        setNeedsLayout();superview?.setNeedsLayout()
    }
    override func layoutSubviews() {
        super.layoutSubviews();let skin=nt_skin();guard bounds.width>0 else{return}
        let padding=user ? CGFloat(skin.padding):0,maxWidth=user ? bounds.width*skin.bubble_fraction:bounds.width
        var width=maxWidth
        if user && blocks.allSatisfy({$0 is NativeRichText}) {width=min(maxWidth,max(60,(blocks.compactMap {($0 as? NativeRichText)?.attributedText.size().width}.max() ?? 60)+padding*2+2))}
        let inner=max(1,width-padding*2);var y=padding
        for (i,view) in blocks.enumerated() {
            let h:CGFloat
            if let text=view as? NativeRichText {h=text.height(inner)}else if let code=view as? NativeCode {h=code.arrange(inner)}else if let table=view as? NativeTable {h=table.arrange(inner)}else {h=1}
            view.frame=CGRect(x:padding,y:y,width:inner,height:h);y+=h+(i+1<blocks.count ? CGFloat(skin.gap):0)
        }
        if let spinner {spinner.frame=CGRect(x:padding,y:y+3,width:18,height:18);y+=24}
        y+=padding;bubble.frame=CGRect(x:user ? bounds.width-width:0,y:0,width:width,height:y);said.frame=bubble.bounds
        copy?.frame=CGRect(x:user ? bounds.width-36:-8,y:y+2,width:44,height:32)
        let h=ceil(y+(copy==nil ? 0:36));if abs(heightConstraint.constant-h)>0.5 {heightConstraint.constant=max(1,h);superview?.setNeedsLayout()}
    }
}
