// Firm reason: 6 (UIKit native text editing, measurement, glass and SF Symbols).
// Layout adapted from T3 Code f313d0a6cb (MIT); provenance in components/fonts/.
import UIKit
import CoreText

/// The shared kit's composer, projected with the platform editor. Draft and
/// send state come from Coax; UITextView owns selection, IME, paste and undo.
@MainActor final class NativeComposer:UIView,UITextViewDelegate {
    let editor=UITextView()
    let placeholder=UILabel(),model=UIButton(type:.system),plus=UIButton(type:.system),mic=UIButton(type:.system),send=UIButton(type:.system)
    private let circle=UIView(),arrow=UIImageView()
    private let provider=UIImageView(image:UIImage(systemName:"sparkles")),chevron=UIImageView(image:UIImage(systemName:"chevron.down")),modelLabel=UILabel()
    private let glass:UIVisualEffectView
    var changed:((String)->Void)?,press:((String)->Void)?
    var fieldID="",sendID="",modelName="Gemini 4 Hydrogen"
    private var height:CGFloat=48
    private var active=false,enabled=false,editingAllowed=true
    override init(frame:CGRect) {
        if #available(iOS 26.0,*) {glass=UIVisualEffectView(effect:UIGlassEffect(style:.regular))}
        else {glass=UIVisualEffectView(effect:UIBlurEffect(style:.systemMaterialDark))}
        super.init(frame:frame)
        for name in ["DMSans-Regular","DMSans-Medium"] {
            if let url=Bundle.main.url(forResource:name,withExtension:"ttf") {CTFontManagerRegisterFontsForURL(url as CFURL,.process,nil)}
        }
        layer.shadowColor=UIColor.black.cgColor;layer.shadowOpacity=0.35;layer.shadowOffset=CGSize(width:0,height:6);layer.shadowRadius=28
        addSubview(glass);glass.clipsToBounds=true
        for v in [editor,placeholder,plus,model,mic,send] {glass.contentView.addSubview(v)}
        editor.backgroundColor = .clear;editor.textColor=UIColor(white:0.96,alpha:1)
        editor.font=UIFontMetrics(forTextStyle:.body).scaledFont(for:UIFont(name:"DMSans-Regular",size:16) ?? .systemFont(ofSize:16))
        editor.adjustsFontForContentSizeCategory=true;editor.delegate=self
        let paragraph=NSMutableParagraphStyle();paragraph.minimumLineHeight=23;paragraph.maximumLineHeight=23
        editor.typingAttributes=[.font:editor.font!,.foregroundColor:editor.textColor!,.paragraphStyle:paragraph]
        editor.textContainer.lineFragmentPadding=0;editor.textContainerInset=UIEdgeInsets(top:6,left:0,bottom:6,right:0)
        editor.isScrollEnabled=false;editor.returnKeyType = .default;editor.keyboardDismissMode = .interactive
        placeholder.textColor=UIColor(white:0.51,alpha:1);placeholder.font=editor.font;placeholder.isUserInteractionEnabled=false
        plus.setImage(UIImage(systemName:"plus",withConfiguration:UIImage.SymbolConfiguration(pointSize:20)),for:.normal)
        plus.tintColor=UIColor(white:0.96,alpha:1);plus.accessibilityLabel="Chat options";plus.accessibilityIdentifier="chat.options"
        plus.menu=UIMenu(children:[UIAction(title:"New chat",image:UIImage(systemName:"square.and.pencil")) { [weak self] _ in self?.press?("chat.new") },UIAction(title:"Clear draft",image:UIImage(systemName:"xmark")) { [weak self] _ in self?.press?("chat.clear") }]);plus.showsMenuAsPrimaryAction=true
        mic.setImage(UIImage(systemName:"mic",withConfiguration:UIImage.SymbolConfiguration(pointSize:16)),for:.normal)
        mic.isEnabled=false;mic.tintColor=UIColor(white:0.51,alpha:0.45);mic.accessibilityLabel="Dictation: use the microphone on the keyboard"
        model.tintColor=UIColor(white:0.51,alpha:1);model.showsMenuAsPrimaryAction=true
        for v in [provider,chevron,modelLabel] {v.isUserInteractionEnabled=false;model.addSubview(v)}
        provider.contentMode = .scaleAspectFit;chevron.contentMode = .scaleAspectFit
        provider.tintColor=UIColor(white:0.96,alpha:1);chevron.tintColor=UIColor(white:0.96,alpha:1)
        modelLabel.textColor=UIColor(white:0.96,alpha:1);modelLabel.font=UIFont(name:"DMSans-Medium",size:14);modelLabel.text="Gemini 4 Hydrogen"
        send.addSubview(circle);circle.addSubview(arrow);circle.isUserInteractionEnabled=false
        send.addAction(UIAction { [weak self] _ in guard let self else{return};self.press?(self.sendID) },for:.primaryActionTriggered)
        NotificationCenter.default.addObserver(self,selector:#selector(typeChanged),name:UIContentSizeCategory.didChangeNotification,object:nil)
        accessibilityIdentifier="chat.composer.frame"
    }
    required init?(coder:NSCoder) {fatalError()}
    @objc private func typeChanged() {
        editor.font=UIFontMetrics(forTextStyle:.body).scaledFont(for:UIFont(name:"DMSans-Regular",size:16) ?? .systemFont(ofSize:16));placeholder.font=editor.font;setNeedsLayout();invalidateIntrinsicContentSize()
    }
    func render(_ e:NativeElement) {
        guard let field=e.children.first(where:{$0.role=="field"}),let action=e.children.last(where:{$0.role=="button"}) else{return}
        fieldID=field.id;sendID=action.id
        editor.accessibilityIdentifier=field.id;editor.accessibilityLabel=field.label;placeholder.text=field.label
        // Do not replace the buffer on a frame/keystroke: selection and marked
        // text survive. A Coax clear/load is a deliberate controlled-value edit.
        if editor.text != field.value && editor.markedTextRange==nil {editor.attributedText=NSAttributedString(string:field.value,attributes:editor.typingAttributes)}
        // A pending turn locks edits without resigning the platform editor and
        // collapsing the keyboard between send, acknowledgement and reply.
        editingAllowed = !field.disabled;active=action.label=="Stop";enabled = !action.disabled
        send.accessibilityIdentifier=action.id;send.accessibilityLabel=active ? "Stop generating":"Send message";send.isEnabled=enabled
        plus.isEnabled = !field.disabled
        model.accessibilityLabel=modelName;model.menu=UIMenu(children:[UIAction(title:modelName,state:.on) {_ in }])
        setNeedsLayout();invalidateIntrinsicContentSize()
    }
    override var intrinsicContentSize:CGSize {CGSize(width:UIView.noIntrinsicMetric,height:height)}
    override func layoutSubviews() {
        super.layoutSubviews();let w=bounds.width;guard w>0 else{return}
        let scale=(editor.font?.pointSize ?? 16)/16
        let inlineWidth=max(40,w-186)
        let inline=editor.sizeThatFits(CGSize(width:inlineWidth,height:.greatestFiniteMagnitude)).height-12
        let expanded=inline>23*scale*1.5 && !editor.text.isEmpty
        let measured=expanded ? editor.sizeThatFits(CGSize(width:w-28,height:.greatestFiniteMagnitude)).height-12:inline
        let g=nc_layout(w,expanded ? max(measured,46*scale):measured,scale,!editor.text.isEmpty)
        let next=CGFloat(g.height)
        if abs(next-height)>0.1 {height=next;invalidateIntrinsicContentSize();superview?.setNeedsLayout()}
        glass.frame=CGRect(x:0,y:0,width:w,height:next);glass.layer.cornerRadius=g.radius
        editor.frame=CGRect(x:g.editor_x,y:g.editor_y,width:g.editor_w,height:g.editor_h)
        editor.isScrollEnabled=measured+12>g.editor_h+1
        placeholder.frame=CGRect(x:editor.frame.minX,y:editor.frame.minY+6,width:editor.frame.width,height:23*scale);placeholder.isHidden = !editor.text.isEmpty
        plus.frame=CGRect(x:0,y:g.toolbar_y,width:44,height:44)
        model.frame=CGRect(x:44,y:g.toolbar_y,width:g.expanded ? min(w-176,220):50,height:44)
        provider.frame=CGRect(x:8,y:14,width:16,height:16)
        modelLabel.isHidden = !g.expanded;modelLabel.frame=CGRect(x:32,y:0,width:model.bounds.width-58,height:44)
        chevron.frame=CGRect(x:g.expanded ? model.bounds.width-18:32,y:17,width:10,height:10)
        mic.frame=CGRect(x:w-88,y:g.toolbar_y,width:44,height:44);send.frame=CGRect(x:g.send_x,y:g.toolbar_y,width:44,height:44)
        circle.frame=CGRect(x:7,y:7,width:g.send_size,height:g.send_size);circle.layer.cornerRadius=g.send_size/2
        circle.backgroundColor=enabled ? UIColor(red:52/255,green:107/255,blue:241/255,alpha:1):UIColor(white:0.96,alpha:0.12)
        arrow.image=UIImage(systemName:active ? "stop.fill":"arrow.up",withConfiguration:UIImage.SymbolConfiguration(pointSize:16,weight:.semibold));arrow.tintColor=UIColor(white:1,alpha:enabled ? 1:0.38);arrow.frame=circle.bounds.insetBy(dx:7,dy:7);arrow.contentMode = .scaleAspectFit
    }
    func textViewDidChange(_ textView:UITextView) {changed?(textView.text);setNeedsLayout()}
    func textView(_ textView:UITextView,shouldChangeTextIn range:NSRange,replacementText text:String)->Bool {editingAllowed}
    override func becomeFirstResponder()->Bool {editor.becomeFirstResponder()}
}
