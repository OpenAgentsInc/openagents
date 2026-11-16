// Firm reason: 6 (UIKit text input over a GPU component surface: the platform editor and its font).
// The platform editors that sit over GPU kit fields (components/gpu-components.md, "Text entry";
// GitHub #13645): the kit's own face at the size a surface draws it, and the multiline editor a
// composer uses while it holds the keyboard. A surface's Coax says how the editor is styled (the
// text's size and line in points, then the ink, the placeholder's and the caret's as straight
// RGBA: 14 floats); nothing here names a colour or a size.
import UIKit
import CoreText

/// The kit's own face at the size the GPU surfaces draw it, for the platform editors over them.
enum NativeKitText {
    static func face(_ size:CGFloat)->UIFont {
        if let path=Bundle.main.path(forResource:"Geist",ofType:"ttf",inDirectory:"Components/fonts"),let data=CGDataProvider(url:URL(fileURLWithPath:path) as CFURL),let font=CGFont(data) {
            return CTFontCreateWithGraphicsFont(font,size,nil,nil) as UIFont
        }
        return .systemFont(ofSize:size)
    }
    static func color(_ style:[Float],_ at:Int)->UIColor {UIColor(red:CGFloat(style[at]),green:CGFloat(style[at+1]),blue:CGFloat(style[at+2]),alpha:CGFloat(style[at+3]))}
}

