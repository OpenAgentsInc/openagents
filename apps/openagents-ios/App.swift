// Firm reason: 6 (UIKit lifecycle, the keyboard and text input, and VoiceOver).
// The OpenAgents iOS app (components/gpu-components.md; GitHub #13645): one screen, the chat page.
// The page is compiled Coax (chat.coax) drawn by the shared GPU component host in one Metal view.
// Nothing runs in JavaScript. This file owns only what a GPU surface cannot:
//   - the display link, which sleeps while nothing moves and stops in the background;
//   - the platform editor: one UITextView over the composer's text while it holds the keyboard;
//   - VoiceOver: an element per semantic row; the Metal view itself is hidden from accessibility;
//   - a tap outside the focused field ends editing (KitFocus), and the page hears focus end.
import UIKit

@main final class OpenAgentsApp:UIResponder,UIApplicationDelegate {
    var window:UIWindow?
    func application(_ application:UIApplication,didFinishLaunchingWithOptions options:[UIApplication.LaunchOptionsKey:Any]?=nil)->Bool {
        let window=UIWindow(frame:UIScreen.main.bounds);window.rootViewController=ChatScreen();window.makeKeyAndVisible();self.window=window;return true
    }
}

@MainActor final class ChatScreen:UIViewController,UITextViewDelegate {
    private var owner:UnsafeMutableRawPointer?
    private var canvas:UIView?
    private var display:CADisplayLink?
    private var keyboard:CGFloat=0
    private var stamp:Int32 = -1
    private var rowIDs:[String]=[]
    private var said=""
    /// The composer field's row, for the tap that ends editing (KitTapOutside).
    private var fieldBox=CGRect.null
    private var tapOutside:KitTapOutside?
    private lazy var editor:ComposerEditor={var s=[Float](repeating:0,count:14);_ = oc_style(&s);return ComposerEditor(style:s)}()

    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black;view.accessibilityIdentifier="chat.screen"
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({oc_open($0)}),let raw=oc_view(owner) else {
            let label=UILabel();label.text="OpenAgents could not start. Restart the app.";label.numberOfLines=0;label.textAlignment = .center;label.textColor = .label
            label.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(label)
            NSLayoutConstraint.activate([label.centerYAnchor.constraint(equalTo:view.centerYAnchor),label.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:24),label.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-24)]);return
        }
        self.owner=owner
        // A touch lands on the Metal view, not here: the surface says when one changed something.
        oc_on_change(owner,{ context in
            guard let context else {return}
            let screen=Unmanaged<ChatScreen>.fromOpaque(context).takeUnretainedValue()
            MainActor.assumeIsolated {screen.wake()}
        },Unmanaged.passUnretained(self).toOpaque())
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();self.canvas=canvas
        canvas.frame=view.bounds;canvas.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(canvas)
        canvas.isAccessibilityElement=false;canvas.accessibilityElementsHidden=true
        // A tap outside the focused field gives up the keyboard; the surface still gets the tap.
        tapOutside=KitTapOutside(on:canvas,field:{ [weak self] in guard let self,self.editor.isFirstResponder else {return nil};return self.fieldBox },end:{ [weak self] in _ = self?.editor.resignFirstResponder() })
        editor.delegate=self;editor.host=view;editor.attach={ [weak self] in self?.attachEditor() };editor.activate={ [weak self] in self?.focus() ?? false }
        let center=NotificationCenter.default
        center.addObserver(self,selector:#selector(keyboardMoved(_:)),name:UIResponder.keyboardWillChangeFrameNotification,object:nil)
        center.addObserver(self,selector:#selector(keyboardHid),name:UIResponder.keyboardWillHideNotification,object:nil)
        center.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(wake),name:UIApplication.didBecomeActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(wake),name:UIAccessibility.reduceMotionStatusDidChangeNotification,object:nil)
    }
    override func viewDidAppear(_ animated:Bool) {super.viewDidAppear(animated);wake()}
    override func viewDidLayoutSubviews() {super.viewDidLayoutSubviews();wake()}
    override func viewSafeAreaInsetsDidChange() {super.viewSafeAreaInsetsDidChange();wake()}
    override var preferredStatusBarStyle:UIStatusBarStyle {.lightContent}

    // MARK: frames

    /// Something changed: draw, and keep drawing while the page says it moves.
    @objc func wake() {
        guard owner != nil,viewIfLoaded?.window != nil,UIApplication.shared.applicationState != .background else {return}
        if display==nil {
            let link=CADisplayLink(target:ChatTick(self),selector:#selector(ChatTick.tick));link.preferredFrameRateRange=CAFrameRateRange(minimum:30,maximum:60,preferred:60)
            link.add(to:.main,forMode:.common);display=link
        }
        display?.isPaused=false
    }
    @objc private func background() {display?.invalidate();display=nil}
    fileprivate func step() {
        guard let owner else {return}
        let insets=view.safeAreaInsets,editing=editor.isFirstResponder
        oc_env(owner,Float(insets.top),Float(insets.bottom),Float(keyboard),editing ? 1:0,editing ? Float(editor.textHeight(width:editor.bounds.width)):0,UIAccessibility.isReduceMotionEnabled ? 1:0)
        oc_frame(owner)
        if oc_action(owner)==1 {_ = focus()}
        placeEditor();refreshRows()
        // A static page sleeps: no frame runs until something wakes it.
        if oc_moving(owner)==0 {display?.isPaused=true}
    }

    // MARK: the platform editor

    private func box()->CGRect? {
        guard let owner else {return nil}
        var b=[Float](repeating:0,count:6)
        guard oc_editor(owner,&b) != 0 else {return nil}
        return CGRect(x:CGFloat(b[0]),y:CGFloat(b[1]),width:CGFloat(b[2]),height:CGFloat(b[3]))
    }
    private func attachEditor() {
        if editor.superview==nil {view.addSubview(editor)}
        if let frame=box() {editor.frame=frame}
    }
    private func placeEditor() {
        guard let owner else {return}
        // A sent draft is cleared by the page: the editor follows when it has no marked text.
        var bytes=[CChar](repeating:0,count:17000);_ = oc_text(owner,&bytes,Int32(bytes.count));let held=String(cString:bytes)
        if editor.markedTextRange==nil,editor.text != held {editor.set(held)}
        guard editor.superview != nil,let frame=box() else {return}
        if editor.frame != frame {editor.frame=frame}
        editor.isScrollEnabled=editor.textHeight(width:frame.width)+12>frame.height+1
    }
    @discardableResult private func focus()->Bool {attachEditor();let took=editor.becomeFirstResponder();wake();return took}
    func textViewDidBeginEditing(_ textView:UITextView) {wake()}
    func textViewDidEndEditing(_ textView:UITextView) {editor.removeFromSuperview();wake()}
    func textViewDidChange(_ textView:UITextView) {if let owner {_ = oc_set_text(owner,textView.text ?? "")};wake()}
    @objc private func keyboardMoved(_ note:Notification) {
        guard let end=(note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,let window=view.window else {return}
        let inView=view.convert(end,from:window.screen.coordinateSpace)
        keyboard=max(0,view.bounds.maxY-inView.minY);wake()
    }
    @objc private func keyboardHid() {keyboard=0;wake()}

    // MARK: VoiceOver

    /// Builds the accessibility elements from the committed semantic rows when any row changed.
    private func refreshRows() {
        guard let owner else {return}
        let now=oc_rows_stamp(owner)
        guard now != stamp else {return}
        stamp=now
        var elements:[Any]=[],names:[String]=[],notice=""
        for i in 0..<oc_rows(owner) {
            var row=[Float](repeating:0,count:7),id=[CChar](repeating:0,count:64),label=[CChar](repeating:0,count:256)
            guard oc_row(owner,i,&row,&id,Int32(id.count),&label,Int32(label.count)) != 0 else {continue}
            let name=String(cString:id),words=String(cString:label),state=Int(row[5]),role=Int(row[4])
            let rect=CGRect(x:CGFloat(row[0]),y:CGFloat(row[1]),width:CGFloat(row[2]),height:CGFloat(row[3]))
            names.append(name)
            if role==3 {
                // The field is the platform editor: over the text while it has the keyboard, else standing for the field's row.
                editor.box=rect;fieldBox=rect;editor.accessibilityIdentifier=name;editor.accessibilityLabel=words;editor.placeholder=words
                elements.append(editor);continue
            }
            let element=ChatRow(accessibilityContainer:view!)
            element.accessibilityIdentifier=name;element.accessibilityLabel=words;element.accessibilityFrameInContainerSpace=rect
            var traits:UIAccessibilityTraits=role==0 ? .button:.staticText
            if role==9 {traits.insert(.header)}
            if state & 1 != 0 {traits.insert(.notEnabled)}
            if state & 1024 != 0 {notice=words}
            element.accessibilityTraits=traits
            if role==0 {element.activate={ [weak self] in guard let self,let owner=self.owner else {return false};let pressed=oc_activate(owner,name) != 0;self.wake();return pressed }}
            elements.append(element)
        }
        view.accessibilityElements=elements
        if names != rowIDs {rowIDs=names;UIAccessibility.post(notification:.layoutChanged,argument:nil)}
        if notice != said {said=notice;if !notice.isEmpty {UIAccessibility.post(notification:.announcement,argument:notice)}}
    }
}

/// The display link's target, so the link does not keep the screen alive.
@MainActor private final class ChatTick:NSObject {
    weak var screen:ChatScreen?
    init(_ screen:ChatScreen) {self.screen=screen}
    @objc func tick() {screen?.step()}
}

/// A semantic row as VoiceOver sees it. Activation is the page's own press.
private final class ChatRow:UIAccessibilityElement {
    var activate:(()->Bool)?
    override func accessibilityActivate()->Bool {activate?() ?? false}
}
