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
    private let style:[Float]
    private let face:UIFont
    private let attributes:[NSAttributedString.Key:Any]
    private var line:CGFloat {CGFloat(style[1]>0 ? style[1]:23)}
    weak var host:UIView?
    var box=CGRect.zero
    var attach:(()->Void)?,activate:(()->Bool)?
    private let hint=UILabel()
    var placeholder:String? {didSet {hint.text=placeholder}}
    init(style:[Float]) {
        self.style=style;face=NativeKitText.face(CGFloat(style[0]>0 ? style[0]:17))
        let paragraph=NSMutableParagraphStyle();let height=CGFloat(style[1]>0 ? style[1]:23);paragraph.minimumLineHeight=height;paragraph.maximumLineHeight=height
        attributes=[.font:face,.foregroundColor:NativeKitText.color(style,2),.paragraphStyle:paragraph]
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
    private let style:[Float]
    private let face:UIFont
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

/// A field that does not hold the keyboard, as VoiceOver sees it: a text field that is never in
/// the view hierarchy and never edits. Activating it gives the field the keyboard, and the
/// platform editor takes its place. A secret field's value is never read out.
final class FieldProxy:UITextField {
    weak var host:UIView?
    var box=CGRect.zero
    var activate:(()->Bool)?
    override var isAccessibilityElement:Bool {get {true} set {}}
    override var accessibilityFrame:CGRect {get {host.map {UIAccessibility.convertToScreenCoordinates(box,in:$0)} ?? .zero} set {}}
    override var accessibilityValue:String? {get {isSecureTextEntry ? nil:text} set {}}
    override func accessibilityActivate()->Bool {activate?() ?? false}
    override var canBecomeFirstResponder:Bool {false}
}

/// Ends text entry on a tap outside the focused field (KitFocus), for every field of a surface.
/// It never takes the tap from the surface: the surface's own touches still arrive, so a control
/// under the tap acts as usual and the same tap gives up the keyboard. Install it once on the
/// surface's view; `field` gives the focused field's frame in that view, nil when none holds the
/// keyboard, and `end` resigns the platform editor.
@MainActor final class KitTapOutside:NSObject,UIGestureRecognizerDelegate {
    private let field:()->CGRect?
    private let end:()->Void
    private weak var view:UIView?
    init(on view:UIView,field:@escaping ()->CGRect?,end:@escaping ()->Void) {
        self.field=field;self.end=end;self.view=view
        super.init()
        let tap=UITapGestureRecognizer(target:self,action:#selector(tapped(_:)))
        tap.cancelsTouchesInView=false;tap.delaysTouchesBegan=false;tap.delaysTouchesEnded=false;tap.delegate=self
        view.addGestureRecognizer(tap)
    }
    @objc private func tapped(_ tap:UITapGestureRecognizer) {
        guard tap.state == .ended,let view else {return}
        if KitFocus.tapEndsEditing(at:tap.location(in:view),field:field()) {end()}
    }
    func gestureRecognizer(_ g:UIGestureRecognizer,shouldRecognizeSimultaneouslyWith other:UIGestureRecognizer)->Bool {true}
}