/// The one platform editor for a GPU composer. While it does not hold the keyboard it is out of
/// the view hierarchy and stands for the field's row in the accessibility tree: activating it
/// gives the text the keyboard.
final class ComposerEditor:UITextView {
    private var style:[Float]
    private var face:UIFont
    private var attributes:[NSAttributedString.Key:Any]
    private var line:CGFloat {CGFloat(style[1]>0 ? style[1]:23)}
    weak var host:UIView?
    var box=CGRect.zero
    var attach:(()->Void)?,activate:(()->Bool)?
    private let hint=UILabel()
    var placeholder:String? {didSet {hint.text=placeholder}}
    private static func look(_ style:[Float])->(UIFont,[NSAttributedString.Key:Any]) {
        let face=NativeKitText.face(CGFloat(style[0]>0 ? style[0]:17))
        let paragraph=NSMutableParagraphStyle();let height=CGFloat(style[1]>0 ? style[1]:23);paragraph.minimumLineHeight=height;paragraph.maximumLineHeight=height
        return (face,[.font:face,.foregroundColor:NativeKitText.color(style,2),.paragraphStyle:paragraph])
    }
    init(style:[Float]) {
        self.style=style;(face,attributes)=ComposerEditor.look(style)
        super.init(frame:.zero,textContainer:nil)
        backgroundColor = .clear;textColor=NativeKitText.color(style,2);tintColor=NativeKitText.color(style,10);keyboardAppearance = .dark
        font=face;typingAttributes=attributes
        textContainer.lineFragmentPadding=0;textContainerInset=UIEdgeInsets(top:6,left:0,bottom:6,right:0)
        isScrollEnabled=false;returnKeyType = .default;keyboardDismissMode = .interactive
        // The editor draws the placeholder while it has the keyboard; the surface draws it otherwise.
        hint.font=face;hint.textColor=NativeKitText.color(style,6);hint.isUserInteractionEnabled=false;hint.isAccessibilityElement=false;addSubview(hint)
        NotificationCenter.default.addObserver(self,selector:#selector(textChanged),name:UITextView.textDidChangeNotification,object:self)
    }
    required init?(coder:NSCoder) {fatalError()}
    func set(_ value:String) {attributedText=NSAttributedString(string:value,attributes:attributes);typingAttributes=attributes;textChanged()}
    /// The surface draws its text at another size (large text): the editor's face, line and placeholder follow.
    /// Marked text is left to finish; the next edit takes the new style.
    func restyle(_ next:[Float]) {
        style=next;(face,attributes)=ComposerEditor.look(next)
        font=face;hint.font=face;typingAttributes=attributes
        if markedTextRange==nil {set(text ?? "")}
        setNeedsLayout()
    }
    @objc private func textChanged() {hint.isHidden = !(text ?? "").isEmpty}
    override func layoutSubviews() {super.layoutSubviews();hint.frame=CGRect(x:0,y:6,width:bounds.width,height:line)}
    /// The text's own height at a width, without the insets: what the composer grows by.
    func textHeight(width:CGFloat)->CGFloat {max(0,sizeThatFits(CGSize(width:max(40,width),height:.greatestFiniteMagnitude)).height-12)}
    override func becomeFirstResponder()->Bool {if superview==nil {attach?()};return super.becomeFirstResponder()}
    override var isAccessibilityElement:Bool {get {true} set {}}
    override var accessibilityFrame:CGRect {
        get {if superview==nil,let host {return UIAccessibility.convertToScreenCoordinates(box,in:host)};return super.accessibilityFrame}
        set {super.accessibilityFrame=newValue}
    }
    override func accessibilityActivate()->Bool {if superview==nil {return activate?() ?? false};return super.accessibilityActivate()}
}

/// What a single-line field takes, as its surface's Coax says it.
enum KitFieldKind: Int32 {
    case plain = 0, email = 1, code = 2, secret = 3
}

/// The one platform editor for a GPU single-line field (an email address, a one-time code, a
/// secret) while it holds the keyboard. It is the only editing authority: the surface receives
/// each whole value and paints no text or caret under it. A code field takes the system's
/// one-time-code AutoFill; a secret field hides what is typed and is never offered to
/// autocorrection or the keyboard's learning.
final class FieldEditor:UITextField {
    private var style:[Float]
    private var face:UIFont
    private(set) var kind:KitFieldKind = .plain
    init(style:[Float]) {
        self.style=style;face=NativeKitText.face(CGFloat(style[0]>0 ? style[0]:17))
        super.init(frame:.zero)
        borderStyle = .none;backgroundColor = .clear;font=face;textColor=NativeKitText.color(style,2);tintColor=NativeKitText.color(style,10)
        autocapitalizationType = .none;autocorrectionType = .no;spellCheckingType = .no;smartDashesType = .no;smartQuotesType = .no
        keyboardAppearance = .dark;enablesReturnKeyAutomatically=true;contentVerticalAlignment = .center
        adjustsFontForContentSizeCategory=false
    }
    required init?(coder:NSCoder) {fatalError()}
    /// The surface draws its text at another size (large text): the field's face follows.
    func restyle(_ next:[Float]) {
        style=next;face=NativeKitText.face(CGFloat(next[0]>0 ? next[0]:17));font=face
        if let p=attributedPlaceholder?.string {attributedPlaceholder=NSAttributedString(string:p,attributes:[.foregroundColor:NativeKitText.color(style,6),.font:face])}
    }
    override var accessibilityValue:String? {get {kind == .secret ? nil:super.accessibilityValue} set {super.accessibilityValue=newValue}}
    override func copy(_ sender:Any?) {
        if kind == .secret {
            guard !isSecureTextEntry,let range=selectedTextRange,let value=text(in:range),!value.isEmpty,!UIScreen.main.isCaptured else {return}
            UIPasteboard.general.setItems([["public.utf8-plain-text":value]],options:[.localOnly:true,.expirationDate:Date().addingTimeInterval(60)])
        }else {super.copy(sender)}
    }
    /// Sets what the field takes and its placeholder. Changing the kind reloads the keyboard.
    func configure(_ next:KitFieldKind,placeholder:String) {
        attributedPlaceholder=NSAttributedString(string:placeholder,attributes:[.foregroundColor:NativeKitText.color(style,6),.font:face])
        guard next != kind || keyboardType == .default && next != .plain else {return}
        kind=next
        switch next {
        case .plain:keyboardType = .default;textContentType=nil;returnKeyType = .done;isSecureTextEntry=false
        case .email:keyboardType = .emailAddress;textContentType = .emailAddress;returnKeyType = .send;isSecureTextEntry=false
        case .code:keyboardType = .numberPad;textContentType = .oneTimeCode;returnKeyType = .go;isSecureTextEntry=false
        case .secret:keyboardType = .asciiCapable;textContentType = .password;returnKeyType = .go;isSecureTextEntry=true
        }
        reloadInputViews()
    }
}
