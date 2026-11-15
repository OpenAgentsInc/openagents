// Firm reason: 6 (UIKit native text editing, measurement and SF Symbols).
// Layout adapted from T3 Code f313d0a6cb (MIT); provenance in components/fonts/.
import UIKit
import CoreText

/// The shared kit's composer, projected with the platform editor. Draft and
/// send state come from Coax; UITextView owns selection, IME, paste and undo.
@MainActor final class NativeComposer:UIView,UITextViewDelegate {
    let editor=UITextView()
    let placeholder=UILabel(),model=NativeButton(.icon),plus=NativeButton(.icon),mic=NativeButton(.icon),send=NativeButton(.action)
    // The kit's composer frame: the skin's panel, its brackets bright while it holds the keyboard.
    private let panel=NativeFrameView(role:NativeSkin.names.panel)
    var changed:((String)->Void)?,press:((String)->Void)?
    var fieldID="",sendID="",modelName=""
    private var height:CGFloat=48
    private var active=false,enabled=false,editingAllowed=true
    override init(frame:CGRect) {
        super.init(frame:frame)
        _=NativeTypography.registered
        let t=NativeSkin.tokens
        addSubview(panel)
        for v in [editor,placeholder,plus,model,mic,send] {addSubview(v)}
        editor.backgroundColor = .clear;editor.textColor=NativeSkin.color(t.ink);editor.tintColor=NativeSkin.color(t.ink);editor.keyboardAppearance = .dark
        editor.font=UIFontMetrics(forTextStyle:.body).scaledFont(for:UIFont(name:"DMSans-Regular",size:16) ?? .systemFont(ofSize:16))
        editor.adjustsFontForContentSizeCategory=true;editor.delegate=self
        let paragraph=NSMutableParagraphStyle();paragraph.minimumLineHeight=23;paragraph.maximumLineHeight=23
        editor.typingAttributes=[.font:editor.font!,.foregroundColor:editor.textColor!,.paragraphStyle:paragraph]
        editor.textContainer.lineFragmentPadding=0;editor.textContainerInset=UIEdgeInsets(top:6,left:0,bottom:6,right:0)
        editor.isScrollEnabled=false;editor.returnKeyType = .default;editor.keyboardDismissMode = .interactive
        placeholder.textColor=NativeSkin.color(t.ink_faint);placeholder.font=editor.font;placeholder.isUserInteractionEnabled=false
        plus.show("Chat options",symbol:"plus");plus.accessibilityIdentifier="chat.options"
        plus.addAction(UIAction { [weak self] _ in
            guard let self else {return}
            NativeMenu.show(from:self.plus,items:[.init(id:"chat.new",title:"New chat",symbol:"square.and.pencil") { [weak self] in self?.press?("chat.new") },.init(id:"chat.clear",title:"Clear draft",symbol:"xmark") { [weak self] in self?.press?("chat.clear") }])
        },for:.primaryActionTriggered)
        mic.show("Dictation: use the microphone on the keyboard",symbol:"mic");mic.isEnabled=false
        model.addAction(UIAction { [weak self] _ in
            guard let self,!self.modelName.isEmpty else {return}
            NativeMenu.show(from:self.model,items:[.init(id:"chat.model.current",title:self.modelName,current:true) {}])
        },for:.primaryActionTriggered)
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
        modelName=e.attrs["title"] ?? ""
        editor.accessibilityIdentifier=field.id;editor.accessibilityLabel=field.label;placeholder.text=field.label
        // Do not replace the buffer on a frame/keystroke: selection and marked
        // text survive. A Coax clear/load is a deliberate controlled-value edit.
        if editor.text != field.value && editor.markedTextRange==nil {editor.attributedText=NSAttributedString(string:field.value,attributes:editor.typingAttributes)}
        // A pending turn locks edits without resigning the platform editor and
        // collapsing the keyboard between send, acknowledgement and reply.
        editingAllowed = !field.disabled;active=action.label=="Stop";enabled = !action.disabled
        send.show(active ? "Stop generating":"Send message",symbol:active ? "stop.fill":"arrow.up");send.accessibilityIdentifier=action.id;send.isEnabled=enabled
        plus.isEnabled = !field.disabled
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
        panel.frame=CGRect(x:0,y:0,width:w,height:next);panel.surface(NativeSkin.names.panel,lit:editor.isFirstResponder);panel.arcadePaint.flags=2 | (editor.isFirstResponder ? 1:0)
        editor.frame=CGRect(x:g.editor_x,y:g.editor_y,width:g.editor_w,height:g.editor_h)
        editor.isScrollEnabled=measured+12>g.editor_h+1
        placeholder.frame=CGRect(x:editor.frame.minX,y:editor.frame.minY+6,width:editor.frame.width,height:23*scale);placeholder.isHidden = !editor.text.isEmpty
        plus.frame=CGRect(x:0,y:g.toolbar_y,width:44,height:44)
        // Beside one line of text the model is its mark alone; under more, its name in a quiet select.
        model.variant=g.expanded ? .quiet:.icon;model.show(modelName,symbol:g.expanded ? "chevron.down":"sparkles")
        model.frame=CGRect(x:44,y:g.toolbar_y,width:g.expanded ? min(w-176,max(50,model.intrinsicContentSize.width)):50,height:44)
        mic.frame=CGRect(x:w-88,y:g.toolbar_y,width:44,height:44);send.frame=CGRect(x:g.send_x,y:g.toolbar_y,width:44,height:44)
    }
    func textViewDidBeginEditing(_ textView:UITextView) {setNeedsLayout()}
    func textViewDidEndEditing(_ textView:UITextView) {setNeedsLayout()}
    func textViewDidChange(_ textView:UITextView) {changed?(textView.text);setNeedsLayout()}
    func textView(_ textView:UITextView,shouldChangeTextIn range:NSRange,replacementText text:String)->Bool {editingAllowed}
    override func becomeFirstResponder()->Bool {editor.becomeFirstResponder()}
}
