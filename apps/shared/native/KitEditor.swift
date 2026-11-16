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
